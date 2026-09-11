# RAWRZ — Postgres hardening, alerting, and restore rehearsal

> **Status:** Draft v0.1 — 2026-09-11. Companion to `rawrz-megastack-spec.md` §8 and §16.
> **Decision it implements:** D43 / R-15 — **single hardened instance + verified backups**;
> no PITR, no replica, no automatic failover.
> **Why this document exists:** R-1 dropped the "media pipeline survives app outages" rule.
> Postgres now sits under Radarr, Sonarr, Prowlarr, RPG, and Deck, so its reliability *is*
> the media pipeline's reliability. This plan makes that single instance as trustworthy as a
> single instance can be, and proves the restore works before it is ever needed.

---

## 1. Scope and accepted risk

**In scope:** service definition, roles/databases, connection sizing, tuning, storage, backups,
alert rules, the rehearsed restore, upgrades, and acceptance criteria.

**Explicitly out of scope (accepted):** streaming replication, automatic failover, PITR/WAL
archiving, connection pooling middleware (PgBouncer). These were offered and declined (D43);
revisiting any of them is a new decision, not a drift.

**Accepted consequences, stated plainly:**

- A Postgres outage is a media outage (Radarr, Sonarr, Prowlarr stop working; Plex keeps
  serving already-mounted media because it has no Postgres dependency).
- **RPO ≈ 24 h** (last successful nightly dump) and **RTO ≈ 30 min** (documented restore) are
  the operating targets. Anything tighter requires the PITR decision that was declined.
- A *silent* backup failure is the worst realistic outcome. That is why backup verification and
  the `BackupStale` alert are treated as first-class, not nice-to-haves.

---

## 2. Service definition

```yaml
  postgres:
    image: postgres:17-alpine
    container_name: rawrz-postgres
    restart: always                     # NOT unless-stopped: an OOM-kill must self-heal
    stop_grace_period: 60s             # let a checkpoint finish; avoid a dirty shutdown
    shm_size: 256mb                    # parallel workers / big sorts
    mem_limit: 1536m
    cpus: "1.5"
    networks: [bearcave]
    # no published host port: reachable as rawrz-postgres:5432 on bearcave only
    environment:
      POSTGRES_USER: ${POSTGRES_SUPERUSER}
      POSTGRES_PASSWORD_FILE: /run/secrets/postgres_superuser_password
      POSTGRES_DB: rawrz_rpg
      POSTGRES_INITDB_ARGS: "--data-checksums --auth-host=scram-sha-256"
      TZ: ${TZ}
    secrets: [postgres_superuser_password]
    volumes:
      - ./config/postgres:/var/lib/postgresql/data
      # init/ runs only on first initialisation (roles + databases)
      - ./config/postgres-init:/docker-entrypoint-initdb.d:ro
    command:
      - postgres
      - -c max_connections=200
      - -c shared_buffers=384MB
      - -c effective_cache_size=1GB
      - -c work_mem=16MB
      - -c maintenance_work_mem=128MB
      - -c wal_compression=on
      - -c checkpoint_completion_target=0.9
      - -c max_wal_size=2GB
      - -c min_wal_size=256MB
      - -c random_page_cost=1.1            # NVMe; default 4.0 assumes spinning rust
      - -c effective_io_concurrency=200    # NVMe
      - -c shared_preload_libraries=pg_stat_statements
      - -c pg_stat_statements.max=5000
      - -c pg_stat_statements.track=all
      - -c log_min_duration_statement=1000
      - -c log_line_prefix='%m [%p] %q%u@%d '
      - -c log_connections=off
      - -c log_disconnections=off
      - -c autovacuum_max_workers=3
      - -c autovacuum_naptime=30s
      - -c idle_in_transaction_session_timeout=300s
      - -c statement_timeout=0            # keep 0: *arr long scans/refreshes are legitimate
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_SUPERUSER} -d rawrz_rpg"]
      interval: 15s
      timeout: 5s
      retries: 5
      start_period: 30s
    logging:
      driver: json-file
      options: { max-size: "10m", max-file: "5" }
```

Rationale for the non-obvious choices:

| Flag | Why |
|---|---|
| `restart: always` | `unless-stopped` leaves a crashed container down after an OOM-kill until a human acts. Postgres is the one service that must always come back. |
| `--data-checksums` | Detects silent storage corruption. Cheap to enable at `initdb`, expensive to add later (a full rewrite). |
| `stop_grace_period: 60s` | The stack already learned this lesson with Plex (Docker's 10 s default SIGKILL produced an unkillable D-state hang). A clean Postgres shutdown matters more, not less. |
| `random_page_cost=1.1`, `effective_io_concurrency=200` | The host is NVMe-backed; the defaults are tuned for spinning disks and make the planner avoid indexes it should use. |
| `max_connections=200` + `idle_in_transaction_session_timeout=300s` | See §4 — the consumers' pools are lazy but numerous, and a stuck idle transaction blocks autovacuum. |
| `statement_timeout=0` | A global timeout would kill legitimate long *arr refresh/scan queries. Slow-query visibility comes from `log_min_duration_statement` instead. |
| No host port | Consistent with Redis; only RAWRZ services connect. |
| Secrets file, not env | The superuser password never appears in `docker inspect` or in `.env`. |

**Non-negotiable rule:** no `docker compose down -v` in this stack, ever. The `./config/postgres`
volume is the database.

---

## 3. Roles and databases

Initialised once by `config/postgres-init/01-roles-and-databases.sh` (idempotent, `DO $$` guards):

| Role | Databases owned | Grants |
|---|---|---|
| `rawrz_rpg` | `rawrz_rpg` | owner |
| `rawrz_deck` | `rawrz_deck` | owner |
| `radarr` | `radarr-main`, `radarr-log` | owner |
| `sonarr` | `sonarr-main`, `sonarr-log` | owner |
| `prowlarr` | `prowlarr-main`, `prowlarr-log` | owner |

- Each app connects with its own role, own password (Docker secrets), and cannot read another
  app's database. Least privilege is cheap here and blunts a compromised app.
- **Database names use the \*arr applications' own defaults** (`radarr-main`, `radarr-log`,
  `sonarr-main`, `sonarr-log`, `prowlarr-main`, `prowlarr-log`), verified in the pinned source
  (`ConfigFileProvider`: `PostgresMainDb` default `"radarr-main"`, `"sonarr-main"`,
  `"prowlarr-main"`; log counterparts likewise). The master spec's §8.1 table listed
  `radarr_main`-style underscore names — **corrected to the app defaults**, which means one fewer
  config key to set per app and no drift from upstream expectations.
- `rawrz_rpg` / `rawrz_deck` keep underscore names (they are RAWRZ-owned).
- `template1` stays untouched; no extensions beyond `pg_stat_statements` (which lives in
  `postgres`), so permissions stay boring.

---

## 4. Connection sizing (verified arithmetic)

Consumers, with the pool sizes verified from source:

| Consumer | Pool | Evidence |
|---|---|---|
| RAWRZ RPG (`deadpool-postgres`) | **max 8** | `backend/rpg/src/persistence.rs:679-681` — `Pool::builder(manager).max_size(8)` |
| RAWRZ Deck (`sqlx`) | TBD, budget 10 | to be set when the Deck port lands (M5) |
| Radarr | Npgsql, main + log | pinned source: `NpgsqlConnectionStringBuilder` with no `MaxPoolSize` set → Npgsql default **Max Pool Size 100**, lazy per connection string |
| Sonarr | Npgsql, main + log | same; **Sonarr v4.0.19.2979 exposes only Host/Port/User/Password/MainDb/LogDb** — no connection-string override, so its pool cannot be tuned via config |
| Prowlarr | Npgsql, main + log | same as Radarr; supports `PostgresMainDbConnectionString`/`PostgresLogDbConnectionString` |

Two consequences drive the config:

1. **Npgsql's pool is lazy and per–connection-string.** Radarr/Prowlarr each build a connection
   string for the main DB and another for the log DB, so each app can hold up to 100 + 100 idle
   connections in the worst case, times three apps. Reality is far lower, but the ceiling is what
   `max_connections` must survive.
2. **Sonarr cannot be tuned**, so `max_connections` must be generous rather than relying on
   per-app pool caps.

Therefore:

- `max_connections = 200`.
- **Pin Radarr and Prowlarr** via their connection-string keys so they cannot balloon:
  `Maximum Pool Size=10;Minimum Pool Size=1;Timeout=15;Command Timeout=60` appended to the
  `PostgresMainDbConnectionString` / `PostgresLogDbConnectionString` values.
- **Sonarr stays on the host/port/user/password/main/log keys** (its pinned version has no
  alternative) and is covered by headroom and by the `ConnectionSaturation` alert.
- Monitor `pg_stat_activity` by `datname` from day one; the first week's baseline sets the real
  numbers, and this table is updated with them.

Escalation path if saturation is ever hit: reduce Radarr/Prowlarr pool caps → raise
`max_connections` (each costs ~10 MB) → only then consider PgBouncer (a new decision).

---

## 5. Storage, growth, and the disk-contention risk

- Data directory: `./config/postgres` on the host NVMe.
- **Contention hazard:** the same host volume also backs the rclone VFS cache
  (`--vfs-cache-max-size=300G`, `--cache-dir=/cache`) and the Docker image/volume store. A
  Postgres checkpoint storm during a large rclone read/write can both slow streaming and inflate
  checkpoint latency.
  - Mitigations: `min-free-space` style free-space alerting (§7 `DiskSpaceLow`), keep
    `max_wal_size` bounded (2 GB) so WAL growth can't fill the disk, place `pg_wal` on the same
    volume only if free space is monitored, and prefer separate volumes if the host layout allows.
  - **Do not** move the rclone cache to fix this without re-reading the stack's own comments: the
    300 G cap was deliberately raised from 50 G, and lowering it caused LRU churn.
- Growth baseline for the migration runbook: SQLite files as of 2026-09-11 were
  `radarr.db` 121 MB + `logs.db` 86 MB, `sonarr.db` 323 MB + `logs.db` 98 MB,
  `prowlarr.db` 44 MB + `logs.db` 15 MB (≈687 MB total). Postgres will run somewhat larger than
  the resident SQLite size (page overhead, indexes, bloat).
- Bloat control: the stack's existing gates (`check_radarr_db_size.py`,
  `prune_radarr_db.py`, `prune_sonarr_db.py`) are repointed at Postgres in the migration runbook;
  `log_min_duration_statement` plus `pg_stat_statements` surface the queries that create bloat.

---

## 6. Backups

**Policy:** nightly logical dumps, verified, retained, and rehearsed.

| Item | Value |
|---|---|
| Method | `pg_dump -Fc` (custom format, parallel-restorable, per-database) |
| Schedule | 04:30 local daily (before Recyclarr's manual window; after the rclone cache's heaviest nightly activity) |
| Scope | all 7 databases + `pg_dumpall --globals-only` (roles) |
| Location | `backups/postgres/<timestamp>/<db>.dump` + `globals.sql` |
| Retention | 14 daily, 8 weekly, 3 monthly |
| **Verification** | every dump is checked with `pg_restore --list` immediately; a non-readable dump fails the job |
| Rehearsal | monthly automated restore of one rotating database into a scratch DB + row-count comparison (§8); quarterly full-instance rehearsal |
| Offsite | the existing Dropbox snapshot flow (`backup_dropbox.py`) is reviewed: it deliberately excludes media/metadata, so Postgres dumps need an explicit decision (dumps contain the full metadata ledger — decide, don't assume) |
| Integration | `scripts/backup.sh` gains a `--postgres` mode; the preflight script asserts the backup timer/unit is installed |

Sketch of the dump step:

```bash
pg_dump -Fc -h "$PGHOST" -U "$PGUSER" -d "$db" -f "$dir/$db.dump"
pg_restore --list "$dir/$db.dump" >/dev/null   # verification: fail the job on error
pg_dumpall --globals-only -h "$PGHOST" -U "$PGUSER" -f "$dir/globals.sql"
```

---

## 7. Alerts

Delivery: **Discord webhook** (`DISCORD_WEBHOOK_URL` already exists in `.env.template`, optional,
and existing workflows already skip alerts when it is unset) as the primary channel; optional
SMTP (RAWRZ-own SMTP creds) for `critical` only. Every alert carries a runbook link to §8.

| Alert | Expression (Prometheus) | Severity | For |
|---|---|---|---|
| `PostgresDown` | `pg_up == 0` (or `up{job="postgres"} == 0`) | critical | 1m |
| `PostgresRestartLoop` | `changes(pg_up[15m]) > 2` | critical | 5m |
| `ConnectionSaturation` | `pg_stat_activity_count / pg_settings_max_connections > 0.85` | warning | 10m |
| `ConnectionSaturationCritical` | same ratio `> 0.95` | critical | 2m |
| `IdleInTransaction` | `max(pg_stat_activity_max_tx_duration{state="idle in transaction"}) > 300` | warning | 5m |
| `AutovacuumBlocked` | `max(pg_stat_activity_max_tx_duration{state=~"idle in transaction\|active"}) > 3600` | warning | 15m |
| `DiskSpaceLow` (PG volume) | `node_filesystem_avail_bytes / node_filesystem_size_bytes < 0.15` | warning | 10m |
| `DiskSpaceCritical` | `< 0.08` | critical | 2m |
| `PostgresWalGrowth` | `rate(pg_stat_wal_bytes_total[30m])` above baseline for 1h | warning | 1h |
| `BackupFailed` | `increase(rawrz_pg_backup_failures_total[26h]) > 0` | critical | 0m |
| `BackupStale` | `time() - rawrz_pg_backup_last_success_timestamp > 93600` (26h) | critical | 0m |
| `RestoreDrillOverdue` | `time() - rawrz_pg_restore_drill_last_success_timestamp > 2678400` (31d) | warning | 0m |
| `TempFileSpill` | `rate(pg_stat_database_temp_bytes_total[15m]) > 50MB/s` | warning | 10m |
| `RadarrBlobBloat` | existing `check_radarr_db_size.py` gate exported as a gauge, threshold per the gate | warning | 1h |
| `ArrApiErrors` | `rate(rawrz_arr_http_errors_total[15m]) > 0.1` | warning | 10m |

Design rules for this alert set:

- **`BackupStale` and `RestoreDrillOverdue` are the load-bearing alerts.** With no replica and no
  PITR, backups are the entire recovery story; a silent backup gap must page.
- Thresholds live in the RAWRZ settings/flags registry, not hard-coded in the rules file.
- Alerts must be **actionable and few**. Anything that fires routinely gets fixed or deleted; the
  list above is the whole set on purpose.
- Each alert's runbook entry names the exact §8 command sequence.

---

## 8. Rehearsed restore procedure

**Targets:** RTO ≈ 30 min, RPO ≤ 26 h. Success is *demonstrated*, not assumed.

### 8.1 Scenario A — single database restore (routine, monthly drill)

```bash
# 1. Announce + quiesce: stop writers of just that database
docker compose stop rawrz-deck                    # or the owning app/RPG

# 2. Pick the newest verified dump
ls -1t backups/postgres/*/rawrz_deck.dump | head -1

# 3. Restore into a SCRATCH database first (never over the live one)
psql -U rawrz_superuser -c 'DROP DATABASE IF EXISTS rawrz_deck_restore;'
psql -U rawrz_superuser -c 'CREATE DATABASE rawrz_deck_restore OWNER rawrz_deck;'
pg_restore -j4 --no-owner -d rawrz_deck_restore backups/postgres/<ts>/rawrz_deck.dump

# 4. Verify counts against the live database (per-table row-count diff)
psql -d rawrz_deck_restore -c 'SELECT count(*) FROM flags;'   # vs live

# 5. Promote only if the diff is expected: rename swap
psql -U rawrz_superuser <<'SQL'
ALTER DATABASE rawrz_deck RENAME TO rawrz_deck_old;
ALTER DATABASE rawrz_deck_restore RENAME TO rawrz_deck;
SQL

# 6. Start writers, verify app health, then drop rawrz_deck_old after 24h
docker compose start rawrz-deck
```

### 8.2 Scenario B — full instance restore (quarterly drill)

1. Stop every Postgres consumer: `rawrz-rpg`, `radarr`, `sonarr`, `prowlarr`, `rawrz-deck`.
   Plex keeps running (no Postgres dependency) — expected and worth confirming during the drill.
2. `docker compose stop rawrz-postgres`.
3. Preserve the failed data directory (`mv config/postgres config/postgres.failed-<ts>`) — never
   delete it before the restore is proven.
4. Bring up a fresh Postgres on an empty `./config/postgres`.
5. `psql -f globals.sql` for roles, then restore each database with `pg_restore -j4 --no-owner`.
6. Verify (§8.3), then start consumers in the reverse of step 1.
7. Record the drill in `docs/stack/postgres-drills.md` with timings and any surprises.

### 8.3 Verification checklist (applied after any restore)

- [ ] `pg_restore --list` succeeded when the dump was written (job-level gate).
- [ ] Role list matches `globals.sql`.
- [ ] Per-database row counts match the pre-restore baseline for the key tables:
      Radarr `Movies`, `MovieFiles`, `History`, `QualityProfiles`, `CustomFormats`, `Tags`;
      Sonarr `Series`, `EpisodeFiles`, `Episodes`, `History`, `QualityProfiles`, `CustomFormats`;
      Prowlarr `Indexers`, `Applications`, `History`;
      RPG `watches`, `characters`, `character_state`, `achievements`, `watch_orders`;
      Deck `flags`, plus whichever tables the Deck port adds.
- [ ] `SELECT last_value FROM <seq>` is ≥ `max(id)` for every identity column (apply `setval()`
      where it is not — the same step the migration runbook uses).
- [ ] App-level: `GET /api/v3/system/status` reports the expected `migrationVersion` and database
      type; `check_radarr_profiles.py`, `check_sonarr_refs.py`, `check_prowlarr_refs.py`,
      `check_radarr_db_size.py` all pass.
- [ ] The RPG's tick runs once and awards nothing unexpected (ledger idempotency after restore).
- [ ] A fresh import/scan path is exercised (or dry-run) before declaring success.

### 8.4 Degradation vs. restore decision tree

| Symptom | Action |
|---|---|
| Postgres container down, `restart: always` recovering | wait one healthcycle, alert resolves itself; verify no data loss |
| Postgres up, one database corrupt | Scenario A (single-DB restore) |
| Instance will not start / data directory damaged | Scenario B (full restore) |
| Data loss suspected but instance healthy | stop writers, snapshot, restore *in parallel* to scratch, compare, then decide |
| Backup unreadable | fix the backup job first; if no good backup exists, the RPO has already been missed — escalate rather than improvise |

---

## 9. Upgrades and maintenance

- **Minor upgrades** (same PG major): acceptable in a maintenance window with a fresh dump first.
- **Major upgrades**: `pg_upgrade` inside a maintenance window, with a full logical dump taken
  beforehand as the real rollback. Never in-place across majors without the dump.
- **Extensions**: adding any (e.g. `pg_cron` for in-DB backups) is a new decision requiring the
  `shared_preload_libraries` change and a restart.
- **Config changes**: `postgres -c` flags take effect on restart; a reload (`SIGHUP`) only covers
  reloadable GUCs. Document per-change which it needs — the stack's bind-mount staleness landmine
  applies (edit the file, then restart the container, or the old config keeps running).
- **No `VACUUM FULL`** without a window: it takes an exclusive lock and rewrites the table.

---

## 10. Acceptance criteria

- [ ] `docker compose config --quiet` passes with the Postgres service added.
- [ ] Postgres starts from an empty volume, initialises roles/databases, and passes its healthcheck.
- [ ] `max_connections`, buffer sizes, and `pg_stat_statements` are in effect
      (`SHOW` / `pg_settings`), and the slow-query log works.
- [ ] Each app role can reach only its own databases (verified by attempting a cross-database
      connection and expecting failure).
- [ ] A nightly dump runs, is `pg_restore --list`-verified, and lands with the documented retention.
- [ ] `PostgresDown`, `BackupStale`, and `ConnectionSaturation` fire correctly in a fault-injection
      test (stop the container; remove a dump; open a synthetic connection storm).
- [ ] A full Scenario B restore completes within the RTO target on the real data volumes.
- [ ] The drill is recorded in `docs/stack/postgres-drills.md` with timings and the row-count diff.
- [ ] Radarr/Prowlarr pool caps are pinned via connection-string keys; Sonarr's limits are
      documented and monitored.
- [ ] `./config/postgres` is excluded from git and covered by the backup/restore story.

---

## 11. Open items

1. `sqlx` pool size for `backend/deck` (set at M5, then update §4).
2. Dropbox offsite policy for Postgres dumps (they contain the full metadata ledger).
3. Volume layout decision: co-locate `pg_wal` with the data directory or give Postgres its own
   volume, given the rclone cache's 300 G ceiling.
4. Postgres major version confirmation (17 chosen here; 18 would default to data checksums).
5. Whether `pg_cron` is wanted for in-database maintenance jobs (requires the extension decision).
6. Baseline tuning pass after the first week of real `pg_stat_statements` data.
