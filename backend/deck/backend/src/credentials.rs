//! Key rotation + Plex token verify (spec §6.4, Appendix D `cred.*` routes).
//!
//! Rotation is honest about what the pinned upstream apps actually accept:
//! - **Radarr/Sonarr/Prowlarr** (`*arr`): `GET {api}/config/host` → set a new
//!   32-hex `apiKey` → `PUT` the full resource back (the controller persists
//!   the whole dictionary, so we echo the GET body with only `apiKey`
//!   changed), authenticated with the *old* key; then verify with the new one.
//! - **Seerr**: `POST /api/v1/settings/main/regenerate` regenerates server-side
//!   and the admin response includes the new `apiKey` (the X-API-Key path
//!   authenticates as the original admin).
//!
//! Every rotation stages the new value into the `.env` draft; the actual write
//! completes through `POST /env/apply` (EXC confirm, landmine-#4 guard) so the
//! app key change and the `.env` change can't drift apart.

use crate::error::ApiError;
use crate::probes::Runtime;
use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;

/// Which app owns a rotatable key. `*arr`-style keys are pushed by echoing the
/// host-config resource with a new `apiKey`; `seerr` regenerates server-side.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationKind {
    Arr,
    Seerr,
}

#[derive(Debug, Clone, Copy)]
pub struct RotationTarget {
    pub key: &'static str, // .env var name, e.g. RADARR_API_KEY
    pub app: &'static str, // human name, e.g. radarr
    pub kind: RotationKind,
}

/// The four keys this milestone rotates. Plex (claim flow) and nzbdav keys are
/// handled separately; the rest of `cred.*` (indexer/list-source) is out of
/// this milestone's scope.
pub const ROTATABLE: &[RotationTarget] = &[
    RotationTarget {
        key: "RADARR_API_KEY",
        app: "radarr",
        kind: RotationKind::Arr,
    },
    RotationTarget {
        key: "SONARR_API_KEY",
        app: "sonarr",
        kind: RotationKind::Arr,
    },
    RotationTarget {
        key: "PROWLARR_API_KEY",
        app: "prowlarr",
        kind: RotationKind::Arr,
    },
    RotationTarget {
        key: "SEERR_API_KEY",
        app: "seerr",
        kind: RotationKind::Seerr,
    },
];

pub fn target_for(key: &str) -> Option<&RotationTarget> {
    ROTATABLE.iter().find(|t| t.key == key)
}

/// Result of a rotation: what was pushed, the staged `.env` draft state, and
/// the blast radius the apply flow will need. Secret values are never echoed
/// back; `staged_key` + `masked` signal presence only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationResult {
    pub key: String,
    pub app: String,
    /// True when the app accepted the new key and a verify call with it 2xx'd.
    pub rotated: bool,
    /// True when the new value was staged into the .env draft (apply pending).
    pub staged: bool,
    pub consumers: Vec<String>,
    /// Error from the push/verify phase, if any (rotation may still have
    /// staged the draft when the push failed — the UI must decide).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_error: Option<String>,
    pub note: &'static str,
}

fn new_hex_key() -> String {
    // 16 bytes from /dev/urandom → 32 hex chars (same shape as the *arr
    // GUID-without-dashes). Linux-only stack, so the device is present.
    let mut buf = [0u8; 16];
    match std::fs::File::open("/dev/urandom").and_then(|mut f| {
        use std::io::Read;
        f.read_exact(&mut buf)
    }) {
        Ok(()) => buf.iter().map(|b| format!("{b:02x}")).collect(),
        Err(_) => {
            // Fallback for sandboxed tests without /dev/urandom (never the
            // live host): time + pid + address entropy, still 32 hex chars.
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .hash(&mut h);
            std::process::id().hash(&mut h);
            format!("{:016x}{:016x}", h.finish(), h.finish() ^ 0xDEADBEEF)
        }
    }
}

/// Push a new key to the owning app. `old_key` authenticates the request; the
/// `*arr` path echoes the host-config resource with `apiKey` replaced.
async fn push_arr_key(
    runtime: &Runtime,
    base: &str,
    api_base: &str,
    old_key: &str,
    new_key: &str,
) -> Result<(), String> {
    let base = base.trim_end_matches('/');
    let get_url = format!("{base}{api_base}/config/host");
    let probe = crate::probes::json_get(runtime, &get_url, &[("X-Api-Key", old_key)]).await;
    let resource = probe.data.ok_or_else(|| {
        format!(
            "GET {get_url} failed (reachable={}, status={:?}, error={:?})",
            probe.reachable, probe.status, probe.error
        )
    })?;

    let mut body = resource;
    body["apiKey"] = json!(new_key);

    let put_url = format!("{base}{api_base}/config/host");
    let response = runtime
        .http
        .put(&put_url)
        .header("X-Api-Key", old_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("PUT {put_url}: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("PUT {put_url} returned HTTP {status}"));
    }

    // Verify with the new key.
    let verify_url = format!("{base}{api_base}/system/status");
    let verify = crate::probes::json_get(runtime, &verify_url, &[("X-Api-Key", new_key)]).await;
    if verify.reachable && verify.status.is_some_and(|s| s < 400) {
        Ok(())
    } else {
        Err(format!(
            "new key did not verify (reachable={}, status={:?})",
            verify.reachable, verify.status
        ))
    }
}

/// Rotate a key end-to-end: generate/push the new key to the app, then stage
/// the new value into the `.env` draft for the guarded apply flow.
pub async fn rotate(runtime: &Runtime, key: &str) -> Result<RotationResult, ApiError> {
    let target = target_for(key)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_rotatable", format!("'{key}' is not a rotatable key (expected one of RADARR/SONARR/PROWLARR/SEERR_API_KEY)")))?;

    let (base, old_key): (String, String) = match target.app {
        "radarr" => (
            runtime.config.radarr_url.clone(),
            runtime.config.radarr_key.clone(),
        ),
        "sonarr" => (
            runtime.config.sonarr_url.clone(),
            runtime.config.sonarr_key.clone(),
        ),
        "prowlarr" => (
            runtime.config.prowlarr_url.clone(),
            runtime.config.prowlarr_key.clone(),
        ),
        "seerr" => (
            runtime.config.seerr_url.clone(),
            runtime.config.seerr_key.clone(),
        ),
        _ => unreachable!(),
    };

    let mut push_error: Option<String> = None;
    let new_key = match target.kind {
        RotationKind::Arr => {
            let new_key = new_hex_key();
            let api_base = if target.app == "prowlarr" {
                "/api/v1"
            } else {
                "/api/v3"
            };
            if let Err(e) = push_arr_key(runtime, &base, api_base, &old_key, &new_key).await {
                push_error = Some(e);
            }
            new_key
        }
        RotationKind::Seerr => {
            // Server-side regenerate; admin response includes the new apiKey.
            let url = format!(
                "{}/api/v1/settings/main/regenerate",
                base.trim_end_matches('/')
            );
            match runtime
                .http
                .post(&url)
                .header("X-API-Key", &old_key)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => match res.json::<serde_json::Value>().await
                {
                    Ok(data) => match data.get("apiKey").and_then(|v| v.as_str()) {
                        Some(k) if !k.is_empty() => k.to_string(),
                        _ => {
                            push_error = Some("Seerr regenerate returned no apiKey".into());
                            old_key.clone()
                        }
                    },
                    Err(e) => {
                        push_error = Some(format!("Seerr regenerate response unparseable: {e}"));
                        old_key.clone()
                    }
                },
                Ok(res) => {
                    push_error = Some(format!("Seerr regenerate returned HTTP {}", res.status()));
                    old_key.clone()
                }
                Err(e) => {
                    push_error = Some(format!("Seerr regenerate request failed: {e}"));
                    old_key.clone()
                }
            }
        }
    };

    // Stage the new value into the .env draft regardless of push outcome so a
    // failed push is still visible in the apply preview (the UI shows the
    // push_error and lets the user choose).
    let mut draft = runtime.env.draft_snapshot().await;
    draft.values.insert(target.key.to_string(), new_key);
    runtime.env.set_draft(draft).await;

    Ok(RotationResult {
        key: target.key.to_string(),
        app: target.app.to_string(),
        rotated: push_error.is_none(),
        staged: true,
        consumers: crate::env::consumers_of(target.key),
        push_error,
        note: "new key staged into the .env draft — apply with POST /env/apply {\"confirm\":\"env\"} (queue guard runs when the blast radius touches nzbdav)",
    })
}

/// Verify the configured Plex token against the running server (`GET
/// /identity` with X-Plex-Token). Reachable+2xx = valid.
pub async fn plex_verify(runtime: &Runtime) -> serde_json::Value {
    let url = format!("{}/identity", runtime.config.plex_url.trim_end_matches('/'));
    let probe = crate::probes::json_get(
        runtime,
        &url,
        &[("X-Plex-Token", &runtime.config.plex_token)],
    )
    .await;
    let ok = probe.reachable && probe.status.is_some_and(|s| s < 400);
    json!({
        "url": runtime.config.plex_url,
        "token_set": !runtime.config.plex_token.is_empty(),
        "valid": ok,
        "reachable": probe.reachable,
        "status": probe.status,
        "error": probe.error,
    })
}

/// Stage a new PLEX_TOKEN into the .env draft (apply via /env/apply). The
/// first-run `PLEX_CLAIM` flow stays a compose-side step; this covers the
/// token itself.
pub async fn plex_token_update(
    runtime: &Runtime,
    token: &str,
) -> Result<serde_json::Value, ApiError> {
    if token.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_token",
            "PLEX_TOKEN must not be empty",
        ));
    }
    let mut draft = runtime.env.draft_snapshot().await;
    draft
        .values
        .insert("PLEX_TOKEN".to_string(), token.trim().to_string());
    runtime.env.set_draft(draft).await;
    Ok(json!({
        "staged": true,
        "key": "PLEX_TOKEN",
        "set": true,
        "consumers": crate::env::consumers_of("PLEX_TOKEN"),
        "note": "staged into the .env draft — apply with POST /env/apply {\"confirm\":\"env\"}",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_hex_key_is_32_hex_chars() {
        let key = new_hex_key();
        assert_eq!(key.len(), 32);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn rotatable_targets_cover_the_four_keys() {
        assert!(target_for("RADARR_API_KEY").is_some());
        assert!(target_for("SONARR_API_KEY").is_some());
        assert!(target_for("PROWLARR_API_KEY").is_some());
        assert!(target_for("SEERR_API_KEY").is_some());
        assert!(target_for("PLEX_TOKEN").is_none());
        assert!(target_for("NOPE").is_none());
    }
}
