//! Scratch-Postgres proof for the persistence layer. Skipped unless
//! RPG_DB_URL points at a scratch database (see
//! scripts/scratch_pg_proof.sh for the disposable one-command harness).
//!
//! Exercises the real surface: migrate() over all ten migrations, the
//! locked/set/verify PIN flows, row-level seed assertions for the bootstrap
//! (character_state, horror genre_access, all 13 settings defaults),
//! idempotency by execution (set + seed twice, zero duplicates), and a
//! no-op re-migrate on an already-migrated database.

use movie_rpg::persistence::{PostgresContentStore, SETTINGS_V1_DEFAULTS};
use movie_rpg::auth::PinVerifyOutcome;

const SCRATCH_DB_URL_ENV: &str = "RPG_DB_URL";

async fn fresh_connection(database_url: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls)
        .await
        .expect("scratch database must be reachable");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// Resets to a truly fresh schema, whatever the scratch database currently
/// holds (including nothing, on a virgin container).
async fn reset_database(database_url: &str) {
    let client = fresh_connection(database_url).await;
    client        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("scratch database must be resettable");
}

/// Row count for a table; callers pass fixed table-name literals only.
async fn count(client: &tokio_postgres::Client, table: &'static str) -> i64 {
    let sql = format!("SELECT count(*) FROM {table}");
    client
        .query_one(&sql, &[])
        .await
        .expect("count probe executes")
        .get::<_, i64>(0)
}

#[tokio::test]
async fn store_proof_runs_the_real_surface_against_scratch_postgres() {
    let Ok(database_url) = std::env::var(SCRATCH_DB_URL_ENV) else {
        eprintln!("skipping store proof: {SCRATCH_DB_URL_ENV} is not set");
        return;
    };
    reset_database(&database_url).await;

    // (1) migrate() over all six migrations on a fresh database.
    let mut store = PostgresContentStore::connect(&database_url)
        .await
        .expect("store connects to scratch database");
    let summary = store.migrate().await.expect("migrate() succeeds");
    assert_eq!(summary.applied, 10, "all ten migrations apply on a fresh database");
    assert_eq!(summary.already_applied, 0);

    // (2a) verify with no account yet: locked.
    assert_eq!(
        store.verify_account_pin("1234").await.expect("locked verify is not an error"),
        None,
        "no account yet means locked"
    );

    // (3a) bootstrap with no account: a no-op.
    let bootstrap = store.bootstrap_single_character().await.expect("bootstrap succeeds");
    assert_eq!(
        (bootstrap.character_state_seeded, bootstrap.genre_access_seeded, bootstrap.settings_seeded),
        (0, 0, 0),
        "bootstrap without an account must be a no-op"
    );

    // (2b) set-PIN: creates the account, character, and full seed in one shot.
    let (outcome, bootstrap) = store
        .set_account_pin("1357")
        .await
        .expect("first set_account_pin succeeds");
    assert!(outcome.account_created, "first set creates the account");
    assert_eq!(
        (bootstrap.character_state_seeded, bootstrap.genre_access_seeded, bootstrap.settings_seeded),
        (1, 1, 13),
        "first set seeds state, horror access, and all 13 settings defaults"
    );

    // (3b) Row-level assertions: the seed actually landed.
    let probe = fresh_connection(&database_url).await;
    assert_eq!(count(&probe, "accounts").await, 1);
    assert_eq!(count(&probe, "characters").await, 1);
    assert_eq!(count(&probe, "character_state").await, 1);
    assert_eq!(count(&probe, "genres").await, 10);
    assert_eq!(count(&probe, "achievements").await, 101, "the full §5.5 first-cut seed lands");
    assert_eq!(
        count(&probe, "character_achievements").await,
        0,
        "no unlocks exist at bootstrap"
    );
    assert_eq!(
        probe
            .query_one(
                "SELECT count(*) FROM achievements WHERE category = 'genre_coverage' AND kind = 'counter'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        12,
        "genre_coverage counters seed with their targets"
    );
    assert_eq!(
        count(&probe, "settings").await,
        SETTINGS_V1_DEFAULTS.len() as i64,
        "all settings defaults land"
    );

    let state = probe
        .query_one(
            "SELECT cs.xp, cs.level, cs.total_watches, cs.current_streak_days,
                    cs.best_streak_days, cs.genres_accessed
             FROM character_state cs",
            &[],
        )
        .await
        .expect("character_state row exists");
    assert_eq!(
        (
            state.get::<_, i64>(0),
            state.get::<_, i32>(1),
            state.get::<_, i32>(2),
            state.get::<_, i32>(3),
            state.get::<_, i32>(4),
            state.get::<_, i32>(5),
        ),
        (0, 1, 0, 0, 0, 1),
        "character_state defaults: level 1, xp 0, streak 0, genres_accessed 1"
    );
    assert_eq!(
        probe
            .query_one("SELECT count(*) FROM character_state WHERE streak_last_watch_date IS NOT NULL", &[])
            .await
            .expect("streak date probe executes")
            .get::<_, i64>(0),
        0,
        "streak_last_watch_date starts NULL"
    );

    let access = probe
        .query_one(
            "SELECT g.name, g.is_opening FROM genre_access ga
             JOIN genres g ON g.id = ga.genre_id",
            &[],
        )
        .await
        .expect("exactly one genre_access row");
    assert_eq!(access.get::<_, String>(0), "Horror");
    assert!(access.get::<_, bool>(1));

    let horror = probe
        .query_one("SELECT list_order FROM genres WHERE name = 'Horror'", &[])
        .await
        .expect("horror genre seeded");
    assert_eq!(horror.get::<_, i32>(0), 1);

    for (key, value) in SETTINGS_V1_DEFAULTS {
        let stored = probe
            .query_opt("SELECT value FROM settings WHERE key = $1", &[key])
            .await
            .expect("settings probe executes");
        assert_eq!(
            stored.map(|row| row.get::<_, String>(0)),
            Some(value.to_string()),
            "setting {key:?} stored verbatim"
        );
    }

    let account = probe
        .query_one("SELECT pin_hash, pin_salts FROM accounts", &[])
        .await
        .expect("account row exists");
    assert!(account.get::<_, String>(0).starts_with("$argon2id$"));
    assert!(!account.get::<_, String>(1).is_empty());
    drop(account);
    drop(probe);

    // (2c) verify with the wrong and the right PIN.
    assert_eq!(
        store.verify_account_pin("2468").await.expect("wrong-PIN verify is not an error"),
        Some(PinVerifyOutcome::Rejected)
    );
    assert_eq!(
        store.verify_account_pin("1357").await.expect("right-PIN verify is not an error"),
        Some(PinVerifyOutcome::Accepted)
    );

    // (4) Idempotency by execution: set again (different PIN, must not take
    // effect) and re-run the seed path; assert zero duplicate rows.
    let (outcome, bootstrap) = store
        .set_account_pin("2468")
        .await
        .expect("second set_account_pin succeeds");
    assert!(!outcome.account_created, "second set must not create another account");
    assert_eq!(
        (bootstrap.character_state_seeded, bootstrap.genre_access_seeded, bootstrap.settings_seeded),
        (0, 0, 0),
        "second set re-seeds nothing"
    );

    let bootstrap = store.bootstrap_single_character().await.expect("re-bootstrap succeeds");
    assert_eq!(
        (bootstrap.character_state_seeded, bootstrap.genre_access_seeded, bootstrap.settings_seeded),
        (0, 0, 0),
        "re-bootstrap re-seeds nothing"
    );

    let probe = fresh_connection(&database_url).await;
    assert_eq!(count(&probe, "accounts").await, 1);
    assert_eq!(count(&probe, "characters").await, 1);
    assert_eq!(count(&probe, "character_state").await, 1);
    assert_eq!(count(&probe, "genre_access").await, 1);
    assert_eq!(count(&probe, "settings").await, 13);
    // The original PIN still verifies; the second PIN never took effect.
    drop(probe);
    assert_eq!(
        store.verify_account_pin("1357").await.expect("original PIN still verifies"),
        Some(PinVerifyOutcome::Accepted)
    );
    assert_eq!(
        store.verify_account_pin("2468").await.expect("second PIN must be rejected"),
        Some(PinVerifyOutcome::Rejected)
    );

    // (5) re-run migrate() on an already-migrated database: clean no-op.
    let summary = store.migrate().await.expect("re-migrate succeeds");
    assert_eq!(summary.applied, 0, "re-migrate applies nothing");
    assert_eq!(summary.already_applied, 10, "re-migrate recognizes all ten versions");

    // (6) HTTP surface: serve the real router on an ephemeral port and drive
    // the full gate flow over TCP (spec §7.3 session mechanics).
    let store = std::sync::Arc::new(tokio::sync::Mutex::new(store));
    let app = movie_rpg::server::router(store);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral bind succeeds");
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server runs");
    });

    let http = reqwest::Client::new();

    // Public routes work without a session.
    let response = http
        .get(format!("{base_url}/healthz"))
        .send()
        .await
        .expect("healthz reachable");
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");

    let status: serde_json::Value = http
        .get(format!("{base_url}/auth/status"))
        .send()
        .await
        .expect("status reachable")
        .json()
        .await
        .unwrap();
    assert_eq!(status["locked"], false, "account exists, gate is ready for login");

    // Gated route without a cookie: 401.
    let response = http
        .get(format!("{base_url}/api/character"))
        .send()
        .await
        .expect("gated route reachable");
    assert_eq!(response.status(), 401);

    // Wrong PIN rejected at the gate.
    let response = http
        .post(format!("{base_url}/auth/login"))
        .json(&serde_json::json!({ "pin": "9999" }))
        .send()
        .await
        .expect("login reachable");
    assert_eq!(response.status(), 401, "wrong PIN must be rejected");

    // Correct PIN sets the session cookie.
    let response = http
        .post(format!("{base_url}/auth/login"))
        .json(&serde_json::json!({ "pin": "1357" }))
        .send()
        .await
        .expect("login reachable");
    assert_eq!(response.status(), 200);
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .expect("login sets the session cookie")
        .to_owned();
    assert!(set_cookie.starts_with("rpg_session="), "cookie is {set_cookie:?}");
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Lax"));
    let token = set_cookie
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("rpg_session=")
        .to_owned();
    let cookie_header = format!("rpg_session={token}");

    // The cookie unlocks the gated API.
    let overview: serde_json::Value = http
        .get(format!("{base_url}/api/character"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("character reachable")
        .json()
        .await
        .unwrap();
    assert_eq!(overview["name"], "The Investigator");
    assert_eq!(overview["level"], 1);
    assert_eq!(overview["xp"], 0);
    assert_eq!(overview["genres"], serde_json::json!(["Horror"]));

    // Logout revokes the session server-side.
    let response = http
        .post(format!("{base_url}/auth/logout"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("logout reachable");
    assert_eq!(response.status(), 200);
    let response = http
        .get(format!("{base_url}/api/character"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("gated route reachable after logout");
    assert_eq!(response.status(), 401, "revoked session must be rejected");

    server_handle.abort();
}
