# CLAUDE.md — Movie / TV RPG

## What this is

A web-based RPG where watching movies/TV from the Bear Cave stack (Plex +
Sonarr + Radarr) is the core mechanic. Detective/investigation theme. Linked
with the stack (reads its APIs) but not a part of it (no new compose
container, separate lifecycle).

- Spec: `movie-rpg-spec.md` (source of truth for design decisions).
- Status: v0.0.0.1 (initial spec release). V1 single-player, V2 shared quests.
- Stack reference: ~/Cave (Bear Cave), AGENTS.md links to `movie-rpg-spec.md`.

## How to work

- Spec-first: update `movie-rpg-spec.md` before implementing design changes.
- Commit style: conventional commits (feat/fix/docs/chore).
- Release: release-please, semver 0.0.0.1, token in `RELEASE_PLEASE_TOKEN`
  GitHub Actions secret. Do not commit the token.
- Secrets: never commit `.env` / real values. RPG reuses Bear Cave `.env`
  secrets plus `RPG_DB_URL`; all gitignored.

## Stack linkage (explicit)

- Reads: Plex (X-Plex-Token, :32400), Sonarr (X-Api-Key, :8989),
  Radarr (X-Api-Key, :7878) over LAN.
- Owns: Postgres (host install, not a compose container), Rust/Axum backend,
  Svelte frontend, port 46532, PIN gate.
- Does NOT: add a container to docker-compose.yml, modify *arr/Plex services,
  expose remotely by default, use webhooks (polling only, 5-min cadence).

## Open design calls (finalize during implementation)

See §12 + §5.1/§5.2/§6.4 of `movie-rpg-spec.md`. Most values are now concrete
in the spec; remaining "finalize during implementation" items are explicit in
the spec and should be resolved there as they're decided, not silently.

## Inspiration & thanks

- **Legends of the Green Dragon (LoGD)** — https://www.lotgd.net/ — a design inspiration
  source (daily loop, fame/renown signal, holiday modules, host/module model, onboarding
  primer, genre/race/specialty flavor, rank ladder, new-game+ dragon cycle) and a model for
  how a host ships/modularizes features. LoGD is a remake/homage of Seth Able's **Legend of
  the Red Dragon (LoRD)** (a BBS door game). LoGD is **not** a technical dependency of this
  RPG (this RPG is Rust/Axum + Postgres, not PHP/MySQL); it is acknowledged here as a design
  inspiration only. Module catalog: https://www.lotgd.net/about.php?op=listmodules ·
  primer: https://www.lotgd.net/petition.php?op=primer .
