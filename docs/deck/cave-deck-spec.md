# Cave Deck — Specification

**Status:** Draft v1 (pre-implementation, interview-complete)
**Date:** 2026-09-06
**Supersedes:** the interactive operational surface of `cave-scripts-spec.md` (Cave-Scripts submodule), the `stack-*`/`cave-*` bash functions, the fish host tools, the waybar integration, and `stack-tui`.

> **RAWRZ supersession (M0, 2026-09-11).** This is the Deck **component** spec. The RAWRZ
> master spec ([`rawrz-megastack-spec.md`](../../rawrz-megastack-spec.md)) governs wherever
> the two disagree. Superseded here:
>
> - **§3.1 — repo/submodule model:** the `services/cave-deck` submodule is retired. Deck
>   lives in-tree at `backend/deck/` and versions with the monorepo; in-tree components
>   replace the "pin the submodule and bump it via PR" lifecycle (master §1.2, §3.4, §3.5).
> - **§3.4 — LAN-only, no login:** a single PIN auth gate fronts Deck and the other RAWRZ
>   UIs (master §7).
> - **catalog `redis` entry:** promoted from an opt-in catalog item to a core RAWRZ
>   service (master §2.1, §4).
> - **This component's own CI and release line:** replaced by the unified pipeline and the
>   single release stream (master §3.5, §13).
>
> M0 itself changes no runtime behavior; the compose service, auth gate, and ingress land
> in later milestones (master §15).

---

## 1. Goal

Replace every interactive interface to The Bear Cave stack — the TUI (`stack-tui`), all `stack-*`/`cave-*` bash functions, the fish host tools, and the waybar integration — with a single sleek, fast, themeable, responsive **web application: Cave Deck**.

Cave Deck can do *anything and everything the stack needs*:

| Capability | Detail |
|---|---|
| **Full ops parity** | Every one of the ~110 bash + 25 fish functions has a GUI equivalent at v1 (§6). **Row-level mapping: Appendix C is authoritative** — it enumerates every function, its GUI feature ID, and its live-harness safety class |
| **Switching APIs** | A credentials/API-sources control panel: rotate *arr/Seerr/Plex/nzbdav keys, edit Usenet provider blocks, manage Prowlarr indexer credentials, switch list sources (TMDb/MDBList/IMDb) (§6.4) |
| **Writing to `.env`** | Validated env editor with diff preview, secret masking, and prompted apply cascades (§6.5) |
| **Unsticking media** | Interactive import-decision UI for blocked/pending imports (§6.6) |
| **Adding containers** | A researched catalog of **100 curated containers** that deploys compose-natively through automated PRs (§6.3) |

**Explicitly ruled out:** FastAPI (backend is Rust), auth flows (LAN-only owner trust), multi-host management, guest/read-only modes.

---

## 2. What is being replaced (the removal inventory)

All of the following are **deleted** when Cave Deck reaches v1 parity ("full removal" decision):

### 2.1 In `thebearcave` repo
| Item | Location today | Notes |
|---|---|---|
| Bash functions | `services/bash-functions/functions/` — 18 files, ~3,463 lines, **110+ functions** (`stack-*`, `__helpers`) | The primary operational surface |
| Bash loader | `services/bash-functions/bearcave-bash.sh` (`.bashrc` source line, env loader) | Reads `.env` literally today — its quirks die with it |
| Completions | `services/bash-functions/completions/stack-completions.sh` (generated) | |
| TUI launcher | `services/bash-functions/scripts/stack-tui`, `stack-tui-toggle.sh` | Waybar-toggled terminal launcher |
| Python-backed CLI helpers | `services/bash-functions/scripts/` (python gates invoked from functions) | Logic ports to Rust (§4.3) |
| Fish host tools | `services/host-tools/functions/*.fish` — 25 files (~265 lines) | Already not loaded on this host; removed from repo |
| Waybar integration | `services/bash-functions/waybar/` (config, style.css, record/nightlight/stack-tui scripts) + `check_waybar_drift.py`/tests | Replaced by Cave Deck's persistent status strip (§6.1.4) |
| Safe-command registry | `tests/live/safe_command_registry.yaml`, `tests/live/run_safe_matrix.sh`, `tests/live/registry.py` | The bash/fish parity harness loses its subject |
| Submodule | `services/cave-scripts` (`.gitmodules` entry) | Unpinned; its spec docs archived per retirement record |
| CI checks | `check_waybar_drift.py`, `check_stack_tui.py` (if present), live-matrix jobs, waybar-drift workflow steps | |
| **Python scripts** | `scripts/*.py` gates/checks/NOTIFIERs that exist solely as function backends | **Port everything to Rust** (§4.3); repo-internal CI-only scripts (compose validators, audit_residue) are evaluated per-script during implementation — anything the GUI needs is ported, anything CI needs stays |

### 2.2 On the host
- `~/.bashrc` loader line for `bearcave-bash.sh`
- `~/.config/fish/functions/stack-*.fish` symlinks (if ever reinstalled)
- `~/.config/waybar/scripts/stack-tui-toggle.sh` and friends (weather/updates/cava/gpu-usage stay **out of scope** — host-only dotfiles, not repo-managed)
- Nightly reclaim cron entry (`install-nightly-reclaim-cron.sh`) — replaced by Cave Deck's scheduler (§6.9)

### 2.3 What is NOT removed
- `docker-compose.yml`, `.env`/`.env.template`, `secrets/` — Cave Deck *manages* these, never replaces them
- Historical records: `CHANGELOG.md`, `HISTORY.md`, `TODO.md`, lifecycle retirement records
- `archive/` (reference material)
- The landmines in `AGENTS.md` — they become **enforced invariants in the Rust backend** (§7)

---

## 3. Architecture

### 3.1 Repository & deployment model ("Separate repo" + "Hybrid")

```
WhispersOfJ/cave-deck          (new repo — own CI, own releases)
├── backend/                   Rust (axum) — single static binary
├── frontend/                  React 18 + TypeScript + Vite
├── catalog/catalog.yaml       The 100-container curated catalog (§6.3)
├── spec/                      This spec's living copy + ADRs
└── .github/workflows/         CI: test, lint, build, release
```

- `thebearcave` pins `cave-deck` as a **submodule** at `services/cave-deck` (same pattern as the retiring `cave-scripts`), and carries the compose service + host-tools replacement records.
- **Compose service** `cave-deck`: container on `bearcave`, published port **7780** (chosen; free on host, not in the port map), `mem_limit: 512m`, healthchecked, binds `docker.sock` (read-write — it manages containers) plus the repo-root bind mounts it needs (`.env`, `docker-compose.yml`, `media/`, `config/` read paths).
- **Host systemd shim** (`cave-deck-host.service`, user-level): a minimal privileged helper the backend calls over a localhost Unix socket for operations the container cannot do: ufw rule edits, host package diagnostics (pacman, SMART, journalctl), filesystem paths outside bind mounts, systemctl timers. The shim is a thin, auditable command allowlist — no general shell.
- Update flow: new Cave Deck release → PR bumps the submodule pin in `thebearcave` (same automated-PR flow it uses for everything else).

### 3.2 Backend: Rust

- **Framework:** axum + tokio. Static binary, containerized (multi-stage Distroless/Alpine), target < 50 MB RSS idle.
- **Docker:** `bollard` (native Docker Engine API client over `docker.sock`).
- **Realtime:** WebSocket hub (tokio-tungstenite native in axum) — §5.3.
- **HTTP clients:** reqwest to the *arr/Plex/Seerr/nzbdav APIs (base URLs from `.env`, keys from `.env`/config).
- **Persistence:** SQLite via `sqlx` — one `cave-deck.db` in `config/cave-deck/` (§5.4).
- **Config/secret access:** reads/writes `.env` and `secrets/` through the same bind mounts compose uses; every write validated then atomically staged (§6.5).
- **Process supervision:** systemd `Restart=on-failure`; compose healthcheck hits `/api/healthz`.

### 3.3 Frontend: React + TypeScript + Vite

- React 18, TypeScript strict, Vite build, TanStack Query for request caching, Tailwind CSS with a token layer for theming.
- **API contract:** the Rust backend serves OpenAPI (utoipa) and the frontend generates a typed client from it — one source of truth, no drift.
- **Theming:** preset themes + dark/light toggle (§5.5). Implementation: CSS custom properties under a `data-theme` attribute; presets shipped as token files; choice persisted in localStorage + backend profile.
- **Responsiveness:** desktop-first layouts that degrade cleanly to phone width (TanStack Query + view transitions; tables become cards under 768px). Touch targets ≥ 44px on mobile.
- Served by the Rust binary as embedded static assets (single-binary deployment; no separate web server).

### 3.4 Network & security posture

- **LAN-only, no login** (owner-trust model, consistent with the rest of the stack).
- ufw rule: `allow from <owner-subnet> to any port 7780 proto tcp` (installed via the host shim during setup; the general bridge-subnet deny stays).
- No TLS (LAN HTTP like every other service here). The backend binds `0.0.0.0:7780` inside the container; exposure is controlled exclusively by ufw.
- CSRF surface: same-origin only; WebSocket origin-checked against the configured host IP.
- Audit log records the caller IP for every mutating action (§5.4).

---

## 4. Data flows & integration map

### 4.1 Surfaces Cave Deck talks to
| Surface | Protocol | Credential source | Used for |
|---|---|---|---|
| Docker | Unix socket API | socket perms | containers, images, stats, events, compose-apply |
| Radarr / Sonarr | REST `/api/v3` | `RADARR_API_KEY`, `SONARR_API_KEY` | queues, imports, series/movies, health, blocklist, calendars |
| Prowlarr | REST `/api/v1` | `PROWLARR_API_KEY` | indexers, applications sync, tests |
| Seerr | REST `/api/v1` | `SEERR_API_KEY` | requests, settings |
| Plex | REST | `PLEX_TOKEN` | sessions, libraries, Butler tasks, analysis, markers |
| NzbDAV (InfiniDysk) | SABnzbd-style `/api` | `FRONTEND_BACKEND_API_KEY` | queue, history, stats, add/cancel |
| rclone RC | `:5572` | `NZBDAV_RCLONE_RC_PASS` | mount status, VFS stats |
| Host | Unix socket → shim | — | ufw, pacman, journalctl, SMART, timers, paths |
| GitHub | `gh` CLI on host shim (or token) | existing gh auth | automated PR flow (§6.3.4) |
| Discord | webhook | `DISCORD_WEBHOOK_URL` | notifications (§6.8) |
| SMTP | `smtp.gmail.com:587` | `.env` `CAVE_DECK_SMTP_USER` + App Password | email digests (§6.8) |

### 4.2 The compose-is-truth principle
Every persistent change Cave Deck makes to the stack (new container, edited env, changed volume) lands as a **tracked change in `thebearcave` git** via automated PR. The GUI never leaves drift between what runs and what's committed.

### 4.3 Python reuse decision — "Port everything"
All function/script logic needed by Cave Deck is reimplemented natively in Rust and the Python originals are retired with the bash functions. Porting order is risk-ordered (§9); each ported op carries the original script's tests as Rust unit tests (the Python test files are the behavioral spec — e.g. `drain_sonarr_queue` semantics, `check_radarr_db_size` gate math, `maintenance_digest` finding levels).

CI-internal-only scripts in `thebearcave` (compose validators, audit_residue, secret manifest checks) **stay Python** — they validate the repo, not the running stack, and are not part of the GUI's job.

---

## 5. Non-functional requirements

### 5.1 Performance
- First contentful paint < 1s on LAN; route transitions < 100ms (SPA, prefetched data).
- Backend API p99 < 50ms for cached reads; live Docker stats sampled 1s and pushed as deltas.
- The backend must stay responsive while any long operation runs — all mutations are jobs (§5.2), never blocking the request path.

### 5.2 Job engine
Every mutating action (recreate, prune, search, import decision, env apply, catalog deploy) is a **job**: queued, cancellable, progress-reported over WebSocket, audit-logged, and idempotent where the underlying API allows. Jobs survive page reloads (server-side state). Concurrency: mutating jobs on the same target serialize; reads never block.

### 5.3 WebSocket topics
| Topic | Payload cadence |
|---|---|
| `docker.stats` | 1s per-container CPU/mem/net deltas |
| `docker.events` | as they occur (die/stop/oom/health_status) |
| `queue.arr` | on change (Sonarr/Radarr merged queue view) |
| `queue.nzbdav` | on change (download queue + speeds) |
| `mount.health` | 10s (mountpoint check, VFS stats, error counters) |
| `jobs` | job state transitions + progress |
| `logs.<container>` | live tail with filter (e.g. `logs.sonarr level=warn`) |
| `plex.sessions` | on change |
| `system.host` | 10s (mem/disk/load, from shim) |

Client auto-reconnects with exponential backoff and resyncs via a `snapshot` message per topic.

### 5.4 SQLite schema (cave-deck.db)
- `events(id, ts, kind, source, severity, message, json)` — stack event history (imports, failures, guard trips, job results)
- `audit(id, ts, actor_ip, action, target, params_json, job_id, outcome)` — every mutation
- `metrics(ts, container, cpu_pct, mem_bytes, rx_bps, tx_bps)` — rolling samples; retention 30d (auto-prune)
- `list_checkpoints(kind, key, state_json, updated_at)` — Letterboxd/MDBList import resume state (replaces `search_missing_scoped_checkpoint.py` behavior)
- `settings(key, value)` — theme choice, dashboard prefs, digest schedule
- WAL mode; file lives in `config/cave-deck/` so the existing backup manifest covers it (one line added to `backup.sh`'s service list).

### 5.5 Theming
- 6+ shipped presets (including a Bear Cave dark default, a light theme, and a high-contrast one), switchable instantly.
- Tokens: color scales, accent, fonts, radius, density, sidebar width.
- Preset = JSON token file in `frontend/themes/`; import/export in settings (no code needed to add themes).

---

## 6. Feature areas (v1 = full parity)

### 6.1 Dashboard & status
1. **Stack overview**: 8 containers + cave-deck — health, uptime, restarts, mem/CPU live, port links. Replaces `stack-status`, `stack-top`, `stack-version`.
2. **At-a-glance rows**: FUSE mount health (replaces `stack-mount-health`), nzbdav queue depth/speed (`stack-nzbdav-queue/stats`), Sonarr/Radarr queue counts (`stack-queue-status`), disk free (`stack-disk-free`/`stack-docker-disk-usage`), recent imports (`stack-arr-recently-added`, `stack-plex-recently-added`), Seerr open requests (`stack-seerr-requests`, `stack-requests`).
3. **Activity feed**: merged live events (WebSocket) + persisted history — replaces `stack-activity-feed`, `stack-arrivals`, `stack-recent`.
4. **Persistent mini status strip**: the waybar replacement — a slim always-visible bar in the app (mount state, queue depth, disk, alerts) that can also be popped out into a small always-on-top browser window ( Picture-in-Picture-style) for desktop display.

### 6.2 Containers
- **Lifecycle**: start/stop/restart/recreate(compose)/remove, with compose-aware dependency ordering (e.g. recreate nzbdav → rclone → dependents). Replaces `stack-container`, `stack-restart-all`.
- **Logs**: live tail any container, severity filter, download.
- **Stats**: per-container CPU/mem/net history charts (from `metrics`).
- **Images**: list, check upstream updates (`stack-image-check`), pull with progress, prune (`stack-disk-reclaim` docker leg).
- **Compose file view**: read-only rendering of the effective compose config with links to edit flows (env §6.5, catalog §6.3).

### 6.3 Container catalog (100 curated entries)
- **Curation:** implementation begins with AI-assisted research producing `catalog/catalog.yaml` — exactly **100 entries** across the four chosen categories, each verified against current upstream images:
  - *Media pipeline* (~35): *arr family (Lidarr, Readarr, Whisparr, Prowlarr-adjacent tools), Audiobookshelf, Navidrome, Kavita/Calibre-web, Tautulli, Overseerr-likes, subtitle tools, metadata agents
  - *Ops & monitoring* (~25): Dozzle, Glances, Netdata, Uptime Kuma-class, Scrutiny, cAdvisor-class, log viewers
  - *Network & security* (~20): AdGuard Home, Pi-hole, WireGuard/Tailscale, Vaultwarden, Caddy/NPM, crowdsec-class
  - *Productivity & home* (~20): Paperless-ngx, Immich, PhotoPrism, Nextcloud, Homarr, Homepage, Gitea/Forgejo, Jellyfin
- **Entry schema** (tracked, reviewable diff per catalog PR):
  ```yaml
  - id: navidrome
    name: Navidrome
    category: media
    retired: false          # false = not on thebearcave's retired registry
    image: ghcr.io/navidrome/navidrome:latest   # pinned digest option
    ports: ["4533:4533"]
    volumes: ["./config/navidrome:/data", "./media/music:/music:ro"]
    env: [{key: ND_SCANSCHEDULE, default: "1h"}]
    compose_fragment: |     # rendered into docker-compose.yml on install
      navidrome:
        <<: *common
        ...
    checks: {healthcheck: "...", docs: "https://..."}
    notes: "requires media/music to exist; add Plex library afterwards"
  ```
- **Retired-services policy: exclude entirely.** Anything on `thebearcave`'s `RETIRED_SERVICES` registry (lidarr, readarr, audiobookshelf, komga, adguard, crowdsec, vaultwarden, watchstate, uptime-kuma, n8n, cleanuparr, arr-dashboard, metacache, …) **never appears** in the catalog. The catalog build step cross-checks against `audit_residue.py`'s registry and CI fails if a retired name leaks in.
- **Install flow (compose-native, automated PR):**
  1. User picks a container → GUI renders the form from the entry schema (ports/env/volumes editable, conflict detection against the live port map).
  2. Cave Deck creates a branch `add-<id>` in `thebearcave`, appends the compose fragment (alphabetical, section-commented), adds config dirs to `validate.sh`'s list if needed, commits, pushes, opens a PR via `gh`, enables auto-merge.
  3. On merge: backend pulls main, runs `docker compose up -d <service>`, verifies health, and reports the result as a job + Discord/email event.
  4. Uninstall = same flow in reverse (removal PR), with a data-keeping choice (config dir retained or deleted).
- **Conflict rules:** port collisions (checked live + against compose), volume path overlap, memory-budget check (host 22 GiB vs current caps), name collisions with compose service/container names.

### 6.4 Credentials & API sources ("switching API" — all of it)
- ***arr / Seerr keys*: rotate Radarr/Sonarr/Prowlarr/Seerr API keys (generate new in-app, POST to each app, update `.env`, recompute dependents: nzbdav + unpackerr env, recyclarr secrets, Prowlarr application sync re-push). Replaces the manual config.xml extraction dance.
- **Plex**: token verification, `PLEX_CLAIM` first-run flow (the compose gap is fixed in `thebearcave` alongside), library token health.
- **Usenet providers**: full CRUD on the `NZBDAV_CONFIG__USENET__PROVIDERS` JSON (host/user/pass/conns/ssl) — incl. **enable/disable per provider** (the Eweka pattern), with a live credential **test** button (DNS→TCP→TLS→AUTHINFO probe, same as the manual test used during onboarding) and connections budget sum vs plan.
- **Indexer credentials**: Prowlarr indexer API keys, per-indexer enable, test, VIP expiry fields surfaced.
- **List sources**: switch import-list metadata source (TMDb/MDBList/IMDb) and credentials for the list-import flows (§6.7).
- **Secrets visibility: reveal on click** — every value masked by default; per-value reveal (auto re-masks after 15s); each reveal is audit-logged.
- Every credential change computes the **blast radius** (which consumers read that var) and routes through the §6.5 apply flow.

### 6.5 `.env` editor & apply engine ("writing to the .env")
- **View**: grouped like the template (identity, internal secrets, providers, arr keys, notification), masked values, `changeme`/stale detection (`__bearcave_warn_stale_keys` behavior, ported).
- **Edit**: validated inputs per var type (port ranges, URLs, hex keys, cron expressions); inline docs from `.env.template` comments; diff preview before save.
- **Apply ("Prompted apply")**:
  1. Backend computes affected services from a var→consumer map (e.g. `SONARR_API_KEY` → sonarr, nzbdav, unpackerr, recyclarr-secrets).
  2. For any recreate touching **nzbdav**, the **queue-empty guard** runs first (landmine #4) — if the queue is non-empty the apply is blocked with the live queue shown.
  3. One user click → writes `.env` atomically (with backup copy), regenerates derived files (recyclarr secrets.yml), recreates services in dependency order, verifies health, reports the cascade.
- **Validation parity**: the GUI runs the same checks as `setup.sh --validate-only` (ported) so a saved `.env` is always compose-valid.

### 6.6 Unstick media ("Import decision UI")
- **Stuck views**: Sonarr/Radarr queues filtered to `importBlocked` / `importPending` / `warning` with the full status messages (the ID-mismatch, missing-episode, sample-check messages). Replaces `stack-arr-queue-errors`, `stack-queue-autofix` manual runs.
- **Decision UI per item** (interactive, like Sonarr's manual import):
  - **Map**: choose the correct series/season/episode (search-assisted) — solves wrong-series grabs (the Live Untucked case).
  - **Override**: force import despite ID-match warnings, with explicit confirmation.
  - **Delete + re-search**: remove from client, optionally blocklist, trigger episode/season search.
  - **Bulk**: select-all-same-cause actions.
- **Guard rails**: recreates triggered from here still respect landmine #4; blocklist default follows Sonarr semantics (on for bad releases, off for wrong-series valid releases — the exact distinction handled manually during the Drag Race wave).
- Backlog surfacing: `stack-arr-backlog`, `stack-backlog-status`, `stack-arr-missing-aired`, `stack-cutoff-unmet` become filterable library-health views with one-click scoped searches (the `search_missing_scoped*` behavior, ported).

### 6.7 Library & list tools (full parity)
- **List imports**: Letterboxd (`stack-letterboxd-*`: import/track/tracked/untrack/history) and MDBList (`stack-mdblist-*`) management UIs; tracked lists table with per-item state, resume checkpoints persisted (§5.4); `stack-import-lists`, `stack-arr-import(-all/-candidates/-starvation)`, `stack-loop-candidates/exclude/unmonitor`, `stack-loop-ratings`, `stack-rating-imdb/mdblist` all have equivalents.
- **Watchable**: `stack-watchable`, `stack-unwatched`, `__watchable_*` — Plex-backed "what can we watch" views with Seerr availability cross-reference.
- **Sonarr/Radarr library health**: monitoring fixes (`stack-sonarr-fix-episode-monitoring`), series/episode toggles (`stack-arr-toggle-search`), blocklist view/clear (`stack-arr-blocklist/-clear-blocklist`), health & logs (`stack-radarr-health`, `stack-arr-logs`, `stack-log-levels`), DB prune buttons (`stack-radarr-prune`/`stack-sonarr-prune` — full backup → prune → vacuum → verify flow as a job).
- **Import-lists overview** (`stack-import-lists`), config sizes (`stack-disk-config-sizes`), OOM/perms checks (`stack-oom-check`, `stack-perms-check`).

### 6.8 Plex suite (all `stack-plex-*`)
- Sessions (live), libraries list, refresh/scan, empty trash (with the red-trash-can landmine check: mount verified healthy *before* allowing the rescan+empty sequence), analyze/deep-analysis/loudness/music analysis, markers (intro/credits/chapters/ad) generate + view, media index, voice activity, butler tasks (all/single), database backup, cache/log cleanup, image clean (idle-gated), EPG refresh, automatic-updates toggle. Each as a job with progress.

### 6.9 Host tools (via shim) + scheduling + notifications
- **Host diagnostics**: pacman updates/orphans/cache (`stack-pkg-*`), journal errors/size (`stack-journal-*`), failed units (`stack-service-failed`), zombies (`stack-zombie-check`), mem pressure, SMART/btrfs (`stack-disk-health`), reboot/kernel pending (`stack-reboot-check`, `stack-kernel-check`), SSH doctor, AUR audit, uptime report, flatpak updates, git status across repos (`stack-git-status-all`), firewall status, cron/timer listing (`stack-cron-list`, `stack-timer-status`), worktree status (`stack-worktree`), config/secret/mount drift (`stack-config-drift` + ported check scripts), audit residue view (`stack-audit-residue`), Claude backup (`stack-claude-full-backup`).
- **Maintenance digest**: the monthly digest (`maintenance_digest.py`) becomes a scheduled Cave Deck job (DB gates radarr/sonarr, nzbdav queue, import-queue depth, host checks) delivered **in-app + Discord + email**; nightly reclaim cron replaced by an internal scheduler (same guarded reclaim logic).
- **Notifications**:
  - **In-app**: activity feed + toasts (always).
  - **Discord**: the existing webhook; events (imports, failures, guard trips, job completions, digest) post as embeds. Replaces `arrival_notifier`/`notify-test` Discord legs.
  - **Email digest**: scheduled summary via **Gmail + App Password** (`smtp.gmail.com:587`, STARTTLS; creds in `.env`: `CAVE_DECK_SMTP_USER`, `CAVE_DECK_SMTP_PASS` — App Password instructions linked in settings; `CAVE_DECK_SMTP_TO` for recipients). Daily or weekly digest cadence selectable; includes overnight arrivals, stuck items, gate failures, pending updates.
- **nzbdav tools**: queue view/cancel, history, stats, dedup check, delete-failures (`stack-nzbdav-*`), and the **safe recreate** flow (`nzbdav-safe-recreate.sh` logic: confirm pending 0 → recreate → verify frontend + authenticated API + PROPFIND health — the landmine #13 triple check).

---

## 7. Landmine enforcement (invariants in code)

The AGENTS.md landmines become runtime guards — the backend **refuses** unsafe operations with a human explanation (two-step confirm dialog shows the guard result):

| # | Landmine | Cave Deck enforcement |
|---|---|---|
| 1 | Bind-mount staleness | Any edit to a bind-mounted file via the env editor flags "restart required" and offers the guarded recreate |
| 2 | FUSE mount fragility | Never umounts; recreate cascade = owner (nzbdav_rclone) then dependents in order; mount-health check before any Plex trash-empty |
| 3 | Plex grace period | Compose apply never shortens `stop_grace_period`; plex recreates wait for clean stop |
| 4 | NzbDAV queue non-persistence | **Hard guard**: any nzbdav recreate requires authenticated queue==0 check; block + show queue otherwise |
| 5 | Plex host network | Catalog/apply validation refuses bridge-network plex edits |
| 6 | rclone obscure | Provider writes that touch rclone.conf run `rclone obscure` server-side; plaintext never written |
| 7 | Exhaustive removal checklists | Removals in `thebearcave` go through the same automated-PR flow with a checklist template |
| 8 | Radarr orphaned profiles | Post-profile-delete verification job (`check_radarr_profiles` port) offered after any quality-profile action |
| 9 | SQLite bloat | DB-size gates on dashboard; prune flows built-in (§6.7) |
| 10 | ImageMaid path validation | If ImageMaid is ever run from catalog, the PhotoTranscoder path warning is built into its form |
| 13 | Frontend-masks-backend | nzbdav health = `/healthz` + authenticated queue API + PROPFIND triple check before "healthy" is displayed |

---

## 8. CI/CD (cave-deck repo)

- **Worktree discipline** carries over: all changes on task worktrees, PRs only, linear history, squash.
- CI: `cargo test/clippy/fmt`, `cargo audit`, `vitest` + `tsc --noEmit` for frontend, Playwright smoke (dashboard renders, one job per feature area) against a compose test-profile, `actionlint`, SHA-pinned actions, gitleaks, Trivy on the built image.
- Releases: release-please; images published to GHCR by digest; `thebearcave` submodule-bump PRs automated.
- Catalog CI: schema validation + retired-registry cross-check + image-tag freshness report (weekly scheduled).

## 9. Rollout (ordered, each step mergeable)

1. **M0 — Skeleton**: repo, Rust+React skeleton, compose service, dashboard read-only (containers, health, mount) — old interfaces still live.
2. **M1 — Read parity**: queues, stats, logs, activity feed, host diagnostics. (All read-only.)
3. **M2 — Env & credentials**: env editor + apply engine, credentials panel with tests, guard #4 live.
4. **M3 — Mutations**: container lifecycle, Plex suite, unstick UI, prunes, nzbdav safe-recreate.
5. **M4 — Catalog**: 100-entry catalog research PR (user reviews), install/uninstall PR flow.
6. **M5 — Lists & watchable**: Letterboxd/MDBList/loop-ratings ports, checkpoints.
7. **M6 — Notifications & digest**: Discord + Gmail SMTP + scheduler.
8. **M7 — Removal**: the §2 inventory PR(s) in `thebearcave` (functions, fish, waybar, TUI, registry, submodule unpinned, CI checks) — the "full removal" commit.
9. **M8 — Polish**: theme presets beyond default, mobile passes, performance budgets.

**Definition of done (v1):** every §6 feature live; §2 inventory deleted; `tests/health/run-all.sh` extended with a cave-deck check; the stack runs one full week with zero terminal commands needed for any operation the GUI covers.

## 10. Acceptance checklist (extract)

- [ ] Every function row in **Appendix C** resolves to a GUI feature ID (or carries an explicit `retire:` marker); the cave-deck repo exports Appendix C as `parity/parity.yaml` and CI fails on any row whose feature ID is not implemented
- [ ] Env write → prompted apply → guarded recreate cascade demonstrable end-to-end
- [ ] Catalog install of a new container produces: PR → CI green → merge → healthy service, no manual terminal steps
- [ ] Retired-service names cannot appear in the catalog (CI-enforced)
- [ ] nzbdav recreate blocked while queue non-empty (red path test)
- [ ] Secret values masked; reveal audit-logged
- [ ] Digest email arrives from Gmail app-password relay; Discord embeds post
- [ ] UI usable at 375px width; theme switch < 100ms; Lighthouse ≥ 95 perf on LAN
- [ ] All landmine guards (#2, #4, #5, #6, #13 at minimum) have red-path tests

## 11. Open questions (non-blocking, decide during implementation)

1. Port 7780 confirmation (or another free port) at M0.
2. Whether `thebearcave`'s remaining CI-only Python scripts absorb any Cave-Deck-style checks later.
3. Exact Discord event taxonomy (which events post vs stay in-app) — default: failures + guard trips + job completions + digest only.
4. Digest default cadence (daily vs weekly) — default weekly.
5. Whether the host shim ships as part of cave-deck repo (Rust, `cargo build`) or a tiny separate binary — default: same repo, `/host-shim`.

---

# Appendix A — Catalog schema & validation rules

This appendix is the normative definition of `cave-deck` repo file `catalog/catalog.yaml`. CI enforces every rule below; the GUI's install form is generated from the same schema.

## A.1 File structure

```yaml
version: 1                  # schema version; bump = breaking change to entries
updated: 2026-09-06         # last review date (CI warns if > 90 days)
entries: [ ... ]            # exactly 100 entries in v1
```

## A.2 Entry schema (authoritative fields)

| Field | Type | Req | Rules |
|---|---|---|---|
| `id` | string | ✅ | `^[a-z0-9][a-z0-9-]*$`; **globally unique**; used as compose service name + config dir name |
| `name` | string | ✅ | Human name; **must not** contain a retired-registry token (§A.4) |
| `category` | enum | ✅ | `media` \| `ops` \| `network` \| `home` |
| `retired` | bool | ✅ | Must be `false` (retired names never ship — §A.4); field exists so a future de-listing is data, not schema |
| `image` | string | ✅ | `registry/repo:tag`; tag required (no bare repo); `:latest` allowed only with `digest_pin: recommended` note |
| `digest` | string | – | `sha256:...` when the image is pinned; CI freshness job flags entries whose tag moved > 30 days |
| `ports` | list | – | `"HOST:CONTAINER[/proto]"`, proto `tcp` default; HOST 1–65535; **unique per (host port, protocol) pair across the whole catalog** — tcp and udp may share a host port (catches collisions at curation time, not install time) |
| `volumes` | list | – | `"SRC:DST[:mode]"`; SRC absolute or `./`-relative (repo-relative in thebearcave); DST absolute; modes `ro` allowed; **DST collisions within one entry are an error** |
| `env` | list | – | `{key, default?, secret?, description?}`; KEY `^[A-Za-z_][A-Za-z0-9_]*$` (Docker's env-name rule — **case-sensitive**, so Pi-hole v6's lowercase `FTLCONF_webserver_api_password` is valid); `secret: true` entries **must not** ship a `default` (the GUI prompts and writes `secrets/`) |
| `capabilities` | list | – | Docker caps to add (`NET_ADMIN`, `SYS_ADMIN`, ...); **non-empty requires `notes` justification** |
| `host_network` | bool | – | default `false`; `true` requires `notes` justification (stack precedent: Plex only) |
| `pid_mode` | string | – | default unset; `host` shares the host PID namespace (process visibility — e.g. Glances); requires `notes` justification |
| `devices` | list | – | e.g. `/dev/dri`, `/dev/net/tun`; requires `notes` |
| `dependencies` | list | – | Other catalog `id`s this entry requires (e.g. paperless → redis); install flow offers to add them together |
| `mem_limit` | string | ✅ | e.g. `512m`; values above `2g` require `notes` (host budget: 22 GiB) |
| `healthcheck` | string | – | CMD-SHELL test; entries with a web UI should ship one |
| `docs` | string (URL) | ✅ | Upstream install docs the entry was verified against |
| `notes` | string | – | Install gotchas, post-install steps, stack-specific warnings |
| `compose_fragment` | string (YAML) | – | **Optional override.** By default the renderer generates the service block from the fields above (`<<: *common` anchor prepended, `container_name = id`, `restart: unless-stopped`, networks `[bearcave]`). Only special cases (host network, caps, custom sysctls) ship a fragment; the fragment must still satisfy §A.5 |

## A.3 Reserved names

`id` must not collide with: the 8 always-on services (`prowlarr, radarr, sonarr, nzbdav, nzbdav-rclone, seerr, plex, unpackerr`), the maintenance profile (`imagemaid, recyclarr`), or `cave-deck`.

## A.4 Retired-registry cross-check (CI-enforced)

- `catalog/retired-registry.lock` is a **synced copy** of `RETIRED_SERVICES` from `thebearcave/scripts/audit_residue.py` (currently: traefik, loki, promtail, grafana, prometheus, alertmanager, node-exporter, cadvisor, nzbdav-exporter, arr-dashboard, landing-page, metacache, lidarr, readarr, audiobookshelf, komga, adguard, crowdsec, vaultwarden, watchstate, cleanuparr, uptime-kuma, n8n, control-panel, bazarr).
- A nightly CI job fetches `audit_residue.py` from `thebearcave@main` and fails if the lock drifted (new retirement ⇒ the catalog entry disappears same day).
- **Check:** for every entry, `id`, `name` (lowercased, non-alphanumerics stripped), and every path token of `image`'s repo are matched against the registry. Any hit ⇒ CI fails. **Substring risk is accepted and mitigated** by word-boundary matching (e.g. `bazarr` must not match a hypothetical `subbazarrx`; but an entry named *Bazarr Reloaded* does fail — curation avoids family names entirely).
- The install flow re-runs the same check at GUI runtime (defense in depth) so a stale lock can't leak a retired service into a live PR.

## A.5 Generated/overridden compose fragment rules

A fragment (generated or shipped) is valid only if it:
1. has exactly one top-level key = `id`;
2. includes `<<: *common` as its first key;
3. sets `container_name: <id>`, `mem_limit`, `networks: [bearcave]` (unless `host_network: true`, then `network_mode: host` and **no** `ports:`);
4. declares no `depends_on` on services outside the stack + catalog `dependencies`;
5. binds no host port already published in `thebearcave`'s compose (renderer checks live + file); 
6. mounts nothing outside `./config/<id>`, `./media/*`, and declared `volumes`.

## A.6 CI pipeline (catalog)

| Job | When | Fails on |
|---|---|---|
| `schema` | every PR | YAML/schema violations, duplicate `id`/ports, missing fields |
| `retired-check` | every PR + nightly | any §A.4 hit; lock drift vs thebearcave |
| `count` | every PR | entries ≠ 100 (v1) |
| `freshness` | weekly | image tags > 30 days behind upstream, digests stale |
| `render` | every PR | fragment generation fails / §A.5 violations |

---

# Appendix B — Seed catalog (26 verified entries)

Proves the schema against real containers. Categories per §6.3 target mix. All images verified current as of 2026-09-06 (rebrand-sensitive paths checked: Kometa, wg-easy v15, Pi-hole v6 env vars). **Deliberately absent:** every retired-registry name (no AdGuard, no Uptime Kuma, no Vaultwarden, no Grafana/Prometheus stack, no Lidarr/Readarr/Audiobookshelf/Komga, no torrent clients — Usenet-only stack policy).

## Media (10)

```yaml
- id: navidrome
  name: Navidrome
  category: media
  retired: false
  image: ghcr.io/navidrome/navidrome:latest
  ports: ["4533:4533/tcp"]
  volumes: ["./config/navidrome:/data", "./media/music:/music:ro"]
  env: []
  mem_limit: 512m
  healthcheck: "wget -qO- http://localhost:4533/ping >/dev/null 2>&1 || exit 1"
  docs: https://www.navidrome.org/docs/installation/#docker
  notes: "Subsonic-compatible music server; create ./media/music first"

- id: tautulli
  name: Tautulli
  category: media
  retired: false
  image: ghcr.io/tautulli/tautulli:latest
  ports: ["8181:8181/tcp"]
  volumes: ["./config/tautulli:/config"]
  env:
    - {key: PLEXURL, description: "set to http://plex:32400 after first run"}
  mem_limit: 512m
  docs: https://github.com/Tautulli/Tautulli/wiki/Installation
  notes: "Plex stats & monitoring; point at PLEX_TOKEN-backed URL in setup"

- id: kometa
  name: Kometa
  category: media
  retired: false
  image: kometateam/kometa:latest
  ports: []
  volumes: ["./config/kometa:/config"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 1g
  docs: https://kometa.wiki/en/latest/kometa/install/docker/
  notes: "Plex collections/metadata (formerly plex-meta-manager); config.yml required before start; runs on schedule via its own config"

- id: calibre-web
  name: Calibre-Web
  category: media
  retired: false
  image: lscr.io/linuxserver/calibre-web:latest
  ports: ["8083:8083/tcp"]
  volumes: ["./config/calibre-web:/config", "./media/books:/books"]
  env:
    - {key: PUID, default: "${PUID}"}
    - {key: PGID, default: "${PGID}"}
    - {key: TZ, default: "${TZ}"}
    - {key: DOCKER_MODS, default: "linuxserver/mods:universal-calibre", description: "optional ebook-conversion binary"}
  mem_limit: 512m
  docs: https://docs.linuxserver.io/images/docker-calibre-web/
  notes: "Requires an existing metadata.db in ./media/books (create with calibre once)"

- id: kavita
  name: Kavita
  category: media
  retired: false
  image: ghcr.io/jvmilazz0/kavita:latest
  ports: ["5000:5000/tcp"]
  volumes: ["./config/kavita:/kavita/config", "./media/reading:/data:ro"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  docs: https://wiki.kavitareader.com/en/install/docker
  notes: "Comics/manga/ebooks; /data expects subfolders per library type"

- id: mylar3
  name: Mylar3
  category: media
  retired: false
  image: lscr.io/linuxserver/mylar3:latest
  ports: ["8090:8090/tcp"]
  volumes: ["./config/mylar3:/config", "./media/reading/comics:/comics"]
  env:
    - {key: PUID, default: "${PUID}"}
    - {key: PGID, default: "${PGID}"}
  mem_limit: 512m
  docs: https://docs.linuxserver.io/images/docker-mylar3/
  notes: "Comic book *arr-style acquisition; downloads land in /comics"

- id: jellyfin
  name: Jellyfin
  category: media
  retired: false
  image: jellyfin/jellyfin:latest
  ports: ["8096:8096/tcp"]
  volumes:
    - "./config/jellyfin:/config"
    - "./config/jellyfin-cache:/cache"
    - "./media/movies:/media/movies:ro"
    - "./media/shows:/media/shows:ro"
  devices: ["/dev/dri:/dev/dri"]
  mem_limit: 2g
  docs: https://jellyfin.org/docs/general/installation/container/
  notes: "Alternative media server; /dev/dri enables VAAPI transcode like Plex; reads the same media trees"

- id: sabnzbd
  name: SABnzbd
  category: media
  retired: false
  image: lscr.io/linuxserver/sabnzbd:latest
  ports: ["8085:8080/tcp"]
  volumes: ["./config/sabnzbd:/config", "usenet:/usenet"]
  env:
    - {key: PUID, default: "${PUID}"}
    - {key: PGID, default: "${PGID}"}
  mem_limit: 1g
  docs: https://docs.linuxserver.io/images/docker-sabnzbd/
  notes: "Standalone Usenet client for a SECOND server/stack; never point the live *arr apps at it (nzbdav is the stack's client)"

- id: jellyseerr
  name: Jellyseerr
  category: media
  retired: false
  image: fallenbagel/jellyseerr:latest
  ports: ["5056:5055/tcp"]
  volumes: ["./config/jellyseerr:/app/config"]
  env: []
  mem_limit: 512m
  docs: https://github.com/Fallenbagel/jellyseerr#readme
  notes: "Requests UI for Jellyfin ecosystems; host port shifted 5055→5056 (Seerr owns 5055 here)"

- id: flaresolverr
  name: FlareSolverr
  category: media
  retired: false
  image: ghcr.io/flaresolverr/flaresolverr:latest
  ports: ["8191:8191/tcp"]
  volumes: []
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 1g
  docs: https://github.com/FlareSolverr/FlareSolverr#installation
  notes: "Cloudflare-bypass proxy for *arr indexers; add as Prowlarr indexer settings → FlareSolverr when an indexer demands it"
```

## Ops & monitoring (8)

```yaml
- id: dozzle
  name: Dozzle
  category: ops
  retired: false
  image: ghcr.io/amir20/dozzle:latest
  ports: ["9999:8080/tcp"]
  volumes: ["/var/run/docker.sock:/var/run/docker.sock:ro"]
  env: []
  mem_limit: 128m
  healthcheck: "wget -qO- http://localhost:8080/health >/dev/null 2>&1 || exit 1"
  docs: https://dozzle.dev/guide/getting-started
  notes: "Live container logs; read-only socket mount. Complements Cave Deck (which owns mutations)"

- id: glances
  name: Glances
  category: ops
  retired: false
  image: nicolargo/glances:latest-full
  ports: ["61208:61208/tcp"]
  volumes: ["/var/run/docker.sock:/var/run/docker.sock:ro", "/:/rootfs:ro"]
  env:
    - {key: GLANCES_OPT, default: "-w"}
  pid_mode: host
  mem_limit: 256m
  docs: https://nicolargo.github.io/glances/
  notes: "Full-system view; pid:host + rootfs:ro required for process/host stats"

- id: netdata
  name: Netdata
  category: ops
  retired: false
  image: netdata/netdata:latest
  ports: ["19999:19999/tcp"]
  volumes:
    - "./config/netdata:/etc/netdata"
    - "netdata-cache:/var/cache/netdata"
    - "/:/host/root:ro,rslave"
    - "/proc:/host/proc:ro"
    - "/sys:/host/sys:ro"
  capabilities: [SYS_PTRACE, SYS_ADMIN]
  mem_limit: 1g
  docs: https://learn.netdata.cloud/docs/netdata-for-infrastructure/installation/docker
  notes: "1-second metrics; caps required for full telemetry — trim SYS_ADMIN by dropping cgroups collector if unwanted"

- id: scrutiny
  name: Scrutiny
  category: ops
  retired: false
  image: ghcr.io/analogj/scrutiny:latest-omnibus
  ports: ["8888:8080/tcp"]
  volumes:
    - "./config/scrutiny:/opt/scrutiny/config"
    - "./config/scrutiny-db:/opt/scrutiny/db"
    - "/run/udev:/run/udev:ro"
  capabilities: [SYS_ADMIN, SYS_RAWIO]
  devices: ["/dev/nvme0n1:/dev/nvme0n1"]
  mem_limit: 256m
  docs: https://github.com/AnalogJ/scrutiny#docker
  notes: "SMART dashboard; devices list must enumerate every physical disk (setup form asks); omni bus image includes its own TSDB"

- id: portainer
  name: Portainer CE
  category: ops
  retired: false
  image: portainer/portainer-ce:latest
  ports: ["9443:9443/tcp"]
  volumes:
    - "./config/portainer:/data"
    - "/var/run/docker.sock:/var/run/docker.sock"
  mem_limit: 256m
  docs: https://docs.portainer.io/start/install/server/docker
  notes: "General Docker UI. Cave Deck remains the stack-specific surface; Portainer is the escape hatch — write access to the socket is intentional here"

- id: gatus
  name: Gatus
  category: ops
  retired: false
  image: twinproduction/gatus:latest
  ports: ["8089:8080/tcp"]
  volumes: ["./config/gatus:/config"]
  env:
    - {key: GATUS_CONFIG_PATH, default: "/config/gatus.yaml"}
  mem_limit: 256m
  healthcheck: "wget -qO- http://localhost:8080/health >/dev/null 2>&1 || exit 1"
  docs: https://gatus.io/docs/installation
  notes: "Uptime/health checks with alerting to the same Discord webhook; gatus.yaml authored at install"

- id: dockge
  name: Dockge
  category: ops
  retired: false
  image: louislam/dockge:1
  ports: ["5001:5001/tcp"]
  volumes:
    - "./config/dockge:/app/data"
    - "/var/run/docker.sock:/var/run/docker.sock"
    - "/home/bear/Cave:/home/bear/Cave"
  mem_limit: 256m
  docs: https://github.com/louislam/dockge#readme
  notes: "Compose stack manager. Repo path mount lets it see thebearcave compose; Cave Deck PR flow remains the tracked path — treat Dockge as read-mostly"

- id: diun
  name: Diun
  category: ops
  retired: false
  image: crazymax/diun:4
  ports: []
  volumes: ["./config/diun:/data", "/var/run/docker.sock:/var/run/docker.sock:ro"]
  env:
    - {key: DIUN_WATCH_WORKERS, default: "5"}
    - {key: DIUN_PROVIDERS_DOCKER, default: "true"}
    - {key: DIUN_NOTIF_DISCORD_WEBHOOKURL, secret: true, description: "reuse stack Discord webhook"}
  mem_limit: 128m
  docs: https://crazymax.dev/diun/
  notes: "Notifies (Discord) when any running image has a newer tag — pin-aware companion, never auto-pulls"
```

## Network & security (4)

```yaml
- id: wg-easy
  name: WireGuard Easy
  category: network
  retired: false
  image: ghcr.io/wg-easy/wg-easy:15
  ports: ["51821:51821/tcp", "51820:51820/udp"]
  volumes: ["./config/wg-easy:/etc/wireguard", "/lib/modules:/lib/modules:ro"]
  env:
    - {key: WG_HOST, description: "public IP or DDNS hostname"}
    - {key: PASSWORD_HASH, secret: true, description: "`docker run ghcr.io/wg-easy/wg-easy -p \"password\"` to generate"}
  capabilities: [NET_ADMIN]
  devices: ["/dev/net/tun:/dev/net/tun"]
  mem_limit: 256m
  docs: https://wg-easy.github.io/wg-easy/v15.3/getting-started/
  notes: "v15 syntax; /lib/modules ro-mount + /dev/net/tun required for the wireguard kernel module; host port 51820/udp must be opened in ufw/router for remote peers; cap NET_ADMIN is inherent to WireGuard"

- id: pihole
  name: Pi-hole
  category: network
  retired: false
  image: pihole/pihole:latest
  ports: ["8082:80/tcp", "53:53/tcp", "53:53/udp"]
  volumes: ["./config/pihole:/etc/pihole"]
  env:
    - {key: TZ, default: "${TZ}"}
    - {key: FTLCONF_webserver_api_password, secret: true, description: "v6 web UI password (NOT legacy WEBPASSWORD)"}
  capabilities: [NET_ADMIN]
  mem_limit: 512m
  healthcheck: "pihole-FTL --verify-dns"
  docs: https://docs.pi-hole.net/docker/configuration/
  notes: "v6 env naming: FTLCONF_* lower-camel suffixes; host ports 53 tcp+udp require systemd-resolved's stub listener disabled first (installer form warns)"

- id: authelia
  name: Authelia
  category: network
  retired: false
  image: authelia/authelia:latest
  ports: ["9091:9090/tcp"]
  volumes: ["./config/authelia:/config"]
  env: []
  mem_limit: 256m
  healthcheck: "wget -qO- http://localhost:9090/api/health >/dev/null 2>&1 || exit 1"
  docs: https://www.authelia.com/installation/docker/
  notes: "Forward-auth/2FA for web apps; needs configuration.yml + users.yml seeded at install (setup form generates them)"

- id: caddy
  name: Caddy
  category: network
  retired: false
  image: caddy:2-alpine
  ports: ["8081:80/tcp", "8443:443/tcp"]
  volumes:
    - "./config/caddy:/data"
    - "./config/caddy-config:/config"
  env: []
  mem_limit: 256m
  docs: https://caddyserver.com/docs/running#docker-compose
  notes: "Reverse proxy w/ auto-TLS (needs routable DNS for LE); LAN-only stack may prefer internal CA — Caddyfile authored at install; host 80/443 shifted to 8081/8443 to avoid conflicts"
```

## Productivity & home (4)

```yaml
- id: paperless-ngx
  name: Paperless-ngx
  category: home
  retired: false
  image: ghcr.io/paperless-ngx/paperless-ngx:latest
  ports: ["8000:8000/tcp"]
  volumes:
    - "./config/paperless:/usr/src/paperless/data"
    - "./config/paperless-media:/usr/src/paperless/media"
    - "./media/documents/consume:/usr/src/paperless/consume"
    - "./media/documents/export:/usr/src/paperless/export"
  env:
    - {key: PAPERLESS_REDIS, default: "redis://paperless-redis:6379"}
    - {key: PAPERLESS_TIME_ZONE, default: "${TZ}"}
    - {key: PAPERLESS_SECRET_KEY, secret: true}
    - {key: PAPERLESS_OCR_LANGUAGE, default: "eng"}
  dependencies: [paperless-redis]
  mem_limit: 1g
  healthcheck: "curl -sf http://localhost:8000/ >/dev/null || exit 1"
  docs: https://docs.paperless-ngx.com/setup/#docker
  notes: "Document archive; requires its redis sidecar (below). OCR consumes CPU on import — 1 CPU suggested"

- id: paperless-redis
  name: Paperless Redis
  category: home
  retired: false
  image: redis:7-alpine
  ports: []
  volumes: ["./config/paperless-redis:/data"]
  env: []
  mem_limit: 256m
  docs: https://docs.paperless-ngx.com/configuration/#redis
  notes: "Internal sidecar for paperless-ngx; no published port; installed together, not listed standalone in the GUI grid"

- id: homarr
  name: Homarr
  category: home
  retired: false
  image: ghcr.io/homarr-labs/homarr:latest
  ports: ["7575:7575/tcp"]
  volumes: ["./config/homarr:/app/data"]
  env: []
  mem_limit: 512m
  docs: https://homarr.dev/docs/getting-started/installation/docker
  notes: "Dashboard linking all stack UIs; v1 image org is homarr-labs (older guides show ekzhang/homarr — wrong)"

- id: photoprism
  name: PhotoPrism
  category: home
  retired: false
  image: photoprism/photoprism:latest
  ports: ["2342:2342/tcp"]
  volumes:
    - "./config/photoprism:/photoprism/storage"
    - "./media/photos:/photoprism/originals"
  env:
    - {key: PHOTOPRISM_ADMIN_PASSWORD, secret: true}
    - {key: PHOTOPRISM_SITE_URL, default: "http://192.168.4.105:2342/"}
  devices: ["/dev/dri:/dev/dri"]
  mem_limit: 2g
  healthcheck: "wget -qO- http://localhost:2342/api/v1/status >/dev/null 2>&1 || exit 1"
  docs: https://docs.photoprism.app/getting-started/docker-compose/
  notes: "Photo library w/ AI indexing; SQLite storage is fine for <100k photos, MariaDB (catalog extension) beyond; /dev/dri for TF/VAAPI accel"
```

### Port budget in the seed set (conflict-free by construction)

4533, 8181, 8083, 5000, 8090, 8096, 8085→8080, 5056→5055, 8191, 9999→8080, 61208, 19999, 8888→8080, 9443, 8081→8080, 5001, —, 51821+51820/udp, 8082→80+53, 9091→9090, 8081→80+8443→443, 8000, —, 7575, 2342 — none collide with each other or the stack's published ports (3000/5055/7878/8989/9696/32400/7780). Where upstream defaults collided (8080×4, 5055, 80/443) the host side was shifted and the entry's `notes` says so — the install form pre-fills these and conflict detection re-verifies at deploy time.

---

## B.2 Catalog entries 27–50 (extended verified set)

24 more entries researched 2026-09-06 (rebrand-sensitive checks: Watchtower archived Dec 2025 → socket-proxy chosen instead; Maintainerr org moved `jorenn92` → `maintainerr`; LSIO Requestrr deprecated → thomst08 fork; Mealie v1 container port 9000; Tdarr node port no longer published; ErsatzTV port 8409). Running total: **50 of 100**.

### Media (11)

```yaml
- id: whisparr
  name: Whisparr
  category: media
  retired: false
  image: ghcr.io/hotio/whisparr:release-3
  ports: ["6969:6969/tcp"]
  volumes: ["./config/whisparr:/config", "./media:/data"]
  env:
    - {key: PUID, default: "1000"}
    - {key: PGID, default: "1000"}
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  healthcheck: "curl -sf http://localhost:6969/ping || exit 1"
  docs: https://hotio.dev/containers/whisparr/
  notes: "*arr-family (adult); no official image — hotio build; v3 tag line"

- id: maintainerr
  name: Maintainerr
  category: media
  retired: false
  image: ghcr.io/maintainerr/maintainerr:latest
  ports: ["6291:6291/tcp"]
  volumes: ["./config/maintainerr:/opt/data"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  healthcheck: "curl -sf http://localhost:6291/ || exit 1"
  docs: https://docs.maintainerr.info/installation/
  notes: "Plex/Jellyfin library maintenance; org moved from jorenn92 (deprecated) — catalog pins the new maintainerr org; data at /opt/data"

- id: requestrr
  name: Requestrr
  category: media
  retired: false
  image: thomst08/requestrr:latest
  ports: ["4545:4545/tcp"]
  volumes: ["./config/requestrr:/config"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 256m
  docs: https://github.com/thomst08/requestrr
  notes: "Discord→Seerr/Sonarr/Radarr chatbot; LSIO image deprecated — entry pins the maintained fork; config path /config per fork README"

- id: tdarr
  name: Tdarr
  category: media
  retired: false
  image: ghcr.io/haveagitgat/tdarr:latest
  ports: ["8265:8265/tcp", "8266:8266/tcp"]
  volumes: ["./config/tdarr/server:/app/server", "./config/tdarr/configs:/app/configs", "./config/tdarr/logs:/app/logs", "./media:/media"]
  env:
    - {key: TZ, default: "${TZ}"}
    - {key: serverIP, default: "0.0.0.0"}
    - {key: serverPort, default: "8266"}
    - {key: webUIPort, default: "8265"}
  devices: ["/dev/dri:/dev/dri"]
  mem_limit: 2048m
  docs: https://docs.tdarr.io/docs/installation/docker/run-compose/
  notes: "transcode/remux automation; /dev/dri for VAAPI (device justification: hardware transcode); node port no longer published per upstream docs; CPU-heavy — keep mem cap 2g+"

- id: ersatztv
  name: ErsatzTV
  category: media
  retired: false
  image: ghcr.io/ersatztv/ersatztv:latest
  ports: ["8409:8409/tcp"]
  volumes: ["./config/ersatztv:/root/.local/share/ersatztv", "./media:/media:ro"]
  env:
    - {key: TZ, default: "${TZ}"}
  devices: ["/dev/dri:/dev/dri"]
  mem_limit: 1024m
  docs: https://ersatztv.org/docs/installation/docker/
  notes: "custom live-TV channels for Plex; data dir varies by image variant — verify at install; vaapi/nvidia variant images exist; /dev/dri for on-the-fly transcode"

- id: nzbhydra2
  name: NZBHydra 2
  category: media
  retired: false
  image: theotherp/nzbhydra2:latest
  ports: ["5076:5076/tcp"]
  volumes: ["./config/nzbhydra2:/config"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  docs: https://github.com/theotherp/nzbhydra2
  notes: "indexer meta-search; complements Prowlarr (either can feed the arrs)"

- id: ombi
  name: Ombi
  category: media
  retired: false
  image: ombi/ombi:latest
  ports: ["3579:3579/tcp"]
  volumes: ["./config/ombi:/config"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  docs: https://github.com/Ombi-app/Ombi/wiki
  notes: "upstream largely dormant — the stack's Seerr (an Overseerr fork) is preferred; entry exists for completeness"

- id: slskd
  name: slskd
  category: media
  retired: false
  image: ghcr.io/slskd/slskd:latest
  ports: ["5030:5030/tcp"]
  volumes: ["./config/slskd:/app", "./media/music:/downloads"]
  env:
    - {key: TZ, default: "${TZ}"}
    - {key: SLSKD_PASSWORD, secret: true, description: "web UI admin password"}
  mem_limit: 512m
  docs: https://github.com/slskd/slskd
  notes: "Soulseek client — music acquisition complement to Navidrome/Beets; optional listen-port range 50000-50300 improves download performance (map + ufw)"

- id: metube
  name: MeTube
  category: media
  retired: false
  image: alexta69/metube:latest
  ports: ["8881:8081/tcp"]
  volumes: ["./media/youtube:/downloads", "./config/metube:/config"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 256m
  docs: https://github.com/alexta69/metube
  notes: "yt-dlp web UI; host port shifted from upstream 8081 (catalog collision); downloads land in media/youtube"

- id: unmanic
  name: Unmanic
  category: media
  retired: false
  image: josh5/unmanic:latest
  ports: ["8889:8888/tcp"]
  volumes: ["./config/unmanic:/config", "./media:/library"]
  env:
    - {key: TZ, default: "${TZ}"}
  devices: ["/dev/dri:/dev/dri"]
  mem_limit: 1024m
  docs: https://docs.unmanic.io/
  notes: "library re-encode/remux automation — CAREFUL: give it a narrow library path, it mutates files; host port shifted from 8888 (scrutiny has it)"

- id: beets
  name: Beets
  category: media
  retired: false
  image: lscr.io/linuxserver/beets:latest
  ports: ["8337:8337/tcp"]
  volumes: ["./config/beets:/config", "./media/music:/music"]
  env:
    - {key: PUID, default: "1000"}
    - {key: PGID, default: "1000"}
    - {key: TZ, default: "${TZ}"}
  mem_limit: 256m
  docs: https://docs.linuxserver.io/images/docker-beets/
  notes: "music library tagger — pairs with slskd (acquire) and navidrome (serve)"
```

### Ops & monitoring (5)

```yaml
- id: socket-proxy
  name: Docker Socket Proxy
  category: ops
  retired: false
  image: lscr.io/linuxserver/socket-proxy:latest
  volumes: ["/var/run/docker.sock:/var/run/docker.sock:ro"]
  env:
    - {key: CONTAINERS, default: "1"}
    - {key: IMAGES, default: "1"}
    - {key: EVENTS, default: "1"}
    - {key: NETWORKS, default: "1"}
    - {key: INFO, default: "1"}
    - {key: POST, default: "0"}
    - {key: EXEC, default: "0"}
    - {key: VOLUMES, default: "0"}
  mem_limit: 64m
  healthcheck: "curl -sf http://localhost:2375/_ping || exit 1"
  docs: https://docs.linuxserver.io/images/docker-socket-proxy/
  notes: "least-privilege Docker API gateway (drop-in for tecnativa/docker-socket-proxy); consumers reach it at tcp://socket-proxy:2375 on bearcave; no published host port by design; enable only the API groups consumers need; NOTE: read-only socket mounts do not actually restrict the API — a proxy is the real mitigation"

- id: kopia
  name: Kopia
  category: ops
  retired: false
  image: ghcr.io/kopia/kopia:latest
  ports: ["51515:51515/tcp"]
  volumes: ["./config/kopia:/app/config", "./config:/config:ro", "./media:/media:ro"]
  env:
    - {key: KOPIA_PASSWORD, secret: true, description: "repository password"}
  mem_limit: 256m
  docs: https://github.com/kopia/kopia
  notes: "encrypted snapshots of config/ + media/ metadata; server mode args go in a compose_fragment (CLI-driven, not env-driven); pairs with the stack's Dropbox backup"

- id: changedetection
  name: Changedetection.io
  category: ops
  retired: false
  image: dgtlmoon/changedetection.io:latest
  ports: ["5002:5000/tcp"]
  volumes: ["./config/changedetection:/datastore"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 256m
  docs: https://github.com/dgtlmoon/changedetection.io
  notes: "website-change monitoring; host port shifted from 5000 (kavita has it); set PLAYWRIGHT_DRIVER_URL to the seeded flaresolverr for JS-heavy sites"
  dependencies: [flaresolverr]

- id: speedtest-tracker
  name: Speedtest Tracker
  category: ops
  retired: false
  image: alexjustesen/speedtest-tracker:latest
  ports: ["8765:80/tcp"]
  volumes: ["./config/speedtest:/config"]
  env:
    - {key: PUID, default: "1000"}
    - {key: PGID, default: "1000"}
    - {key: TZ, default: "${TZ}"}
    - {key: SPEEDTEST_SCHEDULE, default: "30 * * * *"}
    - {key: APP_KEY, secret: true, description: "Laravel app key — generate at install"}
  mem_limit: 256m
  docs: https://github.com/alexjustesen/speedtest-tracker
  notes: "sqlite builtin (MariaDB optional); APP_KEY must be set before first boot and kept stable"

- id: beszel
  name: Beszel
  category: ops
  retired: false
  image: henrygd/beszel:latest
  ports: ["8091:8090/tcp"]
  volumes: ["./config/beszel:/beszel_data"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 256m
  docs: https://github.com/henrygd/beszel
  notes: "lightweight server monitoring hub (sqlite builtin); the beszel-agent companion runs outside compose on monitored hosts — deploy per upstream docs"
```

### Network & security (4)

```yaml
- id: technitium
  name: Technitium DNS Server
  category: network
  retired: false
  image: technitium/dns-server:latest
  ports: ["5380:5380/tcp"]
  volumes: ["./config/technitium:/etc/dns"]
  env:
    - {key: DNS_SERVER_DOMAIN, default: "cave.lan"}
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  healthcheck: "curl -sf http://localhost:5380/api/user/session || exit 1"
  docs: https://hub.docker.com/r/technitium/dns-server
  notes: "authoritative+recursive DNS w/ web console on 5380; map host 53/tcp+udp manually only if the host resolver (systemd-resolved stub) does not claim it; upstream recommends host networking when serving DNS — convert via compose_fragment at install"

- id: tailscale
  name: Tailscale
  category: network
  retired: false
  image: ghcr.io/tailscale/tailscale:latest
  volumes: ["./config/tailscale:/var/lib/tailscale", "/dev/net/tun:/dev/net/tun"]
  env:
    - {key: TS_AUTHKEY, secret: true, description: "auth key from tailnet admin (or login-server URL for headscale)"}
    - {key: TS_HOSTNAME, default: "cave"}
    - {key: TS_EXTRA_ARGS, default: "--advertise-exit-node"}
  capabilities: [NET_ADMIN]
  mem_limit: 128m
  docs: https://tailscale.com/kb/1174/deploy-clients
  notes: "subnet-router/exit-node into the LAN; /dev/net/tun + NET_ADMIN are inherent to WireGuard; pair with headscale via TS_EXTRA_ARGS --login-server for self-hosted control"

- id: unbound
  name: Unbound
  category: network
  retired: false
  image: klutchell/unbound:latest
  ports: ["5353:53/tcp", "5353:53/udp"]
  volumes: ["./config/unbound:/opt/unbound/etc/unbound"]
  env:
    - {key: TZ, default: "${TZ}"}
  mem_limit: 128m
  healthcheck: "drill @localhost localhost 2>/dev/null || dig @localhost localhost || exit 1"
  docs: https://github.com/klutchell/unbound
  notes: "recursive validating DNS resolver; host 5353 avoids the host resolver stub; point pihole/technitium upstream at unbound:5353 over the bearcave network"

- id: headscale
  name: Headscale
  category: network
  retired: false
  image: ghcr.io/headscale/headscale:latest
  ports: ["8086:8080/tcp"]
  volumes: ["./config/headscale:/etc/headscale"]
  mem_limit: 128m
  compose_fragment: |
    headscale:
      <<: *common
      command: serve
  docs: https://github.com/juanfont/headscale
  notes: "self-hosted Tailscale control plane; config.yaml is authored at install (server_url must be reachable by clients); ships compose_fragment for the command override — the only seed entry that needs one"
```

### Productivity & home (4)

```yaml
- id: nextcloud
  name: Nextcloud
  category: home
  retired: false
  image: nextcloud:apache
  ports: ["8880:80/tcp"]
  volumes: ["./config/nextcloud:/var/www/html"]
  env:
    - {key: TZ, default: "${TZ}"}
    - {key: NEXTCLOUD_ADMIN_USER, default: "bear"}
    - {key: NEXTCLOUD_ADMIN_PASSWORD, secret: true, description: "admin password — set once at first boot"}
  mem_limit: 1024m
  healthcheck: "curl -sf http://localhost/status.php || exit 1"
  docs: https://github.com/nextcloud/docker
  notes: "all-in-one image w/ sqlite default — fine for small installs; move to MariaDB (catalog extension) beyond; host port shifted from 80"

- id: gitea
  name: Gitea
  category: home
  retired: false
  image: gitea/gitea:latest
  ports: ["3080:3000/tcp", "3022:22/tcp"]
  volumes: ["./config/gitea:/data"]
  env:
    - {key: USER_UID, default: "1000"}
    - {key: USER_GID, default: "1000"}
    - {key: GITEA__database__DB_TYPE, default: "sqlite3"}
    - {key: TZ, default: "${TZ}"}
  mem_limit: 512m
  healthcheck: "curl -sf http://localhost:3000/api/healthz || exit 1"
  docs: https://docs.gitea.com/installation/install-with-docker
  notes: "self-hosted git; host 3080 avoids nzbdav's 3000; ssh on 3022 — disable SSH server feature if unused"

- id: mealie
  name: Mealie
  category: home
  retired: false
  image: ghcr.io/mealie-recipes/mealie:latest
  ports: ["9925:9000/tcp"]
  volumes: ["./config/mealie:/app/data"]
  env:
    - {key: TZ, default: "${TZ}"}
    - {key: MEALIE_DEFAULT_GROUP, default: "Home"}
  mem_limit: 512m
  healthcheck: "wget -qO- http://localhost:9000/ >/dev/null 2>&1 || exit 1"
  docs: https://docs.mealie.io/documentation/getting-started/installation/installation-checklist/
  notes: "recipe manager; container port 9000, host 9925 per upstream docs convention; sqlite storage is the default"

- id: linkding
  name: Linkding
  category: home
  retired: false
  image: sissbruecker/linkding:latest
  ports: ["9090:9090/tcp"]
  volumes: ["./config/linkding:/etc/linkding"]
  env:
    - {key: LD_SUPERUSER_NAME, default: "bear"}
    - {key: LD_SUPERUSER_PASSWORD, secret: true, description: "initial admin password"}
    - {key: TZ, default: "${TZ}"}
  mem_limit: 256m
  healthcheck: "wget -qO- http://localhost:9090/health >/dev/null 2>&1 || exit 1"
  docs: https://github.com/sissbruecker/linkding
  notes: "bookmark manager; sqlite builtin; host 9090 free (authelia maps host 9091)"
```

### Port budget, entries 27–50

6969, 6291, 4545, 8265, 8266, 8409, 5076, 3579, 5030, 8881, 8889, 8337, 51515, 5002, 8765, 8091, 5380, 5353 (tcp+udp), 8086, 3080, 3022, 9925, 8880, 9090 — none collide with each other, the seed set (B.1), or the live stack (3000/5055/7878/8989/9696/32400/7780). Shifts from upstream defaults (8081→8881, 8888→8889, 5000→5002, 80→8765/8880, 9090→8091, 3000→3080, 8080→8086, 53→5353) are recorded per-entry in `notes`.

---

# Appendix C — Function→GUI parity table (authoritative)

This appendix enumerates **every operational function** being retired and maps it to the Cave Deck feature that replaces it. Feature IDs are dotted lowercase (`area.name`) and resolve to §6 sections. Sources:

| Source | Count | What it is |
|---|---|---|
| `thebearcave/services/bash-functions/functions/*.sh` | **105** | The active operational surface (`stack-*`) |
| `thebearcave/services/bash-functions/waybar/scripts/stack-tui-toggle.sh` | 1 | Waybar/TUI-internal helper (`stack_windows_json`) — retires with the TUI |
| Cave-Scripts submodule `cave-sys-*` / `cave-btrfs-*` / `cave-backup` (bash) | 32 | Host-layer operations (19 safe `sys` + 2 mutating `pkg` + 7 `btrfs` + 1 `backup` + 12 `de`) |
| Cave-Scripts fish `services/host-tools/functions/*.fish` | 23 | Legacy host mirrors of the `cave-sys-*` set + 2 fish-only extras |

**Reconciliation of the "~110 bash + 25 fish" estimate:** the live count is 105 `stack-*` functions (+1 waybar helper). The Cave-Scripts registry (`tests/live/safe_command_registry.yaml`) classifies **68 safe / 72 excluded command surfaces** across `cave-*` mirrors and host families; mirrors collapse onto the same operations as the `stack-*` table below. The fish layer is a legacy mirror of the host families plus two fish-only operations (`flatpak-updates`, `claude-home`), which are folded into the host table. **Unique in-scope operations: 140** (105 stack + 21 sys + 11 btrfs + 1 backup + 2 fish-only); the 12 `de` (desktop-environment) functions are explicitly retired without GUI equivalents.

**Safety-class legend** (from the live-harness registry where registered): `RO` = read-only; `MS` = mutating-safe; `EXC` = excluded from the automated live matrix (mutating / destructive-confirm / heavy) — every `EXC` row ships in the GUI behind a two-step confirm or as a job.

## C.1 Stack functions (`services/bash-functions/functions/`)

### `stack-core.sh` (6)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-status` | Container status overview | `dash.overview` | RO | |
| `stack-container` | Single-container restart/stop/start | `ctnr.lifecycle` | EXC | Registry: no read-only arg mode |
| `stack-restart-all` | Restart the whole stack | `ctnr.lifecycle` | EXC | Bulk action; compose-aware ordering (§6.2) |
| `stack-top` | Live docker stats | `dash.overview` | RO | |
| `stack-version` | Stack versions | `dash.overview` | RO | |
| `stack-help` | Function help index | `retire: UI-native` | — | Superseded by navigation; help text becomes tooltips |

### `stack-queue.sh` (2)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-queue-status` | Radarr/Sonarr queue w/ speed+ETA | `dash.rows` + `stick.queue` | RO | |
| `stack-arr-queue-errors` | Queue items with error messages | `stick.queue` | RO | |

### `stack-arr-1.sh` (7)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-arr-backlog` | Missing/cutoff backlog detail | `stick.backlog` | RO | |
| `stack-arr-recently-added` | Recently imported media | `dash.rows` | RO | |
| `stack-arr-toggle-search` | Toggle monitoring + trigger search | `stick.decide` | EXC | POSTs search commands |
| `stack-arr-blocklist` | View release blocklist | `lib.health` | RO | |
| `stack-arr-clear-blocklist` | Clear blocklist entries | `lib.health` | EXC | Confirm-gated |
| `stack-arr-missing-aired` | Aired-but-missing episodes | `stick.backlog` | RO | |
| `stack-cutoff-unmet` | Quality-cutoff unmet items | `stick.backlog` | RO | |

### `stack-arr-2.sh` (5)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-arr-import` | Trigger manual import scan | `stick.decide` | EXC | The import-decision core (§6.6) |
| `stack-arr-import-all` | Import scan across all items | `stick.decide` | EXC | Bulk path |
| `stack-arr-import-candidates` | List import candidates | `stick.queue` | RO | |
| `stack-arr-import-starvation` | Starvation-risk backlog view | `stick.backlog` | RO | |
| `stack-arr-logs` | *arr log viewer | `lib.health` | RO | |

### `stack-arr-3.sh` (8)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-arr` | Dispatcher: rss-sync / search-missing / unstick | `stick.decide` | EXC | All POST surfaces |
| `stack-backlog-status` | Compact backlog counts | `stick.backlog` | RO | |
| `stack-command-queue-summary` | *arr background command queue | `stick.queue` | RO | |
| `stack-import-lists` | Import-lists overview | `lib.lists` | RO | |
| `stack-radarr-health` | Radarr health-check report | `lib.health` | RO | |
| `stack-radarr-prune` | Radarr DB blob/history prune | `lib.prune` | EXC | Backup→prune→vacuum→verify job |
| `stack-sonarr-prune` | Sonarr DB blob/history prune | `lib.prune` | EXC | Same job shape |
| `stack-prowlarr-indexers` | Indexer status + query rates | `cred.indexers` | RO | |

### `stack-arrivals.sh` (2)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-arrival-notify` | Request→arrival Discord pings | `notif.discord` | EXC | Cursor state moves into SQLite (§5.4) |
| `stack-activity-feed` | Merged media activity feed | `dash.feed` | RO | Becomes a WebSocket topic |

### `stack-disk.sh` (5)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-disk-config-sizes` | du over live config trees | `host.disk` | EXC | Heavy I/O — scheduled/on-demand job |
| `stack-docker-disk-usage` | docker system df | `host.disk` | RO | |
| `stack-disk-reclaim` | Guarded prune/reclaim | `host.disk` | EXC | Destructive-confirm; nightly cron → internal scheduler |
| `stack-nzbdav-dedup-check` | Scan nzbdav history for dupes | `nzbd.dedup` | EXC | Heavy FUSE I/O — on-demand job |
| `stack-nzbdav-delete-failures` | Delete failed nzbdav downloads | `nzbd.deletefail` | EXC | Destructive-confirm |

### `stack-lists.sh` (10)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-mdblist-track` | Track an MDBList | `lib.lists` | EXC | Writes tracked-list state |
| `stack-mdblist-untrack` | Untrack an MDBList | `lib.lists` | EXC | |
| `stack-mdblist-tracked` | List tracked MDBLists | `lib.lists` | RO | |
| `stack-mdblist-import` | Import MDBList items into arrs | `lib.lists` | EXC | Import runs as a job with progress |
| `stack-mdblist-history` | Import history | `lib.lists` | RO | Backed by SQLite checkpoints |
| `stack-letterboxd-track` | Track a Letterboxd list | `lib.lists` | EXC | |
| `stack-letterboxd-untrack` | Untrack a Letterboxd list | `lib.lists` | EXC | |
| `stack-letterboxd-tracked` | List tracked Letterboxd lists | `lib.lists` | RO | |
| `stack-letterboxd-import` | Import Letterboxd items | `lib.lists` | EXC | Resume checkpoints persisted |
| `stack-letterboxd-history` | Import history | `lib.lists` | RO | |

### `stack-loop-ratings.sh` (6)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-loop-candidates` | Loop-detection candidate scan | `lib.lists` | EXC | Heavy arr-side scan — on-demand job |
| `stack-loop-exclude` | Write exclusion state | `lib.lists` | EXC | State moves into SQLite |
| `stack-loop-unmonitor` | Unmonitor looped items | `lib.lists` | EXC | |
| `stack-tmdb-missing` | TMDb missing lookup | `lib.lists` | EXC | External-API heavy, demo-only in harness |
| `stack-rating-imdb` | OMDb rating lookup | `lib.lists` | EXC | External API |
| `stack-rating-mdblist` | MDBList rating lookup | `lib.lists` | EXC | Third-party POST |

### `stack-maintenance.sh` (3)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-maintenance-digest` | Maintenance verification digest | `notif.digest` | EXC | Runs timers' surfaces end-to-end — becomes the scheduled digest job |
| `stack-audit-residue` | Retired-service residue scan | `host.residue` | RO | |
| `stack-config-drift` | Config/secret/mount drift | `host.drift` | RO | |

### `stack-misc.sh` (9)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-seerr-requests` | Open Seerr requests | `dash.rows` | RO | |
| `stack-notify-test` | Test notification | `notif.test` | EXC | Sends Discord/swaync pings |
| `stack-claude-full-backup` | Agent-state backup | `host.backup` | EXC | Backup-archive job |
| `stack-worktree` | Worktree create/status | `host.git` | EXC | Repo mutation |
| `stack-image-check` | Image update check | `ctnr.images` | RO | |
| `stack-perms-check` | Filesystem perms check | `host.perms` | RO | |
| `stack-oom-check` | OOM-kill scan | `host.mem` | RO | |
| `stack-resource-check` | Host resource check | `host.mem` | RO | |
| `stack-log-levels` | *arr log-level report | `lib.health` | RO | |

### `stack-nzbdav.sh` (4)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-nzbdav-queue` | nzbdav queue view | `nzbd.queue` | RO | |
| `stack-nzbdav-history` | nzbdav history | `nzbd.history` | RO | |
| `stack-nzbdav-stats` | nzbdav stats | `nzbd.stats` | RO | |
| `stack-mount-health` | FUSE mount health | `nzbd.mount` | RO | Also a dashboard row + guard input |

### `stack-plex-core.sh` (6)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-plex-sessions` | Live sessions | `plex.sessions` | RO | |
| `stack-plex-recently-added` | Recently added media | `dash.rows` | RO | |
| `stack-plex-libraries` | Libraries list | `plex.libraries` | RO | |
| `stack-plex` | Dispatcher: refresh/scan/empty-trash/analyze | `plex.maintenance` | EXC | All library mutations; empty-trash is landmine-#2 gated |
| `stack-plex-butler` | Trigger one Butler task | `plex.butler` | EXC | |
| `stack-plex-butler-all` | Full Butler sweep | `plex.butler` | EXC | |

### `stack-plex-extra.sh` (21)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-plex-duplicates` | Duplicate-asset report | `plex.libraries` | RO | |
| `stack-plex-garbage-collect-media` | Orphaned metadata removal | `plex.cleanup` | EXC | Destructive-confirm |
| `stack-plex-garbage-collect-blobs` | Orphaned artwork removal | `plex.cleanup` | EXC | Destructive-confirm |
| `stack-plex-backup-database` | DB backup archive | `plex.backup` | EXC | Backup job |
| `stack-plex-automatic-updates` | Auto-update toggle | `plex.maintenance` | EXC | Publisher-setting toggle |
| `stack-plex-process-assets` | Asset processing pass | `plex.maintenance` | EXC | |
| `stack-plex-refresh-epg` | EPG refresh | `plex.maintenance` | EXC | |
| `stack-plex-refresh-local-media` | Local media refresh | `plex.maintenance` | EXC | |
| `stack-plex-clean-cache-files` | Cache-file cleanup | `plex.cleanup` | EXC | Destructive-confirm |
| `stack-plex-clean-log-files` | Log-file cleanup | `plex.cleanup` | EXC | Destructive-confirm |
| `stack-plex-image-clean` | PhotoTranscoder cleanup | `plex.cleanup` | EXC | Idle-gated (landmine #10 behavior) |
| `stack-plex-deep-media-analysis` | Deep analysis queue | `plex.analysis` | EXC | Heavy |
| `stack-plex-upgrade-media-analysis` | Upgrade-priority analysis | `plex.analysis` | EXC | Heavy |
| `stack-plex-music-analysis` | Music analysis | `plex.analysis` | EXC | Heavy |
| `stack-plex-loudness-analysis` | Loudness analysis | `plex.analysis` | EXC | Heavy |
| `stack-plex-generate-media-index` | Media index generation | `plex.analysis` | EXC | |
| `stack-plex-generate-voice-activity` | Voice-activity markers | `plex.markers` | EXC | |
| `stack-plex-generate-intro-markers` | Intro markers | `plex.markers` | EXC | |
| `stack-plex-generate-credits-markers` | Credits markers | `plex.markers` | EXC | |
| `stack-plex-generate-ad-markers` | Ad markers | `plex.markers` | EXC | |
| `stack-plex-generate-chapter-thumbs` | Chapter thumbnails | `plex.markers` | EXC | |

### `stack-plex-markers.sh` (1)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-plex-markers` | Marker report/view | `plex.markers` | RO | |

### `stack-plex-updates.sh` (6)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-plex-updates` | Plex update check/report | `plex.maintenance` | RO | |
| `stack-plex-analyze` | Queue library analysis | `plex.analysis` | EXC | CPU-heavy |
| `stack-plex-empty-trash` | Empty Plex trash | `plex.maintenance` | EXC | Landmine #2: mount-verified first |
| `stack-plex-refresh-libraries` | Library metadata refresh | `plex.libraries` | EXC | |
| `stack-queue-autofix` | Auto-fix stuck queue items | `stick.queue` | EXC | The manual-run becomes the decision UI's bulk path |
| `stack-sonarr-fix-episode-monitoring` | Repair episode monitoring state | `lib.health` | EXC | PUT to Sonarr |

### `stack-watchable.sh` (4)

| Function | Purpose | GUI feature ID | Class | Notes |
|---|---|---|---|---|
| `stack-watchable` | "What's watchable tonight" | `lib.watchable` | RO | |
| `stack-unwatched` | Unwatched library view | `lib.watchable` | RO | |
| `stack-recent` | Recent watchable additions | `lib.watchable` | RO | |
| `stack-requests` | Seerr request cross-reference | `lib.watchable` | RO | |

**C.1 subtotal: 105** (104 mapped to feature IDs, `stack-help` retired as UI-native).

## C.2 Host layer (Cave-Scripts `cave-*` bash + fish mirrors)

One row per operation; the fish file that mirrored it is noted. `de` family = desktop-environment control (Hyprland/waybar/swaync session tools).

| Operation | bash | fish | GUI feature ID | Class | Notes |
|---|---|---|---|---|---|
| Disk free | `cave-sys-disk-free` | `stack-disk-free.fish` | `host.disk` | RO | |
| Memory pressure | `cave-sys-mem-pressure` | `stack-mem-pressure.fish` | `host.mem` | RO | |
| Journal errors | `cave-sys-journal-errors` | `stack-journal-errors.fish` | `host.journal` | RO | |
| Journal size | `cave-sys-journal-size` | `stack-journal-size.fish` | `host.journal` | RO | |
| Uptime report | `cave-sys-uptime-report` | `stack-uptime-report.fish` | `host.uptime` | RO | |
| Failed units | `cave-sys-service-failed` | `stack-service-failed.fish` | `host.services` | RO | |
| Zombie processes | `cave-sys-zombie-check` | `stack-zombie-check.fish` | `host.services` | RO | |
| Timer status | `cave-sys-timer-status` | `stack-timer-status.fish` | `host.cron` | RO | |
| Cron listing | `cave-sys-cron-list` | `stack-cron-list.fish` | `host.cron` | RO | |
| Pending kernel | `cave-sys-kernel-check` | `stack-kernel-check.fish` | `host.reboot` | RO | |
| Reboot required | `cave-sys-reboot-check` | `stack-reboot-check.fish` | `host.reboot` | RO | |
| Package updates | `cave-sys-pkg-updates` | `stack-pkg-updates.fish` | `host.pkg` | RO | |
| Package orphans | `cave-sys-pkg-orphans` | `stack-pkg-orphans.fish` | `host.pkg` | RO | |
| Package history | `cave-sys-pkg-history` | `stack-pkg-history.fish` | `host.pkg` | RO | |
| Install updates | `cave-sys-pkg-update` | `stack-pkg-update.fish` | `host.pkg` | EXC | `--yes` bypasses confirm — GUI keeps the confirm |
| Clean package cache | `cave-sys-pkg-clean-cache` | `stack-pkg-clean-cache.fish` | `host.pkg` | EXC | Destructive-confirm |
| Flatpak updates | — | `stack-flatpak-updates.fish` | `host.pkg` | RO | Fish-only; joins the pkg view |
| AUR audit | `cave-sys-aur-audit` | `stack-aur-audit.fish` | `host.aur` | RO | |
| Git status across repos | `cave-sys-git-status-all` | `stack-git-status-all.fish` | `host.git` | RO | |
| Firewall status | `cave-sys-firewall-status` | `stack-firewall-status.fish` | `host.firewall` | RO | |
| SSH doctor | `cave-sys-ssh-doctor` | `stack-ssh-doctor.fish` | `host.ssh` | RO | |
| SMART/btrfs disk health | `cave-sys-disk-health` | `stack-disk-health.fish` | `host.smart` | RO | |
| Agent-home backup | — | `stack-claude-home.fish` | `host.backup` | RO | Fish-only; folds into the backup job |
| btrfs usage | `cave-btrfs-usage` | — | `host.btrfs` | RO | |
| btrfs subvolume list | `cave-btrfs-subvolumes` | — | `host.btrfs` | RO | |
| btrfs device stats | `cave-btrfs-device-stats` | — | `host.btrfs` | RO | |
| btrfs scrub status | `cave-btrfs-scrub-status` | — | `host.btrfs` | RO | |
| btrfs balance status | `cave-btrfs-balance-status` | — | `host.btrfs` | RO | |
| btrfs qgroup show | `cave-btrfs-qgroup` | — | `host.btrfs` | RO | |
| snapper list | `cave-btrfs-snapper` | — | `host.btrfs` | RO | |
| btrfs snapshot create/delete | `cave-btrfs-snapshot` | — | `host.btrfs` | EXC | Mutates snapshot trees |
| btrfs subvolume create | `cave-btrfs-subvol` | — | `host.btrfs` | EXC | |
| btrfs scrub start | `cave-btrfs-scrub` | — | `host.btrfs` | EXC | Hours of IO — job w/ confirm |
| btrfs balance start | `cave-btrfs-balance` | — | `host.btrfs` | EXC | Same |
| Dropbox backup | `cave-backup` | — | `host.backup` | MS | Becomes the backup job family (§6.9) |
| Lock session | `cave-lock` | — | `retire: DE-layer` | — | Out of scope |
| Power menu | `cave-power` | — | `retire: DE-layer` | — | Never automated |
| Idle/presentation toggle | `cave-idle-toggle` | — | `retire: DE-layer` | — | |
| Theme cycle | `cave-theme` | — | `retire: DE-layer` | — | Superseded by Cave Deck's own theming (§5.5) |
| Nightlight toggle | `cave-nightlight` | — | `retire: DE-layer` | — | |
| Screen recording | `cave-record` | — | `retire: DE-layer` | — | |
| Clipboard history | `cave-cliphist` | — | `retire: DE-layer` | — | |
| Audio control | `cave-audio` | — | `retire: DE-layer` | — | |
| Media transport | `cave-media` | — | `retire: DE-layer` | — | |
| Waybar reload | `cave-bar` | — | `retire: DE-layer` | — | Waybar retires (§2.2) |
| Notification center | `cave-notify` | — | `retire: DE-layer` | — | Swaync retires |
| Dotfiles sync | `cave-sync` | — | `retire: DE-layer` | — | Rewrites live config trees |

## C.3 Internal helpers (no GUI surface — absorbed into the backend)

`__stack_curl`, `__arr_api`, `__arr_api_url`, `__arr_api_key`, `__stack_arr_app`, `__plex_api`, `__plex_butler`, `__seerr_api`, `__nzbdav_api`, `__stack_containers`, `__stack_metadata` (bash), `__host_helper`, `__host_containers` (fish), `stack_windows_json` (waybar), `fmt_*` formatters, `gen-bash-completions.sh`, `stack-completions.sh`, `__bearcave_warn_stale_keys`. These are transport/format plumbing: their behavior becomes the Rust backend's HTTP/WS handlers and shared clients (§4.1), not UI features.

## C.4 Enforcement rule

The cave-deck repo exports this appendix as `parity/parity.yaml` (function → feature ID → safety class). CI fails when: (a) a row's feature ID has no implemented route/page, (b) an `EXC` row's GUI path lacks its two-step confirm/job wrapper, or (c) a row disappears without a matching removal note. thebearcave's `audit_residue.py` continues to fail on any surviving `stack-*` reference after the removal PR (§9 M9).

---

# Appendix D — Rust backend API surface (REST + WebSocket)

Normative contract for the axum backend (§3.2). Generated from and cross-checked against Appendix C's feature IDs: **every route is tagged with the feature IDs it serves, and every feature ID must be served by at least one route** (D.4) — CI in the cave-deck repo enforces both directions against `parity/parity.yaml`.

## D.1 Conventions

- Base URL `http://<host-ip>:7780/api/v1/`; JSON bodies; no auth (LAN trust, §3.4).
- **Errors:** non-2xx ⇒ `{"error": {"code": "queue_not_empty", "message": "…", "detail": {…}}}` with stable snake_case codes; never HTML.
- **Pagination:** list routes accept `?page&pageSize` (default 50, max 500) ⇒ `{"data": [...], "page": 1, "pageSize": 50, "total": 1234}`.
- **Mutations are jobs:** destructive/mutating POSTs return `202 {"jobId": "…"}` and progress over WS topic `jobs` (§5.2). Only trivial state flips return `204`.
- **Confirm tokens:** every `EXC`-class operation (Appendix C safety) **requires** `"confirm": "<target-id>"` in the body, matching what the UI's two-step dialog shows. Mismatch ⇒ `422 code=confirm_required`. This ports the live-harness registry's exclusion semantics into the API layer.
- **Audit:** middleware records `(actor_ip, action, target, params, job_id)` for every POST/PUT/DELETE before dispatch (§5.4 `audit`).
- **OpenAPI:** utoipa serves `/api/docs` (UI) and `/api/v1/openapi.json`; the frontend's typed client is generated from it (§3.3).

## D.2 REST routes

### System & dashboard

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/healthz` | GET | — | compose healthcheck target |
| `/api/v1/readyz` | GET | — | deps reachable (docker sock, arrs, plex, nzbdav, sqlite) |
| `/api/v1/version` | GET | `dash.overview` | backend + stack component versions |
| `/api/v1/dashboard` | GET | `dash.overview` `dash.rows` | one snapshot: containers, mount, queues, disk, arrivals, requests |
| `/api/v1/activity` | GET | `dash.feed` | persisted events (§5.4 `events`); filters `?kind&severity&since&source` |

### Containers (ctnr)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/containers` | GET | `ctnr.lifecycle` | incl. compose service grouping + dependency order |
| `/api/v1/containers/{id}` | GET | `ctnr.lifecycle` | detail: state, health, image, mounts, restarts |
| `/api/v1/containers/{id}/start` | POST | `ctnr.lifecycle` | `202` job |
| `/api/v1/containers/{id}/stop` | POST | `ctnr.lifecycle` | `202` job; EXC confirm |
| `/api/v1/containers/{id}/restart` | POST | `ctnr.lifecycle` | `202` job; EXC confirm; dependency-aware |
| `/api/v1/containers/{id}/recreate` | POST | `ctnr.lifecycle` | `202` job; compose-native; nzbdav ⇒ queue guard (§6.5) |
| `/api/v1/containers/{id}` | DELETE | `ctnr.lifecycle` | compose-is-truth: only via catalog uninstall flow (§6.3) |
| `/api/v1/containers/{id}/logs` | GET | `ctnr.lifecycle` | `?tail&level&since` snapshot; live tail is WS `logs.<container>` |
| `/api/v1/containers/{id}/stats` | GET | `dash.overview` | current + `?from&to` history (§5.4 `metrics`) |
| `/api/v1/images` | GET | `ctnr.images` | list + in-use state |
| `/api/v1/images/check` | POST | `ctnr.images` | upstream-tag comparison job (ports `stack-image-check`) |
| `/api/v1/images/{id}/pull` | POST | `ctnr.images` | `202` job w/ layer progress |
| `/api/v1/images/prune` | POST | `ctnr.images` | `202` job; EXC confirm (dangling only by default) |
| `/api/v1/compose` | GET | `ctnr.lifecycle` | rendered effective compose (read-only) |

### Stick — merged Sonarr/Radarr queue, backlog, decisions

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/stick/queue` | GET | `stick.queue` | merged+unified view w/ speeds/ETAs |
| `/api/v1/stick/queue/errors` | GET | `stick.queue` | importBlocked/importPending/warning w/ full status messages |
| `/api/v1/stick/queue/{itemId}/decide` | POST | `stick.decide` | body `{action: import\|override\|map\|delete\|delete+blocklist\|delete+re-search, mapping?}`; EXC confirm; the §6.6 decision core |
| `/api/v1/stick/queue/bulk` | POST | `stick.decide` | same action over selected items; EXC confirm |
| `/api/v1/stick/autofix` | POST | `stick.queue` | the `stack-queue-autofix` heuristic as a job; EXC confirm |
| `/api/v1/stick/backlog` | GET | `stick.backlog` | missing/cutoff/starvation views (`?view=missing\|aired\|cutoff\|starvation`) |
| `/api/v1/stick/search` | POST | `stick.backlog` `stick.decide` | scoped search (series/movie/episode/season); EXC confirm |
| `/api/v1/stick/monitoring` | POST | `stick.decide` | toggle + `fix-episode-monitoring` repair; EXC confirm |
| `/api/v1/stick/blocklist` | GET | `stick.decide` | list (ports `stack-arr-blocklist`) |
| `/api/v1/stick/blocklist/{id}` | DELETE | `stick.decide` | EXC confirm |
| `/api/v1/stick/history` | GET | `stick.queue` `stick.decide` | grab/import/failure history |

### Library & lists (lib)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/library/health` | GET | `lib.health` | arr health checks + log levels |
| `/api/v1/library/logs` | GET | `lib.health` | arr log viewer (`?app&level`) |
| `/api/v1/library/prune/{app}` | POST | `lib.prune` | backup→prune→vacuum→verify job (landmine #9); EXC confirm |
| `/api/v1/library/lists` | GET | `lib.lists` | tracked lists + import-list overview |
| `/api/v1/library/lists/{kind}` | POST/DELETE | `lib.lists` | track/untrack (`kind=mdblist\|letterboxd`) |
| `/api/v1/library/lists/{kind}/import` | POST | `lib.lists` | `202` resumable import job (§5.4 checkpoints) |
| `/api/v1/library/lists/{kind}/history` | GET | `lib.lists` | import history + checkpoints |
| `/api/v1/library/loop` | GET/POST | `lib.lists` | loop candidates / exclude / unmonitor (scan = job) |
| `/api/v1/library/ratings` | POST | `lib.lists` | IMDb/MDBList/TMDb lookups (external-API, rate-limited) |
| `/api/v1/library/watchable` | GET | `lib.watchable` | watchable/unwatched/recent + Seerr cross-ref (`?view=`) |
| `/api/v1/library/requests` | GET | `lib.watchable` `dash.rows` | Seerr requests |

### nzbdav & mount (nzbd)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/nzbdav/queue` | GET | `nzbd.queue` | SAB-style queue view |
| `/api/v1/nzbdav/queue/{id}` | DELETE | `nzbd.queue` | cancel/remove; EXC confirm |
| `/api/v1/nzbdav/history` | GET | `nzbd.history` | paginated |
| `/api/v1/nzbdav/stats` | GET | `nzbd.stats` | speeds, connections, provider health |
| `/api/v1/nzbdav/dedup-check` | POST | `nzbd.dedup` | heavy FUSE scan as job (§6.9) |
| `/api/v1/nzbdav/delete-failures` | POST | `nzbd.deletefail` | EXC confirm |
| `/api/v1/nzbdav/recreate` | POST | `nzbd.queue` | safe-recreate: triple health check after (landmine #13); refuses if pending>0 |
| `/api/v1/mount/health` | GET | `nzbd.mount` | mountpoint + VFS stats + error counters; also the guard input for §6.5/§6.8 |

### Plex (plex)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/plex/sessions` | GET | `plex.sessions` | live; also WS `plex.sessions` |
| `/api/v1/plex/libraries` | GET | `plex.libraries` | incl. duplicates report (`?view=duplicates`) |
| `/api/v1/plex/libraries/{id}/refresh` | POST | `plex.libraries` `plex.maintenance` | scan/refresh/update (EXC confirm) |
| `/api/v1/plex/empty-trash` | POST | `plex.maintenance` | **mount-health guard**: 409 `code=mount_unhealthy` if FUSE degraded (landmine #2); EXC confirm |
| `/api/v1/plex/butler/{task}` | POST | `plex.butler` | single task or `all`; `202` job |
| `/api/v1/plex/analysis/{kind}` | POST | `plex.analysis` | `kind=analyze\|deep\|upgrade\|music\|loudness\|media-index\|voice`; `202` job |
| `/api/v1/plex/markers` | GET/POST | `plex.markers` | report + generate (`kind=intro\|credits\|ad\|chapters`) |
| `/api/v1/plex/backup` | POST | `plex.backup` | DB backup job; EXC confirm |
| `/api/v1/plex/cleanup/{kind}` | POST | `plex.cleanup` | `kind=cache\|logs\|gc-media\|gc-blobs\|images`; images is idle-gated (landmine #10); EXC confirm |
| `/api/v1/plex/maintenance` | GET/POST | `plex.maintenance` | update check, auto-updates toggle, EPG/local-media refresh, assets |

### Credentials & API sources (cred)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/credentials` | GET | all cred | grouped, masked (`***) |
| `/api/v1/credentials/{key}/reveal` | POST | cred.* | returns plaintext once; audit-logged; UI auto-re-masks 15s |
| `/api/v1/credentials/{key}` | POST | cred.* | rotate/update → returns computed blast radius + requires apply flow (§6.5) |
| `/api/v1/credentials/{key}/blast-radius` | GET | cred.* | var→consumer map |
| `/api/v1/usenet/providers` | GET/POST/DELETE | cred.* | full CRUD on `NZBDAV_CONFIG__USENET__PROVIDERS` JSON (incl. enable/disable) |
| `/api/v1/usenet/providers/test` | POST | cred.* | DNS→TCP→TLS→AUTHINFO probe |
| `/api/v1/indexers` | GET | `cred.indexers` | Prowlarr indexers, status, query rates, VIP expiry fields |
| `/api/v1/indexers/{id}/test` | POST | `cred.indexers` | |
| `/api/v1/list-sources` | GET/PUT | `lib.lists` | metadata source switch (TMDb/MDBList/IMDb) + creds |
| `/api/v1/plex/token` | GET/POST | cred.* | token verify + `PLEX_CLAIM` first-run flow |

### `.env` engine (env)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/env` | GET | env | grouped view, masked, stale/`changeme` detection |
| `/api/v1/env/draft` | PUT | env | in-memory draft + validation parity w/ `setup.sh --validate-only` |
| `/api/v1/env/diff` | GET | env | draft vs current |
| `/api/v1/env/consumers/{var}` | GET | env | blast radius for one var |
| `/api/v1/env/apply` | POST | env | `202` job: write `.env` atomically → regenerate secrets.yml → dependency-ordered recreates → health verify; nzbdav recreate ⇒ queue guard; EXC confirm |
| `/api/v1/env/template-docs` | GET | env | inline docs per var (from `.env.template` comments) |

### Catalog & deployments (catalog)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/catalog` | GET | catalog | entries (§6.3 schema) + install-state per id |
| `/api/v1/catalog/{id}` | GET | catalog | entry detail + generated install form |
| `/api/v1/catalog/conflicts` | POST | catalog | live conflict check for a draft install (ports, volumes, mem, names) |
| `/api/v1/catalog/{id}/install` | POST | catalog | `202` job → PR flow (branch, fragment, commit, push, `gh pr create`, auto-merge) |
| `/api/v1/catalog/{id}/uninstall` | POST | catalog | reverse PR flow w/ data-retention choice; EXC confirm |
| `/api/v1/deployments` | GET | catalog | PR/install history + status (`deploy.status` WS) |
| `/api/v1/deployments/{id}/logs` | GET | catalog | PR flow step log |

### Jobs, metrics, settings, notifications

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/jobs` | GET | all | list w/ states; `?active=true` |
| `/api/v1/jobs/{id}` | GET | all | detail + progress + log tail |
| `/api/v1/jobs/{id}/cancel` | POST | all | cooperative cancel; EXC confirm |
| `/api/v1/metrics` | GET | `dash.overview` | `?container&from&to` from §5.4 `metrics` |
| `/api/v1/settings` | GET/PUT | — | theme, dashboard prefs, digest cadence |
| `/api/v1/notifications/discord` | GET/PUT | `notif.discord` | webhook config + event filter |
| `/api/v1/notifications/smtp` | GET/PUT | `notif.digest` | Gmail SMTP + App Password config |
| `/api/v1/notifications/digest` | GET/PUT | `notif.digest` | schedule + last digest preview |
| `/api/v1/notifications/test` | POST | `notif.test` | fires one test through configured channels |
| `/api/v1/notifications/history` | GET | `notif.discord` `notif.digest` | delivery log |

### Host (via shim)

| Route | Method | Feature IDs | Notes |
|---|---|---|---|
| `/api/v1/host/overview` | GET | `host.mem` `host.disk` | mem/load/disk snapshot (WS `system.host` mirrors) |
| `/api/v1/host/disk` | GET | `host.disk` | free, docker df, config sizes (sizes = job) |
| `/api/v1/host/reclaim` | POST | `host.disk` | guarded reclaim job; EXC confirm |
| `/api/v1/host/journal` | GET | `host.journal` | errors + size (`?unit&since`) |
| `/api/v1/host/services` | GET | `host.services` | failed units + zombies |
| `/api/v1/host/pkg` | GET | `host.pkg` | updates, orphans, history, flatpak, AUR audit |
| `/api/v1/host/pkg/update` | POST | `host.pkg` | shim job; EXC confirm (keeps the human confirm the registry flagged) |
| `/api/v1/host/pkg/clean-cache` | POST | `host.pkg` | EXC confirm |
| `/api/v1/host/btrfs` | GET | `host.btrfs` | usage/subvolumes/device-stats/scrub+balance status/qgroup/snapper (`?view=`) |
| `/api/v1/host/btrfs/{op}` | POST | `host.btrfs` | snapshot/subvol/scrub/balance jobs; EXC confirm |
| `/api/v1/host/smart` | GET | `host.smart` | SMART + disk health |
| `/api/v1/host/reboot` | GET | `host.reboot` | reboot-required + pending kernel |
| `/api/v1/host/cron` | GET | `host.cron` | crons + systemd timers |
| `/api/v1/host/git` | GET | `host.git` | multi-repo status + worktrees |
| `/api/v1/host/firewall` | GET | `host.firewall` | ufw status |
| `/api/v1/host/ssh` | GET | `host.ssh` | ssh doctor |
| `/api/v1/host/uptime` | GET | `host.uptime` | uptime report |
| `/api/v1/host/backup` | POST | `host.backup` | agent-state + Dropbox backup jobs; EXC confirm |
| `/api/v1/host/drift` | GET | `host.drift` | config/secret/mount drift |
| `/api/v1/host/residue` | GET | `host.residue` | retired-service residue scan |
| `/api/v1/host/health` | GET | `host.perms` | perms + OOM scan (`?view=perms\|oom`) |

## D.3 WebSocket endpoints

`ws://<host-ip>:7780/api/v1/ws` — single socket, multiplexed by topic. Client sends `{"sub":["topic",…]}` / `{"unsub":[…]}`; server answers each topic with a `snapshot` frame then deltas; heartbeat ping 10s; origin-checked (§3.4).

| Topic | Frame shape | Serves |
|---|---|---|
| `docker.stats` | 1s per-container deltas | `dash.overview` |
| `docker.events` | die/stop/oom/health_status | `ctnr.lifecycle` `dash.feed` |
| `queue.arr` | merged queue diffs | `stick.queue` `dash.rows` |
| `queue.nzbdav` | download queue + speeds | `nzbd.queue` |
| `mount.health` | 10s mountpoint + VFS | `nzbd.mount` |
| `plex.sessions` | on change | `plex.sessions` |
| `system.host` | 10s mem/disk/load | `host.mem` `host.disk` |
| `jobs` | state transitions + progress | all mutating flows |
| `logs.<container>` | live tail (`level=` filter param in sub) | `ctnr.lifecycle` `lib.health` |
| `deploy.status` | catalog PR/install step events | catalog |

## D.4 Feature-ID coverage matrix

Every Appendix C feature ID must appear here. (Compact form: `ID → primary routes → WS topics`.) CI asserts both directions: matrix ⊆ routes, and Appendix C's mapped IDs ⊆ matrix. `retire:` rows (`UI-native`, `DE-layer`) assert **no** routes on purpose — a route serving them is a CI error.

| Feature ID | Routes | WS topics |
|---|---|---|
| `dash.overview` | dashboard, version, containers/{id}/stats, metrics | docker.stats, docker.events, system.host |
| `dash.rows` | dashboard, library/requests | queue.arr |
| `dash.feed` | activity | docker.events |
| `ctnr.lifecycle` | containers* | docker.events, jobs, logs.* |
| `ctnr.images` | images* | jobs |
| `stick.queue` | stick/queue*, stick/autofix, stick/history | queue.arr |
| `stick.backlog` | stick/backlog, stick/search | jobs |
| `stick.decide` | stick/queue/{id}/decide, bulk, monitoring, blocklist, history | jobs |
| `lib.health` | library/health, library/logs, host/health | logs.* |
| `lib.lists` | library/lists*, library/loop, library/ratings, list-sources | jobs |
| `lib.prune` | library/prune/{app} | jobs |
| `lib.watchable` | library/watchable, library/requests | — |
| `nzbd.queue` | nzbdav/queue*, nzbdav/recreate | queue.nzbdav |
| `nzbd.history` | nzbdav/history | — |
| `nzbd.stats` | nzbdav/stats | queue.nzbdav |
| `nzbd.mount` | mount/health | mount.health |
| `nzbd.dedup` | nzbdav/dedup-check | jobs |
| `nzbd.deletefail` | nzbdav/delete-failures | jobs |
| `plex.sessions` | plex/sessions | plex.sessions |
| `plex.libraries` | plex/libraries* | — |
| `plex.maintenance` | plex/empty-trash, plex/maintenance, plex/libraries/{id}/refresh | jobs |
| `plex.butler` | plex/butler/{task} | jobs |
| `plex.analysis` | plex/analysis/{kind} | jobs |
| `plex.markers` | plex/markers | jobs |
| `plex.backup` | plex/backup | jobs |
| `plex.cleanup` | plex/cleanup/{kind} | jobs |
| `cred.indexers` | indexers* | — |
| `cred.*` (arr/seerr/plex/usenet keys) | credentials*, usenet/providers*, plex/token | jobs |
| env (§6.5) | env* | jobs |
| catalog (§6.3) | catalog*, deployments* | deploy.status, jobs |
| `host.disk` | host/disk, host/reclaim, host/overview | system.host |
| `host.mem` | host/overview, host/health | system.host |
| `host.journal` | host/journal | — |
| `host.services` | host/services | — |
| `host.pkg` | host/pkg* | jobs |
| `host.aur` | host/pkg | — |
| `host.btrfs` | host/btrfs* | jobs |
| `host.smart` | host/smart | — |
| `host.reboot` | host/reboot | — |
| `host.cron` | host/cron | — |
| `host.git` | host/git | — |
| `host.firewall` | host/firewall | — |
| `host.ssh` | host/ssh | — |
| `host.uptime` | host/uptime | — |
| `host.backup` | host/backup | jobs |
| `host.drift` | host/drift | — |
| `host.residue` | host/residue | — |
| `host.perms` | host/health | — |
| `notif.discord` | notifications/discord*, notifications/history | jobs |
| `notif.digest` | notifications/smtp, notifications/digest | jobs |
| `notif.test` | notifications/test | — |

## D.5 Shim command surface (host systemd helper)

The backend reaches the shim over a **localhost-only Unix socket** with a fixed, auditable command allowlist (§3.1) — no general shell. Each command maps 1:1 to a host route above: `journal.query`, `services.failed`, `pkg.{updates,orphans,history,update,clean,flatpak,aur}`, `btrfs.{usage,subvols,devstats,scrub.status,balance.status,qgroup,snapper,snapshot.create,snapshot.delete,subvol.create,scrub.start,balance.start}`, `smart.query`, `reboot.{required,kernel}`, `cron.list`, `git.status`, `ufw.status`, `ufw.allow`, `ssh.doctor`, `path.{read,list}` (constrained roots), `backup.{run,restore}`. Every invocation is logged to the audit trail; mutating shim commands additionally require the confirm-token header from D.1.

## D.6 API-layer enforcement (cave-deck CI)

1. `openapi` job: utoipa spec builds; generated TS client compiles (no drift).
2. `feature-coverage` job: parses Appendix C's `parity/parity.yaml` + the route table's feature-ID tags; fails on unmapped IDs, unserved IDs, or any route tagged with a `retire:` row.
3. `safety` job: every route whose feature-ID rows are `EXC`-class must declare `confirm` in its handler metadata and a `202`-job return type; destructive-confirm rows must additionally name the shim/job wrapper used.
4. `guard` job: the `plex/empty-trash`, `env/apply`, and `nzbdav/recreate` handlers must reference the landmine guards (#2, #4, #13) in their integration tests — the red-path tests from §7 are pinned to these routes.
