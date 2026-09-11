# Contributing to Movie / TV RPG

## Worktree discipline

All edits happen on a dedicated git worktree per task, branched off `main`.
Before editing, create the task worktree:

```bash
git worktree add ../movie-rpg-<task> -b <task-branch> main
```

The main checkout stays reference-only. Deliver via PR (linear history;
squash/rebase only). After merge, remove the worktree.

## Commit style

Use conventional commits for PR titles and squash commits:

```
feat: description
fix: description
docs: description
chore: description
```

PR titles must match `type(scope): subject`. Release-please opens a release
PR from `feat:`/`fix:` commits only.

## Spec-first

Changes to the RPG's design go through `movie-rpg-spec.md` first. Update the
spec, then implement. Flag spec contradictions in the PR.

## Secrets

Never commit `.env`, `.env.template` with real values, or secrets. The RPG
reuses the Bear Cave stack's `.env` secrets (PLEX_TOKEN, SONARR_API_KEY,
RADARR_API_KEY, etc.) plus its own `RPG_DB_URL`; all are gitignored.
