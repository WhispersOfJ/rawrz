# RAWRZ — Merged Megastack Specification

> **Status:** Draft v0.2 — 2026-09-11, produced from a seven-round interview plus a
> resolution round. **R-1 and R-4 are resolved** (§17): the media-availability rule is
> formally dropped (§2.1, §10.2) and the Seerr cache patch design is settled with source
> evidence (§5.2).
> **Purpose:** Specify the merge of three repositories (`movie-rpg`, `TheBearCave`,
> `cave-deck`) into a single monorepo named **RAWRZ**, plus a Redis caching layer,
> an nginx ingress tier, a shared Postgres data layer, and an aggressively
> over-built feature set whose complexity is the point of the exercise.
> **Stated intent (verbatim):** *"would like to propose a massive change, which would
> merge this repo with the ~/Cave repo the cave-deck repo and have them seamlessly work
> with one another, adding redis as a caching layer in the stack for apps that are
> installed, especially Seerr with all sorts of bells, whistles and overly complex
> features for a small stack just to see if I can manage it."*
> **Companion plans:** `rawrz-m0-plan.md` (subtree imports, unified CI, docs),
> `rawrz-postgres-hardening.md` (service config, alerts, restore rehearsal),
> `rawrz-arr-db-migration-runbook.md` (SQLite → Postgres), and
> `rawrz-seerr-cache-patch-plan.md` (CacheBackend interface + patch plan).
> **Predecessors superseded by this document:** `movie-rpg-spec.md` §1/§10/§11/FIX.md §8,
> `~/Cave/AGENTS.md` (8-service slim-down rule + worktree discipline),
> `cave-deck-spec.md` §3.1/§3.4. See §19 for the full supersession register.
> **No code, migration, or runtime change is implied by this document.** It is a target
> state plus a phased plan.

---

## 1. Overview

### 1.1 The three things being merged

| Source repo | What it is today | Path today | Version line |
|---|---|---|---|
| `WhispersOfJ/movie-rpg` | "Movie / TV RPG" — Rust/Axum backend implementing the Lantern Academy progression engine over a Postgres mirror of the Plex/Sonarr/Radarr stack. 14 migrations, 89 unit tests + 1 live proof, PIN gate, 5-minute poll loop, port 46532. Spec is 176 KB. | `/home/bear/movie-rpg` | `0.0.0.x` (release-please) |
| `WhispersOfJ/TheBearCave` | The Bear Cave — 8 always-on Docker Compose media services (Prowlarr, Radarr, Sonarr, nzbdav, nzbdav_rclone, Seerr, Plex, Unpackerr) + 2 profile-gated maintenance services (ImageMaid, Recyclarr). Python/Bash scripts, bash-function operational surface, Trivy/CodeQL/nightly CI, `docker-compose.yml`, `.env` + `secrets/`. Also carries a partial, incomplete Rust `backend/` (stack-management dashboard, no `Cargo.toml`/`main.rs`) and a stale copy of `movie-rpg-spec.md`. | `/home/bear/Cave` | `1.35.0` (release-please) |
| `WhispersOfJ/cave-deck` | Cave Deck — a separate repo (Rust/axum backend + React/TS/Vite frontend, own CI/releases) that is pinned into TheBearCave as a **submodule** at `services/cave-deck`. It is a catalog-driven stack manager: a 100-container `catalog/catalog.yaml`, install/uninstall via compose-native automated PRs ("compose-is-truth"), port 7780, LAN-only, no login. The catalog already contains a drafted `redis` entry (`redis:7-alpine`, 256m, `./config/redis:/data`, "shared cache/queue for catalog services… no host port on purpose — reachable as redis:6379 on bearcave"). | `/home/bear/Cave/.worktrees/cave-deck` | own line |

### 1.2 What "merge" means here (decided)

A **brand-new monorepo named RAWRZ** absorbs all three repositories. The three existing
repos are archived read-only with redirect pointers. Git history from all three is
preserved as **subtrees** so per-component history stays followable. The merged tree uses
`backend/rpg/` and `backend/deck/` for the two applications, with the stack (compose,
`config/`, `scripts/`, `services/`, `docs/`) at the repo root.

### 1.3 What RAWRZ becomes

A single, self-managed, LAN-only home media + game + control-plane monorepo:

- **RAWRZ Stack** — the 8 media services plus Postgres, Redis, nginx, and the
  observability tier; everything containerized, one compose world.
- **RAWRZ Deck** — the control plane and GUI (former Cave Deck), extended to own
  container lifecycle, the catalog, the ingress/cache dashboards, feature flags, and
  cross-app surfaces.
- **RAWRZ RPG** — the former Movie/TV RPG (Lantern Academy), now rebranded and
  containerized, sharing auth, Redis, Postgres, and the event bus with the rest.
- **RAWRZ Redis** — a shared caching, queue, lock, session, and pub/sub layer with two
  explicit key policies (ephemeral cache vs durable queue).
- **RAWRZ ingress** — nginx on subdomains with internal TLS, fronting every service and
  operating a cache tier of its own.

---

## 2. Decisions register

Everything below is a decision made during the interview, grouped for auditability.
Deeper rationale lives in the referenced sections.

| # | Topic | Decision | § |
|---|---|---|---|
| D1 | Merge shape | Brand-new monorepo named **RAWRZ**; all three repos imported; originals archived | §3 |
| D2 | History | Preserved as subtrees (`git log --follow` works per component) | §3.2 |
| D3 | Layout | `backend/rpg` + `backend/deck`; stack files at root | §3.3 |
| D4 | Versioning | **One** release stream for the monorepo | §3.5 |
| D5 | Repo rules | A **new unified ruleset** (not inherited wholesale from either side) | §3.6 |
| D6 | Hosting | Personal account (`WhispersOfJ`), **public** | §3.1 |
| D7 | Redis host | Compose container on `bearcave`, no published host port | §4 |
| D8 | Redis roles | Seerr image/API cache **and** RPG provider cache + sessions **and** Deck queues + pub/sub **and** catalog-service cache **and** stack API response cache | §4.2 |
| D9 | Seerr caching | **Patch Seerr** (maintained fork, CI-built image) — not a proxy | §5 |
| D10 | Redis durability | Cache keys ephemeral; **queue/lock keys are a durable exception** (AOF) | §4.4 |
| D11 | Redis failure | Cache fails **open**; queue/lock tier fails closed | §4.6 |
| D12 | Topology | **One Redis instance**; no cap promise in the spec | §4.7 |
| D13 | Reverse proxy | **Reverse the no-reverse-proxy rule**; front all services | §6 |
| D14 | Proxy choice | **nginx** | §6.1 |
| D15 | Routing | **Subdomains + internal TLS** via the existing mkcert CA | §6.2 |
| D16 | Proxy caching | **Cache all services** (`proxy_cache` per upstream) | §6.4 |
| D17 | Cache guardrails | **Aggressive, LAN trust** (no cookie-keyed isolation) | §6.5 |
| D18 | Auth seam | **One auth**: shared session store in Redis; nginx gates RAWRZ UIs | §7 |
| D19 | Unified truth | One auth **and** one front door **and** one data store **and** one control plane | §1.3, §7, §8, §9 |
| D20 | Data layer | One Postgres **instance**, **separate databases per component** | §8 |
| D21 | Media app DBs | **Migrate Radarr/Sonarr/Prowlarr to Postgres too** | §8.3 |
| D22 | DB migration posture | Maintenance window + verified backups; SQLite retained for rollback | §8.4 |
| D23 | *arr ownership | **Full ownership** + RAWRZ **writes config into** the *arr apps | §9.1 |
| D24 | Event sources | `*arr` Connect webhooks **and** Seerr request webhooks **and** Plex webhooks; polling stays as reconciliation | §9.2 |
| D25 | Blast radius | **Accept shared fate** — one big compose project | §10.2 |
| D26 | Ambition | **Maximal** — complexity is the point (bells, whistles, experiments) | §11 |
| D27 | Bells selected | Observability stack, cache-hit dashboards, Redis event bus + realtime, cross-app features, compose-is-truth expansion, cache-everything, feature flags | §11.2 |
| D28 | Observability | Re-adopt a Prometheus/Grafana-style tier (documented reversal of the 2026 slim-down) | §11.3 |
| D29 | Feature flags | Present; authoritative flag state must be durable (not Redis-only) | §11.5 |
| D30 | Rebrand depth | **Everything**: display names, docs, paths, crates, binaries, env vars, DB names, secrets keys, ports, **and the Lantern Academy theme** | §12 |
| D31 | Naming model | Full rebrand: **RAWRZ Stack / RAWRZ Deck / RAWRZ RPG** | §12.1 |
| D32 | Docs | **One master spec + component sub-specs** under `docs/` | §13 |
| D33 | Env/secrets | **One root `.env`** (union of keys) + keep the existing `secrets/` dir | §10.5 |
| D34 | Deployment | **Everything containerized** — no host daemons left (RPG, Postgres, nginx, observability all become services) | §10.1 |
| D35 | Rollout | **Phased, cut over when green** | §15 |
| D36 | Verification | **CI e2e on an ephemeral compose stack** is the gate | §16 |
| D37 | First milestone | **RAWRZ skeleton + CI + docs** (no runtime change) | §15.1 |
| D38 | Definition of done | **Cut-over complete + new features proven** | §16.4 |
| D39 | Seerr fork upkeep | **Automated upstream sync PR**, CI-tested, digest-pinned | §5.5 |
| D40 | This document | `rawrz-megastack-spec.md` | — |
| D41 | R-1 resolution | **Drop the media-availability rule**; shared fate stands; Postgres hardened instead | §2.1, §10.2 |
| D42 | R-4 resolution | **Async `CacheBackend` patch** for Seerr (memory + ioredis backends, `SEERR_CACHE_BACKEND`) | §5.2 |
| D43 | Postgres resilience | **Single hardened instance + verified backups**; no PITR, replica, or automatic failover | §8.5 |

### 2.1 Deliberately *not* preserved

The interview asked which existing hard rules must survive the merge. The following were
**not** selected, i.e. they are explicitly relaxed or removed:

- ~~"Plex/\*arr untouched (SQLite stays)"~~ → full ownership, config writes, and Postgres migration
- ~~"Polling only, no webhooks"~~ → webhooks from all three media apps
- ~~"Redis single instance + 256m cap"~~ → one instance, but no cap promise
- ~~"No new compose containers"~~ → **everything** becomes a container
- ~~"No reverse-proxy tier"~~ → **reversed**; nginx fronts all services

Also dropped in the resolution round (R-1):

- ~~"Media pipeline must survive app outages"~~ → **dropped.** The user chose to drop
  the rule rather than isolate the data tier, because Radarr/Sonarr/Prowlarr now run on
  the containerized Postgres. A Postgres outage is an accepted media outage (§10.2,
  §8.5); the response is hardening Postgres, not re-architecting around it.

Still preserved:

- **LAN-only, no remote access** (no Tailscale/port-forward/TLS-from-the-internet)
- Redis remains a single instance (D12)
- Redis is **never required** by the media pipeline (fail-open everywhere, §4.6) — the
  one availability property that survived, because it is enforced by design

---

## 3. Repository model

### 3.1 Hosting

- New repo: **`github.com/WhispersOfJ/rawrz`**, **public**, personal account.
- Release-please token scope, workflows, and Dependabot carry over from
  `TheBearCave` (which already runs release-please, validate/actionlint, Trivy,
  CodeQL, nightly healthcheck, PR labeler/lint, stale, Dependabot).
- The three source repos are archived after cut-over, each with a short redirect
  `README.md` pointing to RAWRZ and the commit SHA of the import.

### 3.2 History: preserved as subtrees

Goal: `git log --follow` works per component, and the audit trail of all three repos
survives.

Mechanism (to be executed in the migration PR, not now):

```bash
# example shape — exact SHAs recorded in the migration PR
git clone git@github.com:WhispersOfJ/rawrz.git && cd rawrz
git remote add rpg  git@github.com:WhispersOfJ/movie-rpg.git
git remote add deck git@github.com:WhispersOfJ/cave-deck.git
git remote add cave git@github.com:WhispersOfJ/TheBearCave.git
git fetch --all

# stack (root) from TheBearCave history
git merge --allow-unrelated-histories -s ours cave/main   # then read-tree/subtree
# rpg/ into backend/rpg, deck/ into backend/deck via `git subtree add --prefix=`
git subtree add --prefix=backend/rpg  rpg/main  main
git subtree add --prefix=backend/deck deck/main main
```

Constraints and rules:

- **Each import is one clearly-labeled commit** naming the source repo and its exact
  source commit SHA.
- Subtree remotes are kept in `.git/config` during migration only; after cut-over the
  subtree remotes are removed so RAWRZ is the only origin (no accidental pushes back).
- The collision case is called out: `~/Cave` currently contains a **stale copy** of
  `movie-rpg-spec.md` (160,427 bytes, 2026-09-08) and `~/movie-rpg-spec.md` (25,750
  bytes), while the canonical spec in the RPG repo is 175,503 bytes. **Only the
  canonical repo copy is imported**; the stale copies are discarded, not merged.
  The `~/Cave` copy must not be treated as a source of truth during migration.

### 3.3 Layout

```
rawrz/
├── docker-compose.yml            # stack + Postgres + Redis + nginx + observability + apps
├── .env  .env.template           # single canonical env (D33)
├── secrets/                      # Docker secrets (kept)
├── config/                       # per-service config (now incl. redis/, postgres/, nginx/, grafana/)
├── media/                        # rclone-populated symlink trees
├── scripts/                      # bash operational surface + preflight/pre-push hooks
├── services/
│   ├── bash-functions/           # existing operational surface
│   ├── host-tools/               # host shim
│   └── nginx/  observability/    # new service homes
├── backend/
│   ├── rpg/                      # former movie-rpg backend (Cargo.toml → rawrz-rpg)
│   └── deck/                     # former cave-deck backend + frontend (Cargo.toml → rawrz-deck)
├── docs/
│   ├── stack/                    # AGENTS.md content, API.md, landmines, lifecycle
│   ├── rpg/                      # movie-rpg-spec.md (canonical)
│   ├── deck/                     # cave-deck-spec.md
│   └── agents/                   # unified agent contract + FIX/HANDOFF lineage
├── tests/                        # bash + integration + new e2e tier
└── .github/workflows/            # one CI for everything
```

Notes:

- `backend/` mirrors the RPG spec's original "separate crates in a shared `backend/`
  directory" intent (§7.2 of `movie-rpg-spec.md`); the merge finally makes that
  arrangement literal.
- The dead stack-management crate in `~/Cave/backend/src/` (`routes.rs` with `todo!()`
  handlers, `docker.rs`, `executor.rs`, `jobs.rs`, no `Cargo.toml`, no `main.rs`) is
  **not imported as code**. Its feature intent is superseded by RAWRZ Deck (§9.1), and
  its functional inventory is already captured in `cave-deck-spec.md` Appendix C. A
  short `docs/stack/legacy-backend.md` records what it was and where it went.

### 3.4 Submodule retirement

`services/cave-deck` as a submodule disappears: Deck becomes `backend/deck` plus a
compose service in the root stack. The Cave Deck spec's "pin the submodule and bump it
via PR" lifecycle (§3.1) is replaced by "components live in-tree and version with the
monorepo."

### 3.5 Versioning: one release stream

- A single release-please config drives the whole monorepo. Component pins/manifests
  from `movie-rpg` (`.release-please-config.json`, `.release-please-manifest.json`) and
  `cave-deck` are removed; one manifest remains.
- Default: continue the stack's numbering (`1.35.0` → the first RAWRZ release is
  `1.36.0`), so the merge is auditable as a step, not a discontinuity.
- **Open (small):** whether the rebrand justifies restarting at `2.0.0`. Recorded in
  §18, not decided.
- The RPG's `0.0.0.x` line and Deck's own line are both retired. Component versions are
  surfaced in `docs/` and in build metadata, not as separate tags.
- Release automation must keep working with a single `RELEASE_PLEASE_TOKEN`; the
  token stays a GitHub Actions secret and is never committed.

### 3.6 Unified ruleset (D5)

A new root `CONTRIBUTING.md` + `AGENTS.md` govern everything. Composition:

| Rule | Source | Notes for RAWRZ |
|---|---|---|
| Conventional Commits, PR-title lint, release-please triggers on `feat:`/`fix:` only | TheBearCave | adopted verbatim |
| One worktree per task, PR-only, protected `main`, no mixed work | `~/Cave/AGENTS.md` | adopted; worktrees must live **inside** the repo under `.worktrees/<task>` |
| Spec-first: update the spec before implementing design changes | `movie-rpg/CLAUDE.md` | adopted, now one master spec + sub-specs |
| Compose-is-truth: config changes land as automated PRs | `cave-deck-spec.md` §4.2 | expanded to the whole monorepo (D27) |
| Validate before push: `docker compose config --quiet`, shellcheck/actionlint, DB-integrity + secret-drift guards, pre-push hook | TheBearCave | adopted; preflight script becomes repository-wide |
| Rust gates: `cargo fmt --check`, `cargo clippy --all-targets` (zero warnings), `cargo test --all-targets` for both crates | `movie-rpg/HANDOFF.md` conventions | adopted for both crates |
| Source-first answers: grep `~/TRUTH` pinned upstream clones before web/memory | `~/Cave/AGENTS.md` | adopted; `~/TRUTH` gains the Seerr fork |
| Never commit `.env`/secrets; secrets live in `.env` + `secrets/` | both | adopted (D33) |
| Completion status protocol (DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT) | `~/Cave/CLAUDE.md` | adopted |
| Confusion protocol: stop and present 2–3 options for high-stakes ambiguity | `~/Cave/CLAUDE.md` | adopted |

Explicitly **not** inherited: the 8-service cap and "no new containers" rule, the
RPG's lighter direct-commit posture, and the two-repo split rationale.

---

## 4. Redis layer

### 4.1 Deployment (D7, D12)

```yaml
redis:
  image: redis:7-alpine
  container_name: rawrz-redis
  networks: [bearcave]
  # no published host port — reachable as redis:6379 on bearcave only
  volumes:
    - ./config/redis:/data
  environment:
    TZ: ${TZ}
  command: >
    redis-server
    --appendonly yes
    --appendfsync everysec
    --maxmemory 512mb
    --maxmemory-policy volatile-lru
  healthcheck:
    test: ["CMD-SHELL", "redis-cli ping || exit 1"]
    interval: 30s
    timeout: 5s
    retries: 3
    start_period: 10s
```

Rationale for each flag:

- **No host port** — matches the already-drafted catalog entry's intent and keeps Redis
  off the LAN; only RAWRZ services need it.
- **`volatile-lru`** — the crux of D10/D12. Cache keys carry TTLs; queue/lock keys do
  not. A `volatile-*` policy can only evict keys with a TTL, so the durable queue tier
  is structurally protected from eviction inside a *single* instance. `allkeys-lru`
  would silently destroy queued jobs.
- **AOF everysec** — the durable exception for queues/locks (D10). Disk cost is small at
  this scale; `./config/redis` is already the drafted volume path.
- **512m starting cap, no spec promise** (D12) — the spec fixes neither 256m nor 512m as
  a contract; it requires that a cap exists and that the eviction policy stays
  `volatile-*`. Cap changes are routine PRs.

### 4.2 Roles (D8)

| Role | Consumer | Namespace | Durability | Failure |
|---|---|---|---|---|
| Seerr cache (images + API) | patched Seerr | `rawrz:seerr:cache:*` | ephemeral | fail open |
| RPG provider cache | `backend/rpg` enrichment | `rawrz:rpg:provider:*` | ephemeral | fail open |
| RPG sessions | `backend/rpg` auth | `rawrz:rpg:session:*` | ephemeral (regenerable: re-login) | fail closed for gated routes |
| Deck queues/jobs/locks | `backend/deck` | `rawrz:deck:queue:*`, `rawrz:deck:lock:*` | **durable (AOF)** | fail closed |
| Catalog service cache | catalog services (e.g. Paperless) | `rawrz:catalog:<app>:*` | ephemeral | fail open |
| Stack API response cache | `backend/rpg`, `backend/deck`, scripts | `rawrz:api:<service>:<route-hash>` | ephemeral | fail open |
| Event bus (pub/sub + streams) | all RAWRZ backends | `rawrz:bus:*`, `rawrz:stream:*` | streams durable, pub/sub not | fail open (degrade to polling) |
| Cache/queue metrics | Deck dashboards | `rawrz:stats:*` | ephemeral | fail open |

### 4.3 Key conventions

- Prefix everything: `rawrz:<component>:<surface>:<key>`.
- Cache values are JSON with an envelope: `{ "v": 1, "fetched_at": …, "provider": …, "payload": … }`
  so provenance survives a cache read (the RPG spec's provenance requirement, §4.5/§4.6).
- TTLs are explicit per surface, never implicit; TTL values live in RAWRZ settings so
  they are tunable without a redeploy.
- No key may contain a secret value or a credential; API keys are never part of cache
  keys.
- Logical DB indexes are reserved even though one instance is used, so a future split is
  a config change not a rewrite: `0` cache, `1` sessions, `2` queues/locks, `3` streams/bus.

### 4.4 Durability policy — the reconciled contradiction

Round 2 of the interview selected "ephemeral, no persistence"; round 7 surfaced that
**queues are not cache** and the resolution was "durable exception for queues". The
spec therefore defines **two policies in one instance**:

| Tier | Membership | Persistence | Eviction | On restart | On Redis loss |
|---|---|---|---|---|---|
| Cache tier | all `*:cache:*`, `*:provider:*`, `*:api:*`, `*:stats:*` | none | `volatile-lru` (TTL keys) | cold cache, warm lazily | degrade to uncached |
| Durable tier | `deck:queue:*`, `deck:lock:*`, `stream:*`, `rpg:session:*` | AOF everysec | never evicted (no TTL) | recovered | fail closed; sessions force re-login |

Consequence to encode in code review: **a key in the durable tier must never be given a
TTL**, or `volatile-lru` becomes able to evict it. A test should assert that queue keys
report `TTL == -1`.

### 4.5 Invalidation

- **TTL-first.** Every cache entry has a TTL; correctness never depends on explicit purge.
- **Event-driven purge** for the hot surfaces: `*arr` Connect and Seerr webhooks publish
  a purge message on `rawrz:bus:invalidate:<service>`; subscribers delete the affected
  key families.
- **Seerr settings flush** (`cache.flush()` in `server/routes/settings/index.ts`) must map
  to a Redis namespace purge, preserving today's semantics.
- **nginx purge**: a small RAWRZ endpoint (in Deck) issues `proxy_cache_purge` to nginx
  for affected routes, called by the same event bus subscriber.
- Purges are idempotent and logged with the triggering event id.

### 4.6 Failure behavior (D11)

- **Redis unavailable at startup:** RAWRZ services start anyway; cache clients enter a
  pass-through mode (every read is a miss, every write is a no-op) and emit a warning
  metric. Nothing silently dies.
- **Redis unavailable at runtime:** the cache tier fail-opens; the durable tier returns
  explicit `503`-class errors for queue/lock operations and Deck surfaces a degraded
  banner. The RPG's gated routes treat session lookup failure as "not authenticated"
  (fail closed for access, but the process stays up).
- **Redis flushed:** cache is cold (fine); queue keys are recovered from AOF, and
  downstream consumers must be idempotent by design (§9.3).
- **Rule:** Redis is never the only copy of anything authoritative. Postgres owns ledger
  truth — watches, XP, achievements, requests, jobs' terminal state (D29 and §11.5
  restate this for feature flags).

### 4.7 Instrumentation

- Per-namespace counters: `hits`, `misses`, `evictions`, `keyspace_hits`, `used_memory`,
  `blocked_clients`, plus RAWRZ-side key counts.
- Exported to the observability tier and rendered in the Deck cache dashboard (§11.4).
- A cache hit ratio below a configured floor raises a Deck alert — this is the
  "cache-hit dashboards" deliverable (D27).

---

## 5. Seerr: the Redis patch

### 5.1 The verified problem

Grounded in the pinned upstream source at `~/TRUTH/seerr` (Seerr v3.4.1, the version the
stack runs as `ghcr.io/seerr-team/seerr:v3.4.1`):

- `server/lib/cache.ts` defines a `Cache` class that wraps **`node-cache`** and a
  `CacheManager` holding 10 named caches: `tmdb`, `radarr`, `sonarr`, `rt`, `imdb`,
  `github`, `plexguid`, `plextv`, `plexwatchlist`, `tvdb`.
- Upstream TTLs (to be preserved exactly): `tmdb` 21600s, `rt` 43200s, `imdb` 43200s,
  `github` 21600s, `plexguid` 604800s, `plextv` 604800s, `tvdb` 21600s, and a
  `DEFAULT_TTL` of 300s for `radarr`/`sonarr`/`plexwatchlist`; `DEFAULT_CHECK_PERIOD` 120s.
- **There is no Redis support anywhere in the Seerr source** (`grep -ri redis` over
  `~/TRUTH/seerr` returns nothing). Seerr cannot be configured into Redis; it must be
  patched.
- The exposed surface is bounded and fully enumerated in the companion patch plan:
  **14 consumer call sites** (10 × `cacheManager.getCache(...)` + 4 × direct `.data`
  accesses), **8 internal touchpoints inside `ExternalAPI`** across four methods (`get`,
  `post`, `getRolling`, `removeCache`), and **3 touchpoints inside `cache.ts`** —
  **11 files** in total. This is what makes a patch tractable.
- `package.json` depends on `node-cache` 5.1.2 and does **not** include `ioredis`; the fork
  adds that dependency and must therefore track the lockfile.

### 5.2 RESOLVED — the sync/async question (R-4)

First-pass analysis assumed `node-cache`'s synchronous API blocked a Redis patch. Reading
every call site shows the opposite: **all cache access happens in async contexts**, so an
async cache backend is a clean conversion rather than a workaround.

Verified surface (`~/TRUTH/seerr`, v3.4.1):

| Site | Access | Context |
|---|---|---|
| `server/api/externalapi.ts:53,65,73,89,93,115,128,147` | `get`/`set` in **four** methods (`get`, `post`, `getRolling`, `removeCache`) plus `getTtl` in `getRolling` | `protected async get<T>()` and its siblings — **all already async** |
| `themoviedb/index.ts:141`, `tvdb/index.ts:56`, `servarr/base.ts:109`, `github.ts:74`, `plextv.ts:151`, `rating/imdbRadarrProxy.ts:167`, `rating/rottentomatoes.ts:119` | pass `.data` into `ExternalAPI` as the `nodeCache` option | constructors calling Seerr's own in-repo `ExternalAPI` |
| `plextv.ts:282,308` | `watchlistCache.data.get/set` | inside `public async getWatchlist()` |
| `lib/scanners/plex/index.ts:388,439` | `guidCache.data.get/set` | inside `private async getMediaIds()` |
| `lib/cache.ts:37,41` | `getStats()`, `flushAll()` | sync signatures on the wrapper class |
| `routes/settings/index.ts:784,787` | `getCache(...)`, `cache.flush()` | async route handler |

Design (adopted):

1. **`CacheBackend` interface** (async): `get<T>(key)`, `set<T>(key, value, ttl)`,
   `del(key)`, **`getTtlMs(key)`**, `flush()`, `stats()`. Two implementations:
   - `MemoryCacheBackend` — today's `node-cache` behavior, unchanged.
   - `RedisCacheBackend` — `ioredis`, keys `rawrz:seerr:cache:<cacheId>:<hash>`, upstream
     TTLs preserved (§5.1), values in the standard RAWRZ envelope.
   `getTtlMs` is **required, not optional**: `getRolling()` reads the remaining TTL to decide
   whether to refresh in the background. `has`/`keys` are unused upstream and are deliberately
   left out of the interface.
2. **`ExternalAPI`** holds a `CacheBackend` instead of a `NodeCache`, and the option is renamed
   `nodeCache:` → `cache:`. Four methods touch the cache and each gets `await` (`get`, `post`,
   `getRolling`, `removeCache`), including the `getTtl` → `getTtlMs` conversion in `getRolling`.
3. **Direct sites** `await` the two `plextv` and two scanner accesses; the seven `.data`
   handoffs pass the backend instead of the raw instance. The settings flush route becomes
   `async`.
4. **`Cache` wrapper** exposes `getStats()`/`flush()` as async for its async caller
   (`routes/settings/index.ts`); stats come from local counters so a stats read never
   blocks on Redis.
5. **Backend selection** via `SEERR_CACHE_BACKEND=redis|memory` (default `memory`), with
   automatic degradation to memory on connect failure or a dropped connection (§5.3).
6. **No L1 mirror.** Rejected deliberately: a read-behind mirror serves the *next*
   request, not the current one, so it buys restart-survival but not real cache hits —
   which is the entire point. With an async backend, a Redis hit serves the request.

Accepted cost: **11 files** whose diff must be re-applied on every upstream sync (§5.5).
That is the price of genuine Redis-backed Seerr caching, and it is why the diff must stay this
small. The full interface, per-file diffs, behavior contract, and test plan live in
`rawrz-seerr-cache-patch-plan.md`.

### 5.3 Behavioral requirements for the patch

1. **Fail-open by default.** If `REDIS_URL` is unset, unparseable, or the connection
   drops, Seerr must fall back to `MemoryCacheBackend` (unchanged `node-cache` behavior)
   and keep serving. A Redis outage must never take Seerr down (D11). Fallback is
   automatic and logged once per transition, not once per request.
2. **TTL parity.** Every upstream stdTtl is preserved as the Redis TTL; the cache ids and
   names stay identical so Seerr's own settings/UI continue to report meaningfully.
3. **Namespace isolation.** Keys use `rawrz:seerr:cache:<cacheId>:<hash>`; values are the
   same serialized payloads upstream cached, wrapped in the standard RAWRZ envelope.
4. **Flush semantics.** `cache.flush()` (settings route) purges only the affected
   namespace, never the whole Redis instance.
5. **No secrets in cache keys or values** — Seerr caches API payloads; the patch must not
   start persisting user sessions, auth tokens, or Plex tokens.
6. **Cookie/session safety note:** because nginx will also cache Seerr responses (§6.5),
   the two tiers must not disagree about what is cacheable; the patch defines an explicit
   "do not cache" list (auth, requests, user, settings).
7. **Observability.** Emit hit/miss counters into the RAWRZ stats namespace.
8. **Interface, not inheritance.** Backends implement one interface; no call site imports
   `node-cache` types except `MemoryCacheBackend`. This is what keeps the upstream diff
   reviewable and rebaseable.
9. **`getStats()`/`flush()` stay usable.** Stats resolve from `MemoryCacheBackend`
   counters or Redis `INFO` without blocking a request; `flush()` purges only the
   `rawrz:seerr:cache:*` namespace (§4.5) — never `FLUSHALL`.
10. **No `node-cache` removal.** The dependency stays for `MemoryCacheBackend`; the patch
    adds `ioredis` and keeps the memory path as the default and the fallback.

### 5.4 Image build and pinning

- The fork publishes an image through RAWRZ CI to `ghcr.io/whispersofj/rawrz-seerr`,
  built from the same base as upstream.
- The stack pins it **by digest**, consistent with the stack's existing pinning posture.
- The fork is added to the Trivy baseline so CVE scanning and the CVE-baseline report
  keep covering Seerr (§11.3, §17 R-3).
- Fallback: if the patch proves unmaintainable, the documented retreat is the
  `~/Cave/.worktrees/cave-deck/catalog/catalog.yaml`-adjacent alternative in §6.4 —
  nginx `proxy_cache` for Seerr, plus Redis for everything else.

### 5.5 Upstream tracking (D39)

- A scheduled workflow watches upstream Seerr releases/tags.
- On a new upstream release it opens an **automated sync PR**: rebase the RAWRZ patch
  branch, run the fork's tests plus RAWRZ cache tests, and build a candidate image.
- The PR must be CI-green to merge; the pinned digest moves only after that.
- The patch is authored as **the smallest possible diff** and is structured to be
  upstreamable (a Redis cache backend behind the existing cache interface is a
  plausible upstream contribution; the spec welcomes landing it upstream and retiring
  the fork, but does not depend on it).
- Version skew discipline from `~/Cave/AGENTS.md` applies: `~/TRUTH/seerr` is re-pinned
  whenever the stack bumps the Seerr image.

---

## 6. nginx ingress

### 6.1 Choice (D13, D14)

**nginx** becomes RAWRZ's ingress tier on the `bearcave` network. This is an explicit,
documented reversal of two prior positions: the RPG spec's "no reverse proxy / no
Traefik" (§11) and the 2026 slim-down that retired Traefik. The reversal is recorded in
§19 so it is not later mistaken for drift.

nginx is chosen over Traefik/Caddy/a custom Rust gateway for one decisive reason: the
`proxy_cache` module provides the "cache all services" requirement (D16) natively, and
nginx is the same proxy whose caching semantics the Seerr patch (§5) is designed to
complement rather than duplicate.

### 6.2 Routing and TLS (D15)

- **Subdomain per service**, served on `*.rawrz.lan` (name to be confirmed at
  implementation; see §18):
  `rawrz.lan`, `deck.rawrz.lan`, `rpg.rawrz.lan`, `seerr.rawrz.lan`,
  `radarr.rawrz.lan`, `sonarr.rawrz.lan`, `prowlarr.rawrz.lan`,
  `nzbdav.rawrz.lan`, `plex.rawrz.lan`, `metrics.rawrz.lan`, `grafana.rawrz.lan`.
- **Internal TLS only**, issued from the existing private mkcert CA already mounted at
  `config/ca` (`/etc/ssl/certs/mkcert` in containers). No public CA, no ACME, no
  exposure. Clients must trust the mkcert root (§17 R-13).
- nginx listens on `80` (redirect to 443) and `443`; the media services keep their
  existing host ports published as an escape hatch during migration, then those
  publications are retired (see §15).
- **DNS:** LAN name resolution is a real prerequisite. Options (resolve in §18):
  `/etc/hosts` entries on client devices, a dnsmasq/AdGuard instance, or mDNS. The proxy
  must work with plain `Host` headers regardless.
- **Plex caveat:** Plex runs on `host` networking (required for GDM/DLNA/remote-access),
  so it is proxied by IP rather than container name; its UI is the least safe thing to
  front, and it keeps its native auth.

### 6.3 Auth gateway

nginx performs `auth_request` to the RAWRZ auth endpoint (§7) for the RAWRZ-owned UIs
(Deck, RPG, dashboards). Service UIs (`Seerr`, `Plex`, `*arr`) keep their native logins
during migration; a later phase may add forward-auth for those that support it. This is
the only way "one auth" (D18) coexists with keeping the media apps operable.

### 6.4 Caching all services (D16)

Cache zones, one per upstream class:

| Zone | Upstream | Cached | TTL (starting point) |
|---|---|---|---|
| `rawrz_images` | Seerr image proxy, Plex artwork, Fanart/TMDB posters | images, artwork, static assets | 7d |
| `rawrz_arr` | Radarr/Sonarr/Prowlarr API GETs | list/detail GETs | 60s–5m |
| `rawrz_seerr` | Seerr API GETs | discovery/search/status GETs | 60s–10m |
| `rawrz_plex` | Plex metadata/library GETs (never playback) | metadata GETs | 5m |
| `rawrz_rpg` | RAWRZ RPG/Deck API GETs | board/status GETs (never ledger writes) | 30s–2m |

- Responses always carry **`X-Cache-Status`** (`HIT`/`MISS`/`BYPASS`/`EXPIRED`) so the
  dashboards can prove the tier is working.
- Cache keys include scheme, host, URI, and query string. Per D17 they do **not** include
  the cookie by default.
- `proxy_cache_bypass`/`proxy_no_cache` honor upstream `Cache-Control: no-store` on
  mutating or auth endpoints.
- **Never cached, at any tier:** auth/login, session, logout, user, request-approval,
  settings mutation, playback/progress, webhook receivers, and anything with
  `Set-Cookie`.

### 6.5 Guardrails and the explicit risk acceptance (D17)

The interview chose **"aggressive, LAN trust"**: cache broadly and accept that a
single-user LAN makes cross-session leakage a non-issue.

The spec records that choice *and* its boundary, because the risk changes the moment the
premise does:

- **Accepted:** authenticated GETs may be cached without a per-user cache key. On a
  single-operator LAN this is a performance win.
- **Condition of acceptance:** LAN-only is preserved (also a preserved rule). If RAWRZ
  ever gains a second user, remote access, or an untrusted device on the LAN, this
  policy must be revisited before exposure — the mitigation is one directive
  (`proxy_cache_key "$scheme$host$uri$is_args$args$cookie_session"`) plus a purge.
- **Recommended minimum safeguard (non-blocking):** keep the `Set-Cookie` exclusion
  above and add an explicit allowlist file (`nginx/cacheable-routes.conf`) so "cache
  everything" is enumerable rather than implicit.
- **Never cache** Plex playback/session endpoints even under this policy.

### 6.6 nginx operations

- Config lives in `services/nginx/` (conf.d + zones), versioned; changes are normal PRs
  under compose-is-truth (D27).
- Health: `nginx -t` in CI, `/healthz` on the container, and a smoke request per
  subdomain in the acceptance checklist (§16).
- Purge endpoint restricted to the RAWRZ bridge network; never exposed on a subdomain.
- Logs go to the observability tier; cache hit/miss ratios are scraped per zone.

---

## 7. Unified auth (D18)

### 7.1 Model

The RPG's existing PIN gate is generalized into the single RAWRZ auth authority:

- **Factor:** one Argon2id-hashed PIN (4–12 digits), no usernames/passwords — consistent
  with the RPG's finalized model (`movie-rpg-spec.md` §6.4.1, §7.3).
- **Sessions move to Redis** (`rawrz:rpg:session:*` / logical DB 1): opaque 128-bit
  tokens, HttpOnly + SameSite=Lax cookie scoped to `.rawrz.lan`, 7-day TTL with
  **sliding renewal** and **sweep on issue**.
- **Logout** revokes the Redis entry and clears the cookie; a revocation counter/version
  also invalidates anything the caches still hold.
- **Cookie `Secure` flag becomes enforceable** now that nginx terminates TLS — this
  closes the RPG's deliberately-deferred FIX.md F-13 item.
- **Throttling** (5 failures → 30s×2ⁿ lockout) is preserved. With Redis available it can
  become shared across processes instead of per-process in-memory — a strict improvement
  over FIX.md F-10, and it survives restarts.
- **Session persistence across restarts** (FIX.md F-39's "restart clears sessions") is
  resolved by design.

### 7.2 What each surface gets

| Surface | Auth today | In RAWRZ |
|---|---|---|
| RPG UI/API | PIN gate + in-memory session | PIN gate + Redis session; unchanged UX |
| Deck UI/API | none (LAN-only) | same PIN gate via nginx `auth_request` |
| Dashboards (Grafana, metrics) | n/a | same PIN gate |
| Seerr / Plex / *arr UIs | native logins | native logins retained (phase 1); forward-auth explored later |
| Inter-service calls | API keys | API keys (unchanged) + mTLS as an optional later experiment |

### 7.3 Rules

- The auth service is part of `backend/deck` (it already owns host-facing surfaces) or a
  small shared crate — the choice is deferred (§18), but **there must be exactly one
  implementation**, used by nginx, Deck, and the RPG.
- The RPG's server-side-authority rule is preserved: **auth sessions never grant
  progression**; unlocking the UI does not fabricate watches, XP, or achievements.
- Session values are opaque and carry no PII beyond a role and timestamps.

---

## 8. Data layer (D19, D20, D21)

### 8.1 One instance, separate databases

Postgres becomes a compose service on `bearcave` (D34), with data on
`./config/postgres:/var/lib/postgresql/data`.

| Database | Owner | Notes |
|---|---|---|
| `rawrz_rpg` | `backend/rpg` | the existing 14 migrations, renamed env `RAWRZ_RPG_DB_URL` |
| `rawrz_deck` | `backend/deck` | ported from Deck's SQLite `cave-deck.db`; Deck's sqlx migrations move too |
| `radarr-main`, `radarr-log` | Radarr | from SQLite; **app-default names** (verified in `ConfigFileProvider`) |
| `sonarr-main`, `sonarr-log` | Sonarr | from SQLite; app-default names |
| `prowlarr-main`, `prowlarr-log` | Prowlarr | from SQLite; app-default names |

Separate databases (rather than schemas in one database) were chosen to keep migration
tooling, backups, and blast radius independent per component (D20). The \*arr databases use
each application's **own default names**, so switching an app to Postgres needs no extra
config keys (see `rawrz-arr-db-migration-runbook.md` §1).

**Verified feasibility:** all three `*arr` apps support PostgreSQL in the exact pinned
versions the stack runs. Confirmed by reading `~/TRUTH`:
`ConnectionStringFactory` in each app resolves `PostgreSqlVars` /
`PostgreSqlConnectionString` (`sonarr …/Datastore/ConnectionStringFactory.cs:25`,
`prowlarr …/Datastore/ConnectionStringFactory.cs:30-34`, `radarr …/Datastore/ConnectionStringFactory.cs`),
with `DatabaseType.PostgreSQL` handled in `BasicRepository.cs`/`DbFactory.cs`, and the
`PostgresHost/Port/User/Password/PostgresMainDb/PostgresLogDb` config keys present in
`ConfigFileProvider`. This is a supported configuration, not a hack.

### 8.2 Connection sizing

- Set `max_connections` for the union of consumers: Radarr, Sonarr, Prowlarr (each with
  main+log pools), RPG (`deadpool-postgres`), Deck (sqlx pool), and scripts/jobs.
- Budget it explicitly and monitor; the *arr apps' default pool sizes are not tuned for a
  shared instance. A request queue exhaustion would surface as *arr API 500s — the same
  class of failure the stack already documents for orphaned quality profiles.
- Use per-app roles with least privilege: each app owns its own databases only.

### 8.3 Ownership scope (D23)

RAWRZ becomes the full owner of the media apps and is allowed to **write configuration
into them**:

- Indexers/sync in Prowlarr; applications sync to Radarr/Sonarr.
- Quality profiles, custom formats, and scores (Recyclarr's job — Recyclarr stays as a
  maintenance-profile service, now invoked by RAWRZ automation rather than by hand).
- Notifications/webhooks: RAWRZ registers its own Connect webhooks (§9.2).
- Backups, integrity gates (e.g. the existing `check_radarr_db_size.py` blob-bloat gate,
  the `stack-radarr-prune`/`stack-sonarr-prune` remediation flows), and lifecycle are
  driven from RAWRZ.

Hard boundary that *stays*: RAWRZ never fabricates watch completions or writes
playback state. It may configure and observe Plex/*arr; it may not assert that something
was watched. That rule is what keeps the game honest and is preserved regardless of how
much ownership is granted.

### 8.4 Migration posture (D22)

- **Maintenance window**, with a verified backup first for every database, and the
  original SQLite files preserved read-only for rollback.
- Order: Prowlarr (least gameplay-critical) → Sonarr → Radarr (the one with the 1.5 GB,
  MediaInfo-bloated DB and the documented orphaned-quality-profile landmine).
- Verification per app: item counts, representative record equality spot-checks,
  quality-profile resolution, API health, and a rescan.
- The exact SQLite→Postgres mechanism must be confirmed against the pinned versions
  during implementation (§18). Upstream documents a supported migration path for these
  versions; a `pgloader`-style dump/restore is the fallback. This is *not* to be
  improvised on the live database.
- Rollback: stop app → restore SQLite → point config back → start. The SQLite files are
  never deleted during the merge, only after a soak period.

### 8.5 Postgres operations — hardened single instance (resolved)

R-1 was resolved by dropping the media-availability rule, which makes Postgres the
single most consequential service in RAWRZ. The chosen resilience level is a **single
hardened instance plus backups** — no PITR, no replica, no automatic failover (R-15).
Hardening requirements:

- `restart: always` (not `unless-stopped`) so an OOM-killed Postgres comes back without
  operator action; explicit healthcheck (`pg_isready`) and a container start period.
- Resource reservation and a cap budgeted in §11.6; Postgres must not be the container
  that gets starved when Prowlarr/Sonarr scans spike.
- Tuned `max_connections` sized to the documented consumer set (§8.2) with monitoring and
  an alert before exhaustion.
- Backups join the existing `scripts/backup.sh` flow on a schedule, and **restores are
  rehearsed**, not assumed (§16.3). Dumps are verified (`pg_restore --list` / restore to
  scratch), not merely written.
- Alerts on: instance down, replication of nothing but connection saturation, disk growth,
  and backup failure.
- The RPG's FIX.md F-1 hazard (a proof run able to wipe its target database) is re-checked
  against the unified stack: the e2e tier must use a scratch database, never the live one.
- Documented, accepted consequence: **a Postgres outage is a media outage.** The mitigation
  is reliability engineering, not architectural separation.

---

## 9. Seams: control plane, events, ownership

### 9.1 One control plane (D19, D23)

RAWRZ Deck becomes the single control plane:

- Container lifecycle, compose apply, catalog install/uninstall — as today.
- Redis: install/configure/inspect, cache dashboards, namespace flush, memory/eviction
  views.
- Postgres: connection health, database inventory, backup status.
- nginx: zone list, route table, purge, hit/miss ratios.
- RPG: sync status, last poll, current tick summary, character/level summary, and the
  ability to trigger `POST /api/orders/refresh`-equivalent work.
- Feature flags (§11.5) and the audit log for every mutating action (Deck's existing
  audit model extends to cover RAWRZ-wide actions).

### 9.2 Event sources (D24)

| Source | Events | Consumer |
|---|---|---|
| Radarr/Sonarr Connect | `Download`/`Import`, `Grab`, `Rename`, `Health`, `Test` | Deck (activity feed, cache purge), RPG (new-arrival cards) |
| Seerr notify webhook | request created/approved/available/declined | Deck (requests view), RPG (coursework/featured hints) |
| Plex webhooks | `media.play`, `media.stop`, `media.scrobble`, `playback` | RPG (near-real-time watch signal), Deck (session activity) |
| Internal bus | RAWRZ component events (cache purge, job state, flag change) | all RAWRZ services via Redis pub/sub + streams |
| Polling (retained) | RPG 5-minute sync → tick; Deck health polls | reconciliation source of truth |

Rules:

1. **Webhooks are hints, not truth.** A Plex webhook never writes a `watches` row
   directly. The RPG still reads Plex server-side state to confirm the ≥95% completion
   before awarding (the RPG spec's core rule, preserved). Webhooks shorten latency;
   polling guarantees correctness.
2. **Idempotency everywhere.** Every webhook carries a delivery id; consumers dedupe on
   `(source, delivery_id)` before acting. *arr and Plex retry deliveries.
3. **Ordering.** Events enter a Redis stream (`rawrz:stream:events`), are consumed by
   component workers, and mutations land transactionally in Postgres. Streams are the
   durable, replayable tier; pub/sub is fire-and-forget for UI updates.
4. **Polling reconciles.** Anything a missed webhook dropped is caught by the next poll
   cycle. Webhooks may be disabled (feature flag) and the system stays correct.
5. **Webhook receivers are never cached** by nginx and are not exposed on a subdomain
   unless explicitly required.

### 9.3 Realtime

- Deck's existing WebSocket plan (tokio-tungstenite) becomes the single hub.
- Redis pub/sub fans out events to the hub; the hub pushes to browsers.
- RPG and Deck UIs subscribe for: watch awards, level-ups, order reveals, job progress,
  cache stats, container state, flag changes.
- WebSocket connections are auth-gated by the same session cookie; origin-checked.

### 9.4 Cross-app features (D27)

Concrete first set (this is the "bells and whistles" budget):

1. **RPG honors in Deck** — achievements, level, streak, and current coursework visible
   in the Deck overview without opening the RPG.
2. **Request → coursework** — a Seerr request becoming available creates/updates RPG
   coursework cards and can feed the featured-case ranking.
3. **Watch → Deck action** — a completed watch can trigger Deck actions (activity entry,
   library scan nudge, cache warm of that item's artwork).
4. **Stack health → RPG context** — sync/provider degradation surfaces in the RPG as
   "academy ledger paused" rather than silent stale numbers.
5. **Unified search** — one search box spanning *arr/Plex/Seerr/RPG content, backed by
   the API cache tier.
6. **Cache-aware UI** — every list view shows whether it was served from cache.

---

## 10. Deployment, ops, configuration

### 10.1 Everything containerized (D34)

New/relocated services on `bearcave`:

| Service | Image/binary | Port | Notes |
|---|---|---|---|
| `rawrz-redis` | `redis:7-alpine` | none published | §4 |
| `rawrz-postgres` | `postgres:17-alpine` | none published | §8 |
| `rawrz-nginx` | `nginx:alpine` | `80`, `443` | §6 |
| `rawrz-rpg` | locally built Rust image (`backend/rpg`) | internal `46532` | was a host process |
| `rawrz-deck` | locally built Rust+embedded frontend image (`backend/deck`) | internal `7780` | was a container via submodule |
| observability tier | Prometheus + Grafana + exporters (+ log aggregation) | internal, via subdomains | §11.3 |

Consequences:

- No host daemons remain for RAWRZ (Postgres moves off the host; the RPG stops being a
  systemd-ish host process).
- The `cave-deck-host.service` systemd shim (used for ufw, pacman, journalctl, SMART,
  timers) **stays** — the container still cannot do those things. It remains a thin
  allowlist, unchanged in principle.
- Host `sudo`-free posture is preserved; nothing outside the project directory is written
  by CI.

### 10.2 Shared fate (D25) — RESOLVED, with the availability rule dropped

Round 7 chose **"accept shared fate"** (one compose world); the interview also initially
preserved "media pipeline must survive app outages". Those could not both hold once
Radarr/Sonarr/Prowlarr moved onto the containerized Postgres: Postgres is the only new
hard dependency any media service gained (Redis is fail-open, nginx only fronts UIs,
observability is pull-based, and no media service depends on an app-tier service today —
the compose `depends_on` graph references only `nzbdav`, `nzbdav_rclone`, and `prowlarr`).

The resolution round chose to **formally drop the availability rule** rather than split
the stack into tiers, reverse the *arr DB migration, or keep a rule it could not honor.

What that means in practice:

- A Postgres outage **is** a media outage. So is an nginx outage if a UI is the only way
  in. This is accepted, not mitigated by architecture.
- The one availability property that survived is structural: **Redis is never required.**
  Every consumer is fail-open (§4.6), so Redis — the riskiest new component — cannot
  degrade media availability at all.
- Because the rule is dropped, Postgres hardening becomes the load-bearing mitigation
  (§8.5) and is tracked as its own risk (R-15).
- The e2e tier asserts the accepted behavior explicitly (§16.1 step 6) so the failure
  mode is known rather than discovered in production.

### 10.3 Port map

| Port | Service | Exposure |
|---|---|---|
| 443 / 80 | nginx (subdomain ingress) | LAN, TLS |
| 46532 | RAWRZ RPG | internal only (was LAN-direct) |
| 7780 | RAWRZ Deck | internal only (was LAN-direct) |
| 6379 | Redis | bridge network only |
| 5432 | Postgres | bridge network only |
| 5055 | Seerr | proxied; published port retired post-cut-over |
| 7878 | Radarr | proxied; published port retired post-cut-over |
| 8989 | Sonarr | proxied; published port retired post-cut-over |
| 9696 | Prowlarr | proxied; published port retired post-cut-over |
| 3000 | nzbdav (WebDAV) | proxied; **WebDAV for rclone may need to stay direct** |
| 32400 | Plex (host network) | proxied by IP; direct access retained |

The nzbdav WebDAV path is the one that must not be casually moved behind a cache: the
rclone FUSE mount reads from it, so caching or rewriting that traffic risks the mount.

### 10.4 Volumes and persistent state

- `config/<service>/` for every service, now including `config/redis/`,
  `config/postgres/`, `config/nginx/`, `config/grafana/`, `config/prometheus/`.
- `secrets/` remains gitignored and in use.
- `backups/` gains Postgres dumps alongside the existing backup output.
- `media/` symlink trees unchanged.
- Bind-mount staleness landmine still applies: **restart containers after editing
  bind-mounted files** (`sed -i`/`vim` changes the inode; the container serves stale
  content with no error).

### 10.5 One root `.env` (D33)

- A single canonical `.env` at the repo root is the union of the stack's keys
  (`RADARR_API_KEY`, `SONARR_API_KEY`, `PROWLARR_API_KEY`, `PLEX_TOKEN`, `PLEX_CLAIM`,
  `NZBDAV_*`, `HOST_IP`, `TZ`, `PUID`, `PGID`, `RELEASE_PLEASE_TOKEN`, …) and the RPG's
  keys (`RPG_DB_URL`, `TMDB_API_KEY`, `TVDB_API_KEY`, `OMDB_API_KEY`, `FANART_API_KEY`,
  `RPG_BIND_ADDRESS`, …) plus the new `REDIS_URL`, `POSTGRES_*`, `RAWRZ_*` keys — all
  renamed per §12.
- `.env.template` is updated in the same PR; secrets are never committed.
- `secrets/` (Docker secrets) is kept as-is.
- **Key-collision rule:** where both repos define the same concept (e.g. TZ, HOST_IP,
  DB URLs), the stack's key name wins and the RPG's key is renamed, and the rename table
  in §12.3 is the record.

---

## 11. The bells, whistles, and deliberate over-complexity

### 11.1 Stance (D26)

Maximal is the point. The spec optimizes for exploring what a small stack can be made to
do, while requiring each addition to be **isolated enough to turn off** (flags, profiles,
or a single compose revert). Complexity is allowed; *unrecoverable* complexity is not.

### 11.2 Committed scope (D27)

| Feature | Shape | Off-switch |
|---|---|---|
| Observability stack | Prometheus + Grafana + exporters + log aggregation, all services scraped | compose profile + flag |
| Cache-hit dashboards | Per-tier hit/miss/eviction/TTL views in Deck | Deck view flag |
| Redis event bus + realtime | Streams + pub/sub + WebSocket hub | flag (falls back to polling only) |
| Cross-app features | §9.4 set | per-feature flags |
| Compose-is-truth expansion | Every config change lands as an automated PR | process rule |
| Cache everything | nginx zones + Redis namespaces per §4/§6 | per-zone/per-namespace disable |
| Feature flags / experiments | Registry + UI toggles + audit | §11.5 |

### 11.3 Observability tier (D28)

- Re-adopting Prometheus/Grafana-class tooling is a **deliberate reversal** of the
  2026-08-30 slim-down (which retired traefik, loki, promtail, grafana, prometheus,
  alertmanager, node-exporter, cadvisor and more). The reversal is recorded in §19 and
  is budgeted in §11.6.
- Scrape targets: container metrics, nginx per-zone cache stats, Redis stats, Postgres
  stats, *arr/Plex/Seerr health, RPG tick counters, Deck job counters.
- Traces: OpenTelemetry-style tracing across Deck → nginx → upstream is in scope as an
  experiment, behind a flag; the RPG's `tracing` dependency removal (FIX.md F-24 note) is
  revisited here rather than in isolation.
- Alerts: Redis down/degraded, Postgres connection exhaustion, nginx cache hit ratio
  floor, *arr API errors, RPG poll failures, disk/cache growth (`config/nzbdav-rclone/cache`).

### 11.4 Cache instrumentation

Every cache tier (nginx zones and Redis namespaces) exposes hit/miss/eviction/size.
Deck renders them next to the services they protect, so "cache everything" is provable
rather than aspirational.

### 11.5 Feature flags (D29)

- **Authoritative flag state lives in Postgres** (`rawrz_deck.flags`), not Redis, because
  Redis is ephemeral and fail-open. Redis may hold an in-memory copy for fast reads;
  losing it costs latency, never truth.
- Precedence: env override → Postgres → default. Env override exists so a broken flag can
  be corrected without a DB round-trip (break-glass).
- Every flag has: key, description, default, owner, created-at, and a documented removal
  date for experiments.
- Flag changes are audit-logged and (optionally) published on the event bus.
- Flags may never gate: watch awarding, progression truth, ledger writes, backup jobs
  (those are correctness, not features).

### 11.6 Memory budget (mandatory, because the slim-down was memory-driven)

The host has **22 GiB**; the 8-service stack's caps total **≈12.1 GiB**, and the slim-down
existed because ~19 GiB of caps against 22 GiB caused OOM incidents (Bazarr crash-loop,
Radarr 500s). RAWRZ must therefore publish a budget and monitor it.

Starting budget (to be finalized at implementation):

| Addition | Suggested cap |
|---|---|
| `rawrz-postgres` | 1024m |
| `rawrz-redis` | 512m |
| `rawrz-nginx` | 128m |
| `rawrz-rpg` | 256m |
| `rawrz-deck` | 512m |
| observability (all) | 1536m |
| **Added total** | **≈4.0 GiB** |
| **New grand total** | **≈16.1 GiB of 22 GiB** |

The headroom is real but thin for concurrent scans/downloads. If the observability tier
creeps, it is the first thing to profile-ize or cap down.

---

## 12. The RAWRZ rebrand (D30, D31)

### 12.1 Naming

| Today | After |
|---|---|
| The Bear Cave / `TheBearCave` | **RAWRZ Stack** |
| Cave Deck / `cave-deck` | **RAWRZ Deck** |
| Movie / TV RPG, Lantern Academy | **RAWRZ RPG** |
| `movie-rpg-spec.md` | `docs/rpg/<new name>.md` (sub-spec) |
| `cave-deck-spec.md` | `docs/deck/<new name>.md` (sub-spec) |
| `AGENTS.md` (Cave) | `docs/stack/AGENTS.md` + root `AGENTS.md` |
| `CLAUDE.md`, `FIX.md`, `HANDOFF.md` | consolidated under `docs/agents/` |

### 12.2 Cadence-first rule

RAWRZ is a name and a brand, not a claim about the media. The RPG's original-IP policy
(`movie-rpg-spec.md` §2.2) is preserved in full: no franchise names, characters, logos,
spell names, film artwork, or artist imitation. Because the **Lantern Academy theme is
in scope for renaming/retheming under RAWRZ** (D30), the retheme must be authored inside
those boundaries — original academy world, original vocabulary, original or licensed
assets — and is a design task in its own right (§18).

### 12.3 Rename table (initial)

| Surface | Today | After |
|---|---|---|
| Repo | `movie-rpg`, `TheBearCave`, `cave-deck` | `rawrz` |
| Rust crate/bin (RPG) | `movie-rpg` | `rawrz-rpg` |
| Rust crate/bin (Deck) | `cave-deck` | `rawrz-deck` |
| Compose services | `seerr`, `plex`, … / `cave-deck` | media services unchanged; apps → `rawrz-rpg`, `rawrz-deck`, `rawrz-redis`, `rawrz-postgres`, `rawrz-nginx` |
| Env: RPG DB | `RPG_DB_URL` | `RAWRZ_RPG_DB_URL` |
| Env: Deck | `CAVE_DECK_SMTP_USER`, … | `RAWRZ_DECK_SMTP_USER`, … |
| Env: bind | `RPG_BIND_ADDRESS` | `RAWRZ_RPG_BIND_ADDRESS` |
| Env: new | — | `REDIS_URL`, `POSTGRES_HOST/USER/PASSWORD`, `RAWRZ_ENV` |
| Env: stack/media services | `RADARR_API_KEY`, `PLEX_TOKEN`, … | **unchanged** (they're vendor contract names) |
| Databases | `RPG_DB_URL` target, `cave-deck.db` (SQLite) | `rawrz_rpg`, `rawrz_deck`, `radarr-main`, `sonarr-main`, `prowlarr-main`, … (app defaults) |
| Ports | 46532, 7780 | internal-only, unchanged numbers (less churn, fewer client updates) |
| Systemd shim | `cave-deck-host.service` | `rawrz-host.service` |
| GHCR image | `ghcr.io/seerr-team/seerr` | media images unchanged; fork `ghcr.io/whispersofj/rawrz-seerr` |
| Release manifest | three manifests | one |

**Vendor-owned identifiers are explicitly exempt** from the rebrand: `RADARR_*`,
`SONARR_*`, `PROWLARR_*`, `PLEX_*`, `NZBDAV_*`, container images for upstream services,
and *arr config keys. Renaming those would break contract boundaries for no benefit.

**No compatibility shims.** The rebrand is a single cut-over with a documented table
(above), executed as its own milestone, not spread opportunistically through other work.

---

## 13. Documentation model (D32)

- **`rawrz-megastack-spec.md` (this file) is the master spec**: architecture, seams,
  decisions, plan, risks.
- **Component sub-specs remain authoritative for their own domains**, relocated:
  - `docs/rpg/` ← `movie-rpg-spec.md` (the canonical 175 KB copy), and the RPG's
    agent-facing lineage (`FIX.md`, `HANDOFF.md`) moves to `docs/agents/`.
  - `docs/deck/` ← `cave-deck-spec.md`.   - `docs/stack/` ← the Bear Cave's `AGENTS.md` content, `docs/stack/API.md`, landmines,

    lifecycle, service docs.
- **Companion plans** live under `docs/plans/`: `rawrz-m0-plan.md`,
  `rawrz-postgres-hardening.md`, `rawrz-arr-db-migration-runbook.md`, and
  `rawrz-seerr-cache-patch-plan.md`; the master spec links them from its header.
- **One unified agent contract** at the repo root (`AGENTS.md`, plus `CLAUDE.md` if kept)
  pointing at the sub-specs; no competing root-level agent docs.
- The RPG's **supersession notes must be edited** where the master spec overrides it
  (§19) so the sub-spec is never internally contradicted.
- The stale spec copies in `~/Cave` and `~` are discarded, not merged (§3.2).

---

## 14. Agent & developer workflow in RAWRZ

- Worktrees live inside the repo: `.worktrees/<task-name>` (the Cave rule that site
  worktrees must live inside the repo is kept; the "under `/home/bear/cave/`" path is
  updated to the RAWRZ checkout).
- `~/TRUTH` remains the source-of-truth corpus for upstream behavior, now including the
  Seerr fork; the map gains a row for `rawrz-seerr` and re-pins on every Seerr bump.
- CI tiers (unified): compose validation + actionlint + shellcheck/ruff; both Rust crates
  (fmt/clippy/test); catalog validation; Trivy scan (incl. the fork); e2e tier (§16).
- Pre-push hook runs the extended preflight (secret-drift guard, DB-integrity checks,
  compose validation).

---

## 15. Migration plan (phased, cut over when green)

Each milestone is independently mergeable and reversible. Nothing in a milestone may
break the running stack; the live media pipeline stays up until M7.

| # | Milestone | Outcome | Runtime risk |
|---|---|---|---|
| **M0** | **RAWRZ skeleton + CI + docs** *(chosen first milestone)* | New repo exists; three histories imported as subtrees; unified CI green (both crates build, compose validated); master spec + sub-specs in place; catalog moved; agent docs unified; **no runtime change** | none |
| **M1** | Redis lands **inside the current stack** | `rawrz-redis` container up (no host port), instrumented; the caching thesis is proven on real traffic through non-Seerr consumers first | low (new container only) |
| **M2** | Seerr fork prototype | Patch built, image published to GHCR, digest-pinned test run **alongside** the current Seerr (port-swapped), cache hit/miss measured vs upstream | medium (one service, rollback = revert pin) |
| **M3** | nginx + subdomains + TLS | nginx in front of all services; `X-Cache-Status` and zones live; published ports still available | medium (new ingress path) |
| **M4** | Containerize the RPG + Postgres | RPG becomes a service; Postgres containerized with `rawrz_rpg`; RPG sessions move to Redis; polling/webhooks both verified | medium-high (DB move) |
| **M5** | Deck ported in-tree | `backend/deck` + `rawrz_deck` Postgres; submodule retired; Deck owns Redis/nginx/Postgres dashboards; feature flags land | medium |
| **M6** | Seerr cut-over | Production Seerr is the fork; upstream sync workflow active; old image retired | medium |
| **M7** | \*arr DB migration to Postgres | Prowlarr → Sonarr → Radarr onto the shared instance, maintenance window, verified backups, soak, rollback available | **high** |
| **M8** | Ownership + events + cross-app | Connect/Seerr/Plex webhooks live; streams + realtime; cross-app features; observability tier complete; compose-is-truth for everything | medium |
| **M9** | Cut-over + archival | RAWRZ runs the host; direct published ports retired; three source repos archived with redirect READMEs; rollback drill rehearsed and documented | medium |

### 15.1 M1 delivery contract

M1 adds Redis without changing any existing media-service dependency graph. The
service runs on `bearcave` with no published host port, persists its AOF under
`./config/redis`, uses `volatile-lru`, and is capped at 512 MiB. The host-run
activity feed is the first non-Seerr consumer: it caches each Radarr/Sonarr
history page for 60 seconds under its own namespace, reports hit/miss/error
counters, and falls back to the live API when Redis is unavailable. The
committed contract is checked offline by `scripts/check_redis.py`, while
`scripts/test_activity_feed.py` and `scripts/test_redis_runtime.py` exercise
cache hits, TTL, AOF restart persistence, and fail-open outage behavior. Rollback
is removing the activity-feed cache seam and Redis service; the media pipeline
has no Redis dependency.


Batching rule: M7 (the \*arr DB migration) requires **Postgres hardening and a rehearsed
restore in place first** (§8.5). The R-1 resolution dropped the media-availability rule,
so Postgres reliability is now what protects the pipeline — not architectural separation.

---

## 16. Verification & acceptance (D36, D38)

### 16.1 CI e2e on an ephemeral compose stack (the gate)

A CI job (nightly + on PRs touching compose/services/backends) that:

1. Brings up the merged stack from scratch in a clean environment (scratch Postgres, no
   host state), using the repo's compose + a CI env file.
2. Runs the existing pipeline test shape: request → download → serve → play (mocked or
   fixture-driven where the internet is unavailable).
3. Runs the RPG loop: sync → tick → watch award → order reveal → achievement unlock.
4. Asserts cache behavior: a second request for the same resource returns
   `X-Cache-Status: HIT`; Redis keyspace shows the expected namespace and TTL; queue keys
   report `TTL == -1`; and Seerr (with `SEERR_CACHE_BACKEND=redis`) serves a repeat lookup
   from Redis rather than upstream.
5. Asserts webhook idempotency: duplicate deliveries produce one ledger row.
6. Asserts failure modes: kill Redis → services stay up and degrade, including the
   patched Seerr falling back to `MemoryCacheBackend`; kill nginx → healthchecks report;
   kill Postgres → media services report unavailable and **recover automatically on
   restart** (the accepted failure mode per R-1, asserted rather than assumed).
7. Tears down cleanly; uses a scratch database and can never touch a real one
   (FIX.md F-1's lesson).

The CI tier must not require a real Plex or an internet connection for the core assertions.

### 16.2 Live acceptance checklist

A manual, documented run on the real host covering: every subdomain over TLS from a
LAN device, the full media pipeline, the RPG daily loop, the Deck's dashboards, cache
hit ratios, the Redis/Postgres/nginx degradation paths, backups, and the host shim's
operations. Evidence (logs, screenshots, metrics) is attached to the cut-over PR.

### 16.3 Rollback drill

Before M9: rehearse restoring the pre-RAWRZ stack — media services back on published
ports, SQLite files restored, RPG back to its host process, ingress stopped. The drill's
timing and steps are recorded.

### 16.4 Definition of done (D38)

Cut-over complete on the live host **and** the new feature set proven: observability
data flowing, cache dashboards showing real hit ratios, realtime events in the UI,
cross-app features working, flags toggling, compose-is-truth enforced.

---

## 17. Risks & open tensions (register)

| # | Risk | Why it exists | Mitigation / decision needed |
|---|---|---|---|
| **R-1** | ~~Shared fate vs media availability~~ **RESOLVED** | The interview both accepted shared fate (D25) and preserved "media survives app outages"; incompatible once *arr apps run on the containerized Postgres | **Resolution: the availability rule is dropped.** Postgres is hardened as the critical service (§8.5) and its outage is an accepted media outage (R-15) |
| R-2 | `*arr` SQLite→Postgres migration | Large DBs (1.5 GB Radarr, 3.2 GB Sonarr per the landmine log), MediaInfo blobs, no rehearsed procedure yet | Maintenance window + verified backups + Prowlarr-first ordering + soak + retained SQLite (§8.4) |
| R-3 | Seerr fork maintenance | Maintains an upstream diff forever; upstream may change the cache API | Automated sync PR + CI gate + smallest-possible diff + documented retreat to nginx `proxy_cache` (§5.5) |
| R-4 | ~~Sync vs async cache patch~~ **RESOLVED** | Seerr's `node-cache` API looked like a hard synchronous barrier | **Resolution: async `CacheBackend` patch** (§5.2). Every call site verified async-capable, so no L1 mirror is needed; cost is **11 files** per upstream sync (`rawrz-seerr-cache-patch-plan.md`) |
| R-5 | Cache-all auth leakage | Aggressive caching without cookie keys (D17) | Explicitly accepted on a single-user LAN; documented boundary + recommended route allowlist + never-cache list (§6.5) |
| R-6 | One Redis, two durability policies | Cache and queues share an instance | `volatile-lru` structurally protects no-TTL queue keys; test asserts queue `TTL == -1`; logical DBs reserved for a future split (§4.4) |
| R-7 | nginx is a new single point of failure | Every UI now depends on it | Keep published ports as an escape hatch until M9, healthchecks, and a documented bypass path |
| R-8 | Observability memory creep | The slim-down happened because of memory, and RAWRZ adds ~4 GiB of caps | Budget table (§11.6) + alerts + profile-ize/cap-down as the first remedy |
| R-9 | Webhook trust | Client-adjacent events could be spoofed or replayed | Webhooks are hints only; server-side confirmation before any ledger write; delivery-id dedupe; receivers not cached, not subdomain-exposed (§9.2) |
| R-10 | Rebrand churn across env/DB/ports | D30 renames identifiers that live in `.env`, secrets, DBs, workflows | Single cut-over with a rename table; vendor identifiers exempt; no shims (§12.3) |
| R-11 | nzbdav WebDAV behind a proxy | The rclone FUSE mount depends on WebDAV reliability; the stack documents FUSE fragility as a landmine | Keep WebDAV traffic uncached and preferably direct; never proxy-cache it (§10.3) |
| R-12 | Subdomain DNS prerequisite | `*.rawrz.lan` needs LAN name resolution | Decide hosts-file vs dnsmasq/AdGuard vs mDNS (§18) |
| R-13 | mkcert trust on every client | Internal TLS requires installing the CA per device | Document the trust step; fall back to HTTP per-service if a device can't |
| R-14 | Spec contradictions left in place | The RPG spec, FIX.md, Cave AGENTS.md, and Deck spec all contain rules this document reverses | §19 lists every supersession; sub-specs are edited in M0 |
| R-15 | **Postgres is now media-critical** | R-1 dropped the availability rule, so a Postgres outage stops Radarr/Sonarr/Prowlarr (and RPG/Deck) | Single hardened instance: `restart: always`, healthcheck, resource reservation, tuned `max_connections`, scheduled verified dumps, rehearsed restore, alerts (§8.5). No replica or automatic failover was chosen — accepted risk |

---

## 18. Open questions (deferred, non-blocking)

1. ~~**R-1 resolution** (§17)~~ — **resolved** in the resolution round: the media-availability rule is dropped (§2.1, §10.2).
2. **Version restart** — continue `1.35.x` or restart at `2.0.0` for the rebrand.
3. **LAN DNS mechanism** — `/etc/hosts`, dnsmasq/AdGuard, or mDNS.
4. **Subdomain root** — `.rawrz.lan`, `.cave.lan`, or another internal TLD.
5. **Auth service home** — inside `backend/deck` or a shared `rawrz-auth` crate.
6. **Observability composition** — Prometheus+Grafana+Loki vs a lighter exporter-only
   tier vs re-adopting the exact retired set.
7. **\*arr migration tooling** — upstream-supported path vs pgloader-style dump/restore,
   confirmed against the pinned versions.
8. **Prowlarr log DB** — migrate or leave logging on SQLite.
9. **Tile/theme design** — the RAWRZ retheme of the academy presentation (design task,
   inside the original-IP boundary).
10. **Deck SQLite port scope** — whether any Deck data is intentionally *not* moved.
11. **Trace sampling policy** and retention for logs/traces.
12. **Whether direct published ports are retired permanently** at M9 or kept as a
    permanent break-glass path.
13. **RPG `rawrz` crate naming** vs keeping `movie-rpg` crate names to limit churn.
14. **Redis logical DB split** — whether to actually split cache/sessions/queues/streams
    now or keep one keyspace with prefixes.

---

## 19. Superseded decisions (explicit register)

| Source | Superseded position | Replaced by |
|---|---|---|
| `FIX.md` §8 | "No Redis / external caching layer for V1; revisit on V2/multi-session or measured latency" | §4 — Redis is now core, and the revisit trigger (sessions, scale) has been explicitly invoked |
| `FIX.md` F-11 / F-39 | In-memory sessions, no sliding renewal, `Secure` deferred, restart clears sessions | §7 — Redis sessions with sliding renewal; `Secure` enforceable behind nginx TLS |
| `movie-rpg-spec.md` §1, §10.1, §10.4 | "Not part of the stack, no new container, separate lifecycle, host process" | §10 — the RPG is a container in the merged stack |
| `movie-rpg-spec.md` §11 | Out of scope: new container, reverse proxy/Traefik, webhooks | §4/§6/§9 — all three are now in scope |
| `movie-rpg-spec.md` §7.2 | "New crate alongside the stack-management crate in `~/Cave/backend/`" | §3.3 — literal `backend/rpg` + `backend/deck` in one monorepo |
| `movie-rpg-spec.md` §10.3 / env | `RPG_*` env naming, `RPG_DB_URL` | §12.3 — `RAWRZ_*` naming |
| `movie-rpg-spec.md` §2.1 / theme | Lantern Academy presentation as settled | §12.2 — rethemed under RAWRZ, same original-IP rules |
| `~/Cave/AGENTS.md` | "8 always-on services" cap; slim-down as the standing posture; no new containers | §1.3/§11.3 — Redis, Postgres, nginx, observability, and both apps join the stack as a deliberate reversal |
| `~/Cave/AGENTS.md` | Worktree paths under `/home/bear/cave/`; three-repo separation; submodule pinning | §14, §3.4 — one repo, `backend/` in-tree |
| `~/Cave/AGENTS.md` linkage note | "The RPG is linked with but not a part of the stack" | §1.2 — RAWRZ is one repository |
| `cave-deck-spec.md` §3.1 | Separate repo + submodule + own releases | §3.4 — in-tree, monorepo releases |
| `cave-deck-spec.md` §3.4 | LAN-only, no login | §7 — one auth (PIN gate) across RAWRZ UIs |
| `cave-deck-spec.md` catalog `redis` entry | Redis as an opt-in catalog cache/queue for catalog services | §4 — Redis becomes a first-class core service (the entry's shape — no host port, `./config/redis:/data` — is retained) |
| `movie-rpg-spec.md` §4.2/§5.5 rules | Server-side watch authority | **Not superseded** — explicitly preserved (§9.2 rule 1) |

---

## Appendix A — Interview provenance

Seven rounds of multiple-choice questions, all answered by the user.

**Round 1 — merge shape.** RAWRZ brand-new monorepo (not "TheBearCave absorbs", not
three-repos-with-submodules); history preserved **as subtrees**; layout
`backend/rpg` + `backend/deck`; **one** release stream; a **new unified ruleset**.

**Round 2 — Redis.** Compose container (not host daemon, not dual, not catalog-install);
roles = Seerr cache, RPG provider cache + sessions, Deck queues + pub/sub, catalog
service cache, stack API cache (**all five**); Seerr = **patch it** (after being told
upstream has no Redis support); durability = ephemeral; failure = fail open, never
source of truth.

**Round 3 — seams and rules.** "Seamless" = **all four** seams (one auth, one front door,
one data store, one control plane); ambition = **maximal, complexity is the point**;
preserved rules = LAN-only, no reverse-proxy tier, media pipeline survives app outages
(everything else relaxed); rollout = **phased, cut over when green**; naming = **full
rebrand**.

**Round 4 — reach and reversal.** Into *arr: **full ownership + config writes** (DB
migration not yet chosen); event sources = *arr Connect + Seerr + Plex webhooks; Redis =
one instance, no cap promise; proxy = **reverse the no-reverse-proxy rule, front
everything**; Seerr fork = maintained, CI-built image; rebrand depth = **everything**
including env vars, DB names, secrets, ports, and the academy theme.

**Round 5 — ingress and data.** Proxy = **nginx**; routing = **subdomains + internal
TLS**; media DBs = **migrate to Postgres too**; features = **all seven** (observability,
cache dashboards, event bus + realtime, cross-app, compose-is-truth, cache-everything,
flags); done = **cut-over + features proven**; Postgres = **one instance, separate DBs**.

**Round 6 — shape of the stack.** **Everything containerized** (RPG, Postgres, nginx,
observability); docs = master spec + sub-specs; env = one root `.env` + `secrets/`;
nginx = **cache all services**; Seerr fork = **automated upstream sync PR**; hosting =
personal account, public; spec filename = `rawrz-megastack-spec.md`.

**Round 7 — edges.** Blast radius = **accept shared fate**; cache guardrails =
**aggressive, LAN trust**; Redis truth = **durable exception for queues** (reconciling
round 2's "ephemeral" with queues being non-cache); DB migration = **maintenance window +
verified backups**; verification = **CI e2e on ephemeral compose**; first milestone =
**RAWRZ skeleton + CI + docs**.

**Round 8 — resolution.** R-1: **drop the availability rule** (not the two-tier split,
not reversing the *arr DB migration, not keeping a rule that could not hold) — shared
fate stands and Postgres is hardened instead. R-4: **async `CacheBackend` patch** (not the
sync L1/L2 mirror, not dropping the fork, not a partial fork), because source review
showed every cache call site is already async-capable. Postgres resilience: **single
hardened instance plus backups** — no PITR, no replica, no automatic failover.

## Appendix B — Verified source facts used in this spec

| Fact | Evidence |
|---|---|
| Seerr v3.4.1 has no Redis support | `grep -ri redis ~/TRUTH/seerr` → no matches |
| Seerr's cache is `node-cache` with 10 named caches and the TTLs tabulated in §5.1 | `~/TRUTH/seerr/server/lib/cache.ts` (91 lines, read in full) |
| The Seerr patch surface is bounded | 14 consumer call sites (10 `getCache(` + 4 direct `.data` accesses), 8 cache touchpoints across 4 `ExternalAPI` methods, **11 files** total (`rawrz-seerr-cache-patch-plan.md` §2) |
| Radarr/Sonarr/Prowlarr support Postgres in the pinned versions | `ConnectionStringFactory.cs` (`PostgreSqlVars`, `PostgreSqlConnectionString`) in each of `~/TRUTH/{radarr,sonarr,prowlarr}`; `DatabaseType.PostgreSQL` in `BasicRepository.cs`; `PostgresHost/Port/User/Password/MainDb/LogDb` in `ConfigFileProvider.cs` |
| The stack runs 8 always-on services with ≈12.1 GiB of caps on a 22 GiB host | `~/Cave/AGENTS.md` memory-cap table; `~/Cave/README.md` |
| A `redis` catalog entry is already drafted with no host port and `./config/redis:/data` | `~/Cave/.worktrees/cave-deck/catalog/catalog.yaml` (lines ~1064–1078) |
| Seerr is run as `ghcr.io/seerr-team/seerr:v3.4.1` | `~/Cave/docker-compose.yml` `seerr:` block |
| Cave Deck is a separate repo pinned as a submodule, has its own CI/releases, port 7780, LAN-only/no-login | `~/Cave/cave-deck-spec.md` §3.1, §3.3, §3.4 |
| The RPG currently lives outside the stack with its own spec, FIX.md, HANDOFF.md, and 14 migrations | `/home/bear/movie-rpg` tree; `backend/rpg/migrations/` |
| The prior no-Redis decision and its revisit trigger | `/home/bear/movie-rpg/FIX.md` §8 |
| A stale `movie-rpg-spec.md` copy exists in `~/Cave` (160 KB) and `~` (25 KB) vs the 176 KB canonical | `ls -la` on all three; §3.2 dedupe rule |
| The incomplete stack-management Rust backend exists only as sources (no `Cargo.toml`/`main.rs`) | `find /home/bear/Cave/backend -type f` → 5 `.rs` files, `routes.rs` with `todo!()` handlers |
| `ExternalAPI.get()` is async yet calls the cache synchronously today | `~/TRUTH/seerr/server/api/externalapi.ts:53,56,65,73` (`private cache?: NodeCache`, `protected async get<T>()`, `this.cache?.get<T>(cacheKey)`, `this.cache.set(cacheKey, …, ttl ?? DEFAULT_TTL)`) |
| Every remaining cache site sits in an async context | `~/TRUTH/seerr/server/api/plextv.ts:273` (`public async getWatchlist`), `~/TRUTH/seerr/server/lib/scanners/plex/index.ts:381` (`private async getMediaIds`), `~/TRUTH/seerr/server/routes/settings/index.ts:784` (async route handler) |
| No media service currently depends on an app-tier service | `~/Cave/docker-compose.yml` `depends_on` blocks reference only `nzbdav`, `nzbdav_rclone`, and `prowlarr` |
