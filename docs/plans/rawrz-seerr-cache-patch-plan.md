# RAWRZ — Seerr CacheBackend: interface + concrete patch plan

> **Status:** Draft v0.1 — 2026-09-11. Companion to `rawrz-megastack-spec.md` §5.
> **Target:** Seerr **v3.4.1** (`ghcr.io/seerr-team/seerr:v3.4.1`), pinned source at `~/TRUTH/seerr`.
> **Decision it implements:** D42 / R-4 — *async `CacheBackend` patch*, no L1 mirror.
> **No upstream file has been modified.** This document is a plan plus the exact diffs to apply
> on a fork branch. `~/TRUTH/seerr` is a read-only reference corpus.

---

## 1. Objective and non-goals

**Objective.** Give Seerr a Redis-backed cache so its upstream API payloads (TMDb, TVDB, IMDb,
Rotten Tomatoes, GitHub, Plex GUID/TV, Plex Watchlist, Radarr, Sonarr) survive restarts, are
shared/observable inside RAWRZ, and are governed by one TTL policy — without changing Seerr's
behavior when Redis is absent.

**Non-goals.**

- Not a rewrite of Seerr's caching semantics. TTLs, cache ids, cache names, and cache-flush
  behavior stay exactly as upstream.
- Not a multi-replica correctness project. The stack runs one Seerr instance; sharing exists
  because RAWRZ's own components and restarts benefit, not because of horizontal scale.
- Not a second image-proxy cache. nginx `proxy_cache` (§6.4 of the master spec) handles image
  bytes; this patch handles the API cache Seerr already keeps.
- Not a place for sessions, tokens, or user data (see §9, rule R3).

---

## 2. Verified current state (the real call-site census)

Master-spec numbers were approximate. The precise census from `~/TRUTH/seerr` v3.4.1:

| Metric | Count |
|---|---|
| Files that must change | **11** |
| Consumer call sites | **14** — 10 × `cacheManager.getCache(...)` + 4 × direct `.data` accesses |
| Cache API touchpoints inside `externalapi.ts` | **8**, across **4** methods (`get`, `post`, `getRolling`, `removeCache`) |
| Touchpoints inside `cache.ts` | **3** (construction, `getStats`, `flush`) |
| Distinct sync cache methods required | `get`, `set`, `getTtl`, `del`, `flushAll`, `getStats` |

The master spec accounted for `ExternalAPI.get()` only. `post()`, `getRolling()`, and
`removeCache()` also touch the cache, and `getRolling()` additionally needs **`getTtl`** — a
method missing from the master spec's interface sketch. Both corrections are applied to the
master spec (§5.2) as part of this plan.

### 2.1 The wrapper (`server/lib/cache.ts`, 91 lines)

```ts
class Cache {
  public id: AvailableCacheIds;
  public data: NodeCache;               // ← the raw instance every call site reaches into
  public name: string;
  constructor(id, name, options: { stdTtl?: number; checkPeriod?: number } = {}) {
    this.data = new NodeCache({ stdTTL: options.stdTtl ?? DEFAULT_TTL, checkperiod: ... });
  }
  public getStats() { return this.data.getStats(); }
  public flush(): void { this.data.flushAll(); }
}
class CacheManager {
  private availableCaches: Record<AvailableCacheIds, Cache> = { tmdb, radarr, sonarr, rt,
    imdb, github, plexguid, plextv, plexwatchlist, tvdb };
  public getCache(id) { return this.availableCaches[id]; }
  public getAllCaches() { return this.availableCaches; }
}
export default new CacheManager()   // singleton
```

Verified TTLs to preserve byte-for-byte:

| Cache id | Name | `stdTtl` | `checkPeriod` |
|---|---|---|---|
| `tmdb` | The Movie Database API | 21600 | 1800 |
| `rt` | Rotten Tomatoes API | 43200 | 1800 |
| `imdb` | IMDB Radarr Proxy | 43200 | 1800 |
| `github` | GitHub API | 21600 | 1800 |
| `plexguid` | Plex GUID | 604800 | 1800 |
| `plextv` | Plex TV | 604800 | 60 |
| `tvdb` | The TVDB API | 21600 | 1800 |
| `radarr`, `sonarr`, `plexwatchlist` | — | 300 (`DEFAULT_TTL`) | 120 (`DEFAULT_CHECK_PERIOD`) |

### 2.2 The consumer (`server/api/externalapi.ts`)

`ExternalAPI` is Seerr's own in-repo class. `protected async get<T>()` is already async, so the
"sync barrier" that motivated the rejected L1-mirror design does not exist. All four methods
follow the same shape: serialize a key → sync cache read → on miss, HTTP → sync cache write.

```ts
private cache?: NodeCache;                                   // :26
this.cache = options.nodeCache;                              // :53
const cachedItem = this.cache?.get<T>(cacheKey);             // :65  (get)
if (this.cache && ttl !== 0) this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);  // :72-74
const keyTtl = this.cache?.getTtl(cacheKey) ?? 0;            // :115 (getRolling — extra method)
this.cache?.del(cacheKey);                                   // :147 (removeCache)
```

Semantics that must be preserved exactly (they are behavior, not bugs):

- **Truthiness as hit-test.** `if (cachedItem)` means a cached `0`, `''`, or `false` is treated
  as a **miss**. The Redis backend must not "fix" this.
- **`ttl === 0` bypasses caching** for reads and writes (`get`, `post`, `getRolling`).
- **`post()` is cacheable** when a caller passes a non-zero `ttl`; `serializeCacheKey` for
  `post` includes `config.params` + `data`, so keys stay distinct.
- **`getRolling()` uses `getTtl` in milliseconds** and refreshes in the background when the
  remaining TTL crosses `ttl*1000` minus a 10-second rolling buffer.
- **`removeCache()` has no callers in the pinned source**, but it is public API on the class:
  keep it working (`del`).
- Cache keys are `baseUrl + endpoint + JSON.stringify(options)` — long, whitespace-heavy, and
  occasionally credential-adjacent (query params). They must be **hashed** for Redis (see §5.3).

### 2.3 The seven handoffs

Every one passes the raw instance into `ExternalAPI`:

| File | Line | Expression |
|---|---|---|
| `server/api/github.ts` | 74 | `nodeCache: cacheManager.getCache('github').data` |
| `server/api/plextv.ts` | 151 | `nodeCache: cacheManager.getCache('plextv').data` |
| `server/api/rating/imdbRadarrProxy.ts` | 167 | `nodeCache: cacheManager.getCache('imdb').data` |
| `server/api/rating/rottentomatoes.ts` | 119 | `nodeCache: cacheManager.getCache('rt').data` |
| `server/api/servarr/base.ts` | 109 | `nodeCache: cacheManager.getCache(cacheName).data` |
| `server/api/themoviedb/index.ts` | 141 | `nodeCache: cacheManager.getCache('tmdb').data` |
| `server/api/tvdb/index.ts` | 56 | `nodeCache: cacheManager.getCache(finalConfig.cachePrefix).data` |

### 2.4 The four direct accesses

| File | Line | Expression | Enclosing context (verified async) |
|---|---|---|---|
| `server/api/plextv.ts` | 282 | `watchlistCache.data.get<PlexWatchlistCache>(this.authToken)` | `public async getWatchlist()` |
| `server/api/plextv.ts` | 308 | `watchlistCache.data.set<PlexWatchlistCache>(this.authToken, cachedWatchlist)` | same |
| `server/lib/scanners/plex/index.ts` | 388 | `guidCache.data.get<MediaIds>(plexitem.ratingKey)` | `private async getMediaIds()` |
| `server/lib/scanners/plex/index.ts` | 439 | `guidCache.data.set(plexitem.ratingKey, mediaIds)` | same |

### 2.5 The settings route

```ts
settingsRoutes.post<{ cacheId: AvailableCacheIds }>('/cache/:cacheId/flush',
  (req, res, next) => {                       // :780 — needs to become async
    const cache = cacheManager.getCache(req.params.cacheId);   // :784
    if (cache) { cache.flush(); return res.status(204).send(); } // :787 — needs await
    next({ status: 404, message: 'Cache not found.' });
  });
```

There are **no tests** for the cache in the pinned tree (`find` for `*cache*test*` → nothing),
so this patch also *adds* the first cache test suite (§10).

---

## 3. Interface design

### 3.1 The interface

```ts
// server/lib/cache/backend.ts  (new file)
export interface CacheStats {
  keys: number;
  hits: number;
  misses: number;
  ksize: number;   // bytes, best effort
  vsize: number;   // bytes, best effort
}

export interface CacheBackend {
  readonly kind: 'memory' | 'redis';
  readonly cacheId: AvailableCacheIds;

  /** Truthiness semantics of node-cache are preserved: falsy stored values are misses. */
  get<T>(key: string): Promise<T | undefined>;
  set<T>(key: string, value: T, ttlSeconds: number): Promise<void>;
  del(key: string): Promise<void>;

  /** Milliseconds remaining, or undefined when absent/no TTL — node-cache getTtl parity. */
  getTtlMs(key: string): Promise<number | undefined>;

  /** Purges this backend's namespace only. Never FLUSHALL. */
  flush(): Promise<void>;

  stats(): CacheStats;
  ready(): boolean;
  close(): Promise<void>;
}
```

Design notes:

- **Fully async**, including `getTtlMs` and `flush`. Every caller is already async, so this keeps
  one programming model rather than mixing sync and async surfaces.
- **`stats()` stays synchronous.** It is read from process-local counters, so a stats read never
  waits on Redis and the `/settings/cache` route keeps a fast path.
- `getTtlMs` replaces node-cache's millisecond `getTtl`, which `getRolling()` needs.
- No `keys()`/`mget`/`mset`: nothing in the tree calls them, and adding unused surface widens the
  upstream diff.

### 3.2 Key namespacing and envelope

The backend owns namespacing; call sites keep passing the raw serialized key.

```
key   : rawrz:seerr:cache:<cacheId>:<sha256(serializedKey)>       (64 hex chars)
value : {"v":1,"c":"tmdb","k":"<serializedKey>","t":"<ISO8601>","d":<payload>}
```

- **Hash the key**: raw keys are long, contain JSON/whitespace, and can include request params.
  Hashing bounds key length and makes `SCAN`-based tooling predictable. The original key is kept
  inside the envelope (compressed by Redis only if it dominates; measure at implementation) so
  operators can attribute entries without a lookup table.
- **TTL is set via `SET ... EX <ttl>`** (seconds, matching upstream's `stdTtl` units). `getTtlMs`
  uses `PTTL`.
- **Envelope version `v`** allows a format change without a migration: unknown `v` is treated as
  a miss (which is safe — a miss means one extra upstream fetch).
- **No secrets in keys.** The envelope stores the original key, which for `servarr` includes
  `apikey` in `params`? — verified: `servarr/base.ts` passes `{ apikey: apiKey }` as axios
  *params*, and `serializeCacheKey` for `get` includes `config.params` **only when the caller
  supplies them**, not the constructor params. Still, rule R3 (§9) requires the implementation to
  assert that no `apikey`/`token` substring appears in a serialized key during tests. If a hit is
  found, store a hash-only envelope (`k` omitted) — the observability benefit is not worth a
  credential in Redis.

### 3.3 Backend selection and lifecycle

```
server/lib/cache/runtime.ts     CacheRuntime  — owns the shared ioredis client, health, degradation
server/lib/cache/memory.ts      MemoryCacheBackend
server/lib/cache/redis.ts       RedisCacheBackend
server/lib/cache/backend.ts     CacheBackend, CacheStats, createBackend()
server/lib/cache.ts             Cache + CacheManager (modified in place)
```

`createBackend(cacheId, opts)` reads:

| Env | Default | Meaning |
|---|---|---|
| `SEERR_CACHE_BACKEND` | `memory` | `memory` or `redis`. Anything else → `memory` + one warning. |
| `REDIS_URL` | unset | e.g. `redis://rawrz-redis:6379/0`. Unset + `redis` → degrade to memory. |
| `SEERR_CACHE_REDIS_PREFIX` | `rawrz:seerr:cache` | Namespace root, for test isolation. |
| `SEERR_CACHE_REDIS_TIMEOUT_MS` | `2000` | Connect/command timeout. |

`CacheRuntime` responsibilities:

- Lazy-connect a single ioredis client (`lazyConnect: true`, `enableOfflineQueue: false`,
  `maxRetriesPerRequest: 1`, `retryStrategy` with capped exponential backoff) — fail fast so a
  sick Redis degrades instead of queueing every cache read.
- **Degrade on failure:** on connect failure or command error, log once, flip the metric,
  swap every `Cache.backend` to its memory implementation, and keep serving. Re-probe with
  `PING` every 30s; on success, log once and swap back. Swap is atomic per cache (backends are
  reference-swapped, and an in-flight promise resolves against whichever backend it started on).
- **Do not flush memory on swap.** Entries written while degraded stay in memory only; entries
  written while healthy are already in Redis. No dual-write (that was the rejected L1 design).
- Expose `kind` per cache for the settings/health surfaces.

### 3.4 `cache.ts` modifications

```ts
import type { CacheBackend } from '@server/lib/cache/backend';
import { createBackend } from '@server/lib/cache/backend';

class Cache {
  public readonly id: AvailableCacheIds;
  public readonly name: string;
  private backend: CacheBackend;

  constructor(id, name, options: { stdTtl?: number; checkPeriod?: number } = {}) {
    this.backend = createBackend(id, options);
  }

  /** Internal seam for CacheRuntime degradation swaps. */
  setBackend(backend: CacheBackend) { this.backend = backend; }

  public getStats() { return this.backend.stats(); }
  public flush(): Promise<void> { return this.backend.flush(); }
}
```

- **`data` is removed.** It is the entire reason for the patch; keeping it would let new code
  reach the raw `NodeCache` again. The compiler then finds every remaining site — a useful
  migration aid.
- `getAllCaches()` is retained (used by the settings stats route).
- `Cache` construction stays eager, so startup behavior is unchanged when `SEERR_CACHE_BACKEND=memory`.

---

## 4. Exact patch plan, file by file

Applied on a fork branch off the pinned tag; kept as one squashed commit (and as a `.patch`
artifact in RAWRZ, §11).

### 4.1 `server/lib/cache.ts` — rewrite of the wrapper

```diff
-import NodeCache from 'node-cache';
+import type { CacheBackend, CacheStats } from '@server/lib/cache/backend';
+import { createBackend } from '@server/lib/cache/backend';

@@ class Cache
-  public id: AvailableCacheIds;
-  public data: NodeCache;
-  public name: string;
+  public readonly id: AvailableCacheIds;
+  public readonly name: string;
+  private backend: CacheBackend;

   constructor(id, name, options = {}) {
     this.id = id;
     this.name = name;
-    this.data = new NodeCache({ stdTTL: ..., checkperiod: ... });
+    this.backend = createBackend(id, options);
   }

-  public getStats() { return this.data.getStats(); }
-  public flush(): void { this.data.flushAll(); }
+  public setBackend(backend: CacheBackend): void { this.backend = backend; }
+  public getStats(): CacheStats { return this.backend.stats(); }
+  public flush(): Promise<void> { return this.backend.flush(); }
```

`DEFAULT_TTL` / `DEFAULT_CHECK_PERIOD` move to `memory.ts` as defaults but stay re-exported from
`cache.ts` so `cachePrefix` typing and any external import keep resolving.

### 4.2 `server/api/externalapi.ts` — the core change

```diff
-import type NodeCache from 'node-cache';
+import type { CacheBackend } from '@server/lib/cache/backend';

 export interface ExternalAPIOptions {
-  nodeCache?: NodeCache;
+  cache?: CacheBackend;
   headers?: Record<string, unknown>;
   timeout?: number;
   rateLimit?: { maxRPS: number; maxRequests: number };
 }

-  private cache?: NodeCache;
+  private cache?: CacheBackend;
@@ constructor
-    this.cache = options.nodeCache;
+    this.cache = options.cache;

@@ get()
-    const cachedItem = this.cache?.get<T>(cacheKey);
+    const cachedItem = await this.cache?.get<T>(cacheKey);
     if (cachedItem) return cachedItem;
     const response = await this.axios.get<T>(endpoint, config);
     if (this.cache && ttl !== 0) {
-      this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);
+      await this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);
     }

@@ post()          same two-line change (await get / await set)
@@ getRolling()
-    const cachedItem = this.cache?.get<T>(cacheKey);
+    const cachedItem = await this.cache?.get<T>(cacheKey);
     if (cachedItem) {
-      const keyTtl = this.cache?.getTtl(cacheKey) ?? 0;
+      const keyTtl = (await this.cache?.getTtlMs(cacheKey)) ?? 0;
       if (keyTtl - (ttl ?? DEFAULT_TTL) * 1000 < Date.now() - DEFAULT_ROLLING_BUFFER) {
-        this.axios.get<T>(endpoint, config).then((response) => {
-          this.cache?.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);
-        });
+        void this.axios.get<T>(endpoint, config).then((response) =>
+          this.cache?.set(cacheKey, response.data, ttl ?? DEFAULT_TTL)
+        ).catch(() => undefined);   // background refresh stays fire-and-forget
       }
       return cachedItem;
     }

@@ removeCache()
-    this.cache?.del(cacheKey);
+    await this.cache?.del(cacheKey);
```

Two deliberate details: (a) the background refresh keeps fire-and-forget semantics but now
catches, so an unhandled rejection cannot crash the process when Redis is unhealthy; (b)
`removeCache` becomes async — it has no callers in the pinned tree, so there is no caller churn.

### 4.3 The seven handoffs (one-line each)

```diff
-- github.ts:74        nodeCache: cacheManager.getCache('github').data,
++ github.ts:74        cache: cacheManager.getCache('github').backend,
-- plextv.ts:151       nodeCache: cacheManager.getCache('plextv').data,
++ plextv.ts:151       cache: cacheManager.getCache('plextv').backend,
-- imdbRadarrProxy.ts:167  nodeCache: cacheManager.getCache('imdb').data,
++ imdbRadarrProxy.ts:167  cache: cacheManager.getCache('imdb').backend,
-- rottentomatoes.ts:119   nodeCache: cacheManager.getCache('rt').data,
++ rottentomatoes.ts:119   cache: cacheManager.getCache('rt').backend,
-- servarr/base.ts:109     nodeCache: cacheManager.getCache(cacheName).data,
++ servarr/base.ts:109     cache: cacheManager.getCache(cacheName).backend,
-- themoviedb/index.ts:141 nodeCache: cacheManager.getCache('tmdb').data,
++ themoviedb/index.ts:141 cache: cacheManager.getCache('tmdb').backend,
-- tvdb/index.ts:56        nodeCache: cacheManager.getCache(finalConfig.cachePrefix).data,
++ tvdb/index.ts:56        cache: cacheManager.getCache(finalConfig.cachePrefix).backend,
```

`backend` must be public on `Cache` for this to work (or expose a `getBackend()` accessor — prefer
the accessor if the fork wants `setBackend` to stay the only mutator).

### 4.4 `server/api/plextv.ts` — watchlist cache

```diff
       const watchlistCache = cacheManager.getCache('plexwatchlist');
-      let cachedWatchlist = watchlistCache.data.get<PlexWatchlistCache>(this.authToken);
+      let cachedWatchlist = await watchlistCache.backend.get<PlexWatchlistCache>(this.authToken);
       ...
-        watchlistCache.data.set<PlexWatchlistCache>(this.authToken, cachedWatchlist);
+        await watchlistCache.backend.set<PlexWatchlistCache>(this.authToken, cachedWatchlist, 300);
```

The explicit `300` preserves the cache's `stdTtl` (node-cache previously applied it implicitly).

### 4.5 `server/lib/scanners/plex/index.ts` — GUID cache

```diff
       const guidCache = cacheManager.getCache('plexguid');
-      const cachedGuids = guidCache.data.get<MediaIds>(plexitem.ratingKey);
+      const cachedGuids = await guidCache.backend.get<MediaIds>(plexitem.ratingKey);
       ...
-      guidCache.data.set(plexitem.ratingKey, mediaIds);
+      await guidCache.backend.set(plexitem.ratingKey, mediaIds, 604800);   // plexguid stdTtl
```

### 4.6 `server/routes/settings/index.ts` — flush + stats

```diff
 settingsRoutes.post<{ cacheId: AvailableCacheIds }>('/cache/:cacheId/flush',
-  (req, res, next) => {
+  async (req, res, next) => {
     const cache = cacheManager.getCache(req.params.cacheId);
     if (cache) {
-      cache.flush();
+      await cache.flush();
       return res.status(204).send();
     }
     next({ status: 404, message: 'Cache not found.' });
   });
```

`GET /settings/cache` (the stats route) is unchanged in shape because `getStats()` stays sync —
it now reports the active backend's counters, and should gain a `backend: 'memory' | 'redis'`
field so the UI can show degradation.

### 4.7 New files

| File | Contents |
|---|---|
| `server/lib/cache/backend.ts` | `CacheBackend`, `CacheStats`, `createBackend()`, key hashing + envelope codec |
| `server/lib/cache/memory.ts` | `MemoryCacheBackend` (wraps `node-cache`; preserves truthiness, `ttl===0` handled by callers) |
| `server/lib/cache/redis.ts` | `RedisCacheBackend` (ioredis; `SET ... EX`, `PTTL`, `SCAN`+`UNLINK` flush, counters) |
| `server/lib/cache/runtime.ts` | `CacheRuntime` — client, health, degradation/recovery, metrics |
| `server/lib/cache/__tests__/*` | contract tests (§10) |

`package.json`: add `ioredis` (runtime dep). `node-cache` stays (memory backend).

---

## 5. Redis backend specifics

| Concern | Implementation |
|---|---|
| Write | `SET <ns>:<cacheId>:<hash> <envelope> EX <ttl>`; `ttl <= 0` → no write (callers already skip `0`, but defend) |
| Read | `GET` → `JSON.parse` → `v !== 1` → miss; payload falsy → **miss** (truthiness parity) |
| TTL read | `PTTL` → `undefined` when `-1` (no TTL) or `-2` (absent) |
| Delete | `UNLINK` (non-blocking) |
| Flush | `SCAN MATCH <ns>:<cacheId>:* COUNT 500` + `UNLINK` in batches; never `KEYS`, never `FLUSHALL` |
| Stats | Local counters for hits/misses; `keys` via a bounded `SCAN` at most once per stats call; `ksize`/`vsize` best-effort from `MEMORY USAGE` sampling or omitted with zeros |
| Serialization | JSON (human-readable in `redis-cli`, matches Seerr's payloads); no msgpack, to keep operator debuggability |
| Client | One shared ioredis `lazyConnect: true`, `enableOfflineQueue: false`, `maxRetriesPerRequest: 1`, capped backoff |
| Errors | Any command error → counted, logged at most once per transition, backend marked unhealthy |
| Key safety | Keys are `sha256` hex; envelope keeps the original key only if it passes the no-secret assertion (§9 R3) |

---

## 6. Behavior contract (must-not-change list)

| # | Invariant | Test |
|---|---|---|
| B1 | With `SEERR_CACHE_BACKEND=memory`, behavior is byte-identical to upstream (same TTLs, same hit/miss semantics) | contract suite run against both backends |
| B2 | Falsy stored values are misses | store `0`, `''`, `false`, assert re-fetch |
| B3 | `ttl === 0` disables caching for `get`/`post`/`getRolling` | assert no write |
| B4 | `getRolling` background refresh still happens near expiry and never throws | fake timers + failing backend |
| B5 | Cache ids, names, and TTLs are unchanged | snapshot test of `getAllCaches()` |
| B6 | `flush()` affects only that cache id's namespace | seed two caches, flush one, assert the other survives |
| B7 | Redis failure degrades to memory and recovers | integration test with a stoppable container |
| B8 | No secret material in keys or envelope | key-shape assertion over recorded keys |
| B9 | Stats shape is compatible with `GET /settings/cache` | response schema test |

---

## 7. Configuration

`.env.template` (RAWRZ root) additions:

```dotenv
# --- RAWRZ: Seerr cache backend (patch) ---
# memory (default, upstream behavior) | redis
SEERR_CACHE_BACKEND=redis
SEERR_CACHE_REDIS_PREFIX=rawrz:seerr:cache
SEERR_CACHE_REDIS_TIMEOUT_MS=2000
# REDIS_URL is shared with the rest of RAWRZ
REDIS_URL=redis://rawrz-redis:6379/0
```

`docker-compose.yml` (`seerr` service): add `REDIS_URL`, `SEERR_CACHE_BACKEND`,
`SEERR_CACHE_REDIS_PREFIX`, `SEERR_CACHE_REDIS_TIMEOUT_MS`; keep `depends_on` **absent** for
Redis — Seerr must start and work with Redis dead (B7). Ordering is handled by fail-open, not by
startup dependencies.

---

## 8. Observability

Metrics (into the RAWRZ stats namespace, §4.7 of the master spec):

| Metric | Type | Use |
|---|---|---|
| `rawrz_seerr_cache_hits_total{cache}` | counter | hit ratio per cache id |
| `rawrz_seerr_cache_misses_total{cache}` | counter | miss ratio, upstream-call proxy |
| `rawrz_seerr_cache_errors_total{cache}` | counter | Redis command failures |
| `rawrz_seerr_cache_degraded` | gauge | 1 when running on memory |
| `rawrz_seerr_cache_keys{cache}` | gauge | key count / growth |
| `rawrz_seerr_cache_backend_info` | gauge | info, one-hot label |

Alerts: `degraded == 1` for >15m (warning), `errors_total` rate > 0 for 10m (warning), TMDb miss
ratio above a configured floor for 1h (info — indicates upstream instability or an eviction
storm).

---

## 9. Rules (non-negotiable)

- **R1 — Fail open.** No code path may throw because Redis is unavailable. Degrade, serve, log once.
- **R2 — Never `FLUSHALL`/`KEYS`.** Namespaced flushing only; RAWRZ shares this Redis with
  sessions, queues, streams, and other caches.
- **R3 — No credentials in cache.** Seerr's cache stores API payloads; the patch must not start
  storing sessions, tokens, user objects, or anything from `params.apikey`. Test B8 enforces it.
- **R4 — TTL parity.** No TTL may drift from the table in §2.1 without a deliberate, documented change.
- **R5 — Smallest diff.** No refactors, formatting churn, or unrelated lint fixes in the patch
  commit; every hunk must be justifiable to the upstream-sync workflow (§11).
- **R6 — No behavior change when disabled.** The memory backend is the default; `memory` must be
  indistinguishable from today.

---

## 10. Test plan

**Contract suite** (`server/lib/cache/__tests__/backend.contract.ts`) — one suite, run against
both backends:

1. set/get round-trip; miss on absent key.
2. falsy-value parity (B2).
3. TTL expiry with fake timers; `getTtlMs` accuracy and `undefined` cases.
4. `del` removes; `del` on missing key is a no-op.
5. `flush` is namespaced (B6).
6. `stats()` shape and monotonic counters (B9).
7. key hashing determinism + envelope version handling (unknown `v` → miss).
8. key-shape/no-secret assertion (B8).

**Redis integration** (CI service container, or a stopped/started container in the e2e tier):

9. degradation: kill Redis mid-suite → reads keep working (memory), `degraded` flips, no throw (B7).
10. recovery: restart Redis → `PING` re-probe re-enables Redis within one probe interval.
11. namespace isolation: seed `rawrz:seerr:cache:tmdb:*` and a foreign key
    (`rawrz:deck:queue:x`) → flush Seerr's namespace, assert the foreign key survives (R2).

**Route tests:** `POST /settings/cache/:cacheId/flush` → 204 and namespaced purge; unknown id → 404;
`GET /settings/cache` still returns the expected shape plus `backend`.

**Real-traffic acceptance (M1/M6):** enable `redis`, run for a day, confirm hit ratio > 0, Redis
keyspace shows the expected namespaces, and a Seerr restart reuses cached entries (proving restart
survival — the whole point).

---

## 11. Upstream sync strategy (feeds the M6 automated workflow)

- The patch lives on a fork branch `rawrz/redis-cache` tracking the pinned upstream tag, plus an
  exported artifact at `services/seerr/patches/rawrz-redis-cache.patch` in RAWRZ for review and
  for rebuilding the branch from scratch.
- The scheduled sync workflow (§5.5 of the master spec) fetches the new upstream tag, rebases the
  branch (or re-applies the `.patch`), runs typecheck + the contract suite, builds the image, and
  opens a PR.
- **Conflict playbook:** if a hunk conflicts in `externalapi.ts` or any handoff file, the
  resolution is mechanical — re-apply the same four transformations (`nodeCache` → `cache`,
  `.data` → `.backend`, sync → `await`, `getTtl` → `getTtlMs`). `cache.ts` conflicts are resolved
  by re-running the wrapper rewrite. Any conflict that requires rethinking the design is a signal
  to reconsider the fork, not to grow the diff.
- If upstream lands a pluggable cache backend, retire the fork and re-point the stack pin.

---

## 12. Rollout and rollback

| Step | Action | Rollback |
|---|---|---|
| 0 | Land the patch with `SEERR_CACHE_BACKEND=memory` (default) | none needed — no behavior change |
| 1 | Enable `redis` in a dev/second Seerr instance (M2) | set env back to `memory`, restart |
| 2 | Measure hit ratio + Redis memory for a day | same |
| 3 | Cut production over (M6), image pinned by digest | revert the digest pin to upstream `v3.4.1` |

Because the fallback is in-process and env-driven, rollback is a one-line config change plus a
restart — no data migration, and no cache state that matters (it is a cache).

---

## 13. Acceptance criteria

- [ ] `SEERR_CACHE_BACKEND=memory` is byte-identical to upstream behavior (B1–B5).
- [ ] `SEERR_CACHE_BACKEND=redis` serves repeat lookups from Redis (verified with `redis-cli MONITOR`).
- [ ] Killing Redis never throws and never breaks a Seerr request (B7, verified live).
- [ ] Redis keyspace shows `rawrz:seerr:cache:<id>:*` with the expected TTL range per id.
- [ ] `GET /settings/cache` and `POST /settings/cache/:id/flush` work against the Redis backend.
- [ ] Flushing a Seerr namespace cannot touch RAWRZ keys (R2, verified by foreign-key test).
- [ ] No credential substring appears in any Redis key (B8).
- [ ] Seerr restart preserves cache entries (restart survival demonstrated).
- [ ] The patch is ≤ ~10 files and carries no unrelated hunks (R5), and the sync workflow can
      rebuild it from the `.patch` artifact.

---

## 14. Corrections applied to the master spec

| Master-spec claim | Verified correction |
|---|---|
| "~17 touched call sites", "~10 touched files" (§5.2) | **14 consumer call sites** (10 `getCache(` + 4 `.data`), **8 internal touchpoints** across 4 `ExternalAPI` methods, **11 files** |
| Interface sketch listed `get/set/has/del/flush/keys/stats` (§5.2 item 1) | Required set is `get`, `set`, `del`, **`getTtlMs`**, `flush`, `stats` — `getTtl` was missing and is needed by `getRolling()`; `has`/`keys` are unused and dropped |
| "`ExternalAPI` … its lookup becomes `await …` and its write becomes `await …`. One file, two lines." (§5.2 item 2) | Four methods touch the cache (`get`, `post`, `getRolling`, `removeCache`) — 8 touchpoints, not 2 |
| "Seerr's ... `getStats()` consumed by settings/jobs surfaces" (§5.2) | Consumed by the `/settings/cache` route (stats) and `/settings/cache/:cacheId/flush` (flush) |

---

## Appendix — verified evidence index

| Claim | Source |
|---|---|
| Cache wrapper, 10 named caches, TTLs, `getStats`, `flush` | `~/TRUTH/seerr/server/lib/cache.ts` (read in full, 91 lines) |
| `ExternalAPI` async `get`/`post`/`getRolling`/`removeCache`, `nodeCache` option, `serializeCacheKey` | `~/TRUTH/seerr/server/api/externalapi.ts` (read in full) |
| Seven `nodeCache:` handoffs | `github.ts:74`, `plextv.ts:151`, `rating/imdbRadarrProxy.ts:167`, `rating/rottentomatoes.ts:119`, `servarr/base.ts:109`, `themoviedb/index.ts:141`, `tvdb/index.ts:56` |
| Four direct `.data` accesses | `plextv.ts:282,308`; `lib/scanners/plex/index.ts:388,439` |
| Async enclosing contexts | `plextv.ts:273` `public async getWatchlist`; `scanners/plex/index.ts:381` `private async getMediaIds` |
| Settings flush route | `~/TRUTH/seerr/server/routes/settings/index.ts:780-791` |
| No cache tests exist upstream | `find` for `*cache*test*`/`*cache*spec*` under `~/TRUTH/seerr` → none |
| No Redis anywhere upstream | `grep -ri redis ~/TRUTH/seerr` → no matches |
| Seerr pinned version | `~/TRUTH/seerr/package.json` → `"version": "3.4.1"`; image `ghcr.io/seerr-team/seerr:v3.4.1` in `~/Cave/docker-compose.yml` |
