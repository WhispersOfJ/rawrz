from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "services/seerr/patches/rawrz-redis-cache.patch"

text = PATCH.read_text()
required = [
    "server/lib/cache/backend.ts",
    "CacheBackend",
    "SEERR_CACHE_BACKEND",
    "SEERR_CACHE_REDIS_PREFIX",
    "crypto.createHash('sha256')",
    "SCAN",
    "UNLINK",
    "await this.cache?.get",
    "await this.cache.set",
    "await cache.flush()",
]
for needle in required:
    assert needle in text, needle
assert "FLUSHALL" not in text
assert not re.search(r"^\+.*\bKEYS\b", text, re.MULTILINE)
assert not re.search(r"^\+.*nodeCache", text, re.MULTILINE)
assert not re.search(r"^\+.*\.data\.(get|set|del|flushAll|getTtl)", text, re.MULTILINE)
assert "response: response.backend" not in text
assert "604800" in text
assert "300" in text
print("M2 Seerr patch checks passed")
