//! Container lifecycle / compose executor (spec §6.2).
//!
//! This is the only path to container mutations: every action is a job, the
//! runner executes it, and the HTTP handler returns `202 {jobId}`.
//!
//! ### Dependency ordering
//!
//! The Bear Cave compose wires these service relationships, and the executor
//! reproduces them so recreates land in a safe order:
//!
//!   - `nzbdav_rclone` depends_on `nzbdav`                    (service_healthy)
//!   - `radarr`, `sonarr`, `seerr`, `plex`, `unpackerr` -> nzbdav_rclone
//!
//! A recreate of `nzbdav` therefore cascades: nzbdav → nzbdav_rclone → then
//! radarr/sonarr/seerr/plex/unpackerr (each only if the service exists in the
//! stack). A solitary `start/stop/restart` of one container bypasses the cascade.

use crate::docker_helpers;
use crate::jobs;
use crate::probes::Runtime;
use bollard::container::{RemoveContainerOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

/// Resolve the project root so the executor can read/write `.env`, run scripts,
/// and call `docker compose` in the right directory.
fn project_root() -> Option<PathBuf> {
    let marker = ".env";
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe
        .parent()
        .filter(|p| p.as_ref() != std::path::Path::new("/"))?;
    loop {
        if dir.join(marker).exists() {
            return Some(dir);
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => return None,
        }
    }
}

/// Ordered service ids in the config file. This is the source of truth for
/// dependency decisions; containers that aren't in the stack aren't managed
/// here.
pub static STACK_SERVICES: &[&str] = &[
    "prowlarr",
    "radarr",
    "sonarr",
    "nzbdav",
    "nzbdav_rclone",
    "seerr",
    "plex",
    "unpackerr",
];

/// service_healthy dependency graph (from docker-compose.yml depends_on blocks).
fn depends_on_targets(service: &str) -> BTreeSet<&str> {
    match service {
        // consumer of nzbdav_rclone (FUSE mount / config rewrite)
        "radarr" | "sonarr" | "seerr" | "plex" | "unpackerr" => {
            ["nzbdav_rclone"].into_iter().collect()
        }
        "nzbdav_rclone" => ["nzbdav"].into_iter().collect(),
        _ => BTreeSet::new(),
    }
}

/// Services that depend on `service` — used by the cascade recipe.
fn dependents_of(service: &str) -> BTreeSet<&str> {
    STACK_SERVICES
        .iter()
        .filter(|s| *s != service && depends_on_targets(*s).contains(&service))
        .copied()
        .collect()
}

/// A compose-aware action that the executor runs on a single container.
#[derive(Debug, Clone)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Recreate,
    Remove,
}

use ContainerAction::*;

/// Run `action` on `service`. Returns the executor run metadata for the job.
pub async fn run_container(
    handle: &jobs::JobHandle,
    service: &str,
    action: ContainerAction,
) -> Result<RunSnapshot, String> {
    handle.step(format!("{action} {service}"));
    let client = client()?;
    let svc = action.service_id(service);
    let started = std::time::Instant::now();
    match action {
        Start => start(client, service).await?,
        Stop => stop(client, service).await?,
        Restart => restart(client, service).await?,
        Recreate => recreate(client, service).await?,
        Remove => remove(client, service).await?,
    }
    let duration_ms = started.elapsed().as_millis() as i64;
    Ok(RunSnapshot {
        id: service.to_string(),
        action,
        started_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        finished_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        duration_ms: Some(duration_ms),
    })
}

impl ContainerAction {
    fn service_id(&self, service: &str) -> &str {
        service
    }
}

/// Snapshot recorded for each executor step for the job result + audit.
#[derive(Debug, serde::Serialize)]
pub struct RunSnapshot {
    pub id: String,
    pub action: ContainerAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

async fn start(client: bollard::Docker, service: &str) -> Result<(), String> {
    client
        .start_container(service, None::<StartContainerOptions>)
        .await
        .map_err(|e| format!("docker.start {service}: {}", e))?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}

async fn stop(client: bollard::Docker, service: &str) -> Result<(), String> {
    client
        .stop_container(
            service,
            Some(StopContainerOptions { t: 90 }),
        )
        .await
        .map_err(|e| format!("docker.stop {service}: {}", e))?;
    Ok(())
}

/// Restart is a Docker-level restart (container keeps its ID). For compose
/// services this is fine for a singular restart; recreates use `recreate`.
async fn restart(client: bollard::Docker, service: &str) -> Result<(), String> {
    client
        .restart_container(service, Some(RestartContainerOptions { t: 90 }))
        .await
        .map_err(|e| format!("docker.restart {service}: {}", e))?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}

/// Recreate a single compose service: stop → remove → docker compose up -d
/// (the compose side re-creates it from the image/tag in the config).
async fn recreate(client: bollard::Docker, service: &str) -> Result<(), String> {
    let force = RemoveContainerOptions {
        force: true,
        v: false,
        link: false,
    };
    // Best-effort hygiene: stop first so remove doesn't have to force-kill.
    let _ = stop(client.clone(), service).await;
    client
        .remove_container(service, Some(force))
        .await
        .map_err(|e| format!("docker.rm {service}: {}", e))?;

    // Recreate via docker compose up -d (compose reads the service def from the
    // config, so this rebuilds from the pinned image/tag rather than from the
    // now-removed container).
    compose_up(vec![service.to_string()]).await?;
    Ok(())
}

/// Remove (without volumes) — best-effort stop first.
async fn remove(client: bollard::Docker, service: &str) -> Result<(), String> {
    let _ = stop(client.clone(), service).await;
    let force = RemoveContainerOptions {
        force: true,
        v: false,
        link: false,
    };
    client
        .remove_container(service, Some(force))
        .await
        .map_err(|e| format!("docker.rm {service}: {}", e))?;
    Ok(())
}

/// Run `docker compose up -d <services...>` in the stack directory.
async fn compose_up(services: Vec<String>) -> Result<(), String> {
    let stack_dir = project_root().ok_or_else(|| "cannot locate project root (no .env found)".to_string())?;
    let mut cmd = tokio::process::Command::new("docker");
    cmd.current_dir(&stack_dir)
        .args(["compose", "up", "-d"])
        .args(&services)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("docker compose up failed to spawn: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "docker compose up -d {} failed: STDERR: {}\nSTDOUT: {}",
            services.join(" "), stderr, stdout
        ));
    }
    if !stdout_is_empty(&output.stdout) || !stderr_is_empty(&output.stderr) {
        tracing::info!(
            stdout = %String::from_utf8_lossy(&output.stdout),
            stderr = %String::from_utf8_lossy(&output.stderr),
            "docker compose up finished"
        );
    }
    Ok(())
}

fn stdout_is_empty(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

fn stderr_is_empty(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

/// Build the ordered list of services to recreate when `root` is recreated
/// (cascade). Returns (root, then direct dependents in topological order).
pub fn cascade_for(root: &str) -> Vec<String> {
    let mut order = vec![root.to_string()];
    let deps = dependents_of(root);
    for dep in STACK_SERVICES.iter().filter(|s| deps.contains(**s)) {
        order.push(dep.to_string());
    }
    order
}

/// Wait for a compose service to become running, up to `timeout_secs`.
pub async fn wait_healthy(service: &str, timeout_secs: u64) -> Result<(bool, String), String> {
    let client = client()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let inspect = client.inspect_container(service, None).await;
        let status = match inspect {
            Ok(i) => i
                .state
                .and_then(|s| s.status)
                .unwrap_or_else(|| "unknown".into()),
            Err(e) => return Err(format!("inspect {service}: {}", e)),
        };
        if status == "running" {
            return Ok((true, status));
        }
        if !deadline.saturating_duration_since(tokio::time::Instant::now()).is_zero() {
            return Ok((false, status));
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Return the project root path, or an error if `.env` can't be found.
pub fn stack_dir() -> Result<PathBuf, String> {
    project_root().ok_or_else(|| "cannot locate project root (no .env found)".to_string())
}

/// Spawn a container lifecycle job and return the registered job for the 202.
pub fn spawn_container_job(
    runtime: Runtime,
    service: String,
    action: ContainerAction,
) -> Result<jobs::Job, String> {
    // Validate the service is one we know about.
    if !STACK_SERVICES.contains(&service.as_str()) {
        return Err(format!("unknown service '{service}'"));
    }
    let action_str = match &action {
        ContainerAction::Start => "start",
        ContainerAction::Stop => "stop",
        ContainerAction::Restart => "restart",
        ContainerAction::Recreate => "recreate",
        ContainerAction::Remove => "remove",
    };
    let job = jobs::spawn_with_runner(
        format!("container.{action_str}"),
        service.clone(),
        Some(serde_json::json!({ "service": service, "action": action_str })),
        move |handle| {
            let rt = runtime.clone();
            let svc = service.clone();
            let act = action.clone();
            Box::pin(async move {
                let docker = match docker_helpers::runtime_docker(&rt) {
                    Some(d) => d,
                    None => return Err("docker not available".into()),
                };
                let mut snapshots = Vec::new();
                // Single-container actions: just run it.
                let snap = run_container(&handle, &svc, act).await?;
                snapshots.push(snap);
                if handle.cancelled() {
                    return Err("cancelled".into());
                }
                // For recreates, also restart dependents in the cascade if they exist.
                if matches!(act, ContainerAction::Recreate) {
                    let cascade = crate::executor::cascade_for(&svc);
                    for dep in cascade.iter().skip(1) {
                        if handle.cancelled() {
                            break;
                        }
                        let dep_exists = docker
                            .inspect_container(dep, None)
                            .await
                            .map(|r| r.id.is_some())
                            .unwrap_or(false);
                        if !dep_exists {
                            continue;
                        }
                        let dep_snap = crate::executor::run_container(&handle, dep, ContainerAction::Restart).await?;
                        snapshots.push(dep_snap);
                    }
                }
                let total_ms: i64 = snapshots.iter().map(|s| s.duration_ms.unwrap_or(0)).sum();
                Ok(serde_json::json!({
                    "snapshots": snapshots,
                    "totalDurationMs": total_ms,
                }))
            })
        },
    );
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cascade_starts_with_root() {
        let c = cascade_for("nzbdav");
        assert_eq!(c[0], "nzbdav");
    }

    #[test]
    fn cascade_includes_rclone_after_nzbdav() {
        let c = cascade_for("nzbdav");
        assert!(c.contains(&"nzbdav_rclone".to_string()));
    }

    #[test]
    fn cascade_does_not_include_prowlarr_for_nzbdav() {
        let c = cascade_for("nzbdav");
        assert!(!c.contains(&"prowlarr".to_string()));
    }

    #[test]
    fn depends_on_targets_for_radarr_is_rclone() {
        assert_eq!(
            depends_on_targets("radarr").iter().copied().collect::<Vec<_>>(),
            vec!["nzbdav_rclone"]
        );
    }
}
