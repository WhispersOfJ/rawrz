# HANDOFF — Movie / TV RPG work-in-progress

> Resume point for any agent/session. Read this first, then `CLAUDE.md`, then
> `movie-rpg-spec.md` (source of truth).

## Status (updated 2026-09-08)

**Unpushed commit on `main`:** `a285ea0` — feat: add accounts and characters
identity migration. **User said "stop at next push" — do not push without
asking.**

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
- Config/env loading: `backend/rpg/src/config.rs`
- Fixtures + full test suite: **38 tests, all passing**
  (`cd backend/rpg && cargo test`)
- Clippy: 3 pre-existing warnings (MetadataCache len_without_is_empty,
  from_sources too_many_arguments, config.rs items_after_test_module) — not
  blockers, not introduced by recent work.

### Next steps (spec §6.4.11 migration order, after character state)

1. **Migration 0006 — watches + genre_xp_ledger** (§6.4.5 + §6.4.3): the RPG's
   awarded-completion ledger, plus the ledger's deferred `source_watch_id` FK.
2. Then: `cases` → `featured_cases` → `achievements` +
   `character_achievements` → `settings` (V1 defaults in §6.4.10).
3. **Open design call to resolve during implementation:** PIN hashing crate
   (spec suggests argon2) for `accounts.pin_hash` / `pin_salts` — finalize in
   spec per CLAUDE.md ("spec-first") when decided.
4. Seeding on character creation (`character_state` row, `genre_access` horror
   row, `settings` V1 defaults) lands with the app's account/character
   bootstrap logic — not in migrations (single-account V1 has no rows to seed
   until first run).

## How to verify

```bash
cd backend/rpg && cargo test        # expect 38+ passing
cargo clippy --all-targets          # expect only the 3 known warnings
```

## Conventions

- Commit style: conventional commits (feat/fix/docs/chore/test).
- Spec-first: update `movie-rpg-spec.md` before design changes; resolve
  "finalize during implementation" items in the spec, not silently.
- No new compose containers; RPG is a separate crate/process, port 86532.
- Tests follow the existing pattern: mock TCP servers + fixture files in
  `backend/rpg/fixtures/`, SQL asserted by fragment in unit tests. Note: the
  migration test helper `statement_containing` splits on `;` — avoid `;`
  inside SQL comments in migration files.
