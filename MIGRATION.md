# M0 import record

RAWRZ was created on **2026-09-11** by importing three repositories. This file is the
authoritative provenance record for that import. It satisfies the M0 requirement that every
imported component is attributable to its source repository and commit.

## 1. What was imported

| Source repository | Source ref | Commits | Destination | Rail tip after rewrite |
|---|---|---|---|---|
| `WhispersOfJ/TheBearCave` | `b3256a0` | 408 | repo root (the stack, as the repository's own history) | — (not rewritten) |
| `WhispersOfJ/movie-rpg` | `562f3b3` | 43 | `backend/rpg/` | `6608169` |
| `WhispersOfJ/cave-deck` | `af49a84` | 16 | `backend/deck/` | `9b13fa3` |

Total reachable commits in RAWRZ after the import: **470** (408 stack + 43 RPG + 16 Deck +
2 import merges + 1 artifact-routing commit).

The stack's history is the repository's own history — RAWRZ's root *is* The Bear Cave, so no
rewrite was needed for it. The two application repositories were rewritten (below) because
their trees had to move under `backend/`.

## 2. What was committed in the source repositories first

Both source repositories had **uncommitted work** at import time. A history-only import would
have silently dropped it, so it was committed in the source repositories first:

| Repo | Commit | Subject |
|---|---|---|
| `movie-rpg` | `efa1366` | `feat(wizard): add spell resource, affinity, and charge economy` |
| `movie-rpg` | `562f3b3` | `feat(security): add URL credential redaction helpers` |
| `TheBearCave` | `90a76cb` | `feat(nzbdav): re-adopt Eweka as a tertiary Usenet provider` |
| `TheBearCave` | `b3256a0` | `docs(agents): add the movie-rpg linkage note` |

Also included in `90a76cb`: the `NZBDAV_USENET_TERTIARY_*` keys added to `.env.template`, without
which the compose environment-coverage gate would fail (the pending compose change referenced
${VAR}s the template did not define).

**Note on `movie-rpg` `efa1366`:** the wizard spell slice is knowingly incomplete — the
spend-SQL constants are defined but not yet wired to a caller, so the crate compiles with
dead-code warnings. `movie-rpg`'s CI has no clippy gate, so this does not fail anything today;
the M0 plan's proposed `clippy --all-targets -- -D warnings` gate for RAWRZ will fail on this
state until the slice is finished or the gate lands unblocked. Recorded here so it is not
mistaken for an import defect.

## 3. Method (and why not `git subtree`)

The M0 plan originally specified `git subtree add --prefix=…`. **That method was executed first
and rejected**, because it does not deliver the property the plan promised:

- `git subtree add` grafts the source history with **unprefixed paths** and then merges the
  prefixed tree. Path-filtered history and `--follow` therefore do not traverse the boundary:
  `git log --follow -- backend/deck/backend/src/catalog.rs` returned **0** commits.
- For `movie-rpg` the effect was worse-looking than it was, but still wrong: because that
  repository's crate already lives at `backend/rpg/` *inside* the repo, the import nested it at
  `backend/rpg/backend/rpg/Cargo.toml`.

The executed method rewrites each source history so its paths are **already final** before it is
merged, which makes path-filtered log and `--follow` behave normally:

```bash
# 0. Base: the stack repository becomes RAWRZ's own history
gh repo create WhispersOfJ/rawrz --public --description "RAWRZ — home media megastack"
git clone /home/bear/Cave /home/bear/rawrz
cd /home/bear/rawrz
git remote rename origin cave-upstream
git remote add origin https://github.com/WhispersOfJ/rawrz.git

# 1. Rewrite movie-rpg so crate paths stay and everything else moves under backend/rpg/
git clone /home/bear/movie-rpg /tmp/rawrz-rpg && cd /tmp/rawrz-rpg
FILTER_BRANCH_SQUELCH_WARNING=1 git filter-branch -f --index-filter '
  git ls-files -s | sed -E "/\tbackend\/rpg\//! s-\t-\tbackend/rpg/-" |
    GIT_INDEX_FILE=$GIT_INDEX_FILE.new git update-index --index-info &&
    mv "$GIT_INDEX_FILE.new" "$GIT_INDEX_FILE"
' --tag-name-filter cat -- --all

# 2. Rewrite cave-deck so everything moves under backend/deck/
git clone /home/bear/Cave/.worktrees/cave-deck /tmp/rawrz-deck && cd /tmp/rawrz-deck
FILTER_BRANCH_SQUELCH_WARNING=1 git filter-branch -f --index-filter '
  git ls-files -s | sed -E "s-\t-\tbackend/deck/-" |
    GIT_INDEX_FILE=$GIT_INDEX_FILE.new git update-index --index-info &&
    mv "$GIT_INDEX_FILE.new" "$GIT_INDEX_FILE"
' --tag-name-filter cat -- --all

# 3. Merge both rewritten histories into RAWRZ (no tag import, no fast-forward)
cd /home/bear/rawrz
git fetch --no-tags /tmp/rawrz-rpg  main:rpg-import
git merge --allow-unrelated-histories --no-ff rpg-import
git fetch --no-tags /tmp/rawrz-deck main:deck-import
git merge --allow-unrelated-histories --no-ff deck-import
git branch -D rpg-import deck-import
git remote remove cave-upstream
```

Consequences of the rewrite, stated plainly:

- **Every commit object in the two application rails has a new SHA.** Content, authorship,
  order, and messages are preserved; identity is not. The original source SHAs are recorded in
  §1 and in each import commit's message, which is what makes attribution possible.
- **Source tags were not imported** (`--no-tags`). RAWRZ carries only the stack's release tags;
  the RPG's `v0.0.0.1` and any Deck tags stay in their source repositories.
- The source repositories are unmodified and remain the working copies until M9.

## 4. What was deliberately excluded

| Excluded | Why |
|---|---|
| `~/Cave/movie-rpg-spec.md` (stale, 160 KB, untracked) and `~/movie-rpg-spec.md` (25 KB) | Superseded by the canonical 176 KB spec, which arrived with the `movie-rpg` import as `docs/rpg/movie-rpg-spec.md`. Three copies existed; one survived. |
| `~/Cave/backend/src/*.rs` (untracked local scratch: `routes.rs` with `todo!()` handlers, `docker.rs`, `executor.rs`, `jobs.rs`, `config.rs`) | Never committed in The Bear Cave, so not present in its history. Superseded by RAWRZ Deck; its feature intent is captured in `docs/deck/`. |
| `movie-rpg`'s `.github/workflows/` and `release-please` configs | Superseded by the unified CI and single release stream (M0 plan §6, §7.2). |
| `movie-rpg`'s `LICENSE`, `CODE_OF_CONDUCT.md` | Duplicates of the root copies (both MIT). |
| Source tags for the two application repositories | Single release stream (D4); see §3. |

## 5. Layout after the import

```
rawrz/
├── docker-compose.yml  config/  scripts/  services/  tests/  media/   ← The Bear Cave
├── docs/
│   ├── rpg/     movie-rpg-spec.md, CHANGELOG.md, CONTRIBUTING.md
│   ├── agents/  FIX.md, HANDOFF.md, CLAUDE.md
│   ├── deck/            (tracked by the cave-deck import; not yet relocated — M0 PR9)
│   ├── stack/           (Bear Cave docs; AGENTS.md content not yet relocated — M0 PR9)
│   └── plans/           the RAWRZ planning documents
├── backend/
│   ├── rpg/     Cargo.toml Cargo.lock src/ tests/ fixtures/ migrations/ scripts/ .gitignore
│   └── deck/    backend/ frontend/ catalog/ docs/ parity/ scripts/ compose.yml README.md
└── MIGRATION.md
```

Not yet consolidated (deferred to M0 PR9–PR11, all tracked): `backend/deck/.github/`,
`backend/deck/.env.template`, `backend/deck/compose.yml`, and the relocation of the Bear Cave's
`AGENTS.md`/`docs/` into `docs/stack/`.

## 6. Verification

```bash
# History traverses the import boundary for each component
git log --follow --oneline -- backend/rpg/src/lib.rs                    # → early RPG commits
git log --follow --oneline -- backend/deck/backend/src/catalog.rs       # → the deck commit that added it
git log --follow --oneline -- services/bash-functions/bearcave-bash.sh  # → Bear Cave commits

# Counts
git rev-list --count HEAD                          # 470
git log --oneline 562f3b3 | wc -l                  # 43 (the RPG rail, by original SHA)
```

Recorded results at import time:

| Check | Result |
|---|---|
| Total commits | 470 |
| RPG follow (`backend/rpg/src/lib.rs`) | 16 commits, bottoming out at `feat: add metadata provider probe foundation` |
| Deck follow (`backend/deck/backend/src/catalog.rs`) | 1 commit (`feat(catalog): serve the curated catalog from backend and GUI (#21)`) — correct, the file was added and never modified |
| Stack follow (`services/bash-functions/bearcave-bash.sh`) | 2 commits |
| Tags present | stack tags only (`v1.1.0`…) |
| Working tree | clean |

## 7. Rollback

M0 is additive and nothing else depends on it. To undo: archive or delete
`github.com/WhispersOfJ/rawrz` and delete `~/rawrz`. The three source repositories are
unmodified apart from the four commits in §2, which are ordinary commits on `main` in their own
repositories and remain valid there.
