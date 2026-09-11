# Backup & Restore

The most valuable state is the Plex database, followed by application databases and
credentials. Media is remote-backed and can be re-indexed, but the NzbDAV queue is
not persistent across recreation.

## What to back up

| Data | Location | Priority |
|------|----------|----------|
| Plex library database and metadata | `config/plex/` | Critical |
| Radarr, Sonarr, Prowlarr state | `config/{radarr,sonarr,prowlarr}/` | High |
| NzbDAV database and settings | `config/nzbdav/` | High |
| rclone configuration and cache metadata | `config/nzbdav-rclone/` | High; protect credentials |
| Seerr state | `config/seerr/` | Medium |
| Secrets and environment | `secrets/`, `.env` | Critical |
| Compose and scripts | `docker-compose.yml`, `scripts/` | Useful for recovery |

## Create a backup

```bash
./scripts/backup.sh                 # configs, databases, Plex metadata, secrets
./scripts/backup.sh --configs-only  # config tree, compose, and .env copy
./scripts/backup.sh --secrets-only  # secrets and .env only
```

Backups are written under `backups/bearcave_backup_<timestamp>/`. Copy them
off-host. A same-disk backup protects against configuration mistakes, not disk or
host failure. Treat backup directories as sensitive because they may contain `.env`.

For an unattended offsite copy that streams a single tar to Dropbox and never
keeps it on disk (everything except media, generated metadata, and secrets),
see [dropbox-backup.md](dropbox-backup.md).

## Safe restore

1. Stop the stack and ensure no NzbDAV job is active:

   ```bash
   docker compose down
   ```

2. Restore the configuration tree from the backup, preserving ownership and
   permissions. Do not restore retired-service directories into the active tree:

   ```bash
   cp -a backups/bearcave_backup_<timestamp>/configs/config/. config/
   cp -a backups/bearcave_backup_<timestamp>/secrets/. secrets/ 2>/dev/null || true
   cp backups/bearcave_backup_<timestamp>/.env .env 2>/dev/null || true
   chmod 600 .env 2>/dev/null || true
   chmod 700 secrets 2>/dev/null || true
   ```

3. Validate the restored configuration:

   ```bash
   docker compose config --quiet
   python3 scripts/check_compose_mounts.py
   ```

4. Start the dependency chain and wait for the FUSE mount:

   ```bash
   docker compose up -d
   ./tests/health/run-all.sh
   python3 scripts/check_mount_drift.py
   docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav
   ```

5. Verify Plex sees `/data/movies` and `/data/shows` before scanning. If the
   database was restored, do not empty Plex trash until the mount and expected
   files are confirmed.

Plex runs as UID/GID 955 in the container. If a manual restore changes ownership,
correct it before starting Plex:

```bash
chown -R 955:955 config/plex
```

## NzbDAV warning

Always query the queue before recreating NzbDAV:

```bash
./scripts/update-nzbdav.sh --dry-run
```

The guarded update script refuses an unknown or non-empty queue. `--force` is a
last resort: queued NZBs will be lost and may be blocklisted.

## Scheduled backup

A host cron or systemd timer can run the backup without involving Compose, for
example nightly at 03:30:

```cron
30 3 * * * cd /home/bear/cave && ./scripts/backup.sh >> /var/log/bearcave-backup.log 2>&1
```

Prune old backups only after confirming an off-host copy exists.

## Recovery checklist

- [ ] Off-host backup completed
- [ ] `.env` and `secrets/` restored with restricted permissions
- [ ] `docker compose config --quiet` passes
- [ ] NzbDAV queue was empty or explicitly handled before recreation
- [ ] FUSE mount is healthy before any Plex scan
- [ ] Plex sections and expected files are visible before emptying trash
- [ ] All eight health checks pass
