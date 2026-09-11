# RAWRZ — \*arr SQLite → Postgres migration runbook

> **Status:** Draft v0.1 — 2026-09-11. Companion to `rawrz-megastack-spec.md` §8 (D21, D22)
> and `rawrz-postgres-hardening.md`.
> **Scope:** migrate Prowlarr, Sonarr, and Radarr from SQLite onto the RAWRZ Postgres instance,
> one application per maintenance window, in that order.
> **Risk class:** the highest in the RAWRZ program. This plan assumes the migration mechanism is
> **prototyped on copies in a scratch environment before it is ever run on live data** (§5).
> **Live state is never the first place any step in this document runs.**

---

## 1. The decision that makes this safe: the switch is environment-driven

The pinned \*arr versions bind Postgres settings from configuration that is populated from
**environment variables**:

```csharp
// ~/TRUTH/sonarr/src/NzbDrone.Core/Datastore/PostgresOptions.cs
public static PostgresOptions GetOptions() {
    var config = new ConfigurationBuilder().AddEnvironmentVariables().Build();
    var postgresOptions = new PostgresOptions();
    config.GetSection("Sonarr:Postgres").Bind(postgresOptions);
    return postgresOptions;
}
// Bootstrap.cs:109,176 → services.Configure<PostgresOptions>(config.GetSection("Sonarr:Postgres"));
// Radarr → "Radarr:Postgres" (Bootstrap.cs:108,177,178);  Prowlarr → "Prowlarr:Postgres" (:109,178)
```

.NET maps `SONARR__POSTGRES__HOST` (double underscore) onto `Sonarr:Postgres:Host`.

Three consequences that shape this entire runbook:

1. **No `config.xml` editing.** No bind-mount staleness landmine, no in-place file surgery on a
   live app. The switch is a compose-env change — which also makes it a **committed change**
   (compose-is-truth), reviewable and revertible like any other.
2. **Rollback is a git revert plus a restart.** Remove the env block, restart, and the app is back
   on its untouched SQLite file. Nothing has to be restored, because nothing was moved.
3. **The SQLite files are a frozen snapshot, not a backup strategy.** They stop being written the
   moment the app switches. Anything done after the switch exists only in Postgres — that is the
   rollback cost, and it is the reason the soak period is short and the switch is deliberate.

**Verified Postgres option surfaces (pinned versions):**

| App | Env prefix | MainDb default | LogDb default | Connection-string override |
|---|---|---|---|---|
| Prowlarr 2.5.2.5491 | `PROWLARR__POSTGRES__*` | `prowlarr-main` | `prowlarr-log` | ✅ `MAINDBConnectionString` / `LOGDBConnectionString` |
| Sonarr 4.0.19.2979 | `SONARR__POSTGRES__*` | `sonarr-main` | `sonarr-log` | ❌ not available at this version |
| Radarr 6.3.0.10514 | `RADARR__POSTGRES__*` | `radarr-main` | `radarr-log` | ✅ `MAINDBConnectionString` / `LOGDBConnectionString` |

Newer upstream documentation may describe a `config.xml` (`<PostgresHost>` …) route; `ConfigFileProvider`
does read those keys (`GetValue("PostgresHost", …, persist: false)`). **This runbook deliberately does
not use it** — the env route is declarative, committed, and avoids the bind-mount hazard.

---

## 2. Inventory and current state (verified 2026-09-11)

| App | Main DB | Log DB | Total |
|---|---|---|---|
| Prowlarr | `prowlarr.db` 44 MB | `logs.db` 15 MB | 59 MB |
| Radarr | `radarr.db` 121 MB | `logs.db` 86 MB | 207 MB |
| Sonarr | `sonarr.db` 323 MB | `logs.db` 98 MB | 421 MB |
| **Total** | | | **≈687 MB** |

- Paths: `~/Cave/config/{prowlarr,radarr,sonarr}/<db>` (bind-mounted at `/config`).
- `config.xml` currently contains **no** Postgres keys — confirmed by grep; today's config is
  pure SQLite (ports, `UrlBase`, auth, etc.).
- These sizes are **post-prune** numbers. The stack's landmine log records historical sizes of
  1 GB (Radarr) and 3.2 GiB (Sonarr) dominated by MediaInfo blobs and log history; the prune
  scripts have since recovered the space. **Run the prune gate again immediately before the
  migration** — a smaller database is a shorter window and a smaller failure surface.

---

## 3. Preconditions and go/no-go gates

Every box must be checked, with evidence, before the window opens.

| # | Gate | Command / check |
|---|---|---|
| G1 | Postgres hardening landed and drilled (`rawrz-postgres-hardening.md`) | Scenario A drill recorded |
| G2 | Fresh, `pg_restore --list`-verified backup of everything | `./scripts/backup.sh` |
| G3 | NzbDAV queue empty | `python3 scripts/check_nzbdav_queue.py` |
| G4 | No active/failed *arr imports | `python3 scripts/check_arr_import_queue.py`, `python3 scripts/drain_sonarr_queue.py --dry-run` |
| G5 | Disk headroom on the PG volume | free space ≥ 3× the app's DB size + WAL budget |
| G6 | OS/DB bloat pruned | `python3 scripts/prune_radarr_db.py` / `prune_sonarr_db.py` (maintenance profile) |
| G7 | FUSE mount healthy | stack's mount-health check; **never** `umount` during the window |
| G8 | The migration mechanism was prototyped on a copy (§5) | prototype log attached to the change |
| G9 | Seerr/other consumers expected to degrade are flagged | Seerr request handling will error while Radarr/Sonarr are stopped |
| G10 | Rollback rehearsed on the same app, on a copy | rollback log attached |

**Never during the window:** recreate NzbDAV (its queue is not persistent — the stack's landmine
list is explicit that a recreate silently blocklists in-flight items), touch the FUSE mount, or
run `docker compose down -v`.

---

## 4. Window plan and ordering

| Order | App | Why this position | Window |
|---|---|---|---|
| 1 | **Prowlarr** | Least gameplay/ledger-critical; smallest DB; lowest blast radius if something is wrong. It also validates the whole procedure. | 60 min |
| 2 | **Sonarr** | Mid-size; RV/RAWRZ RPG episode ledgers read Sonarr, so it must come before Radarr only because it is the smaller of the two remaining. | 90 min |
| 3 | **Radarr** | Largest, most write-active, carries the orphaned-quality-profile landmine. Last, with everything learned. | 90 min |

Community/stack baseline: existing operational work already stops Radarr and Sonarr for
prune/vacuum (`prune_radarr_db.py`, `prune_sonarr_db.py`), so stopping these apps is an established,
understood operation — this runbook follows the same shape (stop → operate on files → verify →
resume), which lowers procedural novelty.

---

## 5. Migration mechanism (must be prototyped first)

### 5.1 Why two phases

The app owns its schema. Pointed at an empty Postgres database, \*arr runs its own migrations
(`MigrationController` with `.AddPostgres()`) and creates the correct schema for the exact running
version — including identity/sequence behavior. A generic SQLite→Postgres copy tool, by contrast,
would invent a schema from the SQLite file that will not match what the app expects.

So: **let the app build the schema, then copy data into it.**

### 5.2 Phase A — schema creation (per app)

```bash
# 1. Create the role + databases (once, in the Postgres init script)
#    prowlarr / prowlarr-main / prowlarr-log   (then sonarr, radarr)

# 2. Start the app against empty databases by adding ONLY the Postgres env block temporarily
docker compose up -d prowlarr
docker compose logs -f prowlarr      # watch migrations run; wait for "Migrations completed"
curl -s -H "X-Api-Key: $PROWLARR_API_KEY" localhost:9696/api/v1/system/status | jq .migrationVersion

# 3. Stop it and freeze the schema
docker compose stop prowlarr
```

Record the `migrationVersion` — it becomes the reference for verification.

### 5.3 Prepare the target for data

```sql
-- Keep migration bookkeeping; empty everything else.
-- Identify app-managed state tables (VersionInfo and any seed/migration tables) from the
-- prototype and EXCLUDE them from truncation and from the copy.
TRUNCATE TABLE <all other public tables> RESTART IDENTITY CASCADE;
```

Why truncate rather than merge: on first start the app seeds rows (default quality profiles,
definitions, version rows). Those collide with the copied user rows on primary keys. Truncating
gives the copy a clean, empty target while `VersionInfo` preserves the "schema is current" state.

### 5.4 Phase B — data-only copy

Use **pgloader** in a container, one invocation **per table group, in dependency order**
(parents and small tables first, large tables last). Explicit ordering removes foreign-key
questions entirely and gives per-group progress and failure isolation.

```bash
# Ordered groups per app, e.g. for Radarr:
#   1. Tags, QualityProfiles, CustomFormats, RootFolders, ImportLists, DelayProfiles, NamingConfig
#   2. Collections, Movies
#   3. MovieFiles, History, Blocklist, ...
# Last: the biggest tables (MovieFiles/History), so a failure there is cheap to retry.
docker run --rm --network bearcave \
  -v "$PWD/config/radarr:/sqlite:ro" \
  ghcr.io/dimitri/pgloader:latest \
  pgloader \
    --with "data only, workers = 4, concurrency = 2, prefetch rows = 5000, batch rows = 10000" \
    --with "batch concurrency = 4" \
    --set "work_mem to '64MB', maintenance_work_mem to '512MB'" \
    /sqlite/radarr.db "postgresql://radarr:${RADARR_DB_PASSWORD}@rawrz-postgres:5432/radarr-main"
```

Notes that the prototype must confirm (do not assume):

- `data only` suppresses DDL — **required**, since the schema already exists.
- Whether FK-trigger deferral is needed. If grouped-by-dependency loading works (expected), it is
  not. If a group fails on FK order, the fallback is `session_replication_role = replica` for the
  load **plus** an explicit post-load `ALTER TABLE … VALIDATE CONSTRAINT` pass for every FK. That
  fallback is acceptable only because validation then proves the data is consistent.
- Whether any table needs `EXCLUDING TABLE NAMES MATCHING` beyond the migration bookkeeping table.
- Throughput per app (drives the window estimate in §4).

### 5.5 Sequence repair (mandatory, easy to forget)

After the copy, every identity/serial column must be advanced past the copied maximum, or the next
insert collides.

```sql
DO $$
DECLARE r record;
BEGIN
  FOR r IN
    SELECT c.table_name, c.column_name
    FROM information_schema.columns c
    JOIN information_schema.tables t
      ON t.table_schema = c.table_schema AND t.table_name = c.table_name
    WHERE c.table_schema = 'public'
      AND c.column_default LIKE 'nextval(%'
  LOOP
    EXECUTE format(
      'SELECT setval(pg_get_serial_sequence(%L, %L), COALESCE((SELECT MAX(%I) FROM %I), 1), true)',
      'public.' || r.table_name, r.column_name, r.column_name, r.table_name);
  END LOOP;
END $$;
```

Verification for the repair itself: for every sequence, `last_value >= max(column)`.

### 5.6 Prototype requirement (blocking)

Before any live window, run §5.2–§5.5 for **one app** (Prowlarr) against:

- a **copy** of the SQLite file (never the live file), and
- a **scratch Postgres** database (never the live instance).

Then run the full §7 verification and the §8 rollback against that scratch pair. Record
elapsed time per step. If the copy does not verify cleanly at prototype scale, the mechanism is
not ready — that is the purpose of the prototype.

---

## 6. Live procedure (per app)

### T-24 h

1. Run gate checks G1–G7; capture output.
2. Announce the window.
3. Confirm Seerr users know requests will fail during the window.
4. Freeze unrelated changes to the stack (no compose edits, no image bumps).

### T-0

```bash
# 1. Stop the app (and Unpackerr: it polls *arr queues and would only log errors)
docker compose stop unpackerr
docker compose stop radarr                 # app being migrated

# 2. Freeze and verify the SQLite file
sqlite3 config/radarr/radarr.db "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA integrity_check;"
sqlite3 config/radarr/logs.db "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA integrity_check;"

# 3. Snapshot the SQLite files (rollback source) — copy, do not move
ts=$(date +%Y%m%d_%H%M%S)
mkdir -p "backups/arr-sqlite-$ts"
cp -a config/radarr/radarr.db config/radarr/logs.db "backups/arr-sqlite-$ts/"
sha256sum "backups/arr-sqlite-$ts/"*.db | tee "backups/arr-sqlite-$ts/SHA256SUMS"

# 4. Record the SQLite row counts that verification will compare against
for t in Movies MovieFiles History QualityProfiles CustomFormats Tags Collections; do
  printf '%-18s %s\n' "$t" "$(sqlite3 config/radarr/radarr.db "select count(*) from $t;")"
done | tee "backups/arr-sqlite-$ts/rowcounts.txt"

# 5. Create the databases + role (idempotent)
psql -h rawrz-postgres -U "$POSTGRES_SUPERUSER" -f config/postgres-init/10-arr-apps.sql

# 6. Phase A: add the Postgres env block, start, let migrations run, stop
#    (in the prototype this is a compose override file; live it is a real commit)
docker compose up -d radarr && docker compose logs -f radarr   # wait for migration completion
curl -s -H "X-Api-Key: $RADARR_API_KEY" localhost:7878/api/v3/system/status | jq .migrationVersion
docker compose stop radarr

# 7. Truncate populated tables, preserving migration bookkeeping (§5.3)
psql -h rawrz-postgres -U "$POSTGRES_SUPERUSER" -d radarr-main -f scripts/arr-truncate-except-versioninfo.sql

# 8. Phase B: pgloader data-only copy, ordered groups (§5.4), then sequence repair (§5.5)
# 9. Start the app on Postgres
docker compose up -d radarr
curl -s -H "X-Api-Key: $RADARR_API_KEY" localhost:7878/api/v3/system/status | jq '.databaseType?, .migrationVersion'
```

### T+0 → T+24 h (soak)

1. Run the §7 verification checklist immediately and again at T+24 h.
2. Confirm one real import/scan path works end to end (request → download → import → Plex scan).
3. Watch `pg_stat_activity`, slow-query log, and app health endpoints.
4. Keep the SQLite snapshot for **30 days**, then archive it (do not delete).

### Compose changes at T-0 (the actual switch)

```yaml
  radarr:
    environment:
      RADARR__POSTGRES__HOST: rawrz-postgres
      RADARR__POSTGRES__PORT: "5432"
      RADARR__POSTGRES__USER: radarr
      RADARR__POSTGRES__PASSWORD: ${RADARR_DB_PASSWORD}
      RADARR__POSTGRES__MAINDB: radarr-main
      RADARR__POSTGRES__LOGDB: radarr-log
      # pin the pool — verified supported on Radarr/Prowlarr only (not Sonarr at this version)
      RADARR__POSTGRES__MAINDBConnectionString: "Host=rawrz-postgres;Database=radarr-main;Username=radarr;Password=${RADARR_DB_PASSWORD};Maximum Pool Size=10;Minimum Pool Size=1;Timeout=15;Command Timeout=60"
      RADARR__POSTGRES__LOGDBConnectionString:  "Host=rawrz-postgres;Database=radarr-log;Username=radarr;Password=${RADARR_DB_PASSWORD};Maximum Pool Size=5;Minimum Pool Size=1;Timeout=15;Command Timeout=60"
```

These land as one commit per app, so each migration has a revertible one-commit switch.

---

## 7. Verification checklist (per app)

### 7.1 Row-count parity (SQLite snapshot vs Postgres)

Compare the T-0 baseline against Postgres for the app's key tables. Any unexplained difference is a
failed migration, not a rounding error.

| App | Tables to compare |
|---|---|
| Prowlarr | `Indexers`, `Applications`, `IndexerProxies`, `History`, `Tags`, `Definitions` |
| Sonarr | `Series`, `Episodes`, `EpisodeFiles`, `History`, `QualityProfiles`, `CustomFormats`, `Tags`, `Blocklist` |
| Radarr | `Movies`, `MovieFiles`, `History`, `QualityProfiles`, `CustomFormats`, `Tags`, `Collections`, `Blocklist` |

```bash
# SQLite
sqlite3 config/radarr/radarr.db "select count(*) from Movies;"
# Postgres
psql -h rawrz-postgres -U rawrz_superuser -d radarr-main -c "select count(*) from \"Movies\";"
```

### 7.2 Sequence integrity

- [ ] Every identity/serial column's sequence is ≥ `max(value)` (§5.5).
- [ ] A test insert on a copy succeeds (or a real insert during soak produces a non-colliding id).

### 7.3 App-level checks

| Check | Command |
|---|---|
| System status + DB type | `curl -s -H "X-Api-Key: $KEY" localhost:<port>/api/v3/system/status \| jq .` (expect Postgres type, expected `migrationVersion`) |
| Health | `curl -s -H "X-Api-Key: $KEY" localhost:<port>/api/v3/health \| jq .` (no new errors/warnings) |
| Collection endpoints | `/api/v3/movie` or `/api/v3/series` or `/api/v1/indexer` — count matches §7.1 |
| Profiles resolve (landmine #8) | `python3 scripts/check_radarr_profiles.py` |
| References intact | `python3 scripts/check_sonarr_refs.py`, `python3 scripts/check_prowlarr_refs.py` |
| DB growth gate repointed at PG | `python3 scripts/check_radarr_db_size.py --postgres` (added by this work) |
| Prowlarr app sync | trigger "Test All" / "Sync App Indexers" and confirm Radarr/Sonarr receive indexers |
| Library refresh | Radarr `RescanMovie`/`RefreshMovie` command; Sonarr `RescanSeries` — completion without errors |

### 7.4 End-to-end sanity

- [ ] One new request flows Seerr → Radarr/Sonarr → NzbDAV → import → Plex (or an equivalent
      dry-run on the stack's existing pipeline test).
- [ ] Unpackerr resumes cleanly (`docker compose start unpackerr`) with no extraction errors.
- [ ] The RAWRZ RPG's next poll/tick awards expected watches and errors on nothing new.

---

## 8. Rollback (target: under 10 minutes, per app)

Because the switch is env-driven (§1), rollback is deliberately dull:

```bash
# 1. Revert the compose commit for this app (removes the RADARR__POSTGRES__* block)
git revert --no-edit <switch-commit>       # or restore the pre-window compose state

# 2. Stop the app
docker compose stop radarr

# 3. Confirm the SQLite files are the T-0 snapshot (they were never modified after the switch)
sha256sum -c backups/arr-sqlite-<ts>/SHA256SUMS   # from the snapshot directory

# 4. If the live SQLite files were somehow touched, restore from the snapshot:
cp -a backups/arr-sqlite-<ts>/radarr.db backups/arr-sqlite-<ts>/logs.db config/radarr/
# include -wal/-shm if present in the snapshot

# 5. Start the app and verify it is on SQLite
docker compose up -d radarr
curl -s -H "X-Api-Key: $RADARR_API_KEY" localhost:7878/api/v3/system/status | jq '.databaseType?, .migrationVersion'
```

Rollback cost, stated honestly: **every change made after the switch is lost** (they live only in
Postgres). That is why the soak is short and the window is planned — rollback is technically trivial
but semantically a data-loss event for the window's duration.

Cleanup after a successful soak (not part of rollback): drop the Postgres databases for that app
only when the SQLite snapshot is archived, never before.

---

## 9. Failure modes and landmines

| # | Hazard (from the stack's own landmine list) | Handling here |
|---|---|---|
| L1 | **Orphaned quality-profile references make `/api/v3/movie` 500 for the whole collection** | The highest-value verification in §7.3: run `check_radarr_profiles.py` immediately after the switch, before declaring success. If it fails, roll back. |
| L2 | **SQLite/MediaInfo blob bloat** (historically 1 GB/3.2 GiB) | Prune before the window (G6) — smaller copy, shorter window, smaller failure surface. Post-migration, the gate now measures Postgres. |
| L3 | **NzbDAV queue is not persistent** | Never recreate NzbDAV during a window; verify the queue is empty (G3) and never touch it. |
| L4 | **FUSE mount fragility** | Never `umount`; the window touches only app containers and the database. |
| L5 | **Bind-mount file staleness** | Sidestepped by the env-var switch (§1). Any residual `config.xml` edit still requires a container restart. |
| L6 | **Container recreate wipes in-flight state** | `docker compose stop`/`start` (not `down`, not `--force-recreate`) except where a compose env change requires a recreate — which is the deliberate switch commit. |
| L7 | **Sonarr cannot pin its Npgsql pool at this version** | Covered by `max_connections=200` headroom plus the `ConnectionSaturation` alert (`rawrz-postgres-hardening.md` §7). |
| L8 | **Long pause = stale clients** | Seerr and Prowlarr/Sonarr cross-calls fail loudly during the window; restart/retry after. Note that the RPG's poll degrades non-fatally by design, so it will simply record fewer events. |
| L9 | **Sequence drift** | §5.5 repair is mandatory and verified; skipping it produces "duplicate key" errors hours later. |
| L10 | **Copy tooling invents schema** | Prevented by the two-phase design (app creates schema; `data only` copy) and enforced by the prototype gate (G8). |

---

## 10. Post-migration tasks

1. Update `rawrz-megastack-spec.md` §8.1 to the app-default database names
   (`radarr-main`, `radarr-log`, `sonarr-main`, `sonarr-log`, `prowlarr-main`, `prowlarr-log`).
2. Add `*_DB_PASSWORD` entries to `.env.template` and the Postgres init script to `config/`.
3. Repoint every DB-touching gate/script at Postgres: `check_radarr_db_size.py`, `prune_radarr_db.py`,
   `prune_sonarr_db.py`, `db_growth_trend.py`, `check_radarr_profiles.py`, `check_sonarr_refs.py`,
   `check_prowlarr_refs.py` (plus their tests, which the spec's "test AND eval in the same commit"
   rule requires).
4. Update `docs/stack/AGENTS.md` landmines: the blob-bloat entry now describes Postgres, and the
   new landmine is "never `down -v`; Postgres is the only copy".
5. Archive (do not delete) the SQLite snapshots after the 30-day soak; record their location.
6. Confirm the nightly Postgres dump covers the new databases and the `BackupStale` alert is armed.
7. Record the migration in `docs/stack/arr-postgres-migration.md` using the template below.

---

## 11. Record template (fill in per app)

```markdown
### <App> — migrated <YYYY-MM-DD>

| Item | Value |
|---|---|
| Source DB size (main/log) | |
| Migration version at switch | |
| Window start / end | |
| Downtime (app unavailable) | |
| pgloader duration | |
| Sequence repair duration | |
| Verification duration | |
| Row-count parity | (attach rowcounts.txt diff) |
| Health check result | |
| Profile/ref gates | |
| End-to-end import sanity | |
| Rollback needed? | no / yes (time to recover) |
| Snapshot location + retention | |
| Surprises / follow-ups | |
```

---

## 12. Acceptance criteria

- [ ] Prototype (§5.6) completed on copies with all verification steps passing **before** any live window.
- [ ] All ten preconditions (G1–G10) evidenced for each app.
- [ ] Row-count parity for every table in §7.1, with a written explanation for any difference.
- [ ] No sequence below its column maximum (§7.2).
- [ ] `check_radarr_profiles.py` passes on Radarr; ref gates pass on Sonarr and Prowlarr.
- [ ] `databaseType` reports Postgres in each app's `/system/status`.
- [ ] One real end-to-end import verified after each app's switch.
- [ ] Rollback rehearsed on a copy and documented; the live SQLite snapshot's checksums verified.
- [ ] Each switch is exactly one revertible commit.
- [ ] Postgres nightly dumps cover all new databases and are `pg_restore --list`-verified.
- [ ] SQLite snapshots retained and archived (not deleted) after the soak.
