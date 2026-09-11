# Service Lifecycle & Retirement

Tracks services that have been **retired** from the active stack and whether /
how they can come back. "Retired" means removed from `docker-compose.yml` and the
supporting files (landing page registry, docs, tests, scripts, AGENTS.md) — distinct
from a temporary outage. Every retirement here is deliberate: the service was either
superseded, no longer fits, or is gated on an external condition.

A retired service either has a **re-adoption watcher** (an automated upstream watch
that pings the stack when the blocker clears) or is retired **by decision** (no plan to
return, kept only in `archive/`).

## Why this page exists

Removal is expensive and easy to get wrong — the AGENTS.md landmine checklist runs to
compose, config, env vars, Prowlarr sync, landing page registry, docs, tests, scripts,
and the trivy baseline. This page records the reference so nobody has to re-derive what
was retired, why, and what it would take to bring it back.

## Retired services

| Service | Retired | Reason | Re-adoption watcher | Where it went |
|---------|---------|--------|---------------------|---------------|
| `traefik` | 2026-08-30 | 2026-08-30 slim-down (extreme scenario): direct host ports replace reverse proxy; cert management + crowdsec bouncer no longer needed | ❌ none (decision) | removed (no archive) |
| `loki` | 2026-08-30 | 2026-08-30 slim-down: observability stack removed; docker json-file logging retained | ❌ none (decision) | removed (no archive) |
| `promtail` | 2026-08-30 | 2026-08-30 slim-down: shipped docker logs to loki (retired with it) | ❌ none (decision) | removed (no archive) |
| `grafana` | 2026-08-30 | 2026-08-30 slim-down: observability stack removed | ❌ none (decision) | removed (no archive) |
| `prometheus` | 2026-08-30 | 2026-08-30 slim-down: observability stack removed | ❌ none (decision) | removed (no archive) |
| `alertmanager` | 2026-08-30 | 2026-08-30 slim-down: observability stack removed (Discord alerting gone with it) | ❌ none (decision) | removed (no archive) |
| `node-exporter` | 2026-08-30 | 2026-08-30 slim-down: host metrics had no consumer after prometheus retirement | ❌ none (decision) | removed (no archive) |
| `cadvisor` | 2026-08-30 | 2026-08-30 slim-down: was eating 370–410MB to watch other containers; retired with prometheus | ❌ none (decision) | removed (no archive) |
| `nzbdav-exporter` | 2026-08-30 | 2026-08-30 slim-down: queue/config metrics had no consumer after prometheus retirement | ❌ none (decision) | removed (no archive) |
| `arr-dashboard` | 2026-08-30 | 2026-08-30 slim-down (extreme scenario): fish functions cover the same ops surface | ❌ none (decision) | removed (no archive) |
| `landing-page` | 2026-08-30 | 2026-08-30 slim-down (extreme scenario): registry-driven portal for a stack that no longer exists | ❌ none (decision) | removed (no archive) |
| `metacache` | 2026-08-30 | 2026-08-30 slim-down (extreme scenario): optional Plex metadata accelerator, not core acquisition | ❌ none (decision) | removed (no archive) |
| `lidarr` | 2026-08-30 | 2026-08-30 slim-down: long-tail acquisitions unused | ❌ none (decision) | removed (no archive) |
| `readarr` | 2026-08-30 | 2026-08-30 slim-down: long-tail acquisitions unused | ❌ none (decision) | removed (no archive) |
| `audiobookshelf` | 2026-08-30 | 2026-08-30 slim-down: long-tail media server unused | ❌ none (decision) | removed (no archive) |
| `komga` | 2026-08-30 | 2026-08-30 slim-down: long-tail media server unused | ❌ none (decision) | removed (no archive) |
| `adguard` | 2026-08-30 | 2026-08-30 slim-down: LAN DNS blocker, non-media | ❌ none (decision) | removed (no archive) |
| `crowdsec` | 2026-08-30 | 2026-08-30 slim-down: intrusion detection depended on traefik (retired with it) | ❌ none (decision) | removed (no archive) |
| `vaultwarden` | 2026-08-30 | 2026-08-30 slim-down: password manager, non-media | ❌ none (decision) | removed (no archive) |
| `watchstate` | 2026-08-30 | 2026-08-30 slim-down: watch-state tracking, non-essential | ❌ none (decision) | removed (no archive) |
| `cleanuparr` | 2026-08-29 | Torrent-only; no SABnzbd/Usenet client, so nothing to monitor in the Usenet-only stack | ✅ `cleanuparr-sabnzbd-watch.yml` | removed (no archive) |
| `bazarr` | 2026-09-06 | Re-retired by decision: subtitle acquisition not worth an always-on container in the 8-service target; consumers (Plex, Sonarr/Radarr) unaffected | ❌ none (decision) | removed (config deleted) |
| `uptime-kuma` | 2026-08-29 | Dropped from expansion scope by decision (image still CVE-blocked at the time) | ❌ none (decision) | removed (no archive) |
| `n8n` | 2026-08-29 | Workflow automation removed by decision — Discord notifications are handled by alertmanager/CrowdSec hooks; no workflow glue needed | ❌ none (decision) | removed (no archive) |
| `control-panel` (Django) | 2026-08-27 | Django backend superseded — fish functions call services directly, landing page probes via nginx | ❌ none (archived) | `archive/control-panel/` |

## Re-adopted services

### bazarr (2026-09-03 → re-retired 2026-09-06)

Bazarr was removed in the 2026-08-30 slim-down after crash-looping OOM at a 128m
cap, with subtitles deemed non-essential. It returned on 2026-09-03 as a fresh,
tracked implementation (not a copy of the old config): `ghcr.io/hotio/bazarr:release-1.6.0`,
768m cap (4.4× the failed 128m; 174MiB steady-state observed on this host),
healthchecked via the unauthenticated `/ping` endpoint, mounted **read-only** over
the FUSE mount and media trees (`:rslave`) so it can never write into mount
internals, and deliberately *without* a `depends_on` on `nzbdav_rclone` — it only
reads media paths to write subtitle files beside them, so mount-owner restarts
(landmine #2) and NzbDAV recreations (landmine #4) do not touch it.

**Re-retired 2026-09-06 by owner decision.** The 2026-09-03 implementation worked
as designed (subtitle search/download verified end-to-end against OpenSubtitles.com),
but an always-on subtitle container did not earn its place in the 8-service target
composition. Removal was exhaustive per the landmine-#7 checklist: compose service,
`config/bazarr/`, health-check leg, backup manifest, DB-growth/digest gates,
validator directory list, `audit_residue.py` registry, docs, and the Trivy
baselines. Historical documents (`CHANGELOG.md`, `HISTORY.md`, `TODO.md`, the
2026-08-30 incident narrative above) keep their original mentions — they record
what happened, not the current state.

If subtitles are ever wanted again, the 2026-09-03 record below and the API
notes in git history (`docs/API.md` @ pre-2026-09-06) document the working
configuration (form-encoded `settings-<section>-<field>` mutation contract,
`ip=sonarr`/`radarr` container-DNS addressing, empty `base_url`,
`mustNotContain` required in language-profile payloads).

The 2026-09-03 adoption record is preserved verbatim below for that purpose.

### bazarr (2026-09-03 adoption record — historical)

Returned as a fresh, tracked implementation: `ghcr.io/hotio/bazarr:release-1.6.0`,
768m cap, healthchecked via `/ping`, read-only over the FUSE mount and media
trees (`:rslave`), no `depends_on` on `nzbdav_rclone`. The OOM-class failure
that caused the original retirement is guarded by the cap plus the monthly
digest's DB/footprint gates. Removed from the "Retired services" table and
`RETIRED_SERVICES` at the time (reversed 2026-09-06).

## Services with an active re-adoption watcher

### cleanuparr

- **Why it was retired:** Cleanuparr v2.x only supports torrent clients
  (qBittorrent / Deluge / Transmission / rTorrent). This stack is Usenet-only, and with
  no SABnzbd-compatible client there was nothing for it to monitor, so it was removed
  end to end (commit `4604423`).
- **Watcher:** [`.github/workflows/cleanuparr-sabnzbd-watch.yml`](../../../.github/workflows/cleanuparr-sabnzbd-watch.yml)
  — daily 06:23 UTC + manual dispatch. It reads the latest upstream
  `Cleanuparr/Cleanuparr` release; when the release notes mention SABnzbd/Usenet it
  opens a `cleanuparr-adopt` issue in this repo (with the full re-adoption checklist)
  and posts a Discord alert. Idempotent — one alert per adoption issue; a stale open
  issue is closed if upstream stops reporting the feature as shipped.
- **Choosing the signal matters:** the upstream feature request (#137) is closed as
  **`not_planned`** and its duplicates (#263/#273) closed as duplicates, so **issue
  state is intentionally not used** as a shipped signal — only release-notes text is
  trusted. Open SABnzbd PRs are reported as an informational in-flight note, not an alert.
- **To adopt when it fires:** follow the checklist in the `cleanuparr-adopt` issue — it
  mirrors the removal in reverse (compose service, NzbDAV SABnzbd-compatible download
  client at `nzbdav:3000`, `CLEANUPARR_URL`, landing page registry, docs/tests, trivy
  baseline).

## Retired by decision (no watcher)

### 2026-08-30 slim-down (extreme, with Seerr and Unpackerr retained)

After hours of instability (Bazarr OOM crash-loop, Radarr API 500s from an orphaned
quality-profile reference and 1GB DB with MediaInfo blobs, ~19Gi of mem caps against
22Gi host RAM), the stack was pared from 29 configured services to 8 by decision. The retained services are
`prowlarr`, `radarr`, `sonarr`, `nzbdav`, `nzbdav_rclone`, `seerr`, `plex`, and
`unpackerr`.
Removed end to end: the observability stack (loki, promtail, grafana, prometheus,
alertmanager, cadvisor, node-exporter, nzbdav-exporter), the security stack (traefik,
crowdsec, adguard), non-media utilities (vaultwarden, watchstate), long-tail
acquisitions (lidarr, readarr, audiobookshelf, komga), subtitles (bazarr, which was
crash-looping OOM at 128m), and the dashboard/portal pair (arr-dashboard, landing-page)
plus metacache. Seerr and Unpackerr were intentionally retained for request
handling and automatic extraction. Traefik was dropped in favor of direct host
ports; env vars
(`TRAEFIK_DASHBOARD_AUTH`, `LIDARR_API_KEY`, `READARR_API_KEY`, `TMDB_KEY`, `TVDB_KEY`,
`METACACHE_API_KEY`, `WS_*`, `ADGUARD_*`, `VAULTWARDEN_ADMIN_TOKEN`,
`DISCORD_WEBHOOK_URL`, `GRAFANA_*`, `OMDB_KEY`, `MDBLIST_KEY`, and `FANART_KEY`)
and their fish functions, config/data
dirs, docs pages, and test entries were removed. Companion fixes shipped with the
slim-down: Radarr movie 60308's orphaned quality-profile-17 reference was repaired,
MediaInfo blob pruning was identified, and Bazarr's OOM was resolved by removal.
Re-adoption of any retired service is a fresh, tracked implementation, not a copy.

### uptime-kuma

Removed from the 10-service expansion by decision (commit `ad04cf8`) while its image
was still CVE-blocked, rather than gated behind a re-scan. All files — workflow, docs,
spec references, and local images — were removed. There is no watcher and no plan to
return unless monitoring needs change; the service count in AGENTS.md reflects this.

### control-panel (Django)

The Django control panel was removed in Phase 4 (commit `faf4127`) after its role was
superseded: fish functions call the *arr/dashboard APIs directly, and the landing page
probes health through nginx. Its files live in `archive/control-panel/` for reference.
No watcher — this was a deliberate teardown, not a hold for return.

### n8n

Workflow automation was removed by decision (2026-08-29) — no workflows had shipped in
production (the Discord-notifications first workflow from spec §10 Q2 was never built
or activated), and Discord alerting is already covered by alertmanager + CrowdSec
hooks, so the container, config dir, landing page entry, docs, and env vars were
removed end to end. No watcher — if workflow glue is ever needed again, re-adopt
from scratch.

> **Convention:** `archive/` content is reference material and is never revived into the
> active stack; any return must be a fresh, tracked implementation, not a copy.

## How to add a re-adoption watcher

When retiring a service that could return if an external condition clears:

1. **Pick a reliable "so it can come back" signal** — prefer an objective upstream
   artifact (release-note text, a shipped image tag with known-good CVE state). Avoid
   signals that lie: an issue being *closed* is not proof of shipping (it may be
   `not_planned` or a duplicate).
2. **Make it idempotent** — open one tracking issue per detection and reuse it, so an
   always-true signal alerts once, not every run.
3. **Escalate through the stack's alert channel** — Discord via the existing
   `secrets.DISCORD_WEBHOOK_URL` pattern (skip gracefully when unset).
4. **Mirror the removal in the adoption checklist** so the fix is mechanical.
5. Wire the workflow into `docs/ci-cd.md` and run `actionlint` — every workflow is
   actionlint-gated in CI.
