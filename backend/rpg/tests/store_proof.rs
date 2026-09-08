//! Scratch-Postgres proof for the persistence layer. Skipped unless
//! RPG_DB_URL points at a scratch database (see
//! scripts/scratch_pg_proof.sh for the disposable one-command harness).
//!
//! Exercises the real surface: migrate() over all six migrations, the
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
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
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
    assert_eq!(summary.applied, 6, "all six migrations apply on a fresh database");
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
    assert_eq!(summary.already_applied, 6, "re-migrate recognizes all six versions");
}
