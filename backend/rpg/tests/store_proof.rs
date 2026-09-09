//! Scratch-Postgres proof for the persistence layer. Skipped unless
//! RPG_DB_URL points at a scratch database (see
//! scripts/scratch_pg_proof.sh for the disposable one-command harness).
//!
//! Exercises the real surface: migrate() over all eleven migrations, the
//! locked/set/verify PIN flows, row-level seed assertions for the bootstrap
//! (character_state, horror genre_access, all 13 settings defaults),
//! idempotency by execution (set + seed twice, zero duplicates), and a
//! no-op re-migrate on an already-migrated database.

use movie_rpg::persistence::{PostgresContentStore, SETTINGS_V1_DEFAULTS, SkipOutcome};
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
    assert_eq!(summary.applied, 11, "all eleven migrations apply on a fresh database");
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
    assert_eq!(summary.already_applied, 11, "re-migrate recognizes all eleven versions");

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

    // Simulate progress: ten horror episode watches plus a level-up.
    let character_id: i64 = probe
        .query_one("SELECT id FROM characters LIMIT 1", &[])
        .await
        .unwrap()
        .get(0);
    let content_id: i64 = probe
        .query_one(
            "INSERT INTO content (source, source_id, title, content_type, genres)
             VALUES ('proof', 'horror-1', 'Proof Horror', 'episode', '[\"Horror\"]'::jsonb)
             RETURNING id",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    for _ in 0..10 {
        probe
            .execute(
                "INSERT INTO watches (character_id, content_id, content_type, pct_viewed, xp_awarded, normal_xp)
                 VALUES ($1, $2, 'episode', 100, 10, 10)",
                &[&character_id, &content_id],
            )
            .await
            .unwrap();
    }
    probe
        .execute(
            "UPDATE character_state SET level = 2, episode_watches = 10, total_watches = 10",
            &[],
        )
        .await
        .unwrap();

    let evaluation = store
        .evaluate_achievements()
        .await
        .expect("evaluation succeeds")
        .expect("a character exists");
    let mut unlocked = evaluation.unlocked.clone();
    unlocked.sort();
    assert_eq!(
        unlocked,
        vec!["episode_10", "horror_homeground_10", "level_up_2"],
        "counters unlock at their targets"
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
    assert_eq!(unlock_rows, 3, "exactly the three unlocks are stored");

    // (5c) Mystery watch orders (§5.7): generation, derived reveals, leakage
    // guard, completion → skip grant → next cycle, and skip spends.
    // A candidate pool: ten horror movies with distinct ratings so the
    // provider-score ranking is deterministic (cycle 1 takes the top five,
    // cycle 2 the next five).
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
    let tick = movie_rpg::game::run_game_tick(&mut store).await.expect("tick succeeds");
    assert_eq!(tick.watches_awarded, 0, "phase 1 is the documented V1 slot");
    assert_eq!(tick.orders.orders_created.len(), 1, "horror is the only accessible genre");
    assert_eq!(tick.orders.orders_created[0].genre, "Horror");
    assert_eq!(tick.orders.orders_created[0].cycle_number, 1);
    assert!(tick.achievements.expect("phase 3 ran").unlocked.is_empty());

    // Generation is deterministic and five movies long.
    let tick = movie_rpg::game::run_game_tick(&mut store).await.expect("idempotent tick");
    assert!(tick.orders.orders_created.is_empty(), "no duplicate generation");

    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert_eq!(board.len(), 1);
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

    // A skip on the revealed item 2 resolves it without a watch row.
    let skip = store.skip_order_item(board[0].id).await.expect("skip succeeds");
    assert_eq!(skip, SkipOutcome::NoSkipsAvailable, "cycle 1 has no balance yet");
    // The failed spend must not leave a stamped skip (transactionality).
    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert!(!board[0].items[1].skipped, "a denied skip stamps nothing");

    // Resolve items 2-5 by watching; the finale cannot be skipped.
    probe
        .execute(
            "INSERT INTO watches (character_id, content_id, content_type, pct_viewed, xp_awarded, normal_xp)
             SELECT $1, i.content_id, 'movie', 100, 10, 10
             FROM watch_order_items i
             WHERE i.order_id = $2 AND i.position > 1",
            &[&character_id, &board[0].id],
        )
        .await
        .unwrap();
    let skip = store.skip_order_item(board[0].id).await.expect("skip succeeds");
    assert_eq!(skip, SkipOutcome::NothingToSkip, "everything is already resolved");

    let tick = movie_rpg::game::run_game_tick(&mut store).await.expect("tick completes");
    assert_eq!(tick.orders.skips_granted, 1, "cycle 1 completion grants one skip");
    let grant_rows: i64 = probe
        .query_one("SELECT count(*) FROM skip_grants WHERE spent_at IS NULL", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(grant_rows, 1, "the grant is one unspent ledger row");

    // Cycle 2 generates for the genre (excludes cycle-1 content by rule).
    let tick = movie_rpg::game::run_game_tick(&mut store).await.expect("cycle 2 generates");
    assert_eq!(tick.orders.orders_created.len(), 1, "one new cycle for horror");
    assert_eq!(tick.orders.orders_created[0].cycle_number, 2);
    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert_eq!(board.len(), 1, "only the active cycle shows");
    assert_eq!(board[0].cycle_number, 2);

    // A skip now spends the earned balance on current item 1.
    let order_id = board[0].id;
    let skip = store.skip_order_item(order_id).await.expect("skip succeeds");
    assert_eq!(skip, SkipOutcome::Skipped { position: 1 });
    let board = store
        .order_view()
        .await
        .expect("order view succeeds")
        .expect("a character exists");
    assert!(board[0].items[0].skipped, "item 1 is resolved via skip");
    assert!(!board[0].items[1].locked, "the skip reveals item 2");
    let balance: i64 = probe
        .query_one("SELECT count(*) FROM skip_grants WHERE spent_at IS NULL", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(balance, 0, "the skip balance is spent");
    let stamped: i64 = probe
        .query_one(
            "SELECT count(*) FROM skip_grants WHERE spent_item_id IS NOT NULL AND spent_at IS NOT NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stamped, 1, "the spent row carries its audit stamp");

    // Skips never earn XP: the character's xp is untouched by the skip flow.
    let xp: i64 = probe
        .query_one("SELECT xp FROM character_state", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(xp, 0, "skips award no XP");
    drop(probe);

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
    // The engine phase leveled the character to 2.
    assert_eq!(overview["level"], 2);
    assert_eq!(overview["xp"], 0);
    assert_eq!(overview["genres"], serde_json::json!(["Horror"]));

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
    assert_eq!(find("genre_explorer_3")["progress"], 1);

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
    assert_eq!(orders.as_array().map(Vec::len), Some(1));
    // Items 1-2 are revealed by now (item 2 via the skip); 3-5 are locked.
    let locked_item = &orders[0]["items"][2];
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
