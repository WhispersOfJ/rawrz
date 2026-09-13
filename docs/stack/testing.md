# Testing

Validation for the nine-service stack, including nginx: Prowlarr, Radarr, Sonarr, NzbDAV,
nzbdav_rclone, Seerr, Plex, and Unpackerr.

## Repository checks

```bash
docker compose config --quiet
bash -n scripts/*.sh tests/*/*.sh
./tests/bash/test_bash_functions.sh --offline
python3 scripts/test_check_redis.py
python3 scripts/test_activity_feed.py
./scripts/preflight.sh    # requires the pinned local actionlint build
```

The preflight gate checks compose syntax, mount declarations, mount and
config drift (running-container images vs compose pins; mount drift vs
compose definitions), MCP configuration, NzbDAV queue safety, bind-mount
staleness, Python compilation, Radarr + Sonarr DB size gates, retired
residue, and available lint tools.

## M1 Redis acceptance

Redis is internal-only and is consumed by the host-run activity feed when
`REDIS_URL` is set. The cache is namespaced, uses a 60-second history-page TTL,
and fails open: a Redis outage causes a live API fetch rather than a feed outage.

For a live acceptance run, use a temporary Redis endpoint or the Compose Redis
service. The script needs only Python's standard library:

```bash
export REDIS_URL=redis://127.0.0.1:6380/0
KEY="m1-acceptance-$(date +%s)"
VALUE="$(python3 -c 'import uuid; print(uuid.uuid4().hex)')"
python3 scripts/test_redis_runtime.py --phase write --key "$KEY" --value "$VALUE"
# Restart Redis, wait for its healthcheck, then:
python3 scripts/test_redis_runtime.py --phase verify --key "$KEY" --value "$VALUE"
# Stop Redis and verify fail-open behavior:
python3 scripts/test_redis_runtime.py --phase outage --key "$KEY" --value "$VALUE"
```

The runtime gate proves `PING`, JSON `SET`/`GET`, TTL, AOF persistence across a
restart, and an outage recorded as a cache miss/error. The activity-feed
regression test additionally proves a cached history page avoids the upstream
HTTP request and that changing the feed cursor changes the cache key, preventing
stale history from hiding new events. Do not run the outage phase against a
production endpoint.

## Health checks

```bash
./tests/health/run-all.sh
./tests/health/run-all.sh --service plex
./tests/health/run-all.sh --verbose
```

The health runner checks all configured containers. A service with no Docker
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
| After compose changes | compose config, Redis contract, bash syntax, health checks |
| After activity-feed or Redis changes | activity-feed regression test and M1 runtime acceptance |
| After NzbDAV/rclone changes | queue check, mount-drift check, pipeline test |
| After Plex mount recovery | mount check, Plex identity, rescan verification |
| Before merging | preflight and offline smoke tests |
| After restoring backup | full health and pipeline checks |
