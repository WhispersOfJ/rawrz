from pathlib import Path
import json
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path('/home/bear/TRUTH/seerr')
PATCH = ROOT / 'services/seerr/patches/rawrz-redis-cache.patch'
BACKEND = r'''import crypto from 'crypto';
import Redis from 'ioredis';
import NodeCache from 'node-cache';

export interface CacheStats {
  hits: number;
  misses: number;
  keys: number;
  ksize: number;
  vsize: number;
  errors: number;
  backend: 'memory' | 'redis';
  degraded: boolean;
}

export interface CacheBackend {
  get<T>(key: string): Promise<T | undefined>;
  set<T>(key: string, value: T, ttlSeconds: number): Promise<void>;
  del(key: string): Promise<void>;
  getTtlMs(key: string): Promise<number | undefined>;
  flush(): Promise<void>;
  stats(): CacheStats;
}

const PREFIX = process.env.SEERR_CACHE_REDIS_PREFIX || 'rawrz:seerr:cache';
const redisKey = (cacheId: string, key: string) =>
  `${PREFIX}:${cacheId}:${crypto.createHash('sha256').update(key).digest('hex')}`;

class MemoryCacheBackend implements CacheBackend {
  private readonly cache: NodeCache;

  constructor(stdTtl: number, checkPeriod: number) {
    this.cache = new NodeCache({ stdTTL: stdTtl, checkperiod: checkPeriod });
  }

  public async get<T>(key: string): Promise<T | undefined> {
    return this.cache.get<T>(key);
  }

  public async set<T>(key: string, value: T, ttlSeconds: number): Promise<void> {
    this.cache.set(key, value, ttlSeconds);
  }

  public async del(key: string): Promise<void> {
    this.cache.del(key);
  }

  public async getTtlMs(key: string): Promise<number | undefined> {
    return this.cache.getTtl(key);
  }

  public async flush(): Promise<void> {
    this.cache.flushAll();
  }

  public stats(): CacheStats {
    return { ...this.cache.getStats(), errors: 0, backend: 'memory', degraded: false };
  }
}

class RedisRuntime {
  public readonly client: Redis;
  public degraded = false;
  public errors = 0;
  private probe?: NodeJS.Timeout;

  constructor(url: string) {
    this.client = new Redis(url, {
      lazyConnect: true,
      enableOfflineQueue: false,
      maxRetriesPerRequest: 1,
      connectTimeout: Number(process.env.SEERR_CACHE_REDIS_TIMEOUT_MS || 2000),
      retryStrategy: (attempt) => Math.min(attempt * 250, 5000),
    });
    this.client.on('error', () => this.fail());
    this.scheduleProbe();
  }

  private fail(): void {
    this.degraded = true;
    this.errors++;
  }

  private scheduleProbe(): void {
    this.probe = setTimeout(() => void this.probeRedis(), Number(process.env.SEERR_CACHE_REDIS_PROBE_MS || 30000));
    this.probe.unref();
  }

  private async probeRedis(): Promise<void> {
    try {
      await this.client.connect().catch(() => undefined);
      await this.client.ping();
      this.degraded = false;
    } catch {
      this.fail();
    } finally {
      this.scheduleProbe();
    }
  }

  public async command<T>(operation: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await operation();
      this.degraded = false;
      return result;
    } catch {
      this.fail();
      return undefined;
    }
  }
}

const runtime = process.env.SEERR_CACHE_BACKEND === 'redis' && process.env.REDIS_URL
  ? new RedisRuntime(process.env.REDIS_URL)
  : undefined;

class RedisCacheBackend implements CacheBackend {
  private readonly fallback: MemoryCacheBackend;
  private readonly entries = new Map<string, { ksize: number; vsize: number; expiresAt?: number }>();
  private hits = 0;
  private misses = 0;

  constructor(private readonly cacheId: string, stdTtl: number, checkPeriod: number) {
    this.fallback = new MemoryCacheBackend(stdTtl, checkPeriod);
  }

  private get redis(): RedisRuntime {
    if (!runtime) throw new Error('Redis cache runtime is not configured');
    return runtime;
  }

  public async get<T>(key: string): Promise<T | undefined> {
    const raw = await this.redis.command(() => this.redis.client.get(redisKey(this.cacheId, key)));
    if (raw === undefined && this.redis.degraded) return this.fallback.get<T>(key);
    if (raw === null || raw === undefined) {
      this.misses++;
      return undefined;
    }
    try {
      const envelope = JSON.parse(raw) as { v: number; c: string; d: T };
      if (envelope.v !== 1 || envelope.c !== this.cacheId || !Object.prototype.hasOwnProperty.call(envelope, 'd')) {
        this.misses++;
        return undefined;
      }
      this.hits++;
      return structuredClone(envelope.d);
    } catch {
      this.misses++;
      return undefined;
    }
  }

  public async set<T>(key: string, value: T, ttlSeconds: number): Promise<void> {
    const payload = JSON.stringify({ v: 1, c: this.cacheId, d: value });
    const encodedKey = redisKey(this.cacheId, key);
    const result = await this.redis.command(() => ttlSeconds > 0
      ? this.redis.client.set(encodedKey, payload, 'EX', ttlSeconds)
      : this.redis.client.set(encodedKey, payload));
    if (result === undefined && this.redis.degraded) {
      await this.fallback.set(key, value, ttlSeconds);
      return;
    }
    this.entries.set(encodedKey, {
      ksize: Buffer.byteLength(encodedKey),
      vsize: Buffer.byteLength(payload),
      ...(ttlSeconds > 0 ? { expiresAt: Date.now() + ttlSeconds * 1000 } : {}),
    });
  }

  public async del(key: string): Promise<void> {
    const encodedKey = redisKey(this.cacheId, key);
    const result = await this.redis.command(() => this.redis.client.unlink(encodedKey));
    if (result === undefined && this.redis.degraded) await this.fallback.del(key);
    this.entries.delete(encodedKey);
  }

  public async getTtlMs(key: string): Promise<number | undefined> {
    const ttl = await this.redis.command(() => this.redis.client.pttl(redisKey(this.cacheId, key)));
    if (ttl === undefined && this.redis.degraded) return this.fallback.getTtlMs(key);
    if (ttl === -1) return 0;
    return typeof ttl === 'number' && ttl >= 0 ? Date.now() + ttl : undefined;
  }

  public async flush(): Promise<void> {
    try {
      let cursor = '0';
      do {
        const [next, keys] = await this.redis.client.scan(cursor, 'MATCH', `${PREFIX}:${this.cacheId}:*`, 'COUNT', '500');
        cursor = next;
        if (keys.length) await this.redis.client.unlink(...keys);
      } while (cursor !== '0');
    } catch {
      this.redis.degraded = true;
      this.redis.errors++;
      await this.fallback.flush();
    } finally {
      this.entries.clear();
    }
  }

  public stats(): CacheStats {
    const now = Date.now();
    for (const [key, entry] of this.entries) {
      if (entry.expiresAt && entry.expiresAt <= now) this.entries.delete(key);
    }
    const fallback = this.fallback.stats();
    return {
      hits: this.hits + fallback.hits,
      misses: this.misses + fallback.misses,
      keys: this.entries.size + fallback.keys,
      ksize: [...this.entries.values()].reduce((total, entry) => total + entry.ksize, 0),
      vsize: [...this.entries.values()].reduce((total, entry) => total + entry.vsize, 0),
      errors: this.redis.errors,
      backend: this.redis.degraded ? 'memory' : 'redis',
      degraded: this.redis.degraded,
    };
  }
}

export function createCacheBackend(cacheId: string, stdTtl: number, checkPeriod: number): CacheBackend {
  return runtime
    ? new RedisCacheBackend(cacheId, stdTtl, checkPeriod)
    : new MemoryCacheBackend(stdTtl, checkPeriod);
}
'''

with tempfile.TemporaryDirectory(prefix='rawrz-seerr-') as tmp:
    work = Path(tmp) / 'work'
    work.mkdir()
    archive = subprocess.run(['git', '-C', str(SOURCE), 'archive', 'v3.4.1'], capture_output=True, check=True).stdout
    subprocess.run(['tar', '-xf', '-', '-C', str(work)], input=archive, check=True)
    subprocess.run(['git', '-C', str(work), 'init', '-q'], check=True)
    subprocess.run(['git', '-C', str(work), 'add', '.'], check=True)
    subprocess.run(['git', '-C', str(work), '-c', 'user.name=RAWRZ', '-c', 'user.email=rawrz@example.invalid', 'commit', '-qm', 'upstream'], check=True)
    package = json.loads((work / 'package.json').read_text())
    package['dependencies']['ioredis'] = '5.8.2'
    package['dependencies'] = dict(sorted(package['dependencies'].items()))
    (work / 'package.json').write_text(json.dumps(package, indent=2) + '\n')
    subprocess.run(['pnpm', 'add', 'ioredis@5.8.2', '--lockfile-only', '--ignore-scripts', '--config.engine-strict=false'], cwd=work, check=True)
    backend = work / 'server/lib/cache/backend.ts'
    backend.parent.mkdir(parents=True, exist_ok=True)
    backend.write_text(BACKEND)
    cache = work / 'server/lib/cache.ts'
    text = cache.read_text()
    text = text.replace("import NodeCache from 'node-cache';", "import type { CacheBackend, CacheStats } from '@server/lib/cache/backend';\nimport { createCacheBackend } from '@server/lib/cache/backend';")
    text = text.replace('  public data: NodeCache;', '  public readonly backend: CacheBackend;')
    text = text.replace("""    this.data = new NodeCache({
      stdTTL: options.stdTtl ?? DEFAULT_TTL,
      checkperiod: options.checkPeriod ?? DEFAULT_CHECK_PERIOD,
    });""", "    this.backend = createCacheBackend(id, options.stdTtl ?? DEFAULT_TTL, options.checkPeriod ?? DEFAULT_CHECK_PERIOD);")
    text = text.replace("""  public getStats() {
    return this.data.getStats();
  }

  public flush(): void {
    this.data.flushAll();
  }""", """  public getStats(): CacheStats {
    return this.backend.stats();
  }

  public async flush(): Promise<void> {
    await this.backend.flush();
  }""")
    cache.write_text(text)
    external = work / 'server/api/externalapi.ts'
    text = external.read_text().replace("import type NodeCache from 'node-cache';", "import type { CacheBackend } from '@server/lib/cache/backend';")
    text = text.replace('nodeCache?: NodeCache', 'cache?: CacheBackend').replace('private cache?: NodeCache', 'private cache?: CacheBackend').replace('this.cache = options.nodeCache', 'this.cache = options.cache')
    text = text.replace('this.cache?.get<T>(cacheKey)', 'await this.cache?.get<T>(cacheKey)').replace('this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL)', 'await this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL)').replace('this.cache?.getTtl(cacheKey)', 'await this.cache?.getTtlMs(cacheKey)').replace('this.cache?.del(cacheKey)', 'await this.cache?.del(cacheKey)').replace('protected removeCache(', 'protected async removeCache(')
    text = text.replace("""this.axios.get<T>(endpoint, config).then((response) => {
          this.cache?.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);
        });""", "void this.axios.get<T>(endpoint, config).then((response) => this.cache?.set(cacheKey, response.data, ttl ?? DEFAULT_TTL)).catch(() => undefined);")
    text = text.replace('if (cachedItem) {', 'if (cachedItem !== undefined) {')
    external.write_text(text)
    for name in ['server/api/github.ts', 'server/api/plextv.ts', 'server/api/rating/imdbRadarrProxy.ts', 'server/api/rating/rottentomatoes.ts', 'server/api/servarr/base.ts', 'server/api/themoviedb/index.ts', 'server/api/tvdb/index.ts']:
        p = work / name
        p.write_text(p.read_text().replace('nodeCache:', 'cache:').replace('.data,', '.backend,'))
    p = work / 'server/api/plextv.ts'
    text = p.read_text().replace('watchlistCache.data.get', 'watchlistCache.backend.get').replace('watchlistCache.data.set', 'watchlistCache.backend.set')
    text = text.replace('let cachedWatchlist = watchlistCache.backend.get', 'let cachedWatchlist = await watchlistCache.backend.get')
    text = text.replace('response: response.backend,', 'response: response.data,')
    text = text.replace('watchlistCache.backend.set<PlexWatchlistCache>(\n          this.authToken,\n          cachedWatchlist\n        );', 'await watchlistCache.backend.set<PlexWatchlistCache>(this.authToken, cachedWatchlist, 300);')
    p.write_text(text)
    p = work / 'server/lib/scanners/plex/index.ts'
    text = p.read_text().replace('guidCache.data.get', 'guidCache.backend.get').replace('guidCache.data.set', 'guidCache.backend.set')
    text = text.replace('guidCache.backend.get<MediaIds>(plexitem.ratingKey)', 'await guidCache.backend.get<MediaIds>(plexitem.ratingKey)').replace('guidCache.backend.set(plexitem.ratingKey, mediaIds);', 'await guidCache.backend.set(plexitem.ratingKey, mediaIds, 604800);')
    p.write_text(text)
    p = work / 'server/routes/settings/index.ts'
    p.write_text(p.read_text().replace('  (req, res, next) => {\n    const cache = cacheManager.getCache(req.params.cacheId);', '  async (req, res, next) => {\n    const cache = cacheManager.getCache(req.params.cacheId);').replace('      cache.flush();', '      await cache.flush();'))
    subprocess.run(['git', '-C', str(work), 'add', '.'], check=True)
    patch = subprocess.run(['git', '-C', str(work), 'diff', '--cached', '--binary'], capture_output=True).stdout.decode()
    patch = '\n'.join(line.rstrip() for line in patch.splitlines()).rstrip('\n') + '\n'
    PATCH.write_text(patch)
    print(f'generated {PATCH} ({PATCH.stat().st_size} bytes)')
