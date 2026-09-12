# CI/CD

The repository validates the active eight-service Compose stack and the scripts
that protect its two fragile resources: the NzbDAV queue and the rclone FUSE mount.

## Active workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `validate.yml` | push/PR to `main` | Compose/env coverage, mount and queue guard tests, shellcheck, Ruff, Fish checks, actionlint |
| `quality.yml` | pull request or manual | YAML, Bash, Python, and Fish quality checks |
| `nightly-healthcheck.yml` | nightly or manual | Compose, path, script, guard, YAML, and environment validation |
| `trivy-scan.yml` | push/PR, weekly, or manual | Scans images named by the active Compose file |
| `codeql.yml` | push/PR and weekly | CodeQL analysis for active Python code |
| `release-please.yml` | push to `main` | Conventional-commit release automation |
| `pr-labeler.yml` / `pr-lint.yml` / `stale.yml` | pull request or schedule | Repository hygiene |
| `secret-guard.yml` | push/PR | Checks workflow secret declarations |
| `scorecard.yml` | push and weekly | OpenSSF supply-chain checks |
| `cleanuparr-sabnzbd-watch.yml` | daily or manual | Historical watcher for a possible future Usenet-compatible adoption |

Retired build, publish, certificate, and observability workflows were removed with
the services they supported. Historical release notes may still mention them.

## Validation contract

Every active Compose variable must have a name in `.env.template`. CI also runs:

- `docker compose config --quiet`
- merged-mount regression tests
- NzbDAV queue and bind-mount guard tests
- Bash syntax checks and ShellCheck
- Ruff and Python compilation
- Bash parse, completion-drift, and offline smoke checks (bash port)
- actionlint on every workflow (pinned to the v1.7.12 release binary via
  checksum in `validate.yml`)

Run the local equivalent before pushing:

```bash
docker compose config --quiet
python3 scripts/test_check_compose_mounts.py
bash -n scripts/*.sh tests/*/*.sh
./tests/bash/test_bash_functions.sh --offline
./scripts/preflight.sh    # fails if the local actionlint is not exactly v1.7.12
```

## Action pinning

Third-party GitHub Actions are pinned to full commit SHAs. The trailing version
comment documents the tag used to resolve the SHA. Upgrade pins deliberately,
then run actionlint and the workflow validation locally.

## Secrets

Runtime secrets are never GitHub workflow inputs. The repository's required-secret
manifest covers GitHub-only credentials; `.env.template` covers Compose variables.
Do not add an application credential to a workflow merely to make a check pass.
