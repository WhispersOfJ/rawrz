#!/usr/bin/env python3
"""DB growth-trend predictor (TODO.md #5).

check_radarr_db_size.py gates the *now*; prune_radarr_db.py remediates
*after*; nothing predicts *when*. This script records a size-history sample
per database and trends the growth rate, turning the bloat incidents into a
calendar entry: "sonarr.db crosses its footprint high-water around 2026-10-14
— schedule stack-sonarr-prune".

Design:

  * Samples append to a small JSONL history (one line per run per DB):
    {"db": ..., "ts": <epoch>, "footprint_bytes": ...}. Stored under
    .cache/db-growth/history.jsonl in the operational checkout — runtime
    state, never committed.
  * Metrics are read through check_radarr_db_size.read_metrics(), the exact
    reader the live gate uses, so the predictor can never disagree with the
    gate about what a DB weighs.
  * The trend is an ordinary least squares slope over (ts, footprint) of the
    retained samples, in bytes/day. A sample is only meaningful after a
    prune/VACUUM resets the file, so history is split into segments at
    downward jumps >10% (a VACUUM drop) and only the current segment trends;
    older segments are kept for forensics but never mask current growth.
  * The prediction answers one question: at the observed rate, when does
    footprint_bytes reach the DB's high-water mark
    (check_radarr_db_size.footprint_default_mb, the same per-app default the
    gate enforces)? Inside --horizon-days == FAIL (schedule a prune),
    otherwise OK; fewer than --min-samples usable samples == WARN (skip).

Exit codes (the check_* family contract):
  0  healthy: slow growth or shrinking — nothing to schedule
  1  predicted crossing within the horizon — schedule the prune now
  2  could not assess (DB or history missing/unreadable — operational skip)

Usage:
  python3 scripts/db_growth_trend.py                 # all three DBs
  python3 scripts/db_growth_trend.py --db config/sonarr/sonarr.db
  python3 scripts/db_growth_trend.py --horizon-days 30 --min-samples 4
  python3 scripts/db_growth_trend.py --json
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_radarr_db_size as checker  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_HISTORY = ROOT / ".cache" / "db-growth" / "history.jsonl"

# The databases the bloat gate watches (same set as maintenance_digest's
# check_db_gates). Label -> repo-relative DB path.
DEFAULT_DBS = (
    ("radarr", "config/radarr/radarr.db"),
    ("sonarr", "config/sonarr/sonarr.db"),
)

# A drop larger than this fraction between consecutive samples is treated as
# a VACUUM/prune reset: the trend history restarts from the smaller sample.
PRUNE_DROP_FRACTION = 0.10

MIN_SAMPLE_SPACING = 30  # seconds; samples closer together are collapsed


def load_history(history_path: Path, db_key: str) -> list[dict]:
    """Parse the JSONL history for one DB. Corrupt lines are skipped (a torn
    last line from a killed run must not blind the predictor forever)."""
    if not history_path.is_file():
        return []
    out = []
    for line in history_path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        if rec.get("db") == db_key and isinstance(rec.get("footprint_bytes"), int):
            out.append(rec)
    out.sort(key=lambda r: r["ts"])
    return out


def current_segment(samples: list[dict],
                    prune_drop_fraction: float = PRUNE_DROP_FRACTION) -> list[dict]:
    """Samples since the last downward jump > prune_drop_fraction (a
    prune/VACUUM reset). Everything before it is history, not trend."""
    if len(samples) < 2:
        return samples
    cut = 0
    for i in range(1, len(samples)):
        prev, cur = samples[i - 1]["footprint_bytes"], samples[i]["footprint_bytes"]
        if prev > 0 and cur < prev * (1.0 - prune_drop_fraction):
            cut = i
    return samples[cut:]


def ols_slope_bytes_per_day(points: list[dict]) -> float | None:
    """Least-squares slope of footprint vs time, bytes/day. None when the
    points cannot define one (fewer than 2, or zero time span)."""
    if len(points) < 2:
        return None
    ts = [p["ts"] for p in points]
    span = ts[-1] - ts[0]
    if span <= 0:
        return None
    n = float(len(points))
    mean_t = sum(ts) / n
    mean_y = sum(p["footprint_bytes"] for p in points) / n
    cov = sum((t - mean_t) * (p["footprint_bytes"] - mean_y) for t, p in zip(ts, points))
    var = sum((t - mean_t) ** 2 for t in ts)
    if var <= 0:
        return None
    return cov / var  # bytes per second; caller scales to days


def predict_crossing(slope_bps: float, current_bytes: int,
                     limit_bytes: int, now: float) -> dict | None:
    """When does footprint reach the high-water at this slope? None when the
    DB is already over (callers treat that as crossing now) or the slope does
    not approach the limit (flat/shrinking/negative)."""
    if slope_bps <= 0:
        return None
    remaining = limit_bytes - current_bytes
    if remaining <= 0:
        return {"days": 0.0, "ts": now}
    seconds = remaining / slope_bps
    return {"days": seconds / 86400.0, "ts": now + seconds}


def assess(samples: list[dict], limit_bytes: int, now: float,
           min_samples: int, horizon_days: float) -> dict:
    """Pure decision layer for one DB: trend + verdict, no I/O.

    Returns {level, message, slope_bytes_per_day, days_to_limit} where level
    follows the check_* contract (ok/fail/warn for rc 0/1/2)."""
    seg = current_segment(samples)
    # Collapse near-duplicate timestamps (rapid repeated runs) so the span
    # check in the slope is meaningful.
    dedup: list[dict] = []
    for s in seg:
        if dedup and s["ts"] - dedup[-1]["ts"] < MIN_SAMPLE_SPACING:
            dedup[-1] = s  # keep the newer of the near-identical pair
            continue
        dedup.append(s)
    if len(dedup) < min_samples:
        return {"level": "warn",
                "message": f"insufficient history ({len(dedup)}/{min_samples} samples in current segment)",
                "slope_bytes_per_day": None, "days_to_limit": None}
    slope_bps = ols_slope_bytes_per_day(dedup)
    if slope_bps is None:
        return {"level": "warn", "message": "history span too short to trend",
                "slope_bytes_per_day": None, "days_to_limit": None}
    slope_bpd = slope_bps * 86400.0
    current = dedup[-1]["footprint_bytes"]
    if current >= limit_bytes:
        return {"level": "fail",
                "message": f"footprint {current} already at/over the "
                           f"{limit_bytes}-byte high-water — prune now",
                "slope_bytes_per_day": slope_bpd, "days_to_limit": 0.0}
    crossing = predict_crossing(slope_bps, current, limit_bytes, now)
    if crossing is None:
        return {"level": "ok",
                "message": f"growth {slope_bpd / (1024 * 1024):+.2f} MiB/day; "
                           "not approaching the high-water mark",
                "slope_bytes_per_day": slope_bpd, "days_to_limit": None}
    days = crossing["days"]
    if days <= horizon_days:
        return {"level": "fail",
                "message": f"at {slope_bpd / (1024 * 1024):+.2f} MiB/day the DB "
                           f"crosses its high-water in {days:.0f} days — "
                           "schedule the prune",
                "slope_bytes_per_day": slope_bpd, "days_to_limit": days}
    return {"level": "ok",
            "message": f"growth {slope_bpd / (1024 * 1024):+.2f} MiB/day; "
                       f"high-water in ~{days:.0f} days (beyond the "
                       f"{horizon_days:.0f}-day horizon)",
            "slope_bytes_per_day": slope_bpd, "days_to_limit": days}


def sample_and_append(db_path: Path, history_path: Path, db_key: str,
                      now: float) -> dict:
    """Read live metrics through the gate's own reader and append one sample.
    Returns the record written."""
    m = checker.read_metrics(db_path)
    rec = {"db": db_key, "ts": int(now), "footprint_bytes": m["footprint_bytes"]}
    history_path.parent.mkdir(parents=True, exist_ok=True)
    with history_path.open("a") as fh:
        fh.write(json.dumps(rec) + "\n")
    return rec


def _gb(n: float) -> str:
    return f"{n / (1024 * 1024 * 1024):.2f} GiB"


def _mb(n: float) -> str:
    return f"{n / (1024 * 1024):.1f} MiB"


def process_db(label: str, db_rel: str, args) -> tuple[int, str, dict | None]:
    """Record today's sample for one DB and assess its trend."""
    db_path = ROOT / db_rel if not args.db else Path(args.db)
    if not db_path.is_file():
        return 2, f"{db_path.name} not found — skipping (fresh install?)", None
    now = time.time()
    try:
        sample_and_append(db_path, args.history, label, now)
        samples = load_history(args.history, label)
    except (OSError, ValueError) as exc:  # pragma: no cover - defensive
        return 2, f"could not sample {db_path.name}: {exc}", None
    limit_mb = checker.footprint_default_mb(db_path.name)
    verdict = assess(samples, int(limit_mb * 1024 * 1024), now,
                     args.min_samples, args.horizon_days)
    tail = f"{db_path.name}: {verdict['message']}"
    return (0 if verdict["level"] == "ok" else
            1 if verdict["level"] == "fail" else 2), tail, verdict


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--db", default=None,
                    help="trend a single DB path instead of the default trio")
    ap.add_argument("--history", type=Path, default=Path(
        os.environ.get("DB_GROWTH_HISTORY", str(DEFAULT_HISTORY))),
        help="JSONL sample history (default: .cache/db-growth/history.jsonl)")
    ap.add_argument("--horizon-days", type=float, default=30.0,
                    help="predicted time to the high-water mark at or below "
                         "which the check fails (default: %(default)s)")
    ap.add_argument("--min-samples", type=int, default=3,
                    help="samples required in the current trend segment "
                         "before a verdict (default: %(default)s)")
    ap.add_argument("--json", action="store_true",
                    help="emit machine-readable JSON instead of text")
    args = ap.parse_args()

    targets = (("custom", args.db),) if args.db else DEFAULT_DBS
    results = []
    for label, db_rel in targets:
        rc, tail, verdict = process_db(label, db_rel, args)
        results.append({"db": db_rel, "rc": rc, "message": tail,
                        "verdict": verdict})

    if args.json:
        # JSON mode prints ONLY the payload — the exit code carries the verdict.
        print(json.dumps({"results": results}, indent=1))
        failed = [r for r in results if r["rc"] == 1]
        if failed:
            return 1
        return 2 if all(r["rc"] == 2 for r in results) else 0

    for r in results:
        print(f"  {r['message']}")
    failed = [r for r in results if r["rc"] == 1]
    if failed:
        names = ", ".join(r["db"] for r in failed)
        print(f"CHECK FAILED: {len(failed)} DB growth trend(s) inside the "
              f"{args.horizon_days:.0f}-day horizon: {names}")
        return 1
    if all(r["rc"] == 2 for r in results):
        print("CHECK SKIPPED: no DB could be assessed")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
