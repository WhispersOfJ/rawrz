# Stack Maintenance Checklist

The active deployment intentionally contains eight services (Bazarr re-retired
2026-09-06). Retired services are
not activated from this document; see [services/lifecycle.md](../services/lifecycle.md)
for historical context and re-adoption policy.

## Before a Compose change

```bash
git status --short
docker compose config --quiet
python3 scripts/check_compose_mounts.py
bash -n scripts/*.sh tests/*/*.sh
```

For changes to NzbDAV, rclone, or any FUSE consumer, confirm the queue and mount:

```bash
./scripts/update-nzbdav.sh --dry-run
python3 scripts/check_mount_drift.py
docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav
```

## Deployment order

```bash
docker compose up -d prowlarr
# NzbDAV is health-gated on Prowlarr.
docker compose up -d nzbdav nzbdav_rclone
# Consumers are health-gated on the mount.
docker compose up -d radarr sonarr plex unpackerr seerr
```

A normal deployment may use `docker compose up -d`; the dependency graph preserves
this order. Never use `--force` on NzbDAV unless queued work is intentionally being
discarded.

## Post-change verification

```bash
docker compose ps
./tests/health/run-all.sh
python3 scripts/check_mount_drift.py
./tests/integration/test_pipeline.sh
```

Verify the application roots remain `/data/movies` and `/data/shows`, and verify
Plex still reports the Movies and Shows sections. Do not empty Plex trash during a
mount outage or before the expected paths are visible.

## Rollback

For a Compose-only change, restore the reviewed previous file and run
`docker compose up -d` after checking the queue. For an NzbDAV image change, use
`scripts/update-nzbdav.sh`; it enforces the queue guard and waits for the mount
cascade to recover.
