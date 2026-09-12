from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / ".cache" / "seerr-patch-source"
PATCH = ROOT / "services" / "seerr" / "patches" / "rawrz-redis-cache.patch"
SOURCE = Path("/home/bear/TRUTH/seerr")


def archive_source(target: Path) -> None:
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    archive = subprocess.run(
        ["git", "-C", str(SOURCE), "archive", "v3.4.1"], capture_output=True, check=True
    ).stdout
    proc = subprocess.Popen(["tar", "-xf", "-", "-C", str(target)], stdin=subprocess.PIPE)
    proc.communicate(archive)


def replace(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"replacement not found in {path}: {old[:80]!r}")
    path.write_text(text.replace(old, new))


archive_source(OUT)
backend = r'''import crypto from 'crypto';
import net from 'net';

export type CacheKind = 'memory' | 'redis';

export interface CacheStats {
  hits: number;
  misses: number;
  keys: number;
  ksize: number;
  vsize: number;
  errors: number;
  backend: CacheKind;
  degraded: boolean;
}

export interface CacheBackend {
  readonly kind: CacheKind;
  get<T>(key: string): Promise<T | undefined>;
  set<T>(key: string, value: T, ttlSeconds: number): Promise<void>;
  del(key: string): Promise<void>;
  getTtlMs(key: string): Promise<number | undefined>;
  flush(): Promise<void>;
  stats(): CacheStats;
}

const DEFAULT_TTL = 300;
const PREFIX = process.env.SEERR_CACHE_REDIS_PREFIX || 'rawrz:seerr:cache';
const REDIS_URL = process.env.REDIS_URL;

function now(): number { return Date.now(); }

class MemoryCacheBackend implements CacheBackend {
  public readonly kind = 'memory' as const;
  private values = new Map<string, { value: unknown; expires: number }>();
  private hits = 0;
  private misses = 0;

  public async get<T>(key: string): Promise<T | undefined> {
    const entry = this.values.get(key);
    if (!entry || (entry.expires > 0 && entry.expires <= now())) {
      if (entry) this.values.delete(key);
      this.misses++;
      return undefined;
    }
    this.hits++;
    return structuredClone(entry.value) as T;
  }

  public async set<T>(key: string, value: T, ttlSeconds = DEFAULT_TTL): Promise<void> {
    this.values.set(key, {
      value: structuredClone(value),
      expires: ttlSeconds > 0 ? now() + ttlSeconds * 1000 : 0,
    });
  }

  public async del(key: string): Promise<void> { this.values.delete(key); }
  public async getTtlMs(key: string): Promise<number | undefined> {
    const entry = this.values.get(key);
    if (!entry || (entry.expires > 0 && entry.expires <= now())) return undefined;
    return entry.expires === 0 ? 0 : entry.expires;
  }
  public async flush(): Promise<void> { this.values.clear(); this.hits = 0; this.misses = 0; }
  public stats(): CacheStats {
    return { hits: this.hits, misses: this.misses, keys: this.values.size, ksize: 0, vsize: 0, errors: 0, backend: 'memory', degraded: false };
  }
}

function encode(command: string, args: string[]): Buffer {
  const parts = [command, ...args];
  return Buffer.from(`*${parts.length}\r\n${parts.map((part) => `$${Buffer.byteLength(part)}\r\n${part}\r\n`).join('')}`);
}

function parseResp(buffer: Buffer): unknown {
  const text = buffer.toString();
  if (text.startsWith('+')) return text.slice(1).split('\r\n', 1)[0];
  if (text.startsWith('-')) throw new Error(text.slice(1).split('\r\n', 1)[0]);
  if (text.startsWith(':')) return Number(text.slice(1).split('\r\n', 1)[0]);
  if (text.startsWith('$')) {
    const end = text.indexOf('\r\n');
    const length = Number(text.slice(1, end));
    if (length < 0) return undefined;
    return text.slice(end + 2, end + 2 + length);
  }
  throw new Error('Unexpected Redis response');
}

async function redisCommand(url: URL, command: string, args: string[]): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: url.hostname, port: Number(url.port || 6379) });
    const chunks: Buffer[] = [];
    const timeout = setTimeout(() => { socket.destroy(); reject(new Error('Redis command timeout')); }, Number(process.env.SEERR_CACHE_REDIS_TIMEOUT_MS || 2000));
    socket.on('connect', () => socket.write(encode(command, args)));
    socket.on('data', (chunk: Buffer) => {
      chunks.push(chunk);
      const data = Buffer.concat(chunks);
      if (data.includes(Buffer.from('\r\n'))) {
        clearTimeout(timeout); socket.end();
        try { resolve(parseResp(data)); } catch (error) { reject(error); }
      }
    });
    socket.on('error', (error) => { clearTimeout(timeout); reject(error); });
  });
}

function safeKey(cacheId: string, key: string): string {
  const hash = crypto.createHash('sha256').update(key).digest('hex');
  return `${PREFIX}:${cacheId}:${hash}`;
}

class RedisCacheBackend implements CacheBackend {
  public readonly kind = 'redis' as const;
  private readonly fallback = new MemoryCacheBackend();
  private hits = 0;
  private misses = 0;
  private errors = 0;
  private degraded = false;
  private readonly url: URL;

  constructor(private readonly cacheId: string) {
    this.url = new URL(REDIS_URL || 'redis://127.0.0.1:6379/0');
  }

  private async command(command: string, args: string[]): Promise<unknown> {
    try {
      const result = await redisCommand(this.url, command, args);
      this.degraded = false;
      return result;
    } catch {
      this.errors++;
      this.degraded = true;
      return undefined;
    }
  }

  public async get<T>(key: string): Promise<T | undefined> {
    const remote = await this.command('GET', [safeKey(this.cacheId, key)]);
    if (remote === undefined && this.degraded) return this.fallback.get<T>(key);
    if (typeof remote !== 'string') { this.misses++; return undefined; }
    try {
      const envelope = JSON.parse(remote) as { v: number; c: string; d: T };
      if (envelope.v !== 1 || envelope.c !== this.cacheId || !envelope.d) { this.misses++; return undefined; }
      this.hits++;
      return structuredClone(envelope.d);
    } catch { this.misses++; return undefined; }
  }

  public async set<T>(key: string, value: T, ttlSeconds = DEFAULT_TTL): Promise<void> {
    if (ttlSeconds === 0) return;
    const envelope = JSON.stringify({ v: 1, c: this.cacheId, d: value });
    const args = [safeKey(this.cacheId, key), envelope];
    if (ttlSeconds > 0) args.push('EX', String(ttlSeconds));
    const result = await this.command('SET', args);
    if (result === undefined && this.degraded) await this.fallback.set(key, value, ttlSeconds);
  }

  public async del(key: string): Promise<void> {
    const result = await this.command('DEL', [safeKey(this.cacheId, key)]);
    if (result === undefined && this.degraded) await this.fallback.del(key);
  }

  public async getTtlMs(key: string): Promise<number | undefined> {
    const result = await this.command('PTTL', [safeKey(this.cacheId, key)]);
    if (result === undefined && this.degraded) return this.fallback.getTtlMs(key);
    if (typeof result !== 'number' || result < 0) return undefined;
    return now() + result;
  }

  public async flush(): Promise<void> {
    let cursor = '0';
    do {
      const result = await this.command('SCAN', [cursor, 'MATCH', `${PREFIX}:${this.cacheId}:*`, 'COUNT', '500']);
      if (!Array.isArray(result) || result.length !== 2) {
        if (this.degraded) await this.fallback.flush();
        return;
      }
      cursor = String(result[0]);
      const keys = result[1] as string[];
      if (keys.length) await this.command('UNLINK', keys);
    } while (cursor !== '0');
  }

  public stats(): CacheStats {
    const fallback = this.fallback.stats();
    return { hits: this.hits + fallback.hits, misses: this.misses + fallback.misses, keys: fallback.keys, ksize: 0, vsize: 0, errors: this.errors, backend: this.degraded ? 'memory' : 'redis', degraded: this.degraded };
  }
}

export function createCacheBackend(cacheId: string, _ttlSeconds: number): CacheBackend {
  return process.env.SEERR_CACHE_BACKEND === 'redis' && REDIS_URL
    ? new RedisCacheBackend(cacheId)
    : new MemoryCacheBackend();
}
'''
(OUT / 'server/lib/cache/backend.ts').parent.mkdir(parents=True, exist_ok=True)
(OUT / 'server/lib/cache/backend.ts').write_text(backend)

cache = OUT / 'server/lib/cache.ts'
replace(cache, "import NodeCache from 'node-cache';\n", "import type { CacheBackend, CacheStats } from '@server/lib/cache/backend';\nimport { createCacheBackend } from '@server/lib/cache/backend';\n")
replace(cache, "  public data: NodeCache;\n", "  public readonly backend: CacheBackend;\n")
replace(cache, """    this.data = new NodeCache({
      stdTTL: options.stdTtl ?? DEFAULT_TTL,
      checkperiod: options.checkPeriod ?? DEFAULT_CHECK_PERIOD,
    });""", "    this.backend = createCacheBackend(id, options.stdTtl ?? DEFAULT_TTL);")
replace(cache, """  public getStats() {
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

external = OUT / 'server/api/externalapi.ts'
replace(external, "import type NodeCache from 'node-cache';\n", "import type { CacheBackend } from '@server/lib/cache/backend';\n")
text = external.read_text().replace("nodeCache?: NodeCache", "cache?: CacheBackend").replace("private cache?: NodeCache", "private cache?: CacheBackend").replace("this.cache = options.nodeCache", "this.cache = options.cache")
text = text.replace('this.cache?.get<T>(cacheKey)', 'await this.cache?.get<T>(cacheKey)')
text = text.replace("const cachedItem = this.cache?.get<T>(cacheKey);", "const cachedItem = await this.cache?.get<T>(cacheKey);")
text = text.replace("this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);", "await this.cache.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);")
text = text.replace("const keyTtl = this.cache?.getTtl(cacheKey) ?? 0;", "const keyTtl = (await this.cache?.getTtlMs(cacheKey)) ?? 0;")
text = text.replace("this.axios.get<T>(endpoint, config).then((response) => {\n          this.cache?.set(cacheKey, response.data, ttl ?? DEFAULT_TTL);\n        });", "void this.axios.get<T>(endpoint, config).then((response) => this.cache?.set(cacheKey, response.data, ttl ?? DEFAULT_TTL)).catch(() => undefined);")
text = text.replace("    this.cache?.del(cacheKey);", "    await this.cache?.del(cacheKey);")
text = text.replace("protected removeCache(", "protected async removeCache(")
external.write_text(text)

for path in [
    'server/api/github.ts', 'server/api/plextv.ts', 'server/api/rating/imdbRadarrProxy.ts',
    'server/api/rating/rottentomatoes.ts', 'server/api/servarr/base.ts',
    'server/api/themoviedb/index.ts', 'server/api/tvdb/index.ts',
]:
    p = OUT / path
    t = p.read_text().replace('nodeCache:', 'cache:')
    t = t.replace("cacheManager.getCache('github').data", "cacheManager.getCache('github').backend")
    t = t.replace("cacheManager.getCache('plextv').data", "cacheManager.getCache('plextv').backend")
    t = t.replace("cacheManager.getCache('imdb').data", "cacheManager.getCache('imdb').backend")
    t = t.replace("cacheManager.getCache('rt').data", "cacheManager.getCache('rt').backend")
    t = t.replace("cacheManager.getCache(cacheName).data", "cacheManager.getCache(cacheName).backend")
    t = t.replace("cacheManager.getCache('tmdb').data", "cacheManager.getCache('tmdb').backend")
    t = t.replace("cacheManager.getCache(finalConfig.cachePrefix).data", "cacheManager.getCache(finalConfig.cachePrefix).backend")
    p.write_text(t)

p = OUT / 'server/api/plextv.ts'
t = p.read_text().replace('watchlistCache.data.get', 'watchlistCache.backend.get').replace('watchlistCache.data.set', 'watchlistCache.backend.set')
t = t.replace('let cachedWatchlist = watchlistCache.backend.get<PlexWatchlistCache>(', 'let cachedWatchlist = await watchlistCache.backend.get<PlexWatchlistCache>(')
t = t.replace('response: response.backend,', 'response: response.data,')
t = t.replace('watchlistCache.backend.set<PlexWatchlistCache>(\n          this.authToken,\n          cachedWatchlist\n        );', 'await watchlistCache.backend.set<PlexWatchlistCache>(this.authToken, cachedWatchlist, 300);')
p.write_text(t)

p = OUT / 'server/lib/scanners/plex/index.ts'
t = p.read_text().replace('guidCache.data.get', 'guidCache.backend.get').replace('guidCache.data.set', 'guidCache.backend.set')
t = t.replace('guidCache.backend.get<MediaIds>(plexitem.ratingKey)', 'await guidCache.backend.get<MediaIds>(plexitem.ratingKey)')
t = t.replace('guidCache.backend.set(plexitem.ratingKey, mediaIds);', 'await guidCache.backend.set(plexitem.ratingKey, mediaIds, 604800);')
p.write_text(t)

p = OUT / 'server/routes/settings/index.ts'
t = p.read_text().replace("  (req, res, next) => {\n    const cache = cacheManager.getCache(req.params.cacheId);", "  async (req, res, next) => {\n    const cache = cacheManager.getCache(req.params.cacheId);")
t = t.replace('      cache.flush();', '      await cache.flush();')
p.write_text(t)

BASE = ROOT / ".cache" / "seerr-patch-base"
archive_source(BASE)
for repo in (OUT, BASE):
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=RAWRZ", "-c", "user.email=rawrz@example.invalid", "commit", "-qm", "upstream"], check=True)
for changed in ['server/lib/cache/backend.ts', 'server/lib/cache.ts', 'server/api/externalapi.ts', 'server/api/github.ts', 'server/api/plextv.ts', 'server/api/rating/imdbRadarrProxy.ts', 'server/api/rating/rottentomatoes.ts', 'server/api/servarr/base.ts', 'server/api/themoviedb/index.ts', 'server/api/tvdb/index.ts', 'server/lib/scanners/plex/index.ts', 'server/routes/settings/index.ts']:
    target = BASE / changed
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(OUT / changed, target)
subprocess.run(["git", "-C", str(BASE), "add", "."], check=True)
patch = subprocess.run(["git", "-C", str(BASE), "diff", "--cached", "--binary", "--no-ext-diff"], capture_output=True, check=True).stdout
PATCH.parent.mkdir(parents=True, exist_ok=True)
PATCH.write_bytes(patch)
print(f'wrote {PATCH} ({len(patch)} bytes)')
