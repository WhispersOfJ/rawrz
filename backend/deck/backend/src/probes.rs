//! M1 read-only probes for the Bear Cave runtime.
//!
//! All integrations are best-effort: a single unavailable dependency is
//! represented in the response rather than making the dashboard unavailable.
//! Secrets are only used in request headers/query parameters and never
//! serialized into responses or logs.

use bollard::query_parameters::{
    ListContainersOptionsBuilder, LogsOptionsBuilder, StatsOptionsBuilder,
};
use bollard::Docker;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

const DEFAULT_TIMEOUT_SECS: u64 = 8;
const CONTAINER_NAMES: [&str; 8] = [
    "prowlarr",
    "radarr",
    "sonarr",
    "nzbdav",
    "nzbdav_rclone",
    "seerr",
    "plex",
    "unpackerr",
];

#[derive(Clone)]
pub struct Runtime {
    pub docker: Option<Docker>,
    pub http: Client,
    pub config: Config,
    /// `.env` engine state (paths + staged draft).
    pub env: crate::env::EnvState,
}

#[derive(Clone)]
pub struct Config {
    pub radarr_url: String,
    pub radarr_key: String,
    pub sonarr_url: String,
    pub sonarr_key: String,
    pub prowlarr_url: String,
    pub prowlarr_key: String,
    pub seerr_url: String,
    pub seerr_key: String,
    pub plex_url: String,
    pub plex_token: String,
    pub nzbdav_url: String,
    pub nzbdav_key: String,
    pub rclone_url: String,
    pub rclone_user: String,
    pub rclone_pass: String,
    pub mountpoint: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            radarr_url: value("RADARR_URL", "http://radarr:7878"),
            radarr_key: value("RADARR_API_KEY", ""),
            sonarr_url: value("SONARR_URL", "http://sonarr:8989"),
            sonarr_key: value("SONARR_API_KEY", ""),
            prowlarr_url: value("PROWLARR_URL", "http://prowlarr:9696"),
            prowlarr_key: value("PROWLARR_API_KEY", ""),
            seerr_url: value("SEERR_URL", "http://seerr:5055"),
            seerr_key: value("SEERR_API_KEY", ""),
            plex_url: value("PLEX_URL", "http://plex:32400"),
            plex_token: value("PLEX_TOKEN", ""),
            nzbdav_url: value("NZBDAV_URL", "http://nzbdav:3000"),
            nzbdav_key: value("FRONTEND_BACKEND_API_KEY", ""),
            rclone_url: value("RCLONE_URL", "http://nzbdav_rclone:5572"),
            rclone_user: value("NZBDAV_RCLONE_RC_USER", "rclone"),
            rclone_pass: value("NZBDAV_RCLONE_RC_PASS", ""),
            mountpoint: value("NZBDAV_MOUNTPOINT", "/mnt/remote/nzbdav"),
        }
    }
}

fn value(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

pub async fn runtime() -> Runtime {
    let docker = Docker::connect_with_local_defaults().ok();
    let http = Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .build()
        .expect("HTTP client configuration is valid");
    Runtime {
        docker,
        http,
        config: Config::from_env(),
        env: crate::env::EnvState::from_env(),
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSnapshot {
    pub id: String,
    pub name: String,
    pub status: String,
    pub health: String,
    pub image: Option<String>,
    pub created_at: Option<i64>,
    pub restart_count: Option<i64>,
    pub ports: Vec<String>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub memory_limit: Option<u64>,
    pub network_rx_bytes: Option<u64>,
    pub network_tx_bytes: Option<u64>,
    pub error: Option<String>,
}

impl ContainerSnapshot {
    fn unavailable(name: &str, error: impl Into<String>) -> Self {
        Self {
            id: name.to_string(),
            name: name.to_string(),
            status: "unavailable".into(),
            health: "unknown".into(),
            image: None,
            created_at: None,
            restart_count: None,
            ports: Vec::new(),
            cpu_percent: None,
            memory_bytes: None,
            memory_limit: None,
            network_rx_bytes: None,
            network_tx_bytes: None,
            error: Some(error.into()),
        }
    }
}

pub async fn containers(runtime: &Runtime) -> Vec<ContainerSnapshot> {
    let Some(docker) = &runtime.docker else {
        return CONTAINER_NAMES
            .iter()
            .map(|name| ContainerSnapshot::unavailable(name, "Docker socket unavailable"))
            .collect();
    };

    let options = ListContainersOptionsBuilder::new().all(true).build();
    let listed = docker.list_containers(Some(options)).await;
    let Ok(items) = listed else {
        let error = listed.unwrap_err().to_string();
        return CONTAINER_NAMES
            .iter()
            .map(|name| ContainerSnapshot::unavailable(name, error.clone()))
            .collect();
    };

    let mut snapshots = Vec::with_capacity(CONTAINER_NAMES.len());
    for expected in CONTAINER_NAMES {
        let found = items.iter().find(|container| {
            container.names.as_ref().is_some_and(|names| {
                names
                    .iter()
                    .any(|name| name.trim_start_matches('/') == expected)
            })
        });
        let Some(container) = found else {
            snapshots.push(ContainerSnapshot::unavailable(
                expected,
                "Container not found",
            ));
            continue;
        };
        let name = container
            .names
            .as_ref()
            .and_then(|names| names.first())
            .map(|name| name.trim_start_matches('/'))
            .unwrap_or(expected)
            .to_string();
        let health = container
            .status
            .as_deref()
            .and_then(parse_health)
            .unwrap_or_else(|| "unknown".into());
        let stats = docker
            .stats(
                &name,
                Some(
                    StatsOptionsBuilder::new()
                        .stream(false)
                        .one_shot(true)
                        .build(),
                ),
            )
            .next()
            .await;
        let stat_values = stats.and_then(Result::ok).map(|stat| {
            let value = serde_json::to_value(stat).unwrap_or(Value::Null);
            let cpu = docker_cpu_percent(&value);
            let memory = value.pointer("/memory_stats/usage").and_then(Value::as_u64);
            let limit = value.pointer("/memory_stats/limit").and_then(Value::as_u64);
            let (rx, tx) = docker_network_totals(&value);
            (cpu, memory, limit, rx, tx)
        });
        snapshots.push(ContainerSnapshot {
            id: expected.to_string(),
            name: name.clone(),
            status: container
                .state
                .as_ref()
                .map(|state| format!("{state:?}").to_lowercase())
                .unwrap_or_else(|| "unknown".into()),
            health,
            image: container.image.clone(),
            created_at: container.created,
            restart_count: docker
                .inspect_container(
                    &name,
                    None::<bollard::query_parameters::InspectContainerOptions>,
                )
                .await
                .ok()
                .and_then(|inspect| serde_json::to_value(inspect).ok())
                .and_then(|value| value.get("RestartCount").and_then(Value::as_i64)),
            ports: container
                .ports
                .as_ref()
                .map(|ports| {
                    ports
                        .iter()
                        .filter_map(|port| {
                            port.public_port
                                .map(|public| format!("{}:{}", public, port.private_port))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            cpu_percent: stat_values.as_ref().map(|v| v.0),
            memory_bytes: stat_values.as_ref().and_then(|v| v.1),
            memory_limit: stat_values.as_ref().and_then(|v| v.2),
            network_rx_bytes: stat_values.as_ref().map(|v| v.3),
            network_tx_bytes: stat_values.map(|v| v.4),
            error: None,
        });
    }
    snapshots
}

fn parse_health(status: &str) -> Option<String> {
    status
        .split_once("(")
        .and_then(|(_, rest)| rest.strip_suffix(")"))
        .map(str::to_lowercase)
}

fn docker_cpu_percent(stat: &Value) -> f64 {
    let cpu_delta = stat
        .pointer("/cpu_stats/cpu_usage/total_usage")
        .and_then(Value::as_u64)
        .unwrap_or_default()
        .saturating_sub(
            stat.pointer("/precpu_stats/cpu_usage/total_usage")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        ) as f64;
    let system_delta = stat
        .pointer("/cpu_stats/system_cpu_usage")
        .and_then(Value::as_u64)
        .unwrap_or_default()
        .saturating_sub(
            stat.pointer("/precpu_stats/system_cpu_usage")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        ) as f64;
    let online = stat
        .pointer("/cpu_stats/online_cpus")
        .and_then(Value::as_u64)
        .unwrap_or(1) as f64;
    if system_delta == 0.0 {
        0.0
    } else {
        (cpu_delta / system_delta) * online * 100.0
    }
}

fn docker_network_totals(stat: &Value) -> (u64, u64) {
    stat.get("networks")
        .and_then(Value::as_object)
        .map(|networks| {
            networks.values().fold((0_u64, 0_u64), |(rx, tx), network| {
                (
                    rx.saturating_add(
                        network
                            .get("rx_bytes")
                            .and_then(Value::as_u64)
                            .unwrap_or_default(),
                    ),
                    tx.saturating_add(
                        network
                            .get("tx_bytes")
                            .and_then(Value::as_u64)
                            .unwrap_or_default(),
                    ),
                )
            })
        })
        .unwrap_or_default()
}

#[derive(Debug, Serialize, Clone)]
pub struct HttpProbe<T> {
    pub reachable: bool,
    pub status: Option<u16>,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> HttpProbe<T> {
    fn error(error: impl Into<String>) -> Self {
        Self {
            reachable: false,
            status: None,
            data: None,
            error: Some(error.into()),
        }
    }
}

pub async fn json_get(runtime: &Runtime, url: &str, headers: &[(&str, &str)]) -> HttpProbe<Value> {
    let mut request = runtime.http.get(url);
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let code = status.as_u16();
            match response.json::<Value>().await {
                Ok(data) if status.is_success() => HttpProbe {
                    reachable: true,
                    status: Some(code),
                    data: Some(data),
                    error: None,
                },
                Ok(data) => HttpProbe {
                    reachable: true,
                    status: Some(code),
                    data: Some(data),
                    error: Some(format!("HTTP {code}")),
                },
                Err(error) => HttpProbe {
                    reachable: true,
                    status: Some(code),
                    data: None,
                    error: Some(error.to_string()),
                },
            }
        }
        Err(error) => HttpProbe::error(error.to_string()),
    }
}

pub async fn arr_queue(runtime: &Runtime, base: &str, key: &str) -> HttpProbe<Value> {
    let url = format!(
        "{}/api/v3/queue?page=1&pageSize=100",
        base.trim_end_matches('/')
    );
    json_get(runtime, &url, &[("X-Api-Key", key)]).await
}

pub async fn nzbdav_queue(runtime: &Runtime) -> HttpProbe<Value> {
    let url = format!(
        "{}/api?mode=queue&output=json&apikey={}",
        runtime.config.nzbdav_url.trim_end_matches('/'),
        urlencoding::encode(&runtime.config.nzbdav_key)
    );
    json_get(runtime, &url, &[]).await
}

pub async fn nzbdav_history(runtime: &Runtime, limit: u32) -> HttpProbe<Value> {
    let url = format!(
        "{}/api?mode=history&output=json&limit={limit}&apikey={}",
        runtime.config.nzbdav_url.trim_end_matches('/'),
        urlencoding::encode(&runtime.config.nzbdav_key)
    );
    json_get(runtime, &url, &[]).await
}

pub async fn nzbdav_stats(runtime: &Runtime) -> serde_json::Value {
    let queue = nzbdav_queue(runtime).await;
    let history = nzbdav_history(runtime, 100).await;
    let slots = queue
        .data
        .as_ref()
        .and_then(|value| value.pointer("/queue/slots"))
        .and_then(Value::as_array);
    let history_slots = history
        .data
        .as_ref()
        .and_then(|value| value.pointer("/history/slots"))
        .and_then(Value::as_array);
    let mb_left = slots
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("mbleft").and_then(Value::as_f64))
                .sum::<f64>()
        })
        .unwrap_or_default();
    let failed = history_slots
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("status")
                        .and_then(Value::as_str)
                        .is_some_and(|status| status.eq_ignore_ascii_case("failed"))
                })
                .count()
        })
        .unwrap_or_default();
    serde_json::json!({ "queued": slots.map_or(Value::Null, |items| Value::from(items.len())), "mb_left": mb_left.round(), "history_count": history_slots.map_or(Value::Null, |items| Value::from(items.len())), "history_failed": failed, "reachable": queue.reachable && history.reachable })
}

pub async fn mount_health(runtime: &Runtime) -> serde_json::Value {
    let mountpoint = runtime.config.mountpoint.clone();
    let path_present = Path::new(&mountpoint).is_dir();
    let mountpoint_ok = Command::new("mountpoint")
        .arg("-q")
        .arg(&mountpoint)
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    let url = format!(
        "{}/core/stats",
        runtime.config.rclone_url.trim_end_matches('/')
    );
    let response = runtime
        .http
        .post(url)
        .basic_auth(
            &runtime.config.rclone_user,
            Some(&runtime.config.rclone_pass),
        )
        .send()
        .await;
    let rclone = match response {
        Ok(response) => {
            let status = response.status();
            let code = status.as_u16();
            match response.json::<Value>().await {
                Ok(data) => HttpProbe {
                    reachable: status.is_success(),
                    status: Some(code),
                    data: Some(data),
                    error: (!status.is_success()).then(|| format!("HTTP {code}")),
                },
                Err(error) => HttpProbe {
                    reachable: status.is_success(),
                    status: Some(code),
                    data: None,
                    error: Some(error.to_string()),
                },
            }
        }
        Err(error) => HttpProbe::error(error.to_string()),
    };
    serde_json::json!({ "mountpoint": mountpoint, "path_present": path_present, "mountpoint_ok": mountpoint_ok, "rclone": rclone, "healthy": path_present && mountpoint_ok && rclone.reachable, "probed": true })
}

pub async fn plex_sessions(runtime: &Runtime) -> HttpProbe<Value> {
    json_get(
        runtime,
        &format!(
            "{}/status/sessions",
            runtime.config.plex_url.trim_end_matches('/')
        ),
        &[("X-Plex-Token", &runtime.config.plex_token)],
    )
    .await
}

pub async fn host_overview() -> serde_json::Value {
    let uptime = tokio::fs::read_to_string("/proc/uptime")
        .await
        .ok()
        .and_then(|text| {
            text.split_whitespace()
                .next()
                .and_then(|value| value.parse::<f64>().ok())
        });
    let meminfo = tokio::fs::read_to_string("/proc/meminfo")
        .await
        .unwrap_or_default();
    let mut memory = serde_json::Map::new();
    for line in meminfo.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if matches!(
            key,
            "MemTotal" | "MemAvailable" | "MemFree" | "SwapTotal" | "SwapFree"
        ) {
            memory.insert(key.to_string(), Value::String(value.trim().to_string()));
        }
    }
    let load_1m = tokio::fs::read_to_string("/proc/loadavg")
        .await
        .ok()
        .and_then(|text| text.split_whitespace().next().map(str::to_string));
    serde_json::json!({ "uptime_seconds": uptime, "load_1m": load_1m, "memory": memory })
}

pub async fn docker_logs(runtime: &Runtime, name: &str, tail: usize) -> serde_json::Value {
    let Some(docker) = &runtime.docker else {
        return serde_json::json!({ "container": name, "lines": [], "error": "Docker socket unavailable" });
    };
    let result = docker
        .logs(
            name,
            Some(
                LogsOptionsBuilder::new()
                    .stdout(true)
                    .stderr(true)
                    .tail(&tail.to_string())
                    .build(),
            ),
        )
        .collect::<Vec<_>>()
        .await;
    let lines = result
        .into_iter()
        .flatten()
        .map(|output| output.to_string())
        .collect::<Vec<_>>();
    serde_json::json!({ "container": name, "lines": lines })
}

pub async fn activity(runtime: &Runtime) -> serde_json::Value {
    let containers = containers(runtime).await;
    let events = containers.iter().filter_map(|container| container.error.as_ref().map(|error| serde_json::json!({ "kind": "container", "source": container.id, "severity": "warn", "message": error }))).collect::<Vec<_>>();
    serde_json::json!({ "events": events, "note": "M1 activity snapshot; persisted event history lands with SQLite" })
}

pub async fn dashboard(runtime: &Runtime) -> serde_json::Value {
    let containers = containers(runtime).await;
    let (sonarr, radarr, nzbdav, mount, host) = tokio::join!(
        arr_queue(
            runtime,
            &runtime.config.sonarr_url,
            &runtime.config.sonarr_key
        ),
        arr_queue(
            runtime,
            &runtime.config.radarr_url,
            &runtime.config.radarr_key
        ),
        nzbdav_queue(runtime),
        mount_health(runtime),
        host_overview(),
    );
    let count = |probe: &HttpProbe<Value>| {
        probe
            .data
            .as_ref()
            .and_then(|value| value.get("records").and_then(Value::as_array))
            .map(|items| items.len())
    };
    let nzb_count = |probe: &HttpProbe<Value>| {
        probe
            .data
            .as_ref()
            .and_then(|value| value.pointer("/queue/slots").and_then(Value::as_array))
            .map(|items| items.len())
    };
    let free_bytes = disk_free_bytes("/").await;
    serde_json::json!({
        "containers": containers,
        "mount": mount,
        "queues": { "sonarr": count(&sonarr), "radarr": count(&radarr), "nzbdav": nzb_count(&nzbdav) },
        "disk": { "free_bytes": free_bytes },
        "host": host,
        "probes": { "sonarr": sonarr, "radarr": radarr, "nzbdav": nzbdav },
        "note": "M1 read-only probes enabled; mutations remain disabled"
    })
}

async fn disk_free_bytes(path: &str) -> Option<u64> {
    let output = Command::new("df")
        .args(["-P", "-B1", path])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_health_from_docker_status() {
        assert_eq!(parse_health("Up 2 hours (healthy)"), Some("healthy".into()));
        assert_eq!(parse_health("Up 2 hours"), None);
    }

    #[test]
    fn config_defaults_to_container_addresses() {
        let config = Config::from_env();
        assert_eq!(
            config.nzbdav_url,
            std::env::var("NZBDAV_URL").unwrap_or_else(|_| "http://nzbdav:3000".into())
        );
    }
}
