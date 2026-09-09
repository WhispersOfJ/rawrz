# HANDOFF — Movie / TV RPG work-in-progress

> Resume point for any agent/session. Read this first, then `CLAUDE.md`, then
> `movie-rpg-spec.md` (source of truth).

## Status (updated 2026-09-08)

**Latest pushed commit on `main`:** `8b15e56` — test: prove persistence
surface against scratch postgres. The HTTP server work (binary, PIN gate,
sessions, port correction to 46532) is **uncommitted local work** on top —
commit it when the session asks.

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
- **HTTP server + binary** (`server.rs`, `main.rs`): Axum on **0.0.0.0:46532**
  (port corrected from 86532 — impossible, TCP max 65535; spec §12 Q9
  updated). Session mechanics per spec §7.3 (finalized): opaque 128-bit
  tokens, `rpg_session` HttpOnly/SameSite=Lax cookie, in-memory sessions,
  7-day TTL. Public: `/healthz`, `/auth/status`, `/auth/set-pin`,
  `/auth/login`; gated: `/auth/logout`, `/api/character`. Startup: `.env` →
  connect → `migrate()` → serve. Run: `cargo run -- [path/to/.env]`
  (defaults to `../.env`). End-to-end smoke-tested with curl against a
  scratch Postgres; HTTP flow also covered in the live-DB proof (§6 below).
- Fixtures + full test suite: **61 tests, all passing**
  (`cd backend/rpg && cargo test`)
- Clippy: 3 pre-existing warnings (MetadataCache len_without_is_empty,
  from_sources too_many_arguments, config.rs items_after_test_module) — not
  blockers, not introduced by recent work.

### Next steps (spec §6.4.11 migration order, after character state)

1. ~~Migration 0008 — cases (§6.4.6), then 0009 — featured_cases (§6.4.7)~~
   **Done:** both landed (0008, 0009), including the deferred
   `watches_featured_case` FK and the `cases_featured_case` FK.
2. ~~`achievements` + `character_achievements` (§6.4.8) and the §5.5
   achievement-list seed~~ **Done:** migration 0010 lands both tables
   plus the full first-cut seed (101 rows, kinds/targets/metadata
   finalized — see spec §6.4.8 implementation notes).
3. Achievement **evaluation engine** (poll/sync + case-completion hooks
   writing `character_achievements`).
4. Frontend (Svelte, §8.4) consuming `/auth/*` + `/api/character`.
5. ~~Open design call~~ **Resolved:** PIN hashing (argon2/Argon2id/PHC),
   set/verify flows, and session mechanics (§7.3) — all wired: `auth.rs`,
   `server.rs`, `main.rs`. Sessions are in-memory (restart logs everyone
   out) — acceptable for V1; revisit if restarts become frequent.
3. Frontend (Svelte, §8.4) consuming `/auth/*` + `/api/character`.
4. ~~Open design call~~ **Resolved:** PIN hashing (argon2/Argon2id/PHC),
   set/verify flows, and session mechanics (§7.3) — all wired: `auth.rs`,
   `server.rs`, `main.rs`. Sessions are in-memory (restart logs everyone
   out) — acceptable for V1; revisit if restarts become frequent.

## How to verify

```bash
cd backend/rpg && cargo test        # expect 61 passing
cargo clippy --all-targets          # expect only the 3 known warnings
```

Live-DB proof (disposable postgres container, ~3s run):

```bash
./scripts/scratch_pg_proof.sh       # migrations + PIN flows + seeds on real PG
```

CI (`.github/workflows/validate.yml`) runs the same suite with a Postgres
service container (`store-proof` job); setting `RPG_DB_URL` locally activates
the same proof inside plain `cargo test`.

## Conventions

- Commit style: conventional commits (feat/fix/docs/chore/test).
- Spec-first: update `movie-rpg-spec.md` before design changes; resolve
  "finalize during implementation" items in the spec, not silently.
- No new compose containers; RPG is a separate crate/process, port 46532
  (corrected from 86532, which exceeded the 65535 TCP limit — spec §12 Q9).
- Tests follow the existing pattern: mock TCP servers + fixture files in
  `backend/rpg/fixtures/`, SQL asserted by fragment in unit tests. Note: the
  migration test helper `statement_containing` splits on `;` — avoid `;`
  inside SQL comments in migration files.
