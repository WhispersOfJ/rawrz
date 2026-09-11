# Contributing to RAWRZ

RAWRZ is a protected-main monorepo. Changes arrive through pull requests and must keep
the three component histories, current stack behavior, and unified CI honest.

## Worktrees and branches

Create one task-named worktree inside the repository, based on `origin/main`:

```bash
mkdir -p .worktrees
git worktree add .worktrees/<task-name> -b <task-branch> origin/main
cd .worktrees/<task-name>
```

Do not edit the reference checkout, mix unrelated work, or push directly to `main`.
Push logical save points to the task branch and open a PR when the local gates are green.
See [`docs/stack/worktree-lifecycle.md`](docs/stack/worktree-lifecycle.md).

## Commits and releases

Use Conventional Commits:

```text
feat: add a capability
fix: correct a regression
docs: clarify the migration record
ci: extend the unified validation pipeline
chore: maintain repository tooling
```

The root release-please configuration is the only release configuration. `feat:` and
`fix:` commits are release-worthy; documentation, CI, and chore commits do not create a
release by themselves. `RELEASE_PLEASE_TOKEN` is a GitHub Actions secret and must never
be committed.

## M0 scope

M0 is intentionally non-runtime: do not add services to the root Compose file, alter
ports, migrate databases, or cut over the live stack as part of migration/layout/CI work.
Update the master spec and a later runbook before implementing target-state runtime work.

## Validation

Run applicable checks before committing:

```bash
docker compose config --quiet
python3 catalog/validate.py
python3 scripts/check_api_contract.py
python3 scripts/check_secret_manifest.py
python3 scripts/check_secret_drift.py
bash -n scripts/*.sh tests/*/*.sh
./tests/bash/test_bash_functions.sh --offline
```

For Rust or frontend changes, also run the crate/frontend checks listed in `README.md`.
All GitHub Actions are full-SHA pinned and must pass `actionlint`. Do not commit `.env`,
`secrets/`, database files, build output, or generated dependency directories.

## Review expectations

- Preserve provenance when moving imported files; use `MIGRATION.md` for source refs.
- Keep docs links valid after paths move.
- Add or update tests with behavior changes.
- Report failed or skipped checks explicitly in the PR description.
