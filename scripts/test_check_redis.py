#!/usr/bin/env python3
"""Tests for the offline M1 Redis contract checker."""

import importlib.util
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("check_redis", ROOT / "scripts" / "check_redis.py")
CHECK = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECK)


VALID = """\
  redis:
    image: redis:7-alpine
    container_name: rawrz-redis
    mem_limit: 512m
    networks: [bearcave]
    volumes:
      - ./config/redis:/data
    command:
      - redis-server
      - --appendonly
      - "yes"
      - --appendfsync
      - everysec
      - --maxmemory
      - 512mb
      - --maxmemory-policy
      - volatile-lru
    healthcheck:
      test: ["CMD-SHELL", "redis-cli ping | grep -q PONG"]
  prowlarr:
"""


def main() -> int:
    failures = 0

    def expect(label: str, condition: bool) -> None:
        nonlocal failures
        if condition:
            print(f"OK: {label}")
        else:
            print(f"FAIL: {label}")
            failures += 1

    expect("valid Redis block passes", CHECK.check_contract(VALID) == [])
    expect(
        "published Redis port fails",
        any("host port" in item for item in CHECK.check_contract(VALID.replace(
            '    volumes:\n', '    ports:\n      - "6379:6379"\n    volumes:\n'
        ))),
    )
    expect(
        "allkeys eviction fails",
        any("eviction policy" in item for item in CHECK.check_contract(VALID.replace(
            "volatile-lru", "allkeys-lru"
        ))),
    )

    with tempfile.TemporaryDirectory() as directory:
        original = CHECK.COMPOSE
        try:
            CHECK.COMPOSE = Path(directory) / "docker-compose.yml"
            CHECK.COMPOSE.write_text("services:\n" + VALID, encoding="utf-8")
            expect("main checker accepts the fixture", CHECK.main() == 0)
        finally:
            CHECK.COMPOSE = original

    if failures:
        print(f"test_check_redis: {failures} assertion(s) failed")
        return 1
    print("test_check_redis: all assertions passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
