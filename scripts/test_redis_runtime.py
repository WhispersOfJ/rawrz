#!/usr/bin/env python3
"""Exercise the M1 Redis cache against a live Redis endpoint.

The write and verify phases are separated so the caller can restart Redis
between them and prove AOF persistence without exposing Redis on a host port.
"""

import argparse
import os
import sys

try:
    from redis_cache import RedisCache
except ImportError:
    from scripts.redis_cache import RedisCache


def fail(message):
    print("FAIL: %s" % message, file=sys.stderr)
    return 1


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("write", "verify", "outage"), default="write")
    parser.add_argument("--key", required=True)
    parser.add_argument("--value", required=True)
    args = parser.parse_args(argv)

    cache = RedisCache(os.environ.get("REDIS_URL"), namespace="rawrz:runtime", timeout=2)
    if args.phase == "outage":
        if cache.get_json(args.key) is not None:
            return fail("an unavailable Redis returned a cache value")
        stats = cache.info()
        if stats["errors"] != 1 or stats["misses"] != 1:
            return fail("outage was not recorded as a fail-open miss: %s" % stats)
        print("Redis outage: ok (fail-open miss verified)")
        return 0

    try:
        if cache.ping() != "PONG":
            return fail("Redis PING did not return PONG")
    except OSError as exc:
        return fail("Redis PING failed: %s" % exc)

    if args.phase == "write":
        value = {"source": "m1", "value": args.value}
        if cache.get_json(args.key) is not None:
            return fail("runtime key was not empty")
        if not cache.set_json(args.key, value, 120):
            return fail("Redis SET failed")
        if cache.get_json(args.key) != value:
            return fail("Redis GET did not return the written JSON")
        ttl = cache.ttl(args.key)
        if ttl < 1 or ttl > 120:
            return fail("Redis TTL was not applied: %s" % ttl)
        stats = cache.info()
        if stats["hits"] != 1 or stats["misses"] != 1 or stats["errors"] != 0:
            return fail("unexpected cache counters: %s" % stats)
        print("Redis runtime write: ok (hit/miss/TTL verified)")
        return 0

    value = cache.get_json(args.key)
    if value != {"source": "m1", "value": args.value}:
        return fail("AOF restart did not preserve the cache value: %r" % (value,))
    print("Redis runtime verify: ok (AOF persistence verified)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
