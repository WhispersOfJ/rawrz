//! Route tree (spec Appendix D). Each handler group documents the feature IDs it
//! serves via `#[doc = "features: [...]"]`; `scripts/check_api_contract.py`
//! scrapes these declarations and asserts full parity coverage (§D.4/D.6).

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::error::ApiError;
use crate::guards;
use crate::jobs;
use crate::ws;

fn todo(msg: &str) -> ApiError {
    ApiError::not_implemented(msg)
}

/// Health, version, dashboard snapshot, activity feed.
#[doc = "features: [\"dash.overview\", \"dash.rows\", \"dash.feed\"]"]
pub fn router_dash() -> Router {
    Router::new()
        .route("/dashboard", get(dashboard_snapshot))
        .route("/activity", get(activity))
}

async fn dashboard_snapshot() -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "containers": [],
            "mount": { "healthy": false, "probed": false },
            "queues": { "sonarr": null, "radarr": null, "nzbdav": null },
            "disk": { "free_bytes": null },
            "note": "M0 skeleton — data plumbing lands in M1"
        })),
    )
        .into_response()
}

async fn activity() -> Response {
    todo("activity feed (dash.feed) lands in M1 — SQLite events table first").into_response()
}

/// Container lifecycle + images. Mutations are jobs (§5.2).
#[doc = "features: [\"ctnr.lifecycle\", \"ctnr.images\"]"]
pub fn router_containers() -> Router {
    Router::new()
        .route("/containers", get(containers))
        .route("/containers/{id}/{action}", post(container_action))
}

async fn containers() -> Response {
    todo("docker listing (bollard) lands in M1").into_response()
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
pub fn router_stick() -> Router {
    Router::new()
        .route("/stick/queue", get(stick_queue))
        .route("/stick/queue/{itemId}/decide", post(stick_decide))
}

async fn stick_queue() -> Response {
    todo("merged queue view lands in M1").into_response()
}

async fn stick_decide() -> Response {
    todo("import decision backend lands in M3").into_response()
}

/// nzbdav + FUSE mount (landmine #2/#4/#13 surface).
#[doc = "features: [\"nzbd.queue\", \"nzbd.history\", \"nzbd.stats\", \"nzbd.mount\", \"nzbd.dedup\", \"nzbd.deletefail\"]"]
pub fn router_nzbdav() -> Router {
    Router::new()
        .route("/nzbdav/queue", get(nzbdav_queue))
        .route("/nzbdav/dedup-check", post(nzbdav_dedup_check))
        .route("/nzbdav/delete-failures", post(nzbdav_delete_failures))
        .route("/mount/health", get(mount_health))
}

async fn nzbdav_dedup_check() -> Response {
    todo("dedup scan (heavy FUSE I/O job) lands in M3").into_response()
}

async fn nzbdav_delete_failures() -> Response {
    todo("delete-failures (destructive-confirm) lands in M3").into_response()
}

async fn nzbdav_queue() -> Response {
    todo("nzbdav SAB-style queue lands in M1").into_response()
}

async fn mount_health() -> Response {
    (
        StatusCode::OK,
        Json(json!({ "mountpoint": "/mnt/remote/nzbdav", "healthy": false, "probed": false })),
    )
        .into_response()
}

/// Plex suite — empty-trash carries the landmine-#2 mount guard.
#[doc = "features: [\"plex.sessions\", \"plex.maintenance\"]"]
pub fn router_plex() -> Router {
    Router::new()
        .route("/plex/sessions", get(plex_sessions))
        .route("/plex/empty-trash", post(plex_empty_trash))
}

async fn plex_sessions() -> Response {
    todo("plex sessions lands in M1").into_response()
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
pub fn router_credentials() -> Router {
    Router::new().route("/indexers", get(indexers))
}

async fn indexers() -> Response {
    todo("prowlarr indexer view lands in M2").into_response()
}

/// `.env` engine.
#[doc = "features: [\"env\"]"]
pub fn router_env() -> Router {
    Router::new().route("/env/apply", post(env_apply))
}

async fn env_apply() -> Response {
    todo("prompted-apply cascade lands in M2 (queue guard wired then)").into_response()
}

/// Catalog & deployments (spec §6.3 / Appendix D). Serves the curated catalog
/// from the compiled-in `catalog/catalog.yaml`; install/uninstall flows are
/// jobs (PR-based, per §6.3) and land with M4's live compose probes.
#[doc = "features: [\"catalog\"]"]
pub fn router_catalog() -> Router {
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
    let conflicts = crate::catalog::check_conflicts(&draft);
    // 200 with a report either way: `conflicts: []` means the draft is clean.
    (StatusCode::OK, Json(json!({ "conflicts": conflicts }))).into_response()
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

/// Host tools (shim) — one router, all host.* IDs declared once.
#[doc = "features: [\"host.disk\", \"host.mem\", \"host.journal\", \"host.services\", \"host.pkg\", \"host.aur\", \"host.btrfs\", \"host.smart\", \"host.reboot\", \"host.cron\", \"host.git\", \"host.firewall\", \"host.ssh\", \"host.uptime\", \"host.backup\", \"host.drift\", \"host.residue\", \"host.perms\"]"]
pub fn router_host() -> Router {
    Router::new().route("/host/overview", get(host_overview))
}

async fn host_overview() -> Response {
    todo("host diagnostics land in M1 (shim required)").into_response()
}

/// Notifications.
#[doc = "features: [\"notif.discord\", \"notif.digest\", \"notif.test\"]"]
pub fn router_notifications() -> Router {
    Router::new().route("/notifications", get(notifications))
}

async fn notifications() -> Response {
    todo("discord/email lands in M6").into_response()
}

/// Library & lists.
#[doc = "features: [\"lib.health\", \"lib.lists\", \"lib.prune\", \"lib.watchable\"]"]
pub fn router_library() -> Router {
    Router::new().route("/library", get(library))
}

async fn library() -> Response {
    todo("library tools land in M3/M5").into_response()
}

/// Job engine surface.
pub fn router_jobs() -> Router {
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
        .route("/ws", get(ws::handler));

    Router::new().nest("/api/v1", api)
}

async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

async fn readyz() -> Response {
    // Honest readiness: dependency probes land in M1.
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "ready": false, "checks": [], "note": "dependency probes land in M1" })),
    )
        .into_response()
}

async fn version() -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "cave_deck": env!("CARGO_PKG_VERSION"),
            "stack": "compose-derived in M1"
        })),
    )
        .into_response()
}
