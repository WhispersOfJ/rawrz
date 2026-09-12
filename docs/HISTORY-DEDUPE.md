# History — Spec Deduplication Record

> **Why this file exists:** M0 imported three repositories, two of which carried
> their own copies of the Movie/TV RPG spec. This file records the three copies,
> their sizes, and why only one survived in the RAWRZ tree.

## The three copies

| Copy | Location (pre-M0) | Size | Fate |
|------|-------------------|------|------|
| **Canonical** | `WhispersOfJ/movie-rpg` root (`movie-rpg-spec.md`) | ~176 KB | Imported to `docs/rpg/movie-rpg-spec.md` — the survivor |
| **Stale (Bear Cave)** | `WhispersOfJ/TheBearCave` root (`movie-rpg-spec.md`) | ~160 KB | Deleted during import — superseded by the canonical copy |
| **Out-of-tree (old)** | `~/movie-rpg-spec.md` (host home, not in any repo) | ~25 KB | Never in a repo; recorded here for the historical record only |

## Why the canonical copy won

- The `movie-rpg` repo is the RPG's origin — its spec is the source of truth for
  design decisions, reviewed 2026-09-10, with the wizard-academy rebrand finalized.
- The Bear Cave's copy was a stale snapshot carried from an earlier link between the
  two repos; it was missing the finalized presentation and rules contract.
- The out-of-tree `~/movie-rpg-spec.md` was a personal scratch copy, never committed.

## What's in tree now

Exactly one copy: `docs/rpg/movie-rpg-spec.md`. The other two exist only in the
import source repositories (which remain untouched) and in this record.

## Other deduplication performed in M0

| Item | Sources | Survivor |
|------|---------|----------|
| `.release-please-config.json` + manifest | Bear Cave + movie-rpg + cave-deck | One at root, Bear Cave's line continued |
| `LICENSE` + `CODE_OF_CONDUCT.md` | Bear Cave + movie-rpg (both MIT) | One each at root |
| `.gitignore` | Bear Cave + movie-rpg + cave-deck | Union at root |
| `CLAUDE.md` (agent contract) | Bear Cave (root) + movie-rpg (`backend/rpg/`) | Bear Cave's content → `docs/stack/AGENTS.md`; RPG's → `docs/agents/CLAUDE.md` with supersession notes; fresh unified root `AGENTS.md` authored |
| `CONTRIBUTING.md` | Bear Cave (root) + movie-rpg (`backend/`) | Root rewritten for monorepo |
| `README.md` | Bear Cave (root) | Root rewritten for RAWRZ |
| `MIGRATION.md` | (new) | Root, records the import provenance |
