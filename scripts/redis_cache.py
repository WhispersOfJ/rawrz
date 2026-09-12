#!/usr/bin/env python3
"""Small fail-open Redis-backed JSON cache for host-run RAWRZ scripts."""
import hashlib
import json
import os
import socket
import time
from dataclasses import dataclass


@dataclass
class CacheStats:
    hits: int = 0
    misses: int = 0
    errors: int = 0


class RedisCache:
    def __init__(self, url=None, namespace="rawrz:activity:cache", timeout=1.0):
        self.url = url or os.environ.get("REDIS_URL", "")
        self.namespace = namespace.rstrip(":")
        self.timeout = timeout
        self.stats = CacheStats()
        self._host = None
        self._port = None
        self._parse_url()

    def _parse_url(self):
        if not self.url.startswith("redis://"):
            return
        try:
            address = self.url[8:].split("/", 1)
            host_port = address[0].rsplit("@", 1)[-1]
            self._host, port = (host_port.rsplit(":", 1) if ":" in host_port else (host_port, 6379))
            self._port = int(port)
        except (TypeError, ValueError):
            self._host = self._port = None

    def _command(self, *parts):
        if self._host is None or self._port is None:
            raise OSError("REDIS_URL is unset or invalid")
        payload = b"*%d\r\n" % len(parts)
        for part in parts:
            encoded = str(part).encode()
            payload += b"$%d\r\n%s\r\n" % (len(encoded), encoded)
        with socket.create_connection((self._host, self._port), self.timeout) as sock:
            sock.sendall(payload)
            buffer = bytearray()
            return self._read_response(sock, buffer)

    @staticmethod
    def _read_exact(sock, buffer, size):
        while len(buffer) < size:
            chunk = sock.recv(4096)
            if not chunk:
                raise OSError("Redis truncated the response")
            buffer.extend(chunk)
        result = bytes(buffer[:size])
        del buffer[:size]
        return result

    @classmethod
    def _read_line(cls, sock, buffer):
        while b"\r\n" not in buffer:
            chunk = sock.recv(4096)
            if not chunk:
                raise OSError("Redis truncated the response")
            buffer.extend(chunk)
        index = buffer.index(b"\r\n")
        line = bytes(buffer[:index])
        del buffer[: index + 2]
        return line

    @classmethod
    def _read_response(cls, sock, buffer):
        prefix = cls._read_exact(sock, buffer, 1)
        line = cls._read_line(sock, buffer)
        if prefix == b"-":
            raise OSError(line.decode(errors="replace"))
        if prefix in (b"+", b":"):
            return line.decode() if prefix == b"+" else int(line)
        if prefix == b"$":
            length = int(line)
            if length == -1:
                return None
            return cls._read_exact(sock, buffer, length + 2)[:-2].decode()
        if prefix == b"*":
            count = int(line)
            return [cls._read_response(sock, buffer) for _ in range(count)] if count >= 0 else None
        raise OSError("Unsupported Redis response")

    def _key(self, key):
        return "%s:%s" % (self.namespace, key)

    def ping(self):
        return self._command("PING")

    def ttl(self, key):
        return self._command("TTL", self._key(key))

    def get_json(self, key):
        try:
            value = self._command("GET", self._key(key))
        except (OSError, ValueError):
            self.stats.errors += 1
            self.stats.misses += 1
            return None
        if value is None:
            self.stats.misses += 1
            return None
        try:
            value = json.loads(value)
        except json.JSONDecodeError:
            self.stats.errors += 1
            self.stats.misses += 1
            return None
        self.stats.hits += 1
        return value

    def set_json(self, key, value, ttl_seconds):
        try:
            self._command("SET", self._key(key), json.dumps(value, sort_keys=True), "EX", ttl_seconds)
            return True
        except (OSError, ValueError):
            self.stats.errors += 1
            return False

    def info(self):
        return {"hits": self.stats.hits, "misses": self.stats.misses, "errors": self.stats.errors}


class MemoryCache:
    def __init__(self):
        self.values = {}
        self.stats = CacheStats()

    def get_json(self, key):
        item = self.values.get(key)
        if item is None or item[0] <= time.time():
            self.stats.misses += 1
            return None
        self.stats.hits += 1
        return item[1]

    def set_json(self, key, value, ttl_seconds):
        self.values[key] = (time.time() + ttl_seconds, value)
        return True

    def info(self):
        return {"hits": self.stats.hits, "misses": self.stats.misses, "errors": 0}


def cache_from_env():
    return RedisCache() if os.environ.get("REDIS_URL") else MemoryCache()


def cache_key(app, page, cursor):
    """Return a bounded key that changes when the app history cursor changes."""
    digest = hashlib.sha256(str(cursor or "0").encode()).hexdigest()[:12]
    return "%s:page:%d:%s" % (app, page, digest)
