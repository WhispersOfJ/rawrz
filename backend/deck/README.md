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
host-shim/        (M3+) privileged systemd helper the backend calls over a Unix socket
```

After the RAWRZ M0 relocation, the shared catalog lives at the repository-root
`catalog/`, the parity contract at `parity/parity.yaml`, and the unified CI +
release configuration at the repository root (this directory no longer carries
its own `.github/` or release-please files).

## Feature IDs

Every route is tagged with the feature IDs it serves (header `x-cave-deck-features`)
and every ID must be served — the root `catalog/validate.py` and
`scripts/check_api_contract.py` and the unified CI `api-contract` step enforce
both directions against `parity/parity.yaml` (spec §D.4/D.6).

## Development

```bash
# backend
cd backend && cargo run            # axum on :7780 (stub mode — no docker sock needed)
cargo test && cargo clippy && cargo fmt --check

# frontend
cd frontend && npm ci               # Node 20+ (package-lock.json is authoritative)
npm run dev                        # vite dev server, proxies /api to :7780
npm test -- --run && npm run build # vitest + tsc + production bundle
```

From the RAWRZ repository root:

```bash
python3 catalog/validate.py        # schema + port collisions + retired-registry
python3 scripts/check_api_contract.py
```

## Deployment (M0)

Compose service `cave-deck` in `thebearcave` (port 7780, `bearcave` network,
binds docker.sock + repo root). Images published to GHCR by digest on release.
