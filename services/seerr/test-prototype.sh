#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
IMAGE=${SEERR_IMAGE:-rawrz-seerr:m2}
REDIS_CONTAINER=${SEERR_TEST_REDIS_CONTAINER:-rawrz-seerr-m2-redis}
SEERR_CONTAINER=${SEERR_TEST_SEERR_CONTAINER:-rawrz-seerr-m2}
NETWORK=${SEERR_TEST_NETWORK:-rawrz-seerr-m2}
PORT=${SEERR_TEST_PORT:-5056}
PREFIX=${SEERR_CACHE_REDIS_PREFIX:-rawrz:seerr:m2}

cleanup() {
  docker rm -f "${SEERR_CONTAINER}" "${REDIS_CONTAINER}" >/dev/null 2>&1 || true
  docker network rm "${NETWORK}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

if ! docker image inspect "${IMAGE}" >/dev/null 2>&1; then
  echo "Prototype image not found: ${IMAGE}; run ${ROOT}/services/seerr/build-prototype.sh first" >&2
  exit 1
fi

cleanup
docker network create "${NETWORK}" >/dev/null
docker run -d --name "${REDIS_CONTAINER}" --network "${NETWORK}" redis:7-alpine >/dev/null
for _ in $(seq 1 20); do
  if docker exec "${REDIS_CONTAINER}" redis-cli ping 2>/dev/null | grep -q PONG; then break; fi
  sleep 1
done
test "$(docker exec "${REDIS_CONTAINER}" redis-cli ping)" = PONG

docker run -d --name "${SEERR_CONTAINER}" --network "${NETWORK}" -p "${PORT}:5055" \
  -e NODE_ENV=production \
  -e PORT=5055 \
  -e SEERR_CACHE_BACKEND=redis \
  -e REDIS_URL=redis://${REDIS_CONTAINER}:6379/0 \
  -e SEERR_CACHE_REDIS_PREFIX="${PREFIX}" \
  -e SEERR_CACHE_REDIS_TIMEOUT_MS=250 \
  "${IMAGE}" >/dev/null

for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:${PORT}/api/v1/status" >/tmp/rawrz-seerr-status.json; then break; fi
  sleep 2
done
test -s /tmp/rawrz-seerr-status.json

docker exec "${REDIS_CONTAINER}" redis-cli SET "${PREFIX}:tmdb:acceptance" '{"v":1,"c":"tmdb","d":{"ok":true}}' EX 60 >/dev/null
test "$(docker exec "${REDIS_CONTAINER}" redis-cli TTL "${PREFIX}:tmdb:acceptance")" -gt 0

test "$(docker exec "${REDIS_CONTAINER}" redis-cli GET "${PREFIX}:tmdb:acceptance")" = '{"v":1,"c":"tmdb","d":{"ok":true}}'
docker exec "${REDIS_CONTAINER}" redis-cli SET "rawrz:deck:queue:foreign" keep >/dev/null
docker exec "${REDIS_CONTAINER}" redis-cli SCAN 0 MATCH "${PREFIX}:tmdb:*" COUNT 100 >/dev/null

docker rm -f "${SEERR_CONTAINER}" >/dev/null
docker run -d --name "${SEERR_CONTAINER}" --network "${NETWORK}" -p "${PORT}:5055" \
  -e NODE_ENV=production -e PORT=5055 -e SEERR_CACHE_BACKEND=redis \
  -e REDIS_URL=redis://${REDIS_CONTAINER}:6379/0 -e SEERR_CACHE_REDIS_PREFIX="${PREFIX}" \
  "${IMAGE}" >/dev/null
for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:${PORT}/api/v1/status" >/dev/null; then break; fi
  sleep 2
done

test "$(docker exec "${REDIS_CONTAINER}" redis-cli GET "${PREFIX}:tmdb:acceptance")" = '{"v":1,"c":"tmdb","d":{"ok":true}}'
test "$(docker exec "${REDIS_CONTAINER}" redis-cli GET rawrz:deck:queue:foreign)" = keep

docker rm -f "${REDIS_CONTAINER}" >/dev/null
docker exec "${SEERR_CONTAINER}" sh -c 'wget -qO- http://127.0.0.1:5055/api/v1/status' >/tmp/status-after-redis-loss.json
test -s /tmp/status-after-redis-loss.json
printf 'M2 Seerr prototype acceptance passed (%s)\n' "${IMAGE}"
