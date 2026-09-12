# Cave-Scripts — M1 Inventory Audit (adopt / convert / skip ledger)

Status: **Draft ledger** · Date: 2026-09-05 · Branch: `docs/cave-scripts-inventory`
Source of truth: [`cave-scripts-spec.md`](cave-scripts-spec.md) (D1 full audit, D3 whole-dir move)
Audit basis: `origin/main` @ `8aa7523` + live host state (`~/.config/*`), grep/`ls`-verified.

Every function and script surface in scope is listed with a disposition.
This ledger seeds `spec/functions.yaml` (M1 registry pass adds per-function
args/flags/output contracts/timeout/safety classes).

## Legend

| Code | Meaning |
|---|---|
| **ADOPT** | Keep; move into Cave-Scripts and port to all three shells under `cave-*` (rename `stack-*`→`cave-*`, permanent alias layer D17/D21). |
| **CONVERT** | Loose host/DE script becomes managed functions/assets (spec §6.2–6.4). |
| **SKIP-superseded** | Behaviour already covered by a live bash function or newer sibling — do not port (archive stays as history). |
| **SKIP-dead** | Targets a retired service / replaced system — do not port. |
| **SKIP-dup** | Exact duplicate of a live tree's copy — one source only (the live one). |
| **SKIP-infra** | Repo infrastructure (CI gates, installers, tests, dotfiles host files) that stays in thebearcave — not part of the shell-function surface. |

## Headline counts

| Surface | Items | Disposition summary |
|---|---|---|
| A. Bash live library (`services/bash-functions/`) | 105 `stack-*` fns, 16 helper/loader fns, loader, completions, scripts/ + waybar/ assets | ADOPT (whole dir, D3) |
| B. Archive fish (`archive/media-stack/fish-functions/`) | 62 function files + 55 completions | 23 SKIP-dup (host-tools legacy) + 22 SKIP-superseded (bash legacy) + 17 SKIP (unique: dead/superseded/generic); 0 adopt; completions SKIP |
| C. Live host-tools fish (`services/host-tools/functions/`) | 25 files (23 `stack-*` + 2 `__host_*`) | ADOPT (all 25) |
| D. DE host surface | 8 hypr scripts, 7 themes, 7 waybar live scripts | CONVERT / ADOPT per §6.2 |
| E. Sway remnants (`~/.config/sway/`) | inert configs + scripts | SKIP-dead (cleanup later, not Cave-Scripts scope) |
| F. Dotfiles | `~/.bashrc`, `~/.zshrc`, (no fish config) | ADOPT via `dotfiles/` (starship conversion D22) |
| G. thebearcave root `scripts/` | 9 `.sh` + 59 `.py` | SKIP-infra (stay) — incl. `nzbdav-safe-recreate.sh` (loader resolves it from the repo root) |

**Live-tree collision check:** bash (105) ∩ host-tools (25) = **∅** — no name
collisions between the two live `stack-*` trees. Safe to rename both sets to
`cave-*` without clashes.

---

## A. Bash live library — `services/bash-functions/` → all **ADOPT** (D3 whole-dir move; M2 rename + zsh/fish ports)

### A1. Functions (`functions/stack-*.sh`, 105 defs) — ADOPT (rename `cave-*`, alias `stack-*`)
| File | Functions (→ `cave-*`) |
|---|---|
| `stack-arr-1.sh` | `stack-arr-backlog`, `stack-arr-recently-added`, `stack-arr-toggle-search`, `stack-arr-blocklist`, `stack-arr-clear-blocklist`, `stack-arr-missing-aired`, `stack-cutoff-unmet` |
| `stack-arr-2.sh` | `stack-arr-import`, `stack-arr-import-all`, `stack-arr-import-candidates`, `stack-arr-import-starvation`, `stack-arr-logs` |
| `stack-arr-3.sh` | `stack-arr`, `stack-backlog-status`, `stack-command-queue-summary`, `stack-import-lists`, `stack-radarr-health`, `stack-radarr-prune`, `stack-sonarr-prune`, `stack-prowlarr-indexers` |
| `stack-arrivals.sh` | `stack-arrival-notify`, `stack-activity-feed` |
| `stack-core.sh` | `stack-status`, `stack-container`, `stack-restart-all`, `stack-top`, `stack-version`, `stack-help` |
| `stack-disk.sh` | `stack-disk-config-sizes`, `stack-docker-disk-usage`, `stack-disk-reclaim`, `stack-nzbdav-dedup-check`, `stack-nzbdav-delete-failures` |
| `stack-lists.sh` | `stack-mdblist-track/untrack/tracked/import/history`, `stack-letterboxd-track/untrack/tracked/import/history` |
| `stack-loop-ratings.sh` | `stack-loop-candidates`, `stack-loop-exclude`, `stack-loop-unmonitor`, `stack-tmdb-missing`, `stack-rating-imdb`, `stack-rating-mdblist` |
| `stack-maintenance.sh` | `stack-maintenance-digest`, `stack-audit-residue`, `stack-config-drift` |
| `stack-misc.sh` | `stack-seerr-requests`, `stack-notify-test`, `stack-claude-full-backup`, `stack-worktree`, `stack-image-check`, `stack-perms-check`, `stack-oom-check`, `stack-resource-check`, `stack-log-levels` |
| `stack-nzbdav.sh` | `stack-nzbdav-queue`, `stack-nzbdav-history`, `stack-nzbdav-stats`, `stack-mount-health` |
| `stack-plex-core.sh` | `stack-plex-sessions`, `stack-plex-recently-added`, `stack-plex-libraries`, `stack-plex`, `stack-plex-butler`, `stack-plex-butler-all` |
| `stack-plex-extra.sh` | `stack-plex-duplicates`, `stack-plex-garbage-collect-media`, `stack-plex-garbage-collect-blobs`, `stack-plex-backup-database`, `stack-plex-automatic-updates`, `stack-plex-process-assets`, `stack-plex-refresh-epg`, `stack-plex-refresh-local-media`, `stack-plex-clean-cache-files`, `stack-plex-clean-log-files`, `stack-plex-image-clean`, `stack-plex-deep-media-analysis`, `stack-plex-upgrade-media-analysis`, `stack-plex-music-analysis`, `stack-plex-loudness-analysis`, `stack-plex-generate-media-index`, `stack-plex-generate-voice-activity`, `stack-plex-generate-intro-markers`, `stack-plex-generate-credits-markers`, `stack-plex-generate-ad-markers`, `stack-plex-generate-chapter-thumbs` |
| `stack-plex-markers.sh` | `stack-plex-markers` |
| `stack-plex-updates.sh` | `stack-plex-updates`, `stack-plex-analyze`, `stack-plex-empty-trash`, `stack-plex-refresh-libraries`, `stack-queue-autofix`, `stack-sonarr-fix-episode-monitoring` |
| `stack-queue.sh` | `stack-queue-status`, `stack-arr-queue-errors` |
| `stack-watchable.sh` | `stack-watchable`, `stack-unwatched`, `stack-recent`, `stack-requests` |

### A2. Helpers / loader — ADOPT (port per shell; `__*` internals stay shell-private, `fmt_*` shared)
- `__helpers.sh`: `__arr_api`, `__arr_api_key`, `__arr_api_url`, `__nzbdav_api`, `__plex_api`, `__seerr_api`, `__stack_arr_app`, `__stack_containers`, `__stack_curl`, `__plex_butler`, `backup`, `copy`, `docker` (guarded wrapper) — ADOPT
- `__metadata.sh`: `__stack_metadata` — ADOPT
- `bearcave-bash.sh` loader: `__bearcave_load_env`, `__bearcave_warn_stale_keys`, `_fmt_color_enabled`, `fmt_heading`, `fmt_success`, `fmt_error`, `fmt_warning`, `fmt_dim`, `fmt_kv`, `fmt_status_dot` — ADOPT (becomes `bash/cave-scripts.sh`; zsh/fish loaders in M2)

### A3. Non-function files — ADOPT (move with D3)
- `completions/stack-completions.sh` → generated; regenerate per shell (bash compgen / zsh compdef / fish `complete`)
- `scripts/gen-bash-completions.sh` → becomes per-shell generators + `--check` gates
- `scripts/stack-tui/` → ADOPT (moves; waybar/stack-tui launcher)
- `waybar/{config,style.css,README.md, scripts/{nightlight-status,record-status,stack-tui-toggle}.sh, sway/stack-tui.conf}` → asset move to `de/waybar/` (D14; config already Hyprland-migrated, #172). `sway/stack-tui.conf` is removed instead of moved — see L3.
- `__pycache__` → gitignored, not moved

---

## B. Archive fish — `archive/media-stack/fish-functions/` (62 function files, reconciled)

Set math (name-level): **23** duplicate live host-tools + **22** legacy copies of
live bash functions + **17** unique to the archive = **62**. Nothing here is
adopted: the archive is history (FISH.md retirement); every live behaviour
resides in A or C. Completions (55) also skipped.

### B1. Duplicates of live host-tools (23) — **SKIP-dup**
`stack-aur-audit`, `stack-claude-home`, `stack-cron-list`, `stack-disk-free`,
`stack-disk-health`, `stack-firewall-status`, `stack-flatpak-updates`,
`stack-git-status-all`, `stack-journal-errors`, `stack-journal-size`,
`stack-kernel-check`, `stack-mem-pressure`, `stack-pkg-clean-cache`,
`stack-pkg-history`, `stack-pkg-orphans`, `stack-pkg-update`,
`stack-pkg-updates`, `stack-reboot-check`, `stack-service-failed`,
`stack-ssh-doctor`, `stack-timer-status`, `stack-uptime-report`,
`stack-zombie-check`
→ Live source is `services/host-tools/functions/` (C). Archive copy = legacy snapshot.

### B2. Legacy copies of live bash functions (22) — **SKIP-superseded**
`stack-claude-full-backup`, `stack-container`, `stack-disk-config-sizes`,
`stack-docker-disk-usage`, `stack-help`, `stack-image-check`,
`stack-log-levels`, `stack-mount-health`, `stack-notify-test`,
`stack-nzbdav-dedup-check`, `stack-nzbdav-delete-failures`,
`stack-nzbdav-history`, `stack-nzbdav-queue`, `stack-nzbdav-stats`,
`stack-oom-check`, `stack-perms-check`, `stack-queue-status`,
`stack-resource-check`, `stack-restart-all`, `stack-status`, `stack-top`,
`stack-version`
→ Pre-retirement fish originals; the live bash ports (A1) supersede them.

### B3. Unique to the archive (17) — **SKIP** (reasons per item)
- Fish-generic conveniences (4) — **SKIP-infra**, defer as candidate `dotfiles/` additions under D19: `copy`, `history`, `__history_previous_command`, `__history_previous_command_arguments`
- Retired-service / dead (6) — **SKIP-dead**: `stack-alacritty-theme` (theme switching moved to hypr/waybar), `stack-cleanuparr-instances`, `stack-cleanuparr-strikes` (Cleanuparr unsupported for this Usenet-only stack), `stack-watchstate-history`, `stack-watchstate-import-now`, `stack-watchstate-status` (watchstate retired 2026-08-30)
- Superseded helpers (3) — **SKIP-superseded**: `__stack_api`, `__stack_arr_app` (bash A2 equivalents), `__stack_containers` (bash helper A2; listed here because the bash set file excluded `__*`)
- Superseded misc (4) — **SKIP-superseded**: `stack-arr-import-backlog` (A1 import/backlog family), `stack-file-backup` (`stack-claude-full-backup` + `scripts/backup.sh` + `backup_dropbox.py`), `stack-pkg-cleanup` (live `stack-pkg-clean-cache`), `stack-tmdb-audit` (`stack-tmdb-missing`/loop-ratings family)

### B4. Completions (`completions/`, 55 files) — **SKIP-superseded**
Static manual completions; replaced by generated completions for all three shells (M2). Commands completed are covered by A/C.

---

## C. Live host-tools fish — `services/host-tools/` → all **ADOPT** (M1 registers, M2 ports bash/zsh + renames `cave-*`)

Helpers: `__host_containers`, `__host_helper` → ADOPT.
Functions (23): `stack-aur-audit`, `stack-claude-home`, `stack-cron-list`,
`stack-disk-free`, `stack-disk-health`, `stack-firewall-status`,
`stack-flatpak-updates`, `stack-git-status-all`, `stack-journal-errors`,
`stack-journal-size`, `stack-kernel-check`, `stack-mem-pressure`,
`stack-pkg-clean-cache`, `stack-pkg-history`, `stack-pkg-orphans`,
`stack-pkg-update`, `stack-pkg-updates`, `stack-reboot-check`,
`stack-service-failed`, `stack-ssh-doctor`, `stack-timer-status`,
`stack-uptime-report`, `stack-zombie-check` → ADOPT → **`cave-sys-*`**
(e.g. `cave-sys-pkg-update`, `cave-sys-disk-free`), alias `stack-*`
retained (L2).

Notes: pure-host diagnostics (no API dependency). Mutating members already
carry guards (`stack-pkg-update --yes`, `stack-pkg-orphans --remove`,
`stack-flatpak-updates --apply`, `stack-pkg-clean-cache`) → registry marks
them `mutating-safe`/`destructive-confirm`. Also ships
`scripts/{install,uninstall}.sh` → fold into the Cave-Scripts per-shell
installer (M2); `README.md` + `docs/stack/services/host-tools.md` content moves
into Cave-Scripts docs.

**Naming (L2):** host diagnostics are renamed **`cave-sys-*`** — e.g.
`cave-sys-pkg-update`, `cave-sys-disk-free`, `cave-sys-mem-pressure`,
`cave-sys-service-failed` — not plain `cave-*`, so the host health domain is
distinct from the media families that map mechanically (`cave-disk-config-sizes`,
`cave-docker-disk-usage`). `stack-*` aliases retained for all 25.

---

## D. DE host surface (live, `~/.config`) → **CONVERT** per spec §6.2 (all three shells) + **ADOPT** as assets

### D1. hypr scripts (`~/.config/hypr/scripts/`, 8) — CONVERT → functions
| Script | → Function family | Notes |
|---|---|---|
| `build-themes.sh` | `cave-theme build` | palettes source for 6 themes |
| `render-theme.sh` | `cave-theme render` | generates hyprlock/waybar/swaync/wofi/alacritty assets per theme |
| `hyprtheme` | `cave-theme set/list/current/pick` + `auto-from-image` | 6 palettes in cycle; `auto` excluded from cycle (D18) |
| `idle-toggle.sh` | `cave-idle-toggle state/toggle/reload` | hypridle presentation mode |
| `gammastep-toggle.sh` | `cave-nightlight state/toggle` | |
| `recorder-toggle.sh` | `cave-record toggle` | pairs with `record-status.sh` |
| `cliphist-pick.sh` | `cave-cliphist pick` | |
| `power.sh` | `cave-power lock/logout/suspend/reboot/shutdown` | guarded per action |

### D2. Themes (`~/.config/hypr/themes/`, 7) — ADOPT assets → `de/hypr/themes/`
`auto` (matugen/Material-You, generated from an image — NOT time-based), `catppuccin-mocha`, `dracula`, `gruvbox`, `nord`, `rose-pine`, `tokyonight`.

### D3. Waybar live scripts (`~/.config/waybar/scripts/`, 7) — ADOPT → `de/waybar/scripts/`
`record-status.sh`, `nightlight-status.sh`, `stack-tui-toggle.sh` (already mirrored in-repo — canonical moves), `weather.sh`, `updates.sh`, `cava-bar.sh`, `gpu-usage.sh` (host-only today; the merged config already execs them — D8 adoption closes the gap). Waybar `config` + `style.css` move as assets (style.css is theme-generated per D14 caveat).

### D4. Sway remnants (`~/.config/sway/`) — **SKIP-dead**
Inert post-migration files (`config`, `config.bak-*`, `idle-toggle.sh`, `swayidle.sh`, `stack-tui.conf`, `backup-20260904`). Not part of Cave-Scripts; delete in M4 housekeeping (after `cave-bar-check` guards the live config). Exception per **L3**: `stack-tui.conf` (tracked + live) is removed **now** as a standalone chore, and the drift checker's sway support is dropped in the same change.

---

## E. Dotfiles — ADOPT via `dotfiles/` (M2, D10/D22)
- `~/.bashrc` loader snippet → `dotfiles/.bashrc.inc`
- `~/.zshrc` (p10k instant prompt + CachyOS config) → `dotfiles/.zshrc.inc` with **starship** conversion (p10k retired D22; CachyOS config source kept)
- fish: no config today → create `dotfiles/config.fish` (fish becomes default shell, D19)
- shared `starship.toml`; idempotent `install.sh`

## F. thebearcave root `scripts/` (9 `.sh`, 59 `.py`) — **SKIP-infra** (stay in thebearcave)
`backup.sh`, `install-dropbox-backup-timer.sh`, `install-git-hooks.sh`,
`install-nightly-reclaim-cron.sh`, `preflight.sh`, `setup.sh`,
`test_setup.sh`, `update-nzbdav.sh`, `nzbdav-safe-recreate.sh` + 59 python
checkers/backends (CI gates, `backup_dropbox.py`, maintenance backends).
→ Not part of the shell-function surface. The bash loader resolves
`nzbdav-safe-recreate.sh` from the repo root via `BEARCAVE_REPO_DIR`, so it
stays behind the guarded `docker` wrapper across the migration. Cave-Scripts'
`cave-backup` family shells out to `backup_dropbox.py` by repo path (D24).

---

## Decision log — open items resolved (2026-09-05)
1. **L1 — Archive-vs-bash parity:** full line-by-line body diff of every paired archive-fish ↔ bash function during **M2**, before the archive fish tree is finalised as history. Nothing is dropped silently; pairs are checked at port time, not now.
2. **L2 — Host-tools naming:** the 23 host functions + 2 helpers are renamed **`cave-sys-*`** (e.g. `cave-sys-pkg-update`, `cave-sys-disk-free`, `cave-sys-mem-pressure`) so host health commands are visually distinct from the mechanically-renamed media families (`cave-disk-config-sizes` etc.); `stack-*` aliases retained. See section C.
3. **L3 — `sway/stack-tui.conf`:** removed **now** as a standalone chore in thebearcave — delete the tracked `services/bash-functions/waybar/sway/stack-tui.conf` + live `~/.config/sway/stack-tui.conf`, drop sway support from `scripts/check_waybar_drift.py` (+ `test_check_waybar_drift.py`), update the waybar README. Tracked-file list in §A3 and §D4 updated accordingly.
4. **L4 — Fish conveniences:** `copy` (DIR1 DIR2), the timestamped `history` wrapper, and `__history_previous_command*` are **adopted into Cave-Scripts `dotfiles/`** as fish-only conveniences (outside the function registry) when `config.fish` is built in M2 (D19).
