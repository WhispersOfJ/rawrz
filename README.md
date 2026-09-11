# RAWRZ

RAWRZ is the monorepo migration of the Bear Cave media stack, Movie/TV RPG, and Cave
Deck control plane.

## Current milestone: M0

M0 preserves the current root Compose runtime and delivers the repository skeleton,
source history, unified documentation, and one CI/release surface. It does **not** add
runtime services or change the live media stack. Redis, Postgres, nginx, observability,
and application containerization belong to later, separately validated milestones.

## Components

| Component | Current location | Purpose |
|---|---|---|
| RAWRZ Stack | root `docker-compose.yml`, `services/`, `scripts/` | Eight-service Usenet/media stack |
| RAWRZ RPG | `backend/rpg/` | Rust/Axum progression engine and Postgres-backed RPG state |
| RAWRZ Deck | `backend/deck/` | Rust/Axum control-plane backend and React frontend |
| Shared catalog | `catalog/` | Curated container catalog and retired-service validation |

The current stack remains the operational eight-service Bear Cave deployment. The
RAWRZ target architecture is described in [`rawrz-megastack-spec.md`](rawrz-megastack-spec.md).

## Repository map

- [`AGENTS.md`](AGENTS.md) — unified engineering and agent contract
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — PR, validation, and release rules
- [`MIGRATION.md`](MIGRATION.md) — source refs, rewritten history, and provenance
- [`docs/stack/`](docs/stack/) — current stack operations and service documentation
- [`docs/rpg/`](docs/rpg/) — RPG specification and component history
- [`docs/deck/`](docs/deck/) — Deck specification and ADRs
- [`docs/agents/`](docs/agents/) — imported RPG review and handoff lineage
- [`docs/plans/`](docs/plans/) — M0 and later migration runbooks

## Development checks

```bash
cp .env.template .env                 # use placeholders for local validation
docker compose config --quiet
python3 catalog/validate.py
python3 scripts/check_api_contract.py
python3 scripts/check_secret_manifest.py
python3 scripts/check_secret_drift.py
bash -n scripts/*.sh tests/*/*.sh
./tests/bash/test_bash_functions.sh --offline

(cd backend/rpg && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets)
(cd backend/deck/backend && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets)
(cd backend/deck/frontend && npm ci && npm test -- --run && npm run build)
```

Do not use real credentials in `.env` during CI or tests. The RPG scratch proof must
only target a disposable database named with `rpg_scratch`.

## License

MIT — see [`LICENSE`](LICENSE).
