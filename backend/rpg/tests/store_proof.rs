//! Scratch-Postgres proof for the persistence layer. Skipped unless
//! RPG_DB_URL points at a scratch database (see
//! scripts/scratch_pg_proof.sh for the disposable one-command harness).
//!
//! Exercises the real surface: migrate() over all thirteen migrations, the
//! locked/set/verify PIN flows, row-level seed assertions for the bootstrap
//! (character_state, horror genre_access, all 13 settings defaults),
//! idempotency by execution (set + seed twice, zero duplicates), and a
//! no-op re-migrate on an already-migrated database.

use movie_rpg::auth::PinVerifyOutcome;
use movie_rpg::persistence::{PostgresContentStore, SETTINGS_V1_DEFAULTS, SkipOutcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

/// F-1 guard: the proof drops the entire schema of whatever RPG_DB_URL
/// points at. Refuse unless the URL clearly names a scratch database
/// (`rpg_scratch` in the DB name) or the operator opts in explicitly with
/// RPG_ALLOW_DB_RESET=1 — a plain `cargo test` must never be able to wipe
/// a real database by accident.
fn assert_scratch_database(database_url: &str) {
    if std::env::var("RPG_ALLOW_DB_RESET").as_deref() == Ok("1") {
        return;
    }
    let looks_scratch = database_url.contains("rpg_scratch");
    assert!(
        looks_scratch,
        "refusing to reset {database_url}: RPG_DB_URL must name a scratch database \
         (contains 'rpg_scratch') or set RPG_ALLOW_DB_RESET=1 to override"
    );
}

/// Resets to a truly fresh schema, whatever the scratch database currently
/// holds (including nothing, on a virgin container).
async fn reset_database(database_url: &str) {
    assert_scratch_database(database_url);
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

    // (1) migrate() over the full catalog on a fresh database.
    let store = PostgresContentStore::connect(&database_url)
        .await
        .expect("store connects to scratch database");
    let summary = store.migrate().await.expect("migrate() succeeds");
    assert_eq!(summary.applied, 13, "all thirteen migrations apply on a fresh database");
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
    assert_eq!(count(&probe, "wizard_archetypes").await, 6);
    assert_eq!(count(&probe, "character_archetypes").await, 1);
    let starter = probe
        .query_one(
            "SELECT a.slug, c.active_archetype_id::text FROM characters c JOIN wizard_archetypes a ON a.id = c.active_archetype_id",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(starter.get::<_, String>(0), "lantern_scholar");
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
    assert_eq!(summary.already_applied, 13, "re-migrate recognizes every version");

    // (5b-prev) Plex-shaped fixture for tick phase 1 (§9.1 detection): two
    // catalog rows joined to ratingKeys, one fully watched movie, one 96%
    // in-progress movie, one below-threshold item, one unknown key.
    probe
        .execute(
            "INSERT INTO content (source, source_id, title, content_type, genres)
             VALUES ('plex', 'fixture-movie-1', 'Fixture Movie One', 'movie', '[\"Horror\"]'::jsonb),
                    ('plex', 'fixture-movie-2', 'Fixture Movie Two', 'movie', '[\"Horror\"]'::jsonb)
             ON CONFLICT (source, source_id) DO NOTHING",
            &[],
        )
        .await
        .unwrap();
    let plex_fixture = vec![
        movie_rpg::awards::PlexWatchState {
            rating_key: "fixture-movie-1".into(),
            view_count: Some(1),
            view_offset_ms: Some(0),
            duration_ms: Some(600_000),
            item_type: "movie".into(),
        },
        movie_rpg::awards::PlexWatchState {
            rating_key: "fixture-movie-2".into(),
            view_count: None,
            view_offset_ms: Some(960_000),
            duration_ms: Some(1_000_000),
            item_type: "movie".into(),
        },
        movie_rpg::awards::PlexWatchState {
            rating_key: "fixture-below".into(),
            view_count: None,
            view_offset_ms: Some(500_000),
            duration_ms: Some(1_000_000),
            item_type: "movie".into(),
        },
        movie_rpg::awards::PlexWatchState {
            rating_key: "fixture-unknown".into(),
            view_count: Some(1),
            view_offset_ms: Some(0),
            duration_ms: Some(600_000),
            item_type: "movie".into(),
        },
    ];
    let today = chrono::Local::now().date_naive();
    let awarded = store
        .award_plex_watches(&plex_fixture, today)
        .await
        .expect("award pass succeeds");
    assert_eq!(awarded, 2, "two completions, one below threshold, one unknown");
    // Award-once: the same state again awards nothing.
    let awarded = store
        .award_plex_watches(&plex_fixture, today)
        .await
        .expect("second award pass succeeds");
    assert_eq!(awarded, 0, "no re-watch credit in V1");
    // State deltas landed: two movies → 40 XP, movie_watches = 2, streak = 1.
    // The active archetype is still the neutral Lantern Scholar here.
    let state = probe
        .query_one(
            "SELECT xp, movie_watches, total_watches, current_streak_days, level
             FROM character_state",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(state.get::<_, i64>(0), 40, "2 × movie XP (§5.1)");
    assert_eq!(state.get::<_, i32>(1), 2);
    assert_eq!(state.get::<_, i32>(2), 2);
    assert_eq!(state.get::<_, i32>(3), 1, "first ever watch starts the streak");
    assert_eq!(state.get::<_, i32>(4), 1, "40 XP stays level 1 (§5.2)");

    // (5b) Achievement engine (§6.4.8 contract): fresh state evaluates every
    // definition, unlocks nothing, and reports the honest not-evaluable set.
    let evaluation = store
        .evaluate_achievements()
        .await
        .expect("evaluation succeeds")
        .expect("a character exists post-bootstrap");
    assert!(evaluation.unlocked.is_empty(), "fresh state unlocks nothing");
    assert_eq!(
        evaluation.evaluated + evaluation.not_evaluable,
        101,
        "every seeded definition is classified"
    );
    assert!(evaluation.not_evaluable > 0, "V1 has an honest non-evaluable set");
    let wall = store
        .badge_wall()
        .await
        .expect("badge wall succeeds")
        .expect("a character exists post-bootstrap");
    assert_eq!(wall.len(), 101);
    assert!(wall.iter().all(|entry| !entry.unlocked));

    // Drive the unlock facts through the real watch-award path: ten distinct
    // Plex episodes, seven consecutive local dates, and three same-day
    // completions. This proves episode counts, streak/best-streak, milestone
    // XP, and level are all updated by the production transition.
    let character_id: i64 = probe
        .query_one("SELECT id FROM characters LIMIT 1", &[])
        .await
        .unwrap()
        .get(0);
    probe
        .execute(
            "INSERT INTO content (source, source_id, title, content_type, genres)
             SELECT 'plex', 'threshold-episode-' || n, 'Threshold Episode ' || n,
                    'episode', '[\"Horror\"]'::jsonb
             FROM generate_series(1, 10) AS n",
            &[],
        )
        .await
        .unwrap();
    let episode_keys: Vec<String> = probe
        .query("SELECT source_id FROM content WHERE source LIKE 'plex' AND source_id LIKE 'threshold-episode-%' ORDER BY id", &[])
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(episode_keys.len(), 10);
    for (index, rating_key) in episode_keys.iter().enumerate() {
        let day = if index < 7 {
            today + chrono::Duration::days(index as i64)
        } else {
            today + chrono::Duration::days(6)
        };
        let state = movie_rpg::awards::PlexWatchState {
            rating_key: rating_key.clone(),
            view_count: Some(1),
            view_offset_ms: Some(0),
            duration_ms: Some(600_000),
            item_type: "episode".into(),
        };
        assert_eq!(
            store.award_plex_watches(&[state], day).await.unwrap(),
            1,
            "new threshold episode {rating_key} awards once"
        );
        let observed_streak: i32 = probe
            .query_one("SELECT current_streak_days FROM character_state", &[])
            .await
            .unwrap()
            .get(0);
        let expected_streak = if index < 7 { index as i32 + 1 } else { 7 };
        assert_eq!(observed_streak, expected_streak, "streak transition on threshold episode {index}");
    }
    let state = probe
        .query_one(
            "SELECT xp, episode_watches, total_watches, current_streak_days,
                    best_streak_days, level
             FROM character_state",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(state.get::<_, i32>(1), 10);
    assert_eq!(state.get::<_, i32>(2), 12);
    assert_eq!(state.get::<_, i32>(3), 7, "real dated watches build a seven-day streak");
    assert_eq!(state.get::<_, i32>(4), 7);
    assert_eq!(state.get::<_, i32>(5), 2, "watch XP crosses level 2 without a state edit");

    // The next level is reached by another real completed Plex movie, not by
    // editing character_state. This opens the third genre in the cascade.
    probe
        .execute(
            "INSERT INTO content (source, source_id, title, content_type, genres)
             VALUES ('plex', 'threshold-level-movie', 'Threshold Level Movie',
                     'movie', '[\"Horror\"]'::jsonb)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .award_plex_watches(
                &[movie_rpg::awards::PlexWatchState {
                    rating_key: "threshold-level-movie".into(),
                    view_count: Some(1),
                    view_offset_ms: Some(0),
                    duration_ms: Some(600_000),
                    item_type: "movie".into(),
                }],
                today,
            )
            .await
            .unwrap(),
        1,
        "the level-boundary movie awards through the real watch path"
    );
    let state = probe
        .query_one("SELECT xp, level FROM character_state", &[])
        .await
        .unwrap();
    assert_eq!(state.get::<_, i64>(0), 250);
    assert_eq!(state.get::<_, i32>(1), 3, "watch XP crosses level 3 without a state edit");

    // Levels 2 and 3 now drive real genre-access transitions. The store owns
    // both each genre_access row and the character_state counter atomically.
    let accessed = store.access_genre("Thriller").await.unwrap();
    assert_eq!(accessed.name, "Thriller");
    assert_eq!(accessed.genres_accessed, 2);
    assert!(accessed.accessed);
    let accessed = store.access_genre("Mystery").await.unwrap();
    assert_eq!(accessed.name, "Mystery");
    assert_eq!(accessed.genres_accessed, 3);
    assert!(accessed.accessed);
    assert!(store.access_genre("Thriller").await.is_err(), "duplicate access is rejected");

    // Complete one newly accessed Thriller and Mystery title through the same
    // watch-award path. This makes genre diversity an observed ledger fact,
    // rather than a manually edited counter.
    probe
        .execute(
            "INSERT INTO content (source, source_id, title, content_type, genres)
             VALUES ('plex', 'transition-thriller', 'Transition Thriller', 'movie', '[\"Thriller\"]'::jsonb),
                    ('plex', 'transition-mystery', 'Transition Mystery', 'movie', '[\"Mystery\"]'::jsonb)",
            &[],
        )
        .await
        .unwrap();
    let accessed_states = vec![
        movie_rpg::awards::PlexWatchState {
            rating_key: "transition-thriller".into(),
            view_count: Some(1),
            view_offset_ms: Some(0),
            duration_ms: Some(600_000),
            item_type: "movie".into(),
        },
        movie_rpg::awards::PlexWatchState {
            rating_key: "transition-mystery".into(),
            view_count: Some(1),
            view_offset_ms: Some(0),
            duration_ms: Some(600_000),
            item_type: "movie".into(),
        },
    ];
    assert_eq!(
        store.award_plex_watches(&accessed_states, today).await.unwrap(),
        2,
        "newly accessed genre watches award through the real path"
    );

    let evaluation = store
        .evaluate_achievements()
        .await
        .expect("evaluation succeeds")
        .expect("a character exists");
    let mut unlocked = evaluation.unlocked.clone();
    unlocked.sort();
    assert_eq!(
        unlocked,
        vec![
            "episode_10",
            "genre_explorer_3",
            "horror_homeground_10",
            "level_up_2",
            "level_up_3",
        ],
        "real watch counters and genre transitions unlock their targets"
    );
    // Idempotent: a second pass unlocks nothing new.
    let evaluation = store
        .evaluate_achievements()
        .await
        .expect("evaluation succeeds")
        .expect("a character exists");
    assert!(evaluation.unlocked.is_empty(), "unlock writes are idempotent");
    let unlock_rows: i64 = probe
        .query_one("SELECT count(*) FROM character_achievements", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(unlock_rows, 5, "exactly the five real achievement unlocks are stored");

    // The order threshold is still unmet at this point. A locked archetype
    // is rejected without changing the active loadout or creating an event;
    // the real order completion below will unlock it.
    assert!(
        store.select_archetype("veil_cartographer").await.is_err(),
        "veil cartographer is locked before the real order completion"
    );

    // Complete a real mystery order to unlock Veil Cartographer. The order
    // transition derives completion from real watch rows and produces its
    // skip grant; no completed-order row is inserted directly here.
    probe
        .execute(
            "INSERT INTO content (source, source_id, title, content_type, genres, rating)
             SELECT 'plex', 'unlock-order-' || n, 'Unlock Order Movie ' || n,
                    'movie', '[\"Horror\"]'::jsonb, 20 - n
             FROM generate_series(1, 5) AS n",
            &[],
        )
        .await
        .unwrap();
    let order_tick = movie_rpg::game::run_game_tick(&store, None).await.unwrap();
    assert_eq!(order_tick.orders.orders_created.len(), 1);
    let order_id = order_tick.orders.orders_created[0].order_id;
    let order_keys: Vec<String> = probe
        .query(
            "SELECT c.source_id
             FROM watch_order_items i JOIN content c ON c.id = i.content_id
             WHERE i.order_id = $1 ORDER BY i.position",
            &[&order_id],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    let order_states: Vec<_> = order_keys
        .into_iter()
        .map(|rating_key| movie_rpg::awards::PlexWatchState {
            rating_key,
            view_count: Some(1),
            view_offset_ms: Some(0),
            duration_ms: Some(600_000),
            item_type: "movie".into(),
        })
        .collect();
    assert_eq!(
        store.award_plex_watches(&order_states, today).await.unwrap(),
        5,
        "each order movie completes through the real watch-award path"
    );    let completion_tick = movie_rpg::game::run_game_tick(&store, None)
        .await
        .unwrap();
    assert_eq!(completion_tick.orders.skips_granted, 1);

    // Simulate recovery from a process interruption between order completion
    // and reward insertion: the repair path must restore exactly one grant,
    // without duplicating the completed order or changing its history.
    probe
        .execute("DELETE FROM skip_grants WHERE source_order_id = $1", &[&order_id])
        .await
        .unwrap();
    let recovery_tick = movie_rpg::game::run_game_tick(&store, None)
        .await
        .unwrap();
    assert_eq!(recovery_tick.orders.skips_granted, 1);
    assert_eq!(
        probe
            .query_one(
                "SELECT count(*) FROM skip_grants WHERE source_order_id = $1",
                &[&order_id],
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1,
        "completed order recovery restores one grant"
    );

    let unlocked_state = store.archetype_state().await.unwrap().unwrap();
    let locked_slugs: Vec<_> = unlocked_state
        .archetypes
        .iter()
        .filter(|archetype| !archetype.unlocked)
        .map(|archetype| archetype.slug.as_str())
        .collect();
    assert!(
        locked_slugs.is_empty(),
        "all six thresholds materialize through the authoritative evaluator; locked={locked_slugs:?}"
    );
    let _ = store.archetype_state().await.unwrap().unwrap();
    let unlock_events: i64 = probe
        .query_one(
            "SELECT count(*) FROM character_archetype_events WHERE event_type = 'unlock'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        unlock_events, 6,
        "all six unlocks, including the starter, have one durable audit event"
    );

    // Two independent stores race the first real loadout selection. Row
    // locking permits exactly one pending request, with no duplicate accepted
    // state; the winning request is then applied at the next tick boundary.
    let concurrent_a = PostgresContentStore::connect(&database_url).await.unwrap();
    let concurrent_b = PostgresContentStore::connect(&database_url).await.unwrap();
    let (first_selection, second_selection) = tokio::join!(
        concurrent_a.select_archetype("ember_adept"),
        concurrent_b.select_archetype("ember_adept")
    );
    assert_eq!(
        [first_selection.is_ok(), second_selection.is_ok()]
            .into_iter()
            .filter(|accepted| *accepted)
            .count(),
        1,
        "concurrent selection accepts exactly one request"
    );
    // Apply the queued Ember selection through the game-tick transition, not
    // by editing the loadout columns. The next tick is the sole activation
    // boundary used by the production path.
    let _ = store.apply_pending_archetype().await.unwrap();

    // (5c) Mystery watch orders (§5.7): generation, derived reveals, leakage
    // guard, completion → skip grant → next cycle, and skip spends.
    // The earlier real order completion consumed cycle 1, so this candidate
    // pool drives cycle 2 and the next five candidates drive cycle 3.
    probe
        .execute(
            "INSERT INTO content (source, source_id, title, content_type, genres, rating)
             SELECT 'proof', 'mystery-' || n, 'Mystery Movie ' || n, 'movie', '[\"Horror\"]'::jsonb, 10 - n
             FROM generate_series(1, 10) AS n",
            &[],
        )
        .await
        .unwrap();
    // Every order action below goes through the game tick (§9.1 phases).
    let tick = movie_rpg::game::run_game_tick(&store, None).await.expect("tick succeeds");
    assert_eq!(tick.watches_awarded, 0, "phase 1 is the documented V1 slot");
    assert_eq!(tick.orders.orders_created.len(), 1, "horror is the only accessible genre");
    assert_eq!(tick.orders.orders_created[0].genre, "Horror");
    assert_eq!(
        tick.orders.orders_created[0].cycle_number,
        2,
        "the real order completion above already consumed cycle 1"
    );
    assert!(tick.achievements.expect("phase 3 ran").unlocked.is_empty());

    // Generation is deterministic and five movies long.
    let tick = movie_rpg::game::run_game_tick(&store, None).await.expect("idempotent tick");
    assert!(tick.orders.orders_created.is_empty(), "no duplicate generation");

    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    // F-30: the completed cycle 1 remains on the board as history; the
    // active cycle 2 sorts first.
    assert_eq!(board.len(), 2);
    assert_eq!(board[0].status, "active");
    assert_eq!(board[1].status, "completed");
    let items = &board[0].items;
    assert_eq!(items.len(), 5);
    assert!(!items[0].locked, "item 1 is revealed at creation");
    assert!(items[0].title.is_some(), "revealed item carries its title");
    assert!(items[1..].iter().all(|item| item.locked), "items 2-5 are locked");
    // THE LEAKAGE GUARD: locked items carry nothing but position + locked.
    for item in items.iter().skip(1) {
        assert!(item.content_id.is_none() && item.title.is_none() && item.year.is_none());
    }

    // The 95% mystery rule is the §6.4.5 ledger: watch current item 1, and
    // item 2 reveals — with no stored completion state involved.
    probe
        .execute(
            "INSERT INTO watches (character_id, content_id, content_type, pct_viewed, xp_awarded, normal_xp)
             SELECT $1, i.content_id, 'movie', 95, 10, 10
             FROM watch_order_items i WHERE i.order_id = $2 AND i.position = 1",
            &[&character_id, &board[0].id],
        )
        .await
        .unwrap();
    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert!(!board[0].items[1].locked, "watching item 1 reveals item 2");
    assert!(board[0].items[2].locked, "item 3 stays locked");

    // A skip on the revealed item 2 resolves it without a watch row. The
    // earlier real order completion supplied this balance, proving the grant
    // ledger carries across orders.
    let skip = store.skip_order_item(board[0].id).await.expect("skip succeeds");
    assert_eq!(skip, SkipOutcome::Skipped { position: 2 });
    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert!(board[0].items[1].skipped, "the earned skip resolves item 2");

    // The second cycle is the final V1 cycle for this genre. Leave its
    // remaining items unresolved so the proof can continue to exercise the
    // mystery-safe active-order API; completion and skip-grant creation were
    // already proven by the first real order above.
    let tick = movie_rpg::game::run_game_tick(&store, None).await.expect("idempotent tick");
    assert!(tick.orders.orders_created.is_empty(), "V1 caps the genre at two cycles");
    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert_eq!(board.len(), 2, "active cycle 2 plus completed cycle 1 as history");
    assert_eq!(board[0].cycle_number, 2);
    let balance: i64 = probe
        .query_one("SELECT count(*) FROM skip_grants WHERE spent_at IS NULL", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(balance, 0, "the first real completion's skip was spent");
    let stamped: i64 = probe
        .query_one(
            "SELECT count(*) FROM skip_grants WHERE spent_item_id IS NOT NULL AND spent_at IS NOT NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stamped, 1, "the spent row carries its audit stamp");

    // Skips never earn XP: the pre-skip XP is unchanged by the skip flow.
    // This includes all real threshold watches, streak milestones, the
    // level-boundary movie, genre-transition movies, and the completed order.
    let xp: i64 = probe
        .query_one("SELECT xp FROM character_state", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(xp, 390, "watch-award XP only — the skip added nothing");

    // (5e) Poll cycle (§9.1, poll.rs): a real `run_poll_cycle` against a
    // Plex-shaped mock stack (Plex healthy; Sonarr/Radarr unreachable;
    // providers failing) proves the unattended cycle end-to-end on real
    // Postgres: sync tolerates partial stack failure and persists the
    // catalog, tick phase 1 awards through the same client surface the
    // loop uses, phases 2–3 cascade, and the degraded-sync rule holds —
    // the two earlier fixture watches are not re-awarded, while the newly
    // synced completed movie earns its one normal movie award.
    let pre_content_rows = count(&probe, "content").await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock stack bind succeeds");
    let mock_addr = listener.local_addr().unwrap();
    let mock_server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let mut request = [0_u8; 8192];
            let bytes_read = stream.read(&mut request).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..bytes_read]);
            let (status, body, content_type) = if request.contains("/library/sections")
                && !request.contains("/all")
            {
                (200, include_str!("../fixtures/plex_sections.xml"), "application/xml")
            } else if request.contains("/library/sections/1/all") {
                (200, include_str!("../fixtures/plex_library.xml"), "application/xml")
            } else if request.contains("/library/sections/2/all") {
                (200, "<MediaContainer size=\"0\"></MediaContainer>", "application/xml")
            } else if request.contains("/api/v3/") {
                // Sonarr/Radarr unreachable: non-retryable status.
                (404, "not found", "text/plain")
            } else {
                // Every metadata provider fails (retryable): the sync must
                // still succeed with zero provider-cache rows.
                (503, "temporary", "text/plain")
            };
            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nRetry-After: 0\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                reason,
                content_type,
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let mock_base = format!("http://{mock_addr}");
    let stack = movie_rpg::poll::PollStack {
        plex: movie_rpg::stack::PlexClient::with_base_url(&mock_base, "plex-token"),
        sonarr: movie_rpg::stack::SonarrClient::with_base_url(&mock_base, "sonarr-key"),
        radarr: movie_rpg::stack::RadarrClient::with_base_url(&mock_base, "radarr-key"),
        tmdb: movie_rpg::providers::TmdbClient::with_base_url(format!("{mock_base}/tmdb"), "k"),
        tvdb: movie_rpg::providers::TvdbClient::with_base_url(format!("{mock_base}/tvdb"), "k"),
        omdb: movie_rpg::providers::OmdbClient::with_base_url(format!("{mock_base}/omdb"), "k"),
        fanart: movie_rpg::providers::FanartClient::with_base_url(format!("{mock_base}/fanart"), "k"),
    };
    let summary = movie_rpg::poll::run_poll_cycle(&store, &stack).await;
    let sync = summary.sync.as_ref().expect("sync tolerates stack+provider failures");
    assert_eq!(
        sync.persistence.content_rows,
        2,
        "the two Plex fixture items sync into the catalog"
    );
    assert_eq!(
        sync.persistence.provider_cache_rows,
        2,
        "failed provider attempts persist auditable cache failure rows"
    );
    assert_eq!(sync.prepared.outcome.stack_failures.len(), 2, "sonarr + radarr failures logged");
    let tick = summary.tick.as_ref().expect("tick runs after degraded sync");
    assert_eq!(tick.watches_awarded, 1, "movie 271 completes (viewCount), the show does not");
    assert_eq!(
        tick.orders.orders_created.len(), 0,
        "existing orders are active; refresh generates nothing"
    );
    let line = movie_rpg::poll::format_cycle_log(&summary);
    assert!(line.contains("+2 content/2 cache rows (2 stack"), "{line}");
    assert!(line.contains("1 watches awarded"), "{line}");
    let probe = fresh_connection(&database_url).await;
    let total_content = count(&probe, "content").await;
    assert_eq!(
        total_content,
        pre_content_rows + 2,
        "sync added exactly the two Plex rows"
    );
    let watch_rows: i64 = probe
        .query_one(
            "SELECT count(*) FROM watches WHERE content_id = (
                 SELECT id FROM content WHERE source = 'plex' AND source_id = '271')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(watch_rows, 1, "tick phase 1 awarded the synced movie once");
    let xp: i64 = probe
        .query_one("SELECT xp FROM character_state", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        xp, 412,
        "the active Ember Adept adds +10% to the new movie award"
    );
    let award_rows = probe
        .query(
            "SELECT c.source_id, w.xp_awarded, w.normal_xp
             FROM watches w JOIN content c ON c.id = w.content_id
             WHERE w.character_id = (SELECT id FROM characters LIMIT 1)
             ORDER BY w.id",
            &[],
        )
        .await
        .unwrap();
    let fixture_awards: Vec<_> = award_rows
        .iter()
        .filter(|row| matches!(row.get::<_, String>(0).as_str(), "fixture-movie-1" | "fixture-movie-2" | "271"))
        .collect();
    assert_eq!(fixture_awards.len(), 3, "two historical fixture awards plus one new award");
    assert_eq!(fixture_awards[0].get::<_, i64>(1), 20, "historical award remains unchanged");
    assert_eq!(fixture_awards[0].get::<_, i64>(2), 20, "historical normal XP remains unchanged");
    assert_eq!(fixture_awards[1].get::<_, i64>(1), 20, "historical award remains unchanged");
    assert_eq!(fixture_awards[2].get::<_, i64>(1), 22, "new award uses active archetype effect");
    // F-6 ledger semantics: normal_xp stays the neutral §5.1 value; the
    // archetype adjustment lives only in xp_awarded.
    assert_eq!(
        fixture_awards[2].get::<_, i64>(2),
        20,
        "new award's normal XP stays the neutral movie value"
    );
    drop(probe);

    // (5f) Loop-level contract on the real store: an already-signalled
    // shutdown runs zero cycles and returns the completed-cycle count.
    let store_handle = std::sync::Arc::new(store);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    shutdown_tx
        .send(true)
        .expect("shutdown receiver is alive");
    let cycles = movie_rpg::poll::run_poll_loop(
        store_handle.clone(),
        stack,
        std::time::Duration::from_millis(50),
        shutdown_rx,
    )
    .await;
    assert_eq!(cycles, 0, "pre-signalled shutdown completes zero cycles");
    mock_server.abort();

    // (6) HTTP surface: serve the real router on an ephemeral port and drive
    // the full gate flow over TCP (spec §7.3 session mechanics).
    let app = movie_rpg::server::router(store_handle.clone());
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

    // The cookie unlocks the gated API, including the first Lantern Academy
    // endpoint. The response is server-owned state for all six definitions.
    let archetypes: serde_json::Value = http
        .get(format!("{base_url}/api/archetypes"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("archetypes reachable")
        .json()
        .await
        .expect("archetypes are json");
    assert_eq!(archetypes["active_archetype"], "ember_adept");
    assert!(archetypes["pending_archetype"].is_null());
    assert_eq!(archetypes["archetypes"].as_array().map(Vec::len), Some(6));
    assert_eq!(
        archetypes["archetypes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["unlocked"] == true)
            .count(),
        6,
        "all six archetypes remain unlocked after the threshold proof"
    );

    // The daily selection guard is enforced through the actual HTTP surface;
    // it must not mutate the active or pending state.
    let response = http
        .post(format!("{base_url}/api/archetypes/lantern_scholar/select"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("archetype selection reachable");
    assert_eq!(response.status(), 409, "same-day selection is a conflict");
    let after_rejection: serde_json::Value = http
        .get(format!("{base_url}/api/archetypes"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("archetypes remain reachable")
        .json()
        .await
        .unwrap();
    assert_eq!(after_rejection["active_archetype"], "ember_adept");
    assert!(after_rejection["pending_archetype"].is_null());

    // The cookie unlocks the rest of the gated API.
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
    // The phase-1 fixture adds 22 XP: the two historical neutral awards were
    // already included, and the newly synced movie earns 22 under Ember
    // Adept. The accumulated real progression is level 3.
    assert_eq!(overview["level"], 3);
    assert_eq!(overview["xp"], 412);
    assert_eq!(
        overview["genres"],
        serde_json::json!(["Horror", "Thriller", "Mystery"]),
        "real genre-access transitions are reflected in the character API"
    );

    // The badge wall reflects the stored unlocks and live progress.
    let wall: Vec<serde_json::Value> = http
        .get(format!("{base_url}/api/achievements"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("achievements reachable")
        .json()
        .await
        .expect("badge wall is a json array");
    assert_eq!(wall.len(), 101);
    let find = |slug: &str| {
        wall.iter()
            .find(|entry| entry["slug"] == slug)
            .unwrap_or_else(|| panic!("badge wall is missing {slug}"))
    };
    assert_eq!(find("horror_homeground_10")["unlocked"], true);
    assert_eq!(find("level_up_2")["unlocked"], true);
    assert_eq!(find("episode_100")["unlocked"], false);
    assert_eq!(find("episode_100")["progress"], 10);
    assert_eq!(find("episode_100")["target"], 100);
    assert_eq!(find("genre_explorer_3")["progress"], 3);

    // The order endpoint serves the mystery-safe JSON shape: a locked item
    // has no title/content fields at all (not just null).
    let orders: serde_json::Value = http
        .get(format!("{base_url}/api/orders"))
        .header("cookie", &cookie_header)
        .send()
        .await
        .expect("orders reachable")
        .json()
        .await
        .expect("orders are a json array");
    assert_eq!(orders.as_array().map(Vec::len), Some(2), "active + completed history");
    // Items 1-3 are revealed by now (item 2 via the skip); 4-5 remain
    // locked because item 3 has not been watched.
    let locked_item = &orders[0]["items"][3];
    assert_eq!(locked_item["locked"], true);
    assert!(locked_item.get("title").is_none(), "locked item leaks no title");
    assert!(locked_item.get("content_id").is_none(), "locked item leaks no content id");
    let revealed_item = &orders[0]["items"][0];
    assert_eq!(revealed_item["locked"], false);
    assert!(revealed_item["title"].is_string());

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
