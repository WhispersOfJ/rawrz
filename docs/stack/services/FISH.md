# Fish Functions — Retired

The fish `stack-*` function library (`services/fish-functions/`) was retired
in favor of the bash port at `services/bash-functions/`. The bash port is a
1:1 translation that runs on the same host-published APIs and is verified
live against the eight-service stack.

## What was retired

The entire `services/fish-functions/` tree (202 files) was removed end to end:

- `functions/*.fish` — the `stack-*` commands and `__*_api` helpers
- `completions/*.fish` — generated fish tab-completions
- `conf.d/bearcave-env.fish` — `.env` loader for fish startup
- `scripts/install.sh` — symlink installer (refused worktrees, self-verified)
- `scripts/uninstall.sh` — symlink pruner
- `scripts/gen-completions.fish` — completion generator + `--check` drift gate

## Why

The stack runs on a Linux host where bash is the universal interactive shell.
Maintaining a parallel fish library doubled the surface for every API-hang
landmine, every endpoint drift (e.g. Sonarr `/missing` → `/wanted/missing`),
and every completion/regeneration step. The bash port consolidates on a
single implementation and a single offline smoke test
(`tests/bash/test_bash_functions.sh`).

## What replaces it

| Retired (fish) | Replacement (bash) |
|---|---|
| `services/fish-functions/` | `services/bash-functions/` |
| `tests/fish/test_fish_functions.sh` | `tests/bash/test_bash_functions.sh` |
| `fish gen-completions.fish --check` | `bash gen-bash-completions.sh --check` |
| `services/fish-functions/scripts/install.sh` | `source services/bash-functions/bearcave-bash.sh` from `~/.bashrc` |
| `docs/services/fish-functions.md` | `docs/services/bash-functions.md` (this file supersedes it) |

See [bash-functions.md](bash-functions.md) for setup, layout, completion
checks, and the per-call-type API timeout policy.

## Re-adoption policy

If fish is needed again, the retirement is reversible from git history
(`git log -- services/fish-functions/`). Re-adoption would require: restoring
the tree, re-adding the fish CI step, re-adding `tests/fish/`, and keeping
both the fish and bash libraries in sync on every API/endpoint change. The
preference is to stay on the single bash implementation.
