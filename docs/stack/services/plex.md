# Plex

Plex is the media server and the only container using host networking. It serves the
Movies and Shows libraries from symlinks backed by the NzbDAV WebDAV/FUSE mount.

| | |
|---|---|
| Image | `plexinc/pms-docker:latest` (mutable tag) |
| Port | 32400, direct host network |
| Config | `config/plex/` |
| Transcode | `config/plex-transcode/` |
| Depends on | `nzbdav_rclone` healthy, with restart cascade |
| Hardware | `/dev/dri` for VAAPI |

## Library paths

| Host path | Container path |
|---|---|
| `media/movies/` | `/data/movies` |
| `media/shows/` | `/data/shows` |
| `/mnt/remote/nzbdav` | `/mnt/remote/nzbdav` with `rslave` propagation |

The existing Plex database contains the `Movies` and `Shows` sections. Preserve
`config/plex/` during upgrades; it contains library metadata and watch history.

## Required settings

- `PLEX_UID=955` and `PLEX_GID=955` preserve the migrated library ownership.
- `PLEX_MEDIA_SERVER_APPLICATION_SUPPORT_DIR=/config` selects the active database tree.
- `stop_grace_period: 90s` is required; Plex can take roughly 40 seconds to stop under load.
- Plex uses direct HTTP at `http://HOST_IP:32400`; there is no reverse proxy or TLS layer.

## Stale-mount recovery

Red trash cans or missing files/seasons usually mean Plex scanned while the FUSE mount
was unavailable. Do not empty trash first:

```bash
docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav
docker exec nzbdav_rclone ls /mnt/remote/nzbdav | head
docker compose restart plex
```

After the mount and Plex are healthy, trigger a scan. Empty trash only after the expected
files are visible again.

## Hardware transcoding

Verify `/dev/dri` is visible in the container and enable hardware acceleration in Plex.
Library analysis and scans still use CPU even when playback uses VAAPI.
