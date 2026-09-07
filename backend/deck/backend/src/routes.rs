//! Route tree (spec Appendix D). M1 makes all read-only surfaces live against
//! Docker and the configured Bear Cave service APIs. Mutations remain jobs/stubs
//! until their later milestones.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::error::ApiError;
use crate::guards;
use crate::jobs;
use crate::probes::{self, Runtime};
use crate::ws;

fn todo(msg: &str) -> ApiError {
    ApiError::not_implemented(msg)
}

/// Health, version, dashboard snapshot, activity feed.
#[doc = "features: [\"dash.overview\", \"dash.rows\", \"dash.feed\"]"]
pub fn router_dash() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/dashboard", get(dashboard_snapshot))
        .route("/activity", get(activity))
}

async fn dashboard_snapshot(State(runtime): State<Arc<Runtime>>) -> Response {
    (StatusCode::OK, Json(probes::dashboard(&runtime).await)).into_response()
}

async fn activity(State(runtime): State<Arc<Runtime>>) -> Response {
    (StatusCode::OK, Json(probes::activity(&runtime).await)).into_response()
}

/// Container lifecycle + images. Mutations are jobs (§5.2).
#[doc = "features: [\"ctnr.lifecycle\", \"ctnr.images\"]"]
pub fn router_containers() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/containers", get(containers))
        .route("/containers/{id}", get(container_detail))
        .route("/containers/{id}/{action}", post(container_action))
        .route("/containers/{id}/logs", get(container_logs))
}

async fn containers(State(runtime): State<Arc<Runtime>>) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "containers": probes::containers(&runtime).await })),
    )
        .into_response()
}

async fn container_detail(State(runtime): State<Arc<Runtime>>, Path(id): Path<String>) -> Response {
    let all = probes::containers(&runtime).await;
    match all.into_iter().find(|container| container.id == id) {
        Some(container) => (StatusCode::OK, Json(container)).into_response(),
        None => ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown_container",
            format!("unknown container '{id}'"),
        )
        .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    tail: Option<usize>,
}

async fn container_logs(
    State(runtime): State<Arc<Runtime>>,
    Path(id): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Response {
    let tail = query.tail.unwrap_or(100).clamp(1, 2_000);
    (
        StatusCode::OK,
        Json(probes::docker_logs(&runtime, &id, tail).await),
    )
        .into_response()
}

async fn container_action(Path((id, action)): Path<(String, String)>) -> Response {
    let known = ["start", "stop", "restart", "recreate"];
    if !known.contains(&action.as_str()) {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown_action",
            format!("unknown container action '{action}'"),
        )
        .into_response();
    }
    match jobs::spawn(format!("container.{action}"), id) {
        Ok(j) => (StatusCode::ACCEPTED, Json(json!({ "jobId": j.id }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Merged Sonarr/Radarr queue, backlog, decisions.
#[doc = "features: [\"stick.queue\", \"stick.backlog\", \"stick.decide\"]"]
pub fn router_stick() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/stick/queue", get(stick_queue))
        .route("/stick/queue/{itemId}/decide", post(stick_decide))
}

async fn stick_queue(State(runtime): State<Arc<Runtime>>) -> Response {
    let (sonarr, radarr) = tokio::join!(
        probes::arr_queue(
            &runtime,
            &runtime.config.sonarr_url,
            &runtime.config.sonarr_key
        ),
        probes::arr_queue(
            &runtime,
            &runtime.config.radarr_url,
            &runtime.config.radarr_key
        ),
    );
    (
        StatusCode::OK,
        Json(json!({ "sonarr": sonarr, "radarr": radarr })),
    )
        .into_response()
}

async fn stick_decide() -> Response {
    todo("import decision backend lands in M3").into_response()
}

/// nzbdav + FUSE mount (landmine #2/#4/#13 surface).
#[doc = "features: [\"nzbd.queue\", \"nzbd.history\", \"nzbd.stats\", \"nzbd.mount\", \"nzbd.dedup\", \"nzbd.deletefail\"]"]
pub fn router_nzbdav() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/nzbdav/queue", get(nzbdav_queue))
        .route("/nzbdav/history", get(nzbdav_history))
        .route("/nzbdav/stats", get(nzbdav_stats))
        .route("/nzbdav/dedup-check", post(nzbdav_dedup_check))
        .route("/nzbdav/delete-failures", post(nzbdav_delete_failures))
        .route("/mount/health", get(mount_health))
}

async fn nzbdav_queue(State(runtime): State<Arc<Runtime>>) -> Response {
    let probe = probes::nzbdav_queue(&runtime).await;
    (StatusCode::OK, Json(probe)).into_response()
}

async fn nzbdav_history(State(runtime): State<Arc<Runtime>>) -> Response {
    let probe = probes::nzbdav_history(&runtime, 100).await;
    (StatusCode::OK, Json(probe)).into_response()
}

async fn nzbdav_stats(State(runtime): State<Arc<Runtime>>) -> Response {
    (StatusCode::OK, Json(probes::nzbdav_stats(&runtime).await)).into_response()
}

async fn nzbdav_dedup_check() -> Response {
    todo("dedup scan (heavy FUSE I/O job) lands in M3").into_response()
}
async fn nzbdav_delete_failures() -> Response {
    todo("delete-failures (destructive-confirm) lands in M3").into_response()
}

async fn mount_health(State(runtime): State<Arc<Runtime>>) -> Response {
    (StatusCode::OK, Json(probes::mount_health(&runtime).await)).into_response()
}

/// Plex suite — empty-trash carries the landmine-#2 mount guard.
#[doc = "features: [\"plex.sessions\", \"plex.maintenance\"]"]
pub fn router_plex() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/plex/sessions", get(plex_sessions))
        .route("/plex/empty-trash", post(plex_empty_trash))
}

async fn plex_sessions(State(runtime): State<Arc<Runtime>>) -> Response {
    (StatusCode::OK, Json(probes::plex_sessions(&runtime).await)).into_response()
}

async fn plex_empty_trash() -> Response {
    match guards::mount_healthy().await {
        Ok(true) => todo("empty-trash lands in M3").into_response(),
        Ok(false) => ApiError::guard_tripped(
            "mount_unhealthy",
            "FUSE mount degraded or unverified — resolve before rescanning (landmine #2)",
        )
        .into_response(),
        Err(e) => e.into_response(),
    }
}

/// Credentials & API sources.
#[doc = "features: [\"cred.indexers\"]"]
pub fn router_credentials() -> Router<Arc<Runtime>> {
    Router::new().route("/indexers", get(indexers))
}
async fn indexers(State(runtime): State<Arc<Runtime>>) -> Response {
    let url = format!(
        "{}/api/v1/indexer",
        runtime.config.prowlarr_url.trim_end_matches('/')
    );
    (
        StatusCode::OK,
        Json(
            probes::json_get(
                &runtime,
                &url,
                &[("X-Api-Key", &runtime.config.prowlarr_key)],
            )
            .await,
        ),
    )
        .into_response()
}

/// `.env` engine.
#[doc = "features: [\"env\"]"]
pub fn router_env() -> Router<Arc<Runtime>> {
    Router::new().route("/env/apply", post(env_apply))
}
async fn env_apply() -> Response {
    todo("prompted-apply cascade lands in M2 (queue guard wired then)").into_response()
}

/// Catalog & deployments.
#[doc = "features: [\"catalog\"]"]
pub fn router_catalog() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/catalog", get(catalog))
        .route("/catalog/conflicts", post(catalog_conflicts))
        .route("/catalog/{id}", get(catalog_entry))
        .route("/catalog/{id}/install", post(catalog_install))
        .route("/catalog/{id}/uninstall", post(catalog_uninstall))
        .route("/deployments", get(deployments))
        .route("/deployments/{id}/logs", get(deployment_logs))
}

async fn catalog() -> Response {
    crate::catalog::ensure_loaded();
    match crate::catalog::get() {
        Some(doc) => (StatusCode::OK, Json(doc)).into_response(),
        None => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "catalog_invalid",
            "catalog failed validation — refusing to serve",
        )
        .into_response(),
    }
}
async fn catalog_entry(Path(id): Path<String>) -> Response {
    crate::catalog::ensure_loaded();
    match crate::catalog::entry(&id) {
        Some(e) => (StatusCode::OK, Json(e)).into_response(),
        None => ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown_entry",
            format!("no catalog entry '{id}'"),
        )
        .into_response(),
    }
}
async fn catalog_conflicts(Json(draft): Json<crate::catalog::DraftInstall>) -> Response {
    crate::catalog::ensure_loaded();
    (
        StatusCode::OK,
        Json(json!({ "conflicts": crate::catalog::check_conflicts(&draft) })),
    )
        .into_response()
}
async fn catalog_install(Path(id): Path<String>) -> Response {
    crate::catalog::ensure_loaded();
    if crate::catalog::entry(&id).is_none() {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown_entry",
            format!("no catalog entry '{id}'"),
        )
        .into_response();
    }
    match jobs::spawn(format!("catalog.install.{id}"), id) {
        Ok(j) => (StatusCode::ACCEPTED, Json(json!({ "jobId": j.id }))).into_response(),
        Err(e) => e.into_response(),
    }
}
async fn catalog_uninstall(Path(id): Path<String>) -> Response {
    crate::catalog::ensure_loaded();
    if crate::catalog::entry(&id).is_none() {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown_entry",
            format!("no catalog entry '{id}'"),
        )
        .into_response();
    }
    match jobs::spawn(format!("catalog.uninstall.{id}"), id) {
        Ok(j) => (StatusCode::ACCEPTED, Json(json!({ "jobId": j.id }))).into_response(),
        Err(e) => e.into_response(),
    }
}
async fn deployments() -> Response {
    todo("deployment history lands in M4 (SQLite-backed)").into_response()
}
async fn deployment_logs(Path(id): Path<String>) -> Response {
    todo(&format!(
        "deployment step logs land in M4 (deployment '{id}' unknown until then)"
    ))
    .into_response()
}

/// Host tools (shim) — M1 exposes the read-only host snapshot.
#[doc = "features: [\"host.disk\", \"host.mem\", \"host.journal\", \"host.services\", \"host.pkg\", \"host.aur\", \"host.btrfs\", \"host.smart\", \"host.reboot\", \"host.cron\", \"host.git\", \"host.firewall\", \"host.ssh\", \"host.uptime\", \"host.backup\", \"host.drift\", \"host.residue\", \"host.perms\"]"]
pub fn router_host() -> Router<Arc<Runtime>> {
    Router::new().route("/host/overview", get(host_overview))
}
async fn host_overview() -> Response {
    (StatusCode::OK, Json(probes::host_overview().await)).into_response()
}

/// Notifications.
#[doc = "features: [\"notif.discord\", \"notif.digest\", \"notif.test\"]"]
pub fn router_notifications() -> Router<Arc<Runtime>> {
    Router::new().route("/notifications", get(notifications))
}
async fn notifications() -> Response {
    todo("discord/email lands in M6").into_response()
}

/// Library & lists.
#[doc = "features: [\"lib.health\", \"lib.lists\", \"lib.prune\", \"lib.watchable\"]"]
pub fn router_library() -> Router<Arc<Runtime>> {
    Router::new().route("/library", get(library))
}
async fn library() -> Response {
    todo("library tools land in M3/M5").into_response()
}

/// Job engine surface.
pub fn router_jobs() -> Router<Arc<Runtime>> {
    Router::new()
        .route("/jobs", get(jobs_list))
        .route("/jobs/{id}/cancel", post(jobs_cancel))
}
async fn jobs_list() -> Response {
    (StatusCode::OK, Json(json!({ "jobs": jobs::list().await }))).into_response()
}
async fn jobs_cancel() -> Response {
    todo("job cancellation lands in M1").into_response()
}

pub async fn router() -> Router {
    let runtime = Arc::new(probes::runtime().await);
    let api = Router::new()
        .merge(router_dash())
        .merge(router_containers())
        .merge(router_stick())
        .merge(router_nzbdav())
        .merge(router_plex())
        .merge(router_credentials())
        .merge(router_env())
        .merge(router_catalog())
        .merge(router_host())
        .merge(router_notifications())
        .merge(router_library())
        .merge(router_jobs())
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/version", get(version))
        .route("/ws", get(ws::handler))
        .with_state(runtime);
    Router::new().nest("/api/v1", api)
}

async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}
async fn readyz(State(runtime): State<Arc<Runtime>>) -> Response {
    let docker = runtime.docker.is_some();
    let response = if docker {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        response,
        Json(json!({ "ready": docker, "checks": { "docker": docker, "config": true } })),
    )
        .into_response()
}
async fn version() -> Response {
    (
        StatusCode::OK,
        Json(json!({ "cave_deck": env!("CARGO_PKG_VERSION"), "stack": "live probes (M1)" })),
    )
        .into_response()
}
