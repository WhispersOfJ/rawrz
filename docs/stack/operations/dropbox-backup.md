# Dropbox Streaming Backup

An unattended, offsite snapshot of The Bear Cave repo that **never touches
local disk** beyond reading the source: one tar archive is streamed straight
to Dropbox via the upload-session API, chunk by chunk (8 MB), then the oldest
snapshots are pruned so a bounded window is retained.

- Engine: `scripts/backup_dropbox.py` — self-contained; mirrors the
  `dropbox-uploader/` folder's env-vars / endpoint / retry / exit-code
  conventions (see `dropbox-uploader/CLAUDE_CODE_PROMPT.md`) but streams with
  a two-chunk sliding window instead of buffering, so archives of any size
  stay flat in RAM.
- Scheduler: daily systemd **user** timer at 02:30 via
  `scripts/install-dropbox-backup-timer.sh` (after the backup doc's 04:00
  reclaim runs nothing — backups are taken first).
- Tests: `python3 scripts/test_backup_dropbox.py` (offline; runs a real GNU
  tar over a fake checkout and asserts the exclusion model end to end).

## What is (and isn't) in a snapshot

A snapshot contains **everything under the checkout that is not media,
generated metadata, or secrets** — including `.git` history and untracked
files on disk (e.g. `dropbox-uploader/`, `bear-i3-config.zip`).

| Excluded | Why | Members |
|---|---|---|
| Huge trees | `config/plex` (~metadata), `config/nzbdav` (148 GB state), `config/_backup-*`, `config/plex-transcode` | `./config/plex`, `./config/nzbdav`, `./config/_backup-*`, `./config/plex-transcode` |
| Media | Remote-backed / re-indexable; would blow any quota | `./media` |
| Secrets | Never leave the host | `.env`, `.env.local`, `.env.*.local`, `secrets/`, `docker-compose.override.yml`, `config/nzbdav-rclone/rclone.conf` |
| Runtime state | Regenerable / git-managed | `data/`, `logs/`, `usenet/`, `backups/`, `.cache/`, `.memsearch/`, `.freebuff/`, `.ruff_cache/`, `.worktrees/`, `tmp/` |
| Generated metadata inside kept config dirs | Library DB state is the "metadata" you asked to skip; settings ride along | `config/*/*.db*` (+ one level deeper), `config/*/Backups`, `config/*/backup*`, `config/*/restore`, `config/*/logs`, `*.log` (all lifted only by `--include-dbs`) |
| Regenerable cache / artwork | Poster/cover artwork, crash dumps, caches inside kept config dirs — `config/radarr/MediaCover` alone was 25 GB, `config/sonarr/MediaCover` 2.9 GB | `config/*/MediaCover`, `config/*/Sentry`, `config/*/cache`, `config/*/.cache` — **never** lifted, not even by `--include-dbs` |
| Code hygiene | — | `__pycache__/`, `*.pyc` |

Kept per-app settings therefore include `config/{radarr,sonarr,prowlarr,
seerr}/*.{xml,ini,json}` — the small files that encode how the stack
is configured — while the sqlite databases, WAL/SHM files, internal
`Backups/` directories of DB copies, and the poster-artwork trees
(observed 2026-09-04: `config/radarr` 30 GB of which `MediaCover` alone is
25 GB, `config/sonarr` 13 GB/2.9 GB) stay out.

If you *do* want the databases shipped, run with `--include-dbs` — it lifts
only the metadata category (DBs + their backup/log dirs); media, secrets,
the huge trees, and the regenerable artwork/cache category are never
included regardless. Expect a much larger and slower upload (and check your
Dropbox quota: those DBs alone exceed the free tier).

## Auth

1. Create an app at <https://www.dropbox.com/developers/apps> → *Scoped
   access* → *Full Dropbox*.
2. Permissions tab → enable `files.content.write` (+ `files.content.read` /
   `files.content.delete` if you want the automatic retention prune).
3. Either generate a long-lived **access token** (Settings → *Access token*),
   or set up the OAuth2 refresh trio (recommended for unattended use).
4. Provide the credentials to the engine via environment:

```bash
export DROPBOX_ACCESS_TOKEN=...            # simplest
# or all three:
export DROPBOX_REFRESH_TOKEN=...
export DROPBOX_APP_KEY=...
export DROPBOX_APP_SECRET=...
```

The timer installer keeps them in `~/.config/thebearcave-dropbox.env`
(chmod 600, never in the repo) and the service unit loads that file via
`EnvironmentFile`.

**Gotchas learned live (2026-09-04) — read before creating the app:**

1. **Enable the scopes BEFORE generating the token.** Dropbox does not
   reliably apply scope changes to an already-issued access token: a token
   created before `files.content.write` was enabled will keep failing the
   upload (HTTP 400 naming the missing scope, then HTTP 401 once the scope
   is added) until you **revoke and regenerate** it on the Settings tab.
2. **The error message tells you which app is at fault** — it names the app
   ID. If the ID differs from the app you think the token belongs to, the
   wrong token is in the environment.
3. **Validate the token before the first upload** with the account check
   (below) — HTTP 200 proves the token and account; it does *not* prove the
   scopes, so still expect the first upload to be the real scope test.
4. `--dry-run` needs **no credentials at all** (it discards the archive), so
   it validates tar/exclusions locally even before auth exists.

## Run

```bash
# Manual snapshot + upload + prune (newest 30 kept by default)
python3 scripts/backup_dropbox.py

# Full pass but discard the archive — measures size, verifies tar/streaming,
# uploads nothing (run this first on a fresh machine)
python3 scripts/backup_dropbox.py --dry-run

# Include the sqlite databases (lifts the metadata category only)
python3 scripts/backup_dropbox.py --include-dbs

# Options: --repo DIR · --dropbox-dir /Some/Path · --keep N (0 = never prune)
#          --overwrite · --compression gzip|none · --verbose
```

Snapshots land at `/Backups/cave/thebearcave-backup-YYYYMMDD-HHMMSS.tar.gz`.
The timestamped name means a re-run never collides; Dropbox additionally keeps
30 days of version history on top of the `--keep` prune.

**Requirements:** GNU `tar` (base) and Python `requests` — on Arch
`sudo pacman -S python-requests` (this is why `NEEDED.md` lists nothing extra:
`requests` is the one pip-style runtime dependency and is packaged in `extra`).

## Daily timer

```bash
scripts/install-dropbox-backup-timer.sh             # install (idempotent)
scripts/install-dropbox-backup-timer.sh --check     # state (exit 0 = installed)
scripts/install-dropbox-backup-timer.sh --remove    # uninstall (keeps creds)
```

The installer fails closed — no token configured, no `requests`, missing
engine → nothing gets wired. The service runs at 02:30 daily
(`Persistent=true`, so a missed run fires on next boot) and logs to the user
journal:

```bash
journalctl --user -u thebearcave-dropbox-backup.service -e
systemctl --user list-timers thebearcave-dropbox-backup.timer
```

Verify each morning that the last service result is success:
`systemctl --user status thebearcave-dropbox-backup.service`.

## Exit codes and failure behavior

| Code | Meaning |
|---|---|
| 0 | Uploaded (+ pruned) |
| 1 | Upload / archive error (retries exhausted, Dropbox 4xx/5xx, tar failure) |
| 2 | Auth / config error (no token, token refresh failed, `requests` missing) |
| 3 | Bad arguments |

On failure the tar child is terminated immediately and nothing is committed in
Dropbox (an abandoned upload session leaves no file). A partially-written
upload can only ever appear after the finish call succeeds, so a failed run
never leaves a corrupt snapshot; the prune runs only after a successful
upload. Retry policy matches the uploader helper: exponential backoff
1/2/4/8 s on HTTP 429 and 5xx and network errors; immediate exit on 401/403.

## Restore

```bash
# List / extract a snapshot (download to a pipe, extract on the fly — the
# archive itself never needs to sit on disk either):
curl -L "https://www.dropbox.com/s/<TOKEN>/<FILE>.tar.gz?dl=1" \
  | tar -tzf -                                   # preview
curl -L "https://www.dropbox.com/s/<TOKEN>/<FILE>.tar.gz?dl=1" \
  | tar -xzf - -C /some/restore/dir              # extract

# Or fetch via the Dropbox API / app and extract normally:
#   tar -xzf thebearcave-backup-20260904-073431.tar.gz -C restore/
```

Restoring into a fresh checkout: extract, then re-run
`scripts/setup.sh` to regenerate `secrets/` and `.env` — this snapshot
deliberately excludes credentials, so it restores code and configuration
*shape*, not the live secrets. See
[backup-restore.md](backup-restore.md) for the full local backup/restore
story (config DBs, Plex metadata, secrets — the things this snapshot skips).

## Troubleshooting

| Symptom | Check |
|---|---|
| Timer never fires | `systemctl --user list-timers` — is `Persistent` catching up? Is the user instance running? |
| Upload refuses with HTTP 400 naming a missing scope (e.g. `files.content.write`) | App Console → Permissions → enable the scope, then revoke + regenerate the token (existing tokens don't pick up new scopes) |
| HTTP 401 on upload while the account check returns 200 | The token predates the scope enablement — regenerate it (see Auth gotchas) |
| Exit 2 | `DROPBOX_*` env: run the installer's `--check` or `python3 scripts/backup_dropbox.py --dry-run` (fails fast at auth) |
| Exit 1 with HTTP 409 | Overwrite collision with autorename disabled — use the default (dated names, autorename on) |
| Upload slow / retries | Chunks are a fixed 8 MB — a slow uplink just takes longer; logs show per-retry backoff progress, and the session resumes across retries |
| Large unexpected tar | `--dry-run` reports the byte count; inspect what grew with the exclusion list in §What is in a snapshot |
