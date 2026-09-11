# Troubleshooting

Start with the first unhealthy component in dependency order. The active stack is:
Prowlarr → NzbDAV → rclone FUSE mount → Radarr/Sonarr/Plex/Unpackerr, with Seerr
providing requests.

## Everything is unhealthy

```bash
docker compose ps
docker compose config --quiet
df -h
```

Check the first failure in this order:

1. `prowlarr` and `nzbdav` healthchecks
2. `nzbdav_rclone` mount health
3. Radarr/Sonarr/Plex/Unpackerr
4. Seerr

## Plex shows red trash cans or missing files

This is usually a scan performed while the FUSE mount was unavailable. Do not empty
trash or rescan until the mount is healthy:

```bash
docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav
docker exec nzbdav_rclone ls /mnt/remote/nzbdav | head
```

Recovery:

```bash
docker compose restart nzbdav nzbdav_rclone
# Wait for the mount healthcheck, then restart consumers.
docker compose restart radarr sonarr plex unpackerr
# Trigger a Plex library scan, then empty trash only after expected files reappear.
```

## Mount gone while the container is Up

The rclone process can fail between Docker restarts. The mountpoint check is authoritative:

```bash
docker inspect --format '{{.State.Health.Status}}' nzbdav_rclone
docker logs --tail=100 nzbdav_rclone
```

The rclone entrypoint clears stale FUSE state before mounting. If it still loops, check
for stale host mounts and follow the FUSE recovery procedure in
[docs/services/nzbdav-rclone.md](../services/nzbdav-rclone.md).

## NzbDAV returns 401

Verify both the running environment and the client key:

```bash
docker exec nzbdav env | grep FRONTEND_BACKEND_API_KEY
grep '^FRONTEND_BACKEND_API_KEY=' .env
```

Radarr and Sonarr must use the same value as `FRONTEND_BACKEND_API_KEY`. NzbDAV’s WebDAV
remote uses the credentials in `config/nzbdav-rclone/rclone.conf`.

## NzbDAV queue is not accessible

```bash
KEY=$(grep '^FRONTEND_BACKEND_API_KEY=' .env | cut -d= -f2)
curl -s "http://localhost:3000/api?mode=queue&output=json&apikey=$KEY"
```

Do not recreate NzbDAV while the queue is unknown. Use
`scripts/check_nzbdav_queue.py` or the guarded update script first.

## Radarr/Sonarr imports are stuck

```bash
docker compose logs --tail=100 radarr sonarr nzbdav unpackerr
docker exec nzbdav_rclone ls /mnt/remote/nzbdav/completed-symlinks | head
```

Confirm the download client is `nzbdav:3000`, the API key matches, the FUSE mount is
healthy, and the root folders are `/data/movies` and `/data/shows`.

For *arr items stuck in the queue as completed (including the "matched by ID —
manual import required" class, where auto-import is impossible by design), drain
them through the manual-import path with `scripts/drain_sonarr_queue.py` (dry-run
by default; `--apply` acts). The script serves both apps via `--app`
(`sonarr` default, `radarr` supported); the matching `$SONARR_API_KEY` /
`$RADARR_API_KEY` env var is used when `--api-key` is omitted:

```bash
python3 scripts/drain_sonarr_queue.py                      # sonarr dry-run (default)
python3 scripts/drain_sonarr_queue.py --app radarr         # radarr dry-run
python3 scripts/drain_sonarr_queue.py --apply --limit 10   # act on sonarr
python3 scripts/drain_sonarr_queue.py --app radarr --apply --limit 10 # act on radarr
python3 scripts/drain_sonarr_queue.py --apply --auto-safe  # provable items only
```

`--auto-safe` restricts imports to items whose file name parses unambiguously to
 the queue item's own series/movie (re-checked through the *arr's parse API,
 since the manual-import preview trusts the grab history — the same claim the
 auto-import guard refuses for "matched by ID" items). Anything unprovable is
 left queued for manual review, never removed.
### "Matched by ID — automatic import is not possible"

A queue item that reports **"Found matching series via grab history, but release
was matched to series by ID. Automatic import is not possible"** (Sonarr) or
**"matched to movie by ID — Manual Import required"** (Radarr) is *not* an
NzbDAV handoff failure. It is an *arr-side import guard firing on purpose.

**What the guard does.** When the *arr searches an indexer, the indexer can
report that a release carries the TVDb/IMDb (or TMDB) ID for the series/movie
you wanted. Sonarr/Radarr grabs on that ID claim, but at import time it parses
the downloaded file's own title — and if that title does not match the
series/movie the ID points to, the *arr refuses to auto-import
("matched by ID … automatic import is not possible"; upstream notes this guard
landed for Sonarr in v3.0.7, commit `2a45b61` / issue
[Sonarr/Sonarr#4935](https://github.com/Sonarr/Sonarr/issues/4935)). The
importer cannot confirm the file's contents belong to the monitored series, so
it demands manual review. This is deliberate anti-pollution: it is exactly what
keeps wrong-show content out of the library.

**Why the NzbDAV handoff is not the cause.** The SABnzbd protocol carries no
per-episode/file metadata — an add is only `nzbname` + `category` + `priority`,
and NzbDAV's schema stores just `Category` + `JobName`, exactly like real
SABnzbd. Correlation between the two systems is by **download ID GUID**, and it
works: the `DownloadId` recorded in each *arr's `DownloadHistory` *is* NzbDAV's
own item ID (`NzbNames.Id` — verified case-insensitively against the live DBs on
both apps). Normal title-matched grabs import end to end through this path with
no manual help; only the ID-matched class trips the guard, and it would trip
identically against real SABnzbd.

**Why ID-matching happens.** Releases end up ID-matched when the indexer's
ID claim is the only link to the monitored series — most often after a bulk
`UserInvokedSearch` "search missing" sweep, which queries indexers by ID
(`tvdbid=`/`tmdbid=`) across many series at once. Real examples from the
2026-09-01 ~230-item pile-up: `ARK: The Animated Series` grabbed into *Trailer
Park Boys: The Animated Series*, `The.Ticket` into *The Tick*, year-suffixed
`Trailer.Park.Boys.2001`, and franchise variants like `Too.Hot.to.Handle.Italy`.
Run through the manual-import drain, ~220 of that sweep's items resolved to
correct episodes and imported, while 19 were wrong-show downloads and were
rejected — the guard had been holding both classes in the queue.

**Why the title can't always be fixed with an alias.** Sonarr's alternate titles
are a shared, community-maintained dataset — there is deliberately no
per-instance "add an alias" field (upstream devs have declined to add one), so
a missing alias is fixed by requesting it upstream so every Sonarr install
benefits. `scripts/alias_candidates.py` mines recent grab history for exactly
these candidates: it replays distinct grabbed titles through Sonarr's parse
API and lists the variant titles whose parse resolves to nothing (rc 1 when
any are found).
Radarr is similar: movies match against the indexer-reported tmdbid /
imdbid plus TMDB alternate titles, with no local alias override. In practice
for this stack:

1. **Verify which class the item is.** If the file is genuinely a *different*
   show/movie than the monitored one (ARK inside Trailer Park Boys), it is a bad
   indexer match — remove/blocklist it, and optionally report the mislabel to
   the indexer. If it is the *right* content under a title the *arr won't
   title-match (year suffix, alternate spelling), the durable fix is requesting
   an alias upstream (see the Sonarr FAQ's "Why can't Sonarr import episode
   files for series X" entry for the request path).
2. **Prefer episodic/season-scoped searches** over whole-series "search missing"
   sweeps; mass sweeps are what give indexers the opening to return wrong-show
   releases. Title-matched RSS grabs are the healthy steady state and need no
   alias at all.
3. **Drain the residue** with `drain_sonarr_queue.py` above — its manual-import
   preview parses each real file, so correct episodes import and wrong-show files
   are rejected, mirroring what the guard protects. Running the drain in its
   default dry-run is also a cheap way to *detect* the class: it lists every
   completed-and-stuck item with its resolution state.

Catch the class early by watching the *arr queue directly (the drain dry-run,
or the UI Activity → Queue view) — a pile-up grows silently until something
looks at it.

### Bulk "Search Missing" sweeps grab wrong shows — search scoped instead

A whole-series or whole-library "Search Missing" blast sends every missing
episode through the indexer at once and auto-grabs whatever comes back.
Releases whose titles don't parse to the searched series fall back to the
"matched by ID" path at grab time (the *arr uses the search criteria's TVDb
ID when the title won't match), which auto-import refuses — the 2026-09-01
sweep left 230 stuck items, ~190 GB of which was later mis-imported as
wrong-show content (ARK into Trailer Park Boys: The Animated Series, The
Sticky into The Tick, the 2022 Staircase dramatization into the 2004
documentary).

Prefer `scripts/search_missing_scoped.py` for any sweep: it searches the
same way Sonarr does internally (per series/season) but in small batches
(`--batch 20`), pauses between batches (`--gap 60`), stops for review after
each batch unless `--yes`, and `--verify` re-parses every new queue item's
title and aborts the sweep — rc 2 — the moment anything is NO_MATCH or a
different series than the one searched. Applied runs persist an atomic
`~/.local/state/thebearcave/search-missing-scoped.json` checkpoint before
posting (override it with `--checkpoint-path`). After the review stop, continue
only with the same scope and options plus explicit `--resume`;
completed and verified groups are skipped, while corrupt or ambiguous state
fails closed without posting. Dry-run by default; only `--apply` POSTs search
commands:

```bash
python3 scripts/search_missing_scoped.py --series 25891                  # dry-run, one series
python3 scripts/search_missing_scoped.py --all --batch 10                # dry-run, whole library
python3 scripts/search_missing_scoped.py --series 25891 --apply --verify  # scoped sweep + guard
python3 scripts/search_missing_scoped.py --series 25891 --apply --verify --resume
python3 scripts/search_missing_scoped.py --all --apply --yes             # no checkpoints
```

Manually: search one series at a time (never the whole Wanted list) and
check the queue between series; prefer season-scoped searches, whose
season packs tend to title-match cleanly. If a checkpoint reports an
ambiguous group with no command ID, stop and inspect it rather than deleting
or editing the file: Sonarr cannot prove whether that POST reached the
server, so the wrapper intentionally refuses to guess.

The executable interface in `scripts/search_missing_scoped.py` only parses
arguments and renders results. `scripts/search_missing_scoped_core.py` owns
Sonarr planning, command lifecycle, and verification; the atomic state-file
logic lives in `scripts/search_missing_scoped_checkpoint.py`.

Three title variants from the Sep-1 burst are *genuine* scene-mapping
candidates — right content under a variant title. Adding them to the shared
scene-mapping dataset makes future grabs title-match and auto-import:

* `Chirurgiens.D.Exception` → The Surgeon's Cut (French title)
* `Trailer.Park.Boys.Out.of.the.Park.Europe` → Trailer Park Boys: Out of the Park
* `Transformers.War.For.Cybertron.Trilogy.Earthrise` → Transformers: War For Cybertron Trilogy

When a bad grab still lands, the backstop is the drain's safe mode:
`drain_sonarr_queue.py --apply --auto-safe` imports only queue items whose
file parses provably to their own series and leaves everything else queued
for manual review.

## Seerr requests do not download

Check Seerr’s connections to Plex, Radarr, and Sonarr, then inspect the relevant *arr
queue and NzbDAV queue. Seerr itself should respond at `http://HOST_IP:5055`.

## Hardware transcoding falls back to software

```bash
docker exec plex ls -l /dev/dri
```

Plex Pass, GPU drivers, `/dev/dri`, and hardware acceleration in Plex settings are
required. Library scans still use CPU even when playback uses VAAPI.

## Container will not stop

Plex has a required 90-second grace period. Give it time before escalating:

```bash
docker stop -t 90 plex
```

For a wedged FUSE handle, restore/restart the mount owner first, then restart consumers.

## Escalation order

1. `docker compose ps`
2. Check the NzbDAV queue before any recreate.
3. Check the FUSE mount before any Plex scan.
4. Inspect logs for the first unhealthy service.
5. Restore from backup before destructive cleanup; see
   [backup & restore](backup-restore.md).
