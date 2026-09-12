#!/usr/bin/env python3
"""Validate the M1 Redis service contract from the Compose source.

This is intentionally offline: CI can prove the safety-critical Redis settings
without needing to start the production-shaped stack.
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
COMPOSE = ROOT / "docker-compose.yml"


def redis_block() -> str:
    text = COMPOSE.read_text(encoding="utf-8")
    match = re.search(r"(?ms)^  redis:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:|\Z)", text)
    if not match:
        raise ValueError("docker-compose.yml has no redis service")
    return match.group("body")


def check_contract(block: str) -> list[str]:
    failures: list[str] = []
    required = {
        "image": "image: redis:7-alpine",
        "container": "container_name: rawrz-redis",
        "network": "networks: [bearcave]",
        "data volume": "./config/redis:/data",
        "aof": "--appendonly",
        "aof enabled": '"yes"',
        "durability": "--appendfsync",
        "durability mode": "everysec",
        "memory cap": "--maxmemory",
        "eviction policy": "volatile-lru",
        "healthcheck": "redis-cli ping",
    }
    for name, needle in required.items():
        if needle not in block:
            failures.append(f"missing Redis {name}: {needle}")
    if re.search(r"(?m)^\s+- \"?[0-9]+:[0-9]+\"?\s*$", block):
        failures.append("Redis must not publish a host port")
    if "--maxmemory\n      - 512mb" not in block:
        failures.append("Redis memory cap must remain the documented 512mb starting point")
    return failures


def main() -> int:
    try:
        failures = check_contract(redis_block())
    except (OSError, ValueError) as error:
        print(f"FAIL: {error}")
        return 1
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("Redis M1 contract: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
