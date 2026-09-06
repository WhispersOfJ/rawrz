//! Landmine guards (spec §7) — the backend refuses unsafe operations.
//!
//! Every guard is fail-closed: until its probe is implemented, it reports
//! "not verifiable", which callers must treat as unsafe.

use crate::error::ApiError;
use axum::http::StatusCode;

/// Landmine #2 input: is the FUSE mount at `/mnt/remote/nzbdav` healthy?
///
/// M0: no probe wired (rclone RC probing lands in M1) ⇒ fail-closed `false`.
/// `plex/empty-trash` returns 409 `mount_unhealthy` on this path — the red-path
/// test in `tests/api_contract.rs` pins this behavior.
#[doc = "features: [\"nzbd.mount\"]"]
pub async fn mount_healthy() -> Result<bool, ApiError> {
    // TODO(M1): mountpoint -q via shim + rclone RC vfs/stats + error counters.
    Ok(false)
}

/// Landmine #4: nzbdav queue must be empty before any recreate.
///
/// M0: fail-closed — no authenticated queue probe yet ⇒ refuses with the
/// live-queue explanation the two-step dialog will show.
pub async fn nzbdav_queue_empty() -> Result<bool, ApiError> {
    // TODO(M2): authenticated GET /api?mode=queue&output=json via frontend key.
    Err(ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "queue_guard_unavailable",
        "nzbdav queue probe not wired (M2) — refusing recreates until then (landmine #4)",
    ))
}

/// Landmine #13: the triple nzbdav health check (frontend /healthz +
/// authenticated queue API + PROPFIND) — "healthy" only when all three pass.
pub async fn nzbdav_triple_healthy() -> Result<bool, ApiError> {
    // TODO(M3): used by the safe-recreate flow's verification step.
    Ok(false)
}
