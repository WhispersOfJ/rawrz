# Cave-Scripts — Specification

Status: **Draft for approval** · Date: 2026-09-05 · Request owner: bear
Authoring worktree: `/home/bear/cave/.worktrees/cave-scripts-spec` (branch `docs/cave-scripts-spec`)

This spec captures the interview answers, researched facts, and design
decisions behind the request. The next deliverable — the multi-part,
pre-researched plan for approval — will be built directly from this file.

---

## 1. Request summary (as understood)

> Extract the entire shell-operational surface of the Bear Cave media stack —
> **every function and script** — out of `thebearcave` and into a new
> standalone **public** GitHub repo **`WhispersOfJ/Cave-Scripts`**, kept as a
> git **submodule** of `thebearcave`. Implement the full library as **three
> genuine, hand-maintained ports — bash, zsh, and fish** — extending coverage
> to all recent Hyprland-switchover work (session, lock/idle, themes,
> recorder/clipboard/media, power, and the waybar bar incl. its live status
> scripts), plus **btrfs** function families (guarded: read-only by default,
> destructive only on explicit acknowledgement). Move the bar/session config
> **assets** in as the versioned source of truth with sync/install commands.
> Prove correctness with per-shell **offline test suites** as the merge gate
> and a **read-only live matrix** run at phase milestones against the real
> stack. Deliver as a **few big milestones** (not per-phase approvals), each
> demoed before the next begins.

## 2. Interview decisions (recorded answers)

| # | Question | Decision |
|---|---|---|
| D1 | Source inventory | **Everything — full audit.** Bash library + DE/session scripts + retired archived fish + the **live `services/host-tools` fish library (25 functions)** + every remaining shell script, audited one by one and classified adopt / convert / skip-with-reason. |
| D2 | Port model | **3 hand-maintained ports** (bash, zsh, fish) translated from one canonical spec; no transpilers. |
| D3 | What moves | **The whole current `services/bash-functions/` dir** — functions, `__*` helpers, loader, completions, and `scripts/` incl. python-backed commands. |
| D4 | Repo model | **Standalone repo + submodule.** `WhispersOfJ/Cave-Scripts` is its own repo; `thebearcave` pins it as a submodule. |
| D5 | Migration cutover | **Move out, update refs.** `services/bash-functions/` is removed from `thebearcave`; every reference (loader path, CI, tests, docs, cron) repointed at Cave-Scripts. |
| D6 | Remote scope | **Public** (like `thebearcave`). No secrets anywhere in the new repo. |
| D7 | DE function surface | **Everything installed for the Hyprland switchover** — session control, lock/idle, themes/nightlight, recorder/clipboard/media, power/bar (full list in §6). |
| D8 | Bar scope | **Waybar + live status scripts** — control functions plus conversion of record/nightlight/weather/updates/cava/gpu-usage/stack-tui scripts. |
| D9 | btrfs scope | **All sensible families**, guarded — read-only/info by default; mutating actions (scrub/balance start, snapshot delete, send/receive writes) require explicit confirmation. |
| D10 | Shell depth | **Full parity incl. completions + unified dotfiles** — loaders, .env loading, guarded docker wrapper, `fmt_*` helpers, tab-completions (bash/zsh/fish), and shared dotfile management for all three shells. |
| D11 | Live testing | **Both, gated by phase.** Offline per-shell suites are the merge gate; read-only live matrix runs at milestone demos. |
| D12 | Plan shape | **Few big milestones** with demo sessions (see §11) — this is the shape of the upcoming approval plan. |
| D13 | Tests & CI home | **Split by kind.** Offline function tests → Cave-Scripts; live/integration tests needing `.env` + running services stay in `thebearcave`. |
| D14 | Config assets | **Move assets + add sync.** waybar + hypr/theme config assets become the versioned source of truth inside Cave-Scripts, with sync/install commands pushing repo → live and reloading. |
| D15 | License | **MIT, © 2026 WhispersOfJ** — mirrors thebearcave's LICENSE exactly. |
| D16 | Submodule mount path | **`services/cave-scripts`, no compatibility shim.** All 23 referencing files (grep-verified on main, 2026-09-05) repointed in one M4 cutover commit; the old `services/bash-functions` path is deleted. |
| D17 | Naming | **Full `cave-*` rename** — every function (media, DE, btrfs) becomes `cave-*`; `stack-*` kept only as deprecated aliases for muscle memory; docs/tests/completions follow the new names. |
| D18 | `auto` theme | **Real theme + from-image regeneration.** `auto` is a matugen/Material-You theme generated from an image; `cave-theme auto-from-image <img>` regenerates it; selectable explicitly but **excluded from `cave-theme cycle`** (cycle = six palettes). |
| D19 | Fish's role | **Make fish the default interactive/login shell.** fish gets full dotfiles; `chsh` lands as an explicit, reversible step at the end of M4; bash and zsh remain installed and fully working. |
| D20 | btrfs backup model | **Stand up a snapper `home` config for `@home` now** (snapper `root` already covers `@`); snapshot cadence set later under D23. The mount-gated `send`/`receive` idea was superseded by D24 (Dropbox). |
| D21 | Alias lifecycle | **Keep `stack-*` aliases permanently** as documented deprecated aliases — no removal sweep; covered by the registry + offline suites forever. |
| D22 | Fish prompt | **starship (cross-shell)** — one shared `starship.toml` for bash, zsh, AND fish; Powerlevel10k is retired from the zsh dotfiles. |
| D23 | Snapper `home` cadence | **Automatic timeline snapshots** for `@home` (hourly 5 / daily 7 / weekly 4 / monthly 3) plus the manual `cave-btrfs` snapshot functions; the `root` config stays untouched. |
| D24 | Backup target | **Dropbox IS the backup target** — no btrfs `send`/`receive` to a device; a `cave-backup` wrapper family (all 3 shells) drives `~/cave/scripts/backup_dropbox.py` (run / dry-run / status). Revises D20's send/recv clause. |
| D25 | Submodule bump cadence | **Bump per Cave-Scripts tag** — Cave-Scripts gets its own release-please tagging; `thebearcave` pins and bumps the latest tag via PR. |

## 3. Goals & non-goals

### Goals
- One canonical, versioned home for the whole shell operational surface, usable from **three shells** interchangeably with identical behaviour, output, and safety guarantees.
- Full coverage of the post-switchover desktop: Hyprland session, waybar bar + live status scripts, themes, recorder/clipboard/media, power — as functions, not loose scripts.
- btrfs awareness for this host (`@home` on `/home`, snapper-active) with a hard safety split: never destructive without explicit acknowledgement.
- Prove parity with tests: every function tested offline in each shell; non-destructive commands exercised live at each milestone.
- `thebearcave` remains the operational media stack; it consumes Cave-Scripts via submodule and keeps its `.env`, python backends that need live data, and integration tests.

### Non-goals
- Not a transpiler or codegen project (D2).
- Not adopting the entire retired fish corpus blindly — the archive audit (M1) may classify functions as skip (retired, superseded, dead paths).
- Not moving `thebearcave`'s `.env`, compose, python *data* backends, or stack runtime config out of the repo — only the shell surface + DE assets.
- No destructive live operations during testing, ever, without explicit sign-off.

---

## 4. Researched facts (pre-research for the plan)

### 4.1 Host & filesystem
- Host: CachyOS (Arch-based), login via `ly`; **Hyprland is the only session** (sway removed 2026-09-05; `~/.config/sway/` inert remnants still on disk).
- Disk: `/dev/nvme0n1p2` is **btrfs**, ~950G; subvolumes mounted at `/home` (`@home`), `/var/cache`, `/var/log`, `/root`; `/boot` is vfat `nvme0n1p1`. **Snapper** is active (auto pre/post snapshots; see §7).
- Shells installed: bash 5.3.15 (daily interactive driver today), zsh 5.9.2 (Powerlevel10k instant prompt + CachyOS zsh config, no oh-my-zsh), fish 4.9.1 (installed; **no** `~/.config/fish/config.fish` — fish is currently a bare binary, its old library was retired). Under D19, **fish becomes the default interactive/login shell** by the end of M4.
- `gh` authed as **WhispersOfJ**; `thebearcave` origin = `https://github.com/whispersofj/thebearcave`.

### 4.2 Current shell surface (`thebearcave`, active)
- `services/bash-functions/` = the **live operational surface** (sourced from `~/.bashrc`). Layout: `bearcave-bash.sh` loader, `functions/` (18 files: `__helpers.sh`, `__metadata.sh`, 16 `stack-*.sh`), `completions/stack-completions.sh` (generated), `scripts/` (`gen-bash-completions.sh`, python-backed commands, installers incl. `install-nightly-reclaim-cron.sh`, `stack-tui`, `nzbdav-safe-recreate.sh`), `waybar/` (config, style.css, README, status scripts).
- Function count: **105 `stack-*` functions** (regex `^name()`) + `__*` helpers + `fmt_*` formatting helpers; API calls centralised via `__stack_curl` with per-type timeouts (LIGHT 10s / MUTATE 20s / HEAVY 30s, env-overridable); python data passed over **stdin** (E2BIG rule); guarded `docker` wrapper routes nzbdav recreates through `nzbdav-safe-recreate.sh`.
- Testing today: `tests/bash/test_bash_functions.sh` (offline + live tiers; `--offline` is CI-safe), `scripts/gen-bash-completions.sh --check`, `bash -n` gates, `stack-config-drift`, `stack-audit-residue` (which **scans the functions tree** for retired residue — must learn Cave-Scripts' tree), maintenance digest.

### 4.3 Retired / archived fish (context for the full audit)
- Fish library was **retired** in favour of bash (docs/stack/services/FISH.md): parallel fish doubled API/endpoint drift surface. Retirement is reversible from git history.
- `archive/media-stack/` still holds **117 `.fish` files** (62 functions + 55 completions). The M1 audit classifies each: re-adopt, superseded-by-bash, or dead.
- **`services/host-tools/` is a *live*, fish-only library** the audit must also cover: 25 `.fish` files — 23 `stack-*` host functions (pkg/journal/firewall/disk/mem/service/timer/cron/ssh/kernel/aur/flatpak/uptime/git/claude-home…) plus `__host_*` helpers — with `scripts/{install,uninstall}.sh` and `docs/stack/services/host-tools.md`. No bash/zsh ports exist. In scope for the M1 audit and the three-shell ports (D1, D19). Repo-wide `.fish` total: **142**.

### 4.4 Recent DE work (2026-09-05, Hyprland switchover) — inventory to turn into functions
- `~/.config/hypr/`: `hyprland.conf`, `hyprlock.conf`, `hypridle.conf`, `hyprpaper.conf`, `README.md`, `.current-theme`, **`themes/` with 7 themes** (`auto`, `catppuccin-mocha`, `dracula`, `gruvbox`, `nord`, `rose-pine`, `tokyonight`), **`scripts/`**: `build-themes.sh`, `render-theme.sh`, `hyprtheme`, `idle-toggle.sh`, `gammastep-toggle.sh`, `recorder-toggle.sh`, `power.sh`, `cliphist-pick.sh`.
- The `auto` theme is **not time-based**: it is a matugen/Material-You palette generated from an arbitrary image (`hyprtheme auto <image>` regenerates it) — D18 treats it as a real, explicitly-selectable theme (excluded from cycling).
- **hypr configs appear unversioned** (no git worktree at `~/.config`) — moving them into Cave-Scripts versions them for the first time.
- Waybar live scripts at `~/.config/waybar/scripts/`: `weather.sh`, `updates.sh`, `cava-bar.sh`, `gpu-usage.sh`, `stack-tui-toggle.sh`, `record-status.sh`, `nightlight-status.sh`. The repo's `services/bash-functions/waybar/` only mirrors config/style/README + record/nightlight/stack-tui — **weather/updates/cava/gpu-usage live only on the host today**.
- **Waybar config already migrated (#172, 2026-09-05):** the live→repo sync replaced the sway-era wiring — the tracked config now uses `hyprland/workspaces`/`hyprland/window`, adds cpu/memory/gpu/weather/cava modules, and toggle paths point at `~/.config/hypr/scripts/`; **zero `sway/` references remain**. The bar config's custom modules exec the 4 host-only live scripts (`gpu-usage.sh`, `weather.sh`, `cava-bar.sh`, `updates.sh` under `~/.config/waybar/scripts/`) that are still unversioned. M3 work: `cave-bar-check` guard against sway refs returning, adopt the host-only scripts, rehome under Cave-Scripts `de/`.
- Installed binaries (all present): hyprctl, hyprlock, hypridle, hyprpaper, waybar, wf-recorder, cliphist, gammastep, rofi, wofi, swaync, playerctl, pactl, powerprofilesctl, pavucontrol, bluetoothctl. Absent: mako, hyprswitch, swww (not needed).
- Toggle scripts are wf-recorder / gammastep / swayidle-era (idle) / theme-cycle / power-menu semantics — the plan maps each to a family in §6.

### 4.5 Wiring that references `services/bash-functions` (must be repointed at Cave-Scripts on cutover)
**23 files** across md/sh/yml/py reference the path (grep-verified on main 2026-09-05 — the earlier 85-count included `.worktrees/` checkouts of the same files); there will be **no compatibility shim** after cutover (D16), so the repoint list must be exhaustive and M1 re-derives it mechanically.
- `~/.bashrc` loader snippet (documented in `docs/stack/services/bash-functions.md`) → path changes to the Cave-Scripts checkout/submodule location.
- `tests/bash/test_bash_functions.sh` (offline tier + live tier split per D13), `gen-bash-completions.sh --check`, `bash -n services/bash-functions/functions/*.sh` gates.
- Docs: `docs/stack/services/bash-functions.md`, `docs/stack/services/FISH.md`, `AGENTS.md` (bash-functions references, waybar pointer, validation steps), `docs/stack/API.md` call-surface notes, waybar README sync flow.
- Cron/timers: `install-nightly-reclaim-cron.sh` (installs a crontab entry whose `bash -lc` sources the loader **by repo path**); user timers for arrival-notify / activity-feed / maintenance digest call python backends and stack-* functions by path.
- CI (`validate.yml` and friends) referencing the tree/tests; `stack-audit-residue`'s repo-surface scan of the functions tree; `stack-config-drift` (path-independent, safe).
- M1 must produce the **complete reference list** (grep-verified) before the cutover commit.

---

## 5. Architecture of Cave-Scripts

### 5.1 Repository layout (proposed)
```
Cave-Scripts/                    # public, WhispersOfJ/Cave-Scripts
├── README.md                    # what it is, install, parity guarantee
├── LICENSE                      # MIT, © 2026 WhispersOfJ (mirrors thebearcave — D15)
├── .env.template                # what the loaders expect (keys only, no secrets)
├── bash/                        # ── port 1: bash ──
│   ├── cave-scripts.sh          # loader (bearcave-bash.sh equivalent)
│   ├── functions/               # __helpers, __metadata, cave-*.sh (renamed per D17, alias layer ships alongside)
│   ├── completions/             # generated cave-* completions
│   └── scripts/                 # gen-completions, installers, python backends (moved)
├── zsh/                         # ── port 2: zsh (new) ──
│   ├── cave-scripts.zsh         # loader (zsh-native: arrays, compdef)
│   ├── functions/
│   ├── completions/             # _cave-* compdef files
│   └── scripts/
├── fish/                        # ── port 3: fish (new home; old fish was retired) ──
│   ├── cave-scripts.fish        # loader + conf.d hook
│   ├── functions/               # one .fish per function (native autoload) or grouped
│   ├── completions/
│   └── scripts/
├── de/                          # desktop assets + runtime scripts (moved source of truth)
│   ├── waybar/{config,style.css,scripts/}
│   ├── hypr/{hyprland.conf,hyprlock.conf,hypridle.conf,hyprpaper.conf,scripts/,themes/}
│   └── sync/                    # sync+reload commands (repo → live), de-stale checker
├── dotfiles/                    # unified shell dotfile source of truth (D10)
│   ├── .bashrc.inc / .zshrc.inc / config.fish
│   ├── starship.toml            # shared prompt for all three shells (D22)
│   └── install.sh               # idempotent per-shell installer
├── spec/                        # the shared function registry (canonical spec, D2)
│   └── functions.yaml           # name, args, flags, output contract, timeout class, safety class
└── tests/
    ├── bash/test_cave_scripts.sh
    ├── zsh/test_cave_scripts.zsh
    └── fish/test_cave_scripts.fish
```
> Locked (2026-09-05): submodule mounts at `services/cave-scripts` with **no**
> compat shim (D16); commands are **all `cave-*`** with a **permanent**
> `stack-*` alias layer (D17/D21); LICENSE is MIT mirroring thebearcave
> (D15); thebearcave re-pins per Cave-Scripts release tag (D25).

### 5.2 The canonical function registry (`spec/functions.yaml`)
Every function — existing, DE, btrfs — is registered once with: name, owning
shell file, arguments/flags, environment overrides, **output contract**
(exact line shape the tests assert), **timeout class**, and **safety class**
(`read-only` / `mutating-safe` / `destructive-confirm`). This registry is the
drift antidote to D2: each port is a 1:1 translation of the same row, and the
offline suites derive expected output from it. Any API/endpoint change edits
the registry once, then every port + test updates against it.

### 5.3 Parity contract (what "hand-maintained port" means)
Identical behaviour across bash/zsh/fish for each registered function:
- command name + flags + exit codes + stdout/stderr split + `fmt_*` colour output (respecting `STACK_COLOR` and non-TTY auto-off);
- `.env` loading (only set unset vars), API base-URL overrides (`*_URL`), timeout budgets + env overrides, guarded `docker` wrapper semantics (nzbdav-safe-recreate routing), python-over-stdin rule;
- per-shell syntax idioms where unavoidable (arrays, string ops, `local`, process substitution) — behaviour identical even where syntax differs; the loader is zsh/fish-native, not a bash re-source (fish cannot parse bash; zsh partial sourcing is fragile — rejected).
- dotfiles parity: `~/.bashrc` / `~/.zshrc` / fish `config.fish` all load the same library from one installer (D10) and share one **starship** prompt config (D22); Powerlevel10k is retired from the zsh dotfiles (the CachyOS zsh config source stays).

### 5.4 Path model after the move
- Cave-Scripts becomes path-independent of thebearcave: `CAVE_SCRIPTS_DIR` derived from the loader's own location; stack `.env` resolves to thebearcave checkout (default `$HOME/cave/.env`, overridable, e.g. `BEARCAVE_REPO_DIR` still honoured for python backends needing `scripts/`).
- Python-backed commands that reference `scripts/…` inside the old tree must switch to Cave-Scripts' own `scripts/` dir (they move with D3); only live-data python (polling APIs, DB gates) stays in thebearcave under tests/integration (D13).
- Submodule pinned at `services/cave-scripts` keeps a single working path for thebearcave's remaining consumers (docs, integration tests, residue audit).

---

## 6. Function families — target inventory

Naming convention (D17): **every function in all three shells is `cave-*`** —
`cave-arr-*`/`cave-plex-*`/`cave-nzbdav-*`… (media, renamed from `stack-*`),
`cave-session-*`, `cave-bar-*`, `cave-theme-*`, `cave-media-*`, `cave-power-*`
(desktop), `cave-btrfs-*` (fs). The full legacy `stack-*` name set ships as
**deprecated aliases** in all three shellsso muscle memory, cron lines and one-off commands keep working; docs/tests/completions use the `cave-*` names. Aliases are **permanent** (D21): documented deprecated shims, covered by the registry and the offline suites.

### 6.1 Existing media-stack surface (renamed `cave-*`, ported to zsh + fish)
All 105 functions (renamed per D17, e.g. `stack-arr-backlog` → `cave-arr-backlog`)
across core, arr (backlog/blocklist/missing/cutoff/import/logs/queue), disk,
lists, loop-ratings, maintenance, misc, arrivals, watchable, nzbdav,
plex-core/extra/markers/updates, queue — plus `__*_api` helpers and `fmt_*`.
The `stack-*` alias layer wraps every renamed command in all three shells.Ported
file-for-file from the M1 inventory; every behaviour change lands in
the registry first. The **live `services/host-tools` fish set** (23
`stack-*` host functions + `__host_*` helpers) is audited and ported the
same way — renamed `cave-*` (e.g. `cave-pkg-update`) with `stack-*` aliases
where not superseded by an existing function.

### 6.2 Desktop / Hyprland switchover (new — all three shells)
1. **Session control** (`hyprctl`-backed, read-only + dispatch): workspace list/switch/next/prev, active window info, monitor list + layout, `hyprctl version/info`, binds/errors status; mirrors hyprland.conf hotkey set.
2. **Lock & idle**: `cave-lock` (hyprlock), `cave-idle-toggle state|toggle` (hypridle process control — presentation mode; replaces swayidle-era `idle-toggle.sh`), idle config reload.
3. **Themes & nightlight**: theme list/get/set across the six palettes plus `auto`; `cave-theme cycle` rotates the **six palettes only**; `cave-theme auto-from-image <img>` regenerates the matugen `auto` theme and applies it; `cave-theme current` reads `.current-theme` (all D18). Converts `hyprtheme` + build/render-theme flows; `gammastep-toggle.sh state|toggle` → `cave-nightlight`; waybar style rebuild + reload after every theme change.
4. **Recorder, clipboard, media**: `wf-recorder` start/stop/status + elapsed (converts `recorder-toggle.sh` + `record-status.sh`), `cliphist` pick (converts `cliphist-pick.sh`), volume/mute + default-sink/source via pactl, media via playerctl.
5. **Power & bar**: power menu action helper (lock/logout/suspend/reboot/shutdown — each its own guarded function; converts `power.sh`), waybar restart/reload/toggle-module/signal, stack-tui launcher state/toggle/opacity (existing script → function), swaync control, systemd-failed-units status.
6. **De-stale guard + script adoption**: the sway→hyprland module/path migration already landed on main (#172). Remaining: `cave-bar-check` (fails on any `sway/` reference or stale `~/.config/sway/…` path returning), adopt the 4 host-only live bar scripts (`weather.sh`, `updates.sh`, `cava-bar.sh`, `gpu-usage.sh`) into `de/waybar/scripts/`, and point custom-module exec paths at Cave-Scripts homes.

### 6.3 Live waybar status scripts (convert to Cave-Scripts-managed assets)
`record-status.sh`, `nightlight-status.sh` (already in repo), `weather.sh`,
`updates.sh`, `cava-bar.sh`, `gpu-usage.sh`, `stack-tui-toggle.sh` (host-only
today → versioned). Executed by the bar; kept as small scripts *owned by*
Cave-Scripts (sync target `~/.config/waybar/scripts/`), with function
equivalents where interactive (record toggle/status, stack-tui).

### 6.4 btrfs (new — all three shells; host layout in §7)
Read-only/info (safe anytime, sudo where needed): `cave-btrfs-usage` (`fi
usage`/df), `cave-btrfs-subvolumes` (list, `@home`/`@root`/`@var_*` aware),
`cave-btrfs-device-stats`, `cave-btrfs-scrub-status`, `cave-btrfs-balance-status`,
`cave-btrfs-qgroup` (show). Snapshot & subvol mgmt (mutating-safe): snapshot
create/list/delete by explicit name, subvol create; snapper-backed listing/diff
for both configs — existing `root` (`@`) and the **new `home` config for
`@home`** (D20). Maintenance actions (**destructive-confirm**): scrub start,
balance start — both require `--yes` plus a confirmation step. Backups live
**outside btrfs** (D24): a `cave-backup` wrapper family (all three shells)
drives the existing `backup_dropbox.py` (repo snapshot → Dropbox) with
`run` / `--dry-run` / `--include-dbs` / retention-status surfaces; no btrfs
`send`/`receive` is in scope. Every mutating function prints exactly what it
will run before doing it.

### 6.5 Unified dotfiles (D10)
`dotfiles/` holds the three shell init files, the shared **starship** prompt
config (D22), and one installer. Powerlevel10k is retired from the zsh init
(the instant-prompt block and `~/.p10k.zsh` go away; the CachyOS zsh config
source stays), and a fish `config.fish` (currently absent) is added. Under
D19, **fish becomes the default interactive/login shell**: `config.fish` is
the primary daily shell config and `install.sh` performs the `chsh` switch as
an explicit, reversible final step (bash and zsh init files are still
installed and kept fully working). `install.sh` is idempotent,
worktree-aware, never touches `.env` or secrets.

---

## 7. btrfs host model (input to §6.4 and the plan)
```
/dev/nvme0n1p2 (btrfs, ~950G) — the ONLY block device on the host
├── @            → /            ← snapper config "root" exists (auto pre/post)
├── @home        → /home        ← NOT snapper-protected today → new "home" config
│                                 (D20) with auto timeline snapshots: hourly 5 /
│                                 daily 7 / weekly 4 / monthly 3 (D23)
├── @root        → /root
├── @srv / @cache / @log / @tmp → /srv, /var/cache, /var/log, /var/tmp
/boot = vfat nvme0n1p1 (not btrfs — exclude from all btrfs functions)
snapper configs: "root" (@) exists (number-based 50/15, timeline off); "home"
(@home) timeline-based per D23
Off-site backup = Dropbox via backup_dropbox.py (repo tar, keep 30) — not a
btrfs send/receive target (D24); the btrfs surface is local-only.
```
Rules baked into the functions: never operate on `/boot`; every action scopes
to a subvolume path (default `@home`); `sudo` used only where btrfs requires
root, with the exact command echoed; destructive classes require
confirmation (see §6.4); snapper snapshot *lists* integrate with btrfs lists
to avoid double-reporting across both configs (`root` and `home`).

---

## 8. Testing strategy (D11, D13)

### 8.1 Offline suites — merge gate, lives in Cave-Scripts (`tests/<shell>/`)
Each shell gets a mirror of today's `tests/bash/test_bash_functions.sh`
offline tier: loader/define checks, helper checks, syntax gates (`bash -n` /
`zsh -n` / `fish -n`), completion drift, guarded-command usage output on
closed stdin, registry-conformance assertions (every function in
`spec/functions.yaml` exists in all three ports and vice versa), and mocked
API tests (wanted/missing/movie fixtures etc. — bash's renderer unit tests
ported per shell). CI in Cave-Scripts runs all three.

### 8.2 Live matrix — read-only, runs at milestone demos, lives in `thebearcave`
An explicit **safe-command registry**: only status/health/read-only functions
(mapped to the real API surfaces: GETs, `docker ps`, queue/health probes,
btrfs info, hyprctl info) execute against the live stack — once per shell.
Every registered mutating function is listed with its exclusion reason.
Reuses `thebearcave` integration scaffolding (`.env`, running services) that
must not exist in the public Cave-Scripts repo.

---

## 9. Config assets & sync design (D14)
- **Source of truth moves to Cave-Scripts `de/`**: waybar config/style/scripts
  (from repo `services/bash-functions/waybar/`) **and** the hypr configs +
  7 themes + scripts (first-ever versioning of `~/.config/hypr/*`).
- `cave-sync waybar|hypr|all` copies repo → live paths and reloads the right
  thing (waybar restart, `hyprctl reload`, theme re-render); `--check`
  reports drift without writing. Mirrors today's manual `cp` + `pkill -x
  waybar` flow documented in the waybar README.
- The live `~/.config/waybar/scripts/*` extras (weather/updates/cava/gpu) get
  adopted into `de/waybar/scripts/` so the live dir becomes a pure sync
  target, never an edit target.

---

## 10. Repo, remote, and delivery mechanics
- **Create** public `WhispersOfJ/Cave-Scripts` via `gh repo create` (no
  secrets; `.gitignore` for `.env`, local state; first commit carries the
  README and an **MIT LICENSE, © 2026 WhispersOfJ** mirroring thebearcave — D15).
- **Submodule**: `git submodule add https://github.com/WhispersOfJ/Cave-Scripts
  services/cave-scripts` in `thebearcave` (locked path, D16 — no compat
  shim). Cave-Scripts runs its own release-please tagging (D25); thebearcave
  pins the latest Cave-Scripts release tag and bumps via PR when a new tag
  ships — never direct commits.
- **Cutover ordering** (guards a broken operational surface): Cave-Scripts
  populated and demoed from its own checkout → submodule added → all **23**
  `thebearcave` references updated in one commit (loader snippet, tests split,
  docs, CI, cron/timer installers, residue audit, `cave-*` naming) →
  `services/bash-functions/` removed (no shim — D16) → shell dotfiles
  installer runs incl. the `chsh` switch to fish (D19) → daily-reclaim cron
  re-run with the new path → post-cutover live smoke.
- Worktree discipline applies in both repos; main checkout stays clean;
  delivery via PRs.

---

## 11. Milestone plan (shape for the upcoming approval plan — D12)
Four big checkpoints, each ending in a demo before the next begins:

- **M1 — Repo, inventory, and asset move.** Create public Cave-Scripts (with
  release-please tagging per D25) + submodule; full audit of every
  function/script (bash live 105, archived fish 117, live host-tools fish 25,
  DE host scripts, host-only waybar scripts, dotfiles) → registry draft
  + adopt/convert/skip ledger; move assets (waybar, hypr, themes) + sync
  commands; cutover reference list compiled. Demo: sync round-trip + inventory
  walkthrough.
  *Progress (2026-09-05): repo created, assets moved to `de/`, registry
  populated (153 rows), inventory ledger + repoint list on main (PR #176);
  submodule pinned (PRs #177/#178 merged), **Demo 1 passed on both sync
  targets** — cave-sync round-trips verified, one defect found and fixed
  (waybar restart, released v0.2.1); M2 open — see plan §M1 status.*
- **M2 — All three shells.** Bash port lands in the new layout under its
  `cave-*` names with the `stack-*` alias layer (D17); zsh and fish ports 1:1
  from the registry; unified dotfiles + installers; offline suites for all
  three shells green; completions parity. Demo: same command run in three
  shells, byte-identical output.
- **M3 — DE + btrfs families.** §6.2–6.4 functions in all three shells;
  host-only bar scripts adopted + sway de-stale guard live; btrfs guarded
  families live; read-only live matrix run against the real stack (per shell)
  at the demo.
- **M4 — Hardening, cutover, docs.** Reference repointing commit in
  `thebearcave` (all 23 files), `services/bash-functions/` removal (no shim),
  dotfiles installer + `chsh` to fish (D19), cron/timer repoint,
  residue-audit update, docs (bash-functions.md/FISH.md/AGENTS.md/API.md),
  Cave-Scripts CI, final end-to-end live smoke; cleanup of worktrees.

## 12. Decision log

All previously-open items resolved 2026-09-05 and recorded in §2 as
D15–D25: license MIT (D15); submodule `services/cave-scripts`, no shim, 23
refs repointed (D16); full `cave-*` rename (D17); `auto` = matugen theme,
excluded from cycling (D18); fish becomes the default shell at end of M4
(D19); snapper `home` config for `@home` (D20); **`stack-*` aliases
permanent** (D21); **starship prompt for all three shells**, p10k retired
(D22); **snapper home = auto timeline** hourly 5 / daily 7 / weekly 4 /
monthly 3 (D23); **Dropbox is the backup target** via `backup_dropbox.py`
with a `cave-backup` wrapper family, btrfs send/receive out of scope (D24);
**thebearcave bumps the submodule per Cave-Scripts release tag** (D25).

Remaining minor notes for the milestone drafts (not blocking): starship needs
installing at M2 (Arch: `pacman -S starship`); the p10k retirement removes
the `~/.p10k.zsh` file and the instant-prompt block from `~/.zshrc` at M2
after the starship look is demoed; snapper `home` limits may be tuned at the
M3 demo after observing space use on @home.

## 13. Risks & mitigations
| Risk | Mitigation |
|---|---|
| Three-way drift (the original reason fish was retired) | Canonical registry `spec/functions.yaml` + conformance tests in all three shells + single-edit workflow for API/endpoint changes |
| Broken operational surface during cutover | Mirror-then-move ordering (M4), loader shim until the cutover commit, live smoke after each step |
| Live-stack tests touching production state | Read-only safe-command registry; destructive functions excluded and unit-tested offline only |
| btrfs commands run with sudo on the wrong subvol | Subvol-scoped helpers, dry-run default, echo-before-exec, explicit confirm on destructive class |
| Secret leakage into a public repo | `.env` never moves; keys stay in thebearcave `.env`; Cave-Scripts CI + pre-commit secret scans |
| Submodule friction (stale pins, split brains) | SHA/tag-pinned bumps via PRs; docs update in thebearcave; single owner discipline |
| Host-only waybar scripts lost in the move | M1 inventory audits `~/.config/waybar/scripts` explicitly before cutover |
| Renaming 105 functions to `cave-*` breaks muscle memory, cron, docs mid-flight | `stack-*` alias layer in all three shells ships with M2; aliases covered by the offline suites; docs/tests move to `cave-*` names in the same commit |
| Making fish the default shell disrupts the daily session | `chsh` is the explicit last M4 step, after fish dotfiles + functions are proven in daily use; bash/zsh stay installed and switchable via `chsh -s` |
| Snapper `home` timeline snapshots a huge, hot `@home` | D23 timeline limits (hourly 5 / daily 7 / weekly 4 / monthly 3) start conservative; free space checked before enabling; limits tunable after the M3 demo |
| Retiring p10k while zsh is today's daily prompt | starship lands in zsh first during M2 and is demoed before the p10k block is removed; `~/.p10k.zsh` kept until the M2 demo confirms the starship look |

## Appendix A — counts at spec time
- bash: 105 `stack-*` functions, 18 `functions/` files, generated completions, `scripts/` incl. python-backed commands + installers; waybar dir with 2 status scripts + toggle.
- fish (retired, in archive): 117 `.fish` files → audit in M1.
- fish (live): 25 `.fish` host functions under `services/host-tools/functions` (23 `stack-*` + 2 `__host_*`) + `scripts/{install,uninstall}.sh` → audit + port in M1/M2. Repo-wide `.fish` total: 142.
- zsh/fish ports: 0 today (new work).
- DE: 8 hypr scripts, 7 themes, 7 live waybar scripts (3 mirrored in repo, 4 host-only: weather/updates/cava-bar/gpu-usage — wired into the bar config but unversioned). Sway-era waybar wiring already migrated on main (#172): zero `sway/` references remain in the tracked config.
- Wiring to repoint on cutover: `~/.bashrc` snippet, CI workflows, `tests/bash/`, docs (`bash-functions.md`, `FISH.md`, `AGENTS.md`, `API.md`), cron/timer installers, residue audit, waybar README.
