# M3 nginx ingress

M3 adds nginx as the active internal TLS ingress while retaining direct application
ports as a reversible rollback path. The ingress is defined in `docker-compose.yml`;
`services/nginx/` owns its config, certificate generator, route allowlist, and
acceptance test.

Use `./services/nginx/generate-cert.sh` after setup, trust the generated
`config/nginx/certs/rawrz-ca.crt` on LAN clients, resolve `*.rawrz.lan` to the host,
and run `./services/nginx/test-ingress.sh`.

The current stack fronts Seerr, Radarr, Sonarr, Prowlarr, NzbDAV, and Plex. Deck,
RPG, metrics, and Grafana routes are reserved until those services enter Compose.
Unified authentication is intentionally deferred; native service authentication
remains required.
