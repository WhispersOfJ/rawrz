//! API-contract integration tests (spec §D.6 `guard` job, red paths).
//!
//! These pin landmine behaviors at the HTTP layer: guards must refuse, not
//! silently pass. Env tests use a hermetic runtime (temp `.env`, mock nzbdav
//! backend) so the apply cascade is exercised without touching the stack.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

use cave_deck::env::EnvState;
use cave_deck::probes::{Config, Runtime};

const SAMPLE_ENV: &str = r#"# ---- Identity / Runtime ----
PUID=1000
PGID=1000

# ---- NzbDAV ----
NZBDAV_WEBDAV_PASS=changeme

# ---- Usenet Providers ----
NZBDAV_USENET_HOST=usenet.example.com
NZBDAV_USENET_PORT=563
NZBDAV_USENET_USER=alice
NZBDAV_USENET_PASS=supersecretpass

NZBDAV_USENET_BACKUP_HOST=backup.example.com
NZBDAV_USENET_BACKUP_PORT=563
NZBDAV_USENET_BACKUP_USER=bob
NZBDAV_USENET_BACKUP_PASS=anothersecretpass

# ---- Plex ----
PLEX_URL=http://192.168.1.100:32400
PLEX_TOKEN=changeme

# ---- *arr API Keys ----
RADARR_API_KEY=deadbeefdeadbeefdeadbeefdeadbeef
SONARR_API_KEY=deadbeefdeadbeefdeadbeefdeadbeef
PROWLARR_API_KEY=deadbeefdeadbeefdeadbeefdeadbeef
"#;

/// Unreachable nzbdav endpoint (closed local port, no DNS dependency) — the
/// queue guard must fail closed.
fn nzbdav_unreachable() -> String {
    "http://127.0.0.1:9".into()
}

/// Hermetic runtime with a temp `.env`. `nzbdav_url` decides what the queue
/// guard sees: the unreachable URL above (fail-closed in CI) or a local mock.
fn runtime_with_nzbdav(tmp: &tempfile::TempDir, nzbdav_url: &str) -> Arc<Runtime> {
    let env_path = tmp.path().join(".env");
    std::fs::write(&env_path, SAMPLE_ENV).unwrap();
    let backup = tmp.path().join("backups");
    Arc::new(Runtime {
        docker: None,
        http: reqwest::Client::new(),
        config: Config {
            nzbdav_url: nzbdav_url.into(),
            nzbdav_key: String::new(),
            radarr_url: "http://radarr:7878".into(),
            radarr_key: String::new(),
            sonarr_url: "http://sonarr:8989".into(),
            sonarr_key: String::new(),
            prowlarr_url: "http://prowlarr:9696".into(),
            prowlarr_key: String::new(),
            seerr_url: "http://seerr:5055".into(),
            seerr_key: String::new(),
            plex_url: "http://plex:32400".into(),
            plex_token: String::new(),
            rclone_url: "http://nzbdav_rclone:5572".into(),
            rclone_user: "rclone".into(),
            rclone_pass: String::new(),
            mountpoint: "/mnt/remote/nzbdav".into(),
        },
        env: EnvState::new(env_path, backup),
    })
}

fn runtime(tmp: &tempfile::TempDir) -> Arc<Runtime> {
    runtime_with_nzbdav(tmp, &nzbdav_unreachable())
}

/// Tiny canned nzbdav queue mock: responds to `GET /api?mode=queue` with the
/// given slots payload. Returns `(base_url, join_handle)`.
/// Scripted HTTP mock: serves one response per incoming connection, in order.
/// Each entry is `(status_line, body)`. Useful for multi-step flows (e.g.
/// rotation's GET config/host → PUT → verify).
fn scripted_mock(steps: &[(&str, &str)]) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let steps = steps
        .iter()
        .map(|(s, b)| (s.to_string(), b.to_string()))
        .collect::<Vec<_>>();
    let handle = std::thread::spawn(move || {
        use std::io::{Read, Write};
        for (status, body) in &steps {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}"), handle)
}

fn nzbdav_mock(slots_json: &str) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let body = slots_json.to_string();
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}"), handle)
}

async fn app_with_runtime(runtime: Arc<Runtime>) -> axum::Router {
    cave_deck::routes::router_with(runtime)
}

async fn app() -> axum::Router {
    cave_deck::routes::router().await
}

async fn json_request(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(Body::from(body.unwrap_or("").to_string()))
        .unwrap();
    let res = app.clone().oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 256 * 1024)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn healthz_ok_readyz_unavailable() {
    let res = app()
        .await
        .oneshot(Request::get("/api/v1/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app()
        .await
        .oneshot(Request::get("/api/v1/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        res.status(),
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));
}

/// Landmine #2 red path: empty-trash must 409 while the mount probe is
/// unimplemented — never a false "OK, trashed".
#[tokio::test]
async fn empty_trash_refused_when_mount_unverified() {
    let res = app()
        .await
        .oneshot(
            Request::post("/api/v1/plex/empty-trash")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "mount_unhealthy");
}

/// Mutations are jobs: container actions must 202 with a jobId.
#[tokio::test]
async fn container_action_returns_job() {
    let res = app()
        .await
        .oneshot(
            Request::post("/api/v1/containers/sonarr/restart")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["jobId"].as_str().unwrap().starts_with("job-"));
}

/// Unknown actions 404, not silently 202.
#[tokio::test]
async fn container_unknown_action_404() {
    let res = app()
        .await
        .oneshot(
            Request::post("/api/v1/containers/sonarr/explode")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Feature-ID header contract: every area router declares what it serves.
#[tokio::test]
async fn dashboard_route_present() {
    let res = app()
        .await
        .oneshot(
            Request::get("/api/v1/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// .env engine (M2 env engine)
// ---------------------------------------------------------------------------

/// GET /env groups by section, masks secrets, flags placeholders.
#[tokio::test]
async fn env_view_masks_and_groups() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(&app, "GET", "/api/v1/env", None).await;
    assert_eq!(status, StatusCode::OK);
    let sections = body["sections"].as_array().unwrap();
    assert!(sections.iter().any(|s| s["name"] == "NzbDAV"));
    let plex = sections.iter().find(|s| s["name"] == "Plex").unwrap();
    let token = plex["vars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["key"] == "PLEX_TOKEN")
        .unwrap();
    assert_eq!(token["secret"], true);
    assert_eq!(token["stale"], true); // 'changeme'
    assert!(token["masked"].as_str().unwrap().contains("••••"));
    assert!(!token["masked"].as_str().unwrap().contains("changeme"));
}

/// Staging an invalid value is refused (blocking) and does not replace the
/// previous draft.
#[tokio::test]
async fn env_draft_blocks_invalid_values() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;

    // Stage a valid PLEX_URL change first.
    let (status, body) = json_request(
        &app,
        "PUT",
        "/api/v1/env/draft",
        Some(r#"{"values":{"PLEX_URL":"http://10.0.0.9:32400"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["staged"], true);

    // Stage an invalid one — must not clobber the earlier valid draft.
    let (status, body) = json_request(
        &app,
        "PUT",
        "/api/v1/env/draft",
        Some(r#"{"values":{"RADARR_URL":"not a url"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["blocking"], true);
    assert_eq!(body["staged"], false);
    let issues = body["issues"].as_array().unwrap();
    assert!(issues.iter().any(|i| i["code"] == "invalid_url"));

    // The earlier PLEX_URL draft survives.
    let (status, body) = json_request(&app, "GET", "/api/v1/env/diff", None).await;
    assert_eq!(status, StatusCode::OK);
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["key"], "PLEX_URL");
}

/// Apply without the EXC confirm token is refused.
#[tokio::test]
async fn env_apply_requires_confirm() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(&app, "POST", "/api/v1/env/apply", Some("{}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "confirm_required");
}

/// Landmine #4 green path: an apply whose blast radius does NOT touch nzbdav
/// proceeds even when the nzbdav queue can't be reached.
#[tokio::test]
async fn env_apply_allowed_when_not_touching_nzbdav() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;

    let (status, _) = json_request(
        &app,
        "PUT",
        "/api/v1/env/draft",
        Some(r#"{"values":{"PLEX_URL":"http://10.0.0.9:32400"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/env/apply",
        Some(r#"{"confirm":"env"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(body["jobId"].as_str().unwrap().starts_with("job-"));
    assert_eq!(body["changes"][0]["key"], "PLEX_URL");
    assert!(body["backup"].as_str().unwrap().contains("backups"));

    // File on disk actually changed + backup created.
    let env_text = std::fs::read_to_string(tmp.path().join(".env")).unwrap();
    assert!(env_text.contains("PLEX_URL=http://10.0.0.9:32400"));
    assert!(
        std::fs::read_dir(tmp.path().join("backups"))
            .unwrap()
            .count()
            >= 1
    );
}

/// Landmine #4 red path: an apply whose blast radius touches nzbdav is refused
/// when the queue is NOT verifiable (fail-closed), with the guard reason.
#[tokio::test]
async fn env_apply_touching_nzbdav_requires_verifiable_queue() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;

    // NZBDAV_WEBDAV_PASS is a consumer of nzbdav — stage a change to it.
    let (status, _) = json_request(
        &app,
        "PUT",
        "/api/v1/env/draft",
        Some(r#"{"values":{"NZBDAV_WEBDAV_PASS":"supersecret"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/env/apply",
        Some(r#"{"confirm":"env"}"#),
    )
    .await;
    // The queue probe targets a closed local port, so the guard must refuse
    // the apply (503), never silently pass.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "queue_guard_unavailable");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("landmine #4"));

    // Nothing was applied — the file is untouched.
    let env_text = std::fs::read_to_string(tmp.path().join(".env")).unwrap();
    assert!(env_text.contains("NZBDAV_WEBDAV_PASS=changeme"));
}

// ---------------------------------------------------------------------------
// Credentials + providers (M2 credentials panel — reveal + provider CRUD)
// ---------------------------------------------------------------------------

/// GET /credentials groups only secret keys and masks their values.
#[tokio::test]
async fn credentials_view_masks_secrets_only() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(&app, "GET", "/api/v1/credentials", None).await;
    assert_eq!(status, StatusCode::OK);
    let groups = body["groups"].as_array().unwrap();
    let arr = groups
        .iter()
        .find(|g| g["name"] == "*arr API Keys")
        .map(|g| g["vars"].as_array().unwrap().clone())
        .unwrap();
    let radarr = arr.iter().find(|v| v["key"] == "RADARR_API_KEY").unwrap();
    assert_eq!(radarr["secret"], true);
    assert!(radarr["masked"].as_str().unwrap().contains("••••"));
    assert!(!radarr["masked"].as_str().unwrap().contains("deadbeef"));
    // Non-secret keys are not in the credentials view.
    let all_keys = groups
        .iter()
        .flat_map(|g| g["vars"].as_array().unwrap().iter())
        .map(|v| v["key"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!all_keys.contains(&"PUID"));
    assert!(!all_keys.contains(&"PLEX_URL"));
}

/// Reveal returns plaintext once and is audit-logged.
#[tokio::test]
async fn credential_reveal_returns_plaintext_and_audits() {
    cave_deck::audit::clear();
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/credentials/RADARR_API_KEY/reveal",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], "deadbeefdeadbeefdeadbeefdeadbeef");
    assert_eq!(body["audited"], true);
    let events = cave_deck::audit::recent(10);
    assert!(events
        .iter()
        .any(|e| e.kind == "cred.reveal" && e.target == "RADARR_API_KEY"));
}

/// Revealing an unknown key is a 404, not an empty string.
#[tokio::test]
async fn credential_reveal_unknown_key_404() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) =
        json_request(&app, "POST", "/api/v1/credentials/NOPE_KEY/reveal", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "unknown_key");
}

/// Blast radius for a secret key.
#[tokio::test]
async fn credential_blast_radius_maps_consumers() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(
        &app,
        "GET",
        "/api/v1/credentials/SONARR_API_KEY/blast-radius",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let consumers = body["consumers"].as_array().unwrap();
    assert!(consumers.contains(&serde_json::json!("nzbdav")));
    assert!(consumers.contains(&serde_json::json!("unpackerr")));
}

/// GET /usenet/providers lists the flat-var slots with masking.
#[tokio::test]
async fn usenet_providers_lists_slots_masked() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(&app, "GET", "/api/v1/usenet/providers", None).await;
    assert_eq!(status, StatusCode::OK);
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 3);
    let primary = providers
        .iter()
        .find(|p| p["nickname"] == "primary")
        .unwrap();
    assert_eq!(primary["enabled"], true);
    assert_eq!(primary["wired"], true);
    assert_eq!(primary["host"], "usenet.example.com");
    assert!(primary["passMasked"].as_str().unwrap().contains("••••"));
    assert!(!primary["passMasked"]
        .as_str()
        .unwrap()
        .contains("supersecretpass"));
    let eweka = providers.iter().find(|p| p["nickname"] == "eweka").unwrap();
    assert_eq!(eweka["wired"], false);
    assert_eq!(eweka["enabled"], false);
}

/// Upsert stages the slot's vars into the env draft (apply still via
/// /env/apply).
#[tokio::test]
async fn usenet_provider_upsert_stages_draft() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/usenet/providers/backup",
        Some(
            r#"{"host":"new-backup.example.com","port":563,"user":"carol","pass":"brandnewpass1"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["staged"], true);
    let changes = body["changes"].as_array().unwrap();
    assert!(changes
        .iter()
        .any(|c| c["key"] == "NZBDAV_USENET_BACKUP_HOST"));
    assert!(changes
        .iter()
        .any(|c| c["key"] == "NZBDAV_USENET_BACKUP_PASS"));
    let consumers = body["consumers"].as_array().unwrap();
    assert!(consumers.contains(&serde_json::json!("nzbdav")));
}

/// Upsert refuses empty host/user (blocking, draft untouched).
#[tokio::test]
async fn usenet_provider_upsert_blocks_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/usenet/providers/backup",
        Some(r#"{"host":"","user":""}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["staged"], false);
    assert_eq!(body["blocking"], true);
    let issues = body["issues"].as_array().unwrap();
    assert!(issues.iter().any(|i| i["code"] == "invalid_host"));
    assert!(issues.iter().any(|i| i["code"] == "invalid_user"));
}

/// DELETE refuses compose-wired slots (primary/backup).
#[tokio::test]
async fn usenet_provider_delete_refuses_wired_slots() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) =
        json_request(&app, "DELETE", "/api/v1/usenet/providers/primary", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "provider_wired");
}

/// DELETE on a dormant slot stages removal for apply.
#[tokio::test]
async fn usenet_provider_delete_stages_dormant_slot() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(&app, "DELETE", "/api/v1/usenet/providers/eweka", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["staged"], true);
    let removed = body["removed"].as_array().unwrap();
    assert!(removed.contains(&serde_json::json!("NZBDAV_USENET_EWEKA_HOST")));
}

/// DELETE of an unknown slot is a 404.
#[tokio::test]
async fn usenet_provider_delete_unknown_404() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, _body) = json_request(&app, "DELETE", "/api/v1/usenet/providers/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Provider test proxies to InfiniDysk's own probe endpoint.
#[tokio::test]
async fn usenet_provider_test_proxies_to_nzbdav() {
    let tmp = tempfile::tempdir().unwrap();
    let (url, server) = nzbdav_mock(r#"{"status":true,"connected":true}"#);
    let app = app_with_runtime(runtime_with_nzbdav(&tmp, &url)).await;
    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/usenet/providers/test",
        Some(
            r#"{"host":"news.example.com","port":563,"user":"alice","pass":"supersecretpass","useSsl":true}"#,
        ),
    )
    .await;
    server.join().unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["connected"], true);
    assert_eq!(body["status"], 200);
}

/// Provider test without host/user is refused client-side.
#[tokio::test]
async fn usenet_provider_test_requires_host_and_user() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/usenet/providers/test",
        Some(r#"{"host":"","user":""}"#),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_provider");
}

// ---------------------------------------------------------------------------
// Key rotation + Plex token (M2 rotation slice)
// ---------------------------------------------------------------------------

/// Rotating an unknown key is a 404 (only the four app keys are rotatable).
#[tokio::test]
async fn credential_rotate_unknown_key_404() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_runtime(runtime(&tmp)).await;
    let (status, body) = json_request(&app, "POST", "/api/v1/credentials/PLEX_TOKEN", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_rotatable");
}

/// *arr rotation: GET config/host → PUT back with new apiKey → verify with it;
/// then the new key is staged into the env draft.
#[tokio::test]
async fn credential_rotate_arr_pushes_and_stages() {
    let tmp = tempfile::tempdir().unwrap();
    let (radarr_url, server) = scripted_mock(&[
        // GET /api/v3/config/host (old-key auth) → full resource to echo.
        (
            "200 OK",
            r#"{"id":1,"port":7878,"apiKey":"oldradarrkey","branch":"master","username":"admin","password":"hash","passwordConfirmation":""}"#,
        ),
        // PUT /api/v3/config/host → accepted.
        ("202 Accepted", "1"),
        // Verify GET /api/v3/system/status with the new key.
        ("200 OK", r#"{"version":"6.0.0"}"#),
    ]);

    let env_path = tmp.path().join(".env");
    std::fs::write(
        &env_path,
        "RADARR_API_KEY=oldradarrkey\nSEERR_API_KEY=oldseerrkey\nPLEX_TOKEN=oldplextoken\n",
    )
    .unwrap();
    let runtime = Arc::new(cave_deck::probes::Runtime {
        docker: None,
        http: reqwest::Client::new(),
        config: cave_deck::probes::Config {
            radarr_url: radarr_url.clone(),
            radarr_key: "oldradarrkey".into(),
            sonarr_url: "http://sonarr:8989".into(),
            sonarr_key: String::new(),
            prowlarr_url: "http://prowlarr:9696".into(),
            prowlarr_key: String::new(),
            seerr_url: "http://seerr:5055".into(),
            seerr_key: "oldseerrkey".into(),
            plex_url: "http://plex:32400".into(),
            plex_token: "oldplextoken".into(),
            nzbdav_url: "http://nzbdav:3000".into(),
            nzbdav_key: String::new(),
            rclone_url: "http://nzbdav_rclone:5572".into(),
            rclone_user: "rclone".into(),
            rclone_pass: String::new(),
            mountpoint: "/mnt/remote/nzbdav".into(),
        },
        env: cave_deck::env::EnvState::new(env_path, tmp.path().join("backups")),
    });
    let app = app_with_runtime(runtime.clone()).await;

    let (status, body) =
        json_request(&app, "POST", "/api/v1/credentials/RADARR_API_KEY", None).await;
    server.join().unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rotated"], true);
    assert_eq!(body["staged"], true);
    assert_eq!(body["key"], "RADARR_API_KEY");
    let consumers = body["consumers"].as_array().unwrap();
    assert!(consumers.contains(&serde_json::json!("nzbdav")));
    assert!(body["push_error"].is_null());

    // The draft holds a 32-hex new key (never the old one).
    let draft = runtime.env.draft_snapshot().await;
    let staged = draft.values.get("RADARR_API_KEY").unwrap();
    assert_eq!(staged.len(), 32);
    assert!(staged.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(staged, "oldradarrkey");
}

/// *arr rotation with an unreachable app still stages the draft and reports
/// the push error (the UI can then decide), never a silent partial success.
#[tokio::test]
async fn credential_rotate_arr_unreachable_reports_error() {
    let tmp = tempfile::tempdir().unwrap();
    let env_path = tmp.path().join(".env");
    std::fs::write(
        &env_path,
        "RADARR_API_KEY=oldradarrkey\nSEERR_API_KEY=oldseerrkey\nPLEX_TOKEN=oldplextoken\n",
    )
    .unwrap();
    let runtime = Arc::new(cave_deck::probes::Runtime {
        docker: None,
        http: reqwest::Client::new(),
        config: cave_deck::probes::Config {
            radarr_url: "http://127.0.0.1:9".into(), // closed port
            radarr_key: "oldradarrkey".into(),
            sonarr_url: "http://sonarr:8989".into(),
            sonarr_key: String::new(),
            prowlarr_url: "http://prowlarr:9696".into(),
            prowlarr_key: String::new(),
            seerr_url: "http://seerr:5055".into(),
            seerr_key: "oldseerrkey".into(),
            plex_url: "http://plex:32400".into(),
            plex_token: "oldplextoken".into(),
            nzbdav_url: "http://nzbdav:3000".into(),
            nzbdav_key: String::new(),
            rclone_url: "http://nzbdav_rclone:5572".into(),
            rclone_user: "rclone".into(),
            rclone_pass: String::new(),
            mountpoint: "/mnt/remote/nzbdav".into(),
        },
        env: cave_deck::env::EnvState::new(env_path, tmp.path().join("backups")),
    });
    let app = app_with_runtime(runtime.clone()).await;

    let (status, body) =
        json_request(&app, "POST", "/api/v1/credentials/RADARR_API_KEY", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rotated"], false);
    assert_eq!(body["staged"], true);
    let push_error = body["pushError"].as_str().unwrap();
    assert!(push_error.contains("config/host") || push_error.contains("failed"));
}

/// Seerr rotation: regenerate server-side, read the new key from the admin
/// response, stage it into the draft.
#[tokio::test]
async fn credential_rotate_seerr_regenerates_and_stages() {
    let tmp = tempfile::tempdir().unwrap();
    let (seerr_url, server) = scripted_mock(&[("200 OK", r#"{"apiKey":"seerr_new_key_12345"}"#)]);

    let env_path = tmp.path().join(".env");
    std::fs::write(
        &env_path,
        "RADARR_API_KEY=oldradarrkey\nSEERR_API_KEY=oldseerrkey\nPLEX_TOKEN=oldplextoken\n",
    )
    .unwrap();
    let runtime = Arc::new(cave_deck::probes::Runtime {
        docker: None,
        http: reqwest::Client::new(),
        config: cave_deck::probes::Config {
            radarr_url: "http://radarr:7878".into(),
            radarr_key: String::new(),
            sonarr_url: "http://sonarr:8989".into(),
            sonarr_key: String::new(),
            prowlarr_url: "http://prowlarr:9696".into(),
            prowlarr_key: String::new(),
            seerr_url: seerr_url.clone(),
            seerr_key: "oldseerrkey".into(),
            plex_url: "http://plex:32400".into(),
            plex_token: "oldplextoken".into(),
            nzbdav_url: "http://nzbdav:3000".into(),
            nzbdav_key: String::new(),
            rclone_url: "http://nzbdav_rclone:5572".into(),
            rclone_user: "rclone".into(),
            rclone_pass: String::new(),
            mountpoint: "/mnt/remote/nzbdav".into(),
        },
        env: cave_deck::env::EnvState::new(env_path, tmp.path().join("backups")),
    });
    let app = app_with_runtime(runtime.clone()).await;

    let (status, body) =
        json_request(&app, "POST", "/api/v1/credentials/SEERR_API_KEY", None).await;
    server.join().unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rotated"], true);
    assert_eq!(body["key"], "SEERR_API_KEY");
    let draft = runtime.env.draft_snapshot().await;
    assert_eq!(
        draft.values.get("SEERR_API_KEY").unwrap(),
        "seerr_new_key_12345"
    );
}

/// Plex token verify reports reachability/validity against the running server.
#[tokio::test]
async fn plex_token_verify_reports_validity() {
    let tmp = tempfile::tempdir().unwrap();
    let (plex_url, server) = scripted_mock(&[(
        "200 OK",
        r#"{"MediaContainer":{"machineIdentifier":"abc"}}"#,
    )]);
    let env_path = tmp.path().join(".env");
    std::fs::write(&env_path, "PLEX_TOKEN=oldplextoken\n").unwrap();
    let runtime = Arc::new(cave_deck::probes::Runtime {
        docker: None,
        http: reqwest::Client::new(),
        config: cave_deck::probes::Config {
            radarr_url: "http://radarr:7878".into(),
            radarr_key: String::new(),
            sonarr_url: "http://sonarr:8989".into(),
            sonarr_key: String::new(),
            prowlarr_url: "http://prowlarr:9696".into(),
            prowlarr_key: String::new(),
            seerr_url: "http://seerr:5055".into(),
            seerr_key: String::new(),
            plex_url: plex_url.clone(),
            plex_token: "oldplextoken".into(),
            nzbdav_url: "http://nzbdav:3000".into(),
            nzbdav_key: String::new(),
            rclone_url: "http://nzbdav_rclone:5572".into(),
            rclone_user: "rclone".into(),
            rclone_pass: String::new(),
            mountpoint: "/mnt/remote/nzbdav".into(),
        },
        env: cave_deck::env::EnvState::new(env_path, tmp.path().join("backups")),
    });
    let app = app_with_runtime(runtime).await;

    let (status, body) = json_request(&app, "GET", "/api/v1/plex/token", None).await;
    server.join().unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], true);
    assert_eq!(body["reachable"], true);
    assert_eq!(body["token_set"], true);
}

/// Staging a new PLEX_TOKEN is refused when empty and staged when valid.
#[tokio::test]
async fn plex_token_update_validates_and_stages() {
    let tmp = tempfile::tempdir().unwrap();
    let env_path = tmp.path().join(".env");
    std::fs::write(&env_path, "PLEX_TOKEN=oldplextoken\n").unwrap();
    let runtime = Arc::new(cave_deck::probes::Runtime {
        docker: None,
        http: reqwest::Client::new(),
        config: cave_deck::probes::Config {
            radarr_url: "http://radarr:7878".into(),
            radarr_key: String::new(),
            sonarr_url: "http://sonarr:8989".into(),
            sonarr_key: String::new(),
            prowlarr_url: "http://prowlarr:9696".into(),
            prowlarr_key: String::new(),
            seerr_url: "http://seerr:5055".into(),
            seerr_key: String::new(),
            plex_url: "http://plex:32400".into(),
            plex_token: "oldplextoken".into(),
            nzbdav_url: "http://nzbdav:3000".into(),
            nzbdav_key: String::new(),
            rclone_url: "http://nzbdav_rclone:5572".into(),
            rclone_user: "rclone".into(),
            rclone_pass: String::new(),
            mountpoint: "/mnt/remote/nzbdav".into(),
        },
        env: cave_deck::env::EnvState::new(env_path, tmp.path().join("backups")),
    });
    let app = app_with_runtime(runtime.clone()).await;

    // Empty token refused.
    let (status, body) =
        json_request(&app, "POST", "/api/v1/plex/token", Some(r#"{"token":""}"#)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_token");

    // Valid token staged into the draft.
    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/plex/token",
        Some(r#"{"token":"newplextoken123"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["staged"], true);
    let draft = runtime.env.draft_snapshot().await;
    assert_eq!(draft.values.get("PLEX_TOKEN").unwrap(), "newplextoken123");
}

/// Landmine #4: an apply touching nzbdav is ALLOWED when the queue is live and
/// empty — the guard is a real probe, not a blanket refusal.
#[tokio::test]
async fn env_apply_touching_nzbdav_allowed_when_queue_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let (url, server) = nzbdav_mock(r#"{"queue":{"slots":[]}}"#);
    let app = app_with_runtime(runtime_with_nzbdav(&tmp, &url)).await;

    let (status, _) = json_request(
        &app,
        "PUT",
        "/api/v1/env/draft",
        Some(r#"{"values":{"NZBDAV_WEBDAV_PASS":"supersecret"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json_request(
        &app,
        "POST",
        "/api/v1/env/apply",
        Some(r#"{"confirm":"env"}"#),
    )
    .await;
    server.join().unwrap();
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(body["jobId"].as_str().unwrap().starts_with("job-"));
    assert!(body["consumers"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("nzbdav")));
    let env_text = std::fs::read_to_string(tmp.path().join(".env")).unwrap();
    assert!(env_text.contains("NZBDAV_WEBDAV_PASS=supersecret"));
}
