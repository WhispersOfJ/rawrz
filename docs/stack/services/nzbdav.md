# InfiniDysk (NzbDAV)

InfiniDysk is the only downloader. It retrieves Usenet content, exposes a SABnzbd-
compatible API, and serves the remote tree over WebDAV.

| | |
|---|---|
| Image | `ghcr.io/infinidysk/infinidysk:dev` |
| Port | 3000 |
| Network | `bearcave` |
| Config | `config/nzbdav/` |
| Depends on | Prowlarr healthy |
| Healthcheck | `curl -fsSL http://localhost:3000/healthz` |

## Active configuration

The Compose environment supplies the API key, WebDAV credentials, Usenet providers,
Radarr/Sonarr instances, repair settings, Prowlarr sync, queue workers, caches, and
watchtower/preflight settings. Repair health checks are capped at 24 concurrent
operations to avoid a remote-request stampede during Plex scans. The container has a
2.5 GiB memory cap and 2 CPU quota for concurrent provider/WebDAV work. Do not replace
the `NZBDAV_CONFIG__*` block with only `WEBDAV_USERNAME`/`WEBDAV_PASSWORD`; those
names do not configure this image.

Important values:

- `FRONTEND_BACKEND_API_KEY`: shared SABnzbd-compatible API key
- `NZBDAV_WEBDAV_USER/PASS`: WebDAV credentials used by rclone
- `NZBDAV_RCLONE_RC_PASS`: rclone remote-control password
- `NZBDAV_USENET_*`: provider credentials — `NZBDAV_USENET_*` (primary) and
  `NZBDAV_USENET_BACKUP_*` (backup). Both are injected into InfiniDysk's
  `NZBDAV_CONFIG__USENET__PROVIDERS` JSON in Compose. A third slot (Eweka,
  `NZBDAV_USENET_EWEKA_*`) was retired 2026-09-06 after the provider rejected
  its credentials server-side; the re-adoption path is documented next to the
  commented-out block in `.env.template`.
- `NZBDAV_PROFILE_TOKEN`: profile/watchtower token

## Library directory (orphan-cleanup protection)

`NZBDAV_CONFIG__MEDIA__LIBRARY_DIR` tells InfiniDysk where the organized library
root lives so its Remove Orphaned Files maintenance can tell Arr-imported content
apart from true orphans. It is set to `/media`, backed by a read-only bind of the
host `./media` directory (the parent of the Radarr/Sonarr root folders):

```yaml
volumes:
  - ./media:/media:ro
environment:
  NZBDAV_CONFIG__MEDIA__LIBRARY_DIR: "/media"
```

`./media/shows` and `./media/movies` hold the ~137k Arr-imported symlinks that
point at InfiniDysk `.ids` content objects; the read-only bind is safe because the
maintenance task only *scans* the library dir and deletes orphans from its own store.

Do **not** point this at `/mnt/remote/nzbdav/completed-symlinks` (the historical
value). That folder is InfiniDysk's virtual view of current history rows and sits
inside the rclone mount; per the upstream docs (infinidysk.com/operations/
retention-cleanup) it cannot protect files after history is cleared, and Remove
Orphaned Files **aborts — dry run included** — when the Library Directory is the
mount or a path inside it. That abort is why orphan cleanup never ran before the
2026-09 fix.

The orphan-cleanup schedule stays disabled
(`NZBDAV_CONFIG__MAINTENANCE__REMOVE_ORPHANED_SCHEDULE_ENABLED: "false"`); runs
are manual only and should always start with the task's dry run and an audit that
no Arr-imported item is a candidate. Rollback of the fix is a two-line revert
(remove the bind, restore the old value) plus `docker compose up -d --no-deps
--force-recreate nzbdav`.

## Queue safety

The queue is not persistent across container recreation. Always check it first:

```bash
KEY=$(grep '^FRONTEND_BACKEND_API_KEY=' .env | cut -d= -f2)
python3 scripts/check_nzbdav_queue.py --api-key "$KEY"
```

Use `scripts/update-nzbdav.sh` for updates. `--force` is only for knowingly accepting
queued-NZB loss.

## Backend wedge watchdog

Landmine #13: the `:3000` frontend can stay healthy while the internal backend on
`:8080` dies — Docker marks the container `unhealthy`, but `restart:
unless-stopped` only fires on *exit*, so nothing acts on it (2026-09-05: backend
process alive with zero open sockets for hours).

`scripts/watch_nzbdav_backend.py` is the actor. It probes the wedge signature
from the host — frontend `/healthz` up + queue API returning 502 — then restarts
the container *queue-safely* (restart, not recreate: `/config/db.sqlite`
survives, queued NZBs are preserved) and pings Discord on detection and
recovery. A crash-loop guard (≥3 restarts in 30 min) stops auto-restarting and
escalates instead of thrashing a bad `:dev` build.

```bash
python3 scripts/watch_nzbdav_backend.py --check   # probe only: 0 healthy / 3 wedged / 4 frontend-down
```

Installed as a user timer (`stack-nzbdav-watch`, every minute); install recipe is
in the script's header, mirroring `stack-arrival-notify`. `DISCORD_WEBHOOK_URL`
unset → detection/restart still happens, alerts are skipped.

## Troubleshooting

```bash
curl -sf http://localhost:3000/healthz
docker compose logs --tail=100 nzbdav
docker exec nzbdav_rclone rclone lsd nzbdav: --config /config/rclone/rclone.conf
```
