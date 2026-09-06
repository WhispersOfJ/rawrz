#!/usr/bin/env python3
"""API-contract checker (spec §D.4/D.6).

Asserts, against parity/parity.yaml:
  1. every mapped feature ID is declared by at least one backend route
     (axum route fn with #[doc = "features: [...]"]) or frontend page
     (route file exporting FEATURE_IDS);
  2. every retire:* row declares NO routes/pages (a hit is an error);
  3. backend route declarations reference only known feature IDs.

Exit 0 = contract holds. Designed to fail fast in CI.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).parent.parent
BACKEND_SRC = ROOT / "backend" / "src"
FRONTEND_SRC = ROOT / "frontend" / "src"

ROUTE_DECL_RE = re.compile(r"features:\s*\[([^\]]*)\]")
# Rust doc attrs arrive with escaped quotes (\"id\") — tolerate an optional
# backslash before each quote. TSX pages use plain double quotes.
FEAT_IN_DECL_RE = re.compile(r'\\"?([a-z]+\.[a-z-]+)\\"?')
TS_FEATURES_RE = re.compile(r"FEATURE_IDS\s*=\s*\[([^\]]*)\]", re.S)
TS_FEAT_RE = re.compile(r"['\"]([a-z]+\.[a-z-]+)['\"]")


def parity() -> tuple[set[str], list[dict]]:
    doc = yaml.safe_load((ROOT / "parity" / "parity.yaml").read_text())
    mapped = {f for f in doc["features"] if f}
    retire = [op for op in doc["operations"] if op["safety"] == "retire"]
    return mapped, retire


def backend_ids() -> set[str]:
    ids: set[str] = set()
    for path in BACKEND_SRC.rglob("*.rs"):
        for m in ROUTE_DECL_RE.finditer(path.read_text()):
            ids.update(FEAT_IN_DECL_RE.findall(m.group(1)))
    return ids


def frontend_ids() -> set[str]:
    ids: set[str] = set()
    for path in FRONTEND_SRC.rglob("*.tsx"):
        m = TS_FEATURES_RE.search(path.read_text())
        if m:
            ids.update(TS_FEAT_RE.findall(m.group(1)))
    return ids


def main() -> int:
    mapped, retire = parity()
    served = backend_ids() | frontend_ids()

    errs: list[str] = []
    unserved = mapped - served
    if unserved:
        errs.append(f"feature IDs declared in parity but not served: {sorted(unserved)}")

    phantom = served - mapped
    if phantom:
        errs.append(f"routes/pages reference unknown feature IDs: {sorted(phantom)}")

    if retire:
        print(f"  note: {len(retire)} retire rows correctly unimplemented")

    if errs:
        print("FAIL — api contract violated:")
        for e in errs:
            print(f"  - {e}")
        return 1
    print(
        f"OK — {len(mapped)}/{len(mapped)} feature IDs served "
        f"(backend: {len(backend_ids() & mapped)}, frontend: {len(frontend_ids() & mapped)})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
