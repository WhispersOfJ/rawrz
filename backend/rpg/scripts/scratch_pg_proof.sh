#!/usr/bin/env bash
# Scratch-Postgres proof harness for the RPG persistence layer.
# Spins up a disposable postgres container, runs the env-gated integration
# test (backend/rpg/tests/store_proof.rs) against it, and always removes the
# container. Idempotent: safe to re-run.
#
# Usage: scripts/scratch_pg_proof.sh
set -euo pipefail

CONTAINER=movie-rpg-scratch-pg
PORT=54329
DB=rpg_scratch
USER_NAME=rpg
PASSWORD=scratch
URL="postgresql://${USER_NAME}:${PASSWORD}@127.0.0.1:${PORT}/${DB}"

cleanup() {
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Fresh start: remove any leftover container from an interrupted run.
docker rm -f "$CONTAINER" >/dev/null 2>&1 || true

echo "==> starting disposable postgres on 127.0.0.1:${PORT}"
docker run -d --name "$CONTAINER" \
  -e POSTGRES_DB="$DB" -e POSTGRES_USER="$USER_NAME" -e POSTGRES_PASSWORD="$PASSWORD" \
  -p 127.0.0.1:${PORT}:5432 \
  postgres:16-alpine >/dev/null

echo "==> waiting for postgres to accept connections"
# The image initializes with a temporary server that then restarts, so
# readiness is only trusted after it holds across a second confirmation pass.
wait_ready() {
  for _ in $(seq 1 120); do
    if docker exec "$CONTAINER" pg_isready -U "$USER_NAME" -d "$DB" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}
wait_ready
docker exec "$CONTAINER" pg_isready -U "$USER_NAME" -d "$DB" >/dev/null
sleep 1
wait_ready
docker exec "$CONTAINER" pg_isready -U "$USER_NAME" -d "$DB" >/dev/null

echo "==> running the store proof against scratch postgres"
cd "$(dirname "$0")/../backend/rpg"
RPG_DB_URL="$URL" cargo test --test store_proof -- --nocapture
