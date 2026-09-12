# Cave Deck

Custom web GUI for the `thebearcave` media stack — replaces the TUI, all `stack-*`
bash functions, the fish host tools, waybar integration, and the operational
Python scripts. Spec of record: `thebearcave/cave-deck-spec.md` (this repo is its
§3.1 implementation home).

**Stack:** Rust (axum) backend · React 18 / TypeScript / Vite frontend · SQLite
(job/audit/event state) · single compose service on port **7780**, LAN-only.

## Structure

```
backend/          axum server — REST + WebSocket (spec Appendix D is the contract)
frontend/         React SPA served as embedded static assets by the backend binary
catalog/          catalog.yaml (100 curated containers) + retired-registry.lock + validator
parity/           parity.yaml — every retired function → GUI feature ID (spec Appendix C)
host-shim/        (M3+) privileged systemd helper the backend calls over a Unix socket
.github/          CI (SHA-pinned actions), release-please, dependabot
```

## Feature IDs

Every route is tagged with the feature IDs it serves (header `x-cave-deck-features`)
and every ID must be served — `catalog/validate.py` and the `api-contract` CI job
enforce both directions against `parity/parity.yaml` (spec §D.4/D.6).

## Development

```bash
# backend
cd backend && cargo run            # axum on :7780 (stub mode — no docker sock needed)
cargo test && cargo clippy && cargo fmt --check

# frontend
cd frontend && npm install
npm run dev                        # vite dev server, proxies /api to :7780
npm test && npm run build          # vitest + tsc + production bundle

# catalog
python3 catalog/validate.py        # schema + port collisions + retired-registry
python3 scripts/check_api_contract.py backend/src frontend/src parity/parity.yaml
```

## Deployment (M0)

Compose service `cave-deck` in `thebearcave` (port 7780, `bearcave` network,
binds docker.sock + repo root). Images published to GHCR by digest on release.
