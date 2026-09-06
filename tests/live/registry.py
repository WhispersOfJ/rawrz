#!/usr/bin/env python3
"""Registry CLI backing tests/live/run_safe_matrix.sh (D11 safe-command matrix).

Subcommands:
  --list                     print safe + excluded rows (human view)
  --verify FUNCTIONS_YAML    drift gates: registry names must exist in
                             spec/functions.yaml; no duplicates; no shell
                             metacharacters in argv; excluded rows need reasons
  --resolve NAME             verdict for a row: "run" or "pending"
  --row-rc NAME              comma-separated allowed exit codes (default "0")
  --rows                     TSV for the runner:
                             name<TAB>family<TAB>args<TAB>surface<TAB>expect<TAB>timeout
"""

from __future__ import annotations

import argparse
import re
import sys

import yaml

# Conservative argv charset: the runner splices argv verbatim into a shell
# command string, so the verify gate rejects anything shell-relevant.
ARGV_TOKEN = re.compile(r"^[A-Za-z0-9_@%+=:,./-]+$")

TIMEOUT_MAP = {"light": 120, "none": 120, "mutate": 180, "heavy": 420}
DEFAULT_TIMEOUT = 120


def load(path: str):
    with open(path, encoding="utf-8") as fh:
        return yaml.safe_load(fh) or {}


def fail(msg: str) -> None:
    print(f"registry error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def cmd_list(reg: dict) -> int:
    print("safe rows (executed once per shell):")
    for row in reg.get("safe") or []:
        status = row.get("status", "active")
        rc = ",".join(str(c) for c in row.get("rc_ok", [0]))
        print(f"  {row['name']:34s} {row.get('family', ''):10s} {status:8s} "
              f"rc_ok={rc:6s} argv={row.get('argv', [])}")
    print("\nexcluded rows (recorded, never executed):")
    for row in reg.get("excluded") or []:
        print(f"  {row['name']:34s} {row.get('reason', '(no reason recorded)')}")
    return 0


def cmd_verify(reg: dict, functions_yaml: str) -> int:
    spec = load(functions_yaml)
    known = {f["name"] for f in spec.get("functions") or []}
    errors: list[str] = []

    # Duplicate policy: within-section duplicates are always errors. A name
    # may appear in BOTH sections only under the narrow-mode pattern — the
    # safe row is one restricted read-only argv mode and the excluded row
    # bans the other (mutating) modes via excluded_args.
    safe_rows = reg.get("safe") or []
    excl_rows = reg.get("excluded") or []
    excl_by_name: dict[str, list[dict]] = {}
    for row in excl_rows:
        excl_by_name.setdefault(row["name"], []).append(row)

    safe_names = [r["name"] for r in safe_rows]
    excl_names = [r["name"] for r in excl_rows]
    seen: dict[str, str] = {}
    for section, names in (("safe", safe_names), ("excluded", excl_names)):
        for name in names:
            if name in seen:
                if seen[name] == section:
                    errors.append(f"{name}: duplicate row within '{section}'")
                elif section == "excluded" and \
                        any(x.get("excluded_args") for x in excl_by_name[name]):
                    pass  # narrow-mode pattern; the argv-overlap gate below holds the line
                else:
                    errors.append(f"{name}: listed in both 'safe' and 'excluded' "
                                  f"without excluded_args on the excluded row")
            seen[name] = section
    for row in safe_rows:
        for xrow in excl_by_name.get(row["name"], []):
            if not xrow.get("excluded_args"):
                continue  # already reported above
            overlap = set(str(a) for a in row.get("argv") or []) \
                & set(str(a) for a in xrow["excluded_args"])
            if overlap:
                errors.append(f"{row['name']}: safe argv {sorted(overlap)} "
                              f"intersects excluded_args — narrow the safe row")

    # registry back-reference
    for section, names in (("safe", safe_names), ("excluded", excl_names)):
        for name in names:
            if name not in known:
                errors.append(f"{name}: no spec/functions.yaml row (add one first)")

    # safe row shape
    for row in reg.get("safe") or []:
        name = row["name"]
        if not row.get("family"):
            errors.append(f"{name}: missing family")
        if not row.get("surfaces"):
            errors.append(f"{name}: missing surfaces (preflight needs one)")
        for token in row.get("argv") or []:
            if not ARGV_TOKEN.match(str(token)):
                errors.append(f"{name}: argv token {token!r} contains shell "
                              f"metacharacters — the runner splices argv verbatim")
        for code in row.get("rc_ok") or [0]:
            if not isinstance(code, int) or not 0 <= code <= 255:
                errors.append(f"{name}: invalid rc_ok entry {code!r}")

    # excluded row shape
    for row in reg.get("excluded") or []:
        if not row.get("reason"):
            errors.append(f"{row['name']}: excluded rows must record a reason")

    if errors:
        for err in errors:
            print(f"  DRIFT: {err}", file=sys.stderr)
        return 1
    print(f"registry OK: {len(safe_names)} safe rows, "
          f"{len(excl_names)} excluded rows, all back-referenced to "
          f"spec/functions.yaml")
    return 0


def find_row(reg: dict, name: str) -> dict | None:
    for row in reg.get("safe") or []:
        if row["name"] == name:
            return row
    return None


def cmd_resolve(reg: dict, name: str) -> int:
    row = find_row(reg, name)
    if row is None:
        fail(f"{name}: not a safe row")
    print("pending" if row.get("status") == "pending" else "run")
    return 0


def cmd_row_rc(reg: dict, name: str) -> int:
    row = find_row(reg, name)
    if row is None:
        fail(f"{name}: not a safe row")
    print(",".join(str(c) for c in row.get("rc_ok") or [0]))
    return 0


def cmd_rows(reg: dict) -> int:
    for row in reg.get("safe") or []:
        timeout = TIMEOUT_MAP.get(row.get("timeout") or "", DEFAULT_TIMEOUT)
        expect = row.get("expect", "")
        args = " ".join(str(a) for a in row.get("argv") or [])
        # '-' placeholders keep TSV columns aligned for empty fields (bash
        # `read` collapses runs of IFS whitespace — including tabs — which
        # would shift every column left).
        print("\t".join((
            row["name"],
            row.get("family", "-"),
            args if args else "-",
            ",".join(row.get("surfaces") or []) or "-",
            expect if expect else "-",
            str(timeout),
            "xfail" if row.get("known_issue") else "-",
        )))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("registry", help="path to safe_command_registry.yaml")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--verify", metavar="FUNCTIONS_YAML")
    parser.add_argument("--resolve", metavar="NAME")
    parser.add_argument("--row-rc", metavar="NAME")
    parser.add_argument("--rows", action="store_true")
    args = parser.parse_args()

    reg = load(args.registry)
    if args.list:
        return cmd_list(reg)
    if args.verify:
        return cmd_verify(reg, args.verify)
    if args.resolve:
        return cmd_resolve(reg, args.resolve)
    if args.row_rc:
        return cmd_row_rc(reg, args.row_rc)
    if args.rows:
        return cmd_rows(reg)
    parser.error("pick one of --list/--verify/--resolve/--row-rc/--rows")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
