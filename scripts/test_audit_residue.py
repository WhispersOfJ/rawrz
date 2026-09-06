#!/usr/bin/env python3
"""Regression test for scripts/audit_residue.py (TODO.md #2).

Verifies the retired-residue audit cannot silently bit-rot:

  * the registry is complete — every service in lifecycle.md's 'Retired
    services' table exists in RETIRED_SERVICES (and vice versa), so recording
    a retirement without teaching the checker its name fails here
  * matching logic — retired names flag; active names (radarr, DISCORD_*,
    maintenance-digest units) never do; comment lines and
    `audit-residue-ignore` lines are deliberate and skipped
  * env vars — retired prefixes (LIDARR_, GRAFANA_, WS_) flag, active ones do
    not
  * docs filenames — retired-named pages flag; the migration record and
    active service pages do not
  * host surfaces — crontab lines and user unit files (enabled or inert)
    referencing dead paths / retired names flag via fixture files
  * the actual repo surfaces (compose, .env.template, workflows minus the
    cleanuparr watcher, functions tree, docs filenames) currently carry no
    residue — enforced here so CI catches any regression

Runs fully offline (no docker, no systemd, no crontab) and works on the CI
runner. Run by validate.yml and nightly-healthcheck.yml, and locally via
`python3 scripts/test_audit_residue.py`. Exits 0 when every assertion holds,
1 otherwise.
"""

import importlib.util
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AUDIT_PATH = ROOT / "scripts" / "audit_residue.py"

spec = importlib.util.spec_from_file_location("audit_residue", AUDIT_PATH)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


def main() -> int:
    failures = 0

    def expect(name, got, want):
        nonlocal failures
        if got == want:
            print(f"OK: {name}")
        else:
            print(f"FAIL: {name} expected {want!r}, got {got!r}")
            failures += 1

    # --- Registry ↔ lifecycle.md sync --------------------------------
    doc = (ROOT / "docs" / "services" / "lifecycle.md").read_text()
    recorded = mod.lifecycle_names(doc)
    expect("every lifecycle retirement is in RETIRED_SERVICES",
           recorded - set(mod.RETIRED_SERVICES), set())
    expect("no registry name is missing from lifecycle.md",
           set(mod.RETIRED_SERVICES) - recorded, set())
    drift = [f for f in mod.check_registry_sync(doc) if f.level == "fail"]
    expect("registry sync check reports no drift", drift, [])

    # --- Token matching ----------------------------------------------
    expect("retired name flags", mod.TOKEN_RE.search("start lidarr now") is not None, True)
    # bazarr re-joined the registry on 2026-09-06 (re-retired); it must flag again.
    expect("re-retired name flags", mod.TOKEN_RE.search("start bazarr now") is not None, True)
    expect("active name never flags", mod.TOKEN_RE.search("radarr import") is not None, False)
    expect("no partial-word match",
           mod.TOKEN_RE.search("the prometheusish era") is not None, False)

    # --- Line scans ---------------------------------------------------
    path = ROOT / "docker-compose.yml"
    hits = mod.scan_lines("compose",
                          "# no Traefik — direct host ports\n  image: ghcr.io/hotio/grafana:latest\n",
                          path, ignore_comments=True)
    expect("compose comment skipped, real hit flagged", len(hits), 1)
    expect("compose hit names the surface/location",
           "grafana" in hits[0].message, True)

    marker = mod.scan_lines("functions",
                            '# stack-loop-ratings uses OMDB like lidarr audit-residue-ignore\n',
                            path)
    expect("audit-residue-ignore line skipped", marker, [])

    dead = mod.scan_lines("functions",
                          'repo="/home/bear/Claude/media-stack"\n', path)
    expect("dead project path flags", len(dead), 1)

    # --- Env vars ------------------------------------------------------
    expect("LIDARR_ prefix flags", mod.env_var_is_retired("LIDARR_API_KEY"), True)
    expect("GRAFANA_ prefix flags", mod.env_var_is_retired("GRAFANA_DS"), True)
    expect("WS_ (watchstate) prefix flags", mod.env_var_is_retired("WS_TOKEN"), True)
    expect("TRAEFIK_ prefix flags", mod.env_var_is_retired("TRAEFIK_DASHBOARD_AUTH"), True)
    expect("RADARR_ prefix is active", mod.env_var_is_retired("RADARR_API_KEY"), False)
    expect("DISCORD_ prefix is active", mod.env_var_is_retired("DISCORD_WEBHOOK_URL"), False)

    # --- Docs filenames ------------------------------------------------
    expect("retired-named page flags", mod.doc_name_is_retired("lidarr.md"), True)
    expect("re-retired service page flags", mod.doc_name_is_retired("bazarr.md"), True)
    expect("active service page does not flag", mod.doc_name_is_retired("radarr.md"), False)

    # --- Crontab -------------------------------------------------------
    cron = mod.scan_crontab(
        "# comment mentioning grafana is fine\n"
        "0 4 * * * /home/bear/Claude/media-stack/scripts/x.sh\n"
        "30 5 * * * stack-disk-reclaim\n")
    expect("dead path cron line flags", len(cron), 1)
    expect("comment and clean lines do not flag",
           cron[0].message.startswith("line 2"), True)

    # --- User units (fixture dir) --------------------------------------
    with tempfile.TemporaryDirectory() as td:
        unit_dir = Path(td)
        (unit_dir / "timers.target.wants").mkdir()
        (unit_dir / "default.target.wants").mkdir()
        (unit_dir / "default.target.wants" / "media-stack.service").symlink_to(
            "/dev/null")
        (unit_dir / "media-stack.service").write_text(
            "[Service]\nWorkingDirectory=/home/bear/Claude/media-stack\n"
            "ExecStart=/home/bear/Claude/media-stack/scripts/legacy.sh\n")
        (unit_dir / "stack-health-check.service").write_text(
            "[Service]\nWorkingDirectory=/home/bear/Claude/media-stack\n"
            "ExecStart=/home/bear/Claude/media-stack/scripts/check-container-health.sh\n")
        (unit_dir / "stack-arr-backup.service").write_text(
            "[Service]\nWorkingDirectory=/home/bear/Claude/media-stack\n"
            "ExecStart=/usr/bin/python3 /home/bear/Claude/media-stack/scripts/arr-app-backup.py\n")
        (unit_dir / "stack-maintenance-digest.service").write_text(
            "[Service]\nExecStart=/home/bear/.local/bin/stack-maintenance-digest-daily.sh\n")
        (unit_dir / "dotfiles-sync.service").write_text(
            "[Service]\nExecStart=/home/bear/.local/bin/dotfiles-daily-sync.sh\n")
        enabled = mod.enabled_unit_names(unit_dir)
        expect("wants symlink detected as enabled", "media-stack.service" in enabled, True)
        unit_hits = mod.scan_user_units(unit_dir, enabled)
        hit_names = [u.message.split("[")[0].strip() for u in unit_hits]
        expect("dead-path unit flagged", "stack-health-check.service" in hit_names, True)
        expect("inert dead-path unit flagged", "stack-arr-backup.service" in hit_names, True)
        media_msg = next(u.message for u in unit_hits
                         if u.message.startswith("media-stack.service "))
        expect("enabled stale unit flagged with state", "[enabled]" in media_msg, True)
        expect("clean units never flagged",
               "stack-maintenance-digest.service" in hit_names
               or "dotfiles-sync.service" in hit_names, False)

    # --- Real repo surfaces carry no residue today ----------------------
    repo_findings = []
    repo_findings += mod.scan_lines("compose", mod.COMPOSE.read_text(), mod.COMPOSE,
                                    ignore_comments=True)
    repo_findings += mod.scan_env_template()
    for wf in sorted(mod.WORKFLOWS.glob("*.yml")):
        if wf.name in mod.WATCHER_FILES:
            continue
        repo_findings += mod.scan_lines("workflows", wf.read_text(), wf)
    for fn in sorted(mod.FUNCTIONS.rglob("*.sh")):
        repo_findings += mod.scan_lines("functions", fn.read_text(), fn)
    repo_findings += mod.scan_doc_filenames()
    repo_fails = [f for f in repo_findings if f.level == "fail"]
    if repo_fails:
        for f in repo_fails[:5]:
            print(f"FAIL: unexpected residue in repo: {f.render()}")
    expect("repo surfaces carry no residue", repo_fails, [])

    if failures == 0:
        print("test_audit_residue: all assertions passed")
        return 0
    print(f"test_audit_residue: {failures} assertion(s) failed")
    return 1


if __name__ == "__main__":
    sys.exit(main())
