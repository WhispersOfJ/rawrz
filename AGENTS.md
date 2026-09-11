# AGENTS.md

## RAWRZ repository contract

RAWRZ is the monorepo for three formerly separate components:

- **RAWRZ Stack** — the current Bear Cave Compose media stack at the repository root.
- **RAWRZ RPG** — the imported Rust RPG crate at `backend/rpg/`.
- **RAWRZ Deck** — the imported Rust/React control plane at `backend/deck/`.

The master design is [`rawrz-megastack-spec.md`](rawrz-megastack-spec.md). Component
specifications live under [`docs/rpg/`](docs/rpg/) and [`docs/deck/`](docs/deck/).
The current stack operations reference remains in [`docs/stack/`](docs/stack/).

## M0 boundary

M0 is an additive migration milestone. It preserves the current root
`docker-compose.yml` and does not add Redis, Postgres, nginx, observability, or an RPG
or Deck service to the live stack. Future runtime milestones must update the master
spec and their own runbook before changing Compose.

The current root stack is still the eight always-on services documented in
`docs/stack/AGENTS.md`; that is the current runtime, not the final RAWRZ target state.

## Worktree and delivery rules

- Work only in one task-named worktree under `.worktrees/<task-name>`.
- Branch from `origin/main`; keep the reference checkout clean.
- One task per worktree. Do not mix migration, runtime, or unrelated maintenance work.
- Deliver through a pull request. `main` is protected; use squash/rebase only.
- Commit logical save points with Conventional Commit messages and push the task branch
  regularly so recovery never depends on an unpushed local state.

## Before editing

1. Read the applicable master/component spec and the relevant `docs/stack/` operational doc.
2. Check `git worktree list --porcelain` and `git status --short --branch`.
3. For upstream application behavior, inspect the pinned source under `~/TRUTH/<service>/`
   before relying on memory or current upstream documentation.
4. Confirm whether the change is M0 documentation/layout/CI work or a later runtime change.

## Validation before push

Run the smallest applicable checks, then the repository gates:

```bash
docker compose config --quiet
python3 catalog/validate.py
python3 scripts/check_api_contract.py
python3 scripts/check_secret_manifest.py
python3 scripts/check_secret_drift.py
bash -n scripts/*.sh tests/*/*.sh
./tests/bash/test_bash_functions.sh --offline
```

For Rust changes, run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
and `cargo test --all-targets` in both crates. The RPG store proof may run only against a
clearly named disposable database such as `rpg_scratch`.

All third-party GitHub Actions must use full immutable SHAs with a version comment;
`actionlint` is part of CI. Never commit `.env`, credentials, database files, or anything
from `~/TRUTH`.

## Safety

- Never run destructive production, database, mount, or container operations without an
  explicit request and the required queue/mount checks.
- M0 edits do not require restarting the live stack. If a later change edits a bind-mounted
  runtime file, restart the serving container as documented in `docs/stack/AGENTS.md`.
- Keep operational output and agent-facing documentation in English.

## Completion status

End task reports with exactly one of: **DONE**, **DONE_WITH_CONCERNS**, **BLOCKED**, or
**NEEDS_CONTEXT**, followed by concise evidence and remaining concerns.
