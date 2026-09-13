# M3 nginx ingress

nginx is part of the primary `docker-compose.yml` deployment. It terminates
internal TLS on `443`, redirects `80` to `443`, routes the current services by
`*.rawrz.lan`, and retains each native application port as the rollback path.

## Start

`./scripts/setup.sh` creates the runtime directories and generates the uncommitted
LAN CA and SAN certificate under `config/nginx/certs`. Trust only
`config/nginx/certs/rawrz-ca.crt` on LAN clients and resolve the `*.rawrz.lan`
names to the stack host.

```bash
docker compose up -d
./services/nginx/test-ingress.sh
```

For a side-by-side check on a host already using ports 80/443, use the primary
Compose graph with `M3_HTTP_PORT=8081 M3_HTTPS_PORT=8443`; the acceptance script
passes those values to Compose. CI uses this mode and a disposable in-network
fixture, so it never depends on live media services or the internet.

Current routes are Seerr, Radarr, Sonarr, Prowlarr, NzbDAV, and Plex. Deck, RPG,
metrics, and Grafana names are reserved until those services enter Compose. Unified
RAWRZ authentication is a later milestone; native application authentication
remains required.

The cache allowlist is explicit and GET/HEAD-only. Requests with cookies or
`Authorization`, mutating routes, upstream `Set-Cookie`, and `private`/`no-store`
responses bypass storage. Proxied responses expose `X-Cache-Status`.

## Rollback

Stop only nginx to return to the unchanged native ports:

```bash
docker compose rm -sf nginx
```
