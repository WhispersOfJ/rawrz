#!/usr/bin/env python3
"""Audit the stack for retired-service and dead-path residue (TODO.md #2).

The automated form of the exhaustive-removal checklist (AGENTS.md landmine
#7): scans every surface a retirement touches for references to retired
services and dead paths, so removal residue is caught mechanically instead of
by hand (the 2026-09-02 session found media-stack cron entries, a
node-exporter-era stack-health-metrics timer, and poster/letterboxd syncs
into the retired /home/bear/Claude/media-stack project by manual grep).

Surfaces scanned:

  repo (always):
    docker-compose.yml            non-comment lines mentioning a retired name
    .env.template                 variables with a retired-service prefix
                                  (e.g. LIDARR_, GRAFANA_, WS_) — derived from
                                  the registry, so no hand-maintained list to
                                  drift
    .github/workflows/*.yml       retired names (re-adoption watcher
                                  workflows are exempt: cleanuparr)
    services/bash-functions/      retired names / dead project paths in
                                  function code and comments
    docs/** filenames             a retired-named page is residue (the
                                  migration record is exempt); retirement
                                  *records* (lifecycle.md, FISH.md) never
                                  match because only filenames are compared

  host (skipped with --repo-only; unavailable -> WARN, never FAIL):
    crontab                       retired names / dead paths in active lines
    ~/.config/systemd/user        unit files whose name or ExecStart /
                                  Description references a retired service or
                                  dead project path

The registry mirrors docs/stack/services/lifecycle.md: the "Retired services"
table is parsed and every row must exist in RETIRED_SERVICES below, so
recording a retirement without teaching this checker its name fails CI
(run the offline test for the exact rule). archive/ holds retired sources by
design and is never scanned. Deliberate in-code mentions can be exempted
with a trailing `audit-residue-ignore` marker on the line.

Exit codes:
  0  no residue (host surfaces unavailable count as WARN, not FAIL)
  1  residue found
  2  not a stack checkout (docker-compose.yml missing)

Usage:
  python3 scripts/audit_residue.py              # repo + host surfaces
  python3 scripts/audit_residue.py --repo-only  # CI / preflight regression
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIFECYCLE_DOC = ROOT / "docs" / "services" / "lifecycle.md"
COMPOSE = ROOT / "docker-compose.yml"
ENV_TEMPLATE = ROOT / ".env.template"
WORKFLOWS = ROOT / ".github" / "workflows"
FUNCTIONS = ROOT / "services" / "bash-functions"
DOCS = ROOT / "docs"

# ---------------------------------------------------------------------------
# Registry — keep in sync with docs/stack/services/lifecycle.md ("Retired services")
# ---------------------------------------------------------------------------
# value = True when the service has an active re-adoption watcher (its name is
# expected inside the watcher workflow, which is exempted by WATCHER_FILES).
RETIRED_SERVICES: dict[str, bool] = {
    "traefik": False, "loki": False, "promtail": False, "grafana": False,
    "prometheus": False, "alertmanager": False, "node-exporter": False,
    "cadvisor": False, "nzbdav-exporter": False, "arr-dashboard": False,
    "landing-page": False, "metacache": False, "lidarr": False, "readarr": False,
    "audiobookshelf": False, "komga": False, "adguard": False,
    "crowdsec": False, "vaultwarden": False, "watchstate": False,
    "cleanuparr": True, "uptime-kuma": False, "n8n": False, "control-panel": False,
    "bazarr": False,
}  # Bazarr re-retired 2026-09-06 (returned to this registry + lifecycle table)

# Retired *project* paths (the merged-source project roots). Reference to one
# of these anywhere operational is residue — nothing runs from them anymore.
DEAD_PATHS = (
    "/home/bear/Claude/media-stack",
    "/home/bear/Claude/metacacharr",
)
DEAD_TOKENS = ("media-stack", "metacacharr")  # basename tokens for file/line matches

# Extra env-var prefixes beyond the service-name-derived ones (e.g. watchstate
# shipped as WS_*). Service-derived prefixes are computed at startup.
EXTRA_ENV_PREFIXES = ("WS_",)

# Workflow files that legitimately discuss a retired service (re-adoption
# watchers) and are exempt from the workflow text scan.
WATCHER_FILES = {"cleanuparr-sabnzbd-watch.yml"}

# docs/ filenames that legitimately name a retired project (migration record).
DOC_EXEMPT = {"docs/migration/from-media-stack.md"}

# A line containing this marker is a deliberate mention, never residue.
IGNORE_MARKER = "audit-residue-ignore"


def _env_prefixes() -> tuple[str, ...]:
    prefixes = []
    for name in RETIRED_SERVICES:
        norm = re.sub(r"[^A-Za-z0-9]", "_", name).upper()
        prefixes.append(f"{norm}_")
    return tuple(sorted(set(prefixes + list(EXTRA_ENV_PREFIXES))))


ENV_PREFIXES = _env_prefixes()
TOKEN_RE = re.compile(
    r"\b(" + "|".join(sorted(set(RETIRED_SERVICES) | set(DEAD_TOKENS))) + r")\b",
    re.IGNORECASE,
)

# ---------------------------------------------------------------------------
# Findings
# ---------------------------------------------------------------------------

class Finding:
    """One audit line. level: 'ok' | 'warn' | 'fail'."""

    def __init__(self, surface: str, level: str, message: str):
        self.surface = surface
        self.level = level
        self.message = message

    def render(self) -> str:
        tag = {"ok": "OK  ", "warn": "WARN", "fail": "FAIL"}[self.level]
        return f"  {tag}  {self.surface:<12} {self.message}"

    def __repr__(self) -> str:  # pragma: no cover - debug aid
        return f"Finding({self.surface!r}, {self.level!r}, {self.message!r})"


def _finding(surface: str, level: str, path: Path, line: int,
             detail: str) -> Finding:
    loc = path.relative_to(ROOT) if path.is_absolute() else path
    return Finding(surface, level, f"{loc}:{line}: {detail}")


# ---------------------------------------------------------------------------
# Lifecycle registry sync (docs/stack/services/lifecycle.md is the record)
# ---------------------------------------------------------------------------

RETIRED_TABLE_RE = re.compile(r"^\| `([^`]+)`", re.MULTILINE)


def lifecycle_names(doc_text: str) -> set[str]:
    """Service names recorded in lifecycle.md's 'Retired services' table."""
    return {m.group(1) for m in RETIRED_TABLE_RE.finditer(doc_text)}


def check_registry_sync(doc_text: str) -> list[Finding]:
    recorded = lifecycle_names(doc_text)
    known = set(RETIRED_SERVICES)
    out = []
    for name in sorted(recorded - known):
        out.append(Finding("registry", "fail",
                           f"lifecycle.md records `{name}` as retired but "
                           "scripts/audit_residue.py does not know it — add it "
                           "to RETIRED_SERVICES"))
    for name in sorted(known - recorded):
        out.append(Finding("registry", "warn",
                           f"audit registry knows `{name}` but lifecycle.md no "
                           "longer records it (re-adopted? remove from "
                           "RETIRED_SERVICES)"))
    return out


# ---------------------------------------------------------------------------
# Repo surface scans (pure text/line logic, offline-testable)
# ---------------------------------------------------------------------------

def scan_lines(surface: str, text: str, path: Path,
               ignore_comments: bool = False) -> list[Finding]:
    """Flag lines whose content mentions a retired name or dead path.

    Lines containing IGNORE_MARKER are deliberate mentions. With
    ignore_comments, comment lines (leading #) are skipped — compose comments
    legitimately reference retirement history ("no Traefik", lifecycle link).
    """
    out = []
    for lineno, raw in enumerate(text.splitlines(), start=1):
        stripped = raw.strip()
        if not stripped or IGNORE_MARKER in stripped:
            continue
        if ignore_comments and stripped.startswith("#"):
            continue
        dead = next((d for d in DEAD_PATHS if d in raw), None)
        if dead:  # a dead-path hit subsumes its token (media-stack etc.)
            out.append(_finding(surface, "fail", path, lineno,
                                f"dead path {dead}"))
            continue
        m = TOKEN_RE.search(raw)
        if m:
            out.append(_finding(surface, "fail", path, lineno,
                                f"mentions retired `{m.group(1).lower()}`"))
    return out


def env_var_is_retired(var: str) -> bool:
    """True when an env var name carries a retired-service prefix (LIDARR_,
    GRAFANA_, WS_, ...)."""
    return var.startswith(ENV_PREFIXES)


def doc_name_is_retired(name: str) -> bool:
    """True when a docs filename names a retired service (a retired-named page
    is residue; the retirement *records* use other filenames and never trip)."""
    return TOKEN_RE.search(name) is not None


def scan_env_template() -> list[Finding]:
    text = ENV_TEMPLATE.read_text() if ENV_TEMPLATE.is_file() else ""
    out = []
    for lineno, raw in enumerate(text.splitlines(), start=1):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        m = re.match(r"^([A-Z0-9_]+)=", stripped)
        if m and env_var_is_retired(m.group(1)):
            out.append(_finding("env template", "fail", ENV_TEMPLATE, lineno,
                                f"retired variable {m.group(1)}"))
    return out


def scan_doc_filenames() -> list[Finding]:
    out = []
    if not DOCS.is_dir():
        return out
    for path in sorted(DOCS.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(ROOT).as_posix()
        if rel in DOC_EXEMPT:
            continue
        if doc_name_is_retired(path.name):
            out.append(Finding("docs", "fail",
                               f"{rel}: retired-named page ({path.name})"))
    return out


# ---------------------------------------------------------------------------
# Host surface scans (crontab, user units)
# ---------------------------------------------------------------------------

def scan_crontab(text: str) -> list[Finding]:
    """Flag active crontab lines (comments/blank skipped) referencing residue."""
    out = []
    for lineno, raw in enumerate(text.splitlines(), start=1):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#") or IGNORE_MARKER in stripped:
            continue
        dead = next((d for d in DEAD_PATHS if d in stripped), None)
        if dead:
            out.append(Finding("crontab", "fail",
                               f"line {lineno}: dead path {dead}"))
            continue
        m = TOKEN_RE.search(stripped)
        if m:
            out.append(Finding("crontab", "fail", f"line {lineno}: "
                             f"mentions retired `{m.group(1).lower()}`"))
    return out


def scan_user_units(unit_dir: Path, enabled: set[str]) -> list[Finding]:
    """Flag user unit files (name or ExecStart/Description) referencing
    residue. `enabled` = basenames with an active .wants symlink (any target);
    enabled units are the ones that actually start, so they are called out.

    Reasons are deduplicated across the file ("dead path X (3 lines)" once,
    not once per line); a dangling symlink (ENOENT) is skipped silently — it
    is broken-unit hygiene, not retired residue."""
    out = []
    if not unit_dir.is_dir():
        return out
    unit_files = sorted(unit_dir.glob("*.service")) + sorted(unit_dir.glob("*.timer"))
    for path in unit_files:
        name = path.name
        try:
            text = path.read_text()
        except OSError as exc:
            if exc.errno == 2:  # dangling symlink — not retired residue
                continue
            out.append(Finding("user units", "warn", f"{name}: unreadable: {exc}"))
            continue
        dead_lines: dict[str, list[int]] = {}   # dead path -> line numbers
        token_lines: dict[str, list[int]] = {}  # token -> line numbers
        name_match = TOKEN_RE.search(name)
        for lineno, raw in enumerate(text.splitlines(), start=1):
            if IGNORE_MARKER in raw:
                continue
            for dead in DEAD_PATHS:
                if dead in raw:
                    dead_lines.setdefault(dead, []).append(lineno)
            m = TOKEN_RE.search(raw)
            if m:
                token_lines.setdefault(m.group(1).lower(), []).append(lineno)
        reasons = []
        if name_match:
            reasons.append(f"name matches retired `{name_match.group(1).lower()}`")
        for token, lines in sorted(token_lines.items()):
            reasons.append(f"mentions retired `{token}` ({len(lines)} "
                           f"line{'s' if len(lines) != 1 else ''})")
        for dead, lines in sorted(dead_lines.items()):
            reasons.append(f"dead path {dead} ({len(lines)} "
                           f"line{'s' if len(lines) != 1 else ''})")
        if reasons:
            state = "enabled" if name in enabled else "inert (not enabled)"
            out.append(Finding("user units", "fail",
                               f"{name} [{state}]: {'; '.join(reasons)}"))
    return out


def enabled_unit_names(unit_dir: Path) -> set[str]:
    """Basenames symlinked from any *.wants directory (the units that start)."""
    enabled: set[str] = set()
    if not unit_dir.is_dir():
        return enabled
    for wants in unit_dir.glob("*.wants"):
        for link in wants.iterdir():
            enabled.add(link.name)
    return enabled


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo-only", action="store_true",
                    help="skip host surfaces (crontab, user units) — for CI "
                         "and preflight regression")
    ap.add_argument("--crontab", help="override crontab source (testing)")
    ap.add_argument("--user-units", help="override user unit dir (testing)")
    args = ap.parse_args()

    if not COMPOSE.is_file():
        print(f"not a stack checkout (no {COMPOSE.name}); run from the repo root.")
        return 2

    findings: list[Finding] = []

    # Registry must know every retirement lifecycle.md records.
    if LIFECYCLE_DOC.is_file():
        findings += check_registry_sync(LIFECYCLE_DOC.read_text())

    # Repo surfaces.
    findings += scan_lines("compose", COMPOSE.read_text(), COMPOSE,
                           ignore_comments=True)
    findings += scan_env_template()
    if WORKFLOWS.is_dir():
        for path in sorted(WORKFLOWS.glob("*.yml")):
            if path.name in WATCHER_FILES:
                continue
            findings += scan_lines("workflows", path.read_text(), path)
    if FUNCTIONS.is_dir():
        for path in sorted(FUNCTIONS.rglob("*.sh")):
            findings += scan_lines("functions", path.read_text(), path)
    findings += scan_doc_filenames()

    # Host surfaces (degrade to WARN when unavailable, never FAIL).
    if not args.repo_only:
        unit_dir = Path(args.user_units) if args.user_units else (
            Path.home() / ".config" / "systemd" / "user")
        enabled = enabled_unit_names(unit_dir)
        findings += scan_user_units(unit_dir, enabled)
        if args.crontab is not None:
            findings += scan_crontab(args.crontab)
        else:
            try:
                proc = subprocess.run(["crontab", "-l"], capture_output=True,
                                      text=True, timeout=15)
                if proc.returncode == 0:
                    findings += scan_crontab(proc.stdout)
                else:
                    findings.append(Finding("crontab", "warn",
                                            "unavailable (crontab -l failed)"))
            except (OSError, subprocess.TimeoutExpired):
                findings.append(Finding("crontab", "warn",
                                        "unavailable (no crontab on this host)"))
    else:
        findings.append(Finding("crontab", "ok", "host surfaces skipped (--repo-only)"))
        findings.append(Finding("user units", "ok",
                                "host surfaces skipped (--repo-only)"))

    print("Retired-residue audit")
    for f in findings:
        print(f.render())

    failed = [f for f in findings if f.level == "fail"]
    if failed:
        print(f"AUDIT FAIL: {len(failed)} residue finding(s); see lines above")
        return 1
    print("AUDIT OK: no retired-service or dead-path residue found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
