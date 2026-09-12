#!/usr/bin/env python3
"""Check that SHA-pinned workflow actions are still current.

Scans every `uses: owner/repo@<40-hex-sha> # <tag>` line in
.github/workflows/ and compares the pinned SHA against the tag's current
commit via the GitHub API. Reports three problem classes:

  1. drifts   — the tag has moved; the pinned SHA is stale
  2. unpinned — a third-party action referenced by a mutable tag instead
                of a full SHA (policy violation, see docs/stack/ci-cd.md)
  3. errors   — the tag/repo could not be resolved (missing tag, rate
                limit, no `gh` CLI, ...)

Run by .github/workflows/pin-drift-check.yml on a weekly schedule; the
workflow opens or updates a tracking issue whenever anything is reported.

Usage:
  python3 scripts/check_action_pins.py            # human-readable; exit 1 on problems
  python3 scripts/check_action_pins.py --json     # JSON report; always exit 0
"""

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS_DIR = ROOT / ".github" / "workflows"

# Pinned form: uses: owner/repo[/sub]@<40-hex> # <tag>
PINNED_RE = re.compile(
    r"uses:\s+([\w.-]+/[\w.-]+(?:/[\w.-]+)?)@([0-9a-f]{40})\s+#\s*([\w.\-]+)"
)
# Any owner/repo@ref form; pinned matches are excluded afterwards.
UNPINNED_RE = re.compile(r"uses:\s+([\w.-]+/[\w.-]+(?:/[\w.-]+)?)@(\S+)")

SHA_RE = re.compile(r"[0-9a-f]{40}")


def resolve_tag_sha(repo: str, tag: str) -> tuple[str | None, str | None]:
    """Return (current_sha, error) for repo@tag via `gh api`.

    sha is None when the tag could not be resolved; error describes why.
    """
    if shutil.which("gh") is None:
        return None, "gh CLI not installed"
    try:
        proc = subprocess.run(
            ["gh", "api", f"repos/{repo}/commits/{tag}", "--jq", ".sha"],
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return None, "GitHub API timeout"
    except subprocess.CalledProcessError as exc:
        stderr = exc.stderr.strip()
        detail = stderr.splitlines()[-1][:200] if stderr else f"API error (exit {exc.returncode})"
        return None, detail
    sha = proc.stdout.strip()
    if not SHA_RE.fullmatch(sha):
        return None, f"unexpected response: {sha[:60]!r}"
    return sha, None


def main() -> int:
    pinned: list[dict[str, object]] = []
    unpinned: list[dict[str, object]] = []

    if WORKFLOWS_DIR.is_dir():
        for wf in sorted(WORKFLOWS_DIR.glob("*.yml")):
            for line_no, line in enumerate(wf.read_text().splitlines(), start=1):
                for match in PINNED_RE.finditer(line):
                    pinned.append(
                        {
                            "file": wf.name,
                            "line": line_no,
                            "ref": match.group(1),
                            "tag": match.group(3),
                            "pinned": match.group(2),
                        }
                    )
                for match in UNPINNED_RE.finditer(line):
                    ref, tag = match.group(1), match.group(2)
                    # Skip local actions (./.github/...) and already-pinned refs.
                    if ref.startswith(".") or SHA_RE.fullmatch(tag):
                        continue
                    unpinned.append(
                        {"file": wf.name, "line": line_no, "ref": ref, "tag": tag}
                    )

    # Resolve each unique owner/repo@tag once (sub-actions share the tag's commit).
    cache: dict[str, tuple[str | None, str | None]] = {}
    drifts: list[dict[str, object]] = []
    errors: list[dict[str, object]] = []
    seen_errors: set[str] = set()

    for entry in pinned:
        ref = str(entry["ref"])
        tag = str(entry["tag"])
        repo = "/".join(ref.split("/")[:2])
        key = f"{repo}@{tag}"
        if key not in cache:
            cache[key] = resolve_tag_sha(repo, tag)
        current, err = cache[key]
        if err is not None:
            if key not in seen_errors:
                seen_errors.add(key)
                errors.append({"ref": key, "error": err})
        elif current != str(entry["pinned"]):
            drifts.append(
                {
                    "ref": ref,
                    "tag": tag,
                    "pinned": entry["pinned"],
                    "current": current,
                }
            )

    report = {
        "checked": len(pinned),
        "drifts": drifts,
        "unpinned": unpinned,
        "errors": errors,
        "ok": not (drifts or unpinned or errors),
    }

    if "--json" in sys.argv:
        print(json.dumps(report, indent=2))
        return 0

    if report["ok"]:
        print(
            f"OK: {len(pinned)} pinned action(s), all match their tags' current commits."
        )
        return 0

    print(
        f"CHECK FAILED: {len(pinned)} pinned action(s) — "
        f"{len(drifts)} drifted, {len(unpinned)} unpinned, {len(errors)} resolution error(s)."
    )
    for drift in drifts:
        print(
            f"  [drift] {drift['ref']}@{drift['tag']}: "
            f"pinned {drift['pinned']} != current {drift['current']}"
        )
    for item in unpinned:
        print(
            f"  [unpinned] {item['ref']}@{item['tag']} "
            f"({item['file']}:{item['line']}) — not a full SHA"
        )
    for error in errors:
        print(f"  [error] {error['ref']}: {error['error']}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
