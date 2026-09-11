# Quick Start

Get The Bear Cave running from zero to streaming with the eight-service stack.

## 1. Prerequisites

- Linux host with Docker Engine 24+ and Compose v2 (`docker compose`)
- `/dev/fuse` and a `/dev/dri/renderD*` node available for rclone and Plex VAAPI
- Credentials for the primary and backup Usenet provider slots
- Optional Intel/AMD GPU exposed as `/dev/dri` for Plex hardware transcoding

Docker Desktop for macOS/Windows is not supported: FUSE and Plex host networking
are Linux-specific requirements.

## 2. Clone and configure

```bash
git clone https://github.com/WhispersOfJ/thebearcave.git
cd thebearcave
cp .env.template .env
nano .env
```

Set real values for `HOST_IP`, Plex and *arr API keys, `FRONTEND_BACKEND_API_KEY`,
WebDAV credentials, `NZBDAV_PROFILE_TOKEN`, and all three Usenet provider blocks.
The setup validator rejects `changeme` placeholders because Compose otherwise
starts services with unusable credentials. The generated rclone config and all
bind-mount directories are prepared by `scripts/setup.sh`.

The setup script creates `config/nzbdav-rclone/rclone.conf` with an
rclone-obscured password derived from `NZBDAV_WEBDAV_PASS`. If you customize
that file later, use `rclone obscure` and never store the plaintext password.

## 3. Validate and start

```bash
./scripts/setup.sh
docker compose config --quiet
docker compose up -d
docker compose ps
```

`setup.sh` creates the gitignored service state directories, external FUSE
mountpoint, public CA trust files, and the private rclone config. It refuses to
start when Compose credentials are missing or still set to `changeme`.

Startup order is Prowlarr → NzbDAV → rclone FUSE mount → Radarr/Sonarr/Plex/Unpackerr.
Seerr starts independently and sends requests to Radarr and Sonarr.

## 4. Configure the applications

### Plex

Open `http://HOST_IP:32400/web`, claim the server, and use these library roots:

- `/data/movies` for movies
- `/data/shows` for TV

The media roots contain symlinks into the WebDAV-backed FUSE mount. Confirm the
mount before scanning:

```bash
docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav && echo "MOUNT OK"
```

### Radarr and Sonarr

Open `http://HOST_IP:7878` and `http://HOST_IP:8989`.

1. Add InfiniDysk as the SABnzbd-compatible download client at `nzbdav:3000`.
2. Use `FRONTEND_BACKEND_API_KEY` as the download-client API key.
3. Use `/data/movies` and `/data/shows` as the root folders.
4. Confirm the generated application API keys match `.env`.
5. Prowlarr syncs indexers to both applications.

### Seerr

Open `http://HOST_IP:5055`, complete setup, then connect Plex, Radarr, and Sonarr.

## 5. Verify

```bash
./tests/health/run-all.sh
./tests/integration/test_pipeline.sh --dry-run
python3 scripts/check_mount_drift.py
```

For the critical path, verify the authenticated queue is empty or inspect it with:

```bash
KEY=$(grep '^FRONTEND_BACKEND_API_KEY=' .env | cut -d= -f2)
curl -s "http://localhost:3000/api?mode=queue&output=json&apikey=$KEY"
```

## 6. Plex stale-mount recovery

If Plex shows red trash cans or missing seasons/files:

1. Stop rescans while the mount is unhealthy.
2. Confirm `mountpoint -q /mnt/remote/nzbdav` succeeds.
3. Restart Plex after the mount is restored.
4. Trigger a full scan with `stack-plex scan` or from the Plex UI.
5. Empty the Plex trash only after confirming the expected files are visible.

## Operational notes

- All services use direct host ports; there is no reverse proxy or HTTPS layer.
- `nzbdav` queue state is not persistent across container recreation. Do not recreate
  it until the queue is empty, or explicitly use the dangerous `--force` path.
- `nzbdav_rclone` is the mount owner. Its restart cascades to the four FUSE consumers.
- See [AGENTS.md](../AGENTS.md), [docs/landmines.md](landmines.md), and the per-service
  pages for recovery procedures.
