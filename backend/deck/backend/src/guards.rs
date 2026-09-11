//! Landmine guards (spec §7) — the backend refuses unsafe operations.
//!
//! Every guard is fail-closed: until its probe is implemented, it reports
//! "not verifiable", which callers must treat as unsafe.

use crate::error::ApiError;
use axum::http::StatusCode;
use serde::Serialize;

/// Landmine #2 input: is the FUSE mount at `/mnt/remote/nzbdav` healthy?
///
/// M0/M1: the mount probe (`probes::mount_health`) is live and read-only, but
/// no mutation yet consumes this guard; `plex/empty-trash` still trips it with
/// 409 `mount_unhealthy` — the red-path test in `tests/api_contract.rs` pins
/// this behavior until the empty-trash flow lands in M3.
#[doc = "features: [\"nzbd.mount\"]"]
pub async fn mount_healthy() -> Result<bool, ApiError> {
    // TODO(M3): used by the empty-trash flow — resolve via probes::mount_health.
    Ok(false)
}

/// Result of the landmine-#4 queue guard: the queue is safe to recreate only
/// when `empty` is true. `count` and the raw slot summary ride along so the
/// two-step dialog can show the live queue that blocked the apply.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueGuard {
    pub empty: bool,
    pub count: usize,
    pub summary: Vec<QueueSlotSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSlotSummary {
    pub id: String,
    pub title: String,
    pub size_mb: Option<f64>,
    pub status: String,
}

/// Landmine #4: nzbdav queue must be empty before any recreate or any apply
/// whose blast radius touches `nzbdav`.
///
/// Live since M2: authenticated `GET /api?mode=queue&output=json` via the
/// frontend API key (`FRONTEND_BACKEND_API_KEY`). Fail-closed: if the probe is
/// unreachable the guard errors (`queue_guard_unavailable`) rather than
/// allowing the recreate.
pub async fn nzbdav_queue_guard(runtime: &crate::probes::Runtime) -> Result<QueueGuard, ApiError> {
    let probe = crate::probes::nzbdav_queue(runtime).await;
    let Some(data) = probe.data.as_ref() else {
        let detail = probe
            .error
            .clone()
            .unwrap_or_else(|| "no queue payload".to_string());
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "queue_guard_unavailable",
            format!("nzbdav queue probe failed — refusing recreates until the queue is verifiable (landmine #4): {detail}"),
        ));
    };
    if !probe.reachable {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "queue_guard_unavailable",
            format!(
                "nzbdav queue probe returned HTTP {} — refusing recreates (landmine #4)",
                probe.status.unwrap_or(0)
            ),
        ));
    }

    let slots = data
        .pointer("/queue/slots")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = slots
        .iter()
        .map(|slot| QueueSlotSummary {
            id: slot
                .get("nzo_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string(),
            title: slot
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("(untitled)")
                .to_string(),
            size_mb: slot.get("size").and_then(|v| v.as_f64()),
            status: slot
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string(),
        })
        .collect::<Vec<_>>();

    Ok(QueueGuard {
        empty: summary.is_empty(),
        count: summary.len(),
        summary,
    })
}

/// Landmine #13: the triple nzbdav health check (frontend /healthz +
/// authenticated queue API + PROPFIND) — "healthy" only when all three pass.
pub async fn nzbdav_triple_healthy() -> Result<bool, ApiError> {
    // TODO(M3): used by the safe-recreate flow's verification step.
    Ok(false)
}
