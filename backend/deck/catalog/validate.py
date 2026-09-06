#!/usr/bin/env python3
"""Catalog validator — normative rules from thebearcave/cave-deck-spec.md Appendix A.

Checks: schema fields, id uniqueness, port collisions (tcp/udp aware, against the
live stack and within the catalog), volume sanity, env-key shape, secret/default
rule, dependency resolution, count (100 in v1), and the retired-registry
cross-check (word-boundary).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml

HERE = Path(__file__).parent
COUNT_V1 = 100
STACK_PORTS = {3000, 5055, 7878, 8989, 9696, 32400, 7780}
CATEGORIES = {"media", "ops", "network", "home"}

SCHEMA_FIELDS = {
    "id", "name", "category", "retired", "image", "digest", "ports", "volumes",
    "env", "capabilities", "host_network", "pid_mode", "devices", "dependencies",
    "mem_limit", "healthcheck", "docs", "notes", "compose_fragment",
}
REQUIRED = {"name", "category", "image", "mem_limit", "docs"}
PORT_RE = re.compile(r"^(\d+):(\d+)(/(tcp|udp))?$")
ENV_KEY_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def load_retired() -> set[str]:
    lock = HERE / "retired-registry.lock"
    return {ln.strip() for ln in lock.read_text().splitlines() if ln.strip() and not ln.startswith("#")}


def main() -> int:
    catalog = yaml.safe_load((HERE / "catalog.yaml").read_text())
    entries = catalog.get("entries", [])
    errs: list[str] = []
    warns: list[str] = []
    strict = "--strict" in sys.argv
    retired = load_retired()

    if catalog.get("version") != 1:
        errs.append("catalog version must be 1")
    if len(entries) != COUNT_V1:
        msg = f"entries = {len(entries)}, v1 requires {COUNT_V1}"
        (errs if strict else warns).append(msg)

    seen_ids: set[str] = set()
    seen_ports: dict[tuple[int, str], str] = {}

    for e in entries:
        eid = e.get("id", "?")
        unknown = set(e) - SCHEMA_FIELDS
        if unknown:
            errs.append(f"{eid}: unknown fields {sorted(unknown)}")
        if not re.match(r"^[a-z0-9][a-z0-9-]*$", eid):
            errs.append(f"{eid}: id violates ^[a-z0-9][a-z0-9-]*$")
        if eid in seen_ids:
            errs.append(f"{eid}: duplicate id")
        seen_ids.add(eid)
        if e.get("retired") is not False:
            errs.append(f"{eid}: retired must be false")
        for f in REQUIRED:
            if f not in e:
                errs.append(f"{eid}: missing required '{f}'")
        if e.get("category") not in CATEGORIES:
            errs.append(f"{eid}: bad category {e.get('category')!r}")

        for p in e.get("ports", []):
            m = PORT_RE.match(p)
            if not m:
                errs.append(f"{eid}: bad port spec {p!r}")
                continue
            host, proto = int(m.group(1)), (m.group(4) or "tcp")
            if host in STACK_PORTS:
                errs.append(f"{eid}: host port {host} collides with the live stack")
            if (host, proto) in seen_ports:
                errs.append(f"{eid}: host {host}/{proto} already claimed by {seen_ports[(host, proto)]}")
            seen_ports[(host, proto)] = eid

        if e.get("host_network") and e.get("ports"):
            errs.append(f"{eid}: host_network=true cannot publish ports")
        if (e.get("capabilities") or e.get("devices") or e.get("pid_mode")) and not e.get("notes"):
            errs.append(f"{eid}: capabilities/devices/pid_mode require notes justification")

        for v in e.get("volumes", []):
            parts = v.split(":")
            if len(parts) < 2:
                errs.append(f"{eid}: bad volume {v!r}")
            dsts_in_entry = [v.split(":")[1] for v in e.get("volumes", []) if len(v.split(":")) > 1]
            if len(dsts_in_entry) != len(set(dsts_in_entry)):
                errs.append(f"{eid}: duplicate container dst within entry (§A.2)")

        for env in e.get("env", []):
            key = env.get("key", "")
            if not ENV_KEY_RE.match(key):
                errs.append(f"{eid}: bad env key {key!r}")
            if env.get("secret") and env.get("default") is not None:
                errs.append(f"{eid}: secret env {key} must not ship a default")

        for dep in e.get("dependencies", []):
            if dep not in seen_ids:
                # forward refs are fine; resolve after the loop
                continue

        blob = " ".join([eid, e.get("name", ""), e.get("image", "")]).lower()
        for name in retired:
            if re.search(rf"\b{re.escape(name)}\b", blob):
                errs.append(f"{eid}: retired-registry hit '{name}' (§A.4)")

    id_set = {e.get("id") for e in entries}
    for e in entries:
        for dep in e.get("dependencies", []):
            if dep not in id_set:
                errs.append(f"{e['id']}: dependency '{dep}' not in catalog")

    if errs:
        print(f"FAIL — {len(errs)} error(s):")
        for x in errs:
            print(f"  - {x}")
        return 1
    for w in warns:
        print(f"  warn: {w}")
    print(f"OK — {len(entries)} entries valid; no port/id/env collisions; retired-registry clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
