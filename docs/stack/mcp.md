# MCP Servers in Freebuff Desktop

How Model Context Protocol (MCP) servers are configured in the Freebuff Desktop app,
which ones are worth adding for this stack, and the exact steps for each path.

---

## How Freebuff Desktop wires MCP servers

Freebuff Desktop (a Codebuff/Claude-engine app) manages MCP servers through
**Settings → Connectors**, backed by a single file:

- **`~/.agents/mcp.json`** — the source of truth, shared with the CLI. The
  app reads and writes this file; the Connectors UI is a thin editor over it.
- **Nothing runs until you approve it.** Pasting an `mcpServers` block only
  writes the file. The Electron main process (`mcp-consent-bridge`) raises a
  native dialog before any third-party program is spawned, with per-server
  controls for launch approval, tool approval, enable/disable, sign-in, and
  removal.
- **Secrets are encrypted at rest** in the app's secret store
  (`mcp-secrets.json` in the app userData dir), keyed by the OS keychain.
- **Kill switch:** `FREEBUFF_MCP_DISABLED=1` disables the consent bridge
  entirely.

> **Note:** `mcpServers` entries in `~/.claude.json` belong to Claude Code's
> own store and are **not** read by Freebuff. Freebuff only reads
> `~/.agents/mcp.json`.

### The empty state

With no `mcp.json` yet, the Connectors modal shows:

> *No servers configured yet. Add one to `~/.agents/mcp.json`, then reload.*

---

## Current state (2026-08-29)

Three servers are configured in `~/.agents/mcp.json` and approved+enabled in
the running Freebuff Desktop:

| Server | Tools | Role |
|--------|-------|------|
| **context7** 4.0.4 | 2 (`resolve-library-id`, `query-docs`) | Up-to-date docs for the stack's tools (Docker Compose, Traefik v3, Next.js, .NET, Python, Prometheus/Grafana, rclone) |
| **playwright** 1.63.0-alpha | 24 (navigate, click, fill_form, screenshot, snapshot, …) | Browser automation |
| **chrome-devtools** 1.8.0 | 29 (page control, network, console, lighthouse_audit, performance traces, …) | Deep browser/devtools inspection |

Verify any time with the probe:

```bash
python3 scripts/check_mcp.py --freebuff
```

Each server should report `enabled=True status=connected approvedLaunch=True
approvedTools=True`.

---

## Recommended: Context7

**Verdict: add it.** Context7 serves up-to-date, version-specific documentation
for the exact tools this stack runs — Docker Compose, Traefik v3, Next.js,
.NET, Python, Prometheus/Grafana, rclone, and more. It needs no API key for
basic use and fixes the most practical gap in agent sessions: model knowledge
lags current library docs.

### Via the Connectors UI (recommended)

1. Open Freebuff Desktop → **Settings** → **Connectors**.
2. In the paste box, enter:

   ```json
   {
     "mcpServers": {
       "context7": {
         "command": "npx",
         "args": ["-y", "@upstash/context7-mcp@latest"]
       }
     }
   }
   ```

   (The modal accepts either the full block above or the `context7` entry
   alone; the app writes the result into `~/.agents/mcp.json`.)
3. The server is written but **not run** — approve the launch dialog, then the
   tool-approval dialog, then **reload**.
4. Verify in a session: ask for current Docker Compose or Traefik v3 docs and
   confirm the `context7` tools answer with up-to-date content.

### Via the file (equivalent)

1. Create `~/.agents/mcp.json` (merge if it already exists):

   ```json
   {
     "mcpServers": {
       "context7": {
         "command": "npx",
         "args": ["-y", "@upstash/context7-mcp@latest"]
       }
     }
   }
   ```

2. In Freebuff: **Settings → Connectors → Reload**, then approve the launch
   prompt.

Both paths land on the same file; the consent dialog is the safety gate before
`npx` ever runs.

---

## Research: MCP servers evaluated for this stack

| Server | Verdict | Why |
|--------|---------|-----|
| **Context7** (`@upstash/context7-mcp`) | ✅ **Add** | Up-to-date, version-specific docs for the stack's tools. No API key needed for basic use. Fixes stale-docs gap in agent sessions. |
| **Docker MCP** (`docker-mcp` via `uvx`) | ⚠️ Skip | Container/compose/log management, but every session already has terminal access to `docker compose`/`docker ps` directly — redundant. Worse, a spawned MCP server holding the Docker socket widens the exact attack surface [HISTORY.md](../HISTORY.md#control-panel-django--archived) (writable socket, CRITICAL) is trying to close. |
| **Homelab MCP** (bjeans unified server) | ❌ Skip | Docker/Podman + Pi-hole/Unifi/Ollama; only the Docker slice overlaps, and that is the redundant-and-risky part. |
| **Filesystem MCP** | ❌ Skip | Native file read/write/search tools exist in every session. |
| **GitHub MCP** | ❌ Skip | Covered by the authenticated `gh` CLI (see `~/ECC` policy audit). |
| **Playwright / chrome-devtools MCP** | ✅ **Added** | Browser automation beyond Freebuff's native preview tools (form filling, network/console inspection, Lighthouse audits). Previously skipped as redundant with the native preview; added 2026-08-29 when full browser control in sessions became desirable. |
| **Postgres / Supabase MCP** | ❌ Skip | This stack is SQLite-only — nothing to connect to. |
| **Stackarr** (media-stack control plane + MCP) | ⚠️ Watch | Purpose-built for Radarr/Sonarr/Plex orchestration — typed, approval-gated Docker actions. See [docs/stackarr.md](stackarr.md) for the full evaluation and adoption path. |

---

## Reproducing the approval (API flow)

The Connectors UI does this by hand; the same steps can be driven against the
running orchestrator's API so the setup is reproducible. The orchestrator's
API port and consent token are per-boot (same-user read from `/proc`; the
`scripts/check_mcp.py` probe locates them automatically).

1. **Write the config** — add the server to `~/.agents/mcp.json`:

   ```json
   {
     "mcpServers": {
       "playwright": {
         "command": "npx",
         "args": ["-y", "@playwright/mcp@latest"]
       }
     }
   }
   ```

2. **Reload** so the running orchestrator picks it up (it does not watch the
   file): `POST /api/mcp/reload`. The server now lists with
   `status=awaiting_launch_approval`.

3. **Approve the launch** — `POST /api/mcp/servers/{id}/approve-launch` with
   `{}`. The response contains the tool manifest **and** a `manifestHash`.

4. **Approve the tools** — `POST /api/mcp/servers/{id}/approve-tools` with
   `{"allowedTools": [<all tool names>], "manifestHash": <hash from step 3>}`.
   ⚠️ **Gotcha:** the hash is only valid paired with the exact manifest from
   that same `approve-launch` response. Do not re-fetch the tool list between
   steps 3 and 4, or the call fails with
   `"changed its tools while you were choosing"` — re-run step 3 and pass its
   fresh hash straight through.

5. **Enable** — `POST /api/mcp/servers/{id}/enable` with `{"enabled": true}`.

6. **Verify** — `python3 scripts/check_mcp.py --freebuff`; the server should
   report `enabled=True status=connected` with its tool count.

---

## Adding any other server

The same JSON shape works for every server. Paste its `mcpServers` block into
the Connectors modal (or merge into `~/.agents/mcp.json`), approve the launch,
and reload. Common shapes:

```json
{
  "mcpServers": {
    "name": {
      "command": "npx",
      "args": ["-y", "package-name@latest"]
    }
  }
}
```

```json
{
  "mcpServers": {
    "name": {
      "command": "uvx",
      "args": ["package-name"]
    }
  }
}
```

Remote (HTTP/SSE) servers instead use `"url": "https://host/path"` with optional
OAuth via the sign-in control.
