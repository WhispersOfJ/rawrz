//! WebSocket hub (spec §5.3 / D.3) — single multiplexed socket at `/api/v1/ws`.
//!
//! M0: on connect the server sends a `snapshot` frame for the `jobs` topic and
//! answers pings; topic subscriptions and live deltas land in M1.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde_json::json;

pub async fn handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    let snapshot = json!({
        "topic": "jobs",
        "frame": "snapshot",
        "data": crate::jobs::list().await,
    });
    if socket
        .send(Message::Text(snapshot.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    while let Some(Ok(_msg)) = socket.recv().await {
        // M1: subscription protocol {"sub": ["topic", ...]} and topic deltas.
    }
}
