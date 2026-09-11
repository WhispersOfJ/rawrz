# Unpackerr

Unpackerr extracts completed archives for Radarr and Sonarr.

| | |
|---|---|
| Image | `golift/unpackerr:0.15.2` |
| Network | `bearcave` |
| Published ports | none |
| Memory cap | 64 MiB |
| Depends on | `nzbdav_rclone` healthy, with restart cascade |

## Environment

- `UN_RADARR_0_URL=http://radarr:7878`
- `UN_RADARR_0_API_KEY=${RADARR_API_KEY}`
- `UN_SONARR_0_URL=http://sonarr:8989`
- `UN_SONARR_0_API_KEY=${SONARR_API_KEY}`

## Paths

The container reads the download and FUSE trees at `/usenet` and
`/mnt/remote/nzbdav`. It has no web UI; inspect logs for extraction failures:

```bash
docker compose logs --tail=100 unpackerr
```
