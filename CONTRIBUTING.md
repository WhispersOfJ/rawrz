# Contributing to RAWRZ

Thanks for wanting to contribute! These guidelines explain how to open good
issues and well-formed pull requests for this repository, what the review
process looks like, and how to report problems.

This repository is governed by the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you agree to uphold it.

## Table of contents

- [Before you start](#before-you-start)
- [Reporting bugs](#reporting-bugs)
- [Requesting features](#requesting-features)
- [Making changes](#making-changes)
- [Worktree discipline](#worktree-discipline)
- [Commit and PR conventions](#commit-and-pr-conventions)
- [Validation checklist](#validation-checklist)
- [Code style](#code-style)
- [Review and merge](#review-and-merge)
- [Getting help](#getting-help)

## Before you start

- **Check existing work.** Search issues and open/merged PRs before opening a
  new one.
- **Read the docs.** Start with `AGENTS.md` (the system reference) and the
  relevant component doc:
  - Stack: `docs/stack/` and `docs/stack/services/`
  - Deck: `docs/deck/cave-deck-spec.md`
  - RPG: `docs/rpg/movie-rpg-spec.md` (⚠ supersession banner applies)
  - Master spec: `rawrz-megastack-spec.md`
- **Know the scope.** RAWRZ is a monorepo containing three components (stack,
  Deck, RPG) with strict CI. Changes must keep all three components buildable
  and the docs honest.

## Reporting bugs

Use the **Bug report** issue template (`.github/ISSUE_TEMPLATE/bug_report.yml`).
Include: what you did, what you expected, what happened, which component(s) were
involved, how to reproduce, relevant logs (scrub credentials first), and versions.

Before filing, check the known failure modes in `docs/stack/landmines.md` and
`AGENTS.md` (Critical Landmines).

## Requesting features

Use the **Feature request** issue template
(`.github/ISSUE_TEMPLATE/feature_request.yml`). Describe the problem you want
solved, not just a solution, and note which component or area it affects.

## Making changes

Every change flows through a pull request. `main` is branch-protected — nobody
pushes to it directly — and **all work happens on dedicated git worktrees, one
worktree per task**.

1. Create a task-named worktree off `origin/main` inside `.worktrees/`.
2. Make exactly this task's changes inside it — never mix unrelated work.
3. Run the [validation checklist](#validation-checklist).
4. Commit with a Conventional Commit message and push the branch.
5. Open a PR against `main`.
6. Address review feedback; keep the branch up to date with `main`.

## Worktree discipline

Mandatory. One worktree per task, named by the task, never mixed with unrelated
work. The main checkout stays clean and is used for reference only.

- Worktrees **must live inside the RAWRZ checkout** under `.worktrees/<task-name>`.
  Do not create worktrees under `/home/bear/.worktrees/`, `/home/bear/wt-*`, or
  any other external path.
- Task names are lowercase and dash-separated, optionally type-prefixed
  (`docs/`, `feat/`, `fix/`, `ci/`, `chore/`, `stack/`, `deck/`, `rpg/`).
- `git worktree list --porcelain` before creating; `git worktree prune` after
  removing.
- After the PR merges, remove the worktree and delete the branch.

Raw git:

```bash
cd /path/to/rawrz
git fetch origin main
git worktree add -b <task-branch> .worktrees/<task-name> origin/main
cd .worktrees/<task-name>
```

## Commit and PR conventions

**PR titles and commit messages must be Conventional Commits** — enforced by
the `pr-lint` workflow. Allowed types:

```
feat  fix  docs  style  refactor  perf  test  build  ci  chore  revert  docker
```

Examples: `fix(stack): bound install verification with a timeout`,
`feat(deck): add catalog install flow`, `docs(m0): consolidate sub-spec banners`.

- Release behavior: `release-please` opens a release PR only for `feat:` and
  `fix:` commits. `ci:`, `docs:`, `chore:`, etc. land silently.
- PR titles follow the same convention; the subject must not start with a space.
- Linear history: merges are squash or rebase only — never merge commits.

### Component prefixes (recommended)

- `stack:` — Docker Compose, stack scripts, stack docs, *arr/NzbDAV/Plex/Seerr
- `deck:` — `backend/deck/`, `catalog/`, Deck frontend
- `rpg:` — `backend/rpg/`, RPG docs
- `ops:` — CI workflows, GitHub config, cross-component scripts
- `docs:` — cross-component or root docs

## Validation checklist

Run these before opening a PR:

```bash
docker compose config --quiet
bash -n scripts/*.sh tests/*/*.sh
python3 -m ruff check .          # Python lint (excl. archive/)
python3 scripts/check_compose_mounts.py
./tests/bash/test_bash_functions.sh --offline
```

For Rust components:

```bash
cd backend/rpg && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets
cd ../deck/backend && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets
```

For the Deck frontend:

```bash
cd backend/deck/frontend && npm install && npm run build && npm test
```

For the catalog:

```bash
python3 catalog/validate.py
```

- `validate.yml` runs compose validation, env coverage, shellcheck, ruff, actionlint,
  Rust fmt/clippy/test for both crates, frontend typecheck/lint/build, and catalog
  validation. `nightly-healthcheck.yml` re-validates everything daily.
- Live checks (e.g. `./tests/integration/test_pipeline.sh`) run against the
  real stack on the host — never rely on them in CI.

## Code style

- **Python:** Ruff; match existing check-script conventions.
- **Rust:** `cargo fmt` + `clippy --all-targets -- -D warnings` must pass; zero
  clippy warnings is the bar for both crates.
- **Shell:** ShellCheck-clean, POSIX-ish bash; `bash -n` must pass.
- **TypeScript:** strict mode; `tsc --noEmit` + `vitest` must pass for the Deck.
- **Workflows:** third-party actions are SHA-pinned with a `# tag` comment;
  validated by `actionlint` (see [docs/stack/ci-cd.md](docs/stack/ci-cd.md)).

## Review and merge

- Branch protection requires CI checks and a Conventional Commit title.
- If `main` advances while your PR is open, rebase and push with
  `--force-with-lease` (never force-push blindly).
- Merges are squash merges via `gh pr merge <number> --squash --delete-branch`,
  followed by `git worktree remove`.
- After merge, delete the local branch if the worktree check-out kept it.

## Component-specific notes

### Stack (The Bear Cave)

- The stack is the 8-service media stack imported from `WhispersOfJ/TheBearCave`.
  M0 changes no runtime behavior.
- Docs live in `docs/stack/` (relocated from the Bear Cave's `docs/` and root
  `AGENTS.md` content).
- The landmines in `docs/stack/landmines.md` and `AGENTS.md` are load-bearing.

### Deck (Cave Deck)

- The Deck is `backend/deck/` — Rust/axum backend + React/TS/Vite frontend.
- The catalog lives at root `catalog/` (it's stack-wide, not Deck-private).
- The Deck spec is `docs/deck/cave-deck-spec.md` (⚠ supersession banner applies).
- Deck CI is merged into the unified `validate.yml`.

### RPG (Movie/TV RPG)

- The RPG is `backend/rpg/` — Rust/Axum backend + Svelte frontend.
- The RPG spec is `docs/rpg/movie-rpg-spec.md` (⚠ supersession banner applies).
- The RPG's `release-as: 0.0.0.1` override was deleted.
- The RPG's FIX.md §8 (no-Redis decision) carries a supersession banner.

## Getting help

- `stack-*` bash functions in `services/bash-functions/` — see
  `docs/stack/services/bash-functions.md`.
- Service documentation: `docs/stack/services/`; CI/CD: `docs/stack/ci-cd.md`;
  testing: `docs/stack/testing.md`.
- For security issues, follow `docs/stack/security.md` and report privately.
- Ask in a GitHub discussion or on an issue before large cross-cutting changes.
