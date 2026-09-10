# HANDOFF — Movie / TV RPG work-in-progress

> Resume point for any agent/session. Read this first, then `CLAUDE.md`, then
> `movie-rpg-spec.md` (source of truth).

## Status (updated 2026-09-10; FIX.md remediation complete)

**Latest pushed commit on `main`:** `f5a5f5c` — docs: finalize Lantern Academy
wizard contract. Local work now **remediates every finding in FIX.md** (F-1
through F-39; see the status block at the top of FIX.md for the per-item map)
and extends the Lantern Academy archetype slice. The current changes are
uncommitted; preserve the untracked `.freebuff/` runtime directory. Spell
implementation, frontend work, and the remaining wizard economy are still
deferred.

### Review & decisions

- **FIX.md** is the full-codebase review (2026-09-10): bugs, security, leaks,
  perf, UX, hygiene — **all findings are remediated and verified** (89 unit
  tests + 1 live scratch-Postgres proof, zero clippy warnings). Highlights:
  - **Pool architecture (F-17/F-19):** the shared `Mutex<PostgresContentStore>`
    is gone. `PostgresContentStore` owns a `deadpool-postgres` pool; server and
    poll loop share `Arc<PostgresContentStore>`. Store methods take `&self`.
  - **Security (F-10/F-11/F-39):** failed-login throttle (5 attempts → 30s×2^n
    in-memory lockout), set-pin 409 + no-hash for existing accounts, set-pin
    issues a session (F-29), sessions sweep on issue and renew on activity.
  - **Data model (F-6/F-7):** migration 0013 adds `UNIQUE (character_id,
    content_id)` on `watches`; the ledger now stores neutral `normal_xp` vs
    adjusted `xp_awarded` and records streak milestones in `bonuses`.
  - **API surface (F-3/F-30–F-35):** gated `POST /api/genres/{name}/access`,
    completed orders stay on the board (`status` field), skip flow generates
    next cycles and embeds the refreshed board, `/api/orders/refresh` returns
    a trimmed `TickSummary` with `skipped_genres`, hidden badges masked.
  - **Sync (F-2/F-4/F-16/F-22):** TV episodes sync with show-parent linkage
    (stack-based XML parser, `watchable_items`), batched content/cache upserts
    (2 statements per sync), fresh-only hydration + 7-day-grace retention.
  - **Robustness (F-5/F-14/F-18/F-28):** TVDB 401 → re-login + retry once,
    4 MiB body caps, one shared `reqwest::Client`, `ProbeError::OutOfRange`.
  - **Config (F-12/F-26/F-27):** `RPG_BIND_ADDRESS`, exe-relative env-file
    fallback, inline `#` comments in `.env` values.
  - Deferred on purpose: `tracing` subscriber wiring (F-24 covers cycle
    counters only; the `tracing` deps were removed until wired), `Secure`
    cookie flag (F-13, TLS-era), `poll_interval_seconds` setting read (F-25,
    documented as reserved — default equals `POLL_INTERVAL`).
- **No Redis / external caching layer** — decided 2026-09-10, rationale and
  revisit trigger documented in FIX.md §8 (cloud-only catalog options, no-new-
  container constraint, and V1 scale make the in-process fixes the right call).

### Done so far

- Content sync pipeline (Plex/Sonarr/Radarr → normalize → enrich → Postgres):
  `backend/rpg/src/{stack,providers,normalization,enrichment,sync,persistence,pipeline}.rs`
- Migration runner + catalog, `backend/rpg/src/migrations.rs`:
  - `0001_content_provider_cache.sql` — content mirror + provider cache (§6.4.4/§6.4.4a)
  - `0002_sync_state.sql` — poll cursors, FK to characters **deferred**
  - `0003_accounts_characters.sql` — accounts + characters (§6.4.1), **completes
    the deferred sync_state FK**
  - `0004_genres.sql` — genres + sub_genres + genre_access + sub_genre_xp
    (§6.4.3), seeds the **finalized §5.2 genre list** (Horror → Thriller →
    Mystery → Sci-Fi → Fantasy → Documentary → Comedy → Drama → Romance →
    Animation; horror opening, 10 genres = 10-level table). `genre_xp_ledger`
    is **deferred to the watches migration** (its `source_watch_id` FK targets
    `watches(id)` — spec §6.4.3 documents this placement).
  - `0005_character_state.sql` — singleton character_state (§6.4.2)
  - `0006_settings.sql` — settings key/value table (§6.4.10). Created early
    (not last) because bootstrap seeding needs it — placement note in spec
    §6.4.11.
  - `0007_watches.sql` — watches ledger (§6.4.5) + the deferred
    `genre_xp_ledger` (§6.4.3, `source_watch_id` FK now resolvable).
    `watches.featured_case_id` is a plain column — its FK to `featured_cases`
    (0009) is deferred, same pattern as sync_state/genre_xp_ledger.
  - `0008_cases.sql` — cases case-board table (§6.4.6) with
    `completion_watch_id` FK and `featured_case_id` created plain (FK
    deferred to 0009, same pattern).
  - `0009_featured_cases.sql` — featured_cases (§6.4.7, ISO-week `period`
    `YYYY-Www`, weekly V1 cadence) and completion of **both** deferred
    FKs: `watches_featured_case` (from 0007) and `cases_featured_case`
    (from 0008).
- Config/env loading: `backend/rpg/src/config.rs`
- Character-creation bootstrap: `PostgresContentStore::bootstrap_single_character`
  (persistence.rs) — one transaction, idempotent: resolves the single account's
  character, seeds `character_state` (defaults), the `genre_access` horror row
  (`genres.is_opening`), and **missing** `settings` V1 defaults
  (`SETTINGS_V1_DEFAULTS`, §6.4.10 verbatim; `featured_selection_mode`
  finalized as `all_time_ranking`). No-op when no account exists yet.
- PIN gate: `backend/rpg/src/auth.rs` — `argon2` (Argon2id, PHC string) +
  `rand_core`/getrandom; `validate_pin` (4–12 digits), `hash_pin` →
  `PinHash { phc_string, salt_b64 }`, `verify_pin` → Accepted/Rejected.
  Store flows in persistence.rs: `set_account_pin` (first-run: validate →
  hash → insert account → create default character → shared seed, one
  transaction, idempotent on re-run), `verify_account_pin` (missing
  account = locked/`None`; wrong PIN = `Rejected`, not an error),
  `account_exists`, `character_overview` (gated API payload).
- **HTTP server + binary** (`server.rs`, `main.rs`): Axum on port **46532**
  (port corrected from 86532 — impossible, TCP max 65535; spec §12 Q9
  updated; bind address configurable via `RPG_BIND_ADDRESS`, default
  `0.0.0.0:46532`). Session mechanics per spec §7.3 (finalized): opaque
  128-bit tokens, `rpg_session` HttpOnly/SameSite=Lax cookie, in-memory
  sessions, 7-day TTL with **sliding renewal** and sweep-on-issue. Public:
  `/healthz`, `/auth/status`, `/auth/set-pin`, `/auth/login`; gated:
  `/auth/logout`, `/api/character`, `/api/archetypes`,
  `/api/archetypes/{slug}/select`, `/api/genres/{name}/access`,
  `/api/achievements`, `/api/orders`, `/api/orders/refresh`,
  `/api/orders/{id}/skip`. Login is throttled (5 failures → 30s×2^n
  lockout); set-pin is 409 once an account exists and issues a session on
  creation. Startup: `.env` (exe-relative fallback) → connect → `migrate()`
  once → serve. The binary also starts the shared 5-minute poll loop
  (`poll.rs`): `SyncPipeline::run` precedes `run_game_tick`; server and
  loop share the pooled store, so no request ever queues behind a poll
  cycle. Ctrl-C drains the HTTP server, then signals the loop to finish its
  current cycle. Each cycle logs sync persistence and all tick phase counts.
  Run: `cargo run -- [path/to/.env]` (defaults to `../.env`, falls back to
  the exe-relative repo root). HTTP and poll flows are covered in the
  live-DB proof.
- Game tick (§9.1, phase order finalized): `backend/rpg/src/game.rs` —
  `run_game_tick(store, Option<&PlexClient>)` runs the ordered phases in
  one place: watch award (`awards.rs` pure computation +
  `award_plex_watches` store flow — dedupes on `(character_id, content_id)`,
  XP 20/10 per §5.1, streak rules per §5.1.1) → order reveals
  (`refresh_watch_orders`) → achievement evaluation. A Plex detection/award
  failure is logged and does not prevent phases 2–3. `POST
  /api/orders/refresh` is the stack-less tick UI entry; skips advance reveals
  + evaluation for their order.
- Unattended polling (§9.1): `poll.rs` owns `PollStack`, `run_poll_cycle`,
  fixed `POLL_INTERVAL` (300 seconds), one-line cycle logging, non-fatal sync
  and phase-1 degradation, and watch-channel shutdown. `main.rs` shares the
  pooled `Arc<PostgresContentStore>` with Axum; the loop counts failed
  cycles in its exit line.
- Lantern Academy archetype slice (local, uncommitted): migration   `0012_wizard_archetypes.sql`, `wizard.rs` (pure rules), and
   `wizard_store.rs` (Postgres adapter), with starter bootstrap integration,
   catalog/selection API, and next-tick application in `game.rs`. The store
  adapter owns all archetype SQL, durable unlock/event materialization, and
  loadout reads/writes; `persistence.rs` owns neutral persistence and delegates
  to that boundary.
- Fixtures + full test suite: **89 unit tests + 1 live proof, all passing**
  (`cd backend/rpg && cargo test --all-targets`). The archetype store boundary
  is `wizard_store.rs`; the pure selection policy is in `wizard.rs`.
- Clippy: **zero warnings** (`cargo clippy --all-targets`).

### Next steps

1. ~~Migration 0008 — cases (§6.4.6), then 0009 — featured_cases (§6.4.7)~~
   **Done:** both landed (0008, 0009), including the deferred
   `watches_featured_case` FK and the `cases_featured_case` FK.
2. ~~`achievements` + `character_achievements` (§6.4.8) and the §5.5
   achievement-list seed~~ **Done:** migration 0010 lands both tables
   plus the full first-cut seed (101 rows, kinds/targets/metadata
   finalized — see spec §6.4.8 implementation notes).
3. ~~Achievement **evaluation engine**~~ **Done:** `achievements.rs`
   (pure kind-dispatch engine over a `ProgressSnapshot`) +
   `evaluate_achievements`/`badge_wall` store flows + gated
   `GET /api/achievements`. Evaluates counter/streak/level shapes from
   live aggregates; combo/once/metadata-dependent rows honestly stay
   unevaluated. Unlocks are idempotent and run in the game tick.
4. ~~PIN hashing, session mechanics, and game-tick phase 1~~ **Done:**
   Argon2id PIN gate, in-memory sessions, Plex watch-state awards, XP,
   streaks, reveals, and achievements are all wired and proven on real
   Postgres.
5. ~~Unattended 5-minute poll loop~~ **Done:** `poll.rs` owns
   `SyncPipeline::run` → `run_game_tick`, fixed cadence, per-cycle logging,
   non-fatal sync/phase-1 degradation, and shutdown coordination; `main.rs`
   shares the store with Axum and waits for the poll task after the server
   drains.
6. **Wizard-academy contract — revised, implementation deferred:**
   `movie-rpg-spec.md` now defines the cozy scholarly Lantern Academy, a
   backend-first vertical release, six pre-designed archetypes with one
   primary mechanic and at most one bounded secondary modifier, immutable
   next-tick archetype selection, five spells with player-selected one-
   discipline affinities, visible watch-driven charge meters, explicit
   thresholds/overflow, guided UI behavior, and server-owned transactional
   auditability. The archetype matrix, exact spell economy, migration fields,
   API payload fields, transactional sequence, and implementation order are
   now explicit. No code, migration, asset, or frontend implementation
   belongs to this documentation pass.
7. **Lantern Academy archetype slice — in progress locally:** migration
   `0012_wizard_archetypes.sql` seeds all six original archetypes, backfills
   existing characters to `lantern_scholar`, and adds permanent unlock rows,
   audit events, active/pending selection fields, and the starter bootstrap.
   `wizard.rs` owns pure unlock predicates and bounded normal-XP effects;
   `wizard_store.rs` is the Postgres adapter for archetype facts, unlock/event
   materialization, catalog reads, queued selection, and tick-boundary
   application. `persistence.rs` delegates archetype state to that boundary
   and applies the active archetype only to newly inserted watch rows
   transactionally, leaving historical rows unchanged. The live proof now
   drives its progression facts through real `award_plex_watches` completions
   (episode/movie counts, dated streaks, XP/levels), `access_genre`
   transactions, `evaluate_achievements`, and real watch-order generation and
   completion; it no longer updates threshold counters or inserts completed
   orders synthetically. It covers threshold rejection, audit idempotency,
   queued selection, next-tick activation, active Ember XP, historical
   immutability, and the concurrent selection conflict. `game.rs` applies
   pending selection before the existing watch → order → achievement phases;
   `server.rs` exposes gated
   `GET /api/archetypes` and `POST /api/archetypes/{slug}/select`.
8. **Next implementation pass:** add the remaining 0012 resource state and
   then migration 0014 for spells, affinity, charges, and casts (0013 is now
   the `watches` unique-award constraint); add pure
   affinity/cap/overflow calculations, apply queued affinity at the tick
   boundary, and run live proofs for all six archetypes and five spells. Then
   build the guided Svelte UI against those APIs. Keep the existing neutral
   `watches`, XP, streak, genre, achievement, `watch_orders`, and `skip_grants`
   ledgers authoritative. Optional reliability polish: wire a `tracing`
   subscriber (F-24 logging half) and re-read `poll_interval_seconds` from
   settings (F-25) when the frontend needs it.

## How to verify

```bash
cd backend/rpg && cargo test --all-targets  # expect 89 unit tests + 1 live proof passing
cargo clippy --all-targets                 # expect zero warnings
```

Live-DB proof (disposable postgres container, ~4s run):

```bash
./scripts/scratch_pg_proof.sh       # migrations + PIN flows + seeds on real PG
```

CI (`.github/workflows/validate.yml`) runs the same suite with a Postgres
service container (`store-proof` job); setting `RPG_DB_URL` locally activates
the same proof inside plain `cargo test`.

## Conventions

- Commit style: conventional commits (feat/fix/docs/chore/test).
- Spec-first: update `movie-rpg-spec.md` before design changes; resolve
  "finalize during implementation" items in the spec, not silently. The
  Lantern Academy rebrand is documented as original IP; do not introduce
  copyrighted franchise names, characters, logos, spells, or film artwork.
  Preserve the next-tick boundary, one-primary/one-secondary archetype
  shape, one-discipline affinity rule, watch-only meter input, and explicit
  charge overflow audit unless the spec is revised again.
- No new compose containers; RPG is a separate crate/process, port 46532
  (corrected from 86532, which exceeded the 65535 TCP limit — spec §12 Q9).
- Tests follow the existing pattern: mock TCP servers + fixture files in
  `backend/rpg/fixtures/`, SQL asserted by fragment in unit tests. Note: the
  migration test helper `statement_containing` splits on `;` — avoid `;`
  inside SQL comments in migration files.
