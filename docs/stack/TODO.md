# TODO — Stack Projects

Ordered project backlog for the stack, drawn from the 2026-09-02 session
(post-slim-down maintenance era). Each entry records the compatibility
research done at write time so work never duplicates what already exists.

> **Working style reminder (AGENTS.md):** every task below ships on its own
> task-named worktree + PR. "For the stack" means **host-runnable, API-backed,
> one job each** — no new always-on containers unless the entry explicitly
> says otherwise (the 2026-08-30 slim-down record is the cautionary tale).

---

## 1. Nightly maintenance-verification digest

One host-runnable job that asserts the maintenance actually happened, then
prints a single morning summary:

- `~/.stack-disk-reclaim.log` updated since the last 04:00 run
- no failed user/system timers (`systemctl --user --failed` empty)
- dotfiles pushed (`.dotfiles` `main` matches `origin/main`; job script exits 0)
- Radarr/Sonarr DB sizes under the `check_radarr_db_size.py` threshold
- NzbDAV queue empty

**Compatibility research (2026-09-02):** NOT built. Host-side health units
exist (`stack-health-check`, `stack-plex-health-monitor`, `stack-plex-report`,
`stack-arr-backup` under `~/.config/systemd/user/`) but none aggregates these
signals. Would have caught the 17-day silent dotfiles-push failure and the
dead media-stack cron entries within 24h. Optional Discord ping via the
existing `DISCORD_WEBHOOK_URL` pattern (alertmanager removed Discord alerting).

**Status (2026-09-02): ✅ built and live.** `scripts/maintenance_digest.py` runs
from a 05:10 user timer and reports nine surfaces — reclaim log, user timers,
dotfiles, radarr + sonarr DB gates, nzbdav queue, residue audit, monthly
sonarr prune log, and config drift (#3's surface) — with `rc` honoring the
check_* family's 0/1/2 semantics.

## 2. `stack-audit-residue` — retired-service/path residue checker

Scan compose, `.env.template`, docs, workflows, crontab, and user timers for
references to retired services and dead paths (the automated form of the
exhaustive-removal checklist, AGENTS.md landmine #7).

**Compatibility research (2026-09-02):** NOT built. This session found such
residue by hand (media-stack cron entries, node-exporter-era
`stack-health-metrics` timer, poster/letterboxd syncs into the retired
`/home/bear/Claude/media-stack`). Pattern precedent:
`scripts/check_mount_drift.py` / `check_secret_drift.py` (failing `check_*`
script + guard wiring in `validate.yml` / `nightly-healthcheck.yml`).

**Status (2026-09-02): ✅ built and live.** `stack-audit-residue` (
`scripts/audit_residue.py`) scans the repo surfaces plus crontab and user
units; the nine stale units it named were removed the same day and the
operational run is green. Host-fed into the digest as its residue-audit
surface.

## 3. Config-drift checker — running images vs compose pins

Surface containers whose running image differs from the compose pin
(unpackerr `0.15.2` running vs `v0.16.1` pinned; Plex on an older digest than
compose's pin — both found manually this session).

**Compatibility research (2026-09-02):** NOT built — drift checking exists only
for mounts (`check_mount_drift.py`) and secrets (`check_secret_drift.py`).
Follow the same `check_*` + CI-guard pattern. Distinct from #2 (this is
*current-config* drift, not retired residue).

**Status (2026-09-02): ✅ built, live, and already paid for itself.**

**Closeout record.** `scripts/check_config_drift.py` + `stack-config-drift`
wrapper: compose pins from `docker compose config --format json` vs running
containers' `docker inspect` `Config.Image`, matched by the
`com.docker.compose.service` label (cwd-proof). Wired into preflight
(docker-gated leg), validate/nightly guard tests, completions/TUI/smoke, and
the digest as its ninth surface. A drift report ends with a recreate-reminder
line naming the `docker compose up -d --no-deps <svc>` command and the oldest
date the pins entered the file (`git log -S`; shallow checkouts degrade to
"unknown"). PRs #124 (checker + wiring) → #125 (digest surface) → #126
(recreate-reminder).

**First real drift (root-caused and remediated the same day).** The live
run immediately flagged the two drifts this entry was written about: unpackerr
`0.15.2` running vs `v0.16.1` pinned, and plex on an older digest. Root cause:
Dependabot re-pinned both on 2026-08-31 12:45/12:46 (weekly docker bump) and
nothing ever recreated the containers — compose-edit-without-recreate, not
image-GC or a manual change. Remediated with `docker compose up -d
--no-deps plex unpackerr` (queue-idle, per the house rule); `stack-config-drift`
and the full preflight sweep went green afterwards. The recreate-reminder is
the recurrence guard for the dependabot-weekly cycle.

**Remaining backlog preview (#4–#8 below):** all five shipped on
2026-09-03 — #4 Bazarr re-adoption, #5 radarr DB growth-trend predictor, #6
"what's watchable tonight" view, #7 request → arrival Discord notifier, and
#8 media activity feed. All stayed host-runnable / API-backed, no new
always-on containers (#4 was the only service-count change, and the only
entry that approved one).

## 4. Bazarr re-adoption (fresh implementation) — ✅ DONE 2026-09-03

Subtitle capability, retired 2026-08-30 for OOM crash-looping at 128m — not
for being useless. Re-adopt per `docs/services/lifecycle.md` convention: fresh
tracked implementation (never a copy from `archive/`), real memory cap
(512m–1g), DB hygiene, Sonarr wiring, Prowlarr sync, trivy baseline.

**Compatibility research (2026-09-02):** retired by decision, no watcher;
re-adoption would raise the service count above 8 and total mem caps
(≈12.1g/22g today) — deliberate scoping decision needed before starting.

**Closeout (2026-09-03):** shipped as the stack's 9th always-on service.
`ghcr.io/hotio/bazarr:release-1.6.0` at **768m / 1 CPU** (174 MiB steady-state
observed on this host; 4.4× the failed cap), healthcheck on unauthenticated
`/ping`, trivy baseline 0 CRITICAL/HIGH (`--ignore-unfixed`). Mounts mirror
Sonarr/Radarr exactly and are read-only (`:rslave`) over the FUSE mount and
media trees; `depends_on` covers sonarr/radarr only, so mount-owner restarts
and NzbDAV recreations never touch it. Wired end to end: audit-residue
registry de-retired (with the registry↔lifecycle cross-check enforcing both
sides), health runner, trivy baseline, README/AGENTS/registry/docs to 9
services, lifecycle re-adoption record, API.md section. One-time step that
needs the UI: connect Sonarr/Radarr in Bazarr's settings (no env-var surface
upstream).

## 5. Radarr DB growth-trend predictor — ✅ DONE 2026-09-03

`check_radarr_db_size.py` (gate) and `prune_radarr_db.py` (remediation) exist;
nothing *predicts* when the next prune is needed from growth history.

**Compatibility research (2026-09-02):** gate + remediation live (AGENTS.md
landmine #9). Add a size-history record (SQLite or flat log) and trend the
growth rate — turns the incident into a calendar entry.

**Closeout (2026-09-03):** `scripts/db_growth_trend.py` (+ regression test).
JSONL sample history at `.cache/db-growth/history.jsonl` (gitignored runtime
state); metrics recorded through the gate's own `read_metrics` so predictor
and gate can never disagree; OLS trend segmented at prune/VACUUM resets
(>10% drops start a new segment, so pre-prune growth never masks current
history); predicts the crossing date of each DB's per-app footprint
high-water (the gate's own `footprint_default_mb`). The nightly digest both
consumes and feeds it: `check_db_growth` delegates with the same rc 0/1/2
semantics (rc 1 = crossing inside the 30-day horizon → schedule the prune;
rc 2 = fresh history, soft WARN), and each nightly run appends the sample
that sharpens tomorrow's forecast. Covers radarr, sonarr, and bazarr.

## 6. "What's watchable tonight" — thin read-only view — ✅ DONE 2026-09-03

Plex unwatched + Radarr/Sonarr recently-added + Seerr request state, rendered
via the existing host-published APIs (no container — a "dashboard that isn't
a dashboard").

**Compatibility research (2026-09-02):** NOT built. `arr-dashboard` and
`landing-page` were retired by decision in the slim-down; this is the thin,
API-only shape that avoids their failure mode. Reuse the `stack-*` bash
function API surface (`__arr_api` / `__plex_api` / `__seerr_api` helpers).

**Closeout (2026-09-03):** `stack-watchable` (one screen) + `stack-requests`
`[take]` + `stack-unwatched` `[limit]` + `stack-recent` `[limit]` in
`services/bash-functions/functions/stack-watchable.sh` — read-only, every
section degrades to its own error line (a wedged service dims one section,
never the view). Seerr status ladder corrected to the real enums
(`seerr-team/seerr` `server/constants/media.ts`: request PENDING=1 /
APPROVED=2 / DECLINED=3 / FAILED=4 / COMPLETED=5; media UNKNOWN=1 ..
AVAILABLE=5) — the draft renderer had status 3 as "processing", which is
actually DECLINED; only PENDING/APPROVED render, an approved request whose
media is already AVAILABLE renders as "available now". `SEERR_API_KEY`
templated into `.env.template` (generated in Seerr Settings → API keys;
not auto-provisioned). Wired into the bash smoke suite (unit tests with
mocked Seerr/Plex payloads + live tier) and completions.

## 7. Request → arrival notifier — ✅ DONE 2026-09-03

Seerr request approved → Radarr/Sonarr grab → download complete → Plex scan →
one Discord ping. The minimal useful slice of retired watchstate.

**Compatibility research (2026-09-02):** NOT built. watchstate retired by
decision. Seerr + Unpackerr were *retained* in the slim-down precisely for
request handling/extraction — this wires their outcomes into a single
notification chain. Webhook env pattern precedent: `DISCORD_WEBHOOK_URL`.

**Closeout (2026-09-03):** `scripts/arrival_notifier.py` (+ regression
test) + `stack-arrival-notify [--dry-run|--no-refresh|--json]` wrapper.
Poller, not listener (the repo's operational surface is timer-driven; a
webhook listener would add an always-on host process, an open port, and
*arr/Seerr webhook config with missed events when down — the poller never
loses or duplicates a ping). Detection is *arr-history-first, Seerr-verdict-
second*: open requests (PENDING/APPROVED) plus unnotified COMPLETED resolve
to *arr items (tmdbId→Radarr, tvdbId→Sonarr) and ask the History API for an
import newer than the request; APPROVED/COMPLETED with media AVAILABLE is
the fallback (covers already-available requests); DECLINED/FAILED drop from
the watch set without a ping. On arrival: refresh the matching Plex section,
then one Discord POST; delivery failure is NOT recorded so the next run
retries; `DISCORD_WEBHOOK_URL` unset → feature disabled (exit 0, pending
arrivals deliver once the webhook exists). State in `.cache/arrivals/
state.json`. Install: 15-minute user timer (unit files in the docstring).

## 8. Media-stack activity feed (thin) — ✅ DONE 2026-09-03

RSS/JSON of imports, upgrades, deletions via the *arr apps' existing webhook
events — a small listener, no container.

**Compatibility research (2026-09-02):** NOT built. Radar/Sonarr webhook events
are already configured endpoints; nothing consumes them today.

**Closeout (2026-09-03):** `scripts/activity_feed.py` (+ regression test,
including an end-to-end `run()` against a canned local HTTP server) +
`stack-activity-feed [limit]` wrapper. Polls the *arr History API with a
per-app cursor (`.cache/activity/state.json`) and records the same event
data their webhook payloads carry — `downloadFolderImported` /
`seriesFolderImported` → import, `movieFileDeleted` / `episodeFileDeleted`
→ delete, a second import for an item still on record → upgrade, everything
else skipped. Appends `.cache/activity/feed.jsonl` and re-renders
`feed.json` + `feed.xml` (RSS 2.0). Poller chosen over an always-on
listener for the same reasons as #7 (no open port, no webhook config, no
missed events when down; a first run backfills 24h instead of the app's
whole history). Install: 15-minute user timer; subscribe by serving
`.cache/activity/` or syncing `feed.xml` to a reader-accessible location.

---

### Compatibility notes shared by all entries

- Host RAM headroom is ~9.8g of 22g after the slim-down rebalance; new
  always-on services are the exception, not the default.
- Reuse the `stack-*` bash surface and `scripts/` guard pattern
  (`check_*` + test + `validate.yml`/`nightly-healthcheck.yml` wiring).
- Every new workflow file is actionlint-gated; pre-commit gates local + CI
  hook parity (gitleaks/ruff/shellcheck/check-yaml/EOF/whitespace).
- Release-worthy changes (`feat:`/`fix:`) trigger release-please; keep
  `docs:`/`chore:` out of release scope unless a release is intended.
