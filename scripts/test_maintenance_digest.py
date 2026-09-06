#!/usr/bin/env python3
"""Offline unit tests for scripts/maintenance_digest.py.

Exercises the pure decision logic (reclaim-log freshness, failed-unit
parsing, dotfiles-ahead semantics via a git-less fake) without systemd,
git remotes, or the live stack. The DB/queue delegations are tested at the
finding-classification level by faking run_script output.

Run:  python3 scripts/test_maintenance_digest.py
"""

import datetime as dt
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import maintenance_digest as md  # noqa: E402


def fake_reclaim_log(mtime: dt.datetime) -> Path:
    tmp = tempfile.mkdtemp()
    p = Path(tmp) / ".stack-disk-reclaim.log"
    p.write_text("Total reclaimed space: 1.2 GB\n")
    os.utime(p, (mtime.timestamp(), mtime.timestamp()))
    return p


def fake_prune_log(mtime: dt.datetime, exit_code: int | None = 0,
                   body: str = "VERIFIED: 1134 -> 750 MiB; integrity ok.") -> Path:
    tmp = tempfile.mkdtemp()
    p = Path(tmp) / ".sonarr-prune.log"
    stamp = mtime.strftime("%Y-%m-%d %H:%M:%S UTC")
    header = f"==== {stamp} (exit {exit_code}) ====" if exit_code is not None else ""
    p.write_text(header + "\n" + body + "\n")
    os.utime(p, (mtime.timestamp(), mtime.timestamp()))
    return p


class TestReclaimLog(unittest.TestCase):
    def test_fresh_after_0400_passes(self):
        # log written 05:00 today; checked 09:00 today
        now = dt.datetime(2026, 9, 3, 9, 0)
        p = fake_reclaim_log(dt.datetime(2026, 9, 3, 5, 2))
        f = md.check_reclaim_log(p, now)
        self.assertEqual(f.level, "ok", f.render())

    def test_before_todays_0400_is_stale(self):
        # log written yesterday 04:30; the 04:00 run today never happened
        now = dt.datetime(2026, 9, 3, 9, 0)
        p = fake_reclaim_log(dt.datetime(2026, 9, 2, 4, 30))
        f = md.check_reclaim_log(p, now)
        self.assertEqual(f.level, "fail", f.render())
        self.assertIn("stale", f.message)

    def test_missing_log_fails(self):
        f = md.check_reclaim_log(Path("/nonexistent/nope.log"),
                                 dt.datetime(2026, 9, 3, 9, 0))
        self.assertEqual(f.level, "fail")
        self.assertIn("missing", f.message)

    def test_latest_0400_wraps_midnight(self):
        # 01:00 belongs to yesterday's 04:00 schedule
        now = dt.datetime(2026, 9, 3, 1, 0)
        self.assertEqual(md.latest_0400(now),
                         dt.datetime(2026, 9, 2, 4, 0))
        # 06:00 is today's
        self.assertEqual(md.latest_0400(dt.datetime(2026, 9, 3, 6, 0)),
                         dt.datetime(2026, 9, 3, 4, 0))


class TestSonarrPruneLog(unittest.TestCase):
    def test_fresh_run_after_boundary_passes(self):
        # pruned 03:40 on the 1st; checked midday on the 1st
        now = dt.datetime(2026, 9, 1, 12, 0)
        p = fake_prune_log(dt.datetime(2026, 9, 1, 3, 40), exit_code=0)
        f = md.check_sonarr_prune_log(p, now)
        self.assertEqual(f.level, "ok", f.render())

    def test_mid_month_previous_run_still_fresh(self):
        # checked on the 15th; last run was the 1st 03:50
        now = dt.datetime(2026, 9, 15, 8, 0)
        p = fake_prune_log(dt.datetime(2026, 9, 1, 3, 50), exit_code=0)
        f = md.check_sonarr_prune_log(p, now)
        self.assertEqual(f.level, "ok", f.render())

    def test_missing_monthly_run_is_stale(self):
        # the 1st-of-month 03:30 run never happened (log is the prior month's)
        now = dt.datetime(2026, 9, 2, 9, 0)
        p = fake_prune_log(dt.datetime(2026, 8, 1, 3, 50), exit_code=0)
        f = md.check_sonarr_prune_log(p, now)
        self.assertEqual(f.level, "fail", f.render())
        self.assertIn("stale", f.message)

    def test_missing_log_fails(self):
        f = md.check_sonarr_prune_log(Path("/nonexistent/prune.log"),
                                      dt.datetime(2026, 9, 1, 12, 0))
        self.assertEqual(f.level, "fail")
        self.assertIn("missing", f.message)

    def test_failed_run_is_flagged_even_when_fresh(self):
        # the cron fired today but the prune reported problems (rc 1)
        now = dt.datetime(2026, 9, 1, 12, 0)
        p = fake_prune_log(dt.datetime(2026, 9, 1, 3, 50), exit_code=1,
                           body="CHECK FAILED: bloat remains after prune")
        f = md.check_sonarr_prune_log(p, now)
        self.assertEqual(f.level, "fail", f.render())
        self.assertIn("exited", f.message)

    def test_log_without_run_record_fails(self):
        now = dt.datetime(2026, 9, 1, 12, 0)
        p = fake_prune_log(dt.datetime(2026, 9, 1, 3, 50), exit_code=None)
        f = md.check_sonarr_prune_log(p, now)
        self.assertEqual(f.level, "fail")
        self.assertIn("no run record", f.message)

    def test_latest_monthly_boundary_wraps(self):
        # before the 1st's 03:30, the boundary is the prior month's
        now = dt.datetime(2026, 9, 1, 2, 0)
        self.assertEqual(md.latest_monthly_boundary(now),
                         dt.datetime(2026, 8, 1, 3, 30))
        # January wraps the year
        self.assertEqual(md.latest_monthly_boundary(dt.datetime(2026, 1, 1, 2, 0)),
                         dt.datetime(2025, 12, 1, 3, 30))
        # after 03:30 on the 1st, it is the current month's run
        self.assertEqual(md.latest_monthly_boundary(dt.datetime(2026, 9, 1, 6, 0)),
                         dt.datetime(2026, 9, 1, 3, 30))


class TestTimers(unittest.TestCase):
    SAMPLE_FAILED = (
        "dotfiles-sync.service\tloaded\tfailed\tfailed\tDaily dotfiles sync\n"
        "stack-health-metrics.service\tnot-found\tfailed\tfailed\tstale unit\n"
    )

    def test_parses_failed_units(self):
        names = md.parse_failed_units(self.SAMPLE_FAILED)
        self.assertEqual(names, ["dotfiles-sync.service",
                                 "stack-health-metrics.service"])

    def test_empty_output_passes(self):
        fs = md.check_timers("")
        self.assertEqual(len(fs), 1)
        self.assertEqual(fs[0].level, "ok")

    def test_failed_unit_is_finding(self):
        fs = md.check_timers(self.SAMPLE_FAILED)
        self.assertEqual(len(fs), 1)
        self.assertEqual(fs[0].level, "fail")
        self.assertIn("dotfiles-sync.service", fs[0].message)

    def test_skip_whitelist(self):
        fs = md.check_timers(self.SAMPLE_FAILED,
                             skipped_units=("dotfiles-sync.service",))
        self.assertEqual(fs[0].level, "fail")  # other unit still failing
        fs = md.check_timers(self.SAMPLE_FAILED,
                             skipped_units=("dotfiles-sync.service",
                                            "stack-health-metrics.service"))
        self.assertEqual(fs[0].level, "ok")


class TestDotfiles(unittest.TestCase):
    """check_dotfiles shells to git; _git is monkeypatched per case."""

    def setUp(self):
        self.repo = Path(tempfile.mkdtemp()) / ".dotfiles"
        self.repo.mkdir()

    def test_missing_repo_fails(self):
        f = md.check_dotfiles(Path("/nonexistent/.dotfiles"))
        self.assertEqual(f.level, "fail")

    def test_unreachable_is_warn_not_fail(self):
        md._git = lambda repo, argv, timeout=20: (128, "Could not resolve host")
        f = md.check_dotfiles(self.repo)
        self.assertEqual(f.level, "warn")

    def test_ahead_is_fail(self):
        md._git = lambda repo, argv, timeout=20: (0, "3")
        f = md.check_dotfiles(self.repo)
        self.assertEqual(f.level, "fail")
        self.assertIn("3 commit", f.message)

    def test_synced_is_ok(self):
        md._git = lambda repo, argv, timeout=20: (0, "0")
        f = md.check_dotfiles(self.repo)
        self.assertEqual(f.level, "ok")

    def test_no_origin_ref_is_warn(self):
        md._git = lambda repo, argv, timeout=20: (128, "unknown revision")
        f = md.check_dotfiles(self.repo)
        self.assertEqual(f.level, "warn")


class TestDelegatedChecks(unittest.TestCase):
    def test_db_gate_classification(self):
        # rc 0 -> ok; rc 1 -> fail; rc 2 -> warn. Both paths must pass an
        # explicit --db so resolution never depends on script location, and
        # the sonarr leg must pass --blob-table EpisodeFiles so its MediaInfo
        # probe reads the real table (MovieFiles is absent in sonarr.db and
        # would read 0 bytes — the 2026-09-02 blind spot).
        seen = []
        def fake(script, args, repo_root, timeout=120):
            seen.append(args)
            if "sonarr.db" in args[1]:
                return (2, "radarr DB not found ... skipping")
            return (0, "OK: radarr.db page size and footprint within healthy limits.")
        orig = md.run_script
        md.run_script = fake
        try:
            fs = md.check_db_gates(Path("/repo"))
        finally:
            md.run_script = orig
        by = {f.check: f for f in fs}
        self.assertEqual(by["radarr db"].level, "ok")
        self.assertEqual(by["sonarr db"].level, "warn")
        self.assertTrue(all(a and a[0] == "--db" for a in seen),
                        "every db gate must pass an explicit --db")
        radarr_args = next(a for a in seen if "radarr.db" in a[1])
        sonarr_args = next(a for a in seen if "sonarr.db" in a[1])
        self.assertNotIn("--blob-table", radarr_args,
                         "radarr keeps the MovieFiles default")
        self.assertEqual(sonarr_args[sonarr_args.index("--db") + 2:],
                         ["--blob-table", "EpisodeFiles"],
                         "sonarr leg must pass --blob-table EpisodeFiles")

    def test_queue_classification(self):
        def fake(script, args, repo_root, timeout=120):
            return (0, "PASS nzbdav queue: queue is empty (0 item(s))")
        orig = md.run_script
        md.run_script = fake
        try:
            f = md.check_nzbdav_queue(Path("/repo"))
        finally:
            md.run_script = orig
        self.assertEqual(f.level, "ok")

    def test_arr_import_queue_classification(self):
        # rc 0 -> ok; rc 1 -> fail (stuck pile-up, names the drain);
        # rc 2 -> warn (unreachable/key missing). Each app must be invoked
        # with its own --app so the right $APP_API_KEY resolves.
        outcomes = [
            (0, "PASS sonarr import queue: no stuck completed items"),
            (1, "FAIL sonarr import queue: 230 stuck completed item(s)"),
            (2, "SKIP sonarr import queue: app queue API unreachable"),
        ]
        wants = ["ok", "fail", "warn"]
        seen = []

        def fake(script, args, repo_root, timeout=120):
            seen.append((script, tuple(args)))
            return outcomes[len(seen) - 1]

        orig = md.run_script
        md.run_script = fake
        try:
            for i in range(len(outcomes)):
                f = md.check_arr_import_queue(Path("/repo"), "sonarr")
                self.assertEqual(f.level, wants[i], f"outcome {i}")
                self.assertEqual(f.check, "sonarr import queue")
        finally:
            md.run_script = orig
        self.assertTrue(all(s == "check_arr_import_queue.py" and a == ("--app", "sonarr")
                            for s, a in seen),
                        "import-queue gate must pass --app per check")
        # radarr row labels itself radarr
        def fake2(script, args, repo_root, timeout=120):
            return (0, "PASS radarr import queue: no stuck completed items")
        md.run_script = fake2
        try:
            f = md.check_arr_import_queue(Path("/repo"), "radarr")
        finally:
            md.run_script = orig
        self.assertEqual((f.check, f.level), ("radarr import queue", "ok"))

    def test_audit_classification(self):
        # rc 0 -> ok (clean repo + host), rc 1 -> fail (residue found),
        # rc 2 -> warn (unusable checkout). Full host mode: no extra args.
        outcomes = [
            (0, "AUDIT OK: no retired-service or dead-path residue found"),
            (1, "AUDIT FAIL: 1 residue finding(s); see lines above"),
            (2, "not a stack checkout (no docker-compose.yml)"),
        ]
        wants = ["ok", "fail", "warn"]
        seen = []
        def fake(script, args, repo_root, timeout=120):
            seen.append((script, args))
            return outcomes[len(seen) - 1]
        orig = md.run_script
        md.run_script = fake
        try:
            for i in range(len(outcomes)):
                f = md.check_audit_residue(Path("/repo"))
                self.assertEqual(f.level, wants[i], f"outcome {i}")
        finally:
            md.run_script = orig
        self.assertTrue(all(s == "audit_residue.py" and a == [] for s, a in seen),
                        "residue audit must run full host mode with no args")

    def test_config_drift_classification(self):
        # rc 0 -> ok (pins satisfied), rc 1 -> fail (drift found),
        # rc 2 -> warn (cannot assess — docker/compose unavailable).
        outcomes = [
            (0, "OK: 8 running container(s) match their compose pins."),
            (1, "CHECK FAILED: 2 running container(s) drifted from their "
                "compose pin:"),
            (2, "CHECK SKIPPED: cannot assess container drift:"),
        ]
        wants = ["ok", "fail", "warn"]
        seen = []

        def fake(script, args, repo_root, timeout=120):
            seen.append((script, args))
            return outcomes[len(seen) - 1]

        orig = md.run_script
        md.run_script = fake
        try:
            for i in range(len(outcomes)):
                f = md.check_config_drift(Path("/repo"))
                self.assertEqual(f.level, wants[i], f"outcome {i}")
        finally:
            md.run_script = orig
        self.assertTrue(all(s == "check_config_drift.py" and a == []
                            for s, a in seen),
                        "config drift must run with no args")


if __name__ == "__main__":
    unittest.main(verbosity=2)
