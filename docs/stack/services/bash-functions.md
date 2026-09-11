# Bash Functions — API-backed CLI

The bash functions are a bash port of the fish `stack-*` library, giving the
same operational surface to bash users. They call the host-published APIs
directly; no proxy or dashboard container is required.

## Layout

```text
services/bash-functions/
├── bearcave-bash.sh            # Sourced from ~/.bashrc; loads .env, fmt_*,
│                               # the guarded docker wrapper, and all stack-*
├── functions/
│   ├── __helpers.sh            # __arr_api, __plex_api, __seerr_api,
│   │                           # __nzbdav_api, __stack_containers, ...
│   ├── stack-arr-1.sh          # backlog, blocklist, recently-added, toggle-search
│   ├── stack-arr-2.sh          # missing, cutoff, import, logs
│   ├── stack-arr-3.sh          # command triggers, backlog status, import lists, prowlarr
│   ├── stack-core.sh           # status, container, restart, top, version, help
│   ├── stack-disk.sh           # config sizes, docker disk, nzbdav dedup/delete-failures
│   ├── stack-lists.sh          # mdblist / letterboxd track / import
│   ├── stack-loop-ratings.sh    # loop candidates, exclude, unmonitor, rating lookups
│   ├── stack-maintenance.sh     # maintenance digest (reclaim log, timers, DBs)
│   ├── stack-misc.sh           # seerr, notify, backup, worktree, host checks
│   ├── stack-arrivals.sh       # arrival notifier + activity feed (TODO #7/#8)
│   ├── stack-watchable.sh      # what's watchable tonight (TODO #6)
│   ├── stack-nzbdav.sh         # nzbdav queue, history, stats, mount-health
│   ├── stack-plex-core.sh      # sessions, scan, butler, trash, analyze
│   ├── stack-plex-extra.sh     # duplicates, gc, updates, image-clean
│   ├── stack-plex-markers.sh   # read-only intro/credits/ad marker audit
│   ├── stack-plex-updates.sh   # updates, refresh-libraries, autofix, sonarr fix
│   └── stack-queue.sh          # queue status, queue errors
├── completions/
│   └── stack-completions.sh    # GENERATED — see scripts/gen-bash-completions.sh
└── scripts/
    └── gen-bash-completions.sh # generate / --check completion drift
```

## Setup

Add to `~/.bashrc` (interactive shells only):

```bash
[ -f "$HOME/cave/services/bash-functions/bearcave-bash.sh" ] && \
    source "$HOME/cave/services/bash-functions/bearcave-bash.sh"
```

The loader reads `.env` at startup (only setting variables not already set),
defines the `fmt_*` helpers and the guarded `docker` wrapper, then sources every
`functions/__*.sh` and `functions/stack-*.sh`, then the generated completions.

Functions target the direct host ports by default (override with `*_URL` env
vars): Prowlarr `:9696`, Radarr `:7878`, Sonarr `:8989`, NzbDAV `:3000`,
Seerr `:5055`, Plex `:32400`.

## Completion checks

```bash
services/bash-functions/scripts/gen-bash-completions.sh --check   # drift gate
bash -n services/bash-functions/functions/*.sh                   # syntax gate
./tests/bash/test_bash_functions.sh --offline                   # full offline smoke
```

## Relationship to the fish functions

The bash functions are a **parallel port** of `services/fish-functions/`.
The fish functions remain the canonical, installed operational surface today;
the bash port is additive. The planned retirement of the fish functions into a
`docs/services/FISH.md` retirement record is tracked separately and will land
in a follow-up PR after this port is validated in daily use.

## API call timeouts

Every API call routes through a central `__stack_curl` wrapper that injects
`--connect-timeout 5 --max-time <budget>`, so a wedged or dead service can
never hang a `stack-*` command forever — it fails-soft (curl exit 28) and the
caller's existing `Cannot reach <app>` guard prints the message.

Budgets are per-call type and overridable via env vars:

| Budget | Default | Covers | Override |
|---|---|---|---|
| `STACK_API_TIMEOUT_LIGHT` | 10s | status, queue, sessions, indexers, single-item GETs | |
| `STACK_API_TIMEOUT_MUTATE` | 20s | POST/PUT/DELETE command triggers, butler, exclusions | |
| `STACK_API_TIMEOUT_HEAVY` | 30s | history pulls, full library scans, external RSS/OMDb/MDBList | |

The four `__*_api` helpers (`__arr_api`, `__plex_api`, `__seerr_api`,
`__nzbdav_api`) pick LIGHT/MUTATE from the HTTP method automatically; raw
call sites in `stack-*.sh` pass the budget explicitly. Python-embedded
`subprocess.run(['curl', ...])` sites carry inline `--max-time`.

## Passing data to embedded python

Python renderers (`python3 -c` / heredocs) receive bulk data over **stdin**
(`echo "$result" | python3 -c ...`), never through environment variables: a
single env var is capped at 128 KB (`MAX_ARG_STRLEN`) on Linux, so a payload
that scales with library size (series maps, history, full-collection pulls)
can fail `execve` with E2BIG silently past a few thousand records. Pass only
scalar config (URLs, keys, IDs, limits) via env; pipe anything data-sized.

## Safety behavior

The `docker` wrapper routes NzbDAV and `nzbdav_rclone` state-changing
operations through `scripts/nzbdav-safe-recreate.sh`, matching the fish
wrapper: recreation is blocked when queued NZBs are present, and `--force` is
explicit and dangerous (queued items are lost). See
[fish-functions safety behavior](fish-functions.md#safety-behavior).

## Nightly maintenance (cron)

`stack-disk-reclaim -y --aggressive` is designed to run unattended from cron
(the wrapper's own header says so). Install it with:

```bash
scripts/install-nightly-reclaim-cron.sh
```

This adds a single entry to the **current user's crontab** (no root needed —
the stack runs as the invoking user):

```cron
# 04:00 daily — prunes dangling volumes/build cache/stopped containers, then
# (--aggressive) every image not referenced by docker-compose.yml, run from
# the executable checkout's bash functions with .env resolved.
0 4 * * * bash -lc 'source "/home/<user>/cave/services/bash-functions/bearcave-bash.sh" && stack-disk-reclaim -y --aggressive' >> "$HOME/.stack-disk-reclaim.log" 2>&1
```

The installer fails closed: it refuses to write the entry when the target
checkout could not actually run the command (missing `stack-disk-reclaim`,
missing `.env`, missing script, moved repo). Point it at another checkout
with `--repo DIR` (must hold `.env` and post-slim-down main).

| Action | Command |
|---|---|
| Install (idempotent) | `scripts/install-nightly-reclaim-cron.sh` |
| Target another checkout | `scripts/install-nightly-reclaim-cron.sh --repo DIR` |
| Remove | `scripts/install-nightly-reclaim-cron.sh --remove` |
| Check | `scripts/install-nightly-reclaim-cron.sh --check` |

Verify each morning: `tail -n 5 ~/.stack-disk-reclaim.log` should show a
`Total reclaimed space: ...` line from the aggressive pass; errors are
logged there too (script output, not silent). To preview what the nightly run
would remove before trusting it: `stack-disk-reclaim --dry-run --aggressive`.

### `stack-maintenance-digest` — verify the maintenance actually ran

TODO.md project #1. The unattended jobs (04:00 reclaim cron, daily dotfiles
push, DB gates) fail silently; the digest runs **after** them each morning
and prints one line per surface, exiting 1 on any FAIL:

```text
Maintenance digest — 2026-09-03 05:10
  OK    reclaim log    written 04:00 (today's 04:00 run)
  OK    user timers    no failed user units
  OK    dotfiles       main matches the remote tip
  OK    radarr db      OK: radarr.db page size and footprint within healthy limits.
  WARN  sonarr db      radarr DB not found ... (fresh checkout / DB elsewhere)
  OK    nzbdav queue   PASS nzbdav queue: queue is empty (0 item(s))
  OK    sonarr import queue  PASS sonarr import queue: no stuck completed items
  OK    radarr import queue  PASS radarr import queue: no stuck completed items
  OK    residue audit  AUDIT OK: no retired-service or dead-path residue found
  OK    sonarr prune   run 2026-09-01 ok (monthly 03:30 cron)
  OK    config drift   OK: 8 running container(s) match their compose pins.
DIGEST OK: nightly maintenance verified
```

Checks: reclaim-log freshness (mtime vs the last 04:00 boundary),
`systemctl --user --failed` (override deliberate failures with repeated
`--skip-user-unit NAME`), dotfiles push state compared against the fetched
remote tip (`FETCH_HEAD` — a bare dotfiles repo keeps no tracking ref, so
`origin/main` can be stale), Radarr + Sonarr DB health via
`check_radarr_db_size.py --db`, the nzbdav queue gate (unreachable =
soft WARN), the *arr import-queue gate (`check_arr_import_queue.py`,
one row per app): completed downloads held from import (importBlocked /
importPending warnings — the matched-by-ID class that piled up 230 items
on 2026-09-02) counted against a threshold (default 10), over which the
digest FAILs so the pile is drained (`scripts/drain_sonarr_queue.py
--app <sonarr|radarr> --apply`, see troubleshooting) before it grows —
unreachable app reads as a soft WARN — the full-host
retired-residue audit (`stack-audit-residue`, TODO.md #2) — a residue
finding fails the digest so removal residue shows up every morning
instead of silently creeping back — and the monthly sonarr prune log
(`~/.sonarr-prune.log`: fresh vs the last 1st-of-month 03:30 boundary
**and** its last recorded run must have exited 0, so a prune that ran but
failed its own verification is flagged) — and the config-drift gate
(`stack-config-drift`, TODO.md #3): running container images vs compose
pins, rc 2 (docker/compose unavailable) read as a soft WARN. Backed by
`scripts/maintenance_digest.py`; `--repo` points DB resolution at the
operational checkout. Suggested schedule: 05:10 daily user timer (after the
04:00 reclaim cron).

### `stack-audit-residue` — retired-service/path residue audit

TODO.md project #2, the automated form of the exhaustive-removal checklist
(AGENTS.md landmine #7). Removal residue is caught mechanically instead of
by hand (the 2026-09-02 session found dead media-stack cron entries and a
node-exporter-era timer by manual grep): scans the repo surfaces (compose
non-comment lines, `.env.template` retired variable prefixes,
workflows minus re-adoption watchers, the functions tree, retired-named
docs pages) plus host surfaces (`crontab -l`, `~/.config/systemd/user` unit
files), flagging references to retired services and dead project paths.

```text
Retired-residue audit
  FAIL  user units   stack-health-check.service [inert]: mentions retired `media-stack` (3 lines); dead path /home/bear/Claude/media-stack (2 lines)
  FAIL  user units   media-stack.service [enabled]: name matches retired `media-stack`; dead path /home/bear/Claude/media-stack (1 line)
AUDIT FAIL: 2 residue finding(s); see lines above
```

The registry mirrors `docs/services/lifecycle.md`: its "Retired services"
table is parsed and every recorded service must exist in the checker's
`RETIRED_SERVICES` (the offline test enforces both directions), so recording
a retirement without teaching the checker its name fails CI. Deliberate
in-code mentions take a trailing `audit-residue-ignore` marker. Repo-only
mode (`--repo-only`) is what preflight and CI run — the full command's host
findings are a triage list, not a gate. Backed by
`scripts/audit_residue.py`; host surfaces degrade to WARN when unavailable
(e.g. a CI runner with no crontab/user units).

### `stack-config-drift` — running images vs compose pins

TODO.md project #3. Surfaces containers whose *running* image differs from
the compose pin — both found manually on 2026-09-02: unpackerr running
0.15.2 while compose pins v0.16.1, and plex on an older digest than the
`@sha256` pin. `docker compose config --format json` is the source of truth
for the pins; `docker inspect`'s `Config.Image` says what each running
container was created from (`docker ps` truncates digest refs to short IDs,
so it is unusable for comparison). Containers are matched to services by
their `com.docker.compose.service` container label, so the check works from
any checkout — it never depends on compose project-name derivation.
Digest pins must be satisfied exactly (a mutable-tag ref does not satisfy
a `@sha256` pin); tag pins are satisfied by any running ref with the same
name+tag. Backed by `scripts/check_config_drift.py`; read-only and safe to
run any time. Exit 0 = every running service matches its pin; 1 = drift
(or docker unavailable). A drift report ends with a **recreate-reminder**:
the exact `docker compose up -d --no-deps <svc>` command and the oldest
date the drifted pins entered the compose file (`git log -S`) — dependabot
re-pins the docker images weekly and merging never recreates containers, so
that date is how stale the mismatch is. The 2026-08-31 dependabot bumps
(unpackerr 0.15.2→v0.16.1, plex digest) were found drifting on 2026-09-02
exactly this way.

### `stack-watchable` — what's watchable tonight (TODO.md #6)

One screen over the host-published APIs — the thin, read-only "dashboard
that isn't a dashboard" (the shape that replaces the retired
`arr-dashboard`/`landing-page` containers):

- `stack-requests [take]` — Seerr request state: count line plus every
  open request (PENDING/APPROVED — Seerr's real status ladder, verified
  against `seerr-team/seerr` `server/constants/media.ts`), with the media
  state label and an `(available now)` marker when the media is already on
  Plex. Declined/failed/completed requests never render.
- `stack-unwatched [limit]` — unwatched Plex items added within the last
  30 days, newest first, per movie/show library.
- `stack-recent [limit]` — recently added movies (Radarr) and series
  (Sonarr).
- `stack-watchable` — all three, one screen.

Every section degrades to a per-source error line when its service is
unreachable — a wedged service dims one section, never the whole view.
`stack-requests` needs `SEERR_API_KEY` (see `.env.template`); the other
views need their app keys as usual.

### `stack-arrival-notify` — request → arrival ping (TODO.md #7)

The minimal useful slice of retired watchstate: watch open Seerr requests
and send **one Discord ping** when the requested item actually lands — the
*arr app's History API confirms the import, the notifier refreshes the
matching Plex section, then one webhook POST. Backed by
`scripts/arrival_notifier.py`; cursor-style state under
`.cache/arrivals/state.json` means a missed run or a down service never
loses or duplicates a ping.

```bash
stack-arrival-notify                  # poll + deliver
stack-arrival-notify --dry-run        # preview without sending/refreshing
stack-arrival-notify --no-refresh     # skip the Plex refresh on arrival
```

`DISCORD_WEBHOOK_URL` unset → feature disabled (exit 0, nothing marked
notified, so setting the webhook later delivers pending arrivals). Install
as a user timer every 15 minutes — the exact unit files are in the
script's docstring:

```bash
systemctl --user daemon-reload
systemctl --user enable --now stack-arrival-notify.timer
```

### `stack-activity-feed` — imports/upgrades/deletions feed (TODO.md #8)

A thin RSS/JSON feed of what the *arr apps actually did, recorded through
their History API (the same event data their webhook events carry) — no
container, no listener, no open port. Backed by `scripts/activity_feed.py`,
which appends to `.cache/activity/feed.jsonl` and re-renders:

- `.cache/activity/feed.json` — latest 50 entries, newest first
- `.cache/activity/feed.xml` — the same as RSS 2.0

```bash
stack-activity-feed 20        # poll, re-render, and print the latest 20
python3 scripts/activity_feed.py   # poll + re-render only (timer job)
```

Imports, upgrades (a second import for an item that already has one on
record), and deletions (`movieFileDeleted`/`episodeFileDeleted`) are
tracked; grabs/renames/failures are skipped. The cursor in
`.cache/activity/state.json` never moves past records it has seen, so a
down app or missed run catches up on the next run. Install as a user timer
every 15 minutes (unit files in the script's docstring); to subscribe from
another machine, serve the directory (`python3 -m http.server` in
`.cache/activity/`) or sync `feed.xml` wherever an RSS reader can reach it.
