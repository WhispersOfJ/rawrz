# FIX — Full-codebase review findings (2026-09-10)

> **REMEDIATION STATUS (2026-09-10): every actionable finding below has been
> implemented and verified** — `cargo test`: 89 unit + 1 live scratch-Postgres
> proof all pass; `cargo clippy --all-targets`: zero warnings.
>
> Per-item status (✅ = fixed in code, 📝 = documented/by-design):
>
> - **F-1** ✅ proof refuses to reset unless URL contains `rpg_scratch` or
>   `RPG_ALLOW_DB_RESET=1`; **F-2** ✅ episodes sync + award (show/season
>   parent linkage, `watchable_items`, stack-based XML parser); **F-3** ✅
>   `POST /api/genres/{name}/access`; **F-4** ✅ stack-based parser (items
>   captured at any nesting depth); **F-5** ✅ TVDB 401 → re-login + retry
>   once; **F-6** ✅ ledger: neutral `normal_xp`, adjusted `xp_awarded`,
>   milestone recorded in `bonuses`; **F-7** ✅ migration 0013
>   `UNIQUE (character_id, content_id)`, violation = already-awarded;
>   **F-8** ✅ random-key fallback on clock failure; **F-9** ✅ counts fixed.
> - **F-10** ✅ failed-login throttle (5 attempts → 30s×2^n lockout, in-memory)
>   + set-pin returns 409 when an account exists and never hashes for an
>   existing account; **F-11** ✅ sweep runs on every issue; **F-12** ✅
>   `RPG_BIND_ADDRESS` (default unchanged); **F-13** 📝 unchanged (noted);
>   **F-14** ✅ 4 MiB body caps on every provider/stack response; **F-15** 📝
>   unchanged (posture already correct).
> - **F-16** ✅ fresh-only hydration + retention prune (7-day grace) in every
>   sync; **F-17** ✅ deadpool-postgres pool with health-checked recycles;
>   **F-18** ✅ one shared `reqwest::Client` (timeouts + caps in one place).
> - **F-19** ✅ the shared store mutex is gone: server and poll loop share an
>   `Arc<PostgresContentStore>` over the pool; **F-20** ✅ Argon2
>   hash/verify on `spawn_blocking`, before any store work; **F-21** ✅
>   migrations run once at startup, hydration/retention per cycle; **F-22** ✅
>   batched content + cache upserts (two statements per sync, parent-linking
>   pass); **F-23** ✅ single refresh in select flow, dead bindings removed.
> - **F-24** ✅ cycle counters include failed cycles in the exit line
>   (logging stays `println!` — subscriber wiring deferred, see HANDOFF);
>   **F-25** 📝 `poll_interval_seconds` documented as reserved — the seeded
>   default equals `POLL_INTERVAL`; **F-26** ✅ exe-relative env-file fallback;
>   **F-27** ✅ inline `#` comments stripped from unquoted values; **F-28** ✅
>   `ProbeError::OutOfRange` variant.
> - **F-29** ✅ set-pin issues a session on account creation; **F-30** ✅
>   completed orders stay on the board (`status: "active" | "completed"`);
>   **F-31** ✅ skip flow runs order generation; **F-32** ✅ locked hidden
>   badges serialize slug/category only; **F-33** ✅ skip response embeds the
>   refreshed order board; **F-34** ✅ `/api/orders/refresh` returns a trimmed
>   `TickSummary`; **F-35** ✅ `skipped_genres: [{genre, reason}]` in the
>   tick/refresh payload.
> - **F-36** ✅ `url`, `serialize`, `tracing` features removed from
>   Cargo.toml (`tracing` deps dropped until wired); **F-37** ✅ root
>   `.gitignore` no longer ignores `/Cargo.lock`; **F-38** ✅ explicit
>   rollback helper on early returns; **F-39** ✅ sliding session renewal on
>   authenticated requests.

---

> Output of a fine-tooth-comb review of the entire repo (backend, migrations,
> tests, CI, scripts, docs) at the current uncommitted state (Lantern Academy
> slice, 12 migrations, 86 unit tests + 1 live proof, all passing; 3 known
> clippy warnings). **No code was changed in this pass** — every item below is
> a recommendation to be triaged and implemented in a later pass.
>
> Verified during review: `cargo test --all-targets` → 86 + 1 passing;
> `cargo clippy --all-targets` → only the 3 documented warnings.

Severity legend: **H** = fix soon (data loss / security / core gameplay),
**M** = fix in the next pass, **L** = hygiene / polish.

---

## 1. Bugs (correctness)

### F-1 · H — `cargo test` can wipe a database pointed at by `RPG_DB_URL`
`tests/store_proof.rs` → `reset_database()` unconditionally runs
`DROP SCHEMA public CASCADE; CREATE SCHEMA public;` whenever `RPG_DB_URL` is
set. CI points it at a scratch service container, and `HANDOFF.md` explicitly
invites setting `RPG_DB_URL` locally to activate the proof inside plain
`cargo test`. If someone sets it to their real RPG database (or any important
DB), a routine `cargo test` **destroys the schema and all data**.
*Fix:* refuse to run unless the URL clearly names a scratch database (e.g.
require `rpg_scratch` in the DB name or a dedicated `RPG_ALLOW_DB_RESET=1`
opt-in), and/or have the harness create+drop its own temporary database
instead of resetting whatever it was given.

### F-2 · H — TV episode watches are (very likely) never awarded
`game.rs::detect_plex_watch_states` walks `plex.sections()` and fetches
`/library/sections/{key}/all` for every section. For a TV section, Plex
returns show-level `Directory` rows (`type="show"`), not episode rows, and
`awards.rs::detect_watches` skips anything whose DB `content_type` is not
`movie`/`episode` — series rows are filtered out. Net effect: §5.1's
episode = 10 XP path and all episode counters/achievements probably never
fire against a real Plex library. The fixtures/tests only exercise movie
sections, so this is untested.
*Fix:* enumerate episodes (section scan → leaves, or a `?includeElements=...`
style request), key them by ratingKey in `content`, and add a live-section
fixture. Verify against a real show section before the next release.

### F-3 · H — `access_genre` has no API surface: progression past Horror is unreachable
`wizard_store.rs::access_genre` (the §5.2 cascade transition) is only called
from the live proof. `server.rs` exposes no route for it. Consequences via
pure gameplay math: `genre_access` stays at Horror forever → `ORDER_GENRES_SQL`
only ever generates Horror orders → `moonlit_mediator` (needs
`genres_accessed ≥ 3`) and every `genre_coverage` achievement are
unreachable through HTTP.
*Fix:* add a gated `POST /api/genres/{name}/access` (mirroring the archetype
selection endpoint's conflict/422 error mapping) before the guided UI is
built against the current API.

### F-4 · M — Plex XML parser closes items early on nested same-name tags
`stack.rs::parse_plex_library_items` treats the first `End` event whose name
matches the current item's tag as the item's end, and ignores nested
`Start`s of the same name. A show item containing nested `<Directory>`
(season) children closes the show at the first `</Directory>` of a season,
and the remaining children leak into malformed/empty items or are lost
(genres, watch-state attributes).
*Fix:* track tag depth per open item (or use quick-xml's element depth
API) so only the matching close ends the item.

### F-5 · M — TVDB token is cached forever and never refreshed on expiry
`providers.rs::TvdbClient::ensure_login` returns `Ok` whenever a token
exists; TVDB v4 tokens expire (roughly monthly). A long-running process
fails all TVDB enrichment with 401 forever and never re-logins.
*Fix:* on a 401 from `series()`, clear the cached token and retry once; or
store the token's issue time and re-login after its TTL.

### F-6 · M — `watches` ledger conflates neutral and archetype-adjusted XP; milestone bonuses never recorded
`WATCH_INSERT_SQL` writes the archetype-adjusted value into **both**
`xp_awarded` and `normal_xp`. Per the wizard contract `normal_xp` should be
the neutral 20/10 (the adjustment is an active-loadout effect), and §5.1.1
streak milestone bonuses are added to `character_state.xp` but never written
to `watches.bonuses` — so per-watch audit rows under-report what the
character actually earned. Historical rows are immutable by design, so this
drift compounds silently going forward.
*Fix:* insert `normal_xp` = neutral value, keep the adjusted value in
`xp_awarded`, and append `{"kind":"streak_milestone","amount":N}` to
`bonuses` when a milestone pays. Land before the next real watch data
accumulates.

### F-7 · M — Award-once is guarded by advisory lock + `NOT EXISTS`, not by a constraint
`award_plex_watches` serializes via `pg_advisory_xact_lock` and an
`INSERT ... WHERE NOT EXISTS` guard. Correct today (single process, single
mutex), but the ledger's core invariant has no database-level enforcement —
a second writer (future worker, manual SQL, second instance) can double-award.
*Fix:* add `UNIQUE (character_id, content_id)` on `watches` in migration
0013 (or an equivalent partial unique index) and treat insert-conflict as
"already awarded". Also speeds up the per-award existence check.

### F-8 · L — `event_key` collision edge case
`wizard_store.rs::select_archetype` builds
`archetype-select:{slug}:{timestamp_nanos}` with
`unwrap_or_default()` — on clock failure the key becomes `...:0`, and the
`ON CONFLICT DO NOTHING` on `character_archetype_events` would silently drop
the audit event. Use a random/UUID key or propagate the error.

### F-9 · L — Stale count in the proof's header comment
`tests/store_proof.rs` says "(1) migrate() over all six migrations" while the
catalog has twelve and the adjacent assertions check 12. Doc-only fix.

---

## 2. Security

### F-10 · H — No brute-force protection or lockout on the PIN gate
`/auth/login` accepts unlimited attempts. A 4-digit PIN is 10,000
combinations; Argon2id throttles to roughly 10–20 attempts/sec/core, so an
on-LAN attacker recovers a 4-digit PIN in tens of minutes. There is no
attempt counter, backoff, or lockout (V1 has a single account, so a simple
in-memory failed-attempt counter with exponential backoff is sufficient).
`/auth/set-pin` is likewise unauthenticated: when an account exists it
doesn't overwrite the PIN (good), but it still validates + **hashes** the
supplied PIN and re-runs bootstrap inside the store lock — an unauthenticated
CPU/latency DoS handle.
*Fix:* rate-limit failed logins (in-memory counter + increasing delay),
return `409` from `/auth/set-pin` when the account already exists, and
check account existence before paying the Argon2 cost.

### F-11 · M — Session store is never swept; expired sessions live forever
`server.rs::SessionStore::sweep_expired` exists but is **never called**
(verified by search). Entries accumulate for the process lifetime. Sessions
are only issued on successful login, so growth is slow in practice, but it
is an unbounded leak tied to a security-relevant table, and the dead method
implies a maintenance path that doesn't actually run.
*Fix:* call `sweep_expired()` opportunistically (e.g. on login/logout, or
from the poll loop once per cycle).

### F-12 · M — Server binds `0.0.0.0` unconditionally, not configurable
`main.rs::BIND_ADDRESS` is hardcoded. The spec is LAN-only; binding all
interfaces also exposes the service on any VPN/WireGuard/docker0 interface.
*Fix:* read an `RPG_BIND_ADDRESS` env var (defaulting to the current value),
and consider defaulting to the LAN-reachable interface only.

### F-13 · L — Timing/robustness details on sessions and cookies
- Session lookup is a plain `HashMap` get (non-constant-time). With 128-bit
  random tokens the practical risk is nil; note only.
- The cookie carries no `Secure` flag — correct while the service is plain
  HTTP; add `Secure` automatically when a TLS deployment exists.
- `GET /auth/status` maps DB errors to `locked: true`
  (`unwrap_or(false)`), which can momentarily show the first-run/set-PIN
  screen during a transient DB failure. Prefer surfacing an error state.

### F-14 · L — Unbounded response bodies from stack providers
`send_with_retry` + `response.text()/json()` read provider responses without
a size cap. These are trusted LAN services, but a misbehaving service (or a
redirect to somewhere large) can OOM the process. Cap body size in the
client.

### F-15 · L — CSRF posture is acceptable but implicit
State-changing routes are POST + JSON (`application/json` is not a
simple CORS content-type) and the cookie is `SameSite=Lax`, which blocks the
classic cross-site form POST. Fine for V1; revisit if HTML forms or GET
mutations are ever added.

---

## 3. Leaks / unbounded growth

### F-16 · M — `content_provider_cache` grows without retention and is fully hydrated every cycle
Nothing ever deletes expired rows from `content_provider_cache`, and
`PROVIDER_CACHE_HYDRATE_SQL` (run at the top of **every** poll cycle via
`SyncPipeline::run`) selects *all* rows with full payloads, `ORDER BY id`,
no freshness filter. Over months this is an unbounded table and an
ever-growing per-cycle memory/IO cost.
*Fix:* add retention (delete rows where `expires_at < now() - grace`) and
hydrate only `expires_at > now()` (plus the entries needed for stale-fallback
of currently-syncing items).

### F-17 · M — Postgres connection is never re-established
`PostgresContentStore::connect` spawns `connection.await` and discards the
result. If Postgres restarts (or the connection drops), every store call
fails forever until the process is manually restarted; the poll loop will
log failures every 5 minutes but never recover.
*Fix:* wrap reconnect logic around the store (e.g. a lazily-reconnecting
handle or a supervisor task that recreates the client on connection loss),
or at minimum log a loud "store connection lost, restart required" marker.

### F-18 · L — One HTTP client per stack service
Each client builds its own `reqwest::Client` (own connection pool). Sharing
one configured client (timeouts, connect limits, size caps) is cheaper and
gives one place to enforce F-14.

---

## 4. Performance & optimizations

### F-19 · H — The shared store mutex serializes the whole poll cycle against the UI
`main.rs` shares one `Arc<Mutex<PostgresContentStore>>` between Axum and the
poll loop, and the mutex is held for the **entire** cycle: full content sync
(network calls to Plex/Sonarr/Radarr + 4 metadata providers, with retries)
plus the game tick. Any slow provider stalls every HTTP request — including
`/healthz`-adjacent auth flows — for the duration. The 5-minute UI "refresh"
endpoint also runs a full tick under the same lock.
*Fix (architectural):* move to a small connection pool (e.g. `deadpool-
postgres`) so HTTP handlers and the poll loop don't contend on one client;
keep the store mutex only for genuinely order-sensitive sections (the tick),
or drop it in favor of per-transaction advisory locks (the award path
already uses one).

### F-20 · M — Argon2 runs on the async runtime while holding the store lock
`hash_pin`/`verify_pin` (~50–100 ms of CPU) execute inside async handlers on
the runtime, blocking a worker thread, and `set_account_pin` holds the store
mutex through it.
*Fix:* wrap hashing in `tokio::task::spawn_blocking` (and combine with the
F-10/F-19 changes).

### F-21 · M — Migration + hydration run every poll cycle
`SyncPipeline::run` calls `store.migrate()` and full `hydrate_cache` on each
of the 288 daily cycles. Migrations are cheap when no-op but pointless here;
hydration is covered by F-16.
*Fix:* run `migrate()` once at startup (it already runs there) and remove it
from the per-cycle path, or gate it behind a debug flag.

### F-22 · M — Content persistence is row-by-row
`PostgresContentStore::persist` issues one parameterized upsert per content
row and per cache row inside one transaction. Initial syncs of real
libraries (thousands of rows) mean thousands of round-trips.
*Fix:* batch via multi-row `INSERT ... SELECT FROM unnest(...)` (the
pattern `SETTINGS_SEED_SQL`/`ORDER_ITEMS_INSERT_SQL` already use) or
`COPY` into a temp table + upsert.

### F-23 · L — Small redundant-work items
- `select_archetype` calls `refresh_archetype_unlocks()` and then
  `archetype_state()`, which refreshes again — one refresh is enough.
- `ORDER_GENRES_SQL` + per-genre candidate queries + inserts run N+1 per
  tick; fine at V1 scale, but batchable later.
- `award_plex_watches` re-reads `CHARACTER_AWARD_STATE_SQL` per award; with
  many same-tick awards a running-local computation would do (current
  per-award round-trip is at least strictly correct).
- `let _ = watch_row;` in `award_plex_watches` — bind with `.map(|_| ())` or
  ignore via the statement shape.

---

## 5. Reliability / operational

### F-24 · M — Poll-cycle failure counting and logging
`run_poll_loop` increments `completed` only when `summary.tick.is_ok()`; a
cycle with a successful sync but failed tick reports as incomplete even
though work landed. Also all logging is `println!/eprintln!` — the Cargo
`tracing`/`tower-log` features are enabled but no subscriber is initialized,
so there is no levelled/structured log to grep in production.
*Fix:* count "cycles run" and "cycles with failures" separately, and wire a
real `tracing` subscriber (respecting `RUST_LOG`).

### F-25 · M — `POLL_INTERVAL` ignores the `poll_interval_seconds` setting
`settings` seeds `poll_interval_seconds = 300` but `poll.rs` uses the
constant. Either document the setting as reserved-for-V2 or read it at
startup.

### F-26 · L — Env file path is CWD-dependent
`main.rs` defaults to `../.env` relative to the process CWD; running the
binary from anywhere other than `backend/rpg` fails startup. Try the exe-
relative path or accept the env-var override in docs.

### F-27 · L — `parse_env_line` keeps inline comments
`KEY=value # comment` yields `value # comment` as the value (quotes are
trimmed only at the ends). Minor, but surprising when someone annotates the
shared `.env`.

### F-28 · L — Misused error variant
`ProbeError::Xml` is raised for "streak/level out of int4 range" in
`award_plex_watches` — nothing to do with XML. Add a dedicated
`InvalidState`/`OutOfRange` variant when touching that code.

---

## 6. UI/UX (API surface — the Svelte frontend is pending, so these are contract-level)

### F-29 · M — Set-PIN doesn't establish a session
After first-run `POST /auth/set-pin` succeeds, the client must immediately
`POST /auth/login` with the same PIN. Returning a session cookie from
set-pin (when it created the account) removes a dead end in onboarding.

### F-30 · M — Completed orders vanish from `GET /api/orders`
`ORDER_VIEW_SQL` filters `wo.status = 'active'`, so the moment a cycle
completes, the player's proof of progress disappears until the next cycle
generates (up to 5 minutes later, see F-31). Include recently completed
orders (or a completion summary field) so the order board reads as history,
not as data loss.

### F-31 · M — Skip completion doesn't generate the next cycle
`skip_order_item` advances reveals/completions/awards but not
`refresh_watch_orders` order generation, so after finishing the last item of
cycle 1 via skip, the next mystery cycle waits for the next tick/refresh.
Run order generation in the skip flow (it already runs the later phases) or
return `next_cycle_generated: bool` so the UI can explain the gap.

### F-32 · L — Hidden achievements leak names/descriptions
`BADGE_WALL_SQL` serializes **all** 101 rows including `visible = false`
with name/description. Single-player, but if hidden badges are meant to be
discovered, lock their display fields (slug + category only) until
unlocked.

### F-33 · L — Skip outcomes could return the refreshed order view
`POST /api/orders/{id}/skip` returns `{skipped, position}` only; the UI must
immediately re-`GET /api/orders`. Embedding the updated order (or a
`state_version`) saves a round trip and avoids flash-of-stale-state.

### F-34 · L — `/api/orders/refresh` exposes raw internal tick report
`GameTickReport` (phase counters, internal order ids) is returned verbatim.
Fine for development; trim or wrap for the guided UI.

### F-35 · L — Genre generation failure is silent
When a genre has fewer than 5 unwatched/unranked candidate movies,
`refresh_watch_orders` silently skips it. Surface a
`skipped_genres: [{genre, reason}]` in the tick report so the UI can explain
"no new mystery available".

---

## 7. Hygiene & docs

### F-36 · L — Dead dependency and unused features
- `url = "2"` in `Cargo.toml` — no `use url` anywhere.
- `quick-xml` `serialize` feature — parsing is manual.
- Axum `tracing`/`tower-log` features — see F-24.

### F-37 · L — `.gitignore` / lockfile contradiction
Root `.gitignore` ignores `/Cargo.lock`, yet `backend/rpg/Cargo.lock` is
tracked (correct for an application). Remove the ignore entry to avoid
confusion.

### F-38 · L — Drop-rollback relies on implicit behavior
Several early-return paths inside open transactions (e.g. `skip_order_item`
finale/no-skip returns) rely on `Transaction` drop = rollback. Correct, but
explicit `rollback().await?` (as `bootstrap_single_character` does) reads
better in audit-sensitive code.

### F-39 · L — Session TTL is fixed-at-issue, cookie and server agree
No sliding renewal; a 7-day session hard-expires mid-play. Acceptable for
V1; note that renewing on activity would also be the natural place to call
`sweep_expired` (F-11).

---

## Priority shortlist (suggested order for the next passes)

| # | Finding | Severity | Theme |
|---|---------|----------|-------|
| 1 | F-1 proof can wipe `RPG_DB_URL` target | H | data loss |
| 2 | F-2 episode watches never detected | H | gameplay |
| 3 | F-3 `access_genre` unreachable via API | H | gameplay |
| 4 | F-10 PIN brute-force + set-pin DoS | H | security |
| 5 | F-19 store mutex stalls UI during poll | H | perf/UX |
| 6 | F-6 / F-7 watches ledger semantics + unique award | M | data model |
| 7 | F-16 provider cache retention + hydration | M | leak |
| 8 | F-17 Postgres reconnect | M | reliability |
| 9 | F-5 TVDB token refresh · F-4 XML nesting | M | bugs |
| 10 | F-11 session sweep · F-12 bind config · F-20 spawn_blocking | M | security/perf |

Everything else was batched into the 2026-09-10 remediation pass. Note: the
spells/affinity migration originally suggested here as 0013 is now **0014** —
0013 is the `watches_character_content_unique` constraint (F-7).

---

## 8. Architectural decision — Redis / external caching layer (evaluated 2026-09-10)

**Verdict: no Redis (or any external cache daemon) for V1.** Decision recorded
here so it isn't re-litigated silently.

1. **Catalog reality:** the Gravity Index integration catalog only offers
   *managed cloud* Redis (Redis Cloud, ElastiCache, Memorystore). Sending Plex
   metadata payloads and PIN-gate session tokens to a cloud vendor violates the
   project's privacy-first home-LAN design and adds a WAN dependency the spec
   explicitly avoids.
2. **Project constraints:** CLAUDE.md forbids new compose containers and the
   stack-linkage contract says the RPG owns Postgres only. Redis would be a new
   host daemon or a forbidden container.
3. **Scale doesn't justify it:** single-account, LAN-only, 5-minute poll
   cadence. The three real cache surfaces are all in-process problems:
   - `content_provider_cache` retention/hydration → F-16 (SQL retention +
     fresh-only hydration), not a cache-engine problem;
   - in-memory sessions → F-11 (call the existing `sweep_expired`);
   - per-cycle `MetadataCache` → microseconds of work.

   Redis adds a network hop and ops burden while fixing none of them.

**Revisit trigger:** V2 shared quests / multi-session concurrency, or a
measured latency problem the in-process fixes cannot solve. If revisited,
prefer an embedded/in-process cache first (e.g. `moka`) over a daemon.
