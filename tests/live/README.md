# tests/live/ — the live tier (D11/D13)

This directory is the **thebearcave live test tier**. The offline per-shell
suites (merge gate) live in the `services/cave-scripts` submodule
(`tests/<shell>/`); they mock `__stack_curl` and never touch the stack. This
tier is the deliberate opposite: it runs against the **real, running stack**
and is executed at milestone demos (Demo 3, Demo 4), not on every push.

## Contents

| File | What it is |
|---|---|
| `safe_command_registry.yaml` | The D11 manifest: every command allowed to run live, with argv, touched surfaces, and output contract; plus every mutating command with its exclusion reason. |
| `run_safe_matrix.sh` | Executes every enabled registry row once per shell (bash/zsh/fish) through the port loaders, with per-surface preflight probes. |
| *(submodule)* `services/cave-scripts/tests/live/test_live_parity.sh` | The three-shell **byte-parity** gates from Demo 2 (same output in all three shells). Ships in the submodule, not here. |

Division of labour: the **parity harness** proves the three ports agree with
each other; the **safe matrix** proves each port talks to each live surface
correctly (arr, Plex, nzbdav, Seerr, docker, host, btrfs, Hyprland). They are
complementary, not redundant.

## The rule (plan §4.2, decision D11)

During development, **only registry rows may touch the live stack** —
status/health/queue/history/session GET surfaces, `docker ps`-class reads,
nzbdav health + authenticated queue + PROPFIND, btrfs info, hyprctl info,
disk/usage, backup `--dry-run`. Mutating and destructive commands run only
inside a milestone demo with the owner watching. Every mutating command is
recorded in the registry with an exclusion reason so the boundary is auditable,
not folklore.

## Running it

```bash
tests/live/run_safe_matrix.sh              # full matrix, all shells
tests/live/run_safe_matrix.sh --list       # print rows, don't execute
tests/live/run_safe_matrix.sh --shell fish # one shell only
tests/live/run_safe_matrix.sh --family btrfs
```

Exit code: `0` when no row FAILED (SKIP is always fine — e.g. Plex answered,
Seerr down), `1` on any FAIL. Requires the submodule checkout at
`services/cave-scripts` (the loaders it sources) and `python3` with PyYAML
(the same dependency the offline suites use).

## Changing the registry

Registry first: a command's argv, surfaces, or output contract changes →
edit `safe_command_registry.yaml` in this repo **and** the row in the
submodule's `spec/functions.yaml` in the same PR. New M3 functions start as
`status: pending` rows (listed for visibility, not executed) and flip to
executable in the PR that implements them — that flip is the Demo 3 / Demo 4
gate for the family.
