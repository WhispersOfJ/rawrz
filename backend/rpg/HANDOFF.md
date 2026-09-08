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
- Config/env loading: `backend/rpg/src/config.rs`
- Fixtures + full test suite: **36 tests, all passing**
  (`cd backend/rpg && cargo test`)
- Clippy: 3 pre-existing warnings (MetadataCache len_without_is_empty,
  from_sources too_many_arguments, config.rs items_after_test_module) — not
  blockers, not introduced by recent work.

### Next steps (spec §6.4.11 migration order, after identity)

1. **Migration 0004 — genre tables** (§6.4.3): `genres` (seed horror-first fixed
   list + `list_order` + `is_opening`), `sub_genres`, `genre_access`,
   `sub_genre_xp` (100 XP purchase threshold, §5.2).
2. **Migration 0005 — character_state** (§6.4.2): xp/level/total_watches/
   streak columns, singleton per character.
3. Then: `watches` → `cases` → `featured_cases` → `achievements` +
   `character_achievements` → `settings` (V1 defaults in §6.4.10).
4. **Open design call to resolve during implementation:** PIN hashing crate
   (spec suggests argon2) for `accounts.pin_hash` / `pin_salts` — finalize in
   spec per CLAUDE.md ("spec-first") when decided.

## How to verify

```bash
cd backend/rpg && cargo test        # expect 36+ passing
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
