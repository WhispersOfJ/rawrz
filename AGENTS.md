# AGENTS.md

Complete reference for AI coding agents working in this repo. Read `CLAUDE.md` first for
work style and non-negotiable rules — this file covers the system itself.

---

## What This Repo Is

A slim, robust media-acquisition-and-serving stack. **8 always-on Compose services**
(Prowlarr, Radarr, Sonarr, nzbdav, nzbdav_rclone, Seerr, Plex, Unpackerr), plus
manual ImageMaid and Recyclarr maintenance profiles, published directly on host ports
— no reverse proxy — with CI/CD via GitHub Actions. Hosted on Linux.

> **2026-08-30 slim-down:** after a stability incident (Bazarr OOM crash-loop, Radarr
> API 500s from an orphaned quality-profile reference, ~19Gi of mem caps against 22Gi
> host RAM), the stack was deliberately pared from 29 configured services down to 8.
> The full retirement record — what was removed, why, and the re-adoption policy — is
> in [docs/services/lifecycle.md](docs/services/lifecycle.md). Legacy files from the
> merged source repos (`media-stack`, `metacacharr`) are preserved in `archive/`.
>
> **2026-09-06 re-retirement:** Bazarr was removed again (8-service target;
> subtitle acquisition did not justify an always-on container). It is back in the
> retired registry. See lifecycle.md's retirement record.
>
> **2026-09-04 demotion:** Recyclarr moved from the always-on set to the manual
> `maintenance` profile — its daily 04:00 sync no longer runs automatically; invoke
> it on demand with `docker compose --profile maintenance run --rm recyclarr sync`.
> Config and secrets are unchanged.

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

### Service Categories

| Category | Services |
|----------|----------|
| **Indexing** | Prowlarr |
| **\*arr apps** | Radarr (movies), Sonarr (TV) |
| **Usenet** | InfiniDysk/nzbdav + nzbdav_rclone sidecar |
| **Requests** | Seerr |
| **Media server** | Plex (host network, VAAPI transcoding) |
| **Queue mgmt** | Unpackerr |
| **Manual maintenance** | ImageMaid (profile-gated PhotoTranscoder cache cleanup), Recyclarr (profile-gated TRaSH-Guides quality-profile sync for both *arr apps) |

### Content flow

Prowlarr indexes → Radarr/Sonarr queue → nzbdav downloads → rclone FUSE mount → Plex serves

- **Networking:** every service is published directly on a host port. There is no
  reverse proxy tier; access services at `http://HOST_IP:<port>` (ports below).
- **Bash functions** (`services/bash-functions/`) are the operational surface: queue
  management, Plex maintenance (scan/empty-trash/Butler), backlog checks, mount health,
  and the manual ImageMaid PhotoTranscoder cleanup command. See
  [docs/services/bash-functions.md](docs/services/bash-functions.md). The retired fish
  functions are recorded in [docs/services/FISH.md](docs/services/FISH.md).

---

## Services (8 always-on services)

| # | Service | Purpose | Port | Network |
|---|---------|---------|------|---------|
| 1 | `prowlarr` | Indexer manager | 9696 | bearcave |
| 2 | `radarr` | Movie management | 7878 | bearcave |
| 3 | `sonarr` | TV show management | 8989 | bearcave |
| 4 | `nzbdav` | Usenet download client + WebDAV | 3000 | bearcave |
| 5 | `nzbdav_rclone` | FUSE mount sidecar (streams on demand) | — | bearcave |
| 6 | `seerr` | Request manager | 5055 | bearcave |
| 7 | `plex` | Media server | 32400 | host |
| 8 | `unpackerr` | Auto-extracts downloads | — | bearcave |

### Memory caps (slim-stack rebalance)

| Service | mem_limit | Note |
|---------|-----------|------|
| `radarr` | 1536m | 1GB DB with MediaInfo blobs; was OOMing at 1g; 1.5 CPU for imports |
| `sonarr` | 1024m | ~365MB actual usage; 1.5 CPU to avoid scan/import throttling |
| `nzbdav` | 2560m | download + WebDAV; 2 CPU for concurrent provider/WebDAV work |
| `nzbdav_rclone` | 4096m | FUSE/WebDAV cache; 2 CPU for concurrent media reads. 4096 (was 3072) because the vfs metadata cache peaks near the old cap during 100k+ item library analysis; host has headroom. |
| `prowlarr` | 512m | |
| `seerr` | 512m | |
| `plex` | 2048m | host network, VAAPI; 4 CPU for library analysis |
| `unpackerr` | 64m | |
| **Total caps** | **≈ 12.1g** | CPU quotas leave headroom for concurrent scans/downloads; memory remains below the 22 GiB host total |

### Network Topology

- **bearcave** — main bridge network for all containerised services
- **host** — Plex uses host networking: GDM, DLNA, and remote-access
  NAT-PMP/UPnP negotiation are unreliable on bridge networking.

There is no reverse proxy tier. Every other service is reached directly at
`http://HOST_IP:<port>`.

### Retired services (2026-08-30 slim-down)

The following were removed end to end (compose, config, env vars, docs, fish
functions, tests). Full reasons and re-adoption policy are in
[docs/services/lifecycle.md](docs/services/lifecycle.md):

traefik, loki, promtail, grafana, prometheus, alertmanager, node-exporter, cadvisor,
nzbdav-exporter, arr-dashboard, landing-page, metacache, lidarr, readarr,
audiobookshelf, komga, adguard, crowdsec, vaultwarden, watchstate, bazarr
(re-retired 2026-09-06).

> Note: the selected plan was the extreme scenario while retaining Seerr and Unpackerr
> because request handling and automatic extraction remain useful in the final
> 8-service composition.

---

## Port Map

```
3000  nzbdav (WebDAV)
5055  Seerr (requests)
7878  Radarr
8989  Sonarr
9696  Prowlarr
32400 Plex (host network)
```

**API surfaces** — the full map of every API surface on the stack (base URLs,
auth conventions, the endpoints each script/function exercises, and canonical
upstream docs) lives in [docs/API.md](docs/API.md). Update it when a script
starts calling a new endpoint.

---

## Technologies

### Backend
- **Python 3.14** — Scripts, tests
- **SQLite** — Local app state (*arr apps, nzbdav)

### Infrastructure
- **Docker Compose** — Service orchestration
- **rclone** — FUSE mount for streaming content
- **InfiniDysk** — Usenet download client + WebDAV server (formerly nzbdav)
- **Plex** — Media server with hardware transcoding (VAAPI)

### Security
- **Trivy** — CVE scanning (nightly CI + weekly schedule)
- **Dependabot** — Docker, pip updates (weekly); GitHub Actions are SHA-pinned so action upgrades are manual (see docs/ci-cd.md)
- **CodeQL** — Code scanning for Python
- **ShellCheck** — Shell script linting
- **Ruff** — Python linting

### CI/CD
- **GitHub Actions** — full policy in [docs/ci-cd.md](docs/ci-cd.md)
  - **All third-party actions are SHA-pinned** (immutable supply chain); the `# tag` comment records the version. Upgrade path: `gh api repos/{owner}/{repo}/commits/{tag} --jq .sha`, then update SHA + comment. Dependabot cannot auto-bump SHA pins.
  - **release-please only opens PRs for `feat:`/`fix:` commits.** `ci:`, `docs:`, `chore:` do not trigger a release. If you need to cut a release, ensure at least one commit uses a release-worthy type.
  - **Brand-new repo race condition:** workflows added in the initial push of a new repo may not trigger on push/PR events. Manual dispatch works. Re-adding or renaming the workflow file fixes it.
  - **actionlint gates every workflow change** in `validate.yml` — syntax, expressions, action refs, and shellcheck on `run:` blocks. Replicate locally: download the pinned actionlint release binary and run `actionlint .github/workflows/*.yml`.
  - `validate.yml` — compose validation, env coverage, shellcheck, ruff, actionlint
  - `release-please.yml` — automated release management
  - `trivy-scan.yml` — CVE scan of compose images, IaC config scan, baseline report
  - `codeql.yml` — CodeQL security analysis (Python)
  - `nightly-healthcheck.yml` — daily compose/Dockerfile/script/config validation
  - `pr-labeler.yml` — auto-label PRs by size and file paths
  - `pr-lint.yml` — enforce Conventional Commits in PR titles
  - `stale.yml` — auto-close stale issues and PRs
  - `dependabot.yml` — automated dependency updates

### Languages
- **Python** — Scripts, tests
- **Bash** — System scripts, CI steps
- **TypeScript** — legacy arr-dashboard sources under `archive/`
- **YAML** — Docker Compose, CI/CD workflows

---

## Configuration

### Environment Variables

All secrets live in `.env` (never committed). See `.env.template` for the full list.
Key groups:

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
| `RELEASE_PLEASE_TOKEN` | PAT for release-please to create release PRs and push tags (required for automated releases) |

### Docker Secrets

Sensitive values should be stored in `secrets/` directory (gitignored).
Run `./scripts/setup.sh` to prepare bind-mount directories, initialize public CA trust,
create the private rclone config, and generate secrets.

### Platform Constraints

- **Linux only** — the stack assumes a Linux host (FUSE, VAAPI, host networking)
- **FUSE mounts** — nzbdav_rclone requires `/dev/fuse` and `SYS_ADMIN` capability
- **Direct ports** — no reverse proxy; ensure the six ports above are free on the host

---

## Historical Issues and Landmines

### Critical Landmines (affect operations today)

1. **Bind-mount file staleness** — `sed -i`/`vim` on a bind-mounted file changes the inode; the container keeps serving the old file until restarted. Always `docker compose restart <container>` after editing a file served by a bind mount.
2. **FUSE mount fragility** — Mount-owner restart breaks all dependents. Never `sudo umount` a FUSE mountpoint. Restart the owner, then all dependents in order. A stale mount is also why Plex shows "red trash cans": if items vanish, verify the mount is healthy *before* rescanning, then rescan and empty the trash.

3. **Plex `stop_grace_period: 90s` required** — Without it, Docker's 10s default SIGKILL fires mid-shutdown, producing a genuine unkillable D-state hang.

4. **NzbDAV queue is not persistent** — Recreate wipes queued NZBs and silently blocklists affected items. Confirm pending is 0 before touching. The Compose healthcheck must probe both the public frontend (`:3000/healthz`) and an authenticated queue API request through the frontend; a green frontend alone is insufficient.

5. **Plex on host network** — Plex cannot run on a bridge network without losing GDM/DLNA/remote access. Access directly at `http://HOST_IP:32400`.

6. **rclone.conf requires `rclone obscure`** — Passwords in rclone.conf must be rclone-obfuscated, not plaintext.

7. **App removal checklists must be exhaustive** — Every removal touches: compose, config, env vars, Prowlarr sync, docs, tests. See the 2026-08-30 slim-down record in [docs/services/lifecycle.md](docs/services/lifecycle.md).

8. **Radarr orphaned quality-profile references** — A movie row pointing at a deleted quality profile makes `/api/v3/movie` return 500 for the whole collection. After deleting profiles in Radarr, verify every movie still resolves (2026-08-30 incident: movie 60308 → profile 17).

9. **SQLite DB bloat from MediaInfo blobs** — Radarr stores 10–300KB MediaInfo blobs per history row; a long-lived instance grows a 1GB `radarr.db` plus a 184MB `logs.db`, pushing the process into OOM at a 1g cap. Sonarr has the same class (2026-09-02: a 3.2 GiB `sonarr.db`, ~2.0 GiB in `EpisodeFiles.MediaInfo`). Prune blobs/logs periodically or raise the cap (now 1536m). The gate only *detects* bloat; remediate with `stack-radarr-prune` (stops radarr, backs up, prunes MediaInfo + old history, vacuums, verifies, resumes) or its Sonarr analogue `stack-sonarr-prune`; both re-verify with the shared `check_radarr_db_size.py` gate (`--blob-table MovieFiles`/`EpisodeFiles`).

10. **ImageMaid path validation is behavioral** — Its `/plex` mount must target the Plex application-support subdirectory `config/plex/Plex Media Server`, not the parent `config/plex`; the upstream process can exit successfully while reporting a missing PhotoTranscoder directory and reclaiming zero bytes.

11. **Profiled run services need generated names** — Do not assign `container_name` to the manual ImageMaid profile; `docker compose run --rm` must be able to repeat after an interrupted run without a stale fixed-name collision.

12. **Main may advance asynchronously** — Release automation can add commits between local review and publication; fetch `origin/main` and rebase a clean local commit before retrying a rejected push, never force-push.

13. **NzbDAV backend outages can be masked by the frontend** — The frontend on port 3000 can remain healthy while its internal backend fails, causing WebDAV/API requests to return 502. Validate `/healthz`, authenticated `/api?mode=queue&output=json`, and authenticated `PROPFIND /` before declaring the service healthy. Never recreate the container until the persisted queue has been checked or the data-loss decision is explicit.

---

## How to Work in This Repo

### Worktree Discipline — mandatory, effective 2026-08-31

From this point forward, **all edits happen on dedicated git worktrees** — one
worktree per task, named by the task, never mixed with unrelated work. This
rule applies to every future change, including the change that introduced it.

**Repository containment rule (2026-09-04):** Every worktree for this site must
live inside `/home/bear/cave/`, preferably under
`/home/bear/cave/.worktrees/<task-name>`. Do not create or retain site
worktrees under `/home/bear/.worktrees/`, `/home/bear/wt-*`, or any other
external path. Before editing, verify with `git worktree list --porcelain`; after
relocating or removing a worktree, run `git worktree prune` and verify again.
The main checkout remains reference-only and must stay free of task edits.

1. **One worktree per task.** Before making any edit, create a task-named
   worktree and branch off `origin/main`:
   `git worktree add <path> -b <task-branch> origin/main`. Both the worktree
   path and the branch name must describe the task.
2. **Never mix unrelated work.** A worktree contains exactly one task's
   changes and nothing else. A second, unrelated need gets its own worktree.
   Do not stack unrelated edits, commits, or topics in one worktree.
3. **The main checkout stays clean.** Do not edit or commit in the main
   working tree; use it for reference only (fetch/status/log). Pre-existing
   untracked files in it are left untouched and are not committed.
4. **Deliver via PR.** `main` is branch-protected: push the task branch, open
   a PR (linear history; squash/rebase only), and keep the branch up to date
   with `origin/main` before merging.
5. **Clean up.** After the PR merges, remove the worktree:
   `git worktree remove <path>`.
6. **Canonical walkthrough.** Example commands for the full lifecycle
   (create → edit → push → PR → merge → remove) live in
   [docs/worktree-lifecycle.md](docs/worktree-lifecycle.md).

### Source-Code Questions: grep `~/TRUTH` first — mandatory, effective 2026-09-05

`/home/bear/TRUTH` (outside this repo, do not commit it) holds shallow git
clones of the **actual upstream application source** for every container in the
stack — each of the 8 always-on services plus both maintenance-profile services
(ImageMaid, Recyclarr) — pinned to the exact version each image runs today.

When a question is about *how something in this stack behaves in code* — an API
endpoint or its parameters, a config/option's meaning, the origin of an error
string, a CLI flag's semantics, a DB schema field, a request/response shape —
answer it **first by local `rg`/`grep` over `~/TRUTH`**, never from memory,
from docs alone, or from a web search of upstream `main` (which may describe a
version this stack does not run). **It is the truth of all source code
investigations.**

Rules of the road:

1. **Grep the owning service's tree first.** `rg -n "<pattern>" ~/TRUTH/<dir>/`
   (add `-i` when unsure of case). Match by symbol/string, then read the hit in
   context. For cross-cutting behaviour (e.g. how Radarr/Sonarr/Prowlarr each
   handle a *arr pattern), grep the sibling trees too.
2. **App source, not packaging.** The `ghcr.io/hotio/*` and similar images are
   packaging only; their source labels point at Dockerfile repos. `~/TRUTH`
   holds the application code (Radarr, Sonarr, Prowlarr, …) — the tree
   you actually want to grep. Do not chase the hotio repo for app behaviour.
3. **Version skew is the first suspect when a search comes up empty.** Clones
   are pinned to the *running* versions; a symbol that exists upstream on
   `main` may legitimately not exist here. Before going to the web, confirm the
   clone's ref matches the running image (table below; exact refs/commits in
   `~/TRUTH/README.md`). Only if the code truly is absent at the pinned version
   is a web/docs consult warranted — and say so in your answer.
4. **Keep the corpus pinned to the stack.** When `docker-compose.yml` bumps an
   image version, re-pin that service's clone (tag-matching the running release,
   e.g. hotio `release-4.0.19.2979` → Sonarr tag `v4.0.19.2979`) and update
   `~/TRUTH/README.md`. Full history for a repo: `git -C ~/TRUTH/<dir> fetch
   --unshallow --tags`.
5. **Scope limit.** Plex's Media Server core is proprietary and has no public
   source; `~/TRUTH/plex` is only the image wrapper (`plexinc/pms-docker`).
   Grep it for image/container behaviour, never for PMS internals. InfiniDysk
   (nzbdav) and ImageMaid run rolling (`dev`/`latest`) images, so their clones
   track upstream default branches — record image digests from
   `docker-compose.yml` when they change.

Service → source map (full table incl. refs and commit SHAs:
`~/TRUTH/README.md`):

| Container / profile service | Grep dir in `~/TRUTH` | What it is |
|---|---|---|
| `prowlarr` | `~/TRUTH/prowlarr` | Prowlarr app source @ v2.5.2.5491 |
| `radarr` | `~/TRUTH/radarr` | Radarr app source @ v6.3.0.10514 |
| `sonarr` | `~/TRUTH/sonarr` | Sonarr app source @ v4.0.19.2979 |
| `nzbdav` | `~/TRUTH/infinidysk` | InfiniDysk source, `main` (rolling `dev` image) |
| `nzbdav_rclone` | `~/TRUTH/rclone` | rclone source @ v1.75.0 |
| `seerr` | `~/TRUTH/seerr` | Seerr source @ v3.4.1 |
| `plex` | `~/TRUTH/plex` | **Core closed source** — pms-docker wrapper only; PMS 1.43.3.10896-cb3ebc72d |
| `unpackerr` | `~/TRUTH/unpackerr` | Unpackerr source @ v0.16.1 |
| `imagemaid` (maintenance) | `~/TRUTH/imagemaid` | ImageMaid source, `master` (rolling `latest` image) |
| `recyclarr` (maintenance) | `~/TRUTH/recyclarr` | Recyclarr source @ v8.7.2 |

### Before Making Changes

1. Create the task-named worktree (see Worktree Discipline above)
2. Read `CLAUDE.md` for work style rules
3. Check `docker compose ps` for current state
4. Read `docs/` for service documentation
5. If the task needs to know how a service behaves in code, grep the pinned
   upstream source in `~/TRUTH` first (see Source-Code Questions above)

### After Making Changes

1. Run validation: `docker compose config --quiet`
2. Run bash syntax checks: `bash -n scripts/*.sh tests/*/*.sh`3. Run bash smoke tests: `bash tests/bash/test_bash_functions.sh --offline` (the bash port is the active operational surface)
4. Use `./tests/integration/test_pipeline.sh --dry-run` for a live infrastructure check when the NzbDAV queue is non-empty; the full pipeline test intentionally fails rather than treating active queued work as safe.
5. Keep agent-facing operational output in English.
6. **Restart containers after editing bind-mounted files** — `sed -i` or `vim` on a bind-mounted file changes the inode; the container keeps serving the old content until restarted. This is invisible (no error).

### Safety Rules

- Never commit `.env` or secrets
- Never run destructive operations without confirmation
- Always restart dependents after mount-owner changes
- Always confirm NzbDAV queue is empty before container operations
- Use `--force-recreate` when .env changes need to take effect
- Plex config directory contains the full library metadata — back up before changes
- ImageMaid is manual and profile-gated; run its PhotoTranscoder-only cleanup only while Plex is idle. It does not resize or recompress artwork.

---

## Archive

Retired services and their re-adoption watchers are tracked in [docs/services/lifecycle.md](docs/services/lifecycle.md).

Legacy files from the source repos are preserved in `archive/`:
- `archive/media-stack/` — 133+ fish functions, scripts, systemd units, CLAUDE.md, STACK.md
- `archive/metacacharr/` — DESIGN.md, tests, monitoring configs

These are reference material only — not part of the active stack.
