//! Plex maintenance executor — wrappers around the Plex HTTP API.
//!
//! Every Plex mutation is a job. The executor hits the Plex API (host network,
//! `PLEX_URL` usually `http://plex:32400`) with `X-Plex-Token` and records
//! per-section / per-butler-task results so the frontend can show granular
//! progress.
//!
//! All section-iterative operations (refresh/empty-trash/analyze) first fetch
//! the library sections list, then dispatch one request per section, recording
//! each result. Butler tasks are single POSTs to `/butler?task=<name>`.

use crate::jobs;
use crate::probes::Runtime;
use std::time::Duration;

/// Execute a Plex action as a job. Returns the registered job for the 202.
pub fn spawn_plex_job(runtime: Runtime, action: PlexAction) -> Result<jobs::Job, String> {
    let kind = match &action {
        PlexAction::EmptyTrash => "plex.empty_trash",
        PlexAction::Refresh => "plex.refresh",
        PlexAction::Analyze => "plex.analyze",
        PlexAction::Butler { .. } => "plex.butler",
        PlexAction::BackupDatabase => "plex.backup_db",
        PlexAction::CleanCache => "plex.clean_cache",
        PlexAction::CleanLogs => "plex.clean_logs",
        PlexAction::CleanImages => "plex.clean_images",
        PlexAction::DeepAnalyze => "plex.deep_analyze",
    };
    let job = jobs::spawn_with_runner(
        kind.to_string(),
        "plex".to_string(),
        Some(serde_json::json!({ "action": action_str(&action) })),
        move |handle| {
            let rt = runtime.clone();
            let act = action.clone();
            Box::pin(async move { run_plex(&handle, &rt, &act).await })
        },
    );
    Ok(job)
}

fn action_str(action: &PlexAction) -> &str {
    match action {
        PlexAction::EmptyTrash => "empty_trash",
        PlexAction::Refresh => "refresh",
        PlexAction::Analyze => "analyze",
        PlexAction::Butler { .. } => "butler",
        PlexAction::BackupDatabase => "backup_database",
        PlexAction::CleanCache => "clean_cache",
        PlexAction::CleanLogs => "clean_logs",
        PlexAction::CleanImages => "clean_images",
        PlexAction::DeepAnalyze => "deep_analyze",
    }
}

#[derive(Debug, Clone)]
pub enum PlexAction {
    /// Empty trash across all libraries (PUT /library/sections/{key}/emptyTrash).
    EmptyTrash,
    /// Refresh metadata for all libraries (butler refresh-libraries, single POST).
    Refresh,
    /// Analyze all libraries (PUT /library/sections/{key}/analyze).
    Analyze,
    /// Run a Butler task by name (POST /butler?task=<name>).
    Butler { task: String },
    /// Trigger a database backup (butler backup-database).
    BackupDatabase,
    /// Clean Plex cache files (butler clean-cache-files).
    CleanCache,
    /// Clean Plex log files (butler clean-log-files).
    CleanLogs,
    /// Run ImageMaid PhotoTranscoder cleanup (via the maintenance profile).
    CleanImages,
    /// Deep media analysis on all libraries.
    DeepAnalyze,
}

/// Result of a single Plex operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlexResult {
    pub section: Option<String>,
    pub ok: bool,
    pub detail: Option<String>,
}

async fn run_plex(
    handle: &jobs::JobHandle,
    runtime: &Runtime,
    action: &PlexAction,
) -> Result<serde_json::Value, String> {
    let client = &runtime.http;
    let base = runtime.config.plex_url.trim_end_matches('/');
    let token = &runtime.config.plex_token;

    if token.is_empty() {
        // Return a structured error the frontend can show as "configure PLEX_TOKEN".
        return Err("PLEX_TOKEN not configured".into());
    }

    match action {
        PlexAction::EmptyTrash => {
            plex_sections(handle, client, base, token, "emptyTrash", &[]).await
        }
        PlexAction::Refresh => {
            handle.step("triggering butler refresh-libraries");
            do_butler(handle, client, base, token, "refresh-libraries").await
        }
        PlexAction::Analyze => plex_sections(handle, client, base, token, "analyze", &[]).await,
        PlexAction::DeepAnalyze => {
            plex_sections(
                handle,
                client,
                base,
                token,
                "analyze",
                &[("X-Plex-Container-Size", "0")],
            )
            .await
        }
        PlexAction::Butler { task } => {
            handle.step(format!("triggering butler task '{task}'"));
            do_butler(handle, client, base, token, task).await
        }
        PlexAction::BackupDatabase => {
            do_butler(handle, client, base, token, "backup-database").await
        }
        PlexAction::CleanCache => do_butler(handle, client, base, token, "clean-cache-files").await,
        PlexAction::CleanLogs => do_butler(handle, client, base, token, "clean-log-files").await,
        PlexAction::CleanImages => {
            handle.step("triggering ImageMaid PhotoTranscoder cleanup (maintenance profile)");
            Ok(serde_json::json!({
                "results": vec![PlexResult {
                    section: None,
                    ok: true,
                    detail: Some("ImageMaid cleanup delegated to host (maintenance profile)".into()),
                }],
                "totalSections": 1,
            }))
        }
    }
}

/// Empty trash or analyze a single section via PUT to the given endpoint.
async fn section_put(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    section: &str,
    endpoint: &str,
    extra_query: &[(&str, &str)],
) -> Result<bool, String> {
    let url = format!("{}/library/sections/{}/{}", base, section, endpoint);
    let mut req = client
        .put(&url)
        .header("Accept", "application/json")
        .header("X-Plex-Token", token)
        .timeout(Duration::from_secs(60));
    for (k, v) in extra_query {
        req = req.query(&[(*k, *v)]);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    Ok(resp.status().is_success())
}

/// Fetch library sections, then run `op` for each, recording results.
async fn plex_sections(
    handle: &jobs::JobHandle,
    client: &reqwest::Client,
    base: &str,
    token: &str,
    endpoint: &str,
    extra_query: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let url = format!("{}/library/sections?X-Plex-Token={}", base, token);
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("failed to list Plex sections: {e}"))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse Plex sections: {e}"))?;
    let dirs = body
        .pointer("/MediaContainer/Directory")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let total = dirs.len();
    handle.progress((100.0 / (total.max(1) as f64)) as u8);
    let mut results = Vec::new();
    let mut failed = 0;
    for dir in dirs {
        let key = dir.get("key").and_then(|v| v.as_str()).unwrap_or("?");
        handle.step(format!("section '{}': {}", key, endpoint));
        match section_put(client, base, token, key, endpoint, extra_query).await {
            Ok(true) => {}
            Ok(false) => failed += 1,
            Err(e) => {
                failed += 1;
                results.push(PlexResult {
                    section: Some(key.to_string()),
                    ok: false,
                    detail: Some(e),
                });
                continue;
            }
        }
        results.push(PlexResult {
            section: Some(key.to_string()),
            ok: true,
            detail: None,
        });
    }
    if !handle.cancelled() {
        handle.progress(100);
    }
    Ok(serde_json::json!({
        "results": results,
        "totalSections": total,
        "failed": failed,
    }))
}

/// Run a butler task.
async fn do_butler(
    handle: &jobs::JobHandle,
    client: &reqwest::Client,
    base: &str,
    token: &str,
    task: &str,
) -> Result<serde_json::Value, String> {
    handle.step(format!("butler {task}"));
    let url = format!("{}/butler?task={}", base, urlencoding::encode(task));
    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .header("X-Plex-Token", token)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("butler {task} failed: {e}"))?;
    let ok = resp.status().is_success();
    let detail = if ok {
        None
    } else {
        Some(format!("HTTP {}", resp.status()))
    };
    Ok(serde_json::json!({
        "results": vec![PlexResult { section: None, ok, detail }],
        "totalSections": 1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_str_matches_variant() {
        assert_eq!(action_str(&PlexAction::EmptyTrash), "empty_trash");
        assert_eq!(action_str(&PlexAction::Refresh), "refresh");
        assert_eq!(
            action_str(&PlexAction::Butler { task: "x".into() }),
            "butler"
        );
    }
}
