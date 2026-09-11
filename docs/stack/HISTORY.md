# History of The Bear Cave

Everything that has already happened to this stack, in one place. Active, current-state
documentation lives in [README.md](README.md) (overview), [AGENTS.md](AGENTS.md)
(operational reference), and [docs/](docs/). This file exists so historical context
doesn't leak into current-state docs — if you're reading about something that is no
longer true of the running stack, it belongs here.

> **Convention:** `archive/` content is reference material and is never revived into the
> active stack; any return must be a fresh, tracked implementation, not a copy.

---

## Timeline

| Date | Milestone |
|------|-----------|
| 2026-08-21 | `media-stack` Stage 1–4 complete: logging (Loki/Promtail/Grafana), secure deployment, control-panel hardening |
| 2026-08-26 | **The Bear Cave born** — `media-stack` + `metacacharr` merged into one repo (initial scaffold `a0c976d`); mkcert local CA, Ansible CA-trust playbook, release automation |
| 2026-08-26 | v1.1–1.5 — landing page ecosystem (registry as single source, health view, Mermaid, backlinks), Alertmanager → Discord, fish functions rewrite (95 API-backed + 23 host tools), monitoring stack |
| 2026-08-27 | **control-panel (Django) removed** in Phase 4 (`faf4127`) — fish functions + landing page probe services directly; archived to `archive/control-panel/` |
| 2026-08-27 | v1.9–1.10 — resource limits on all services, backup/disk/load alerts, fish Phase 1–2 (direct-service routing) |
| 2026-08-28 | **Expansion spec written** — 10 new containers planned (see [Expansion](#the-2026-08-28-expansion)) |
| 2026-08-28 | v1.11–1.12 — Plex beta image, InfiniDysk dev-tag tracking, 15 CRITICAL CVEs eliminated, fish completions |
| 2026-08-29 | **9 of 10 expansion services deployed** (`3d25969`) — Lidarr, Readarr, Bazarr, Audiobookshelf, Komga, AdGuard Home, CrowdSec, Vaultwarden, n8n |
| 2026-08-29 | **Uptime Kuma removed from scope** by decision (`ad04cf8`) — image CVE-blocked at the time |
| 2026-08-29 | v1.13–1.14 — expansion services, Traefik dashboard link, HTTPS nip.io dashboard links |
| 2026-08-29 | **Cleanuparr retired** (`4604423`) — torrent-only, no SABnzbd/Usenet client; upstream watch job added |
| 2026-08-29 | **n8n removed** (`0ee71f2`) by decision — no workflows shipped; Discord alerting covered by alertmanager/CrowdSec |
| 2026-08-29 | MCP tooling (Freebuff Desktop connectors), preflight gate, MCP baseline alerting; v1.15–1.16 |

---

## The 2026-08-28 Expansion

The 10-service expansion (documented at the time in `stack-expansion-spec.md`, removed
2026-08-29 after its content was consolidated here) added media acquisition/serving,
network security, and utility services in three phases. All decisions came from a 4-round user interview.

### What was planned vs. delivered

| Service | Planned | Outcome |
|---------|---------|---------|
| Lidarr (music) | Phase 1 | ✅ deployed |
| Readarr (books) | Phase 1 | ✅ deployed |
| Bazarr (subtitles) | Phase 1 | ✅ deployed |
| Audiobookshelf | Phase 1 | ✅ deployed |
| Komga (comics) | Phase 1 | ✅ deployed |
| AdGuard Home | Phase 2 | ✅ deployed |
| CrowdSec | Phase 2 | ✅ deployed |
| Vaultwarden | Phase 3 | ✅ deployed |
| n8n | Phase 3 | ❌ **removed by decision** 2026-08-29 — no workflows shipped; Discord alerting already covered |
| Uptime Kuma | Phase 3 | ❌ **removed from scope** 2026-08-29 — image CVE-blocked at the time |

### Cross-cutting decisions from the expansion

- **nzbdav categories** — arbitrary category names are config-driven via
  `NZBDAV_CONFIG__API__CATEGORIES`; the music/books/audiobooks/comics categories were
  added queue-gated (a recreate wipes the queue).
- **CVE posture gates images** — lidarr ships `:nightly` (release lagged .NET with 5
  CRITICAL; nightly verified 0), komga pinned to `1.x`, readarr on linuxserver `develop`
  (no hotio image).
- **CrowdSec bouncer** — in-process Traefik middleware plugin (stream mode), not a
  sidecar; there is no first-party CrowdSec Traefik sidecar/forwardAuth bouncer.
- **Auth is per-tool native** — no Traefik basic-auth layer; Vaultwarden/AdGuard/n8n use
  their own logins.
- **Syncthing** was removed from scope during planning (no offsite/LAN peer wanted).

---

## Retired services

Every service removed from `docker-compose.yml` end to end (compose, landing page
registry, docs, tests, scripts, trivy baseline, AGENTS.md). Live tracking of
re-adoption watchers lives in [docs/services/lifecycle.md](docs/services/lifecycle.md).

| Service | Retired | Reason | Re-adoption watcher | Where it went |
|---------|---------|--------|---------------------|---------------|
| `cleanuparr` | 2026-08-29 | Torrent-only; no SABnzbd/Usenet client, so nothing to monitor in the Usenet-only stack | ✅ `cleanuparr-sabnzbd-watch.yml` | removed (no archive) |
| `uptime-kuma` | 2026-08-29 | Dropped from expansion scope by decision (image still CVE-blocked at the time) | ❌ none (decision) | removed (no archive) |
| `n8n` | 2026-08-29 | Workflow automation removed by decision — Discord notifications are handled by alertmanager/CrowdSec hooks; no workflow glue needed | ❌ none (decision) | removed (no archive) |
| `control-panel` (Django) | 2026-08-27 | Django backend superseded — fish functions call services directly, landing page probes via nginx | ❌ none (archived) | `archive/control-panel/` |

### cleanuparr — why it went, how it can come back

Cleanuparr v2.x only supports torrent clients (qBittorrent / Deluge / Transmission /
rTorrent). This stack is Usenet-only, so with no SABnzbd-compatible client there was
nothing for it to monitor. It was removed end to end (commit `4604423`).

The **re-adoption watcher** (`.github/workflows/cleanuparr-sabnzbd-watch.yml`, daily
06:23 UTC + manual dispatch) reads the latest upstream `Cleanuparr/Cleanuparr` release;
when release notes mention SABnzbd/Usenet it opens a `cleanuparr-adopt` issue and posts
a Discord alert. The signal choice matters: the upstream feature request (#137) is
closed as **`not_planned`** and its duplicates (#263/#273) as duplicates, so issue
state is never trusted as a shipped signal — only release-note text is.

Adoption, when the watcher fires, mirrors the removal in reverse: compose service,
NzbDAV SABnzbd-compatible download client at `nzbdav:3000`, `CLEANUPARR_URL`, landing
page registry, docs/tests, trivy baseline.

### uptime-kuma — removed from scope

Dropped from the expansion by decision while its image was still CVE-blocked, rather
than gated behind a re-scan. All files — workflow, docs, spec references, local images
— were removed. No watcher; no plan to return unless monitoring needs change.

### n8n — removed by decision

No workflows had shipped in production: the spec's §10 Q2 "first workflow" (Discord
notifications) was never built or activated, and Discord alerting was already covered
by alertmanager + CrowdSec hooks. The container, config dir, landing page entry, docs,
and env vars were removed end to end (commit `0ee71f2`). No watcher — if workflow glue
is ever needed again, re-adopt from scratch.

### control-panel (Django) — archived

Removed in Phase 4 (commit `faf4127`) after its role was superseded: fish functions
call the *arr/dashboard APIs directly, and the landing page probes health through
nginx. Files live in `archive/control-panel/` for reference. Deliberate teardown, not a
hold for return.

---

## Release history (v1.x)

Auto-generated release notes. Current version: **v1.16.0**.

### v1.16.0 (2026-08-29)
- **feat:** wire MCP baseline alerting, preflight gate, and unpackerr connections (`a92fe8a`)
- **fix:** guard `cd` in preflight.sh against failure (`a34439d`)

### v1.15.0 (2026-08-29)
- **feat:** add MCP baseline comparison and refresh workflow (`a261051`)
- **fix:** rename ambiguous loop variable to pass ruff E741 (`f130332`)

### v1.14.0 (2026-08-29)
- **feat:** Traefik dashboard link on landing page (`ffb56ac`); deploy 9 new stack services with crowdsec bouncer and nzbdav categories (`3d25969`); HTTPS nip.io dashboard links (`69b18b7`); landing page new services/health probes/links (`b2bc2a8`)
- **fix:** `ADGUARD_ADMIN_*` in .env.template (`64ec98c`); trivy baseline re-keyed to pins (`d06c333`); update-nzbdav.sh dependents sync (`293523e`)

### v1.13.0 (2026-08-29)
- **feat:** audiobook + comic servers (`2d21c89`); Bazarr (`8be99d4`); Lidarr + Readarr (`871643c`); network security services (`879282a`); Vaultwarden + n8n (`4e6f494`)
- **fix:** sync secret manifest with workflow usage (`1aeda7f`)

### v1.12.1 (2026-08-28)
- **fix:** node-exporter healthcheck body (stop log flood) (`e4ed066`); eliminate 15 CRITICAL CVEs (`db5c97b`)

### v1.12.0 (2026-08-28)
- **feat:** fish tab completions for all `stack-*` commands (`9a3c154`)
- **fix:** fish systemic breakage remediation (`8689b62`, `a776913`)

### v1.11.0 (2026-08-28)
- **feat:** track InfiniDysk dev tag + queue-guarded update script (`23a4e2a`); Plex official beta image (`e3323c3`)

### v1.10.1 (2026-08-27)
- **fix:** Mermaid graph edge node IDs (`c8efb03`); revert wrong image pins + recreate with resource limits (`fae4f3c`)

### v1.10.0 (2026-08-27)
- **feat:** backup freshness, disk fill rate, load alerts (#39, `e5f924c`); resource limits on all services (#5, `fb3b43a`)
- **fix:** BackupStale metric + HostHighLoad PromQL (`39e7030`)

### v1.9.1 (2026-08-27)
- **fix:** actionlint checksum verification + pre-commit gate (`aebec8d`); pytest in exporter deps (`16d1ab5`); actionlint SHA-256 (`19d6fa9`); harden stack per potential.md audit (`4f4255c`)

### v1.9.0 (2026-08-27)
- **feat:** fish Phase 2 rewrite (71 functions, bypass control panel, `c4e12f1`); landing Phase 3 direct probes (`3efae51`); Phase 4 remove control panel (`faf4127`)
- **fix:** DISCORD_WEBHOOK_URL secret manifest (`54dc06e`); disk-cleanup reclaim (`3798e6c`); actionlint in new workflows (`f27e200`); fish __stack_api fixes (`79058dd`); 17 custom integration functions (`950f925`); restore FRONTEND_BACKEND_API_KEY (`2f8fabe`)

### v1.8.0 (2026-08-27)
- **feat:** fish Phase 1 direct-service API helpers (`a78c8bf`)
- **fix:** helper image/command order (`aef7108`); remove writable Docker socket from control-panel (`4fe6321`)

### v1.7.0 (2026-08-27)
- **feat:** harden control panel + fix no-op Trivy gate (`44f97ae`)

### v1.6.0 (2026-08-27)
- **feat:** Alertmanager + Discord + Cleanuparr setup docs (`18861dc`); `/api/v2/cli/` endpoints (`32f475c`); fish rewrite (95 API-backed + 23 host tools, `979d769`); monitoring stack (`edb6c5c`); login rate limiting (`523d840`)
- **fix:** network constant 'stacknet' → 'bearcave' (`d16248e`); pipefail test fix (`94defd8`)
- **perf:** gzip + lazy-load Mermaid on landing page (`ea70f4d`)

### v1.5.0 (2026-08-26)
- **feat:** full health coverage, live Mermaid colors, registry as single source (`346cd14`)

### v1.4.1 (2026-08-26)
- **fix:** health view 19 services, backlinks absolute, registry validation, docstring accuracy (`6ab8e5c`); pipeline flow parallelism + Mermaid guard (`2753f11`)

### v1.4.0 (2026-08-26)
- **feat:** unified ecosystem — service registry, category layout, Mermaid graph, detail panels, backlinks (`06889f5`)

### v1.3.2 (2026-08-26)
- **fix:** same-origin health fetch fixes HTTPS status dots (`46caf92`)

### v1.3.1 (2026-08-26)
- **fix:** repoint doc links from media-stack to thebearcave (`ff5c3c5`)

### v1.3.0 (2026-08-26)
- **feat:** CA-trust badge backed by live TLS probe (`81d20b6`)

### v1.2.0 (2026-08-26)
- **feat:** control-panel read-only TLS cert diagnostic endpoint (`33de97e`); sync RELEASE_PLEASE_TOKEN to Actions secrets (`178d36a`)

### v1.1.0 (2026-08-26)
- **feat:** initial scaffold — merge media-stack + metacacharr (`a0c976d`); Ansible CA-trust playbook (`3b10053`); certificate setup on landing page (`c78f7f6`); local CA (mkcert) wildcard cert (`736f8c3`); trust CA inside every container (`e2acbe0`)
- **fix:** align docs/gitignore with config/ layout (`85841e7`); PR title lint (`7d889e1`); release-please PAT fallback (`1952a08`); PR labeler size expression (`6ab944b`); shellcheck unused vars (`623432b`); stop failed Let's Encrypt attempts on LAN (`ef2ab3c`)

---

## Pre-merge history

The repo merged two prior projects, both preserved in `archive/`:

- **`archive/media-stack/`** — 133+ fish functions, scripts, systemd units, the original
  `STACK.md`, `HISTORY.md` (pre-merge completed work), security model, and stage-4 CVE
  baseline. Usenet + Plex + *arr acquisition stack.
- **`archive/metacacharr/`** — the original TMDB/TVDB metadata cache project
  (predecessor of the active `services/metacache/` service): DESIGN.md, tests, and
  monitoring configs.

Key pre-merge milestones (from `archive/media-stack/HISTORY.md`):
- **Phase 2: Secure Media Stack Deployment (2026-08-21)** — Stage 1 logging
  (Loki 2.8.0 + Promtail 2.8.0 + Grafana 10.4.0), concurrent Stages 1/4 + control-panel
  hardening, commits `adde0b6`, `5f018fe`, `17fe4f8`, `3f2772d` (reverted), `bc0a989`.

---

## Where old ideas went

- **`potential.md`** (removed 2026-08-29 after its content was consolidated here) held
  90 evidence-backed improvement recommendations —
  security audit findings, software to remove, and software worth adding. Completed
  items shipped across v1.1–1.16 (see Release history); outstanding items were
  re-filed as GitHub issues/PRs where actionable.
- **`stack-expansion-spec.md`** (removed 2026-08-29 after its content was consolidated
  here) held the full 10-container expansion design
  — interview decisions, port allocations, draft compose blocks, image verification
  findings, and the nzbdav category rollout runbook. Its deployment outcome is
  summarized in [The 2026-08-28 Expansion](#the-2026-08-28-expansion).
- **`CHANGELOG.md`** remains the machine-generated release feed (keep — release-please
  rewrites it); this file is the human-readable narrative.
