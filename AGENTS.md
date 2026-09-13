# AGENTS.md — RAWRZ

> **RAWRZ** — home media megastack: stack + deck + RPG. One repository, one release
> stream, one CI. See [`rawrz-megastack-spec.md`](rawrz-megastack-spec.md) for the
> master spec; the four companion plans are in `docs/plans/`.
>
> **This file is the authoritative agent contract.** When another doc disagrees, this
> file wins. It is written for AI coding agents and human contributors alike.

---

## What This Repo Is

A slim, robust media-acquisition-and-serving stack merged with a web GUI (Cave Deck)
and a movie/TV RPG, published on host ports with nginx internal TLS ingress and CI/CD
via GitHub Actions. Hosted on Linux.

| Component | Location | What it is |
|---|---|---|
| **Stack** (8 always-on + 2 manual) | `docker-compose.yml`, `docs/stack/`, `services/` | The Bear Cave media stack: Prowlarr, Radarr, Sonarr, NzbDAV, nzbdav_rclone, Seerr, Plex, Unpackerr + ImageMaid/Recyclarr (maintenance profile) |
| **Deck** | `backend/deck/`, `docs/deck/`, `catalog/` | Cave Deck — Rust/axum backend + React/TS/Vite frontend, 100-container catalog, port 7780, LAN-only |
| **RPG** | `backend/rpg/`, `docs/rpg/` | Movie/TV RPG — Rust/Axum + Svelte, watches as progression, port 46532 |

> **2026-09-06 re-retirement:** Bazarr was removed again (8-service target; subtitle
> acquisition did not justify an always-on container). See `docs/stack/services/lifecycle.md`.
>
> **2026-09-04 demotion:** Recyclarr moved from the always-on set to the manual
> `maintenance` profile. Invoke with
> `docker compose --profile maintenance run --rm recyclarr sync`.

---

## Architecture

```
Prowlarr (indexers) ──▶ Radarr + Sonarr ──▶ nzbdav (Usenet) ──▶ FUSE mount ──▶ Plex
       :9696              :7878 / :8989      :3000        (nzbdav_rclone)   (host network)
                              │            │
                          Seerr :5055
                              │
                        Unpackerr (post-download extraction)
```

### Port Map

```
3000  NzbDAV (WebDAV)
5055  Seerr (requests)
7878  Radarr
8989  Sonarr
9696  Prowlarr
32400 Plex (host network)
```

**API surfaces** — the full map lives in [`docs/stack/API.md`](docs/stack/API.md).
Update it when a script starts calling a new endpoint.

---

## Worktree Discipline — mandatory

From this point forward, **all edits happen on dedicated git worktrees** — one
worktree per task, named by the task, never mixed with unrelated work. This
rule applies to every future change, including the change that introduced it.

**Repository containment rule:** Every worktree for this repository must
live inside the RAWRZ checkout under `.worktrees/<task-name>`. Do not create or
retain worktrees under `/home/bear/.worktrees/`, `/home/bear/wt-*`, or any other
external path. Before editing, verify with `git worktree list --porcelain`; after
relocating or removing a worktree, run `git worktree prune` and verify again.
The main checkout remains reference-only and must stay free of task edits.

1. **One worktree per task.** Before making any edit, create a task-named
   worktree and branch off `origin/main`:
   `git worktree add <path> -b <task-branch> origin/main`.
2. **Never mix unrelated work.** A worktree contains exactly one task's
   changes and nothing else.
3. **The main checkout stays clean.** Use it for reference only (fetch/status/log).
4. **Deliver via PR.** `main` is branch-protected: push the task branch, open
   a PR (linear history; squash/rebase only), keep up to date with `origin/main`.
5. **Clean up.** After merge: `git worktree remove <path>`.
6. **Walkthrough:** [docs/stack/worktree-lifecycle.md](docs/stack/worktree-lifecycle.md).

---

## Source-Code Questions: grep `~/TRUTH` first — mandatory, effective 2026-09-05

`/home/bear/TRUTH` (outside this repo, do not commit it) holds shallow git
clones of the **actual upstream application source** for every container in the
stack — each of the 8 always-on services plus both maintenance-profile services
(ImageMaid, Recyclarr) — pinned to the exact version each image runs today.

When a question is about *how something in this stack behaves in code* — an API
endpoint or its parameters, a config/option's meaning, the origin of an error
string, a CLI flag's semantics, a DB schema field, a request/response shape —
answer it **first by local `rg`/`grep` over `~/TRUTH`**, never from memory,
from docs alone, or from a web search of upstream `main`.

Rules of the road:

1. **Grep the owning service's tree first.** `rg -n "<pattern>" ~/TRUTH/<dir>/`
   (add `-i` when unsure of case). For cross-cutting behaviour, grep sibling trees.
2. **App source, not packaging.** `~/TRUTH` holds the application code — the tree
   you actually want to grep. Do not chase packaging repos for app behaviour.
3. **Version skew is the first suspect when a search comes up empty.** Clones are
   pinned to the *running* versions. Before going to the web, confirm the clone's
   ref matches the running image. Only if the code truly is absent is a web/docs
   consult warranted — and say so in your answer.
4. **Keep the corpus pinned to the stack.** When `docker-compose.yml` bumps an
   image version, re-pin that service's clone and update `~/TRUTH/README.md`.
5. **Scope limit.** Plex's Media Server core is proprietary; `~/TRUTH/plex` is
   only the image wrapper. Grep it for container behaviour, never for PMS internals.

Service → source map (full table in `~/TRUTH/README.md`):

| Container / profile service | Grep dir in `~/TRUTH` | What it is |
|---|---|---|
| `prowlarr` | `~/TRUTH/prowlarr` | Prowlarr app source @ v2.5.2.5491 |
| `radarr` | `~/TRUTH/radarr` | Radarr app source @ v6.3.0.10514 |
| `sonarr` | `~/TRUTH/sonarr` | Sonarr app source @ v4.0.19.2979 |
| `nzbdav` | `~/TRUTH/infinidysk` | InfiniDysk source, `main` (rolling `dev` image) |
| `nzbdav_rclone` | `~/TRUTH/rclone` | rclone source @ v1.75.0 |
| `seerr` | `~/TRUTH/seerr` | Seerr source @ v3.4.1 |
| `plex` | `~/TRUTH/plex` | **Core closed source** — pms-docker wrapper only |
| `unpackerr` | `~/TRUTH/unpackerr` | Unpackerr source @ v0.16.1 |
| `imagemaid` (maintenance) | `~/TRUTH/imagemaid` | ImageMaid source, `master` (rolling `latest`) |
| `recyclarr` (maintenance) | `~/TRUTH/recyclarr` | Recyclarr source @ v8.7.2 |

---

## How to Work in This Repo

### Before Making Changes

1. Create the task-named worktree (see Worktree Discipline above)
2. Read `CLAUDE.md` for work style rules
3. Check `docker compose ps` for current state
4. Read `docs/` for service documentation
5. If the task needs to know how a service behaves in code, grep `~/TRUTH` first

### After Making Changes

1. Run validation: `docker compose config --quiet`
2. Run bash syntax checks: `bash -n scripts/*.sh tests/*/*.sh`
3. Run bash smoke tests: `bash tests/bash/test_bash_functions.sh --offline`
4. Use `./tests/integration/test_pipeline.sh --dry-run` for live infrastructure
   checks when the NzbDAV queue is non-empty.
5. Keep agent-facing operational output in English.
6. **Restart containers after editing bind-mounted files** — `sed -i`/`vim` on a
   bind-mounted file changes the inode; the container keeps serving the old content
   until restarted.

### Safety Rules

- Never commit `.env` or secrets
- Never run destructive operations without confirmation
- Always restart dependents after mount-owner changes
- Always confirm NzbDAV queue is empty before container operations
- Use `--force-recreate` when .env changes need to take effect
- Plex config directory contains the full library metadata — back up before changes
- ImageMaid is manual and profile-gated; run PhotoTranscoder-only cleanup only while
  Plex is idle.

---

## Documentation Map

| Doc | What it covers |
|-----|----------------|
| **`rawrz-megastack-spec.md`** | **Master spec** — RAWRZ design: single repo, one release, Redis (D7–D12), Postgres (§7), reverse proxy (D13), everything containerized (D34), RPG as Compose service (§10), Deck integration (§9) |
| [`docs/plans/`](docs/plans/) | Companion plans: M0 migration, Postgres hardening, *arr DB migration, Seerr cache patch |
| [`docs/stack/`](docs/stack/) | **Stack docs** — architecture, landmines, CI/CD, security, quick-start, testing, worktree lifecycle, MCP, stackarr eval, API map, per-service docs |
| [`docs/stack/services/`](docs/stack/services/) | Per-service docs + lifecycle (retired + re-adoption) |
| [`docs/deck/`](docs/deck/) | **Cave Deck spec** — web GUI design, catalog, features |
| [`docs/rpg/`](docs/rpg/) | **RPG spec** (`movie-rpg-spec.md`), changelog, contributing — **⚠ supersession banner applies** |
| [`docs/agents/`](docs/agents/) | **RPG operational docs** — FIX.md (code review, **⚠ §8 Redis superseded**), HANDOFF.md, CLAUDE.md |

---

## Critical Landmines (affect operations today)

See [`docs/stack/landmines.md`](docs/stack/landmines.md) for the full list and
[`docs/stack/AGENTS.md`](docs/stack/AGENTS.md) for the comprehensive version.

1. **Bind-mount file staleness** — `sed -i`/`vim` on a bind-mounted file changes
   the inode; the container keeps serving the old file until restarted. Always
   `docker compose restart <container>` after editing a bind-mounted file.
2. **FUSE mount fragility** — Mount-owner restart breaks all dependents. Never
   `sudo umount` a FUSE mountpoint. Restart the owner, then dependents in order.
   A stale mount is why Plex shows "red trash cans": verify the mount is healthy
   *before* rescanning.
3. **Plex `stop_grace_period: 90s` required** — Without it, Docker's 10s default
   SIGKILL fires mid-shutdown, producing a D-state hang.
4. **NzbDAV queue is not persistent** — Recreate wipes queued NZBs and silently
   blocklists affected items. Confirm pending is 0 before touching. The healthcheck
   must probe both the frontend (`:3000/healthz`) and an authenticated queue API.
5. **Plex on host network** — Cannot run on bridge without losing GDM/DLNA/remote
   access. Access at `http://HOST_IP:32400`.
6. **rclone.conf requires `rclone obscure`** — Passwords must be rclone-obfuscated.
7. **App removal checklists must be exhaustive** — Every removal touches: compose,
   config, env vars, Prowlarr sync, docs, tests. See
   [docs/stack/services/lifecycle.md](docs/stack/services/lifecycle.md).
8. **Radarr orphaned quality-profile references** — A movie row pointing at a deleted
   quality profile makes `/api/v3/movie` return 500 for the whole collection.
9. **SQLite DB bloat from MediaInfo blobs** — Radarr stores 10–300KB MediaInfo blobs
   per history row; long-lived instances grow a 1GB `radarr.db`. Prune periodically
   or raise the cap (now 1536m).
10. **ImageMaid path validation is behavioral** — Its `/plex` mount must target
    `config/plex/Plex Media Server`, not the parent `config/plex`.
11. **Profiled run services need generated names** — Do not assign `container_name`
    to the manual ImageMaid profile.
12. **Main may advance asynchronously** — Fetch `origin/main` and rebase before
    retrying a rejected push, never force-push.
13. **NzbDAV backend outages can be masked by the frontend** — Validate `/healthz`,
    authenticated `/api?mode=queue&output=json`, and authenticated `PROPFIND /`
    before declaring healthy.

---

## Technologies

### Backend
- **Python 3.14** — Stack scripts, tests
- **Rust** — Deck backend (axum), RPG backend (Axum)
- **SQLite** — Local app state (*arr apps, nzbdav)
- **PostgreSQL** — RPG database (future: shared RAWRZ Postgres per master spec §7)

### Infrastructure
- **Docker Compose** — Service orchestration
- **rclone** — FUSE mount for streaming content
- **InfiniDysk** — Usenet download client + WebDAV server
- **Plex** — Media server with hardware transcoding (VAAPI)

### Frontend
- **React 18 + TypeScript + Vite** — Cave Deck frontend
- **Svelte** — RPG frontend (pending)

### Security
- **Trivy** — CVE scanning (nightly CI + weekly)
- **Dependabot** — Docker, pip, cargo, npm updates (weekly); Actions are SHA-pinned
- **CodeQL** — Code scanning (Python, Rust, JS/TS)
- **ShellCheck** — Shell script linting
- **Ruff** — Python linting
- **actionlint** — Workflow validation

### CI/CD
- **GitHub Actions** — full policy in [docs/stack/ci-cd.md](docs/stack/ci-cd.md)
  - **All third-party actions are SHA-pinned** with a `# tag` comment.
  - **release-please only opens PRs for `feat:`/`fix:` commits.**
  - **actionlint gates every workflow change** in `validate.yml`.
  - `validate.yml` — compose validation, env coverage, shellcheck, ruff, actionlint,
    Rust fmt/clippy/test for both crates, frontend typecheck/lint/build, catalog validation
  - `release-please.yml` — automated release management
  - `trivy-scan.yml` — CVE scan of compose images
  - `codeql.yml` — CodeQL security analysis
  - `nightly-healthcheck.yml` — daily validation
  - `pr-labeler.yml`, `pr-lint.yml`, `stale.yml`, `dependabot.yml` — hygiene

### Languages
- **Python** — Stack scripts, tests
- **Rust** — Deck backend, RPG backend
- **Bash** — System scripts, CI steps
- **TypeScript** — Cave Deck frontend
- **YAML** — Docker Compose, CI/CD workflows

---

## Platform Constraints

- **Linux only** — FUSE, VAAPI, host networking
- **FUSE mounts** — nzbdav_rclone requires `/dev/fuse` and `SYS_ADMIN`
- **Direct ports** — retained as rollback paths; ensure the six ports and nginx 80/443 are free on the host

---

## Configuration

### Environment Variables

All secrets live in `.env` (never committed). See `.env.template` for the full list.

| Variable | Purpose |
|----------|---------|
| `RADARR_API_KEY` | Radarr API authentication |
| `SONARR_API_KEY` | Sonarr API authentication |
| `PROWLARR_API_KEY` | Prowlarr API authentication |
| `PLEX_TOKEN` | Plex authentication |
| `PLEX_CLAIM` | Plex first-run server registration claim token |
| `NZBDAV_WEBDAV_USER/PASS` | WebDAV authentication |
| `NZBDAV_RCLONE_RC_PASS` | rclone remote control password |
| `NZBDAV_USENET_*` | Usenet provider credentials (primary + backup) |
| `HOST_IP` | Host IP address (used for direct service URLs) |
| `RELEASE_PLEASE_TOKEN` | PAT for release-please (required for automated releases) |

### Docker Secrets

Sensitive values in `secrets/` directory (gitignored).
Run `./scripts/setup.sh` to prepare bind-mount directories, initialize CA trust,
create the private rclone config, and generate secrets.

---

## Retired Services

The following were removed end to end (compose, config, env vars, docs, tests):
traefik, loki, promtail, grafana, prometheus, alertmanager, node-exporter, cadvisor,
nzbdav-exporter, arr-dashboard, landing-page, metacache, lidarr, readarr,
audiobookshelf, komga, adguard, crowdsec, vaultwarden, watchstate, bazarr
(re-retired 2026-09-06), control-panel, cleanuparr, uptime-kuma, n8n.

Full reasons and re-adoption policy: [docs/stack/services/lifecycle.md](docs/stack/services/lifecycle.md).

---

## Supersession Policy

When the master spec (`rawrz-megastack-spec.md`) reverses a decision recorded in a
sub-spec, the sub-spec **must** carry a supersession banner pointing at the master spec
section that reverses it. No sub-spec may be left asserting something the master spec
reverses. This is a documentation-correctness requirement.

Current supersession state:
- `docs/rpg/movie-rpg-spec.md` — banner added (§19 reversals: not-part-of, no-container,
  reverse proxy, webhooks, `~/Cave/backend/`, `RPG_*` naming)
- `docs/agents/FIX.md` — banner added (§8 no-Redis reversed by master §4 D7–D12)
- `docs/deck/cave-deck-spec.md` — banner added (§3.1 repo/submodule model, §3.4 LAN-only,
  catalog Redis opt-in → core)
- `docs/agents/CLAUDE.md` — updated (RPG is now a RAWRZ component)

See `rawrz-megastack-spec.md` §19 for the full supersession table.
