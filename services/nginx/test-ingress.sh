#!/usr/bin/env bash
# Real-surface acceptance for the side-by-side M3 ingress.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT_DIR"
COMPOSE=(docker compose -f docker-compose.yml -f docker-compose.m3.yml)
NGINX_SERVICE=nginx
FIXTURE_COMPOSE=(docker compose -f docker-compose.m3.fixture.yml)

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

health=$(curl -fsS http://127.0.0.1:8081/healthz)
expect 'HTTP health endpoint' "$health" 'ok'

redirect=$(curl -sS -o /dev/null -w '%{http_code} %{redirect_url}' \
    -H 'Host: seerr.rawrz.lan' http://127.0.0.1:8081/api/v1/status)
expect 'HTTP redirects to HTTPS' "$redirect" '308 https://seerr.rawrz.lan/api/v1/status'

request() {
    curl -ksS -D - --resolve seerr.rawrz.lan:8443:127.0.0.1 \
        "https://seerr.rawrz.lan:8443/api/v1/status${1:-}" -o "$2"
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

headers=$(curl -ksS -D - --resolve seerr.rawrz.lan:8443:127.0.0.1 \
    -H 'Cookie: session=deliberate-bypass' \
    https://seerr.rawrz.lan:8443/api/v1/status -o /tmp/rawrz-m3-body-cookie)
cache=$(printf '%s\n' "$headers" | awk 'BEGIN { IGNORECASE=1 } /^X-Cache-Status:/ { gsub("\r", "", $2); print $2 }')
expect 'cookie request bypasses cache' "$cache" 'BYPASS'

headers=$(curl -ksS -D - --resolve radarr.rawrz.lan:8443:127.0.0.1 \
    https://radarr.rawrz.lan:8443/ping -o /tmp/rawrz-m3-body-ping)
cache=$(printf '%s\n' "$headers" | awk 'BEGIN { IGNORECASE=1 } /^X-Cache-Status:/ { gsub("\r", "", $2); print $2 }')
expect 'non-allowlisted route bypasses cache' "$cache" 'BYPASS'

unknown=$(curl -ksS -o /dev/null -w '%{http_code}' \
    --resolve unknown.rawrz.lan:8443:127.0.0.1 https://unknown.rawrz.lan:8443/ || true)
expect 'unknown TLS host is rejected' "$unknown" '421'

rm -f /tmp/rawrz-m3-body-1 /tmp/rawrz-m3-body-2 /tmp/rawrz-m3-body-cookie /tmp/rawrz-m3-body-ping
printf 'M3 ingress acceptance passed\n'
