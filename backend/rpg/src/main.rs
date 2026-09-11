//! RPG backend binary (spec §7.3/§10.2): Axum server on port 46532.
//! (Port finalized 2026-09-08: the originally resolved 86532 exceeds the
//! 16-bit TCP limit of 65535 and could never bind — see spec §12 Q9.)
//! Startup: load the shared `.env`, connect to Postgres (pooled — F-17/F-19:
//! HTTP handlers and the poll loop share the pool instead of one mutex'd
//! client), run migrations once, then serve the PIN-gated router beside the
//! 5-minute poll loop (§9.1: content sync + game tick — the one owner of
//! unattended play). Shutdown on Ctrl-C: the server drains, the poll loop
//! finishes its current cycle, then the process returns.

use movie_rpg::persistence::PostgresContentStore;
use movie_rpg::poll::{run_poll_loop, PollStack};
use movie_rpg::{config::ProbeConfig, server};
use std::sync::Arc;
use tokio::sync::watch;

/// F-12: the bind address is configurable for LAN-only deployments. The
/// default binds all interfaces (the historical behavior); a privacy-tight
/// host can narrow it, e.g. RPG_BIND_ADDRESS=192.168.1.20:46532.
fn bind_address() -> String {
    std::env::var("RPG_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:46532".to_owned())
}

#[tokio::main]
async fn main() {
    // F-26: the env file is resolved exe-relative first so the binary runs
    // from any CWD, then CWD-relative (the historical layout for
    // `cd backend/rpg && cargo run`), then the bare default.
    let env_file = std::env::args().nth(1).unwrap_or_else(|| {
        let exe_relative = std::env::current_exe()
            .ok()
            .and_then(|path| path.ancestors().nth(3).map(|dir| dir.join(".env")))
            .filter(|path| path.exists());
        let cwd_relative = std::path::Path::new("../.env")
            .exists()
            .then(|| "../.env".to_owned());
        exe_relative
            .map(|path| path.to_string_lossy().into_owned())
            .or(cwd_relative)
            .unwrap_or_else(|| "../.env".to_owned())
    });
    let config = match ProbeConfig::from_env_file(&env_file) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("startup failed: {error}");
            std::process::exit(1);
        }
    };

    let store = match PostgresContentStore::connect(&config.rpg_db_url).await {
        Ok(store) => Arc::new(store),
        Err(error) => {
            eprintln!("startup failed: {error}");
            std::process::exit(1);
        }
    };

    // Migrations run here, once per process (F-21): the poll loop no longer
    // re-runs them on each of the 288 daily cycles.
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

    let app = server::router(store.clone());
    let bind_address = bind_address();
    let listener = match tokio::net::TcpListener::bind(&bind_address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("startup failed: cannot bind {bind_address}: {error}");
            std::process::exit(1);
        }
    };

    // Poll loop (§9.1): content sync + game tick every 5 minutes. With the
    // pooled store there is no shared lock: a slow provider stalls only the
    // poll cycle, never an HTTP request (F-19). Failures inside a cycle are
    // logged and non-fatal; the loop never exits on them.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let poll_loop = tokio::spawn(run_poll_loop(
        store.clone(),
        PollStack::from_config(&config),
        movie_rpg::poll::POLL_INTERVAL,
        shutdown_rx,
    ));

    println!("RPG backend listening on http://{bind_address}");

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

    // Server drained: stop the loop after its current cycle and wait for
    // it, so no in-flight write is abandoned on the way out.
    let _ = shutdown_tx.send(true);
    match poll_loop.await {
        Ok(cycles) => println!("poll loop stopped after {cycles} completed cycle(s)"),
        Err(error) => eprintln!("poll loop panicked: {error}"),
    }
}
