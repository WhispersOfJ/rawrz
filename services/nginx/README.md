# M3 side-by-side nginx ingress

This overlay adds the first reversible M3 ingress slice without changing the
existing direct host ports. It listens on host ports `8081` and `8443`, routes
currently deployed services by `*.rawrz.lan`, and keeps native ports available
as the rollback path.

## Start locally

1. Prepare the normal stack bind mounts and `.env` with `./scripts/setup.sh`.
2. Generate a local certificate and start nginx:

   ```bash
   ./services/nginx/generate-cert.sh
   docker compose -f docker-compose.yml -f docker-compose.m3.yml up -d nginx
   ```

3. Point the `*.rawrz.lan` names at the stack host using LAN DNS or `/etc/hosts`.
   Trust `config/nginx/certs/rawrz-ca.crt` on each client. The overlay's host
   ports are intentionally not 80/443.
4. Verify the route through the real ingress:

   ```bash
   ./services/nginx/test-ingress.sh
   ```

The overlay currently fronts Seerr, Radarr, Sonarr, Prowlarr, NzbDAV, and Plex.
Deck, RPG, metrics, and Grafana virtual hosts will be added when those services
enter the Compose stack. Authentication remains owned by each existing service;
nginx does not invent an auth endpoint before the unified auth service exists.

The cache allowlist is deliberately small and GET-only. Requests with cookies,
`Authorization`, or mutating methods bypass cache, and upstream `Set-Cookie` /
`Cache-Control: no-store` responses are never stored. Every proxied response has
`X-Cache-Status`; direct service ports remain available for rollback.

## CI

The M3 workflow runs the overlay against a disposable in-network fixture, so it
requires no live media services or internet access. The production overlay can
then be exercised against the running stack by leaving `M3_UPSTREAM` unset.

## Rollback

Stop and remove only the overlay:

```bash
docker compose -f docker-compose.yml -f docker-compose.m3.yml rm -sf nginx
```

The original service containers and their published ports are not modified.
