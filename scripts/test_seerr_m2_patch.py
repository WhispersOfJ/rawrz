from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "services/seerr/patches/rawrz-redis-cache.patch"
text = PATCH.read_text()

# The runtime acceptance test owns behavior; this offline check only protects the
# artifact's integration and the namespace safety invariant.
for needle in (
    "server/lib/cache/backend.ts",
    "ioredis",
    "SEERR_CACHE_BACKEND",
    "SEERR_CACHE_REDIS_PREFIX",
):
    assert needle in text, needle
assert "FLUSHALL" not in text
assert not re.search(r"^\+.*\bKEYS\b", text, re.MULTILINE)
print("M2 Seerr patch checks passed")
