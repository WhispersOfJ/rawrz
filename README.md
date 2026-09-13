# RAWRZ

**A home media megastack — stack + deck + RPG in one repository, one release stream, one CI.**

RAWRZ merges three codebases into a single repository:

| Component | What it is | Location |
|-----------|-----------|----------|
| **Stack** (The Bear Cave) | 8 media services plus the nginx internal TLS ingress — Prowlarr, Radarr, Sonarr, NzbDAV, nzbdav_rclone, Seerr, Plex, Unpackerr — plus ImageMaid and Recyclarr on the manual `maintenance` profile. Direct application ports remain rollback paths. | `docker-compose.yml`, `docs/stack/`, `services/` |
| **Deck** (Cave Deck) | A web GUI replacement for the stack's interactive surface — Rust/axum backend + React/TS/Vite frontend, a 100-container catalog, port 7780, LAN-only, no login. Not yet in Compose; see the spec. | `backend/deck/`, `docs/deck/`, `catalog/` |
| **RPG** (Movie/TV RPG) | A web-based RPG where watching movies and TV is the core mechanic — Rust/Axum backend + Svelte frontend, PostgreSQL, port 46532, PIN gate. Not yet in Compose; will be a future stack service. | `backend/rpg/`, `docs/rpg/` |

> **M3 status:** Redis, the Seerr fork, and nginx internal ingress are shipped.
> Postgres and the RPG/Deck Compose services remain later milestones.

## At a glance

| Metric | Value |
|--------|-------|
| Always-on containers | **9** including nginx (`docker compose ps`) |
| Future services | Postgres, Cave Deck, and RPG |
| Acquisition apps | 2 — Radarr (movies), Sonarr (TV) |
| Download client | NzbDAV (InfiniDysk) — SABnzbd-compatible |
| Media libraries | Movies, Shows |
| Requests | Seerr → Radarr/Sonarr/Plex watchlists |
| Manual maintenance | ImageMaid PhotoTranscoder cache cleanup, Recyclarr TRaSH profile sync (both profile-gated) |
| Memory caps | ≈12.2 GiB total including nginx |

## Quick start

```bash
# 1. Clone and configure
git clone https://github.com/WhispersOfJ/rawrz.git
cd rawrz
cp .env.template .env   # edit with real values (see .env.template)

# 2. Prepare runtime directories and start the stack
./scripts/setup.sh
docker compose config --quiet
docker compose up -d
docker compose ps
```

> **Linux only.** FUSE mount semantics and Plex host networking assume a Linux host.
> Docker Desktop for macOS/Windows is not supported.

## Documentation

| Doc | What it covers |
|-----|----------------|
| [`rawrz-megastack-spec.md`](rawrz-megastack-spec.md) | **Master spec** — the RAWRZ design, decisions, and roadmap |
| [`docs/plans/`](docs/plans/) | Companion plans: M0 migration, Postgres hardening, *arr DB migration, Seerr cache patch |
| [`docs/stack/`](docs/stack/) | Stack docs — architecture, landmines, CI/CD, security, quick-start, API map, per-service docs |
| [`docs/deck/`](docs/deck/) | Cave Deck spec — web GUI design, catalog, features |
| [`docs/rpg/`](docs/rpg/) | RPG spec, changelog, contributing (**⚠ supersession banner applies**) |
| [`docs/agents/`](docs/agents/) | RPG operational docs — FIX.md, HANDOFF.md, CLAUDE.md |

## Testing

```bash
docker compose config --quiet          # compose validation
bash -n scripts/*.sh tests/*/*.sh      # shell syntax
./tests/bash/test_bash_functions.sh --offline   # bash port smoke tests
python3 -m ruff check .                  # Python lint (excl. archive/)
```

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full rules: worktree discipline,
Conventional Commits, PR/lint conventions, and the validation checklist.

The authoritative agent contract is [`AGENTS.md`](AGENTS.md) — read it first if you're
an AI coding agent or a human who wants the full operational reference.

## License

MIT — see [LICENSE](LICENSE). Third-party images/services keep their own licenses.
