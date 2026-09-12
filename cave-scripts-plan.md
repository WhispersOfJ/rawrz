# Cave-Scripts — Multi-part Implementation Plan (for approval)

Status: **Draft for approval** · Date: 2026-09-05 · Owner: bear
Sibling doc: [`cave-scripts-spec.md`](cave-scripts-spec.md) (the "what"); this
file is the "how/when". Every decision D1–D25 in the spec is honoured here.

---

## 0. What you are approving

Approving this plan means: proceed **one milestone at a time**, in order, with
the **demo at the end of each milestone as the go/no-go gate** for the next
(D12 — few big milestones, not per-phase micro-approvals). You can halt or
redirect after any demo; nothing in M2–M4 starts before its predecessor's
demo passes.

| Gate | Happens after | Stop → next step |
|---|---|---|
| Demo 1 (end of M1) | Repo + submodule + inventory + assets | approve → M2 |
| Demo 2 (end of M2) | All three shells + dotfiles/starship | approve → M3 |
| Demo 3 (end of M3) | DE + btrfs + live matrix | approve → M4 |
| Demo 4 (end of M4) | Cutover + `chsh` + final smoke | done / post-M4 follow-ons |

System-level side effects that need your explicit go at the moment they run
(not just plan approval): creating the public GitHub repo (M1), enabling the
snapper `home` timeline (M3, sudo), and switching the login shell to fish
(M4, `chsh`). Each is reversible and called out inline.

---

## 1. Execution rules (how the plan is run)

1. **Worktree discipline** in both repos: one task-named worktree per task,
   branched off `origin/main`, delivered via PR, main checkout stays clean.
   Thebearcave: worktrees under `/home/bear/cave/.worktrees/`. Cave-Scripts:
   same convention under `~/.worktrees/cave-scripts/` once cloned.
2. **Every change ships with its test** (CLAUDE.md: test before ship); the
   per-shell offline suites are the merge gate for every Cave-Scripts PR from
   M2 on.
3. **Registry first:** any behaviour/API/endpoint change edits
   `spec/functions.yaml`, then all three ports + tests update against it. No
   function exists in a port without a registry row.
4. **Secrets:** never commit `.env` or keys; Cave-Scripts `.env.template`
   holds names only; public repo CI scans for secrets; thebearcave `.env`
   stays put.
5. **Live stack safety:** during development only the **safe-command
   registry** (§4.2) may touch the live stack; destructive operations run
   only inside a milestone demo with you watching.
6. **Output + reporting:** agent-facing operational output in English; each
   milestone closes with CLAUDE.md's completion protocol (DONE /
   DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT).
7. **Plan changes** are folded back into the spec's decision log (§12) so the
   two documents never diverge.

---

## 2. Sequencing overview

```
M1  Repo · inventory · assets          ── demo 1 ─▶ ┐
M2  Three shells · dotfiles · starship ── demo 2 ─▶ ├─ each demo is a go/no-go
M3  DE + btrfs families · live matrix  ── demo 3 ─▶ │
M4  Cutover · chsh · hardening         ── demo 4 ─▶ ┘
```
Dependencies: M1 produces the registry draft + Cave-Scripts skeleton every
port lives in. M2 needs the M1 inventory (function list is the porting
contract) and produces the `cave-*` naming + alias layer. M3 builds on the
three-shell machinery from M2. M4 is the only milestone that touches the live
operational wiring (`~/.bashrc`, cron, timers, old tree removal).

---

## 3. Milestones

### M1 — Repo, inventory, and asset move

**Purpose:** stand up `WhispersOfJ/Cave-Scripts` (public, MIT, release-please
per D25), pin it as the `services/cave-scripts` submodule of thebearcave
(D16), and produce the exhaustive inventory that drives every later
milestone — including moving the DE assets into version control for the first
time (D14).

**Tasks**
1. **Create the remote repo** (needs your go): `gh repo create
   WhispersOfJ/Cave-Scripts --public`; first commit = `README.md`, `LICENSE`
   (MIT, © 2026 WhispersOfJ — D15), `.gitignore` (`.env`, `.env.*`, `*.local`,
   local state), `.env.template` (keys only: `RADARR_API_KEY`,
   `SONARR_API_KEY`, `PROWLARR_API_KEY`, `PLEX_TOKEN`, `SEERR_API_KEY`,
   `NZBDAV_*`, `*_URL` overrides, timeout overrides, `STACK_COLOR`,
   `DROPBOX_*`), and a `spec/` scaffold with `functions.yaml` header.
2. **Release-please setup** (D25): mirror thebearcave's release-please config
   + workflow (Conventional Commits, `feat:`/`fix:` trigger tags).
3. **Submodule pin** (D16): `git submodule add
   https://github.com/WhispersOfJ/Cave-Scripts services/cave-scripts` in a
   thebearcave worktree, landed via PR. No symlink shim anywhere.
4. **Full inventory audit** — the complete "every function in the stack"
   ledger (D1), each row classified **adopt / convert / skip-with-reason**:
   - bash: 105 `stack-*` functions across 16 `stack-*.sh` + `__helpers.sh` +
     `__metadata.sh` (18 files under `services/bash-functions/functions/`),
     plus `fmt_*`, loader (`bearcave-bash.sh`), generated completions,
     `scripts/` (gen-completions, installers incl.
     `install-nightly-reclaim-cron.sh`, python-backed commands,
     `nzbdav-safe-recreate.sh`, `stack-tui`), and the `waybar/` dir;
   - archive fish: 117 `.fish` files under `archive/media-stack/` → classify
     re-adopt / superseded-by-bash / dead (expected: mostly superseded);
   - live host-tools fish: 25 `.fish` under `services/host-tools/functions`
     (23 `stack-*` host functions + `__host_*` helpers) +
     `scripts/{install,uninstall}.sh` → classify + port;
   - DE host surface: 8 scripts under `~/.config/hypr/scripts/`, 7 themes
     under `~/.config/hypr/themes/`, 7 live waybar scripts under
     `~/.config/waybar/scripts/` (3 already mirrored in-repo, 4 host-only:
     weather/updates/cava-bar/gpu-usage);
   - dotfiles: `~/.bashrc` (loader line), `~/.zshrc` (p10k instant prompt +
     CachyOS config), no fish config today.
   - every remaining repo shell script (`scripts/*.sh`, `services/*/scripts/`,
     `.github/workflows` run-blocks are out of scope unless function-relevant).
   Ledger = the seed for `spec/functions.yaml` (name, args, flags, output
   contract, timeout class, safety class).
5. **Compile the repoint reference list**: all **23 files** (grep-verified)
   referencing
   `services/bash-functions` (md/sh/yml/py) captured by path + reason —
   categories: loader docs, CI (`validate.yml` + friends), `tests/bash/`,   docs (`docs/stack/services/bash-functions.md`, `FISH.md`, `AGENTS.md`,
    `docs/stack/API.md`, waybar README), cron installer
   (`install-nightly-reclaim-cron.sh` crontab text), user timers, residue
   audit (`stack-audit-residue` functions-tree scan). This list is M4's
   cutover checklist, frozen in this milestone.
6. **Asset move (D14)**: copy `services/bash-functions/waybar/` and the hypr
   tree (`hyprland.conf`, `hyprlock.conf`, `hypridle.conf`,
   `hyprpaper.conf`, `scripts/`, `themes/` incl. `auto`) into
   `Cave-Scripts/de/` as the versioned source of truth (first versioning of
   `~/.config/hypr/*`); adopt the 4 host-only waybar scripts into
   `de/waybar/scripts/`. Live `~/.config/*` dirs become sync targets only.
7. **`cave-sync` skeleton**: `cave-sync waybar|hypr|all` (repo → live +
   reload), `--check` drift mode.

**Deliverables:** public repo + submodule pin PR; `inventory.md` ledger with
adopt/convert/skip rows; frozen 23-file repoint list; `de/` tree with sync
working; registry seed.

**Demo 1:** sync round-trip (edit repo asset → `cave-sync` → live bar/session
reflects it) + walkthrough of the inventory ledger and repoint list.

**Exit gate:** you approve the ledger + repoint list → M2 may start.

#### M1 execution status (2026-09-05)

Progress recorded as of 2026-09-05; task numbers refer to the list above.

**Done**
- **Task 1 — remote repo created:** `WhispersOfJ/Cave-Scripts` is public with
  MIT LICENSE © 2026 WhispersOfJ, README, `.gitignore`, `.env.template`, and
  the `spec/` scaffold (commit `4ee193e`).
- **Task 2 — release-please:** config + workflow mirrored; commits tagged
  `feat: populate spec/functions.yaml registry…` (`f35a1f4`) already flow
  through it; `RELEASE_PLEASE_TOKEN` secret set on the repo.
- **Task 4 — inventory audit:** landed as `cave-scripts-inventory.md` on
  thebearcave main via PR #176 (ledger: 105 bash adopt, 25 live host-tools
  fish adopt → `cave-sys-*`, archive fish 62 reconciled 23+22+17, DE surface
  catalogued; decisions L1–L4 recorded). The earlier "117 archive fish" and
  "105 bash" counts in this plan are superseded by that ledger's exact
  reconciliation (62 files = 23 dup-of-host-tools + 22 legacy-of-bash + 17
  unique).
- **Task 5 — repoint reference list:** the 23-file list is captured in the
  inventory ledger (spec/plan §4.5/§10 figures corrected by PR #175).
- **Task 6 — asset move:** DE assets are versioned in Cave-Scripts under
  `de/` for the first time — `de/hypr/` (hyprland/hyprlock/hypridle/hyprpaper
  confs, 8 scripts, 7 themes incl. `auto`), `de/waybar/` (config, style.css,
  README, 7 scripts incl. the 4 host-only weather/updates/cava-bar/gpu-usage),
  `de/sync/` (commit `c197939`).
- **Task 7 — `cave-sync` skeleton:** `de/sync/cave-sync.sh` shipped with the
  asset move (waybar|hypr|all, `--check`), shellcheck-clean.
- **Registry seed → populated:** `spec/functions.yaml` now carries all **153
  commands** (105 media `cave-*`, 23 host `cave-sys-*`, 13 de, 1 backup, 11
  btrfs) with family / safety / timeout / args / output-contract fields, plus
  the helper inventory. Verified 1:1 against live sources: 105/105 bash
  `stack-*` defs and 23/23 host-tools map to registry rows with no orphans
  (commit `f35a1f4`).

**In flight → resolved**
- **Task 3 — submodule pin:** PR #178 merged (`cc0c4f6`); L3 chore PR #177
  merged (`1cc337d`); nzbdav-watchdog PR #171 merged (`0ca1aa2`).

#### Demo 1 outcome (2026-09-05)

**Demo 1 ran and passed on both sync targets.**

- **Waybar round-trip:** repo edit → `cave-sync waybar` → live bar reflects
  it → revert → re-sync → drift green. All 9 waybar assets verified.
- **Hypr round-trip:** same cycle on `cave-sync hypr` — marker comment in
  `hypridle.conf` propagated live, all 7 theme trees intact, `hyprctl
  reload` sent, session healthy. All 19 hypr assets verified.
- **Final state:** `cave-sync --check all` reports all 28 assets in sync;
  submodule working tree clean.

**Demo 1 found and fixed a defect:** the skeleton's waybar restart
(`pkill -x waybar; nohup waybar &`) is reaped when the invoking shell
exits — the bar died after a sync. Fixed in Cave-Scripts `7c02c92`
(restart via `hyprctl dispatch exec`, the same path `exec-once` uses,
with a `setsid` fallback), released as **v0.2.1** (`1dc59c0`), and pinned
in thebearcave via PR #182 (`b8a104d`).

**M1 exit gate:** inventory ledger + repoint list approved (PR #176,
merged `d17aeb2`) → **M2 may start.**

---

### M2 — All three shells

**Purpose:** the media-stack library lives in Cave-Scripts under `cave-*`
names with a permanent `stack-*` alias layer (D17/D21), ported 1:1 to zsh and
fish (D2), with unified dotfiles + a shared starship prompt (D10/D22), all
gated by per-shell offline suites.

**Tasks**
1. **Bash port lands.** Move (git-history-preserving copy then delete in M4)
   `services/bash-functions/` content into `Cave-Scripts/bash/`
   (`cave-scripts.sh` loader, `functions/cave-*.sh`, `completions/`,
   `scripts/`), renaming 105 functions `stack-*` → `cave-*` per the registry
   (e.g. `stack-arr-backlog` → `cave-arr-backlog`). Ship the **alias layer**
   (`_aliases.sh`: every `stack-*` name forwards to its `cave-*` function) —
   permanent per D21, asserted by tests. Behaviour is unchanged: `.env`
   loader, `fmt_*`, guarded docker wrapper, `__stack_curl` timeout budgets
   (LIGHT/MUTATE/HEAVY + env overrides), python-over-stdin rule, exit codes.
2. **Path model** (§5.4 of spec): `CAVE_SCRIPTS_DIR` derived from the loader;
   stack `.env` resolves via thebearcave checkout (default `$HOME/cave/.env`,
   overridable); python backends that moved keep working from Cave-Scripts'
   own `scripts/`.
3. **zsh port.** `zsh/cave-scripts.zsh` loader (zsh-native arrays/compdef),
   `functions/` translated 1:1 from the registry, `completions/_cave-*`.
4. **fish port.** `fish/cave-scripts.fish` loader + `conf.d` hook,
   `functions/` (one file per function, native autoload), `completions/`.
5. **Completions + generators:** per-shell generator scripts + `--check`
   drift gates (bash compgen / zsh compdef / fish `complete`).
6. **Offline suites** (D11/D13) in `Cave-Scripts/tests/<shell>/` mirroring
   today's tiers: loader/define, helpers, syntax gates (`bash -n`/`zsh -n`/
   `fish -n`), completion drift, guarded-usage-on-closed-stdin, registry
   conformance (every `functions.yaml` row exists in all three ports;
   alias→cave-* mapping complete), and the mocked renderer/unit tests from
   `tests/bash/` ported per shell. Cave-Scripts CI runs all three.
7. **Dotfiles + starship (D22):** `dotfiles/` with `.bashrc.inc`,
   `.zshrc.inc`, `config.fish`, shared `starship.toml`, idempotent
   `install.sh`. Install starship (`pacman -S starship`); land starship in
   zsh first, demo the look, then remove the p10k instant-prompt block and
   `~/.p10k.zsh` (CachyOS zsh config source stays). fish `config.fish`
   created (none exists today).
8. **Alias/completion doc pass** inside Cave-Scripts README (parity
   guarantee, install, naming).

**Deliverables:** three shell ports + alias layer + loaders + completions +
dotfiles/starship installer; three green offline suites; CI wiring.

**Demo 2:** the same read-only command run in bash, zsh, and fish with
**byte-identical output**; tab-completion demo per shell; starship prompt in
all three; alias call (`stack-arr-backlog` ≡ `cave-arr-backlog`).

**Exit gate:** parity demo passes → M3.

---

### M3 — DE + btrfs families + live matrix

**Purpose:** add the desktop (Hyprland switchover) and btrfs function
families in all three shells (§6.2–6.4 of spec), adopt the live bar scripts,
add the sway de-stale guard, and run the read-only live matrix against the
real stack.

**Tasks**
1. **DE function families** (all three shells, D7, §6.2 of spec):
   - *Session* (`cave-session-*`): hyprctl wrappers — workspace list/switch/
     next/prev, active window, monitors, version/info, binds/errors.
   - *Lock & idle* (`cave-lock`, `cave-idle-toggle state|toggle|reload`):
     hyprlock + hypridle presentation-mode control (converts
     `~/.config/hypr/scripts/idle-toggle.sh`; the merged waybar config no
     longer references sway paths (#172) — repoint any remaining custom-module
     exec at Cave-Scripts homes).
   - *Themes & nightlight* (`cave-theme list|current|set|cycle`,
     `cave-theme auto-from-image <img>`, `cave-nightlight state|toggle`):
     converts `hyprtheme`, `build-themes.sh`, `render-theme.sh`,
     `gammastep-toggle.sh`; cycle = six palettes only, `auto` selectable
     explicitly (D18); waybar style rebuild + reload on every change.
   - *Recorder, clipboard, media* (`cave-record start|stop|status`,
     `cave-cliphist pick`, `cave-audio *`, `cave-media *`): wf-recorder with
     elapsed status (converts `recorder-toggle.sh` + `record-status.sh`),
     cliphist pick, pactl volume/mute, playerctl transport.
   - *Power & bar* (`cave-power lock|logout|suspend|reboot|shutdown`,
     `cave-bar restart|reload|toggle-module|signal`, `cave-stack-tui …`,
     `cave-notify`): converts `power.sh` and `stack-tui-toggle.sh`; swaync
     control; systemd-failed-units status.
2. **Bar + de-stale guard** (D8): convert/adopt the live status scripts
   (record/nightlight already mirrored; adopt weather, updates, cava-bar,
   gpu-usage into `de/waybar/scripts/` — the merged config already execs
   them from `~/.config/waybar/scripts/`); the sway→hyprland module/path
   migration is done (#172), so the remaining work is `cave-bar-check`
   (fails if any `sway/` reference or `~/.config/sway/…` path returns).
3. **btrfs families** (§6.4, §7 of spec, all three shells):
   - *Info/read-only*: `cave-btrfs-usage`, `-subvolumes`, `-device-stats`,
     `-scrub-status`, `-balance-status`, `-qgroup`.
   - *Snapshots/subvols (mutating-safe)*: snapshot create/list/delete, subvol
     create, snapper listing/diff across both configs.
   - *Maintenance (destructive-confirm)*: `scrub start`, `balance start`
     (`--yes` + confirmation + echo-before-exec; never touches `/boot`).
   - *Backup (D24)*: `cave-backup run|dry-run|include-dbs|config` wrapper
     family execs `$BEARCAVE_REPO_DIR/scripts/backup_dropbox.py`; **no btrfs
     send/receive anywhere**.
4. **Snapper `home` config (D20/D23)** — system change, needs your go:
   `sudo snapper -c home create-config /home`; set timeline limits hourly 5 /
   daily 7 / weekly 4 / monthly 3; check free space first (~628G free on the
   fs); enable `snapper-timeline.timer`; leave `root` config untouched.
   Manual `cave-btrfs` snapshots keep `@home` covered even between timeline
   ticks.
5. **Live read-only matrix** (D11): build the **safe-command registry** in
   thebearcave (`tests/live/`): every function allowed to run live, once per
   shell — status/health/GET surfaces (arr system/status, queue, history,
   plex sessions, nzbdav health+queue+PROPFIND, docker ps, btrfs info,
   hyprctl info, disk/usage). Mutating functions are listed with exclusion
   reasons. Runs at this demo and Demo 4.

**Deliverables:** DE + btrfs + backup families in three shells;waybar host-only scripts adopted + sway de-stale guard live; snapper `home` timeline live (after your go);
live-matrix harness + safe-command registry.

**Demo 3:** theme switch + record toggle + idle toggle through the new
functions (live, non-destructive); `cave-btrfs-*` info runs; snapper `home`
timeline first snapshot verified; live matrix run per shell; full
`cave-bar-check` clean.

**Exit gate:** demo passes → M4.

---

### M4 — Hardening, cutover, docs

**Purpose:** the one milestone that changes live wiring — repoint all 23
references, remove the old tree (no shim, D16), switch the login shell to
fish (D19), finish docs/CI, and run the final end-to-end smoke.

**Tasks**
1. **Cutover commit in thebearcave** (one PR, from the frozen M1 list):
   repoint loader docs + `~/.bashrc` install snippet to the submodule path;
   move offline test tiers to Cave-Scripts (keep live tier + integration in
   thebearcave — D13); update CI (`validate.yml` etc.), `AGENTS.md`,    `docs/stack/services/bash-functions.md` (retirement record in the FISH.md
    style), `docs/stack/API.md`, waybar docs, `install-nightly-reclaim-cron.sh`
   crontab text + user timers, residue audit (`stack-audit-residue` learns
   the Cave-Scripts tree), stack-config-drift unaffected (path-independent).
2. **Remove `services/bash-functions/`** from thebearcave (no symlink shim).
   Submodule at `services/cave-scripts` is the single home; document the
   `stack-*`/`cave-*` relationship in the README + AGENTS.md.
3. **Dotfiles install + `chsh` to fish (D19)** — needs your go, and only
   after fish has been the daily shell for the M3→M4 gap: run `install.sh`,
   then `chsh -s $(which fish)`. Reversible: `chsh -s $(which bash)` (or
   zsh). bash/zsh sessions keep full function access.
4. **Cave-Scripts CI hardening:** shellcheck, ruff (python backends),
   actionlint-style checks on workflows, secret scan, release-please on tag
   (D25). thebearcave bumps the submodule pin per Cave-Scripts tag (D25) —
   first bump lands with this cutover.
5. **Docs:** Cave-Scripts README (install, parity, naming, safety classes);
   thebearcave service docs updated to the new home; FISH.md-style retirement
   note for the removed bash-functions tree.
6. **Final end-to-end smoke:** functions load from the submodule in all three
   shells from a **fresh shell** (no stale sourcing); nightly reclaim cron
   re-run with new path; maintenance digest green; live matrix re-run;
   `cave-sync --check` clean; btrfs info + snapper `home` healthy.

**Deliverables:** cutover PR; old tree gone; fish default (after your go);
both repos documented and CI-green; submodule pinned at a Cave-Scripts tag.

**Demo 4:** fresh-shell function load in bash/zsh/fish from the submodule;
digest + cron + live matrix green; alias + completions spot checks.

**Exit gate / completion:** DONE with evidence; post-M4 follow-ons (if any)
land as their own tasks.

---

## 4. Testing & quality gates (applies from M2)

### 4.1 Offline suites (merge gate, in Cave-Scripts)
Per shell: syntax gates, loader/define assertions, helper behaviour, mocked
API/renderer unit tests (ported from `tests/bash/test_bash_functions.sh`),
completion drift (`gen-*-completions --check`), registry conformance
(3-shell parity + alias completeness). Cave-Scripts CI runs all three on every
PR.

### 4.2 Safe-command registry (live, in thebearcave)
Only these classes may touch the live stack during development and demos:
status/health/queue/history/session **GET** surfaces, `docker ps`,
nzbdav health + authenticated queue + PROPFIND, btrfs info/status, hyprctl
info, disk/usage, backup `--dry-run`. Everything else is exercised offline or
inside a demo with you present.

### 4.3 Parity harness
Read-only fixture commands run in all three shells; captured stdout diffed
(ignoring timestamps/PIDs). Demo 2 proves parity; the harness stays as a
Cave-Scripts test tool.

---

## 5. Key risks (see spec §13 for full register)

| Risk | When | Mitigation in this plan |
|---|---|---|
| Triple-drift reintroduces the reason fish was retired | M2+ | registry-first workflow + conformance suites (M2 task 6) |
| Live stack touched during dev | M2/M3 | safe-command registry (§4.2); destructive only in demos |
| Rename breaks muscle memory/cron/docs | M2→M4 | permanent alias layer (D21) asserted by tests; repoint list frozen in M1 |
| Snapper `home` balloons space on hot @home | M3 | timeline limits (D23); free-space check before enable; tune after demo |
| fish-default switch disrupts daily work | M4 | `chsh` last, after daily-use proof; reversible one-liner |
| Public repo leaks secrets | M1+ | `.env.template` names only; CI secret scan; `.env` never moves |

**Abort/rollback:** every milestone ends at a demo; stopping after any demo
leaves the live stack untouched (M4 is the only wiring-changing milestone and
its cutover commit is a single revertable PR). Rollback of `chsh` =
`chsh -s $(which bash)`; rollback of snapper `home` = delete config +
disable timer; submodule removal = delete the gitlink line in a PR.

---

## 6. Sizing summary (surface, for expectation-setting)

| Milestone | Main artifacts | Rough scale |
|---|---|---|
| M1 | repo + submodule + inventory + assets | 117 archived + 25 live host-tools fish + 105 bash fns audited; 23 refs listed; 2 asset trees moved |
| M2 | 3 ports + alias layer + dotfiles + tests | ~105 functions × (rename + 2 new ports) + ~6 suites |
| M3 | DE + btrfs + backup families | ~40–60 new functions × 3 shells; host-only scripts + de-stale guard; snapper config |
| M4 | cutover + docs + CI | 1 big PR; 23 refs; 2 repos' docs/CI |

Timing is deliberately not promised in days; each milestone is demo-sized and
the effort scales with review quality, not calendar.

---

## 7. After you approve

1. Approve → I begin **M1 task 1** (`gh repo create` + first commit) and
   report at the first checkpoint (needs your go for the remote).
2. M1 tasks 2–7 proceed in worktrees; the ledger and repoint list are
   presented at Demo 1 for approval before M2.
3. M2–M4 proceed on the demo-go/no-go cadence in §0; system changes (snapper,
   `chsh`) get their own inline go-request at the moment they run.
4. Any plan change mid-flight is reflected in the spec decision log first.
