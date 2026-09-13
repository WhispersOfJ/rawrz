# Active Service Registry

The default Compose deployment contains nine always-on services: eight media services
plus nginx. All default containerized services except Plex use the `bearcave` bridge
network; Plex uses host networking. Profile-gated maintenance services are listed
separately and are not started by the default deployment.

| Service | Port | Role |
|---------|------|------|
| nginx | 80 / 443 | Internal TLS ingress |
| Prowlarr | 9696 | Indexer manager |
| Radarr | 7878 | Movie acquisition |
| Sonarr | 8989 | TV acquisition |
| NzbDAV / InfiniDysk | 3000 | Usenet download client and WebDAV |
| `nzbdav_rclone` | — | WebDAV-backed FUSE mount |
| Seerr | 5055 | Request management |
| Plex | 32400 | Media server |
| Unpackerr | — | Download extraction |

nginx provides the active internal ingress; direct ports remain available for rollback.
ImageMaid and Recyclarr are manual, profile-gated maintenance services (the
`maintenance` profile) and are excluded from the normal nine-container startup; see
[imagemaid.md](imagemaid.md) and [recyclarr.md](recyclarr.md). Retired services are
tracked in [lifecycle.md](lifecycle.md) and preserved only as historical records where
noted.
