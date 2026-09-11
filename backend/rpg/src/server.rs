//! Axum HTTP surface (spec §7.3, session mechanics finalized 2026-09-08):
//! the PIN gate plus the first gated API routes, on port 46532.
//!
//! Sessions are opaque 128-bit random tokens in an `rpg_session` HttpOnly
//! cookie (Path=/, SameSite=Lax), stored server-side in memory with a
//! 7-day expiry (restart clears all sessions — accepted for V1). Every
//! route except `GET /healthz`, `GET /auth/status`, `POST /auth/set-pin`,
//! and `POST /auth/login` requires a valid session cookie (401 otherwise).

use crate::auth;
use crate::persistence::PostgresContentStore;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const SESSION_COOKIE: &str = "rpg_session";
pub const SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// F-10: failed-login throttle. A 4-digit PIN is 10,000 combinations, so
/// unthrottled on-LAN guessing is feasible even behind Argon2id. In-memory
/// is sufficient for the V1 single account (restart clears it; the gate
/// re-locks only after a successful login anyway).
const LOGIN_MAX_FAILURES: u32 = 5;
const LOGIN_BASE_LOCKOUT: Duration = Duration::from_secs(30);

/// Shared handler state: one pooled store, one session store.
#[derive(Clone)]
pub struct AppState {
    store: Arc<PostgresContentStore>,
    sessions: Arc<Mutex<SessionStore>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct LoginFailureState {
    failures: u32,
    locked_until: Option<Instant>,
}

/// In-memory failed-login limiter. V1 intentionally resets this state on
/// restart, like sessions; keys come from the request's best available
/// attempt source (forwarded IP, real IP, or an anonymous fallback).
#[derive(Default)]
struct LoginLimiter {
    attempts: HashMap<String, LoginFailureState>,
}

impl LoginLimiter {
    fn is_locked_at(&self, source: &str, now: Instant) -> bool {
        self.attempts
            .get(source)
            .and_then(|state| state.locked_until)
            .is_some_and(|until| now < until)
    }

    fn record_failure_at(&mut self, source: &str, now: Instant) {
        let state = self.attempts.entry(source.to_owned()).or_default();
        state.failures = state.failures.saturating_add(1);
        if state.failures >= LOGIN_MAX_FAILURES {
            let exponent = state.failures.saturating_sub(LOGIN_MAX_FAILURES).min(8);
            state.locked_until = Some(
                now + LOGIN_BASE_LOCKOUT
                    .checked_mul(1_u32 << exponent)
                    .expect("login lockout duration fits"),
            );
        }
    }

    fn record_success(&mut self, source: &str) {
        self.attempts.remove(source);
    }
}

/// Server-side session table: token → expiry instant.
#[derive(Default)]
pub struct SessionStore {
    sessions: HashMap<String, Instant>,
    /// F-10: failed logins are keyed by attempt source rather than globally,
    /// so one client cannot lock out unrelated clients.
    login_limiter: LoginLimiter,
}

impl SessionStore {
    /// Issues a fresh opaque 128-bit token with a full TTL. Every issue
    /// opportunistically sweeps expired sessions (F-11: the store can no
    /// longer grow without bound for the process lifetime).
    pub fn issue(&mut self) -> String {
        self.sweep_expired();
        let mut bytes = [0_u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex(&bytes);
        self.sessions.insert(token.clone(), Instant::now() + SESSION_TTL);
        token
    }

    /// True when the token maps to an unexpired session. F-39: an
    /// authenticated touch slides the expiry forward, so an actively played
    /// session does not hard-expire mid-run after 7 days.
    pub fn is_valid(&mut self, token: &str) -> bool {
        let now = Instant::now();
        let valid = self
            .sessions
            .get(token)
            .is_some_and(|expires_at| now < *expires_at);
        if valid {
            self.sessions.insert(token.to_owned(), now + SESSION_TTL);
        }
        valid
    }

    /// Deletes a session (logout); returns whether one existed.
    pub fn revoke(&mut self, token: &str) -> bool {
        self.sessions.remove(token).is_some()
    }

    /// Removes expired sessions; returns how many were dropped.
    pub fn sweep_expired(&mut self) -> usize {
        let now = Instant::now();
        let before = self.sessions.len();
        self.sessions.retain(|_, expires_at| now < *expires_at);
        before - self.sessions.len()
    }

    /// F-10: whether a login attempt from this source is permitted right now.
    pub fn login_locked(&self, source: &str) -> bool {
        self.login_limiter.is_locked_at(source, Instant::now())
    }

    /// F-10: records a failed login for this attempt source.
    pub fn record_login_failure(&mut self, source: &str) {
        self.login_limiter
            .record_failure_at(source, Instant::now());
    }

    /// F-10: a successful login clears only this source's failure streak.
    pub fn record_login_success(&mut self, source: &str) {
        self.login_limiter.record_success(source);
    }

    #[cfg(test)]
    fn insert_expired_for_test(&mut self, token: &str) {
        self.sessions
            .insert(token.to_owned(), Instant::now() - Duration::from_secs(1));
    }

    #[cfg(test)]
    fn record_login_failure_at_for_test(&mut self, source: &str, now: Instant) {
        self.login_limiter.record_failure_at(source, now);
    }

    #[cfg(test)]
    fn login_locked_at_for_test(&self, source: &str, now: Instant) -> bool {
        self.login_limiter.is_locked_at(source, now)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

/// Extracts a cookie's value from a Cookie header (first match wins).
fn cookie_value(cookies: &str, name: &str) -> Option<String> {
    cookies.split(';').find_map(|pair| {
        let pair = pair.trim();
        let (key, value) = pair.split_once('=')?;
        (key.trim() == name).then(|| value.trim().to_owned())
    })
}

fn session_token_from(request: &Request) -> Option<String> {
    request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| cookie_value(cookies, SESSION_COOKIE))
}

/// Returns the best available attempt-source identifier without trusting it
/// for authentication. Reverse proxies should provide `X-Forwarded-For`.
fn login_source(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("anonymous")
        .to_owned()
}

// ---- Gate ----

async fn require_session(
    State(app): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let authenticated = session_token_from(&request).is_some_and(|token| {
        app.sessions
            .lock()
            .expect("session mutex poisoned")
            .is_valid(&token)
    });
    if authenticated {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "authentication required" })),
        )
            .into_response()
    }
}

// ---- Handlers ----

async fn healthz() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
pub struct PinBody {
    pub pin: String,
}

/// Enumerates the gate state for the frontend so it can render the right
/// first-run screen: locked (no account yet) vs ready-for-login.
async fn auth_status(State(app): State<AppState>) -> Response {
    match app.store.account_exists().await {
        Ok(account_exists) => Json(json!({ "locked": !account_exists })).into_response(),
        Err(error) => internal_error(error),
    }
}

async fn set_pin(State(app): State<AppState>, Json(body): Json<PinBody>) -> Response {
    // F-10: check account existence before validation/hashing. Existing
    // accounts receive the same response shape without paying Argon2 cost or
    // changing the stored PIN.
    match app.store.account_exists().await {
        Ok(true) => {
            return Json(json!({ "account_created": false })).into_response();
        }
        Ok(false) => {}
        Err(error) => return internal_error(error),
    }
    match app.store.set_account_pin(&body.pin).await {
        Ok((outcome, _bootstrap)) => {
            // F-29: first-run set-PIN lands the player directly in a
            // session — no forced second POST with the same PIN.
            let mut response = Json(json!({
                "account_created": outcome.account_created,
            }))
            .into_response();
            if outcome.account_created {
                let token = app.sessions.lock().expect("session mutex poisoned").issue();
                set_session_cookie(&mut response, &token);
            }
            response
        }
        Err(crate::ProbeError::InvalidPin) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "PIN must be 4-12 digits" })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn login(
    State(app): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PinBody>,
) -> Response {
    let source = login_source(&headers);
    {
        let sessions = app.sessions.lock().expect("session mutex poisoned");
        if sessions.login_locked(&source) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": "too many failed attempts; try again later" })),
            )
                .into_response();
        }
    }
    match app.store.verify_account_pin(&body.pin).await {
        Ok(Some(auth::PinVerifyOutcome::Accepted)) => {
            let token = {
                let mut sessions = app.sessions.lock().expect("session mutex poisoned");
                sessions.record_login_success(&source);
                sessions.issue()
            };
            let mut response = Json(json!({ "authenticated": true })).into_response();
            set_session_cookie(&mut response, &token);
            response
        }
        Ok(Some(auth::PinVerifyOutcome::Rejected)) => {
            app.sessions
                .lock()
                .expect("session mutex poisoned")
                .record_login_failure(&source);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "wrong PIN" })),
            )
                .into_response()
        }
        Ok(None) => {
            // No account yet: count it against the throttle too, so the
            // pre-account gate cannot be probed for free.
            app.sessions
                .lock()
                .expect("session mutex poisoned")
                .record_login_failure(&source);
            (StatusCode::UNAUTHORIZED, Json(json!({ "error": "locked" }))).into_response()
        }
        Err(error) => internal_error(error),
    }
}

/// Gated: deletes the server-side session and clears the cookie.
async fn logout(State(app): State<AppState>, request: Request) -> Response {
    if let Some(token) = session_token_from(&request) {
        app.sessions.lock().expect("session mutex poisoned").revoke(&token);
    }
    let mut response = Json(json!({ "authenticated": false })).into_response();
    clear_session_cookie(&mut response);
    response
}

async fn archetypes(State(app): State<AppState>) -> Response {
    match app.store.archetype_state().await {
        Ok(Some(state)) => Json(state).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no character" })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn select_archetype(
    State(app): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Response {
    match app.store.select_archetype(&slug).await {
        Ok(state) => Json(state).into_response(),
        Err(crate::ProbeError::ArchetypeSelectionConflict(reason)) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": reason })),
        )
            .into_response(),
        Err(crate::ProbeError::InvalidArchetypeSelection(reason)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": reason })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn character(State(app): State<AppState>) -> Response {
    match app.store.character_overview().await {
        Ok(Some(overview)) => Json(overview).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no character" })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

/// Badge wall (§5.5.13): every definition with unlock state and progress.
async fn achievements(State(app): State<AppState>) -> Response {
    match app.store.badge_wall().await {
        Ok(Some(entries)) => Json(entries).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no character" })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

/// The mystery order board (§5.7): locked items serialize as position +
/// locked only — no title, no content id.
async fn orders(State(app): State<AppState>) -> Response {
    match app.store.order_view().await {
        Ok(Some(orders)) => Json(orders).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no character" })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

/// The game tick's UI entry point (§9.1): ordered phases — watch award
/// (slot) → order reveals → achievement evaluation. The response is the
/// trimmed summary (F-34), not the internal tick report.
async fn orders_refresh(State(app): State<AppState>) -> Response {
    // V1: the UI-refresh entry runs the tick without the stack (the poll
    // loop owns stack access); phase 1 is skipped when plex is None.
    match crate::game::run_game_tick(&app.store, None).await {
        Ok(report) => Json(report.summary()).into_response(),
        Err(error) => internal_error(error),
    }
}

/// Spends a skip on the current item of one order (§5.7: finale excluded).
/// F-33: the refreshed order board comes back with the outcome, so the UI
/// does not flash stale state between skip and re-fetch.
async fn order_skip(
    State(app): State<AppState>,
    axum::extract::Path(order_id): axum::extract::Path<i64>,
) -> Response {
    let outcome = app.store.skip_order_item(order_id).await;
    match outcome {
        Ok(outcome) => match outcome {
            crate::persistence::SkipOutcome::Skipped { position } => {
                let orders = app.store.order_view().await.ok().flatten();
                Json(json!({
                    "skipped": true,
                    "position": position,
                    "orders": orders,
                }))
                .into_response()
            }
            crate::persistence::SkipOutcome::NoSkipsAvailable => (
                StatusCode::CONFLICT,
                Json(json!({ "error": "no skips available" })),
            )
                .into_response(),
            crate::persistence::SkipOutcome::FinaleNotSkippable => (
                StatusCode::CONFLICT,
                Json(json!({ "error": "the final item cannot be skipped" })),
            )
                .into_response(),
            crate::persistence::SkipOutcome::NothingToSkip => (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "nothing to skip in this order" })),
            )
                .into_response(),
        },
        Err(error) => internal_error(error),
    }
}

/// F-3: the level-gated genre transition (§5.2 cascade) gets an API
/// surface — without it, progression past the opening genre is unreachable
/// over HTTP. Errors mirror the archetype-select mapping.
async fn access_genre(
    State(app): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    match app.store.access_genre(&name).await {
        Ok(state) => Json(state).into_response(),
        Err(crate::ProbeError::InvalidGenreAccess(reason)) => {
            let conflict = matches!(
                reason.as_str(),
                "genre is already accessed"
                    | "genre is not next in the academy cascade"
                    | "genre access changed concurrently"
            );
            let status = if conflict {
                StatusCode::CONFLICT
            } else if reason == "level has not opened this genre" {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            (status, Json(json!({ "error": reason }))).into_response()
        }
        Err(error) => internal_error(error),
    }
}

fn internal_error(error: crate::ProbeError) -> Response {
    eprintln!("request failed: {error}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal error" })),
    )
        .into_response()
}

// ---- Cookie helpers ----

fn set_session_cookie(response: &mut Response, token: &str) {
    if let Ok(value) = HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        SESSION_TTL.as_secs()
    )) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

fn clear_session_cookie(response: &mut Response) {
    if let Ok(value) = HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
    )) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

// ---- Router ----

/// Builds the app router: public gate routes plus the session-gated API.
pub fn router(store: Arc<PostgresContentStore>) -> Router {
    let app = AppState {
        store,
        sessions: Arc::new(Mutex::new(SessionStore::default())),
    };

    let public = Router::new()
        .route("/healthz", get(healthz))
        .route("/auth/status", get(auth_status))
        .route("/auth/set-pin", post(set_pin))
        .route("/auth/login", post(login))
        .with_state(app.clone());

    let gated = Router::new()
        .route("/auth/logout", post(logout))
        .route("/api/character", get(character))
        .route("/api/archetypes", get(archetypes))
        .route("/api/archetypes/{slug}/select", post(select_archetype))
        .route("/api/genres/{name}/access", post(access_genre))
        .route("/api/achievements", get(achievements))
        .route("/api/orders", get(orders))
        .route("/api/orders/refresh", post(orders_refresh))
        .route("/api/orders/{id}/skip", post(order_skip))
        .layer(middleware::from_fn_with_state(app.clone(), require_session))
        .with_state(app);

    public.merge(gated)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::{
        cookie_value, SessionStore, SESSION_COOKIE, SESSION_TTL, LOGIN_BASE_LOCKOUT,
        LOGIN_MAX_FAILURES,
    };

    #[test]
    fn extracts_cookie_values_from_header_pairs() {
        assert_eq!(
            cookie_value("a=1; rpg_session=abc123; b=2", SESSION_COOKIE),
            Some("abc123".to_owned())
        );
        assert_eq!(cookie_value("rpg_session=xyz", SESSION_COOKIE), Some("xyz".to_owned()));
        assert_eq!(cookie_value("other=1", SESSION_COOKIE), None);
        assert_eq!(cookie_value("", SESSION_COOKIE), None);
        // First match wins on duplicates.
        assert_eq!(
            cookie_value("rpg_session=first; rpg_session=second", SESSION_COOKIE),
            Some("first".to_owned())
        );
    }

    #[test]
    fn sessions_issue_validate_and_revoke() {
        let mut sessions = SessionStore::default();
        assert!(sessions.is_empty());

        let token = sessions.issue();
        assert_eq!(token.len(), 32, "128-bit token is 32 hex chars");
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(sessions.len(), 1);
        assert!(sessions.is_valid(&token));

        assert!(sessions.revoke(&token));
        assert!(!sessions.is_valid(&token));
        assert!(!sessions.revoke(&token));
        assert!(sessions.is_empty());

        assert!(!sessions.is_valid("unknown-token"));
    }

    #[test]
    fn issued_tokens_are_unique() {
        let mut sessions = SessionStore::default();
        let first = sessions.issue();
        let second = sessions.issue();
        assert_ne!(first, second);
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn session_sweep_removes_expired_entries() {
        let mut sessions = SessionStore::default();
        let token = sessions.issue();
        sessions.insert_expired_for_test("expired");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions.sweep_expired(), 1);
        assert!(sessions.is_valid(&token));
        assert!(!sessions.is_valid("expired"));
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn login_limiter_is_source_scoped_and_uses_exponential_lockout() {
        use std::time::Duration;
        let mut sessions = SessionStore::default();
        let now = Instant::now();
        for _ in 0..LOGIN_MAX_FAILURES {
            sessions.record_login_failure_at_for_test("client-a", now);
        }
        assert!(sessions.login_locked_at_for_test("client-a", now));
        assert!(!sessions.login_locked_at_for_test("client-b", now));

        let after_first_lockout = now + LOGIN_BASE_LOCKOUT;
        assert!(!sessions.login_locked_at_for_test("client-a", after_first_lockout));
        sessions.record_login_failure_at_for_test("client-a", after_first_lockout);
        assert!(sessions.login_locked_at_for_test(
            "client-a",
            after_first_lockout + Duration::from_secs(1)
        ));
        sessions.record_login_success("client-a");
        assert!(!sessions.login_locked_at_for_test("client-a", after_first_lockout));
    }

    #[test]
    fn session_ttl_is_seven_days() {
        assert_eq!(SESSION_TTL.as_secs(), 7 * 24 * 60 * 60);
    }
}
