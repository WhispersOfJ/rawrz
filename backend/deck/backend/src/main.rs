//! Cave Deck backend (M0 skeleton).
//!
//! Binary entrypoint; all logic lives in the library target so integration
//! tests drive the same router the server runs.

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cave_deck=info,tower_http=info".into()),
        )
        .init();

    let app = cave_deck::routes::router().await;

    let addr = SocketAddr::from(([0, 0, 0, 0], 7780));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind :7780");
    tracing::info!("Cave Deck listening on http://{addr}");
    axum::serve(listener, app).await.expect("serve");
}
