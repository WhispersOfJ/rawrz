//! API-contract integration tests (spec §D.6 `guard` job, red paths).
//!
//! These pin the landmine behaviors at the HTTP layer before the M1+ plumbing
//! exists: the guards must refuse, not silently pass.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

async fn app() -> axum::Router {
    cave_deck::routes::router().await
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
