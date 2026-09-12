# Landmines

Read this before changing the active stack. The two fragile resources are the
NzbDAV queue and the rclone FUSE mount.

## Critical

### 1. NzbDAV queue is not persistent

Recreating `nzbdav` can wipe queued NZBs and blocklist affected items. Always check
before a recreate:

```bash
./scripts/update-nzbdav.sh --dry-run
```

The update script refuses an unknown or non-empty queue. `--force` is intentionally
dangerous and should be used only when queued work may be discarded.

### 2. FUSE mount cascade

`nzbdav_rclone` owns `/mnt/remote/nzbdav`. Radarr, Sonarr, Plex, and Unpackerr use
that mount and are health-gated dependents with `restart: true`.

- Check the mount before a Plex scan:
  `docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav`.
- Never force-unmount the mount while consumers are running.
- If Plex shows red trash cans or missing files, restore the mount first, restart
  consumers, rescan, and empty trash only after the expected files are visible.
- Run `python3 scripts/check_mount_drift.py` after mount or Compose changes.

### 3. Bind-mounted files can be stale

Replacing a single-file bind mount changes its host inode while the container can
continue serving the old inode. Restart the affected container after changing a
single-file bind, then run:

```bash
python3 scripts/check_bind_mount_staleness.py
```

## High

### 4. Paths are part of the application contract

The databases and containers must agree on these paths:

- Radarr: `/data/movies`
- Sonarr: `/data/shows`
- Plex: `/data/movies` and `/data/shows`
- Shared FUSE tree: `/mnt/remote/nzbdav`

Changing only Compose mounts makes roots inaccessible and can make Plex mark
content as deleted.

### 5. Plex shutdown is deliberately slow

Plex has `stop_grace_period: 90s`. Allow it to shut down cleanly under load:

```bash
docker stop -t 90 plex
```

### 6. Direct ports are LAN surfaces

There is no reverse proxy or central authentication tier. Keep ports 3000, 5055,
7878, 8989, 9696, and 32400 behind the host firewall/VPN and retain native app
authentication. The rclone RC port is not published to the host.

## Diagnostics

| Symptom | First check |
|---------|-------------|
| Plex shows red trash cans | FUSE mount health, then `docker compose logs plex` |
| Imports are stuck | `docker compose logs nzbdav radarr sonarr unpackerr` |
| Root folder inaccessible | Confirm `/data/movies` or `/data/shows` and mount drift |
| NzbDAV returns 401 | Compare `.env` key with the running `nzbdav` environment |
| A recreate is requested | Run `./scripts/update-nzbdav.sh --dry-run` first |
| Everything is unhealthy | `docker compose ps`, then inspect the first dependency failure |

For recovery procedures see [operations/troubleshooting.md](operations/troubleshooting.md)
and [operations/backup-restore.md](operations/backup-restore.md).
