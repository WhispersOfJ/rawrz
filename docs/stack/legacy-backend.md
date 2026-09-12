# Legacy Backend (dead stack-management dashboard)

> **Status:** Documented, not imported. Superseded by Cave Deck (`backend/deck/`).

## What it was

The Bear Cave carried an incomplete Rust `backend/src/` tree — a stack-management
dashboard intent that never reached a `Cargo.toml` or `main.rs`. It was a scratch
directory with exploratory `.rs` files (`routes.rs` with `todo!()` handlers,
`docker.rs`, `executor.rs`, `jobs.rs`, `config.rs`), never compiled, never
deployed, never committed to the Bear Cave's history (it was untracked local state).

## Why it's not in RAWRZ

1. **It was never functional.** No `Cargo.toml`, no `main.rs`, no build, no tests.
   Importing it would add dead code with no value.
2. **Its intent is superseded by Cave Deck.** The stack-management dashboard goal —
   a web GUI that manages the stack — is now Cave Deck (`backend/deck/`), which has
   a real Rust backend, a React frontend, a 100-container catalog, and a spec.
3. **Cave Deck lives in the monorepo.** With RAWRZ, the stack-management GUI is
   `backend/deck/` — same repository, real code, real CI.

## Where the intent lives now

- `backend/deck/` — the real stack-management backend (axum, Docker API via `bollard`,
  WebSocket hub, SQLite state, REST + WebSocket API)
- `docs/deck/cave-deck-spec.md` — the spec, including landmine enforcement invariants (§7)
- `backend/deck/parity/parity.yaml` — feature-ID contract mapping retired bash/fish
  functions to Deck GUI features

## What would have been imported (for the record)

| File (hypothetical) | Purpose | Fate in RAWRZ |
|---------------------|---------|---------------|
| `backend/src/routes.rs` | Stub HTTP routes with `todo!()` handlers | Superseded by `backend/deck/backend/src/routes.rs` |
| `backend/src/docker.rs` | Docker API wrappers | Superseded by Deck's Docker integration |
| `backend/src/executor.rs` | Job execution plumbing | Superseded by `backend/deck/backend/src/jobs.rs` |
| `backend/src/jobs.rs` | Job state machine | Superseded by Deck's job system |
| `backend/src/config.rs` | `.env`/compose config reading | Superseded by Deck's config handling + stack `scripts/` |

None were ever committed to the Bear Cave, so none appear in RAWRZ's imported history.
The Bear Cave's `git log -- backend/src/` returns nothing.

## Relationship to the imported RPG backend

The RPG's real backend (`backend/rpg/`) is a separate, complete, tested crate with
14 migrations, 89 unit tests, and a live scratch-Postgres proof. It is **not**
related to the dead stack-management `backend/src/` — the RPG's `backend/rpg/`
was imported from `WhispersOfJ/movie-rpg` and is fully functional.
