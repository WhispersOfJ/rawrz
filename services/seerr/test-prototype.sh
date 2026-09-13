#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
IMAGE=${SEERR_IMAGE:-rawrz-seerr:m2}
REDIS=${SEERR_TEST_REDIS_CONTAINER:-rawrz-seerr-m2-redis}
FIXTURE=${SEERR_TEST_FIXTURE_CONTAINER:-rawrz-seerr-m2-fixture}
RUNNER=${SEERR_TEST_RUNNER_CONTAINER:-rawrz-seerr-m2-runner}
APP=${SEERR_TEST_APP_CONTAINER:-rawrz-seerr-m2-app}
NETWORK=${SEERR_TEST_NETWORK:-rawrz-seerr-m2}
PREFIX=${SEERR_CACHE_REDIS_PREFIX:-rawrz:seerr:m2}
API_KEY=${SEERR_TEST_API_KEY:-m2-acceptance-key}
STATE="${ROOT}/.cache/seerr-m2-acceptance-${BASHPID}-${RANDOM}"

cleanup() {
  docker rm -f "$APP" "$RUNNER" "$REDIS" "$FIXTURE" >/dev/null 2>&1 || true
  docker network rm "$NETWORK" >/dev/null 2>&1 || true
  rm -rf "$STATE" 2>/dev/null || true
}
trap cleanup EXIT

docker image inspect "$IMAGE" >/dev/null 2>&1 || {
  echo "Prototype image not found: $IMAGE; run $ROOT/services/seerr/build-prototype.sh first" >&2
  exit 1
}
cleanup
mkdir -p "$STATE/config"
chmod 777 "$STATE" "$STATE/config"
docker network create "$NETWORK" >/dev/null
docker run -d --name "$REDIS" --network "$NETWORK" redis:7-alpine >/dev/null
docker run -d --name "$FIXTURE" --network "$NETWORK" "$IMAGE" node -e '
  const http = require("http"); let hits = 0; let falseHits = 0;
  http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/value") return res.end(JSON.stringify({ value: `fixture-${++hits}` }));
    if (req.url === "/false") { falseHits++; return res.end("false"); }
    if (req.url === "/false-count") return res.end(JSON.stringify({ hits: falseHits }));
    if (req.url === "/count") return res.end(JSON.stringify({ hits }));
    res.statusCode = 404; res.end();
  }).listen(8080);
' >/dev/null
for _ in $(seq 1 20); do docker exec "$REDIS" redis-cli ping 2>/dev/null | grep -q PONG && break; sleep 1; done
test "$(docker exec "$REDIS" redis-cli ping)" = PONG
docker exec "$REDIS" redis-cli SET rawrz:deck:queue:foreign keep >/dev/null

# Seed a disposable Seerr database so the real authenticated route can run.
docker run --rm --network "$NETWORK" -e CONFIG_DIRECTORY=/config -e NODE_ENV=production -e API_KEY="$API_KEY" \
  -v "$STATE/config:/config" "$IMAGE" node -e 'const ds=require("/app/dist/datasource").default,{User}=require("/app/dist/entity/User"),{UserType}=require("/app/dist/constants/user"),{getSettings}=require("/app/dist/lib/settings");(async()=>{await ds.initialize();await ds.runMigrations();const u=new User({email:"admin@rawrz.test",userType:UserType.LOCAL,permissions:2,avatar:""});await u.setPassword("m2-test-password");await ds.getRepository(User).save(u);await getSettings().load();await getSettings().save();await ds.destroy()})().catch(e=>{console.error(e);process.exit(1)})'

docker run -d --name "$APP" --network "$NETWORK" -e CONFIG_DIRECTORY=/config -e NODE_ENV=production -e API_KEY="$API_KEY" \
  -e SEERR_CACHE_BACKEND=redis -e REDIS_URL="redis://$REDIS:6379/0" -e SEERR_CACHE_REDIS_PREFIX="$PREFIX" \
  -e SEERR_CACHE_REDIS_PROBE_MS=500 -v "$STATE/config:/config" "$IMAGE" >/dev/null
for _ in $(seq 1 120); do docker exec "$APP" wget -qO- 'http://127.0.0.1:5055/api/v1/status?checkUpdateAvailable=false' >/dev/null 2>&1 && break; sleep 1; done
docker exec "$APP" wget -qO- 'http://127.0.0.1:5055/api/v1/status?checkUpdateAvailable=false' >/dev/null
docker exec "$APP" wget -qO- --header="X-API-Key: $API_KEY" 'http://127.0.0.1:5055/api/v1/settings/cache' | grep -q apiCaches

# Seed the exact TMDB cache envelope used by getMovie, then exercise the product route.
ROUTE_RAW_KEY='https://api.themoviedb.org/3/movie/550{"language":"en","append_to_response":"credits,external_ids,videos,keywords,release_dates,watch/providers","include_video_language":"en"}'
ROUTE_HASH=$(printf '%s' "$ROUTE_RAW_KEY" | sha256sum | cut -d' ' -f1)
ROUTE_KEY="$PREFIX:tmdb:$ROUTE_HASH"
ROUTE_VALUE='{"v":1,"c":"tmdb","d":{"id":550,"adult":false,"budget":0,"genres":[],"videos":{"results":[]},"original_language":"en","original_title":"Fixture Movie","popularity":1,"production_companies":[],"production_countries":[],"release_date":"2026-01-01","release_dates":{"results":[]},"revenue":0,"spoken_languages":[],"status":"Released","title":"Fixture Movie","video":false,"vote_average":0,"vote_count":0,"backdrop_path":null,"homepage":null,"imdb_id":null,"overview":"controlled fixture","poster_path":null,"runtime":90,"tagline":null,"credits":{"cast":[],"crew":[]},"belongs_to_collection":null,"external_ids":{},"watch/providers":{"results":{}},"keywords":{"keywords":[]}}}'
docker exec "$REDIS" redis-cli SET "$ROUTE_KEY" "$ROUTE_VALUE" EX 60 >/dev/null
MOVIE_FIRST=$(docker exec "$APP" wget -qO- --header="X-API-Key: $API_KEY" 'http://127.0.0.1:5055/api/v1/movie/550?language=en')
printf '%s' "$MOVIE_FIRST" | grep -q '"id":550'
printf '%s' "$MOVIE_FIRST" | grep -q 'controlled fixture'
MOVIE_SECOND=$(docker exec "$APP" wget -qO- --header="X-API-Key: $API_KEY" 'http://127.0.0.1:5055/api/v1/movie/550?language=en')
test "$MOVIE_FIRST" = "$MOVIE_SECOND"
test "$(docker exec "$REDIS" redis-cli EXISTS "$ROUTE_KEY")" = 1
docker exec "$APP" wget -qO- --post-data='' --header="X-API-Key: $API_KEY" "http://127.0.0.1:5055/api/v1/settings/cache/tmdb/flush" >/dev/null
test "$(docker exec "$REDIS" redis-cli EXISTS "$ROUTE_KEY")" = 0
test "$(docker exec "$REDIS" redis-cli GET rawrz:deck:queue:foreign)" = keep
docker rm -f "$APP" >/dev/null

cat >"$STATE/runner.js" <<'NODE'
const fs = require('fs'); const http = require('http');
const ExternalAPI = require('/app/dist/api/externalapi').default;
const { createCacheBackend } = require('/app/dist/lib/cache/backend');
const host = process.env.SEERR_TEST_FIXTURE_HOST;
const signal = n => fs.closeSync(fs.openSync(`/state/${n}`, 'w'));
const waitFor = n => new Promise(resolve => { const check = () => fs.existsSync(`/state/${n}`) ? resolve() : setTimeout(check, 100); check(); });
const request = path => new Promise((resolve, reject) => http.get(`http://${host}:8080${path}`, res => { let body=''; res.setEncoding('utf8'); res.on('data', c => body += c); res.on('end', () => resolve(JSON.parse(body))); }).on('error', reject));
const assert = (ok, message) => { if (!ok) throw new Error(message); };
class FixtureAPI extends ExternalAPI { constructor(cache) { super(`http://${host}:8080`, {}, { cache }); } value(path='/value') { return this.get(path, undefined, 60); } }
(async () => {
  const cache = createCacheBackend('tmdb', 60, 1), api = new FixtureAPI(cache);
  const first = await api.value(), second = await api.value();
  assert(first.value === second.value && (await request('/count')).hits === 1, 'miss/hit contract failed');
  const falseFirst = await api.value('/false'), falseSecond = await api.value('/false');
  assert(falseFirst === false && falseSecond === false && (await request('/false-count')).hits === 1, 'falsy contract failed');
  const edge = createCacheBackend('edge', 60, 1);
  for (const [key, value, ttl] of [['zero', false, 0], ['persistent', true, -1]]) await edge.set(key, value, ttl);
  for (const [key, value] of [['zero', false], ['persistent', true]]) assert(await edge.get(key) === value, `TTL contract failed for ${key}`);
  assert(await edge.getTtlMs('persistent') === 0, 'persistent TTL contract failed'); await edge.flush();
  await cache.flush(); const afterFlush = await api.value();
  assert(afterFlush.value !== second.value && (await request('/count')).hits === 2, 'flush contract failed'); signal('ready');
  await waitFor('outage'); const fallback = await api.value(), degraded = cache.stats();
  assert(fallback.value && degraded.backend === 'memory' && degraded.degraded, 'fallback contract failed'); signal('fallback');
  await waitFor('resume');
  for (let i=0; i<60 && (cache.stats().backend !== 'redis' || cache.stats().degraded); i++) await new Promise(r => setTimeout(r, 250));
  assert(cache.stats().backend === 'redis' && !cache.stats().degraded, 'recovery contract failed');
  const recoveredApi = new FixtureAPI(createCacheBackend('tmdb', 60, 1));
  const live = await recoveredApi.value(), hit = await recoveredApi.value();
  assert(live.value === hit.value && (await request('/count')).hits === 4, 'post-recovery contract failed'); signal('done');
  console.log('Seerr cache route/backend contract passed');
})().catch(error => { console.error(error); process.exit(1); });
NODE

docker run -d --name "$RUNNER" --network "$NETWORK" -e SEERR_CACHE_BACKEND=redis -e REDIS_URL="redis://$REDIS:6379/0" \
  -e SEERR_CACHE_REDIS_PREFIX="$PREFIX" -e SEERR_CACHE_REDIS_PROBE_MS=500 -e SEERR_TEST_FIXTURE_HOST="$FIXTURE" \
  -v "$STATE:/state" -u 0 -v "$STATE/runner.js:/runner.js:ro" "$IMAGE" node /runner.js >/dev/null
for _ in $(seq 1 120); do test -f "$STATE/ready" && break; sleep 0.25; done
test -f "$STATE/ready"
KEY=$(docker exec "$REDIS" redis-cli --scan --pattern "$PREFIX:tmdb:*"); test -n "$KEY"; test "$(printf '%s\n' "$KEY" | wc -l)" -eq 1
test "$(docker exec "$REDIS" redis-cli TTL "$KEY")" -gt 0; test "$(docker exec "$REDIS" redis-cli GET rawrz:deck:queue:foreign)" = keep

docker rm -f "$REDIS" >/dev/null; touch "$STATE/outage"
for _ in $(seq 1 120); do test -f "$STATE/fallback" && break; sleep 0.25; done
test -f "$STATE/fallback"
docker run -d --name "$REDIS" --network "$NETWORK" redis:7-alpine >/dev/null
for _ in $(seq 1 20); do docker exec "$REDIS" redis-cli ping 2>/dev/null | grep -q PONG && break; sleep 1; done
test "$(docker exec "$REDIS" redis-cli ping)" = PONG
docker exec "$REDIS" redis-cli SET rawrz:deck:queue:foreign keep >/dev/null
touch "$STATE/resume"
for _ in $(seq 1 120); do test -f "$STATE/done" && break; sleep 0.25; done
test -f "$STATE/done"
KEY=$(docker exec "$REDIS" redis-cli --scan --pattern "$PREFIX:tmdb:*"); test -n "$KEY"
test "$(docker exec "$REDIS" redis-cli GET rawrz:deck:queue:foreign)" = keep
docker logs "$RUNNER"
printf 'M2 Seerr prototype acceptance passed (%s)\n' "$IMAGE"
