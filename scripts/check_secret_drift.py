#!/usr/bin/env python3
"""Check that secrets never drift into the git tree.

Backs up the SECURITY.md / docs/stack/security.md promise that secrets are never
committed. Fails when a sensitive path is *tracked by git*, or when the
.gitignore rules protecting it are removed (which would let an accidental
`git add -A` stage it). Guarded surfaces:

  * `.env` and its local variants (`.env.local`, `.env.*.local`)
  * the `secrets/` directory
  * the rclone remote-credentials file (`config/nzbdav-rclone/rclone.conf`)
  * app database files under `config/` (`config/*/*.db*`, incl. WAL/SHM and
    backup variants) — the *arr/NzbDAV DBs contain credentials and state

Only queries git (read-only). In CI the checkout is a fresh clone, so a
tracked sensitive file means it reached the repo tree; on the host, an
untracked-but-present `.env` is the expected setup and does not fail.
`.env.template` and archive `.env.example` are committed by design and are
not guarded.

Exit codes: 0 clean, 1 drift found, 2 not a git repository (skip).
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (pathspec for the tracked check — globs allowed, ignore probe, label)
SENSITIVE_SPECS = [
    (".env", ".env", ".env"),
    (".env.local", ".env.local", ".env.local"),
    (".env.*.local", ".env.staging.local", ".env.*.local variants"),
    ("secrets/", "secrets/", "secrets/"),
    ("config/nzbdav-rclone/rclone.conf",
     "config/nzbdav-rclone/rclone.conf",
     "config/nzbdav-rclone/rclone.conf"),
    ("config/*/*.db*", "config/radarr/radarr.db",
     "config/<app> database files (config/*/*.db*)"),
]


def is_git_repo(repo_root: Path = ROOT) -> bool:
    """True when repo_root is inside a git working tree."""
    r = subprocess.run(
        ["git", "-C", str(repo_root), "rev-parse", "--is-inside-work-tree"],
        capture_output=True, text=True,
    )
    return r.returncode == 0 and r.stdout.strip() == "true"


def tracked_files(repo_root: Path, pathspec: str) -> list[str]:
    """Tracked files matching the given pathspec (empty when none)."""
    r = subprocess.run(
        ["git", "-C", str(repo_root), "ls-files", "--", pathspec],
        capture_output=True, text=True,
    )
    return [line for line in r.stdout.splitlines() if line]


def is_ignored(repo_root: Path, path: str) -> bool:
    """True when the path is covered by a gitignore rule (git check-ignore)."""
    r = subprocess.run(
        ["git", "-C", str(repo_root), "check-ignore", "-q", "--", path],
        capture_output=True, text=True,
    )
    return r.returncode == 0


def assess(repo_root: Path = ROOT) -> list[str]:
    """Return a list of drift errors; empty means the tree is clean."""
    errors: list[str] = []
    for pathspec, probe, label in SENSITIVE_SPECS:
        tracked = tracked_files(repo_root, pathspec)
        if tracked:
            errors.append(f"{label} is tracked by git (committed secrets): {', '.join(tracked)}")
    for _, probe, label in SENSITIVE_SPECS:
        if not is_ignored(repo_root, probe):
            errors.append(f"{label} is not covered by .gitignore (accidental staging risk)")
    return errors


def main() -> int:
    if not is_git_repo():
        print(f"[skip] {ROOT} is not a git working tree; nothing to check.")
        return 2

    errors = assess()
    if errors:
        for error in errors:
            print(f"  [error] {error}")
        print(f"{len(errors)} secret-drift error(s) — fix before merging.")
        return 1

    print("OK: .env variants, secrets/, rclone.conf, and config/ DBs are untracked and gitignored.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
