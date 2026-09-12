#!/usr/bin/env python3
"""Detect and remediate a wedged nzbdav (InfiniDysk) backend.

Landmine (docs/stack/landmines.md #13): the :3000 frontend can stay healthy while
the internal backend on :8080 dies, leaving the queue API and WebDAV serving
502s. Docker marks the container `unhealthy` (the compose healthcheck probes
both), but `restart: unless-stopped` only fires on *exit* — nothing acts on
`unhealthy`. The 2026-09-05 incident ran like that for hours: the backend
process stayed alive with zero open sockets while node kept 502ing.

This watcher is the actor. It probes the wedge signature from the host (no
docker needed to detect), restarts the container queue-safely when wedged,
and pings Discord on detection and recovery.

Signature — the queue API returns HTTP 200 whenever the backend is up (empty
queue or not), so a failed queue probe with a live frontend *is* the wedge:

  frontend UP   — GET http://localhost:3000/healthz        → 200
  backend DOWN  — GET /api?mode=queue&output=json&apikey=… → 502/refused

Remediation is a queue-safe *restart*, never a recreate: the download queue
persists in /config/db.sqlite across restarts (recreate wipes it — landmine
#4), so queued NZBs survive. The crash-loop guard stops auto-restarting a
dev build that re-wedges faster than it recovers (>= 3 restarts in 30 min),
and escalates instead of thrashing.

Exit codes:
  0  healthy, or fault handled / recovered / already actioned
  1  fault detected but the Discord notification failed
  2  cannot assess (FRONTEND_BACKEND_API_KEY not set)
  3  --check only: wedge signature present (no remediation)
  4  --check only: frontend unreachable (container down/starting)

Usage:
  python3 scripts/watch_nzbdav_backend.py                  # watch (default)
  python3 scripts/watch_nzbdav_backend.py --check          # probe only
  python3 scripts/watch_nzbdav_backend.py --state-file /tmp/w.json

Install as a user timer (every minute):

  # ~/.config/systemd/user/stack-nzbdav-watch.service
  [Unit]
  Description=Bear Cave nzbdav backend wedge watcher
  [Service]
  Type=oneshot
  EnvironmentFile=/home/bear/cave/.env
  ExecStart=/usr/bin/python3 /home/bear/cave/scripts/watch_nzbdav_backend.py

  # ~/.config/systemd/user/stack-nzbdav-watch.timer
  [Unit]
  Description=Bear Cave nzbdav backend wedge watcher (every minute)
  [Timer]
  OnBootSec=1min
  OnUnitActiveSec=1min
  [Install]
  WantedBy=timers.target

  systemctl --user daemon-reload
  systemctl --user enable --now stack-nzbdav-watch.timer
"""

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_STATE = ROOT / ".cache" / "watch-nzbdav" / "state.json"
COMPOSE_DIR = ROOT
FRONTEND_PATH = "/healthz"

API_TIMEOUT = 8  # seconds, light probe
RESTART_WAIT = 90  # seconds to poll for recovery after a restart
POLL_INTERVAL = 5  # seconds between recovery polls
# Crash-loop guard: >= 3 restarts inside this window disables auto-restart.
CRASH_WINDOW = 30 * 60
MAX_RESTARTS = 3
# Skip a new restart if one was triggered within this window (recovery may
# still be in progress; the healthcheck already covers startup).
ACTION_COOLDOWN = 120


def _get(url: str, timeout: int) -> bool:
    """GET and return True only on HTTP 200."""
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            return resp.status == 200
    except (urllib.error.URLError, OSError, ValueError):
        return False


def probe_frontend(base_url: str, timeout: int = API_TIMEOUT) -> bool:
    """True when the :3000 frontend serves /healthz."""
    return _get(f"{base_url.rstrip('/')}{FRONTEND_PATH}", timeout)


def probe_backend(base_url: str, api_key: str, timeout: int = API_TIMEOUT) -> bool:
    """True when the backend is reachable through the frontend queue API.

    HTTP 200 means the backend answered; any 502/refused/timeout means it
    is down. The queue's *contents* are irrelevant here — an empty queue
    still returns 200.
    """
    if not api_key:
        raise RuntimeError("FRONTEND_BACKEND_API_KEY not set")
    url = (f"{base_url.rstrip('/')}/api?mode=queue&output=json&apikey={api_key}")
    return _get(url, timeout)


def classify(frontend_up: bool, backend_up: bool) -> str:
    """Wedge state: healthy | wedged | frontend-down."""
    if frontend_up and backend_up:
        return "healthy"
    if frontend_up and not backend_up:
        return "wedged"
    return "frontend-down"


def load_state(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(state, indent=2))
    tmp.replace(path)


def restarts_in_window(state: dict, now: float, window: int) -> int:
    cutoff = now - window
    return sum(1 for t in state.get("restarts", []) if t >= cutoff)


def should_restart(state: dict, now: float) -> tuple[bool, str]:
    """Queue-safe restart decision with cooldown and crash-loop guard."""
    last_action = state.get("last_action_at", 0)
    if now - last_action < ACTION_COOLDOWN:
        return False, "recent action still in progress (cooldown)"
    if restarts_in_window(state, now, CRASH_WINDOW) >= MAX_RESTARTS:
        return False, "crash loop — >= %d restarts in %ds; escalating" % (
            MAX_RESTARTS, CRASH_WINDOW)
    return True, ""


def restart_nzbdav() -> tuple[bool, str]:
    """Queue-safe `docker compose restart nzbdav` via sudo. Returns
    (success, output-tail). Restart (not recreate) keeps /config/db.sqlite,
    so queued NZBs survive (landmine #4)."""
    try:
        proc = subprocess.run(
            ["sudo", "-n", "docker", "compose", "restart", "nzbdav"],
            cwd=str(COMPOSE_DIR), capture_output=True, text=True, timeout=120,
        )
    except (subprocess.TimeoutExpired, OSError) as exc:
        return False, str(exc)
    if proc.returncode != 0:
        return False, (proc.stderr or proc.stdout).strip()[-400:]
    return True, (proc.stdout or proc.stderr).strip()[-200:]


def wait_for_backend(base_url: str, api_key: str, max_wait: int = RESTART_WAIT) -> bool:
    deadline = time.time() + max_wait
    while time.time() < deadline:
        if probe_backend(base_url, api_key):
            return True
        time.sleep(POLL_INTERVAL)
    return probe_backend(base_url, api_key)


def discord_post(webhook: str, content: str, timeout: int = API_TIMEOUT) -> None:
    """POST one Discord message; raises on any failure."""
    body = json.dumps({"content": content}).encode("utf-8")
    req = urllib.request.Request(
        webhook, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout):
        return None


def watch(args) -> int:
    base_url = args.url
    api_key = os.environ.get("FRONTEND_BACKEND_API_KEY", "")
    webhook = os.environ.get("DISCORD_WEBHOOK_URL", "")
    if not api_key:
        print("Cannot assess: FRONTEND_BACKEND_API_KEY not set", file=sys.stderr)
        return 2

    state = load_state(Path(args.state_file))
    now = time.time()

    frontend_up = probe_frontend(base_url)
    backend_up = probe_backend(base_url, api_key)
    kind = classify(frontend_up, backend_up)

    if kind == "healthy":
        if state.get("state") in ("wedged", "crash-loop"):
            # Episode ended on its own (or our restart won) — record recovery.
            state["state"] = "healthy"
            save_state(Path(args.state_file), state)
            if webhook:
                try:
                    discord_post(webhook, "✅ nzbdav backend is healthy again.")
                except (urllib.error.URLError, OSError) as exc:
                    print(f"Recovery alert failed: {exc}", file=sys.stderr)
                    return 1
        return 0

    if kind == "frontend-down":
        # Container down or mid-restart — not the wedge signature; docker's
        # restart policy and the healthcheck handle startup. Stay quiet.
        return 0

    # --- wedged: frontend up, backend down -----------------------------------
    go, reason = should_restart(state, now)
    prev = state.get("state")

    if not go:
        if prev != "crash-loop" and "crash loop" in reason:
            state["state"] = "crash-loop"
            save_state(Path(args.state_file), state)
            if webhook:
                try:
                    discord_post(webhook, (
                        "🚨 nzbdav backend keeps re-wedging — crash loop detected "
                        f"(≥{MAX_RESTARTS} restarts in {CRASH_WINDOW // 60} min). "
                        "Auto-restart disabled; needs manual triage."))
                except (urllib.error.URLError, OSError) as exc:
                    print(f"Crash-loop alert failed: {exc}", file=sys.stderr)
                    return 1
        return 0

    # First alert for this episode, then restart.
    if prev != "wedged":
        state["state"] = "wedged"
        if webhook:
            try:
                discord_post(webhook, (
                    "🚨 nzbdav backend WEDGED — frontend :3000 up but backend "
                    ":8080 refusing (queue API 502). Restarting container "
                    "(queue-safe restart, downloads preserved)."))
            except (urllib.error.URLError, OSError) as exc:
                print(f"Wedge alert failed: {exc}", file=sys.stderr)
                return 1

    ok, out = restart_nzbdav()
    state["last_action_at"] = now
    state.setdefault("restarts", []).append(now)
    # Keep the restart history bounded.
    state["restarts"] = [t for t in state["restarts"] if t >= now - CRASH_WINDOW]

    if not ok:
        state["state"] = "crash-loop"
        save_state(Path(args.state_file), state)
        print(f"restart failed: {out}", file=sys.stderr)
        if webhook:
            try:
                discord_post(webhook, (
                    "🚨 nzbdav wedge restart FAILED — manual intervention "
                    f"required. {out}"))
            except (urllib.error.URLError, OSError) as exc:
                print(f"Restart-failure alert failed: {exc}", file=sys.stderr)
                return 1
        return 0

    # Poll for recovery.
    recovered = wait_for_backend(base_url, api_key)
    state["state"] = "healthy" if recovered else "wedged"
    save_state(Path(args.state_file), state)

    if recovered and webhook:
        try:
            discord_post(webhook, "✅ nzbdav backend recovered after restart.")
        except (urllib.error.URLError, OSError) as exc:
            print(f"Recovery alert failed: {exc}", file=sys.stderr)
            return 1
    if not recovered and webhook:
        try:
            discord_post(webhook, (
                "⚠️ nzbdav still not serving %ds after watchdog restart — "
                "watching; next check re-evaluates." % int(RESTART_WAIT)))
        except (urllib.error.URLError, OSError) as exc:
            print(f"Still-down alert failed: {exc}", file=sys.stderr)
            return 1
    return 0


def check_only(args) -> int:
    """Probe-only mode: never touches the container, exit code encodes state."""
    api_key = os.environ.get("FRONTEND_BACKEND_API_KEY", "")
    if not api_key:
        print("Cannot assess: FRONTEND_BACKEND_API_KEY not set", file=sys.stderr)
        return 2
    kind = classify(probe_frontend(args.url), probe_backend(args.url, api_key))
    labels = {"healthy": 0, "wedged": 3, "frontend-down": 4}
    print(f"  {kind}")
    return labels[kind]


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--url", default=os.environ.get("NZBDAV_URL", "http://localhost:3000"),
                    help="nzbdav base URL (default: %(default)s)")
    ap.add_argument("--state-file", default=str(DEFAULT_STATE),
                    help="state file path (default: %(default)s)")
    ap.add_argument("--check", action="store_true",
                    help="probe only — no restart, no alerts (exit 0/3/4/2)")
    args = ap.parse_args()

    if args.check:
        return check_only(args)
    return watch(args)


if __name__ == "__main__":
    sys.exit(main())
