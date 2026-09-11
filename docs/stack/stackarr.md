# Stackarr — Evaluation for Future Adoption

Status: **watch, do not adopt yet.** Written 2026-08-29.

---

## What it is

[Stackarr](https://stackarr.app) (github.com/polyphonic/stackarr, GPL-3.0-only) is
an agentic control plane for self-hosted apps: a Docker container that exposes a
Model Context Protocol (MCP) server so agents can manage the media stack from
chat. It is the closest thing to purpose-built agent control for this exact
stack.

**Managed services (overlap with this stack bolded):** **Sonarr, Radarr,
Prowlarr, Plex, Seerr**, Jellyfin, Immich,
RomM, BookOrbit, Transmission, qBittorrent, Postgres, Cloudflare.

**MCP design (its main strength):**

- Typed app actions, **not** a generic shell or unrestricted Docker tool.
  Actions are allowlisted per app; disabled apps contribute no tools.
- Authority profiles set at client start — `observe` (read-only), `manage`
  (everyday ops with approval prompts), `admin` (setup/config), `unrestricted`
  (full autonomy). Agents cannot promote themselves.
- Local stdio transport via `docker exec` into the private container; no public
  MCP port by default. Remote `/mcp` endpoint exists but is disabled by default
  and requires a revocable bearer token.
- Redacted activity history for auditing what an agent changed.

---

## Why it's tempting for this stack

- **On-target scope:** it manages the core of this stack — Radarr, Sonarr,
  Prowlarr, Plex, Seerr, and backups — with typed
  actions instead of raw API calls.
- **It answers the Docker-socket problem better than raw Docker MCP servers:**
  a typed, approval-gated action surface is far safer than giving an agent a
  generic `docker exec`. This is the same CRITICAL finding
  ([HISTORY.md](../HISTORY.md#control-panel-django--archived) — the writable-socket finding) that killed the control-panel's writable socket — but
  Stackarr's design is the socket *with guardrails*.
- **Chat-led setup** can dry-run before applying anything (the exact
  cautious-operations posture this repo requires).

---

## Why not to adopt yet

| Concern | Detail |
|---------|--------|
| **Early access** | `0.3.0-alpha.19` (2026-08), project ~1 month old. Moving `:alpha` image tag; only an exact-version tag is reproducible. |
| **Docker socket still required** | `docker-compose.yml` mounts `/var/run/docker.sock` — host-level access. The guardrails mitigate but do not remove the exposure. Must stay loopback-bound until auth is configured. |
| **Overlap with arr-dashboard** | The existing arr-dashboard already covers queue/calendar/history, TRaSH guides, cleanup, auto-hunting, Plex analytics, notifications. Stackarr would need a defined division of labor. |
| **Postgres in its managed set** | This stack is deliberately SQLite-only; adopting Stackarr's Postgres-inclusive model is a philosophy change, not a bolt-on. |
| **Supply-chain posture** | New alpha project with no track record against this repo's CVE-gate and digest-pinning requirements (image pinning commit `ad04cf8`). |
| **Value vs. cost for a single operator** | The stack's operator already has direct terminal + API access to every app. The marginal value of chat-driven control is real but small until it reaches 1.0. |

---

## What adoption would take

When it matures (re-evaluate at ≥1.0, or when it shows sustained release
cadence), adoption is a standard expansion per [HISTORY.md](../HISTORY.md#the-2026-08-28-expansion):

1. **Compose entry** — new `stackarr` service on port 7777, bind to loopback
   first (`STACKARR_BIND_IP=127.0.0.1`), mount `/var/run/docker.sock`,
   pin `polyphonic/stackarr:<exact-version>` (no `:alpha`), `.env` vars.
2. **Security review** — confirm the socket mount against the writable-socket
   guidance in [HISTORY.md](../HISTORY.md#control-panel-django--archived); keep the MCP endpoint local-stdio only; no Traefik
   exposure of the dashboard until authentication exists.
3. **CVE gate** — add the pinned image to the trivy scan workflow's image
   list and the `.github/trivy-baseline.json`; it must pass the CRITICAL gate
   like every other image.
4. **Registry + landing page** — add to    the active service registry and documentation, plus the AGENTS.md service
   table and port map.
5. **MCP wiring** — generate the client entry
   (`docker exec app /app/bin/stackarr mcp config claude --profile manage`)
   and add the resulting `mcpServers` block to `~/.agents/mcp.json` per
   `docs/mcp.md`.
6. **Docs** — `docs/services/stackarr.md` + a row in the README docs index.
7. **Rollback plan** — because it touches the Docker socket, the removal
   checklist must be exhaustive (compose, config, env, registry, traefik
   labels), per AGENTS.md landmine 8.

---

## Verdict

**Watch.** The design is the right answer to the biggest objection (typed,
approval-gated Docker access instead of a raw socket), and its managed-service
list maps almost 1:1 onto this stack. The blockers are maturity
(0.3.0-alpha, one month old) and philosophy drift (Postgres, another control
surface). Revisit when it reaches 1.0 or shows six months of stable releases —
at that point the adoption path above is a single standard expansion.
