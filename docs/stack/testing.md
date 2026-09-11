# Testing

Validation for the lean eight-service stack: Prowlarr, Radarr, Sonarr, NzbDAV,
nzbdav_rclone, Seerr, Plex, and Unpackerr.

## Repository checks

```bash
docker compose config --quiet
bash -n scripts/*.sh tests/*/*.sh
./tests/bash/test_bash_functions.sh --offline
./scripts/preflight.sh
```

The preflight gate checks compose syntax, mount declarations, mount and
config drift (running-container images vs compose pins; mount drift vs
compose definitions), MCP configuration, NzbDAV queue safety, bind-mount
staleness, Python compilation, Radarr + Sonarr DB size gates (AGENTS.md
landmine #9 — `MovieFiles` and `EpisodeFiles` MediaInfo bloat), retired
residue, and available lint tools.

## Health checks

```bash
./tests/health/run-all.sh
./tests/health/run-all.sh --service plex
./tests/health/run-all.sh --verbose
```

The health runner checks all eight configured containers. A service with no Docker
healthcheck, such as Unpackerr, is considered passing when its container is running.

## Pipeline integration test

```bash
./tests/integration/test_pipeline.sh --dry-run
./tests/integration/test_pipeline.sh
```

The live test covers Docker readiness, Plex/Radarr/Sonarr/NzbDAV availability, the
rclone mount and RC endpoint, Plex library access, *arr root folders, NzbDAV health,
and sampled symlink integrity. The test must not scan or mutate media.

## Live critical-path checks

```bash
# NzbDAV health
curl -sf http://localhost:3000/healthz

# Authenticated queue
KEY=$(grep '^FRONTEND_BACKEND_API_KEY=' .env | cut -d= -f2)
curl -sf "http://localhost:3000/api?mode=queue&output=json&apikey=$KEY"

# FUSE mount
mountpoint -q /mnt/remote/nzbdav || docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav

docker exec nzbdav_rclone ls /mnt/remote/nzbdav | head
docker exec nzbdav_rclone rclone lsd nzbdav: --config /config/rclone/rclone.conf

# Application pings
curl -sf http://localhost:9696/ping
curl -sf http://localhost:7878/ping
curl -sf http://localhost:8989/ping
curl -sf http://localhost:5055/api/v1/status
curl -sf http://localhost:32400/identity
```

## Plex rescan verification

When Plex displays red trash cans or missing seasons, verify the FUSE mount first.
After recovery, trigger the scan and verify the sections before emptying trash:

```bash
stack-plex scan
# or use the Plex UI: Library → Scan Library Files
```

Only empty trash once the expected files and seasons are visible again.

## When to run what

| Moment | Checks |
|---|---|
| After compose changes | compose config, bash syntax, health checks |
| After NzbDAV/rclone changes | queue check, mount-drift check, pipeline test |
| After Plex mount recovery | mount check, Plex identity, rescan verification |
| Before merging | preflight and offline fish tests |
| After restoring backup | full health and pipeline checks |
