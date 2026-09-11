# Prowlarr

Prowlarr manages the indexers shared by Radarr and Sonarr.

| | |
|---|---|
| Image | `ghcr.io/hotio/prowlarr:release-2.5.2.5491` |
| Port | 9696 |
| Network | `bearcave` |
| Config | `config/prowlarr/` |
| Healthcheck | `curl -sf http://localhost:9696/ping` |

Access it directly at `http://HOST_IP:9696`. NzbDAV also uses Prowlarr’s internal
service address for its periodic indexer synchronization.
