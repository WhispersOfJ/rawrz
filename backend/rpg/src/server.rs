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

/// Shared handler state: one store, one session store.
#[derive(Clone)]
pub struct AppState {
    store: Arc<tokio::sync::Mutex<PostgresContentStore>>,
    sessions: Arc<Mutex<SessionStore>>,
}

/// Server-side session table: token → expiry instant.
#[derive(Default)]
pub struct SessionStore {
    sessions: HashMap<String, Instant>,
}

impl SessionStore {
    /// Issues a fresh opaque 128-bit token with a full TTL.
    pub fn issue(&mut self) -> String {
        let mut bytes = [0_u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex(&bytes);
        self.sessions.insert(token.clone(), Instant::now() + SESSION_TTL);
        token
    }

    /// True when the token maps to an unexpired session.
    pub fn is_valid(&self, token: &str) -> bool {
        self.sessions
            .get(token)
            .is_some_and(|expires_at| Instant::now() < *expires_at)
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

// ---- Gate ----

async fn require_session(
    State(app): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let authenticated = session_token_from(&request)
        .is_some_and(|token| app.sessions.lock().expect("session mutex poisoned").is_valid(&token));
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
    let account_exists = app
        .store
        .lock()
        .await
        .account_exists()
        .await
        .unwrap_or(false);
    Json(json!({ "locked": !account_exists })).into_response()
}

async fn set_pin(State(app): State<AppState>, Json(body): Json<PinBody>) -> Response {
    match app.store.lock().await.set_account_pin(&body.pin).await {
        Ok((outcome, _bootstrap)) => Json(json!({
            "account_created": outcome.account_created,
        }))
        .into_response(),
        Err(crate::ProbeError::InvalidPin) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "PIN must be 4-12 digits" })),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn login(State(app): State<AppState>, Json(body): Json<PinBody>) -> Response {
    match app.store.lock().await.verify_account_pin(&body.pin).await {
        Ok(Some(auth::PinVerifyOutcome::Accepted)) => {
            let token = app.sessions.lock().expect("session mutex poisoned").issue();
            let mut response = Json(json!({ "authenticated": true })).into_response();
            set_session_cookie(&mut response, &token);
            response
        }
        Ok(Some(auth::PinVerifyOutcome::Rejected)) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "wrong PIN" })),
        )
            .into_response(),
        Ok(None) => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "locked" }))).into_response(),
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

async fn character(State(app): State<AppState>) -> Response {
    match app.store.lock().await.character_overview().await {
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
    match app.store.lock().await.badge_wall().await {
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
    match app.store.lock().await.order_view().await {
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
/// (slot) → order reveals → achievement evaluation.
async fn orders_refresh(State(app): State<AppState>) -> Response {
    // V1: the UI-refresh entry runs the tick without the stack (the poll
    // loop owns stack access); phase 1 is skipped when plex is None.
    match crate::game::run_game_tick(&mut *app.store.lock().await, None).await {
        Ok(report) => Json(report).into_response(),
        Err(error) => internal_error(error),
    }
}

/// Spends a skip on the current item of one order (§5.7: finale excluded).
async fn order_skip(
    State(app): State<AppState>,
    axum::extract::Path(order_id): axum::extract::Path<i64>,
) -> Response {
    match app.store.lock().await.skip_order_item(order_id).await {
        Ok(outcome) => match outcome {
            crate::persistence::SkipOutcome::Skipped { position } => {
                Json(json!({ "skipped": true, "position": position })).into_response()
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
pub fn router(store: Arc<tokio::sync::Mutex<PostgresContentStore>>) -> Router {
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
    use super::{cookie_value, SessionStore, SESSION_COOKIE, SESSION_TTL};

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
    fn session_ttl_is_seven_days() {
        assert_eq!(SESSION_TTL.as_secs(), 7 * 24 * 60 * 60);
    }
}
