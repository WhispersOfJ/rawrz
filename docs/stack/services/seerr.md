# Seerr

Seerr is the request and discovery front door for movies and TV.

| | |
|---|---|
| Image | `ghcr.io/whispersofj/rawrz-seerr@sha256:bd2aa03911a54d48dff1c052564f49ae2d02f575f592cce1bd7c6d5b11908a96` |
| Direct port | 5055 |
| Ingress | `https://seerr.rawrz.lan` |
| Network | `bearcave` |
| Config | `config/seerr/` |
| Healthcheck | `wget -qO- http://localhost:5055/api/v1/status` |

Use the nginx hostname for normal LAN access after trusting the generated internal
CA. The direct `:5055` port remains available as the M2 rollback/bypass path.
Connect Plex, Radarr, and Sonarr during the setup wizard. Requests flow through the
normal pipeline: Seerr → Radarr/Sonarr → Prowlarr → NzbDAV → Plex.
