#!/usr/bin/env bash
# Real-surface acceptance for the M3 nginx ingress.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT_DIR"
COMPOSE=(docker compose -f docker-compose.yml)
NGINX_SERVICE=nginx
FIXTURE_COMPOSE=(docker compose -f docker-compose.m3.fixture.yml)
HTTP_PORT=${M3_HTTP_PORT:-80}
HTTPS_PORT=${M3_HTTPS_PORT:-443}
export NGINX_HTTP_PORT="$HTTP_PORT" NGINX_HTTPS_PORT="$HTTPS_PORT"

cleanup() {
    "${COMPOSE[@]}" rm -sf "$NGINX_SERVICE" >/dev/null 2>&1 || true
    if [ "${M3_UPSTREAM:-}" = fixture ]; then
        "${FIXTURE_COMPOSE[@]}" rm -sf fixture >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

fail() {
    printf 'M3 acceptance failed: %s\n' "$1" >&2
    exit 1
}

expect() {
    local name="$1" actual="$2" wanted="$3"
    [ "$actual" = "$wanted" ] || fail "$name: expected '$wanted', got '$actual'"
    printf 'OK: %s\n' "$name"
}

./services/nginx/generate-cert.sh >/dev/null
if [ "${M3_UPSTREAM:-}" = fixture ]; then
    docker network inspect bearcave >/dev/null 2>&1 || docker network create bearcave >/dev/null
    "${FIXTURE_COMPOSE[@]}" up -d fixture >/dev/null
fi
"${COMPOSE[@]}" up -d --no-deps "$NGINX_SERVICE" >/dev/null
"${COMPOSE[@]}" exec -T "$NGINX_SERVICE" nginx -t >/dev/null

health=$(curl -fsS "http://127.0.0.1:${HTTP_PORT}/healthz")
expect 'HTTP health endpoint' "$health" 'ok'

redirect=$(curl -sS -o /dev/null -w '%{http_code} %{redirect_url}' \
    -H 'Host: seerr.rawrz.lan' "http://127.0.0.1:${HTTP_PORT}/api/v1/status")
expect 'HTTP redirects to HTTPS' "$redirect" "308 https://seerr.rawrz.lan/api/v1/status"

request() {
    curl -ksS -D - --resolve "seerr.rawrz.lan:${HTTPS_PORT}:127.0.0.1" \
        "https://seerr.rawrz.lan:${HTTPS_PORT}/api/v1/status${1:-}" -o "$2"
}

headers=$(request '' /tmp/rawrz-m3-body-1)
status=$(printf '%s\n' "$headers" | awk 'NR == 1 { print $2 }')
cache=$(printf '%s\n' "$headers" | awk 'BEGIN { IGNORECASE=1 } /^X-Cache-Status:/ { gsub("\r", "", $2); print $2 }')
expect 'HTTPS Seerr route responds' "$status" '200'
expect 'first cache status is MISS' "$cache" 'MISS'

headers=$(request '' /tmp/rawrz-m3-body-2)
cache=$(printf '%s\n' "$headers" | awk 'BEGIN { IGNORECASE=1 } /^X-Cache-Status:/ { gsub("\r", "", $2); print $2 }')
expect 'repeat Seerr route is a HIT' "$cache" 'HIT'
cmp /tmp/rawrz-m3-body-1 /tmp/rawrz-m3-body-2 || fail 'cached response body changed'

headers=$(curl -ksS -D - --resolve "seerr.rawrz.lan:${HTTPS_PORT}:127.0.0.1" \
    -H 'Cookie: session=deliberate-bypass' \
    "https://seerr.rawrz.lan:${HTTPS_PORT}/api/v1/status" -o /tmp/rawrz-m3-body-cookie)
cache=$(printf '%s\n' "$headers" | awk 'BEGIN { IGNORECASE=1 } /^X-Cache-Status:/ { gsub("\r", "", $2); print $2 }')
expect 'cookie request bypasses cache' "$cache" 'BYPASS'

for spec in \
    'radarr.rawrz.lan /ping' \
    'sonarr.rawrz.lan /ping' \
    'prowlarr.rawrz.lan /ping' \
    'nzbdav.rawrz.lan /healthz' \
    'plex.rawrz.lan /identity'; do
    read -r host path <<< "$spec"
    status=$(curl -ksS --max-time 10 -o /dev/null -w '%{http_code}' \
        --resolve "${host}:${HTTPS_PORT}:127.0.0.1" "https://${host}:${HTTPS_PORT}${path}")
    expect "${host} route responds" "$status" '200'
done

unknown=$(curl -ksS -o /dev/null -w '%{http_code}' \
    --resolve "unknown.rawrz.lan:${HTTPS_PORT}:127.0.0.1" "https://unknown.rawrz.lan:${HTTPS_PORT}/" || true)
expect 'unknown TLS host is rejected' "$unknown" '421'

rm -f /tmp/rawrz-m3-body-1 /tmp/rawrz-m3-body-2 /tmp/rawrz-m3-body-cookie
printf 'M3 ingress acceptance passed\n'
