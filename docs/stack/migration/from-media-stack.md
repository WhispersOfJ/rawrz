# Migrating from media-stack

> **Historical** — this merge completed 2026-08-26 (see [HISTORY.md](../../HISTORY.md#pre-merge-history)).
> Kept for reference if the migration ever needs to be re-run; the stack today is 29 services.

Moving an existing media-stack deployment into The Bear Cave. The goal: zero data loss,
zero library re-matching, one uninterrupted evening.

---

## What changes

| | media-stack | The Bear Cave |
|---|---|---|
| Compose file | `docker-compose.yml` at repo root | same, but grew from 22 services at merge to 29 today |
| Config layout | `./config/<app>/` | `./services/<app>/config/` |
| Network | `stacknet` | `bearcave` + `traefik` |
| Reverse proxy | none | Traefik :80/:443 |
| Metacache | built from external path | built from `services/metacache/` |
| Secrets | `.env` only | `.env` + `secrets/` |

**Data migration is mostly a copy job** — app configs are self-contained directories,
so nothing needs to be re-matched by hand.

---

## Step 1 — Copy app configs

```bash
# From the old repo:
SRC=/home/bear/Claude/media-stack
DST=/home/bear/cave

mkdir -p $DST/services

for app in prowlarr radarr sonarr nzbdav nzbdav-rclone seerr \
           plex watchstate; do
  if [ -d "$SRC/config/$app" ]; then
    mkdir -p "$DST/services/$app"
    cp -a "$SRC/config/$app" "$DST/services/$app/config"
  fi
done

# Loki/Promtail/Prometheus/Grafana configs live in the repo (not gitignored),
# but their DATA dirs migrate too:
cp -a "$SRC/data" "$DST/data" 2>/dev/null || true
cp -a "$SRC/logs" "$DST/logs" 2>/dev/null || true
```

## Step 2 — Plex (the important one)

```bash
# media-stack also kept Plex at ./config/plex. Copy the whole tree:
cp -a "$SRC/config/plex" "$DST/config/plex"

# Ownership must stay 955:955 (the image runs as PLEX_UID/PLEX_GID)
chown -R 955:955 "$DST/config/plex"
chown -R 955:955 "$DST/services/plex/transcode" 2>/dev/null || true
```

`PLEX_MEDIA_SERVER_APPLICATION_SUPPORT_DIR=/config` and the flat layout are identical,
so Plex boots with the same library, watch history, users, and settings.

## Step 3 — rclone.conf

```bash
# The old stack's rclone.conf already has the obscure-encoded password:
cp "$SRC/config/nzbdav-rclone/rclone.conf" "$DST/config/nzbdav-rclone/rclone.conf"
```

If you only have the plaintext WebDAV password: `rclone obscure "password"` and put the
output in the file.

## Step 4 — .env

```bash
cp $DST/.env.template $DST/.env
# Copy every real value from the old .env into the new one.
# New vars you must set: TRAEFIK_DASHBOARD_AUTH, ACME_EMAIL (optional)
```

All the old variable names are unchanged — the compose file uses the same `${VAR}`
names (RADARR_API_KEY, PLEX_TOKEN, NZBDAV_*, WS_*, etc.).

## Step 5 — First boot

```bash
cd $DST
./scripts/setup.sh --validate-only   # config + env sanity
docker compose up -d --build
docker compose ps                    # all healthy?
./tests/health/run-all.sh
```

## Step 6 — Post-boot verification

1. **Plex** — open it, confirm the library is intact (sections, watch history, users)
2. **Radarr/Sonarr** — download clients point at `nzbdav:3000` (hostname changed from
   the old network name? No — the compose service name is still `nzbdav`, so internal
   DNS is unchanged)
3. **Metacache** — warm and register in Plex
4. **Traefik** — hit each `https://<svc>.HOST_IP.nip.io`
5. **InfiniDysk** — confirm the queue/import health and streaming

## Gotchas

- **FUSE mount**: the mount target path is identical (`/mnt/remote/nzbdav`), so no
  symlink rewrites are needed — but pre-cutover symlinks are broken anyway; new imports
  only (same as every past cutover)
- **Do not run both stacks at once** — they share `/mnt/remote/nzbdav` and ports
- **Old fish functions** — archived at `archive/media-stack/fish-functions/`; they
  reference the old repo's paths, update the `stack-claude-home`-style paths or drop them
- **WatchState** — its DB copies over; keep `WS_CRON_IMPORT=true` and the webhook
