#!/usr/bin/env python3
"""Fail when the nzbdav (InfiniDysk) download queue is non-empty.

The landmine (docs/stack/landmines.md, docs/operations/backup-restore.md):
recreating the nzbdav container wipes the in-memory queue and silently
blocklists every queued NZB. scripts/update-nzbdav.sh guards the
*intended* update path, but a bare `docker compose up -d nzbdav` or
`docker compose restart nzbdav` bypasses that script and silently
destroys queued work. This check sits at the recreate boundary so the
guard fires regardless of how the recreate was invoked.

The queue is queried via the same SABnzbd-compatible API the exporter
uses (`mode=queue&output=json`, keyed by FRONTEND_BACKEND_API_KEY), and
the slot count is compared against a configurable threshold (default 0:
any queued item blocks the recreate). CI runs this against a stubbed
response (see tests/check_nzbdav_queue/); the live stack run is
preflight-gated on nzbdav being reachable.

Exit codes:
  0  queue is at/under threshold (safe to recreate)
  1  queue exceeds threshold (recreate would destroy queued work)
  2  nzbdav unreachable / API error / config missing (preflight skips,
     but a live recreate should abort — set --allow-unreachable to treat
     as safe)

Run by scripts/preflight.sh and .github/workflows/validate.yml (offline
mode), and by scripts/nzbdav-safe-recreate.sh before any recreate.

Usage:
  python3 scripts/check_nzbdav_queue.py
  python3 scripts/check_nzbdav_queue.py --threshold 0
  python3 scripts/check_nzbdav_queue.py --url http://localhost:3000 --api-key "$KEY"
  python3 scripts/check_nzbdav_queue.py --offline   # CI: validate against a fixture
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

DEFAULT_URL = "http://localhost:3000"
DEFAULT_TIMEOUT = 15


def _queue_url(base: str, api_key: str) -> str:
    return f"{base.rstrip('/')}/api?mode=queue&output=json&apikey={api_key}"


def fetch_queue(base_url: str, api_key: str, timeout: int) -> dict:
    """Fetch and parse the queue API response. Raises on any failure."""
    if not api_key:
        raise RuntimeError("FRONTEND_BACKEND_API_KEY not set")
    req = urllib.request.Request(_queue_url(base_url, api_key))
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def queue_depth(data: dict) -> int:
    """Count queued items from a parsed queue response.

    SABnzbd's queue response has queue.slots (list) or queue.noofslots
    (int) depending on version; honor both. Returns -1 if unparseable.
    """
    q = data.get("queue", {}) if isinstance(data, dict) else {}
    slots = q.get("slots")
    if isinstance(slots, list):
        return len(slots)
    if isinstance(slots, int):
        return slots
    noof = q.get("noofslots")
    if isinstance(noof, int):
        return noof
    return -1


def check(base_url: str, api_key: str, timeout: int, threshold: int,
          allow_unreachable: bool) -> tuple[int, str]:
    """Run the check. Returns (exit_code, message)."""
    try:
        data = fetch_queue(base_url, api_key, timeout)
    except (urllib.error.URLError, RuntimeError, json.JSONDecodeError, OSError) as exc:
        msg = f"nzbdav queue API unreachable: {exc}"
        if allow_unreachable:
            return 0, f"OK (skipped — unreachable, --allow-unreachable): {msg}"
        return 2, msg

    depth = queue_depth(data)
    if depth < 0:
        return 2, "could not parse queue API response"

    if depth > threshold:
        return 1, (
            f"queue is NOT empty ({depth} item(s) > threshold {threshold}). "
            "Recreating nzbdav would wipe queued NZBs and blocklist them. "
            "Wait for downloads to finish, clear the queue, or re-run with "
            "--force if you accept the data loss."
        )
    return 0, f"queue is empty ({depth} item(s) <= threshold {threshold}) — safe to recreate"


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--url", default=os.environ.get("NZBDAV_URL", DEFAULT_URL),
                    help="nzbdav base URL (default: %(default)s)")
    ap.add_argument("--api-key", default=os.environ.get("FRONTEND_BACKEND_API_KEY", ""),
                    help="API key (default: $FRONTEND_BACKEND_API_KEY)")
    ap.add_argument("--threshold", type=int, default=0,
                    help="max queued items allowed (default: %(default)s)")
    ap.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT,
                    help="HTTP timeout seconds (default: %(default)s)")
    ap.add_argument("--allow-unreachable", action="store_true",
                    help="treat an unreachable nzbdav as safe (CI/offline)")
    ap.add_argument("--offline", action="store_true",
                    help="CI mode: skip the live check entirely (exit 0)")
    args = ap.parse_args()

    if args.offline:
        print("OK (offline mode — live queue check skipped)")
        return 0

    code, msg = check(args.url, args.api_key, args.timeout,
                      args.threshold, args.allow_unreachable)
    prefix = {0: "PASS", 1: "FAIL", 2: "SKIP"}[code]
    print(f"  {prefix}  nzbdav queue: {msg}")
    return code


if __name__ == "__main__":
    sys.exit(main())
