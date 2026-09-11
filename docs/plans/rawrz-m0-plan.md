# RAWRZ — M0 migration plan (skeleton, subtree imports, unified CI, docs)

> **Status:** Draft v0.1 — 2026-09-11. Companion to `rawrz-megastack-spec.md` §3, §13, §15.
> **Delivery:** PR1–PR11 are delivered on the task branch `chore/complete-rawrz-migration`;
> §9 records what is verified locally and what remains a GitHub-side operation.
> **Milestone:** **M0** — the first mergeable step (D37): create RAWRZ, import all three
> repositories with preserved history, stand up unified CI, consolidate docs.
> **Hard constraint:** **M0 changes no runtime behavior.** No containers, no Redis, no nginx,
> no Postgres, no compose changes, no cut-over. The live host keeps running The Bear Cave
> exactly as it does today, and the three source repos stay untouched and functional.

---

## 1. Goal, non-goals, and the escape hatch

**Goal.** A brand-new public repository `WhispersOfJ/rawrz` that:

1. contains all three codebases in the target layout,
2. preserves each repository's history as subtrees,
3. builds and tests both Rust crates plus the stack's existing validation in **one CI**,
4. carries the master spec plus relocated component sub-specs and a unified agent contract,
5. is trivially abandonable: deleting the new repo restores the status quo ante (nothing else
   has changed).

**Non-goals for M0 (explicit).** No `rawrz-redis`, no nginx, no Postgres service, no RPG
containerization, no Deck port, no submodule removal on the live host, no \*arr DB migration, no
webhook wiring, no rebranding of running services. Every one of those is a later milestone with
its own rollback story.

**Escape hatch.** M0 is purely additive. If it goes wrong: archive the new repo, keep working in
the three existing repos. No live system is a dependency of M0.

---

## 2. Import inventory (verified 2026-09-11)

| Source repo | Commits | Version line | First commit | Notes |
|---|---|---|---|---|
| `WhispersOfJ/TheBearCave` | **406** | `1.35.0` (`.release-please-manifest.json`) | 2026-08-26 `a0c976d` — "initial scaffold — merge media-stack + metacacharr" | Carries the stack, 16 workflows, `scripts/`, `config/`, docs, the dead `backend/src/`, and a **stale** `movie-rpg-spec.md` (160 KB) |
| `WhispersOfJ/movie-rpg` | **41** | `0.0.0.1` (with `release-as: 0.0.0.1`) | 2026-09-08 `5bc23ee` — "initial commit — Movie / TV RPG spec" | Carries the canonical spec (176 KB), `backend/rpg` (14 migrations, 89 unit tests + 1 live proof), FIX.md/HANDOFF.md |
| `WhispersOfJ/cave-deck` | **16** | own line; workflows `ci.yml`, `pr-lint.yml`, `release.yml` | (worktree at `~/Cave/.worktrees/cave-deck`) | Carries `backend/`, `frontend/` (React 18 + TS + Vite), `catalog/catalog.yaml` (100 entries), `spec/` |

Total preserved history: **463 commits**.

---

## 3. Subtree import procedure

### 3.1 Rules

- **One labeled commit per source repo**, each naming the source repository and its exact
  source commit SHA. History must remain attributable.
- **The canonical spec wins.** `movie-rpg`'s 176 KB `movie-rpg-spec.md` is imported;
  TheBearCave's stale 160 KB copy and `~/movie-rpg-spec.md` (25 KB) are **deleted, not merged**.
  A `docs/HISTORY-DEDUPE.md` records the three copies, their sizes, and why one survived.
- Subtree remotes are used during migration and **removed** at the end of M0 so RAWRZ is the
  only origin (no accidental push-back to a retired repo).
- Do not rewrite the imported history. If a file must change location afterward, that is a
  separate commit — mixing a move into the import commit is what breaks `git log --follow`.

### 3.2 Commands

> **⚠ Superseded — see §3.2a.** This recipe was executed on 2026-09-11 and **does not** deliver
> the `--follow` property claimed below: `git subtree add --prefix=…` grafts history with
> *unprefixed* paths, so path-filtered log and `--follow` do not traverse the import boundary
> (measured: 0 commits for a Deck sample), and movie-rpg nested its crate at
> `backend/rpg/backend/rpg/`. The method actually used is in §3.2a.

```bash
# 0. Create the new repo (public, personal account), clone it
gh repo create WhispersOfJ/rawrz --public --description "RAWRZ — home media megastack: stack + deck + RPG"
git clone git@github.com:WhispersOfJ/rawrz.git && cd rawrz
git checkout -b m0/import

# 1. Add the three sources as read-only remotes
git remote add cave git@github.com:WhispersOfJ/TheBearCave.git
git remote add rpg  git@github.com:WhispersOfJ/movie-rpg.git
git remote add deck git@github.com:WhispersOfJ/cave-deck.git
git fetch --all --tags

# 2. Stack at the repo root: import TheBearCave's tree as the base, keeping its history
git merge --allow-unrelated-histories -s ours cave/main -m \
  "chore(m0): import TheBearCave history as the RAWRZ base"
git read-tree -u --reset cave/main              # adopt the stack's working tree
git commit --amend --no-edit -m \
  "chore(m0): import TheBearCave history as the RAWRZ base (source: TheBearCave@$(git rev-parse --short cave/main))"

# 3. RPG into backend/rpg
git subtree add --prefix=backend/rpg rpg/main main -m \
  "chore(m0): import movie-rpg into backend/rpg (source: movie-rpg@$(git rev-parse --short rpg/main))"

# 4. Deck into backend/deck
git subtree add --prefix=backend/deck deck/main main -m \
  "chore(m0): import cave-deck into backend/deck (source: cave-deck@$(git rev-parse --short deck/main))"

# 5. Remove the temporary remotes — RAWRZ is the only origin from here on
git remote remove cave rpg deck
```

### 3.2a Verified method (executed 2026-09-11)

**Finding.** `git subtree add` preserves commits but not *paths*: the grafted rail keeps the
source's original paths while the merge adds the prefix. Consequences: the Deck import returned
0 commits for a path-filtered/`--follow` query, and the RPG import nested the crate at
`backend/rpg/backend/rpg/Cargo.toml` because movie-rpg's crate path (`backend/rpg/`) coincided
with the chosen prefix.

**Executed method.** Rewrite each application repository so its paths are already final, then
merge the rewritten history. Both histories are small (43 and 16 commits), so `git filter-branch`
is sufficient — `git filter-repo` is not installed on this host and installing it was out of
scope.

```bash
# Base: the stack repository becomes RAWRZ's own history
gh repo create WhispersOfJ/rawrz --public --description "RAWRZ — home media megastack"
git clone /home/bear/Cave /home/bear/rawrz && cd /home/bear/rawrz
git remote rename origin cave-upstream
git remote add origin https://github.com/WhispersOfJ/rawrz.git

# movie-rpg: crate paths stay, everything else moves under backend/rpg/
git clone /home/bear/movie-rpg /tmp/rawrz-rpg && cd /tmp/rawrz-rpg
FILTER_BRANCH_SQUELCH_WARNING=1 git filter-branch -f --index-filter '
  git ls-files -s | sed -E "/\tbackend\/rpg\//! s-\t-\tbackend/rpg/-" |
    GIT_INDEX_FILE=$GIT_INDEX_FILE.new git update-index --index-info &&
    mv "$GIT_INDEX_FILE.new" "$GIT_INDEX_FILE"
' --tag-name-filter cat -- --all

# cave-deck: everything moves under backend/deck/
git clone /home/bear/Cave/.worktrees/cave-deck /tmp/rawrz-deck && cd /tmp/rawrz-deck
FILTER_BRANCH_SQUELCH_WARNING=1 git filter-branch -f --index-filter '
  git ls-files -s | sed -E "s-\t-\tbackend/deck/-" |
    GIT_INDEX_FILE=$GIT_INDEX_FILE.new git update-index --index-info &&
    mv "$GIT_INDEX_FILE.new" "$GIT_INDEX_FILE"
' --tag-name-filter cat -- --all

# Merge both rails; do NOT import source tags
cd /home/bear/rawrz
git fetch --no-tags /tmp/rawrz-rpg  main:rpg-import
git merge --allow-unrelated-histories --no-ff rpg-import
git fetch --no-tags /tmp/rawrz-deck main:deck-import
git merge --allow-unrelated-histories --no-ff deck-import
git branch -D rpg-import deck-import
git remote remove cave-upstream
```

**Consequences to record (and they are recorded in `MIGRATION.md`):** the two application rails'
commit SHAs are rewritten (content, authorship, and messages are preserved; the original SHAs live
in the import commit messages and `MIGRATION.md`); source tags are not imported; the two source
repositories stay unmodified and remain the working copies until M9.

**Do this first:** commit any uncommitted work in the source repositories, or the import silently
drops it. Both repositories had pending changes at import time (see `MIGRATION.md` §2).

### 3.3 Verification (must pass before M0 is considered done)

```bash
# History follows across the import for a file from each source repo
git log --follow --oneline backend/rpg/src/persistence.rs | tail -3
git log --follow --oneline backend/deck/backend/src/main.rs | tail -3
git log --follow --oneline services/bash-functions/functions/stack-radarr-prune.sh | tail -3

# Commit counts survived (allow for the import commits themselves)
git log --oneline -- backend/rpg  | wc -l     # ≈ 41
git log --oneline -- backend/deck | wc -l     # ≈ 16
# The stack's history is the repo's own history: expect ≥ 406

# Nothing was left behind
ls backend/rpg/Cargo.toml backend/deck/backend/Cargo.toml catalog/catalog.yaml docker-compose.yml
```

- [x] `git log --follow` resolves each sample file back to its original repo's commit subjects.
      Verified on the task branch: RPG `backend/rpg/src/persistence.rs` bottoms out at
      `feat: add content persistence upsert plan`; Deck `backend/deck/backend/src/catalog.rs` at
      its catalog commit; stack `services/bash-functions/bearcave-bash.sh` at the `stack-*` CLI
      port.
- [x] The stale spec copies are gone; the canonical spec exists in exactly one place
      (`docs/rpg/movie-rpg-spec.md`; the Deck spec at `docs/deck/cave-deck-spec.md`).
- [x] `MIGRATION.md` records, for each source repo: URL, branch, commit SHA, imported date, and the
      destination path.
- [ ] Original repos still exist, untouched, with their tags and release history intact.
      *External to this repository: the source checkouts are on disk and M0 changed nothing in
      them; confirm in their own checkouts before archiving them at cut-over.*

---

## 4. Target layout at M0

```
rawrz/
├── docker-compose.yml            # imported stack, UNCHANGED in M0
├── .env.template                 # union of both repos' templates (§7.1)
├── secrets/                      # pattern kept (gitignored; placeholder README only)
├── config/                       # imported stack config (gitignored contents)
├── media/                        # symlink trees (gitignored contents)
├── scripts/                      # imported stack scripts + preflight/hooks
├── services/
│   ├── bash-functions/           # imported
│   ├── host-tools/               # imported
│   └── cave-deck → backend/deck  # RETIRED as a submodule; code now in backend/deck
├── backend/
│   ├── rpg/                      # imported (crate `movie-rpg`)
│   └── deck/                     # imported: backend/ + frontend/ + catalog/? (see §5)
├── docs/
│   ├── stack/                    # AGENTS.md content, API.md, landmines, lifecycle, services/
│   ├── rpg/                      # movie-rpg-spec.md (canonical)
│   ├── deck/                     # cave-deck-spec.md
│   ├── agents/                   # FIX.md, HANDOFF.md lineage
│   └── plans/                    # this file + postgres hardening + migration runbook + seerr patch plan
├── tests/                        # imported bash/integration tests
├── MIGRATION.md                  # the import record
├── AGENTS.md                     # NEW unified agent contract
├── CONTRIBUTING.md               # NEW unified contributor rules
├── README.md                     # NEW, short, points at the spec
├── CODE_OF_CONDUCT.md  LICENSE   # one each (MIT both sides)
└── .github/workflows/            # consolidated (§6)
```

Deliberately **deferred** (not created in M0): `services/nginx/`, `config/redis/`,
`config/postgres/`, `config/grafana/`, `services/observability/`. Creating empty scaffolds for
future milestones invites confusion and half-built state.

---

## 5. Source → destination move map

| From | To | Action |
|---|---|---|
| TheBearCave `docker-compose.yml`, `config/`, `media/`, `scripts/`, `tests/`, `docs/`, `services/`, `archive/`, `usenet/`, `.trivyignore`, `.pre-commit-config.yaml` | repo root | import as-is |
| TheBearCave `AGENTS.md`, `CLAUDE.md`, `NEEDED.md`, `TODO.md`, `HISTORY.md` | `docs/stack/` (content) | relocate; root `AGENTS.md` authored fresh (§6.4) |
| TheBearCave `movie-rpg-spec.md` (stale) | — | **delete** (superseded by the canonical copy) |
| TheBearCave `backend/src/*.rs` (dead stack-management dashboard, no `Cargo.toml`) | `docs/stack/legacy-backend.md` | document, do not import as code; intent is superseded by Deck |
| TheBearCave `.release-please-*` | root | keep, becomes the single stream (§7.2) |
| movie-rpg `backend/rpg/**` | `backend/rpg/**` | subtree import |
| movie-rpg `movie-rpg-spec.md` | `docs/rpg/movie-rpg-spec.md` | move + supersession edits (§6.3) |
| movie-rpg `FIX.md`, `HANDOFF.md` | `docs/agents/` | move (lineage retained) |
| movie-rpg `CLAUDE.md`, `CONTRIBUTING.md` | merged into root `AGENTS.md`/`CONTRIBUTING.md` | merge (§6.4) |
| movie-rpg `.release-please-*` | — | **delete** (single stream) |
| movie-rpg `CHANGELOG.md` | `docs/rpg/CHANGELOG.md` | keep as component history |
| cave-deck `backend/`, `frontend/` | `backend/deck/` | subtree import |
| cave-deck `catalog/` | `catalog/` at root (recommended) | the catalog is stack-wide, not Deck-private |
| cave-deck `spec/` | `docs/deck/` | move |
| cave-deck `.release-please-*`/`release.yml` | — | **delete** (single stream) |
| cave-deck `services/cave-deck` submodule entry | — | retire; document in `MIGRATION.md` |
| `~/Cave/movie-rpg-spec.md`, `~/movie-rpg-spec.md` | — | out-of-tree; recorded in `docs/HISTORY-DEDUPE.md`, never merged |

**Open at execution (decide, don't drift):** whether the catalog lives at root `catalog/` or at
`backend/deck/catalog/`. Recommendation: root — it describes the whole stack, and Deck's own spec
already treats it as repository-level data. If it moves, Deck's catalog CI paths must move with it
in the same commit.

---

## 6. Unified CI

### 6.1 Workflow inventory and mapping

| Source | Workflow | M0 disposition |
|---|---|---|
| TheBearCave | `validate.yml` | **keep**, extended with Rust + catalog jobs |
| TheBearCave | `release-please.yml` | **keep** (single stream, §7.2) |
| TheBearCave | `trivy-scan.yml` | **keep**; add the future Seerr fork image when it exists |
| TheBearCave | `codeql.yml` | **keep**; add `rust`/`javascript-typescript` languages |
| TheBearCave | `nightly-healthcheck.yml`, `disk-cleanup.yml`, `stale.yml`, `pr-labeler.yml`, `pr-lint.yml`, `secret-guard.yml`, `scorecard.yml` | **keep as-is** |
| TheBearCave | `pre-commit.yml`, `quality.yml`, `pin-drift-check.yml`, `cleanuparr-sabnzbd-watch.yml`, `mcp-baseline-refresh.yml` | **keep, then review**: read each during execution and retire anything subsumed by the merged `validate.yml`. Do not delete blind — the pin-drift and secret guards are load-bearing. |
| movie-rpg | `validate.yml` | **merge** into the unified `validate.yml` (Rust fmt/clippy/test jobs) |
| movie-rpg | `release-please.yml` | **delete** (duplicate of the stack's) |
| cave-deck | `ci.yml` | **merge** (frontend build/lint/test + catalog validation jobs) |
| cave-deck | `pr-lint.yml` | **delete** (duplicate) |
| cave-deck | `release.yml` | **delete** (single stream) |

### 6.2 The unified `validate.yml` shape

Ordered so cheap failures surface first:

1. **Create dummy env** — `cp .env.template .env` (the stack's existing trick for `docker compose config`).
2. **Compose validation** — `docker compose config --quiet`, `check_compose_mounts.py`,
   environment-coverage check (every `${VAR}` present in `.env.template`).
3. **Guard regression tests** — the existing long list of `scripts/test_*.py` plus the offline
   guard runs (`check_nzbdav_queue.py --offline`, `check_bind_mount_staleness.py --offline`, …).
   Preserve `fetch-depth: 0` — the config-drift guard resolves pin-change dates via `git log -S`.
4. **Shell/tooling** — `bash -n` sweep, shellcheck, ruff, actionlint.
5. **Rust** — matrix `[backend/rpg, backend/deck/backend]`: `cargo fmt --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`.
   - **The RPG's live proof needs a database.** `backend/rpg/tests/store_proof.rs` is a live
     scratch-Postgres proof; CI gains a `services: postgres:17` container plus
     `RPG_DB_URL=postgresql://postgres:postgres@localhost:5432/rpg_proof`, and the proof must
     create/drop its own scratch database. **Never point it at a shared or persistent database**
     (FIX.md F-1: a proof run was able to wipe its target).
6. **Frontend (Deck)** — `pnpm install --frozen-lockfile`, typecheck, lint, build. Path-filtered so
   it only runs when `backend/deck/frontend/**` changes at first; tighten later.
7. **Catalog** — the Deck catalog's own validation (schema rules, port-collision uniqueness,
   retired-registry cross-check).
8. **E2E skeleton (disabled)** — a placeholder job for the M8 ephemeral-compose e2e tier. Land the
   job definition so the shape is agreed, guarded off until the tier exists.

### 6.3 CI hardening requirements

- **All third-party actions SHA-pinned** with a `# tag` comment (stack policy; Dependabot cannot
  bump SHA pins, so the `pin-drift-check.yml` guard stays).
- **Path filtering** to keep the pipeline tolerable: Rust jobs on `backend/**`, frontend jobs on
  `backend/deck/frontend/**`, stack guards on `scripts/**`, `config/**`, `docker-compose.yml`,
  `.github/workflows/**`.
- **Caching**: `Swatinem/rust-cache`-equivalent for both crates; pnpm store cache for the frontend.
- **No secrets required** for the default CI path. `RELEASE_PLEASE_TOKEN` stays scoped to the
  release workflow only.
- **Branch protection** required checks (set once CI is green): `validate`, `rust (backend/rpg)`,
  `rust (backend/deck/backend)`, `frontend`, `catalog`, plus `pr-lint`. Squash/rebase only,
  linear history, auto-delete head branches.

### 6.4 New root docs to author

| File | Source of truth |
|---|---|
| `AGENTS.md` | Composition table in `rawrz-megastack-spec.md` §3.6 — worktree-per-task, PR-only, spec-first, compose-is-truth, validate-before-push, grep-`~/TRUTH`-first, completion-status protocol, confusion protocol |
| `CONTRIBUTING.md` | Conventional Commits, release-please trigger rules, PR/lint discipline, secrets rules |
| `README.md` | Short orientation: what RAWRZ is, the three components, where the specs live, the current milestone |
| `docs/plans/` | This file, `rawrz-postgres-hardening.md`, `rawrz-arr-db-migration-runbook.md`, `rawrz-seerr-cache-patch-plan.md` |

### 6.5 Supersession edits (required, or the sub-specs contradict the master spec)

| File | Edit |
|---|---|
| `docs/rpg/movie-rpg-spec.md` | Add a header banner: superseded items per master spec §19 — "not part of the stack / no new container" (§1, §10.1, §10.4), out-of-scope reverse proxy + webhooks (§11), `~/Cave/backend/` placement (§7.2), `RPG_*` naming (§10.3), Lantern Academy as settled presentation (§2.1). Cross-link §1.2, §4, §6, §7, §9, §10, §12 of the master spec. |
| `docs/agents/FIX.md` | Banner on §8 (no-Redis decision): superseded by master spec §4 (D7–D12) — the revisit trigger is invoked. Note F-11/F-39 are resolved by Redis sessions (master §7). |
| `docs/deck/cave-deck-spec.md` | Banner: repo/submodule model superseded (§3.1), LAN-only-no-login superseded (§3.4), catalog `redis` entry promoted from opt-in to core (master §4). |
| `docs/stack/AGENTS.md` | Edit the 8-service rule, slim-down posture, worktree path rules, and the RPG "linked but not part of" linkage note. Keep every landmine verbatim — they are all still true. |

**Rule:** no sub-spec may be left asserting something the master spec reverses. This is a
documentation-correctness requirement, not a nice-to-have.

---

## 7. Config, release, and tooling consolidation

### 7.1 Env

- One `.env.template` = union of the stack's template and the RPG's (`RPG_DB_URL`, `TMDB_API_KEY`,
  `TVDB_API_KEY`, `OMDB_API_KEY`, `FANART_API_KEY`, `RPG_BIND_ADDRESS`, …) plus the future RAWRZ
  keys (`REDIS_URL`, `POSTGRES_*`, `RAWRZ_*`), documented with a group comment per component.
- **Collision rule:** stack keys win; RPG keys are renamed per the master spec §12.3 rename table.
- `check_secret_drift.py` / `check_secret_manifest.py` must pass against the union template —
  that is the acceptance test for this step.

### 7.2 Release

- **One** `.release-please-config.json` and **one** `.release-please-manifest.json`.
- **Continue the stack's line** (start from `1.35.0`) so the merge is a version step, not a
  discontinuity; the `2.0.0`-restart question stays open in the master spec §18.
- Adopt the RPG config's **changelog sections** (feat/fix/docs/refactor/test/chore) and its
  `include-v-in-tag: false` policy explicitly — do not inherit either side by accident.
- **Delete** the RPG's `release-as: 0.0.0.1` override: it would force the merged stream's version.
- Keep the stack's `extra-files` README updater only after the README is rewritten to suit it
  (otherwise it will rewrite text that no longer exists).
- `RELEASE_PLEASE_TOKEN` remains a repository secret; **never** committed.
- Component versions are recorded in `docs/` (the RPG's `CHANGELOG.md` moves to `docs/rpg/`).

### 7.3 Repo hygiene files

| File | Action |
|---|---|
| `.gitignore` | **union**, with comments preserving each rule's purpose (stack: `config/*/`, `secrets/`, `.worktrees/`; RPG: `/target`, `**/target/`, `.cache/`, `.data/`). Verify `backend/*/target` is covered. |
| `.pre-commit-config.yaml` | keep the stack's; add a Rust `cargo fmt --check` hook and an actionlint hook if not already present |
| `.trivyignore` | keep (still relevant) |
| `LICENSE`, `CODE_OF_CONDUCT.md` | one each (both MIT); keep the stack's files, note the RPG's dual authorship if desired |
| `.github/dependabot.yml` | extend to `cargo` (both manifests) + `npm` (Deck frontend) + existing `docker`/`pip`/actions |
| `.github/ISSUE_TEMPLATE/`, PR template | keep the stack's |
| `scripts/install-git-hooks.sh` | keep; document `core.hooksPath` + the `--no-verify` escape hatch |
| `.worktrees/` | gitignored; document the **revised** rule: worktrees live inside the RAWRZ checkout under `.worktrees/<task>` |

---

## 8. Ordered PR list (each independently mergeable)

| PR | Contents | Definition of done |
|---|---|---|
| **PR1** | Repo creation, `.gitignore` union, LICENSE/CoC, README stub, `MIGRATION.md` stub | Repo exists, default branch protected later; no code yet |
| **PR2** | TheBearCave history + tree as the base | `git log` shows 406 stack commits; compose unchanged |
| **PR3** | `git subtree add backend/rpg` | `git log --follow` on an RPG file resolves to original commits |
| **PR4** | `git subtree add backend/deck` + catalog placement + submodule retirement | Deck frontend builds locally; catalog path decision recorded |
| **PR5** | Unified `validate.yml` (compose + guards + shell/ruff/actionlint) | Green on a fresh clone with a dummy `.env` |
| **PR6** | Rust CI jobs for both crates incl. the scratch-Postgres live proof | Both crates green; proof runs against an ephemeral CI database |
| **PR7** | Deck frontend + catalog CI jobs | Frontend typecheck/lint/build and catalog validation green |
| **PR8** | Release consolidation (one config/manifest, RPG override deleted, changelog sections adopted) | release-please dry run produces a sensible 1.36.0-shaped release PR |
| **PR9** | Docs move + **all supersession edits** (§6.5) | No sub-spec contradicts the master spec; links resolve |
| **PR10** | Root `AGENTS.md`, `CONTRIBUTING.md`, `README.md`, `docs/plans/*` | Agent contract complete; plan docs present |
| **PR11** | Dead-code/legacy cleanup: delete stale spec copies, document `backend/src`, remove temp subtree remotes | `MIGRATION.md` final; no stale duplicates |

M0 is done when **PR11 merges and every check in §9 passes**. The repository-side work is
complete on the task branch and §9 lists the remaining GitHub-side operations. Only then does M1
(Redis lands in the existing stack) begin.

---

## 9. M0 acceptance criteria

Verified on the task branch (`chore/complete-rawrz-migration`). "Local" means the repository and
host toolchain; "GitHub" means it can only be confirmed once the PR runs in Actions and `main`
is protected.

| # | Criterion | Status | Evidence / remaining action |
|---|---|---|---|
| 1 | `github.com/WhispersOfJ/rawrz` exists, public, personal account, branch-protected `main` | Partial | `origin` is `https://github.com/WhispersOfJ/rawrz.git` and the task branch pushes; **public visibility and branch protection are external and unverified here** — set protection when the PR opens |
| 2 | All source commits reachable; `MIGRATION.md` records the three source SHAs, dates, and destinations | Done | **491** commits reachable; `MIGRATION.md` §1 |
| 3 | `git log --follow` works for one representative file from each source repo | Done | See §3.3 |
| 4 | `docker-compose.yml` unchanged in effect (M0 changes no runtime behavior) | Done | No M0 commit touches `docker-compose.yml`; its newest history entries are stack commits, and `docker compose config --quiet` passes against `.env.template` |
| 5 | Both Rust crates pass `fmt` / `clippy -D warnings` / `test --all-targets` in CI, including the scratch-Postgres live proof | Done locally | RPG: 98 unit tests plus the live proof against a disposable Postgres; Deck: 29 tests. The `rust` job in `validate.yml` runs both on every `backend/**` change |
| 6 | The Deck frontend typechecks, lints, and builds; catalog validation passes | Done locally | `npm ci`, 15 vitest tests, `tsc --noEmit && vite build`; `catalog/validate.py` and `catalog/test_validate.py`. No separate frontend linter is configured, so typecheck + tests are the gate |
| 7 | Every existing stack guard test still runs and passes in the unified pipeline | Done locally | The complete offline guard list from §6.2 passes, including `test_audit_residue` and the registry-sync check |
| 8 | One release config/manifest exists; the RPG's `release-as: 0.0.0.1` override is gone; `RELEASE_PLEASE_TOKEN` is a secret, not a file | Done | Root `release-please-config.json` + `.release-please-manifest.json` only; no `release-as` in the tree; the token remains a secret/env placeholder |
| 9 | `.env.template` is the union and the secret-drift/manifest guards pass | Done | `check_secret_manifest.py` and `check_secret_drift.py` both pass against the union template |
| 10 | No sub-spec contradicts the master spec (§6.5 edits complete) | Done | Supersession banners in the RPG spec, Deck spec, `docs/agents/FIX.md` §8, and the stack AGENTS; every relative markdown link resolves |
| 11 | No secrets in the repo (`secret-guard` green); `~/TRUTH`-style reference material is not committed | Done | `secret-guard`/secret-drift pass; `~/TRUTH` appears only as documented path references, no reference corpus is committed |
| 12 | The three source repos are unmodified, still building, and still serving their existing roles | External | M0 changes nothing in them; confirm in their own checkouts before archiving at cut-over |
| 13 | `docs/plans/` contains the master spec's four companion plans; the master spec's header links them | Done | `rawrz-megastack-spec.md`'s header links all four under `docs/plans/` |

**M0 changes nothing in the live stack.** The only outstanding items are GitHub-side operations
(repository visibility, branch protection, required checks), not repository content.

---

## 10. Risks

| # | Risk | Mitigation |
|---|---|---|
| M0-R1 | Subtree import breaks `--follow` linkage | Verify explicitly (§3.3) with sample files from each repo; never mix a move into an import commit |
| M0-R2 | Repo size / clone time from imported history | Measure after import; if painful, keep `archive/` and large historical artifacts out of the new repo (they remain in the archived originals) |
| M0-R3 | CI cost and duration (406-commit repo, two Rust crates, frontend) | Path filtering, caching, matrix; accept a ~10-minute pipeline and tune later |
| M0-R4 | Duplicate or conflicting GitHub Actions names/permissions after merging three CI sets | Explicit workflow mapping (§6.1); read each workflow before keeping it; one job-naming convention |
| M0-R5 | The unified `.env.template` breaks the stack's secret guards | Guards are part of PR acceptance, not a follow-up |
| M0-R6 | release-please renumbering surprise (RPG `release-as`, `include-v-in-tag`, README extra-file) | Consolidate deliberately in PR8 and dry-run a release PR before merging |
| M0-R7 | Dead stack-management `backend/src` leaks into the tree and confuses future work | Excluded by design; documented in `docs/stack/legacy-backend.md` |
| M0-R8 | M0 scope creep into runtime changes (someone adds Redis "while we're here") | §1 non-goals are explicit; the acceptance criteria assert the compose diff, so creep fails CI review |
| M0-R9 | Two copies of the RPG spec lingering in the repo | `docs/HISTORY-DEDUPE.md` + the PR11 cleanup checks for duplicates |
| M0-R10 | Worktree paths recorded in agent docs point at `~/Cave` | §7.3 revises the rule to `.worktrees/<task>` inside the RAWRZ checkout |

---

## 11. What M0 proves (and what it does not)

**Proves:** the three codebases can live in one repository, build under one CI, share one release
stream, and be documented without contradiction — with zero risk to the running stack.

**Does not prove:** that Redis helps, that nginx can front everything safely, that the RPG
survives containerization, that the \*arr DB migration works, or that the Deck can be ported
in-tree. Each of those is its own milestone (M1–M7 in the master spec), each with its own gates
and rollback.
