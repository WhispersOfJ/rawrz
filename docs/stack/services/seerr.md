# Seerr

Seerr is the request and discovery front door for movies and TV.

| | |
|---|---|
| Image | `ghcr.io/seerr-team/seerr:v3.4.1` |
| Port | 5055 |
| Network | `bearcave` |
| Config | `config/seerr/` |
| Healthcheck | `wget -qO- http://localhost:5055/api/v1/status` |

Access it directly at `http://HOST_IP:5055`; no reverse proxy is deployed.
Connect Plex, Radarr, and Sonarr during the setup wizard. Requests flow through the
normal pipeline: Seerr → Radarr/Sonarr → Prowlarr → NzbDAV → Plex.
