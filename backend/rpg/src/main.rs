//! RPG backend binary (spec §7.3/§10.2): Axum server on 0.0.0.0:46532.
//! (Port finalized 2026-09-08: the originally resolved 86532 exceeds the
//! 16-bit TCP limit of 65535 and could never bind — see spec §12 Q9.)
//! Startup: load the shared `.env`, connect to Postgres, run migrations,
//! then serve the PIN-gated router. Shutdown on Ctrl-C.

use movie_rpg::persistence::PostgresContentStore;
use movie_rpg::{config::ProbeConfig, server};
use std::sync::Arc;
use tokio::sync::Mutex;

const BIND_ADDRESS: &str = "0.0.0.0:46532";

#[tokio::main]
async fn main() {
    let env_file = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../.env".to_owned());
    let config = match ProbeConfig::from_env_file(&env_file) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("startup failed: {error}");
            std::process::exit(1);
        }
    };

    let mut store = match PostgresContentStore::connect(&config.rpg_db_url).await {
        Ok(store) => store,
        Err(error) => {
            eprintln!("startup failed: {error}");
            std::process::exit(1);
        }
    };

    match store.migrate().await {
        Ok(summary) => {
            println!(
                "migrations: {} applied, {} already applied",
                summary.applied, summary.already_applied
            );
        }
        Err(error) => {
            eprintln!("startup failed: {error}");
            std::process::exit(1);
        }
    }

    let app = server::router(Arc::new(Mutex::new(store)));
    let listener = match tokio::net::TcpListener::bind(BIND_ADDRESS).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("startup failed: cannot bind {BIND_ADDRESS}: {error}");
            std::process::exit(1);
        }
    };
    println!("RPG backend listening on http://{BIND_ADDRESS}");

    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            println!("shutting down");
        })
        .await
    {
        eprintln!("server error: {error}");
        std::process::exit(1);
    }
}
