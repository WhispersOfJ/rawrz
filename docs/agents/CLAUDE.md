# CLAUDE.md — Movie / TV RPG (RAWRZ component)

> **⚠ Superseded items — see `rawrz-megastack-spec.md` §10 and the banner in
> `docs/rpg/movie-rpg-spec.md`.** This RPG is now a RAWRZ component, not a separate
> project. It will be a Compose service in the merged stack (master spec D34).

## What this is

A web-based RPG where watching movies/TV from the RAWRZ stack (Plex + Sonarr +
Radarr) is the core mechanic. Detective/investigation theme. Part of RAWRZ —
reads its APIs and will run as a Compose service.

- Spec: `docs/rpg/movie-rpg-spec.md` (source of truth for design decisions;
  supersession banner applies).
- Status: v0.0.0.1 (initial spec release). V1 single-player, V2 shared quests.
- Stack reference: `~/Cave` (Bear Cave), RAWRZ root `AGENTS.md` links to
  `docs/rpg/movie-rpg-spec.md`.
- Source location: `backend/rpg/` in the RAWRZ monorepo.

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
- Owns: Postgres (host install, not a compose container — **⚠ superseded:**
  RAWRZ adds shared Postgres per master spec §7), Rust/Axum backend,
  Svelte frontend, port 46532, PIN gate.
- **Will be:** a Compose service in the RAWRZ stack (master spec D34), on the
  `bearcave` network, with Redis (master spec D7–D12) available for sessions.

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
