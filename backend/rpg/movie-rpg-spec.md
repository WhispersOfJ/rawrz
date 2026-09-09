# Movie / TV RPG — Specification

> **Status:** Draft v1.2 — gathered from user interview; reviewed 2026-09-10. The progression engine is implemented; the original wizard-academy rebrand is now a finalized presentation and rules contract, with the backend-first vertical release defined and implementation intentionally deferred to a later migration/API task. Open questions in §12 are either resolved or explicitly deferred.
> **Related:** The Bear Cave stack at `~/Cave` (8-service Usenet media stack: Prowlarr, Radarr, Sonarr, NzbDAV, nzbdav_rclone, Seerr, Plex, Unpackerr). This RPG is *linked with* the stack but *not a part of it*.

---

## 1. Overview

A web-based RPG where watching movies and TV shows is the core mechanic. Points are earned by watching content served from the Bear Cave stack (Plex + Sonarr + Radarr). The RPG is presented as an **original wizard academy**: watching is study and spellcraft, and progress unfolds through disciplines, assignments, enchanted watch orders, archetypes, spells, and awards. The underlying progression engine remains theme-neutral; the wizard vocabulary is a presentation and rules layer over the existing watches, XP, streak, genre, achievement, and order ledgers.

**Relationship to the stack (explicit):**

- **Linked with** the stack: reads Plex, Sonarr, and Radarr APIs over the LAN. Consumes the same `.env` secrets / API keys the stack uses.
- **Not a part of** the stack: no new container in `docker-compose.yml`. Separate deployment, separate lifecycle. Does not join the `bearcave` network as a Compose service.
- **Deployment model (decided):** Extend the existing Rust backend in `~/Cave/backend/`, adding a new crate alongside the existing (incomplete) stack-management crate. Same directory, loose coupling inside `backend/`. The backend serves the RPG web UI and logic.
- **Database (decided):** PostgreSQL, introduced as a *shared metadata store* that the RPG uses but that can also serve other stack-adjacent tooling over time. Not a new container in compose, but a host-side or otherwise-available Postgres instance. Existing *arr services keep their SQLite.

---

## 2. Theme & Framing

- **Genre:** Light wizard-academy adventure and personal collection game.
- **Setting:** **The Lantern Academy**, an original school of storycraft where students study the many disciplines hidden inside movies and television. The academy is a presentation frame, not a claim that the media itself is magical.
- **Metaphor:** Watching = attending a lesson and practicing spellcraft. Plex is the viewing library; the RPG is the academy's private study ledger.
- **Tone:** Warm, playful, curious, and slightly mysterious — parchment and constellation-board charm without grimdark stakes or imitation of a film franchise.
- **Positions watching as:**
  - **Movies** = completed assignments or one-sitting practicals.
  - **TV shows** = term-long courses and multi-episode studies.
  - **Mystery watch orders** = enchanted reading lists whose next title is sealed until the prior assignment is resolved.
  - **Achievements** = certificates, honors, and displayed academy awards.
  - **Archetypes** = selectable study paths that change how future progress behaves.
  - **Spells** = limited, auditable abilities that bend one rule without bypassing the watch ledger.
- **Newly arrived content** is new coursework from the stack, but the player still chooses what to study.

### 2.1 Rebrand boundary and vocabulary map (finalized 2026-09-09)

The product-facing language changes, but stable database identifiers and progression semantics do not. Internal code may retain neutral names such as `character`, `watches`, `watch_orders`, `achievements`, and `genre_access`; those names are contracts, not visible theme copy.

| Neutral engine term | Lantern Academy presentation | Rule boundary |
|---|---|---|
| Character / player | Student, wizard, or academy member | One persisted V1 progression identity remains. |
| Case / case board | Assignment board or lesson board | Existing case semantics remain; no automatic assignment. |
| Evidence / watch log | Viewing journal or study log | Still shows only awarded `watches` rows. |
| Genre | Magical discipline | Existing genre IDs and access rules remain stable. |
| Mystery watch order | Enchanted watch list | Existing reveal, finale, and skip rules remain authoritative. |
| Skip grant | Academy free pass | Still a `skip_grants` ledger row; it is not XP and is not a spell charge. |
| Achievement / badge wall | Honor, certificate, or award shelf | Existing unlock idempotency and hidden/visible rules remain. |
| XP / level | Mastery / academy level | XP values and level thresholds remain the neutral engine contract. |
| Poll / game tick | Academy bell / study cycle | Existing sync → tick ordering remains unchanged. |

The words **house**, **wand**, **patronus**, **quidditch**, and other recognizable franchise-specific constructs are not part of this setting. The academy has disciplines, study paths, spell charges, and original awards instead.

The current backend may still expose legacy neutral/default copy such as `The Investigator` or seeded achievement descriptions. That is a compatibility fact, not the new product vocabulary: this documentation pass does not silently mutate existing rows or APIs. The later presentation migration/API work must provide academy labels alongside stable slugs, and may offer an explicit copy migration only after the new UI and acceptance checks are ready.

**Release shape (finalized 2026-09-10):** implementation is backend-first but vertical. The first wizard release is not a schema-only foundation: it completes migrations 0012/0013, server-owned archetype and spell flows, tick integration, and live proofs for all six archetypes and five spells. The Svelte UI follows against those stable APIs, beginning with a guided dossier rather than requiring the backend to expose unfinished or client-computed rules.

### 2.2 Original IP and asset policy (finalized 2026-09-09)

This product must not use copyrighted Harry Potter names, characters, house names, spell names, logos, typography, screenshots, promotional stills, actor likenesses, or film artwork. It must not ask an image model or artist to imitate the film artwork or a living artist's distinctive style. The reference is only the broad category of whimsical wizard-school illustration; all names, symbols, portraits, UI motifs, and assets are original or properly licensed.

The visual brief is: inked marginalia, warm parchment, brass and midnight-blue interface chrome, lanterns, constellations, botanical diagrams, geometric sigils, stained-glass color accents, and expressive original student portraits. Avoid lightning scars, school crests that resemble known franchises, copied costume silhouettes, recognizable props, or any derivative film composition. Initial art may use generated SVG geometry, abstract spell diagrams, and commissioned/licensed original portraits; placeholders must be replaceable through stable `portrait_key`/asset IDs rather than hard-coded image URLs.

### 2.3 Core loop in the academy frame

1. Content arrives as new coursework or is discovered in the library.
2. The student chooses an assignment, course, or enchanted watch list.
3. The student watches normally in Plex.
4. The poll detects the completed watch and writes the neutral `watches` ledger.
5. Neutral XP, streaks, bonuses, order reveals, and achievements advance in the existing game tick.
6. The active archetype and any valid spell effect modify only the explicitly permitted future outcome.
7. The UI presents the result as mastery, a revealed lesson, a spell charge, or an academy award.

The academy layer never creates a completion, watch, XP award, achievement unlock, or order resolution directly from client claims.

---

## 3. Core Loop

The neutral engine remains the contract underneath the academy presentation. The UI may say "lesson" and "mastery"; persistence and server-side evaluation continue to use the existing watch, XP, genre, order, and achievement terms.

1. **Something arrives** on the stack (Sonarr/Radarr import, or already-in-library content).
2. **The RPG surfaces it as coursework** on the assignment board (player-driven — the student chooses).
3. **The student watches** the content via Plex (normal Plex playback; nothing special required).
4. **Plex records the watch.** The RPG polls Plex (every few minutes) and picks up the completed lesson.
5. **Mastery/XP is awarded** based on completion (episode, season, series) and any applicable bonuses.
6. **Progression:** XP → academy level → discipline access, archetype eligibility, spell grants, and awards.
7. **Repeat.**

The app does not change how the student watches. Plex remains the viewing surface; the RPG reads behind the scenes and is the authority for progression outcomes.

---

## 4. Data Sources

### 4.1 Plex (primary)

**Why:** Plex has the authoritative playback record — what was watched, when, how much, sessions.

**Endpoints used (Plex Media Server URL API, `X-Plex-Token` auth):**

- `GET /library/sections` — discover library sections (Movies, Shows).
- `GET /library/sections/{key}/all` — library items (movies and shows), with metadata.
- `GET /library/sections/{key}/episodes` — TV episodes (for TV "campaign" tracking).
- `GET /status/sessions` — active playback sessions (informational/optional).
- `GET /library/sections/{key}/recentlyAdded` — recently added items.
- `GET /library/sections/{key}/unwatched` — unwatched items (for case generation).
- `GET /library/metadata/{ratingKey}/progress` or episode progress endpoints — watch Progress / percent viewed.
- Playback state / watched state per item and per episode.

**Auth:** `PLEX_TOKEN` from `.env` (same token the stack scripts use). `PLEX_URL` = `http://HOST_IP:32400`.

**What Plex provides to the RPG:**

- Watch events (item/episode marked as watched, watch progress).
- Library contents (title, year, genres, rating, summary, TMDb/TVDb ids where available).
- Section structure (Movies vs Shows distinction).
- Recently added / unwatched lists (for case generation).

### 4.2 Sonarr (TV)

**Why:** TV-specific metadata and history that Plex may not fully expose — series/episode structure, import history (what was newly added), episode file state, statistics.

**Endpoints used (Sonarr `/api/v3`, `X-Api-Key` auth):**

- `GET /series?includeStatistics=true` — series list with stats.
- `GET /series/{id}` — single series detail.
- `GET /episode?seriesId={id}` — episodes for a series.
- `GET /history?seriesId={id}&eventType=downloadFolderImported&pageSize=1` — recent import (arrival) events.
- `GET /queue` — current queue (optional, informational).
- Series/episode metadata: titles, season/episode numbers, status, genres, TMDb/TVDb IDs.

**Auth:** `SONARR_API_KEY` from `.env`. `SONARR_URL` = `http://HOST_IP:8989`.

### 4.3 Radarr (Movies)

**Why:** Movie metadata and import history.

**Endpoints used (Radarr `/api/v3`, `X-Api-Key` auth):**

- `GET /movie?tmdbId={id}` or list movies — movie metadata.
- `GET /history?movieId={id}&eventType=downloadFolderImported&pageSize=1` — recent import (arrival) events.
- Movie metadata: title, year, genres, rating, TMDb ID, etc.

**Auth:** `RADARR_API_KEY` from `.env`. `RADARR_URL` = `http://HOST_IP:7878`.

### 4.4 Data flow decision

- The RPG **reads** Plex/Sonarr/Radarr via their LAN APIs (polling; see §9).
- The RPG stores what it learns in its **own Postgres schema** (see §6). This is a read-only mirror from the stack's perspective — the RPG never writes back to Plex/Sonarr/Radarr.
- The stack's existing services are **not modified** to connect to Postgres by this spec. The "shared metadata store" framing means the RPG's Postgres *could* be reused by future stack-adjacent tooling, but V1 is RPG-only consumers of it.

### 4.5 Metadata mirror — mirror everything (resolved §12 Q13)

**Mirror from Plex / Sonarr / Radarr on each poll (incremental, keyed by IDs + last-sync markers): mirror everything the APIs expose, not a trimmed subset.** The RPG stores the full metadata it can get from Plex/Sonarr/Radarr — titles, year, TMDb/TVDb IDs, genres (including sub-genres where the source exposes them), ratings (MPAA/RR/score and any external rating reachable), run time, summaries/plots, poster paths/URLs, fanart/extra artwork where exposed, library section/key, release/air dates, status (continuing/ended), episode-level data (season/episode numbers, titles, air dates, runtimes, watched state per episode from Plex, file state from Sonarr), series/movie file metadata and MediaInfo blobs where the *arr APIs return them, credits/extras/indexer metadata where exposed, and any other fields the APIs provide.

- **Sync keys:** use stable external IDs (TMDb for movies, TMDb/TVDb for series/episodes) plus Plex `ratingKey` where needed for watch-state. Incremental sync is keyed on these IDs + last-sync timestamps so a poll only fetches what changed.
- **Watch state (authoritative source for completion):** Plex is the ground truth for watched/unwatched and watch progress. Sonarr/Radarr contribute import/arrival events and episode file state, not watch state. (This matches §5.1's completion-based model: the RPG awards points when Plex reports a near-complete watch.)
- **Ratings source hierarchy (for featured-case ranking — see §5.4):** rank by the normalized provider score in this order: TMDb vote average, OMDb IMDb rating, TVDB rating, then Plex/Sonarr/Radarr ratings. Preserve vote counts and every provider score for display and audit; do not merge unlike scales into a single opaque value.

**Why mirror everything:** the user wants the full picture available, and the library is brand new (small: Plex Movies = 4 items, Plex TV Shows = 8 shows / Sonarr = 24 series per the 2026-09-08 probe), so there's no scaling pressure to trim for V1. Storing the full mirror keeps options open for the UI (assignment board, student dossier, discipline map, achievement context, discipline filtering for the unlock model §5.2, featured-case ranking §5.4) without a later "add back what we skipped" pass. The stack remains the source of truth; the RPG's Postgres is a read-only mirror.

**Probe correction (2026-09-08) — stack genres plus external enrichment:** Plex exposes operational top-level genres as `<Genre tag="...">`; Sonarr/Radarr do not reliably expose genres in this library. Plex remains the fallback operational genre source. The selected V1 enrichment pipeline then adds canonical TMDb parent genre IDs, TMDb keyword-derived sub-genres, and TVDB genre/tag candidates where available. OMDb and Fanart.tv do not determine gameplay genre access. Every normalized tag records its provider and source payload in the mirror/cache (§4.6, §6.4.4a).

---

### 4.6 External metadata providers and enrichment policy

The Bear Cave services remain authoritative for operational state: Plex owns playback progress and watched state; Sonarr/Radarr own import history, file state, and their managed IDs. External providers enrich the RPG mirror only; the RPG never writes to them or uses their metadata to fabricate a watch completion.

**Provider roles (V1):**

- **TMDb** — canonical cross-source enrichment for movie/TV details, official genre IDs, keywords used as sub-genre candidates, release/air dates, ratings, credits, collections, and provider image paths. Query by the `tmdbId` from Sonarr/Radarr. Required configuration: `TMDB_API_KEY`; use the v3 API with the key sent as a query parameter or bearer authentication, never log it.
- **TVDB** — TV identity and episode/season enrichment, TVDB IDs, genres/tags, episode metadata, ratings, and artwork references where TMDb data is incomplete. Authenticate once through TVDB API v4 `/login` using `TVDB_API_KEY`, cache the returned short-lived bearer token in memory/runtime state, and refresh on expiry. Required configuration: `TVDB_API_KEY`.
- **OMDb** — IMDb-facing validation and rating fallback for movies and series, including IMDb ID, IMDb rating/votes, Rotten Tomatoes rating when returned, awards, plot, cast, and director fields. Query by `imdbId` when Radarr supplies it; otherwise use a controlled title/year lookup. Required configuration: `OMDB_API_KEY`.
- **Fanart.tv** — artwork enrichment only: posters, backgrounds, logos, clearart, banners, and thumbnails for movies and TV. Query movies by TMDb ID and TV by TVDB ID, preserving all returned artwork records and attribution/provider URLs. Required configuration: `FANART_API_KEY`.

**Precedence and conflict rules:** Plex/Sonarr/Radarr win for operational fields; TMDb wins for canonical parent genre IDs and TMDb IDs; TVDB wins for TVDB identity and episode numbering when available; OMDb is the IMDb/Rotten Tomatoes rating fallback; Fanart.tv wins only for artwork slots it supplies. Conflicting values are retained in the provider payload cache and provenance metadata rather than discarded. The featured-case ranking uses TMDb vote average first, then OMDb IMDb rating, TVDB rating, and source ratings as fallbacks.

**Caching, limits, and failure behavior:** Enrichment is incremental and keyed by `(provider, provider_id, content_id)`. A provider is queried only for new/changed content, missing fields, or an expired cache entry; normal polls reuse cached payloads. Responses, `fetched_at`, `expires_at`, HTTP status, and error details are persisted in a provider cache table. Apply bounded concurrency, exponential backoff, and `Retry-After` handling for 429/5xx responses. A provider outage never deletes a previously successful payload or blocks Plex watch detection; stale metadata is marked stale and retried on a later poll.

**TMDb genre/sub-genre rule:** TMDb official genre IDs become parent genres. TMDb keywords are normalized to lowercase slugs and become sub-genre candidates only when mapped to a parent genre by the seeded mapping table. TVDB genres/tags can add TV sub-genre candidates through the same mapping. The mapping and provider source for every tag are stored in the mirror so the rules can evolve without rewriting watch history.

---

## 5. RPG Mechanics

### 5.1 Points & XP model — completion-based (concrete V1 values)

**Core principle:** You earn points by *completing* watches, not merely by time elapsed. Partial watches yield little or nothing.

**Completion trigger (resolved §12 Q4):** A watch counts as "completed" if Plex shows the episode/movie was played to **≥95%** of its duration. Below 95% = no completion credit. The threshold is **configurable** (RPG config value; 95% default). Implementation reads Plex watch-progress / viewed state per item and episode.

**Granularity — concrete V1 values (resolved §12 Q5):**

| Unit | Trigger | XP (V1 concrete) |
|------|---------|--------|
| Episode (TV) | Episode marked watched to ≥95% | **base episode XP = 10** |
| Movie | Movie marked watched to ≥95% | **base movie XP = 20** (≈2× episode; reflects movie-length commitment) |
| TV season | All episodes of one season watched (every episode in that season at ≥95%) | **season completion bonus = 10 × (number of episodes in that season)**. E.g. a 10-episode season → +100 XP on the final episode that completes the season. Awarded once per season, on the episode that completes it. |
| TV series | All episodes across all seasons watched (every episode of every season at ≥95%) | **series completion bonus = season-completion-bonus style scaled up: 25 × (total episode count of the series)**. E.g. a 20-episode series → +500 XP when the last episode completes the series. Awarded once per series, on the episode that completes it. |
| First-completion bonus | First time you complete a given movie or series (one-time) | **+10 XP** on the completion that is the first completion of that title (movie or series). One-time per title. E.g. first time you finish a series, the series-completion XP includes the normal series bonus plus this +10. |
| New-arrival bonus | Complete a movie/episode within its **new-arrival window** (see below) after its Sonarr/Radarr import | **+5 XP** per qualifying completion. See new-arrival window definition below. |
| Featured case bonus | Complete a featured case (any selection mode) | **+10 XP** per featured-case completion, on top of the normal episode/movie XP for that watch. |
| Streak bonus | See §5.1.1 (concrete streak table) | escalating per-day streak bonus, paid at end of each day that extends the streak |
| Genre variety bonus | Complete watches in multiple genres in the same real day (see §5.1.2) | small per-day breadth bonus, paid once per day |

**New-arrival window (concrete):** a title is "new arrival" eligible for **48 hours** after its Sonarr/Radarr import (import timestamp from Sonarr/Radarr history). Within that 48h window, completing the title gives the +5 new-arrival bonus. The 48h window is **configurable** (RPG config; 48h default). The RPG detects the import via Sonarr/Radarr history polling (§9.1) and records the arrival timestamp in the content/case state; the bonus is awarded when a ≥95% completion is detected within the window.

**Streak definition (concrete):** a **day-streak** = at least one completed watch (episode or movie, ≥95%) on each of N consecutive **real calendar days** (local date, based on the host's `TZ` from `.env`). A watch counts toward the day's streak if its completion is detected on that calendar day (i.e., the poll that detects it runs on that day). A day with no detected completion breaks the streak. (Episode-vs-movie, sub-genre, etc. don't matter for the streak — any completion counts.)

**Point sinks / spending (future, V2):** XP remains earn-and-progress only; the spell-charge economy is a bounded affinity resource and does not turn XP into a spendable currency. Spell casts consume spell charges, not XP.

#### 5.1.1 Streak bonus table (concrete)

| Streak length (consecutive days with ≥1 completion) | Streak bonus (paid once, when the streak reaches that length) |
|---|---|
| 2 days | +5 XP |
| 3 days | +10 XP |
| 5 days | +25 XP |
| 7 days | +50 XP |
| 10 days | +100 XP |
| 14 days | +200 XP |
| 21 days | +400 XP |
| 30 days | +800 XP |
| 60 days | +1600 XP |

- Streak bonuses are **cumulative milestones**: each milestone in the table pays when the streak first reaches that length. E.g. reaching a 7-day streak pays +5 (2-day) +10 (3-day) +25 (5-day) +50 (7-day) = +90 XP total for the 7-day milestone event, awarded in one batch when the 7th day completes.
- If the streak breaks (a day with no completion), all streak milestone progress resets; the next completion starts a new 1-day streak. No partial credit for a broken streak.
- Streak bonus is **in addition to** the normal episode/movie XP for the watches that day — it's a bonus on top.

#### 5.1.2 Genre variety bonus (concrete)

- If, in a single real day, you complete watches in **3 or more different genres**, you earn a **genre variety bonus of +5 XP** for that day (paid once per day, at end of day / when the third-distinct-genre completion is detected).
- If you complete watches in **5 or more different genres** in the same real day, the bonus is **+15 XP** for that day (replaces the +5; i.e. 3+ genres → +5, 5+ genres → +15).
- "Different genres" = different parent genres (the genre bucket, not sub-genre). Sub-genre XP accumulation (§5.2) is separate from this variety bonus.
- Variety bonus is **in addition to** normal XP + streak bonus (if any) for the day.

### 5.2 Character & progression (concrete V1 values)

**Character:** Single student character (V1, single-player), presented as a Lantern Academy wizard.

**Level thresholds (concrete):** XP accumulates → level up at thresholds. **V1 level table:**

| Level | Cumulative XP to reach (from level 1) |
|---|---|
| 1 | 0 (starting level) |
| 2 | 100 |
| 3 | 250 |
| 4 | 500 |
| 5 | 900 |
| 6 | 1400 |
| 7 | 2000 |
| 8 | 2700 |
| 9 | 3500 |
| 10 | 4400 |

- XP is **cumulative**: you level up when your total XP crosses the threshold for the next level. Level 1 → level 2 at 100 XP total (≈10 episodes, or ≈5 movies, or a mix). The base episode XP = 10 / movie XP = 20 anchors mean ~10 episodes or ~5 movies to level 2 as a rough pace.
- On level-up, the character's level field updates and any **level-triggered unlocks** fire (discipline-access broadening — see below — plus the archetype/spell eligibility rules in §5.8). Level-up is evaluated during the poll/sync when XP is awarded.
- **Academy ranks (presentation only):** soft narrative titles at milestone levels — level 1–2 **Novice of the Lantern**, level 3–4 **Journeyman of Storycraft**, level 5–6 **Keeper of the Archive**, level 7–8 **Senior Spellwright**, and level 9–10 **Master of the Lantern**. Ranks are cosmetic display labels; they never replace the numeric level or change unlock truth.

**What leveling unlocks (decided: unlock new academy disciplines and study privileges, with concrete thresholds):**

- **Genre unlock model (resolved §12 Q6, concrete):**
  - **Horror is the opening unlocked genre** — the student begins qualified for the Shadowcraft discipline (neutral slug `horror`) at level 1. All other disciplines start **locked**.
  - **Level-broadens-discipline-access (concrete):** at each level, the set of disciplines whose content can earn full XP broadens. The database continues to use the stable `genre` terminology. Concretely:
    - **Level 1:** only Shadowcraft/horror is fully unlocked (earn full XP from horror watches).
    - **Level 2:** one additional discipline/genre becomes accessible (player's choice of the next available genre — see purchase model below; the "next available" is the next genre in the cascade, §5.2). At level 2, the player can also start **accumulating sub-genre XP** toward buying the next genre.
    - **Level 3:** a second additional genre becomes accessible, etc.
    - In general: **each level unlocks access to one more discipline** (beyond Shadowcraft/horror). So at level N (N≥1), the student has access to N genres total (horror + (N−1) purchased/accessed genres). This is the "level = gate to next discipline purchase" mapping.
    - "Access" means: you can earn **full XP** from watches in an accessed genre. For a genre that is **not yet accessed** (not unlocked, beyond your current level's allowance), watches still **accumulate sub-genre XP toward buying it** but **do not grant full XP** (or grant reduced XP) until you buy it / until your level broadens access to include it. Exact reduced-XP rule for non-accessed genres: **0 XP** from non-accessed genres until purchased/accessed (cleanest gate), or a small "exploration" XP (TBD — finalize during implementation; the spec's default assumption is 0 XP from non-accessed genres, full XP once accessed). The sub-genre XP that accumulates toward purchase still accumulates regardless of access (so you can "save up" sub-genre XP for a genre before you're high-level enough to access it — the purchase itself is gated by access, but the savings accumulate).
    - This means: **level gives the student the right to buy/access the next discipline; sub-genre XP gives the cost to buy it once that right is available.** Both are gates; both are needed.
  - **Unlock purchase via sub-genre XP (concrete):** each genre has sub-genres (from the library, filtered by sub-genre metadata, §4.5). Watching movies/episodes in a sub-genre accumulates **sub-genre XP** toward that sub-genre. When a sub-genre's accumulated sub-genre XP reaches a **purchase threshold**, the player can **"buy" that sub-genre unlock** by spending the accumulated sub-genre XP. Buying a sub-genre:
    - Marks that sub-genre as **owned/unlocked** for the character.
    - Counts as **accessing the parent genre** (if that genre wasn't already accessed) — i.e., buying the first sub-genre of a parent genre = unlocking that genre for full XP.
    - Costs the accumulated sub-genre XP (spent on purchase).
  - **Sub-genre XP purchase threshold (concrete):** **buy a sub-genre at 100 sub-genre XP accumulated in that sub-genre.** I.e. watch enough content in a sub-genre to accumulate 100 sub-genre XP toward it, then spend it to buy the sub-genre. The sub-genre XP accumulation rate: **each completed watch (episode or movie, ≥95%) in a sub-genre adds sub-genre XP equal to the normal XP for that watch** (episode → +10 sub-genre XP, movie → +20 sub-genre XP) **toward that sub-genre's purchase progress.** So a 100-sub-genre-XP purchase threshold ≈ 10 episode-watches or 5 movie-watches in that sub-genre to buy it.
    - Sub-genre XP is **per sub-genre per character** (not global). Each sub-genre has its own accumulation bucket.
    - Sub-genre XP is **spent on purchase** (not retained as general XP). It's a sink specifically for genre unlocking.
    - **Example:** horror is open at level 1. Say the library has horror sub-genres "slasher", "supernatural", "psychological". Watching horror movies/episodes accumulates sub-genre XP in each horror sub-genre you watch. Once you've accumulated 100 sub-genre XP in, say, "supernatural", you can buy the "supernatural" sub-genre — which unlocks the "supernatural horror" access (horror already open, so this is more about sub-genre coverage / suggested-title targeting / map detail, and about progress toward the genre-unlock cascade). Wait — refinement: horror is already the opened genre, so buying horror sub-genres doesn't "unlock the genre" (it's already unlocked). The purchase model's genre-unlock gating applies to **non-horror genres**: to unlock, say, "sci-fi", you buy a sci-fi sub-genre (e.g. "space opera" or "cyberpunk") once you've accumulated 100 sub-genre XP in it **and** your level gives you access to buy the next genre. The first sub-genre purchased in a non-horror parent genre = that genre becomes accessed (full XP from that genre).
    - **Cascade (concrete):** the order of disciplines available to purchase is a **fixed genre list order** (**finalized 2026-09-08:** Horror → Thriller → Mystery → Sci-Fi → Fantasy → Documentary → Comedy → Drama → Romance → Animation — 10 genres matching the 10-level table; seeded into `genres` by migration 0004 and mirrored in the §6.4.10 `genre_list_order` setting). At level 2, the player can buy the **first non-horror genre in the list** (the next one after horror) by purchasing any of its sub-genres (100 sub-genre XP in one of its sub-genres). At level 3, the **second** non-horror genre in the list becomes purchasable, etc. So level unlocks the right to buy the next genre in the list; sub-genre XP pays for it. One genre at a time, in list order, player chooses when to buy. (The list is fixed as above as of 2026-09-08; the standing rule remains "fixed ordered genre list, horror first, one new genre accessible per level, buy via 100 sub-genre XP in any of its sub-genres.")
  - **Suggested coursework per sub-genre (concrete):** when a sub-genre is **in progress** (sub-genre XP accumulated but not yet bought) or **just bought** or **available to pursue** (the next genre in the list is accessible at current level), the assignment board / discipline map suggests actual titles from the stack library filtered to that sub-genre (from the content mirror, §4.5), so the student knows what to watch to accumulate sub-genre XP toward buying it. Suggested-title lists are derived from the content mirror on each poll (filter content by sub-genre tags).
  - **Discipline difficulty:** Shadowcraft starts unlocked as the student's home discipline. Other disciplines are sealed until bought via sub-genre XP plus the level gate. Later disciplines are not intrinsically harder; their position and purchase requirements are the progression gates, not a hidden difficulty rating.
  - **Holiday / date-appropriate bonuses (resolved §12 Q6, concrete):** the app **detects the current date and applies date-appropriate bonuses** via a **holiday window calendar** (fixed calendar mapping, finalize before build — at least Halloween + winter locked down now; more deferred). Example windows:
    - **Halloween window:** Oct 1 – Oct 31. Horror-content watches (≥95% completion) during this window get **+50% XP** (episode → +15 XP, movie → +30 XP) in addition to normal XP. Applies to the **Horror** genre (the opening genre) and any horror-adjacent genres once they're unlocked (finalize exact genre scope during implementation — default: Horror only for V1).
    - **Winter holiday window:** Dec 1 – Dec 31. Cozy/holiday-adjacent content gets **+50% XP** during the window. **Genre scope for V1 (finalize before build):** pick the genre(s) that qualify for the winter window (e.g. Comedy, Drama, or "holiday-themed" — which may be identifiable by title/keywords since Plex doesn't have a "holiday" genre; finalize the winter-window genre set before build, or defer winter to a later holiday cycle).
    - **Other seasonal windows:** TBD — e.g. summer blockbuster window (Jun–Aug, action/movies), Valentine's romance window (Feb, romance), spring documentary window, etc. The spec commits to "a fixed holiday-window calendar, each window = date range + genre filter + bonus multiplier (≥95% completion in-window → bonus XP)." The Halloween + winter examples are concrete starters; the rest are finalized-before-build or deferred.
    - **Bonus type:** V1 = **bonus XP** (multiplier on the normal XP for qualifying watches in the window). **Items/perks** as the bonus are noted as a V1-possible extension (finalize during implementation — the spec's default is bonus XP only for V1; items are optional and deferred unless decided otherwise).
    - Holiday windows are **implemented as date-gated feature modules** (§15.4/§15.5 inspiration from LoGD's holiday text modules): each window = a small feature with a start date, end date, genre filter, and bonus multiplier, stored in `settings.holiday_windows` (§6.4.10). New windows can be added by adding a new window record without touching core.
- **Legacy neutral perks:** older §5.2 perk ideas remain documented historical material but are not part of the wizard implementation contract. The wizard contract in §5.8 supersedes them: archetype modifiers and spells are the only new selectable effects. The `perks_unlocked` setting remains disabled for this release; do not implement both systems or stack an archetype with a separate perk tree without a new spec decision.
- **Academy ranks:** the presentation-only rank ladder in the preceding bullet is shown on the student dossier alongside the unchanged numeric level.

**Stats (V1):** XP (total), level, watch count (total completions), episode count, movie count, disciplines accessed, sub-genres owned, current streak (days), best streak, completed assignments/courses (series completed, movies completed, featured cases completed), sub-genre XP per sub-genre (for purchase progress), holiday-window bonus count, achievement count (unlocked / total). Displayed on the student dossier.

### 5.3 Movies vs TV — distinct roles

| Aspect | Movies | TV Shows |
|--------|--------|----------|
| RPG framing | One-off *assignments* | Multi-episode *courses* / *term studies* |
| Completion unit | Whole movie | Episode → Season → Series |
| Bonus structure | Movie completion + new-arrival + first-completion | Episode XP + season bonus + series bonus + streak |
| Coursework presentation | A movie = an assignment card the student may choose | A show = an ongoing course; episodes are lessons collected toward completion |

Both feed the same XP/level system. They differ in *granularity and framing*, not in fundamentals.

### 5.4 New arrivals & featured cases

**New arrivals:** When Sonarr/Radarr records a new import (or a new item appears in the library), the RPG can generate an *assignment card* for it. The card represents new coursework. It is **not assigned** — the student sees it on the assignment board and chooses whether to take it on.

**Featured cases:** A smaller set of cases highlighted each period (V1 default: weekly). Select from available content using either the configured new-arrival mode or all-time mode. Rank candidates by normalized provider score: TMDb vote average, then OMDb IMDb rating, TVDB rating, then stack ratings. Restrict candidates to the character's accessed genres and retain the selected provider scores in the featured-case audit. Featured cases give +10 XP when completed.

**Player-driven case picking:** The player sees a board of available cases (new arrivals + existing library unwatched + featured) and *chooses* which to take. The RPG doesn't auto-assign. This preserves autonomy — you watch what you want; the RPG just frames it.

### 5.5 Achievements & badges

**Hybrid model (decided, Q8 resolved — massive list):** a large/extensive achievement list across five categories, split into **visible** (player can see and work toward) and **hidden** (revealed only when unlocked). Specific items below are the first cut; the list is meant to be extensive and can grow during implementation. Categories match §5.5's category model: completion milestones, genre coverage, time/streak, novelty, themed/quirky.

**How achievements award:** each achievement has a trigger condition evaluated during the poll/sync (§9) and/or on assignment completion. On unlock, the achievement is recorded in `character_achievements` (unlocked-at, source) and shown on the certificate wall / student dossier. Hidden achievements reveal their name + description on unlock; visible achievements show progress toward them.

#### 5.5.1 Completion milestones (visible)

- **First Blood:** Complete your first episode (any show). ✔ visible
- **Case Closed:** Complete your first movie. ✔ visible
- **Double Feature:** Complete 2 movies in the same real day. ✔ visible
- **Episode 10:** Complete 10 episodes total. ✔ visible
- **Episode 50:** Complete 50 episodes total. ✔ visible
- **Episode 100:** Complete 100 episodes total. ✔ visible
- **Silver Screen Novice:** Complete 10 movies total. ✔ visible
- **Silver Screen Devotee:** Complete 50 movies total. ✔ visible
- **One Season Under Your Belt:** Complete all episodes of one season of any show. ✔ visible
- **Two Seasons:** Complete all episodes of two seasons (same or different shows). ✔ visible
- **Series Completed:** Finish every episode of an entire series (all seasons). ✔ visible
- **Collector:** Complete 5 different series. ✔ visible
- **Completist:** Complete 10 different series. ✔ visible
- **Backlog Burner:** Complete a series where at least 3 episodes were already marked watched before you took the case (i.e., you backtracked into completion). ✔ visible
- **Full Slate:** Complete at least one episode and one movie in the same real day. ✔ visible

#### 5.5.2 Completion milestones (hidden)

- **The Quiet 100:** Complete 100 episodes without ever manually refreshing the case board (all discoveries via poll). ✔ hidden
- **Ghost Completer:** Complete a series where no episode was watched live via Plex during your case — i.e., the whole series was already in your watched history when you claimed it. ✔ hidden
- **One-Click Wonder:** Complete a featured case on the same day you first saw it appear. ✔ hidden
- **Twice in a Day:** Complete two different series' final episodes in the same real day. ✔ hidden
- **Slow Burn:** Complete a 10+ season series (or equivalent long campaign). ✔ hidden

#### 5.5.3 Genre coverage (visible)

- **First Genre Explored:** Complete at least one watch in your first unlocked genre (horror, the opening genre). ✔ visible
- **Genre Explorer:** Complete at least one watch in 3 different genres. ✔ visible
- **Genre Explorer:** Complete at least one watch in 5 different genres. ✔ visible
- **Genre Explorer:** Complete at least one watch in 8 different genres. ✔ visible
- **Genre Explorer:** Complete at least one watch in 10 different genres. ✔ visible
- **Horror Homeground:** Complete 10 horror watches (the opening genre). ✔ visible
- **Horror Homeground:** Complete 25 horror watches. ✔ visible
- **Genre Purchase:** Buy your first sub-genre unlock with sub-genre XP (the first purchased genre beyond horror). ✔ visible
- **Genre Purchase:** Buy 3 sub-genres via sub-genre XP. ✔ visible
- **Genre Purchase:** Buy 5 sub-genres via sub-genre XP. ✔ visible
- **Genre Fiesta:** Complete at least one watch in a sub-genre you just bought (same poll cycle or next). ✔ visible
- **Variety Player:** Complete watches in at least one sub-genre for 5 different parent genres. ✔ visible
- **Broad Coverage:** Complete at least one watch in sub-genres across 3 different parent genres in the same real day. ✔ visible

#### 5.5.4 Genre coverage (hidden)

- **Horror Native:** Complete 50 horror watches before buying any other genre. ✔ hidden
- **Sub-genre Hoarder:** Buy 10 sub-genres via sub-genre XP. ✔ hidden
- **Full Catalog:** Unlock every sub-genre the library exposes (within the current library's genre set). ✔ hidden
- **Peerless Variety:** Complete watches in 15 different genres. ✔ hidden
- **Depth & Breadth:** Complete both a 10+ season series and watches in 10+ genres. ✔ hidden

#### 5.5.5 Time / streak (visible)

- **Back-to-Back:** Complete watches on 2 consecutive real days. ✔ visible
- **Streak Starter:** Maintain a 3-day watch streak (a completed watch on each of 3 consecutive real days). ✔ visible
- **Streak Builder:** Maintain a 5-day watch streak. ✔ visible
- **Streak Keeper:** Maintain a 7-day watch streak. ✔ visible
- **Streak Veteran:** Maintain a 14-day watch streak. ✔ visible
- **Streak Legend:** Maintain a 30-day watch streak. ✔ visible
- **No Gap:** Maintain a 7-day streak where each day also included a new-arrival bonus watch (watched something within its new-arrival window). ✔ visible
- **Weekend Warrior:** Complete at least one watch on each of 4 weekends in a row (Saturday or Sunday). ✔ visible
- **Holiday Heat:** Complete a watch during a detected holiday window (e.g., Halloween horror window) and earn the holiday bonus for that window. ✔ visible
- **Holiday Heat:** Earn the holiday bonus in 3 different holiday windows (e.g., Halloween, winter holidays, etc.). ✔ visible

#### 5.5.6 Time / streak (hidden)

- **Iron Streak:** Maintain a 60-day watch streak. ✔ hidden
- **Unbroken:** Maintain a 30-day streak without a single day relying solely on a re-watch (every day had a new completion). ✔ hidden
- **Holiday Sprinter:** Complete 3 watches across 3 different holiday windows within their respective windows. ✔ hidden
- **Marathon:** Reach level 5 (or whatever the level-5 threshold ends up being) while maintaining a 14-day streak through the level-up. ✔ hidden
- **Seasonal Veteran:** Earn holiday bonuses in all holiday windows the current calendar year exposes. ✔ hidden

#### 5.5.7 Novelty / firsts (visible)

- **First Featured Case:** Complete your first featured case (any selection mode). ✔ visible
- **Featured Fan:** Complete 5 featured cases. ✔ visible
- **Featured Master:** Complete 10 featured cases. ✔ visible
- **First New-Arrival Bonus:** Earn a new-arrival bonus (watched something within its new-arrival window). ✔ visible
- **New-Arrival Habit:** Earn new-arrival bonuses on 5 different titles. ✔ visible
- **Speed Demon:** Complete a new arrival within 48 hours of its arrival on the stack (Sonarr/Radarr import). ✔ visible
- **Speed Demon:** Complete a new arrival within 24 hours of arrival. ✔ visible
- **First Purchase:** Buy your first sub-genre unlock (first genre purchase beyond horror). ✔ visible
- **Level Up:** Reach level 2. ✔ visible
- **Level Up:** Reach level 3. ✔ visible
- **Level Up:** Reach level 5. ✔ visible
- **Level Up:** Reach level 10. ✔ visible
- **First Banked Day:** (if daily budget is enabled) use a daily investigation budget for the first time. ✔ visible
- **First Fame Tick:** (if fame is enabled) earn your first fame/renown increment. ✔ visible

#### 5.5.8 Novelty / firsts (hidden)

- **Instant Case:** Take a case from the board and complete it within the same poll cycle (i.e., you watched it between polls after taking it). ✔ hidden
- **Double Speed:** Complete two different new arrivals within 24 hours of each of their arrivals. ✔ hidden
- **Featured Streak:** Complete 3 featured cases in a row (3 consecutive featured cases completed, one after another). ✔ hidden
- **First-Day Fighter:** On your very first day in the RPG, complete both a movie and an episode. ✔ hidden
- **New-Old Hybrid:** Complete a new-arrival bonus on a title that was also a featured case. ✔ hidden

#### 5.5.9 Themed / quirky (visible)

- **Double Feature Special:** Watch two movies from the same franchise/franchise-indicated pair (same series/franchise metadata) in the same real day. ✔ visible
- **Director's Cut:** Complete two movies by the same director (director metadata, where available) within 7 days. ✔ visible
- **Themed Night:** Complete 3 movies/episodes in the same genre in the same real day. ✔ visible
- **Binge Builder:** Complete 5 episodes of the same series within 48 hours. ✔ visible
- **Weekend Binge:** Complete 5 episodes of the same series over a single weekend. ✔ visible
- **Marathon:** Complete an entire season's worth of episodes within 7 days of starting it. ✔ visible
- **Holiday Haunter:** Complete a horror watch during the Halloween window (holiday bonus earned). ✔ visible
- **Holiday Warmth:** Complete a cozy/winter-holiday-adjacent watch during the winter holiday window (holiday bonus earned). ✔ visible

#### 5.5.10 Themed / quirky (hidden)

- **Spooky Season:** Complete 10 horror watches during the Halloween window (cumulative across years, or current year — finalize during implementation). ✔ hidden
- **Franchise Head:** Complete the first movie of 5 different franchises (first installment of 5 franchises). ✔ hidden
- **Director's Passport:** Complete movies by 5 different directors (where director metadata is available). ✔ hidden
- **Actor's Playground:** (if cast metadata is mirrored) complete two movies/episodes sharing a lead actor (where lead-actor metadata is available) — hidden, finalize during implementation based on what metadata is actually mirrored. ✔ hidden
- **The Oldie:** Complete a movie/episode whose release/air date is more than 20 years before the current date. ✔ hidden
- **The Newcomer:** Complete a movie/episode whose release/air date is within the last 30 days. ✔ hidden
- **Tuesday Thriller:** Complete a thriller/mystery watch on a Tuesday (day-of-week + genre quirk). ✔ hidden
- **Friday Night Film:** Complete a movie on a Friday night (evening local time). ✔ hidden
- **Dawn Watcher:** Complete a watch that started before 6:00 local time. ✔ hidden
- **Night Owl:** Complete a watch that started after 22:00 local time. ✔ hidden
- **Rainy Day (metadata-dependent):** (if weather/date info is available on the host) complete a watch on a date that matches a library holiday/seasonal theme not yet earned a bonus for. ✔ hidden — finalize during implementation.

#### 5.5.11 Cross-category combo achievements (visible)

- **Well-Rounded:** Complete at least one movie and at least one episode, and earn at least one new-arrival bonus, all in the same real day. ✔ visible
- **Streak Collector:** Maintain a 7-day streak while also completing a full series during that streak. ✔ visible
- **Genre + Streak:** Maintain a 7-day streak where each day included a watch in a different genre. ✔ visible
- **Featured + New:** Complete a featured case that was also a new arrival (within its new-arrival window). ✔ visible
- **Horror + Holiday:** Complete the Halloween holiday bonus and also complete 5 horror watches in the same window. ✔ visible
- **First 100 + Streak:** Reach 100 total completed episodes while maintaining a 7-day streak. ✔ visible

#### 5.5.12 Cross-category combo achievements (hidden)

- **Perfect Day:** In a single real day: at least one movie, at least one episode, at least one new-arrival bonus, at least one featured case, and at least one holiday-window bonus — all in the same day. ✔ hidden
- **Silent Completer:** Reach 100 total episodes completed without ever having a watch logged via manual refresh (all via poll), and without ever missing a day in a 30-day streak that overlaps the 100th episode. ✔ hidden
- **Genre Omnivore:** Unlock and complete at least one watch in every sub-genre the library exposes, plus complete 3 full series, plus maintain a 14-day streak. ✔ hidden
- **Holiday Sweep:** Earn holiday bonuses in all holiday windows for the current year, and complete a series in each of 3 different genres during those windows. ✔ hidden

#### 5.5.13 Badge rendering (decided)

- Badges/achievements appear on the **character sheet** and/or a **badge wall** view.
- **Visible achievements:** show name + description + progress (e.g. "5/10", "3/5", "not started", or a progress bar) on the badge wall.
- **Hidden achievements:** show as locked/unknown until unlocked; on unlock, reveal name + description + the unlock date, and add to the badge wall.
- Recently unlocked achievements get a "just unlocked" emphasis (e.g. pinned at top of badge wall for a short time).
- Some achievements could render as small icons/badges on the character sheet (e.g. "Horror Native" badge near the horror genre region on the map, or a streak badge near the streak stat).
- Badge artwork is an implementation detail (simple icons / emoji / generated SVGs to start; polished art later).

### 5.6 Shared quests — V2 only

**V1 = single-player only.** The opt-in shared quest concept (post a shared case, another accepts, co-op bonus) is noted as a V2 extension. Not built in V1. Documented in the spec for future reference.

### 5.7 Mystery watch orders (finalized 2026-09-08)

A **mystery watch order** is a per-genre, numbered sequence of movies where the player only ever sees the current item. The next title is not revealed until the previous one is resolved.

- **The loop:** one active order per genre per character (its `cycle_number` increments on completion). An order has an ordered item list; the **first item is revealed on creation**. Item `n+1` reveals when item `n` is resolved.
- **Reveal rule (single source of truth):** an item is *resolved* when the player has an awarded watch for it — exactly the §6.4.5 award, i.e. a `watches` row at ≥95% `pct_viewed`. Completion/progress is **always derived** from `watches` (never stored on the item), so there is one threshold, one ledger, and no drift between the mystery rule and the XP rule.
- **Rewards:** completing an order grants **skips** (V1: 1 skip per completed order) recorded in an audited grant ledger. A skip resolves the **current** item without watching it — no XP, no watch row, and **the final item of an order can never be skipped** (the finale must be watched). A skipped item still counts toward order completion. Skips are spendable in any active order of the **next** cycle of that genre (and any later one — banked skips persist).
- **Mystery is enforced at the API:** a locked item serializes as `{position, locked: true}` and nothing else — no title, no poster, no content id. The reveal timestamps (`revealed_at`) exist for audit and achievement triggers, not for display.
- **Finalized decisions:** (1) **Authoring** — V1 generates orders algorithmically from the library (deterministic per cycle: genre-matching content the character has no awarded watch for, ranked by provider score per §5.4); curated JSON pick-lists are a noted V1.1 extension, not a V1 dependency. (2) **Pre-watched items** — content the character already completed is **excluded at generation** (the honest-mystery rule; consistent with §5.5 Ghost Completer). (3) **Movies only** in V1 (`content_type = 'movie'`) — episode orders fight natural binge behavior. (4) **Per-genre achievements** extend the §5.5 list in a `watch_orders` category (completion trophies per genre, no-skip completions, streaks of consecutive cycles) with a per-discipline **awards shelf** on the certificate wall — the shelf rendering is a frontend concern.
- **Where it hooks:** generation and reveals run inside the game tick (poll/sync + refresh, §9) — detect watches → award → **reveal next item (and grant skips on completion)** → evaluate achievements.

### 5.8 Wizard archetypes and spellbook (finalized 2026-09-10; implementation deferred)

The wizard layer is a strategic loadout over the one existing V1 character. It does **not** create a second `characters` row, watch history, XP pool, or parallel progression ledger. The neutral `character_state`, `watches`, `genre_access`, `sub_genre_xp`, `watch_orders`, `skip_grants`, and `character_achievements` tables remain authoritative. The backend-first release implements the complete six-archetype/five-spell contract before the Svelte UI is treated as done.

#### 5.8.1 Active archetype and queued switching

- The account begins with **The Lantern Scholar**, a neutral starter with no modifiers. Existing characters are backfilled to this archetype by 0012 without changing their name, level, XP, watches, or achievements.
- A character may permanently unlock many archetypes but has exactly **one active archetype**. No archetype stacking, party, house, or alternate save exists in V1.
- Selecting an already-unlocked archetype creates one immutable **pending selection**. It does not change the current loadout immediately. The pending selection is applied transactionally at the next game tick, before that tick awards or resolves any operation; the response exposes both current and pending state.
- A new selection is accepted only when there is no pending selection and the character has not accepted an archetype selection during the current local calendar day. The accepted request's local date is stored immediately for rate limiting; the active loadout changes only when the next tick applies the pending selection. A request that conflicts with either rule is rejected without side effects. This makes the selection effective at a deterministic tick boundary rather than halfway through a poll.
- A request for the already-active archetype is a no-op rejection, not a daily selection. A pending selection cannot be cancelled or replaced in V1; if the requested archetype is no longer eligible when the tick runs, the transaction clears the pending request without changing the active archetype and records the rejected event.
- Unlocks are permanent and idempotent. Unlock facts are evaluated from persisted progression during the game tick; selecting an archetype never rewrites prior watches, XP, achievements, orders, spell events, or affinity progress.
- The backend evaluates the active archetype at the moment an operation is awarded or resolved. The UI cannot retroactively choose a loadout for an existing event.

#### 5.8.2 V1 archetype matrix and unlock conditions

These are original Lantern Academy identities, not references to any external franchise. Every archetype has one primary mechanic and at most one secondary modifier. Percentage effects are exact, additive, prospective, and bounded to ±10%; the two token/ward effects are explicitly capped resources rather than hidden multipliers.

| Archetype | Unlock condition | Primary mechanic | Secondary modifier / weakness |
|---|---|---|---|
| **Lantern Scholar** | Granted at character bootstrap | Baseline: no primary modifier | No secondary modifier; reliable neutral route. |
| **Ember Adept** | Reach level 2 | `+10%` movie normal-XP component | `-10%` episode normal-XP component. |
| **Veil Cartographer** | Complete one mystery watch order | Gain one **preview token** per newly created order cycle; it may reveal one additional non-final item without resolving it | `-10%` affinity progress from movie watches. |
| **Rune Forger** | Unlock three achievements | `+10%` to the displayed progress meter for visible achievements only | `-10%` normal-XP component on all watches; unlock truth is unchanged. |
| **Star Shepherd** | Reach a seven-day streak | Gain one **streak ward** on unlock and one replacement ward after each 30-day local cooldown; a ward protects one otherwise cold-gap transition | `-10%` affinity progress from episode watches. |
| **Moonlit Mediator** | Access three parent genres | `+10%` affinity progress for watches matching the selected spell discipline | `-10%` normal-XP component for Shadowcraft/horror watches. |

A preview token reveals content but never resolves an item; it cannot reveal the finale before its predecessor is resolved and it cannot bypass §5.7 redaction for an item not legally revealable. One preview token is granted for each newly created order cycle while Veil Cartographer is active, is consumed by a successful extra reveal, and is never retroactive; an order with no eligible extra reveal consumes nothing. A streak ward changes only the streak transition, never the watch date, watch row, XP ledger, or completion threshold. Star Shepherd grants one ward on archetype unlock and one additional ward after each 30-day local cooldown only if the prior ward has been consumed; unused wards do not stack beyond one. An effect that would produce a negative amount is clamped to zero. Archetypes never change the 95% threshold, award-once dedupe, finale rule, or achievement truth.

#### 5.8.3 Modifier precedence and tick boundary

At the start of a game tick, the server applies any pending archetype selection and pending spell-affinity selections in one transaction. For each newly awarded watch it computes the neutral base first, applies the active archetype's one primary effect and optional secondary effect to their named components, applies a targeted spell effect, then applies existing contextual bonuses (holiday, new-arrival, featured, streak, and variety) according to their neutral rules. This order is the only legal stacking order.

- Percent modifiers on the same component add, rather than multiply; final component multipliers are clamped to **50%–200%** before integer rounding down.
- A primary non-percentage effect is represented as an explicit token/ward balance and is never smuggled in as an XP multiplier.
- `watches.normal_xp`, `watches.bonuses`, and `watches.xp_awarded` record the actual applied result. Archetype changes and definition changes are prospective; historical rows are immutable.
- Display-only progress effects never alter the neutral evaluator or unlock condition. A single active archetype and a single targeted spell may affect an operation; no other loadout state participates.

#### 5.8.4 Spell affinity economy

Spells are finite, auditable charges earned from the student's viewing choices. Each spell has exactly **one selected parent discipline** (the stable genre slug) and a visible meter from `0` to `100` affinity points toward the next charge.

- On first setup, the player selects the spell's discipline before the spell can earn a charge. The initial selection is queued and becomes active at the next game tick.
- A player may request at most **one affinity change per spell per local calendar day**. The request is immutable once queued, takes effect at the next tick, and never reclassifies earlier watches. Changing the selected discipline does not reset the meter; only newly awarded watches after activation can add progress.
- Only a newly inserted `watches` row can advance affinity. Re-watches, skipped items, preview tokens, spell casts, syncs, manual claims, and achievement rows add no affinity points. A spell's selected discipline must be an accessed parent genre; a watch matches when its authoritative parent genre matches that selection. If a newly awarded watch matches, it adds `floor(neutral normal_xp / 2)` points: **5 for an episode and 10 for a movie**, before the active archetype's affinity modifier. Apply a `+10%` affinity modifier by multiplying the unrounded point value, then round down; the bounded modifier therefore yields 5/10 normally or 5/11 for episode/movie when Moonlit Mediator applies. A matching watch may advance every spell currently assigned to that discipline; there is no hidden global affinity pool.
- Each full **100 affinity points** grants one charge and carries any remainder into the next meter, unless doing so would put unspent charges above 3. At most **3 unspent charges per spell** may exist. When a matching watch arrives while the spell is already at 3 charges, all calculated affinity points are dropped and one `overflow_noop` audit event records the amount. If a watch crosses the cap, charges are minted only up to 3 and any remainder that cannot be retained because the cap was reached is also recorded as `overflow_noop`; the meter is left at 0 at the cap. After a charge is spent, later matching watches resume from the stored meter value. Overflow never creates debt or retroactive charges.
- Charges persist without expiry. Grants and casts are transactional. A valid cast locks and consumes exactly one available charge; an invalid target, locked item, missing affinity setup, or insufficient balance consumes nothing. Each grant, overflow, affinity change, and cast has a stable source/event key and is recorded for audit.

#### 5.8.5 Initial spellbook and exact effects

| Spell | Effect | Ledger/order boundary |
|---|---|---|
| **Vanishing Step** | Resolve the current non-final mystery-order item without watching it | Consumes one charge and stamps `watch_order_items.skipped_at`; it does not consume `skip_grants`, create `watches`, grant XP, or resolve the finale. |
| **Unsealing Light** | Reveal the next legally revealable locked item without resolving its predecessor | Consumes one charge and records a reveal audit; the item remains unresolved and all other locked-item redaction rules remain in force. |
| **Chronicle Ward** | Protect one future cold-gap streak transition | Consumes one charge only when that transition occurs; it preserves the streak transition rule but never changes the watch date or fabricates a completion. |
| **Focus Sigil** | Add `+10%` to the next watch's normal-XP component | Consumes one charge on the next newly inserted watch, applies in the precedence order, respects the clamp, and records the applied amount in `watches.bonuses`. |
| **Second Sight** | Mark one active order as the student's study target for the next cycle | Consumes one charge and targets only an unstarted active order; it changes priority/presentation, never candidate ranking, reveal legality, skip legality, completion, or XP truth. |

The five spell definitions are visible even before setup. The UI distinguishes `needs_affinity_setup`, `ready`, `charged`, `at_capacity`, and `locked` states, and displays the `progress_points/100`, `unspent_charges/3`, selected discipline, pending discipline, and next eligible change time. The server exposes enough state to explain a capped meter and every discarded overflow event.

#### 5.8.6 Unlocking and displaying wizard progression

Archetype unlock facts remain: Lantern Scholar at bootstrap; Ember Adept at level 2; Veil Cartographer after one completed order; Rune Forger after three achievements; Star Shepherd at a seven-day streak; Moonlit Mediator after three accessed parent genres. Spell charges are **not** granted by those milestones. The five spell definitions become usable when their own unlock facts are met: Vanishing Step after the first completed order, Unsealing Light at level 2, Chronicle Ward at a seven-day streak, Focus Sigil at five achievements, and Second Sight at three accessed parent genres. A definition may remain visible as `locked` before its fact is met, but it cannot be assigned affinity or cast.

Affinity watches are the sole V1 charge source, so the economy remains predictable and route-plannable. The first affinity setup is available only after that spell is unlocked; setup must select one accessed discipline and is applied at the next tick. A spell with no setup has no active meter and cannot receive affinity from earlier watches.

The guided student dossier shows academy level/mastery, active and pending archetype, the single primary and optional secondary effect with their tradeoff explanation, neutral progression facts, and each spell's affinity meter and charge balance. It recommends the starter loadout and explains why a selection/cast is unavailable; it never computes eligibility or effects client-side.

---

## 6. Data Model (PostgreSQL)

### 6.1 Rationale

PostgreSQL is introduced as a **shared metadata store** — the RPG uses it as its primary DB, and the expectation is that other stack-adjacent tooling could reuse it over time (e.g., richer activity feeds, discovery tooling, cross-service views). Existing *arr/Plex services keep their own SQLite and are not modified.

### 6.2 Schema (initial, TBD in detail during implementation)

**Core tables (conceptual):**

- `characters` — student characters (V1 = one row; multi-player future).
- `character_stats` / `character_xp` — XP, level, per-discipline unlock state, and perks.
- `watches` — records of watches earned: character, content id (TMDb/TVDb), content type (movie/episode/series), watched-at, watched-via (plex/manual-flag for V2), points awarded, source metadata.
- `content` — enriched content catalog the RPG knows about: movies and series/episodes, pulled from Plex/Sonarr/Radarr. Includes metadata (title, year, genres, ratings, IDs, section/path). Incremental sync, not a full rebuild each poll.
- `cases` — assignment cards: content reference, case type (movie-case / series-campaign / featured), status (available / taken / in-progress / completed), taken-at, completed-at, bonus flags.
- `featured_cases` — periodic featured case assignments (period, content ref, bonus).
- `achievements` — achievement definitions (id, name, description, category, visible/hidden, trigger condition).
- `character_achievements` — which character unlocked which achievement and when.
- `watch_orders` — per-genre mystery watch orders (§5.7) with ordered, progressively revealed items and an audited skip-grant ledger.
- `genre_unlocks` / `genre_progress` — which genres are unlocked at current level, coverage counts.
- `sync_state` — poll cursors / last-sync markers for Plex/Sonarr/Radarr incremental syncs.
- `wizard_archetypes` / `character_archetypes` — original archetype definitions and permanent unlocks; a single active selection overlays `character_state`.
- `spells` / `character_spell_ledger` — spell definitions plus auditable grant/consumption rows; never a replacement for `watches` or `skip_grants`.
- `wizard_presentations` — optional display metadata for genres, achievement awards, portraits, and spell art; it must not duplicate progression truth.

**Sync pattern:** The RPG maintains a curated content table by periodically syncing from Plex/Sonarr/Radarr (incremental, keyed by IDs and last-sync markers). Watches flow from Plex watch-state changes into the `watches` table when detected by polling.

**Design principle:** The content table is a *read-only mirror* of stack library metadata. The watches table is the RPG's own record of what it awarded points for. Both are in Postgres.

### 6.3 Connection & deployment of Postgres

- **Not a new Compose container.** Postgres is expected to be available on the host network.
- **Hosting mode (resolved §12 Q1, with probe correction):** **host-side Postgres install**, available on host network, not a new compose container. **Probe result (2026-09-08):** **no Postgres currently available on the host** — no `pg_isready`/`psql` on PATH, no systemd `postgresql` service, nothing on port 5432. The RPG backend cannot connect to Postgres until one is installed/provisioned.
- **Concrete provisioning step (finalize before build — pre-build task, not RPG code):** install Postgres on the host (default assumption: host package-manager install → systemd `postgresql` service → a dedicated `rpg` database + a dedicated db user + `RPG_DB_URL` = `postgresql://<user>:<pass>@localhost:5432/rpg`). Exact install method (package manager + version), data directory, service name, and auth (password for the db user) are implementation/pre-build details. The RPG backend only ever reads `RPG_DB_URL` from `.env` and connects — it does not install Postgres itself.
- **Auth/config:** `RPG_DB_URL` (Postgres connection string, e.g. `postgresql://rpg_user:rpg_pass@localhost:5432/rpg`) in `.env`/`.env.template`. Not committed. The account/password for the RPG login (§7.3/Q10, the set-a-pin) is stored in Postgres (the `accounts.pin_hash`), not in `.env`. The Postgres db-user password (in `RPG_DB_URL`) is the only Postgres credential in `.env` — also not committed.
- **Migration approach:** A migration layer (e.g., SQLx migrations, or a lightweight migration table) to manage schema versioning.

---

## 7. Backend — Rust, extending `~/Cave/backend/`

### 7.1 Current state of `backend/`

As of now, `backend/src/` contains a partial Rust/Axum codebase:

- `routes.rs` — full route tree for a **stack-management dashboard** (containers, stick/queue, nzbdav, plex, credentials, env editor, catalog, host overview, notifications, library, jobs). Many handlers are `todo!()` placeholders pending later milestones.
- `config.rs` — project-root resolution, `.env` path, script/config path helpers.
- `docker.rs` — thin `bollard` Docker helpers.
- `executor.rs`, `jobs.rs` — executor and job scaffolding (partial).

There is **no `Cargo.toml`**, no `main.rs`, and no complete runtime yet — the backend is early/incomplete.

### 7.2 Decision: new crate alongside, same `backend/` directory

- The RPG is a **separate crate** in `backend/` (e.g., `backend/rpg/` with its own `Cargo.toml`), alongside the existing stack-management crate.
- **Rationale:** Loose coupling. The stack-management routes are a different product (stack ops dashboard) from the RPG (personal RPG UI). Keeping them as separate crates in the same directory avoids mixing concerns while staying colocated.
- **Shared modules (resolved §12 Q11):** **common libs are fine** — both crates may share a common `backend/` library crate (e.g., shared `.env` loading/config resolution, shared HTTP helper patterns, shared auth/secret patterns) where it reduces duplication. The existing `routes.rs` and stack-management code is **not** the RPG. The RPG gets its own router, its own modules (rpg logic, rpg db, rpg probes for Plex/Sonarr/Radarr), and its own handlers. Whether to extract a shared lib is fine to decide during implementation; the spec permits it and expects minimal, non-coupling sharing (no shared RPG state in the common lib).
- The existing `routes.rs` and stack-management code is **not** the RPG. The RPG gets its own router, its own modules (rpg logic, rpg db, rpg probes for Plex/Sonarr/Radarr), and its own handlers.

### 7.3 RPG backend surface (decided)

- **Rust + Axum** (consistent with existing `backend/`).
- **New router:** e.g., `/rpg/*` or a top-level RPG route tree — TBD.
- **RPG-specific modules:**
  - DB layer (SQLx or similar) connecting to Postgres.
  - Plex/Sonarr/Radarr probe layer (HTTP clients, auth from `.env`).
  - RPG logic: XP/level calculation, achievement evaluation, case generation, genre unlock logic.
  - Polling/sync scheduler: periodic sync of content from Plex/Sonarr/Radarr, periodic check for new watches.
- **Auth (resolved §12 Q10):** **set-a-pin gate** — a simple PIN-based authentication gate on the RPG frontend/backend (not a full username/password account system). V1 scope: a single PIN stored in Postgres (hashed), one character/account behind the pin for the single-player V1 use; a wrong PIN is rejected at the gate. PIN hashing is **finalized (§6.4.1): the RustCrypto `argon2` crate, Argon2id variant, PHC string format** (`pin_hash` = full PHC string, `pin_salts` = base64 salt), and the **set/verify flows are finalized (§6.4.1)**: set-PIN is first-run-only (4–12 digits, rejected if an account exists) and triggers character bootstrap; verify loads the single `pin_hash` (no account = locked), re-derives Argon2id, and rejects wrong PINs without side effects. **Session mechanics (finalized 2026-09-08):** the backend serves the PIN gate as an **Axum HTTP server on port 46532** (§10.2): after a successful set-PIN or login, the backend issues an **opaque random session token** (128-bit, server-generated) delivered as an **`rpg_session` HttpOnly cookie** (`Path=/; SameSite=Lax`); sessions live **server-side in memory** with a **7-day expiry** (restart clears all sessions — acceptable for V1, no session persistence), and logout deletes the session and clears the cookie. Every route except `GET /healthz`, `GET /auth/status`, `POST /auth/set-pin`, and `POST /auth/login` requires a valid session cookie (401 otherwise). The frontend (Svelte, §8.4) consumes this API.

### 7.4 What the backend serves

- **API endpoints** for the frontend: character state, watches, cases, achievements, stats, content/case board, sync status. Auth-gated where appropriate (character/account data behind login).
- **Frontend serving:** Svelte assets (see §8.4), served by the Axum backend (or a separate dev server during development).
- **Wizard academy boundary (finalized 2026-09-10):** the backend returns authoritative archetype eligibility, current/pending loadout, active modifiers, affinity selections and progress, spell balances, spell target validation, order legality, and award results. The frontend renders the academy shell, portraits, spellbook, discipline map, and awards shelf; it never computes XP, unlocks, reveals, skips, affinity progress, or cast legality.
- **Response contract:** neutral IDs/slugs, timestamps, ledger amounts, and eligibility reasons remain available for audit. The UI is guided rather than power-user-first: it recommends the starter path, explains tradeoffs and requirements, and progressively surfaces advanced decisions without hiding their underlying state. Presentation fields (`display_name`, `description`, `portrait_key`, `icon_key`, `art_key`, discipline label, and award label) are additive and replaceable. A locked archetype or spell is returned with its unlock requirement and zero balance, not hidden; a locked order item remains governed by §5.7's redaction rule. Mutating responses return the authoritative post-transaction state plus a machine-readable event summary, so the UI does not infer whether a switch, affinity change, cast, reveal, or grant succeeded.
- **Required wizard state fields:** the character payload exposes `active_archetype`, `pending_archetype` (or null), `archetype_switch_eligible_at`, `archetypes[]` with `unlocked`, `primary_effect`, `secondary_effect`, `strengths`, `weaknesses`, and eligibility reason; `spells[]` with `selected_discipline_slug`, `pending_discipline_slug`, `affinity_progress_points`, `affinity_threshold`, `unspent_charges`, `charge_cap`, `status`, and `next_affinity_change_at`; and a tick/event summary containing applied selections, affinity changes, grants, overflow no-ops, and casts. The API must distinguish current from pending state and must not expose a supposedly revealed locked-order title through any other endpoint.
- **Smallest later API slice:** preserve `GET /api/character`, `GET /api/achievements`, and `GET /api/orders` for neutral facts. Add `GET /api/archetypes` (definitions + unlock state + current/pending selection), `POST /api/archetypes/{slug}/select` (immutable next-tick selection), `GET /api/spells` (definitions + affinity/balance state), `POST /api/spells/{slug}/affinity` (immutable next-tick discipline selection), and `POST /api/spells/{slug}/cast` (server-validated target and transaction). Extend the character response with academy display metadata, discipline labels, archetype/spell state, and charge summaries rather than creating parallel `/wizard/*` copies of existing progression APIs. A future `GET /api/theme` is optional and may return a static manifest; it is not a progression endpoint.

---

## 8. Frontend — game-like UI

### 8.1 Aesthetic

**Lantern Academy / original wizard-school** game-like feel:

- Student dossier with academy level, mastery bar, active archetype, modifiers, and spellbook.
- Assignment board and enchanted watch lists, with sealed next lessons for mystery orders.
- Certificate wall / awards shelf for visible achievements, newly unlocked honors, and per-genre completion awards.
- **Discipline map** (see §8.2), using original magical study motifs rather than a literal fantasy world map.
- Viewing-journal cards with parchment, ink, constellation, botanical, and geometric-sigil accents.
- Original student portraits and asset keys only; no franchise-derived character art or film stills.

### 8.2 The map — genre/watching map

**Decided:** A discipline map represents the student's watching coverage across the stable neutral genres.

| Neutral genre slug | Academy display name | Visual motif |
|---|---|---|
| `horror` | Shadowcraft | lantern-black ink, moonlit sigils |
| `thriller` | Tension Weaving | taut red-gold threads |
| `mystery` | Pattern Reading | constellations and clue-like geometry |
| `sci-fi` | Far-Realm Studies | star charts and brass instruments |
| `fantasy` | Mythic Arts | botanical forms and luminous ink |
| `documentary` | World Lore | field notes and specimen diagrams |
| `comedy` | Gleeful Charms | bright stained-glass marks |
| `drama` | Human Studies | layered portrait silhouettes |
| `romance` | Heartwork | paired orbit motifs |
| `animation` | Living Illustration | kinetic color blocks |

The API keeps the neutral slug and source genre data as the identifier; the academy display name and motif are presentation metadata. Watching content in a genre "lights up" its discipline. Coverage level (watches, variety, and owned sub-genres) determines how explored it looks. Locked disciplines appear sealed and show their level gate plus sub-genre-XP requirement, but never imply that the content is inaccessible in Plex.

**V1 scope:** a visual map component showing discipline coverage, access state, and the next purchase path. It is not a literal navigable world map and has no movement, combat, or spatial gameplay. A more literal academy/world map is V2.

### 8.3 Views / pages (V1)

- **Student dossier** — stats, academy rank, level, XP, active archetype, spells, discipline access, and streaks.
- **Assignment board** — available coursework (new arrivals + unwatched library + featured), taken/active assignments, and completed work. The student chooses assignments here.
- **Viewing journal** — history of awarded watches (content, when, points, and via-Plex flag).
- **Certificate wall / awards shelf** — visible honors, recently unlocked hidden awards, and per-discipline completion shelves.
- **Discipline map** — the coverage map and sealed/unlocked study paths.
- **Settings / sync status** — lightweight; sync state, last poll, maybe manual refresh. Wizard settings show active-loadout switch eligibility, spell balances, and the neutral-versus-presentation boundary; they do not expose database terminology as the primary UI.

**UI state and accessibility contract:** locked disciplines, archetypes, and spells remain visible with their requirement and a plain-language reason; unavailable actions are disabled rather than simulated. Every modifier has both a short label and an expanded explanation, every cast/selection reports success or a non-consuming failure, and sealed order items retain the §5.7 redaction rule. The first UI can use responsive cards and keyboard-accessible controls; portraits and decorative effects are optional and must never be required to understand level, XP, eligibility, or ledger outcomes.

### 8.4 Frontend tech (resolved §12 Q2)

**Frontend stack (resolved):** **Svelte** (likely SvelteKit, or Svelte + Vite for a SPA). The spec does not otherwise mandate React vs server-rendered HTML — Svelte is the chosen direction. The existing backend is Rust/Axum; the Svelte frontend can be served as static assets by the Axum backend, or as a separate dev server during development. Exact build setup (SvelteKit adapter, Vite config, asset serving from Axum) is an implementation detail.

The UI remains **game-like** (student dossier, assignment board, discipline map, spellbook, and awards shelf) regardless of the Svelte-flavored implementation.

---

## 9. Polling & real-time

### 9.1 Polling model (decided)

- The RPG backend **polls** Plex/Sonarr/Radarr on a timer.
- **Poll interval (resolved §12 Q3):** **5 minutes** between full poll cycles. On each poll:
  - Sync content metadata incrementally from Plex/Sonarr/Radarr into the RPG's Postgres content table.
  - Check Plex watch-state changes and award points for newly completed watches.
  - Check Sonarr/Radarr import history for new arrivals → generate case cards as appropriate.
- **Game tick phase order (finalized 2026-09-08):** each poll (and each UI refresh) runs the game state forward as **one `GameTick` with ordered phases**, in one place — not scattered hooks: **(1) watch award** — detect newly completed (≥95%) watches from Plex watch-state and write `watches` rows + XP/streak updates; **(2) order reveals** — `refresh_watch_orders` (§5.7): stamp reveals, complete finished cycles, grant skips, generate next cycles; **(3) achievement evaluation** — `evaluate_achievements` (§6.4.8) reads the post-award, post-reveal state so unlocks reflect the same pass. Later phases (case generation from imports, featured cases) append after these. `POST /api/orders/refresh` is the tick's UI entry point; skipping a skip-ahead item also advances phases 2–3 for its order.
- **Phase 1 detection semantics (finalized 2026-09-08):** the tick reads the Plex library per section (`type` movie/show) and derives a completion per item: `viewCount ≥ 1` → completed at 100%; otherwise `viewOffset / duration ≥ 0.95` (the §12 Q4 threshold, configurable — V1 constant) → completed at that percentage; anything else is not a completion. A completion matches a content row by `(source = 'plex', source_id = ratingKey)`; unknown ratingKeys are ignored (sync owns the catalog). **Award-once rule:** each content item earns at most one `watches` row (V1: no re-watch credit) — the insert is `NOT EXISTS`-guarded and audited via `via_plex`. XP follows §5.1 (movie 20 / episode 10) as `normal_xp`; the streak advances per §5.1 (same-day detection is neutral, yesterday continues, anything older restarts at 1 — local calendar day via the host clock) and §5.1.1 milestone bonuses are paid exactly when `current_streak_days` first equals a milestone value (once per streak run by construction). `character_state` is updated in the same pass: xp (normal + milestone bonus), watch counts, streak fields, and **level re-evaluated from the §5.2 thresholds** (level-up during award, per §5.2).
- 5-minute polling is sufficient; near-real-time is not required. Exact timer implementation (tokio async timer, interval jitter, back-off on API errors) is an implementation detail.
- **Poll loop semantics (finalized 2026-09-08, implemented as `poll.rs`):** one loop owns unattended play. Each cycle runs, in order: **content sync** (`SyncPipeline::run` — mirror Plex/Sonarr/Radarr + provider enrichment into Postgres), then the **game tick** (`run_game_tick` with the stack clients — phase 1 detection, phase 2 reveals, phase 3 evaluation). Cadence is a fixed 5-minute `tokio::time::interval` from cycle start (jitter/back-off deliberately not implemented in V1 — the stack is small and rate limits are not a factor). **Failure policy:** any sync or phase-1 failure is logged and non-fatal — the cycle still advances phases 2–3 on known state (the tick's documented degradation), and the loop never exits due to poll failures. **Graceful shutdown:** Ctrl-C both drains the HTTP server and signals the loop to finish its current cycle and exit before the process returns — no cycle is abandoned mid-write. **Observability:** every cycle prints one log line — sync rows persisted + per-phase tick counts (watches awarded, orders created, skips granted, achievements unlocked) — so unattended play is auditable from logs alone.

### 9.2 Why polling, not push/webhooks

- Polling matches the existing stack's operational pattern (the stack's `stack-activity-feed` and `stack-arrival-notify` are timer-driven, not webhook listeners).
- No open port, no webhook config, no missed events when the backend is briefly down.
- Simpler and consistent with the stack's ethos.

### 9.3 Backfill / first run (resolved §12 Q12)

- **Library is brand new → backfill is not the starting scenario.** The spec does **not** assume a large existing library to backfill on first run. The first-run flow is therefore **faster, more frequent updates at first** rather than a big backfill blast.
- **Faster updates at first:** early poll cycles can run more frequently (or do a more complete incremental sync) until the content table is populated to a stable state, then settle into the normal 5-minute poll (§9.1). This is a "catch-up then settle" pattern: while the library is small / the content mirror is still filling in, sync faster; once caught up, use the standard poll cadence.
- **What "caught up" means (finalize during implementation):** the content mirror has current stack metadata, required provider IDs have been queued for enrichment, and the last poll found no new/changed items. Provider enrichment may continue asynchronously; the normal 5-minute cadence must not wait on external providers.
- **Existing watch state:** on first run the RPG can pull current Plex watched-state for the (small, new) library so the character doesn't start from zero if there's already watching history — but this is lightweight (Plex Movies = 4 items, Plex TV Shows = 8 shows per the 2026-09-08 probe), not a full historical backfill.

---

## 10. Deployment & access

### 10.1 Deployment model (decided)

- **Not a new container in `docker-compose.yml`.** The RPG is linked with the stack but not part of it.
- The RPG runs as a **separate process** on the stack host — either a systemd user service, a background process, or similar. It is the Rust backend (the new RPG crate) plus its serving of the frontend.
- It connects to the stack's services over the LAN (Plex `:32400`, Sonarr `:8989`, Radarr `:7878`) the same way existing stack scripts do.
- It connects to Postgres via a connection string; Postgres is available on the host network but is **not** a new Compose container.

### 10.2 Access (decided)

- **LAN only, on the stack host.** The RPG is accessed from devices on the LAN (the host's browser, or other LAN devices).
- No external exposure by default. Remote access (if ever wanted) is a future addition (e.g., Tailscale), not V1.
- Port: **46532** (finalized 2026-09-08 — originally resolved as 86532, which exceeds the 16-bit TCP port limit of 65535 and could never bind; 46532 keeps a collision-free high port). Document the port clearly.

### 10.3 Configuration & secrets

- The RPG reuses the same `.env` secrets the stack uses: `PLEX_TOKEN`, `SONARR_API_KEY`, `RADARR_API_KEY`, `PLEX_URL`, `SONARR_URL`, `RADARR_URL`, `HOST_IP`, etc.
- **Additional RPG env:** `RPG_DB_URL` (Postgres connection string), `TMDB_API_KEY`, `TVDB_API_KEY`, `OMDB_API_KEY`, and `FANART_API_KEY`. `TVDB_API_KEY` is exchanged for a runtime bearer token; the token is not committed or persisted as a secret. RPG poll interval, cache TTLs, concurrency limits, and enrichment toggles are RPG-specific config.
- **`.env.template` update:** Add RPG-specific entries to `.env.template` (documented, not committed with real values).
- **No new secrets infrastructure** — reuse the existing `.env` + Docker secrets pattern the stack already has.

### 10.4 Lifecycle

- The RPG has its own lifecycle: start/stop on the host, independent of `docker compose up -d` / `down`.
- It does not restart when the stack restarts (unless you configure it to). It does not block stack operations.
- Its own healthcheck concept (TBD — could be a `/healthz`-style endpoint the backend serves).

---

## 11. Out of scope (V1)

- **Multi-player / shared quests** — V2.
- **Manual watch claims** — V2 (V1 trusts Plex; manual claims deferred).
- **Spending points / economy / inventory** — V1 is earn-and-progress only.
- **A literal navigable world map** — V1 is a genre/watching coverage map.
- **New container in the stack's Compose** — by design.
- **Modifying existing *arr/Plex services** to connect to Postgres — by design; they keep their SQLite.
- **Remote access** — LAN only for V1.
- **Push/webhooks** — polling only for V1.
- **Reverse proxy / Traefik** — none; matches the stack's no-reverse-proxy posture.

---

## 12. Open questions for implementation

**Status (2026-09-10): the original wizard-academy rebrand and backend-first release decisions below are resolved for the next implementation task.** The existing 14 infrastructure questions remain resolved. The wizard contract is documented in §§2.1–2.3, 5.8, 6.4.14, 7.4, and 8.1–8.3; no wizard code, migration, endpoint, or asset implementation is implied by this documentation pass.

15. **Wizard setting — RESOLVED:** use the original **Lantern Academy** setting and broad wizard-school visual language. Do not use Harry Potter names, characters, logos, houses, spells, film artwork, actor likenesses, or imitation of its film art. Use original academy vocabulary and original/licensed assets only. The product tone is cozy, scholarly, curious, and lightly mysterious. (See §§2.1–2.2, 8.1.) ✔
16. **Progression ownership — RESOLVED:** one existing V1 `character` and neutral ledgers remain authoritative. Archetypes are loadouts, not alternate saves; spells are audited charges, not a second XP/economy ledger. (See §5.8.) ✔
17. **Archetype roster and build tradeoffs — RESOLVED:** seed the six original archetypes in §5.8.2. Unlocks are permanent and fact-based; each archetype has one primary mechanic and at most one secondary modifier, with percentage effects bounded to ±10%. One archetype is active; its selection is immutable once queued and applies at the next game tick. (See §5.8.1–§5.8.3.) ✔
18. **Spell affinity — RESOLVED:** each spell has one player-selected parent discipline. First setup and later affinity changes are queued, immutable, limited to once per local day per spell, and applied at the next game tick. Only newly awarded watches advance the selected spell's meter. (See §5.8.4.) ✔
19. **Spell economy — RESOLVED:** each spell advances by 5 affinity points per episode or 10 per movie matching its selected discipline, grants one charge at 100 points, carries remainders, caps at 3 unspent charges, and records overflow as an auditable no-op. Milestones unlock definitions but do not mint charges. (See §5.8.4–§5.8.6.) ✔
20. **Switching and modifier precedence — RESOLVED:** pending archetype and affinity changes apply before awards at the next tick; neutral base first, active archetype effects second, targeted spell effect third, contextual bonuses thereafter. Same-component percentages add and clamp to 50%–200%; historical rows never change. (See §5.8.1 and §5.8.3–§5.8.4.) ✔
21. **Power boundary and presentation — RESOLVED:** spells may reveal, protect, prioritize, or resolve only eligible non-final order items; they never fabricate watches, normal XP, final completion, or achievement truth. The guided UI shows current/pending state, requirements, meters, tradeoffs, and machine-readable outcomes while stable neutral slugs remain authoritative. (See §§5.7–5.8, 7.4, 8.1–8.3.) ✔
22. **Implementation seam and order — RESOLVED:** the backend-first vertical release implements migrations 0012/0013, bootstrap/backfill, pure archetype/affinity calculations, tick-boundary application, transactional casts, and integration proofs before the Svelte UI. The minimal API is character extension plus archetype list/select, spell list/affinity/cast; no parallel progression APIs. (See §6.4.14 and §7.4.) ✔

1. **Postgres provisioning — RESOLVED (host install, NOT yet available — needs provisioning):** host-side Postgres install, available on host network, not a new compose container. **Probe result (2026-09-08):** no Postgres currently running on the host — no `pg_isready`/`psql` on PATH, no systemd `postgresql` service, nothing on port 5432. The RPG backend cannot connect to Postgres until one is installed/provisioned on the host. **Concrete provisioning decision (finalize before build):** plan + run a host-side Postgres install (the spec's default assumption is a packaged install via the host's package manager + systemd service + a dedicated `rpg` db + a dedicated db user, with `RPG_DB_URL` = `postgresql://<user>:<pass>@localhost:5432/rpg`). Provisioning is a pre-build step, not part of the RPG backend code itself. (See §6.3, §6.4.10.) ✔
2. **Frontend tech — RESOLVED (Svelte):** Svelte (likely SvelteKit or Svelte+Vite SPA), served as static assets by the Axum backend (or separate dev server during dev). (See §8.4.) ✔
3. **Poll interval — RESOLVED (5 minutes):** 5-minute poll cycle. (See §9.1.) ✔
4. **Near-end threshold — RESOLVED (95%, configurable):** watch counts as completed at ≥95% viewed; configurable default. (See §5.1.) ✔
5. **XP numbers & level thresholds — RESOLVED (concrete V1 values):** episode XP = 10, movie XP = 20, season bonus = 10 × episode count, series bonus = 25 × total episode count, first-completion +10, new-arrival +5 (48h window), featured +10, day-streak bonus table (§5.1.1), genre variety bonus +5/+15 (§5.1.2), level table 1→10 with cumulative XP thresholds (§5.2). (See §5.1, §5.2.) ✔
6. **Genre unlock thresholds and enrichment — RESOLVED:** Horror opens at level 1; other genres use the fixed cascade, level gate, and 100 sub-genre XP purchase threshold. TMDb is the canonical movie/TV genre and keyword provider; TVDB supplies TV tags/genres when available; Plex tags remain the fallback. Provider IDs, mappings, cache TTLs, and stale-data behavior are defined in §4.6 and §6.4.4a. ✔
7. **Featured case selection logic — RESOLVED (new or all-time ranking by provider score):** featured cases are selected as either **(a) new arrivals** or **(b) all-time ranking**. Candidates are restricted to accessed genres and ranked by normalized provider score in this order: TMDb vote average, OMDb IMDb rating, TVDB rating, then stack ratings. Provider scores and the selected mode are retained for audit. ✔
8. **Achievement list — RESOLVED (massive list, finalize the concrete items during implementation):** a **large/ extensive achievement list** is wanted. Categories are decided (§5.5): completion milestones, genre coverage, time/streak, novelty, themed/quirky (incl. hidden). The spec does **not** enumerate every achievement now — that's a "massive list" to be written as part of implementation (with visible + hidden split). The spec commits to "many achievements across the categories; finalize the list during implementation." ✔
9. **RPG backend port — RESOLVED (46532, corrected 2026-09-08):** the RPG backend binds to host port **46532** on the stack host (LAN-only access, §10.2). Not conflicting with the stack's existing ports (3000, 5055, 7878, 8989, 9696, 32400). **Correction:** the originally resolved 86532 is impossible — TCP ports are 16-bit (max 65535) — and was replaced during implementation with 46532. ✔
10. **Auth on the RPG frontend — RESOLVED (set-a-pin gate):** PIN-based gate (§7.3), PIN stored in Postgres, V1 single-user. No full username/password account system for V1. ✔
11. **Shared modules between the two backend crates — RESOLVED (common libs fine):** both crates may share a common `backend/` lib (§7.2) for non-coupling shared bits (`.env`/config, HTTP helpers, auth/secret patterns). No shared RPG state in the common lib. ✔
12. **Backfill scope & speed on first run — RESOLVED (no big backfill assumed; faster updates at first, settle to 5-min cadence once caught up):** the library is brand new, so first-run is "faster, more frequent updates at first" rather than a large backfill blast; settle to the normal 5-minute poll once the content mirror is caught up. **Probe result (2026-09-08):** confirms the assumption — Plex Movies = 4 items, Plex TV Shows = 8 shows (24 series in Sonarr). Tiny/new library. (See §9.3.) ✔
13. **Stack endpoints and metadata mirror — RESOLVED:** mirror the complete Plex/Sonarr/Radarr payloads described in §4, then enqueue enrichment by TMDb/TVDB/OMDb/Fanart.tv using stable TMDb, TVDB, and IMDb IDs. Plex is authoritative for watch state; Sonarr/Radarr for imports and file state; providers enrich metadata only. ✔
14. **Provider-enriched sub-genres — RESOLVED:** use TMDb official genre IDs as parent genres and mapped TMDb keywords as sub-genre candidates; TVDB tags/genres may supplement TV content. Cache raw responses and normalized provenance. OMDb and Fanart.tv are used for ratings/identity and artwork respectively, not genre access. ✔

---

## 6.4 Concrete Postgres schema (V1)

**Convention:** table names are snake_case, plural. All tables have `id` (bigserial PK) unless noted. Timestamps are `timestamptz` (UTC). The schema is written for the **single-character V1** (one character row; the character_id foreign keys resolve to that one character). Multi-character support is a future extension; the schema is written so adding more characters is additive (character_id on the relevant tables), not a rewrite.

### 6.4.1 Accounts / auth (PIN gate, §7.3)

```sql
CREATE TABLE accounts (
  id          bigserial PRIMARY KEY,
  pin_hash    text NOT NULL,        -- full PHC string from the argon2 crate (Argon2id; embeds salt + params)
  pin_salts   text NOT NULL,        -- raw per-account salt (base64) for explicit access / future rotation
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE characters (
  id          bigserial PRIMARY KEY,
  account_id  bigint NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name        text NOT NULL DEFAULT 'The Investigator',  -- legacy neutral fallback; display name is changeable
  created_at  timestamptz NOT NULL DEFAULT now()
);
```

- **PIN hashing (finalized 2026-09-08):** the **`argon2` crate** (RustCrypto; pure Rust, matching the crate's no-OpenSSL/rustls posture) with the **Argon2id** variant, in **PHC string format**. `pin_hash` stores the complete PHC string (`$argon2id$v=19$m=…,t=…,p=…$<salt>$<hash>`), so the salt and cost parameters travel with the hash and verification re-derives from the string's embedded params. `pin_salts` stores the base64 salt separately for explicit access and future rotation/rehash flows. Rehashing on successful login if parameters are upgraded is allowed but optional (V1: single account, set once). Version pin: `argon2 = "0.5"` (with `password-hash` PHC support, its default).
- **PIN set / verify flows (finalized 2026-09-08):** **set-PIN** (first run, exactly once): the PIN must be **4–12 digits** (digits only); it is hashed with Argon2id and the single account row is inserted, then character-creation bootstrap (§6.4.11) seeds `character_state`, the horror `genre_access` row, and missing `settings` defaults. If an account already exists, set-PIN is rejected (no PIN change flow in V1). **Verify** (each gate entry): load the single account's `pin_hash` (a missing account = **locked**, nothing to verify), re-derive with Argon2id from the PHC string's embedded params, and compare; a wrong PIN is rejected without side effects. Verify never reports a wrong PIN as an error; a malformed `pin_hash` is an operational error, not a login failure. (Session issuance after successful verify — cookie vs token, lifetime — remains an implementation detail.)
- V1 = one account, one character. The PIN is set once at first run / first login (PIN set flow is an implementation detail; the spec commits to "PIN stored hashed in Postgres, single character behind the PIN").
- `characters.name` is changeable (a simple student-name feature). Existing rows and the legacy database default remain untouched by the documentation-only rebrand; the academy presentation supplies the starter archetype and rank independently of this free-form name.

### 6.4.2 Character state (XP, level, stats)

```sql
CREATE TABLE character_state (
  character_id bigint PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  xp           bigint NOT NULL DEFAULT 0,
  level        int NOT NULL DEFAULT 1,
  total_watches int NOT NULL DEFAULT 0,     -- total completed watches (episode + movie)
  episode_watches int NOT NULL DEFAULT 0,
  movie_watches  int NOT NULL DEFAULT 0,
  current_streak_days int NOT NULL DEFAULT 0,
  best_streak_days  int NOT NULL DEFAULT 0,
  streak_last_watch_date date,             -- last calendar date (local) with a completion; NULL if none
  genres_accessed int NOT NULL DEFAULT 1,  -- count of genres accessed (starts at 1 = horror)
  last_level_up_at timestamptz
);
```

- `character_state` is a **singleton per character** (one row, `character_id` PK). Updates are atomic increments.
- `xp` is total cumulative XP. `level` is derived from `xp` against the §5.2 level table — but stored explicitly so the UI can read it without recomputing every time; the RPG logic recomputes level from XP on every XP change and updates `level` if it changed.
- `current_streak_days` / `streak_last_watch_date` implement the §5.1 day-streak: on each new completion, if `streak_last_watch_date` == yesterday (local date) → increment streak; if == today → no change; otherwise reset to 1 and set last date to today. (Local date uses the host TZ from `.env`.) `best_streak_days` is the max ever.
- `genres_accessed` counts how many genres the character has access to (starts at 1 = horror). Used by the UI and by the level-broadens-access rule (§5.2): at level N, max genres accessed can be up to N; the character can't access more than N genres until leveling up. (The actual per-genre access state is in `genre_access` below.)

### 6.4.3 Genre access & sub-genre XP

```sql
CREATE TABLE genres (
  id          bigserial PRIMARY KEY,
  name        text NOT NULL UNIQUE,       -- e.g. 'Horror', 'Sci-Fi', 'Documentary'
  list_order  int NOT NULL,               -- position in the fixed genre list order (§5.2); horror = 1
  is_opening  boolean NOT NULL DEFAULT false  -- true for horror (the opening unlocked genre)
);

CREATE TABLE sub_genres (
  id          bigserial PRIMARY KEY,
  genre_id    bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  name        text NOT NULL,              -- e.g. 'Slasher', 'Supernatural', 'Space Opera'
  UNIQUE (genre_id, name)
);

CREATE TABLE genre_access (
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  genre_id     bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  accessed_at  timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (character_id, genre_id)
);

CREATE TABLE sub_genre_xp (
  character_id  bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  sub_genre_id  bigint NOT NULL REFERENCES sub_genres(id) ON DELETE CASCADE,
  xp            bigint NOT NULL DEFAULT 0,   -- accumulated sub-genre XP toward purchase
  purchased     boolean NOT NULL DEFAULT false,
  purchased_at  timestamptz,
  PRIMARY KEY (character_id, sub_genre_id)
);

CREATE TABLE genre_xp_ledger (
  id           bigserial PRIMARY KEY,
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  sub_genre_id bigint NOT NULL REFERENCES sub_genres(id) ON DELETE CASCADE,
  xp_added     bigint NOT NULL,
  source_watch_id bigint REFERENCES watches(id),  -- the watch that generated this sub-genre XP (if any)
  at           timestamptz NOT NULL DEFAULT now()
);
```

- `genres.list_order` = the fixed genre list order from §5.2 (horror first = list_order 1, then the next genre at list_order 2, etc.). The cascade "next available genre to buy" = the lowest-list_order genre the character hasn't accessed yet, among genres with list_order > the character's currently-accessed-count. (Genre list contents + order **finalized 2026-09-08** — the 10-genre §5.2 list, seeded by migration 0004.)
- `genre_access` = which genres the character has accessed (full XP). Horror is pre-populated for the new character (one row, horror genre_id, accessed_at = character creation). Other genres get a row when purchased/accessed.
- `sub_genre_xp` = per-sub-genre accumulation bucket. `xp` accumulates as the character completes watches in that sub-genre (§5.2: +10 per episode, +20 per movie, toward that sub-genre). `purchased` flag + `purchased_at` when the player spends the XP to buy the sub-genre (purchase is a separate action — the player clicks "buy" when xp >= 100; the backend spends the xp, sets purchased=true, and adds a genre_access row for the parent genre if not already accessed).
- `genre_xp_ledger` = an audit trail of sub-genre XP additions (so the UI can show "you earned X sub-genre XP from this watch"). `source_watch_id` links to the watch that generated it (if applicable).
- **Migration placement (2026-09-08, updated):** `genre_xp_ledger` is created in the **watches migration (0007)**, not with the other genre tables (0004) — its `source_watch_id` foreign key targets `watches(id)`, the same deferred-prerequisite pattern used for `sync_state` (0002 → 0003).

### 6.4.4 Content mirror (read-only from stack)

```sql
CREATE TABLE content (
  id                bigserial PRIMARY KEY,
  source            text NOT NULL,                 -- 'plex' | 'sonarr' | 'radarr'
  source_id         text NOT NULL,                 -- the ID the source uses (Plex ratingKey, Sonarr seriesId, Radarr movieId)
  external_id       text,                          -- TMDb ID / TVDb ID when available (the stable cross-source key)
  external_id_type  text,                         -- 'tmdb' | 'tvdb' | null
  title             text NOT NULL,
  year              int,
  content_type     text NOT NULL,                 -- 'movie' | 'series' | 'episode'
  parent_id         bigint,                       -- for episodes: points to the series content row (self-ref via content.id)
  season_number     int,
  episode_number    int,
  runtime_seconds   int,
  release_date      date,
  first_air_date    date,
  status            text,                         -- series status: 'continuing' | 'ended' | etc. (where available)
  summary           text,
  rating           numeric,                       -- source rating (MPAA/score/whatever the source exposes)
  rating_source     text,                         -- which source the rating came from
  poster_url        text,
  fanart_url        text,
  section_key       text,                         -- Plex library section key (e.g. '/library/sections/<key>')
  section_title     text,                         -- Plex library section title (e.g. 'Movies', 'Shows')
  genres            jsonb NOT NULL DEFAULT '[]',  -- list of genre names from the source
  sub_genres        jsonb NOT NULL DEFAULT '[]', -- enriched tags from TMDb keywords / TVDB tags, mapped to parent genres
  metadata_blob     jsonb NOT NULL DEFAULT '{}', -- full raw stack metadata mirror: cast, directors, writers, studio, mpaa, network, file state, MediaInfo, credits, extras, etc.
  provider_metadata jsonb NOT NULL DEFAULT '{}', -- normalized provider values + provenance for TMDb/TVDB/OMDb/Fanart.tv
  last_synced_at    timestamptz NOT NULL DEFAULT now(),
  last_enriched_at  timestamptz,
  UNIQUE (source, source_id)
);

CREATE INDEX content_external_id ON content(external_id) WHERE external_id IS NOT NULL;
CREATE INDEX content_type_type ON content(content_type);
CREATE INDEX content_parent ON content(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX content_genres_gin ON content USING GIN (genres jsonb_path_ops);
CREATE INDEX content_sub_genres_gin ON content USING GIN (sub_genres jsonb_path_ops);
CREATE INDEX content_section ON content(section_key);
CREATE INDEX content_provider_metadata_gin ON content USING GIN (provider_metadata jsonb_path_ops);
```

### 6.4.4a External provider cache

```sql
CREATE TABLE content_provider_cache (
  id           bigserial PRIMARY KEY,
  content_id   bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  provider     text NOT NULL,                 -- 'tmdb' | 'tvdb' | 'omdb' | 'fanart'
  provider_id  text NOT NULL,                 -- TMDb ID, TVDB ID, IMDb ID, or provider lookup key
  payload      jsonb NOT NULL,
  fetched_at   timestamptz NOT NULL DEFAULT now(),
  expires_at   timestamptz,
  http_status  int,
  error        text,
  UNIQUE (content_id, provider, provider_id)
);

CREATE INDEX content_provider_cache_lookup ON content_provider_cache(provider, provider_id);
```

The cache preserves the complete successful response and the latest failure metadata for each provider. `content.provider_metadata` contains the normalized values used by gameplay; the cache is the auditable raw source.

- `content` is the **full metadata mirror** (§4.5: mirror everything). Core searchable fields are columns (`title`, `year`, `content_type`, `external_id`, `genres`, `sub_genres`, `section_key`, `rating`, etc.); **everything else** the source exposes goes into `metadata_blob` (jsonb) so the mirror is complete without a schema per source field. This matches "mirror everything" + "full picture, no trimming for V1".
- `source` + `source_id` is the unique key per source. `external_id` (TMDb/TVDb) is the stable cross-source key used to de-duplicate / match across Plex/Sonarr/Radarr when the same title appears in multiple sources.
- For episodes: `content_type = 'episode'`, `parent_id` points to the series' content row (content_type = 'series'), `season_number` + `episode_number` identify the episode.
- `genres`/`sub_genres` are jsonb arrays of name strings (from the source's genre/sub-genre exposure). The GIN indexes let the case board / map / suggested-title lists filter by genre/sub-genre efficiently.
- `metadata_blob` stores the full extra metadata (cast, directors, writers, summary, full ratings, fanart, episode file state, MediaInfo, credits, extras, etc.) — complete mirror, §4.5. The RPG can read from it for UI richness and for achievement triggers that need it (e.g. director-based achievements, franchise-based achievements).
- `last_synced_at` is updated on each poll's incremental sync for the rows that changed.

### 6.4.5 Watch records (the RPG's own awarded watches)

```sql
CREATE TABLE watches (
  id                    bigserial PRIMARY KEY,
  character_id           bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  content_id             bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  content_type          text NOT NULL,                 -- 'movie' | 'episode' (the unit that was completed)
  completed_at           timestamptz NOT NULL DEFAULT now(),  -- when the RPG detected/awarded the completion
  pct_viewed             numeric NOT NULL,              -- the Plex-reported % viewed at detection time (for audit / threshold confirmation)
  xp_awarded             bigint NOT NULL,               -- total XP awarded for this completion (normal + bonuses)
  normal_xp              bigint NOT NULL,               -- the base XP (episode=10 / movie=20)
  bonuses                jsonb NOT NULL DEFAULT '[]',  -- list of bonus objects applied: {name, xp} e.g. {name:'new_arrival', xp:5}, {name:'featured', xp:10}, {name:'season', xp:100}, {name:'series', xp:500}, {name:'streak', xp:50}, {name:'variety', xp:5}, {name:'holiday_halloween', xp:15}
  new_arrival            boolean NOT NULL DEFAULT false,
  new_arrival_at         timestamptz,                  -- the title's arrival timestamp (from Sonarr/Radarr import), when set
  featured               boolean NOT NULL DEFAULT false,
  featured_case_id       bigint REFERENCES featured_cases(id),
  season_bonus           boolean NOT NULL DEFAULT false,
  series_bonus           boolean NOT NULL DEFAULT false,
  first_completion       boolean NOT NULL DEFAULT false,
  holiday_bonus          jsonb,                        -- which holiday window(s) applied, if any: {window:'halloween', multiplier:1.5}
  via_plex               boolean NOT NULL DEFAULT true,
  via_manual             boolean NOT NULL DEFAULT false,  -- V2 manual-claim flag; V1 always via_plex=true
  created_at            timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX watches_character ON watches(character_id);
CREATE INDEX watches_completed_at ON watches(character_id, completed_at DESC);
CREATE INDEX watches_content ON watches(content_id);
```

- `watches` is the **RPG's own record** of awarded completions (not the stack's watch state — that stays in Plex). Each row = one completion the RPG awarded points for.
- **Migration placement (2026-09-08):** `watches.featured_case_id` references `featured_cases(id)`, a **later** migration (0009) — the FK is deferred there, same pattern as `sync_state` (0002 → 0003) and `genre_xp_ledger` (0004 → 0007). The column itself is created now so the table is complete from birth.
- `xp_awarded` = total XP for the row (normal + all bonuses). `normal_xp` = the base (10/20). `bonuses` jsonb lists each bonus applied with its name + XP, so the watch log / character sheet can show "this completion earned +10 normal +5 new-arrival +10 featured = +25 XP".
- `new_arrival` / `new_arrival_at` / `featured` / `featured_case_id` / `season_bonus` / `series_bonus` / `first_completion` / `holiday_bonus` / `via_plex` / `via_manual` are the audit flags for achievements (e.g. "first new-arrival bonus", "featured streak", "perfect day", "via-plex vs manual").
- `pct_viewed` is stored for audit/threshold confirmation (≥95% at detection time).

### 6.4.6 Cases (player-driven case board)

```sql
CREATE TABLE cases (
  id              bigserial PRIMARY KEY,
  character_id    bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  content_id      bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  case_type       text NOT NULL,                 -- 'movie_case' | 'series_campaign' | 'featured'
  status          text NOT NULL DEFAULT 'available',  -- 'available' | 'taken' | 'in_progress' | 'completed'
  taken_at        timestamptz,
  completed_at    timestamptz,
  completion_watch_id bigint REFERENCES watches(id),  -- the watch row that completed this case (for movie_case: the movie watch; for series_campaign: the watch that completed the final episode)
  bonus_flags     jsonb NOT NULL DEFAULT '[]',  -- which bonuses applied to this case's completion
  created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX cases_character ON cases(character_id);
CREATE INDEX cases_status ON cases(character_id, status);
```

- `cases` = the **assignment board**. `case_type` remains the stable neutral value: movie_case (one-off, content_type='movie'), series_campaign (multi-episode, content_type='series' — the assignment completes when all episodes are watched), featured (a featured assignment, linked to featured_cases).
- `status`: available (on the board, not yet taken), taken (player chose it), in_progress (at least one episode/movie watched but not completed), completed (done). For movie_case, taken→completed on the movie watch. For series_campaign, taken→in_progress on first episode watch, in_progress→completed on the episode that completes the series.
- `completion_watch_id` links to the watch row that completed the case (for audit + achievement triggers like "one-click wonder", "instant case").
- Cases are **player-driven**: the player takes a case from available; the backend doesn't auto-assign. Case generation (which content becomes a case card) is §5.4 / §9.1.
- **Implementation note (2026-09-08):** `cases.featured_case_id` links a `featured`-type case to its `featured_cases` row. The spec DDL above omits the column; migration 0008 creates it as a plain `bigint` with the FK deferred to 0009 (same deferred-prerequisite pattern as `watches.featured_case_id`).

### 6.4.7 Featured cases (periodic)

```sql
CREATE TABLE featured_cases (
  id          bigserial PRIMARY KEY,
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  period      text NOT NULL,                     -- e.g. '2026-wk37' or '2026-10' — identifies the period
  content_id  bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  selection_mode text NOT NULL,                  -- 'new_arrival' | 'all_time_ranking'
  bonus_xp    bigint NOT NULL DEFAULT 10,       -- featured bonus XP (§5.1: +10)
  created_at  timestamptz NOT NULL DEFAULT now(),
  UNIQUE (character_id, period)
);
```

- `featured_cases` = one featured case per period per character (the featured case for that period). `period` identifies the period — **finalized (2026-09-08, during implementation):** V1 cadence is weekly (§5.4), so `period` holds the ISO week label `YYYY-Www` (e.g. `2026-W37`); monthly would be `YYYY-MM` if the cadence ever changes. The old period's featured case becomes historical; a new one is generated each period.
- `selection_mode` = how it was chosen (new_arrival vs all_time_ranking, §5.4/Q7). `content_id` = the featured title. `bonus_xp` = the featured bonus (§5.1: +10 XP on completion).
- Featured case generation runs during the poll/sync (§9.1): pick the featured title per the selection rule, insert a new featured_cases row for the new period.

### 6.4.8 Achievements

```sql
CREATE TABLE achievements (
  id           bigserial PRIMARY KEY,
  slug         text NOT NULL UNIQUE,             -- e.g. 'first_blood', 'horror_native'
  name         text NOT NULL,
  description  text NOT NULL,
  category     text NOT NULL,                    -- 'completion_milestone' | 'genre_coverage' | 'time_streak' | 'novelty_firsts' | 'themed_quirky' | 'combo'
  visible      boolean NOT NULL DEFAULT true,   -- visible vs hidden (§5.5)
  kind         text NOT NULL,                     -- 'once' | 'progress' | 'streak' | 'counter' | 'combo' -- achievement evaluation kind
  -- progress/counter kinds store their target + current in achievement_state; once-kinds are evaluated purely from watches/content state
  target_value bigint,                           -- for counter/progress kinds: the target count
  metadata     jsonb NOT NULL DEFAULT '{}',     -- achievement-specific evaluation metadata (e.g. required genre, required sub-genre, date window, day-of-week, hour range, franchise filter, director filter, etc.) — finalize per-achievement during implementation
  created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE character_achievements (
  character_id   bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  achievement_id bigint NOT NULL REFERENCES achievements(id) ON DELETE CASCADE,
  unlocked_at    timestamptz NOT NULL DEFAULT now(),
  progress       bigint NOT NULL DEFAULT 0,     -- current progress for progress/counter kinds (e.g. 5/10)
  PRIMARY KEY (character_id, achievement_id)
);

CREATE INDEX achievements_category ON achievements(category);
CREATE INDEX achievements_visible ON achievements(visible);
```

- `achievements` = the achievement definitions (the §5.5 list). `slug` is the stable internal id. `visible` = visible vs hidden. `kind` = how it's evaluated: `once` (fires when a condition is met, one-time), `progress`/`counter` (has a current + target, shows progress), `streak` (evaluated against the streak state), `combo` (multi-condition).
- `metadata` jsonb holds per-achievement evaluation parameters (e.g. a genre-coverage achievement's required genre count, a themed achievement's required genre + day-of-week + hour range, a holiday achievement's window, a director achievement's director id filter). Finalize each achievement's metadata during implementation — the list in §5.5 is the first cut.
- `character_achievements` = which character unlocked which achievement + when, plus current progress for progress/counter kinds. Visible achievements read progress from here for the badge wall.
- **Implementation notes (2026-09-08, migration 0010):** the §5.5 first-cut list is seeded verbatim as 101 rows. Tiered achievements sharing a display name get distinct slugs (`genre_explorer_3/5/8/10`, `horror_homeground_10/25`, `level_up_2/3/5/10`, …). Kind mapping: counted thresholds → `counter` + `target_value`, day-streaks → `streak` + `target_value`, single-fire conditions → `once`, multi-condition → `combo`. Finalized ambiguities: **Spooky Season** counts cumulatively across years; **Actor's Playground** and **Rainy Day** carry a `metadata_dependent` flag and stay unevaluated until their metadata is confirmed mirrored; **First Banked Day** / **First Fame Tick** are gated on their optional features being enabled.
- **Evaluation contract (finalized 2026-09-08):** evaluation is a **pure function** of an achievement definition plus a `ProgressSnapshot` (episode/movie watch totals, current streak, level from `character_state`; distinct genres, horror watches, distinct holiday-bonus windows, new-arrival titles from `watches`+`content`; purchases from `sub_genre_xp`; completed featured cases from `cases`). Kind dispatch: `counter`/`progress` compare the metadata-selected metric against `target_value` (at target → unlock, below → report progress); plain `streak` (metadata `{}`) compares `current_streak_days`; `once` checks its single evaluable condition; `combo` and metadata-qualified streaks, plus achievements whose inputs the V1 snapshot lacks (per-day breakdowns, arrival timestamps, series/season completion scopes), are **not evaluable in V1** and stay unchanged — not silently unlocked. `character_achievements` rows are written **only at unlock** (`progress` records the value at unlock); live progress for the badge wall is computed from the snapshot, not stored. Unlocks are idempotent (`ON CONFLICT DO NOTHING`).

### 6.4.9 Sync state (poll cursors)

```sql
CREATE TABLE sync_state (
  character_id  bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  source        text NOT NULL,                 -- 'plex' | 'sonarr' | 'radarr'
  last_sync_at  timestamptz NOT NULL DEFAULT now(),
  cursor        text,                          -- last-sync marker per source (e.g. last ratingKey synced, last import timestamp checked, etc. — finalize per source during implementation)
  PRIMARY KEY (character_id, source)
);
```

- `sync_state` = per-source last-sync markers for incremental sync (§4.5, §9.1). The poll uses these to only fetch what changed.

### 6.4.10 Settings / config (RPG config values, §5.1/§5.2 tunable)

```sql
CREATE TABLE settings (
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  key          text NOT NULL,
  value        text NOT NULL,                  -- stringified config value (numbers/booleans as text; parse in app)
  PRIMARY KEY (character_id, key)
);
```

- `settings` = the RPG's configurable values for the character/server (the LoGD-inspired module/settings surface, §15.4). V1 samples:
  - `near_end_threshold_pct` = '95' (§5.1)
  - `new_arrival_window_hours` = '48' (§5.1)
  - `poll_interval_seconds` = '300' (§9.1: 5 min)
  - `provider_cache_ttl_seconds` = '86400' (default enrichment freshness; provider-specific overrides may be added later)
  - `provider_max_concurrency` = '2' (bounded external-provider concurrency)
  - `horror_list_order` / genre list order — stored as a JSON array of genre names in order, e.g. key `genre_list_order`, value `'["Horror","Thriller","Mystery","Sci-Fi","Fantasy","Documentary","Comedy","Drama","Romance","Animation"]'` (§5.2 cascade)
  - `sub_genre_purchase_xp_threshold` = '100' (§5.2)
  - `holiday_windows` = JSON array of window objects: `[{"name":"halloween","start":"10-01","end":"10-31","genres":["Horror"],"multiplier":1.5}, {"name":"winter_holiday","start":"12-01","end":"12-31","genres":["Comedy","Drama"],"multiplier":1.5}]` (finalize genres per window during implementation)
  - `perks_unlocked` = legacy JSON setting retained for compatibility; it is disabled for the Lantern Academy release and must not be combined with the §5.8 archetype modifier system
  - `daily_budget_enabled` = 'false' (§15.2 #1 — off by default for V1; toggleable)
  - `daily_budget_actions` = '3' (if enabled)
  - `featured_selection_mode` = 'new_arrival' | 'all_time_ranking' | 'rotate' (§5.4/Q7) — **V1 default finalized 2026-09-08: `all_time_ranking`** (all-time ranking always yields candidates from unwatched accessed-genre content — important on the tiny probe-confirmed library, where new-arrival mode would dead-end between imports; new_arrival/rotate remain configurable).
  - `fame_enabled` = 'false' (§15.2 #4 — off by default for V1; toggleable)
- Settings are the **toggleable/tunable feature surface** modeled on LoGD's Superuser Grotto module manager + game settings (§15.4). V1 defaults above reflect the spec's current commitments (no daily budget, no fame, 95% threshold, 48h new-arrival window, 5-min poll, 100 sub-genre XP purchase, horror-first genre list).

### 6.4.11 Migration approach

- **Watch-order placement (2026-09-08):** `watch_orders`, `watch_order_items`, and `skip_grants` land in migration **0011** (§6.4.13) — they depend only on `characters`/`genres`/`content`/`watches` (0001–0007), so they could have been earlier; 0011 keeps them adjacent to the achievements work they feed.

- **Migration layer:** use SQLx migrations (or a lightweight migrations table) to version the schema. Each migration is a versioned SQL file applied in order on first run / upgrade.
- **Initial migration (V1 schema):** the tables above, in dependency order: `accounts` → `characters` → `genres` (seed horror + the genre list order) → `sub_genres` (seed from library mirror on first sync, or a small starter set) → `character_state` (one row, seeded at character creation with horror accessed, level 1, xp 0) → `genre_access` (seed horror row for the character) → `sub_genre_xp` (empty buckets, created per sub-genre on first watch in that sub-genre or pre-created from the library's sub-genre list) → `content` → `watches` → `cases` → `featured_cases` → `achievements` (seed the §5.5 achievement list) → `character_achievements` (empty) → `sync_state` (one row per source, seeded at first sync) → `settings` (seed V1 defaults).
- **Seed data (first run):**
  - `genres`: seed the fixed genre list order (horror first + the rest in order) — the finalized §5.2 list: Horror → Thriller → Mystery → Sci-Fi → Fantasy → Documentary → Comedy → Drama → Romance → Animation (migration 0004).
  - `achievements`: seed the §5.5 list (name, description, category, visible/hidden, kind, target_value where applicable, metadata where applicable).
  - `character_state`: one row at character creation, level 1, xp 0, horror accessed, streak 0.
  - `genre_access`: horror row for the character at creation.
  - `settings`: seed V1 defaults (§6.4.10).
- **Incremental sync:** the `content` table is populated/updated by the poll/sync (§9.1), not by a one-time backfill (the library is brand new — §9.3). `sub_genres` can be pre-seeded from the first content sync's sub-genre exposures, or created on demand as watches accumulate in new sub-genres.
- **Migration placement (2026-09-08):** `settings` is created in migration **0006** (right after `character_state`), not last — it has no dependency beyond `characters` and is required by the character-creation bootstrap seeding. `sync_state` similarly landed early as 0002. The §6.4.11 list order remains the logical dependency order, not the file numbering.
- **Character-creation bootstrap (finalized 2026-09-08):** character creation is performed by the **application** (not migrations), as one transaction, idempotently: ensure the account's single character exists, insert the `character_state` row (level 1, xp 0, streak 0, `genres_accessed` = 1), insert the `genre_access` horror row, and insert any **missing** `settings` V1 defaults (§6.4.10 samples verbatim, including the winter window scope = Comedy+Drama as a configurable default). With no account yet (PIN not set), bootstrap is a no-op; the PIN-set flow triggers it after account creation.
- **Schema evolution:** future migrations add columns/tables for V2 features (manual claims → add `via_manual` handling, shared quests → add multi-character + shared_cases tables, fame → add fame table + state). The wizard 0012/0013 seam is additive and does not replace neutral ledgers. The `metadata_blob` + `bonuses`/`holiday_bonus`/`metadata` jsonb columns already give room to add data without early schema churn.

### 6.4.13 Mystery watch orders (finalized 2026-09-08, §5.7)

```sql
CREATE TABLE watch_orders (
  id           bigserial PRIMARY KEY,
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  genre_id     bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  cycle_number int NOT NULL,                     -- 1, 2, 3… per (character, genre)
  status       text NOT NULL DEFAULT 'active',   -- 'active' | 'completed'
  created_at   timestamptz NOT NULL DEFAULT now(),
  completed_at timestamptz,
  UNIQUE (character_id, genre_id, cycle_number)
);

CREATE TABLE watch_order_items (
  id           bigserial PRIMARY KEY,
  order_id     bigint NOT NULL REFERENCES watch_orders(id) ON DELETE CASCADE,
  position     int NOT NULL,                     -- 1-based within the order
  content_id   bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  revealed_at  timestamptz NOT NULL DEFAULT now(),  -- item 1 at creation; others at reveal
  skipped_at   timestamptz,                      -- set when resolved via skip (no watch)
  UNIQUE (order_id, position)
);

-- Item resolution is DERIVED: an item is resolved when a `watches` row exists
-- for its content (the ≥95% award, §6.4.5) or skipped_at IS NOT NULL. No
-- stored completion column — one threshold, one ledger.

CREATE TABLE skip_grants (
  id              bigserial PRIMARY KEY,
  character_id    bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  source_order_id bigint NOT NULL REFERENCES watch_orders(id) ON DELETE CASCADE,
  earned_at       timestamptz NOT NULL DEFAULT now(),
  spent_at        timestamptz,
  spent_item_id   bigint REFERENCES watch_order_items(id) -- the current item skipped
);

CREATE INDEX watch_orders_character ON watch_orders(character_id);
CREATE INDEX watch_order_items_order ON watch_order_items(order_id, position);
CREATE INDEX skip_grants_character ON skip_grants(character_id) WHERE spent_at IS NULL;
CREATE INDEX watch_order_items_content ON watch_order_items(content_id);
```

- **Reveal derivation (the invariant):** item `n+1` is visible iff item `n` is resolved — `EXISTS (SELECT 1 FROM watches w JOIN watch_order_items i ON i.content_id = w.content_id WHERE i.order_id = … AND i.position = n) OR i_n.skipped_at IS NOT NULL`. The store's order-flow computes this; it must never trust a client or store a completion flag. An academy spell may add an explicit reveal audit, but it must not turn a reveal into a resolution.
- **Skip ledger semantics:** balance = unspent rows (`spent_at IS NULL`). Spending stamps `spent_at`/`spent_item_id` on one row (audit of what was spent where), never deletes. Grants are per completed order (V1: exactly 1); the grant's `source_order_id` links the reward to its earning order for the awards shelf.
- **Generation (V1, algorithmic):** at creation, pick N movies (V1: 5) matching the genre that the character has no awarded watch for, ranked by provider score per §5.4; ties broken by id for determinism. Item 1 revealed at creation. Re-generation for cycle n+1 happens at completion, inside the game tick.
- **Integration notes:** `cases` stays untouched (a case is one content card; an order is a sequence — overloading `case_type` would muddy §6.4.6 status semantics). `ProgressSnapshot` (§6.4.8 contract) gains `orders_completed`, `orders_completed_by_genre`, `skips_earned`, `skips_used` for the §5.7 achievement batch in category `watch_orders`.

### 6.4.14 Wizard rebrand storage seam (finalized 2026-09-10; planned migrations, not implemented)

The smallest future schema change is two additive migrations after 0011; no existing neutral ledger is renamed or rewritten. The order below is also the implementation order for the backend-first vertical release. 0012 owns durable current/pending loadout and affinity state; 0013 owns spell definitions and the append-only charge/cast audit.

**Migration 0012 — archetypes, pending loadouts, affinity, and presentation:**

- Create `wizard_archetypes` with stable `slug`, display name, description, `portrait_key`, one `primary_effect` JSONB value, optional `secondary_effect` JSONB value, unlock kind/target, and immutable `strengths`/`weaknesses` display metadata. Seed the six archetypes and the matrix in §5.8.2.
- Add `characters.active_archetype_id` as a nullable FK to `wizard_archetypes(id)`, plus nullable pending-selection fields (`pending_archetype_id`, `pending_archetype_requested_at`, `pending_archetype_event_key`, `archetype_selected_local_date`). The active choice belongs on `characters`, not `character_state`, because it is a loadout; the local-date field enforces one accepted selection per local day. Existing rows are backfilled to `lantern_scholar` during the migration-safe bootstrap. The pending request is immutable until the next tick and is cleared on successful application or recorded rejection.
- Create `character_archetypes` with `(character_id, archetype_id)` as the primary key, `unlocked_at`, and `unlock_source_event_key`; permanent unlocks are idempotent and are not inferred from client state.
- Create `spell_affinities` with `(character_id, spell_id)` as the primary key, `selected_genre_id`, `pending_genre_id`, `pending_requested_at`, `pending_event_key`, `affinity_progress_points`, `last_affinity_change_local_date`, and `updated_at`. This stores one selected discipline per spell and its meter; it is not a general XP ledger. Because `spells` is seeded in 0013, `spell_id` is created as a deferred-reference column in 0012 and receives its FK in 0013. Setup/change requests must reference an accessed genre and obey one accepted change per spell per local day; pending state is applied or rejected at the next tick.
- Create `wizard_presentations` only if compiled API metadata cannot provide the academy labels/art: `(entity_type, entity_slug, display_name, description, icon_key, art_key, metadata)`. It may cover genre labels and achievement award names without touching neutral slugs.

**Migration 0013 — spell definitions and append-only charge audit:**

- Create `spells` with stable `slug`, display name, description, `effect_type`, parameters, and `charge_cap = 3`; seed the five spells in §5.8.5. Definitions are visible before affinity setup, but a spell remains unusable until its own unlock fact in §5.8.6 is satisfied.
- Create `character_spell_ledger` as an append-only audit of affinity events, grants, overflow, and casts: `id`, `character_id`, `spell_id`, `entry_type` (`affinity`, `grant`, `overflow_noop`, or `cast`), `source`, `source_event_key`, `affinity_points`, `granted_at`, `spent_at`, `target_order_id`, `target_item_id`, `outcome`, and optional `grant_id` pointing to the consumed grant. A valid cast locks and stamps one eligible grant row and appends one cast row; history is never deleted. Enforce idempotency for grants and affinity events with unique `(character_id, spell_id, source_event_key, entry_type)` keys; `overflow_noop` records the dropped points but never creates debt. Cast rows record the active archetype, selected discipline, and target-state summary in metadata so later audits can explain the decision without reconstructing mutable state.
- Prefer `watch_order_spell_reveals(order_id, item_id, spell_ledger_id, revealed_at)` for `Unsealing Light` rather than giving `watch_order_items.revealed_at` a second meaning. `Vanishing Step` stamps the existing `skipped_at`; neither spell creates a `watches` row or resolves a finale.

**Required transactional sequence:** at each game tick, lock the character's pending loadout rows, apply and clear valid pending archetype/affinity selections, then award newly detected watches and calculate affinity. A selection or affinity request either commits with its audit event or has no effect. A cast validates ownership, charge balance, target legality, and current order state in one transaction; invalid casts consume nothing. The tick/event response is the source of truth for what was applied.

**Implementation order for the vertical release:** (1) apply 0012/0013 and backfill the starter archetype without mutating neutral history; (2) add pure archetype, affinity, cap, and overflow calculations with unit tests; (3) add transactional bootstrap, pending-selection application, watch-award integration, and unlock evaluation; (4) add archetype/affinity/spell read and mutation endpoints; (5) prove all six archetypes and all five spells against disposable Postgres, including next-tick boundaries, same-day rejection, affinity-only-on-new-watch, charge thresholds, cap overflow, invalid casts, and finale protection; (6) build the guided Svelte dossier and spellbook against those APIs. No UI or client-side calculation is a prerequisite for the backend release.

No migration is needed for the neutral XP, watch, streak, genre, achievement, or free-skip ledgers. If compiled presentation labels are sufficient, 0012 and 0013 are the complete schema addition; a static theme manifest can be served without a table.

### 6.4.12 Design notes / rationale

- **Single-character V1 with character_id throughout:** even though V1 is one character, the schema carries `character_id` on every state table. This is intentional — it lets V2 multi-character / shared quests land without a rewrite (just add more character rows). The V1 app always reads/writes the single character's rows.
- **`content.metadata_blob` + `watches.bonuses` + `achievements.metadata` + `settings.value` as jsonb:** the spec says "mirror everything" (§4.5) and "finalize during implementation" for many values. jsonb columns absorb that without a schema-per-field explosion now, and without losing queryability for the fields that matter (genres, sub_genres, external_id, content_type, etc. are real columns with indexes).
- **`watches.bonuses` as a list:** makes the viewing journal / student dossier renderable ("this completion earned +10 normal +5 new-arrival +10 featured") and makes achievement triggers auditable ("perfect day" checks bonuses for new_arrival + featured + holiday).
- **`cases` separate from `watches`:** cases are the player-facing board; watches are the RPG's awarded-completion ledger. They're linked by `completion_watch_id` so achievements like "instant case" / "one-click wonder" can be evaluated.
- **`sync_state` cursors:** finalize the exact cursor shape per source during implementation (e.g. Plex: last ratingKey synced; Sonarr: last import timestamp checked; Radarr: last import timestamp checked). The spec commits to incremental sync keyed by IDs + last-sync markers (§4.5), not to a specific cursor format.

---

## 13. Assumptions

- The Bear Cave stack is running and reachable on the LAN at the configured ports.
- Plex has a populated library (Movies + Shows) with watch-state data.
- Sonarr/Radarr have API keys and are reachable.
- The host runs Linux (the stack is Linux-only; the RPG sitting on the same host inherits that).
- PostgreSQL is or can be made available on the host network without adding a Compose container.
- The existing `backend/` Rust code is a starting point / reference for style and helpers; the RPG is a new crate, not a continuation of the stack-management routes.

---

## 14. Non-goals

- This is **not** a replacement for Plex, Sonarr, or Radarr.
- This is **not** a general media dashboard (the stack already has `stack-watchable` / `stack-unwatched` / `stack-recent` for that).
- This is **not** a tool that writes back to the stack or changes playback behavior.
- This is **not** a multiplayer game in V1.

---

---

## 15. Inspiration & reference: Legends of the Green Dragon (LoGD)

> **Why this section exists:** the user asked to mine LoGD for ideas, mods, and host-inspiration. LoGD is a PHP/MySQL browser RPG (remake/homage of Seth Able's Legend of the Red Dragon, a BBS door game). It is not a technical dependency of this RPG — this spec's stack is Rust/Axum + Postgres, not PHP/MySQL. LoGD is referenced here purely as a **design inspiration source** and as a model for how a host ships/modularizes features.

### 15.1 What LoGD is (summary)

- **Format:** text-based browser multiplayer RPG. Played by clicking links; no real-time action.
- **Loop:** a **game day** is the main cycle. Each game day grants a set of **forest fights** (action points / turns), plus refreshed buffs/stats; when you run out you start a **new day** (some servers have an explicit "New Day" link; days can also tick on a timer, e.g. 2 game days per real day in the classic server).
- **Progression:** 15 levels per rank, then slay the dragon, then next rank, repeat (new game+). Ranks: Farmboy/Farmgirl → Page → Squire → … → Gladiator etc.
- **Combat:** turn-based forest fights vs monsters; death costs on-hand gold + some XP (bank gold is safe). Healer's hut to recover.
- **Economy:** gold earned in fights, deposited in the bank for **daily interest**; gems are a scarce secondary currency found while exploring, spent on permanent upgrades (e.g. stallion, vitality) or temporary boosts (ale).
- **PvP:** optional; new players protected for first 5 game days / 1500 XP. Attacking a player yields a share of their XP + on-hand gold; losing costs XP + on-hand gold.
- **Fame:** there is a **fame rating / fame bar** concept — a visible reputation signal; the primer and the module list both reference fame-related mechanics (fame gain, fame display, fame contests). (Exact fame arithmetic is server/version-dependent; treat "visible fame signal tied to deeds" as the inspiration, not a specific formula.)
- **Social:** inn (sleep to protect from casual PvP; bribe the bartender to attack someone in the inn), clans, mail/chat RP, commentators/spectators, graveyard/shades underworld when dead.

### 15.2 Design ideas to borrow / adapt (entertainment RPG → media-watching RPG translation)

1. **Daily turn bucket.** LoGD gives a fixed number of forest fights per game day, refreshed on new day; unused turns don't carry over. **Adaptation idea:** a daily "investigation budget" — e.g. a daily cap on how many completion credits / case-picks you can make per real day (or per poll cycle), refreshed daily. Gives a daily-login rhythm and prevents one session from burning through everything. The existing spec's 5-minute poll + completion model stays; this would be an extra **per-day activity budget** layered on top. (Optional — V1 could omit; LoGD's "daily turns" is the inspiration, not a requirement.)

2. **New Day as a rhythm event.** LoGD's new day is a visible event: fresh fights, interest on banked gold, buff refresh, resurrect if dead. **Adaptation idea:** a visible "end of day / new day" moment in the RPG that surfaces: interest-like bonus on "saved up" progress (e.g. a small bonus for having watched something that day, or a streak refresh), fresh featured case for the new period, any daily-budget reset. Makes the passage of time feel like part of the game rather than invisible.

3. **Interest on banked progress.** LoGD rewards leaving gold in the bank (daily interest). **Adaptation idea (loose):** reward "not spending / not burning your budget" — e.g. a tiny bonus for carrying momentum across days (streak-adjacent), or a bonus for completing the same series across multiple days (slow-burn investment). Keep it light; the spec's existing streak model is the vehicle.

4. **Fame as a visible reputation signal.** LoGD has a fame rating/bar that reflects deeds. **Adaptation idea:** an explicit **fame / renown** number or bar on the character sheet, separate from XP/level, that goes up from notable deeds (first completion of a title, completing a featured case, unlocking a genre, a big streak, a holiday-window completion). Fame could feed into the "investigator reputation" flavor and unlock social-facing niceties. (Loose adaptation — finalize during implementation; the spec commits to "a fame/renown signal is a good LoGD-inspired idea to consider," not to a formula.)

5. **Spectators / commentary.** LoGD has commentators and a visible social layer (chat, mail, public commentary). **Adaptation idea (V2, household):** a lightweight "case feed" or "case commentary" where household members can leave a note on a case ("this one's good") — ties to the deferred V2 shared-quest idea. V1 is single-player, so this is just noted as LoGD-inspired future work.

6. **Death as a soft setback, not a full reset.** LoGD: dying costs on-hand gold + some XP, bank is safe, you linger in the graveyard doing things until resurrected. **Adaptation idea (optional, abstract):** a "cold case" or "on the shelf" state — if you let a taken case go too long without progress, it goes back to available (no big penalty; you just lose the in-progress status). Gives the case board a sense of "cases don't wait forever" without punishing. Very loose; finalize during implementation or omit.

7. **New-player onboarding ramp.** LoGD's primer + first-day guidance is explicit (the primer doc is written because new players were confused). **Adaptation idea:** a short first-login walkthrough in the RPG: "here's your character sheet, here's the case board, pick a case, watch something, come back in 5 minutes." The spec's V1 is single-player on a brand-new library, so onboarding matters. (Concrete onboarding text TBD during implementation.)

8. **Rank/title ladder as flavor.** LoGD's ranks are mostly flavor + a gate (15 levels then dragon). **Adaptation idea:** keep the spec's title/rank flavor (Junior Investigator → Detective → …) at level milestones, primarily cosmetic + the genre-unlock gate. LoGD's "rank = gate to dragon" maps loosely to "level = gate to next genre purchase."

9. **Seasonal / holiday content.** LoGD has holiday text modules (Christmas, April Fool's, Talk Like a Pirate Day, etc.) — date-gated flavor/special events. **Adaptation idea:** this is the LoGD ancestor of the spec's **date-detected holiday bonuses** (§5.2, Q6). LoGD's existing holiday-module pattern (a module that fires on a date and adds special text/effects) is a direct model for how to implement the RPG's holiday windows: a date-aware module that activates a bonus for the relevant genre during the window. Good concrete precedent.

10. **Grind-with-a-purpose pacing.** LoGD is deliberately paced: 15 levels per rank, forest fights as a finite daily resource, dragon as the long-term goal. **Adaptation idea:** keep the media RPG's pacing intentional rather than "watch everything, get everything instantly." The genre-unlock-via-sub-genre-XP model (§5.2/Q6) is the RPG's version of "work toward a gate." The daily budget idea (point 1) is the RPG's version of "finite turns per day." Both give the watching a game-loop shape rather than pure consumption.

### 15.3 What NOT to copy

- **PvP predation on new players.** LoGD's PvP-predation-on-farmboys is a multiplayer-social dynamic; the media RPG is single-player V1 and a household/co-op V2 — replicating predatory PvP wouldn't fit. (If V2 household competition ever appears, make it friendly/optional, not predatory.)
- **Gold/XP loss on death as the main tension.** The media RPG isn't a combat game; "death" tension doesn't translate directly. The "cold case goes back to the board" idea (point 6) is the softened analog if used at all.
- **PHP/MySQL architecture.** This RPG is Rust/Axum + Postgres, separate from the stack, not a new container. LoGD's PHP/MySQL is referenced for design only. (Though §15.4 below borrows the **module/host model**, not the stack.)
- **Real-time anything.** LoGD is turn/daily-based and click-through; that part actually **does** translate well (the RPG is also not real-time — 5-minute poll, daily rhythms). So LoGD's non-real-time daily-loop design is compatible, not opposed.

### 15.4 Host / module model — what a LoGD host does, and what to steal for this RPG

LoGD's **host model** is the relevant structural inspiration, more than any single mechanic:

- **Core + modules.** LoGD ships a core game plus a large set of **modules** (administrative, clan, darkhorse games, dragon mods, forest specials, gardens, graveyard specials, holiday texts, inn specials, lodge, mounts, pvp, quests, races, shades, specialties, travel, village, village specials, etc.). Hosts install the core, then **select which modules to install and activate** via the installer / Superuser Grotto module manager.
- **Baseline + optional.** The installer installs a recommended baseline of modules; hosts then toggle additional modules on/off and configure each module's settings in the game settings page. A host's "flavor" is largely the **module set + settings** they choose.
- **Host differentiation.** Different LoGD servers run different module sets and settings — that's how servers differentiate (more forest fights, new day link, PvP on/off, extra shops, extra races, extra quests, holiday modules active/inactive, etc.). The r/LotGD community and DragonPrime Reborn / NB-Core +nb fork exist specifically to help hosts find, rehost, and refactor modules.
- **Module release pipeline.** Historically via DragonPrime.net (now DragonPrime Reborn, a snapshot archive of legacy modules that often need refactoring for PHP 8+); modern forks (NB-Core +nb, StephenKise) add hooks, Composer integration, Twig templates, async/Ajax, and a Docker deployment path. The **module = a packaged add-on a host can install/activate/configure** is the key structural idea.

**What to steal structurally for the media RPG (this is the most useful part for the user's "mods/host inspiration" ask):**

- **Treat the RPG as core + optional modules/features.** The spec already has"finalize during implementation" items (featured-case selection rule, achievement list, genre unlock thresholds, perk list, holiday calendar, daily budget, fame formula). Model these as **configurable features** a host (the player/household) can turn on/off and tune, rather than hard-coded everything.
- **A "module/features" selection + settings page** for the RPG's own admin (the player's settings): which achievement categories are active, which holiday windows are enabled, daily budget on/off and size, fame formula on/off, featured-case mode (new vs all-time ranking), poll interval, near-end threshold, genre unlock costs. This mirrors LoGD's Superuser Grotto module manager + game settings page at the small scale of a single-player/household app.
- **Seasonal/holiday modules as date-gated feature modules.** LoGD's holiday text modules are a direct pattern for the RPG's holiday windows: each holiday window = a small feature module that activates on a date range and adds a bonus/rule. Easy to add new holiday windows later by adding a new module/rule, without touching core.
- **Config-driven, not hard-coded, wherever the spec says "finalize during implementation."** That's the LoGD lesson: the fun host customization comes from configuration + modules, not from rewriting core. For a single-player/household RPG the "host" is the player; give them a settings surface that reads like LoGD's module/settings grotto, just scaled down.
- **Onboarding primer.** LoGD's written primer (because new players were confused) is a reminder to invest in first-login guidance. The RPG should have a short onboarding walkthrough.

### 15.5 Concrete "modules" the RPG could ship as configurable features (inspired by LoGD's module catalog)

These are **possible configurable features** for the RPG's settings surface — inspired by LoGD's module names/categories, adapted to a media-watching RPG. None are required for V1; they're a menu the player can turn on later. Naming is LoGD-flavored for fun.

| LoGD module inspiration | RPG feature idea (configurable) | Spec section |
|---|---|---|
| Forest / forest fights / new day | Daily investigation budget (turns per day), new-day rhythm event, interest-on-momentum | §15.2 #1, #2, #3 |
| Fame bar / fame rating | Fame/renown signal on character sheet, from notable deeds (finalize formula) | §15.2 #4 |
| Holiday texts (Christmas, April Fool's, TLPD, etc.) | Date-detected holiday/seasonal bonus windows per genre (finalize calendar) | §5.2/Q6 #9 |
| Inn / sleeping / bartender bribe | (abstract) "cold case goes back to board" timeout; inn = a "signed off / away" state that protects a taken case's progress from aging? Very loose | §15.2 #6 |
| Races / classes / specialties | Genre specialization = the RPG's equivalent of race/class/specialty: your unlocked genres are your "specialties"; hidden achievements could be "specialty" unlocks (e.g. "completed 5 horror sub-genres" → horror specialist badge) | §5.2/Q6, §5.5 |
| Quests (bandit, dags, manticore, minotaur) | Featured cases / case types = the RPG's "quests"; maybe named case templates later (e.g. "The 5-night horror sweep" = watch 5 horror sub-genres in a window) | §5.4/Q7 |
| Mounts (stallion, rarity, upgrade) | (abstraction) a "long-term companion" perk you buy once with sub-genre XP or level — e.g. a permanent small XP boost for a chosen genre, flavoring the "stallion fights with you" idea as "your specialist consultant boosts this genre" | §5.2, §15.2 #10 |
| Graveyard / shades / Ramius | (abstract) the "cold case / on the shelf" underworld state where abandoned cases sit | §15.2 #6 |
| Bank / interest | (abstract) momentum interest / daily streak refresh | §15.2 #3 |
| PvP / slay other players | Not for V1; V2 household co-op/competition should be friendly and opt-in, not predatory | §15.3 |
| Commentary / spectators / chat | V2 household case feed / commentary ("this one's good") — ties to deferred shared quests | §15.2 #5 |
| Clan system | V2 household "case squad" / shared case board — deferred | V2 |
| Donators / points transfer / store | Not relevant to a personal media RPG; skip the real-money/donation patterns entirely | §15.3 |

### 15.6 Sources consulted

- LoGD official site & module list: <http://www.lotgd.net/> and `about.php?op=listmodules` (full module catalog used above).
- LoGD New Player Primer: <http://www.lotgd.net/petition.php?op=primer> (day loop, PvP, death, new day, forest fights, interest).
- LoGD gameplay hints (community): <http://www.geocities.ws/riochas/LoGD.html> (daily rhythm, stallion, vitality, gems, bank interest, 15-level-per-rank, dragon kill, new day link, server-to-server variation).
- LoGD wiki (Muds Wiki / Fandom): game format, ranks, dragon/new game+, versions/licensing, server list, Dragonprime/Dragonbones.
- NB-Core +nb fork: <https://github.com/NB-Core/lotgd> (modern PHP 8.3+, Composer, Twig, async/Ajax, Docker, module hooks, newday cron, settings-as-config).
- StephenKise revival: <https://github.com/stephenKise/Legend-of-the-Green-Dragon> (installer, PHP 8.4+/MySQL 8+, module system, permission system, translator tools, new-day loop).
- jimlunsford/lotgd + jimlunsford/lotgd-modules: core file layout (modules/ directory, modules.php, runmodule.php, superuser module manager, game settings per module) and a modules repo.
- DragonPrime Reborn (community module rehost/snapshot archive) and r/LotGD (community module ideas, recreation efforts) for the host/modding-ecosystem picture.
- LoGD server list (lotgd.net + wiki): how different hosts run different module sets/settings — the host-differentiation model.

### 15.7 How to use this section going forward

- Treat §15 as an **inspiration menu**, not a requirements list. Items marked "(optional)" or "(V2)" or "finalize during implementation" are not committed.
- When you resolve a §12-style open question that overlaps something here (e.g. "should there be a daily budget?", "fame formula?"), resolve it in §12/§5 and reference §15 as the inspiration source.
- The **host/module model (§15.4)** is the most actionable takeaway: design the RPG's settings/feature surface to be modular/config-driven like LoGD's module manager, scaled to a single-player/household app. Write that decision into the spec when it's resolved (it's currently "inspiration, not committed").

### 15.8 Thanks / attribution

- **Legends of the Green Dragon (LoGD)** — <http://www.lotgd.net/> — the original browser RPG this spec's §15 drew inspiration from (daily loop, fame/renown signal, holiday modules, host/module model, onboarding primer, genre/race/specialty flavor, rank ladder, new-game+ dragon cycle). LoGD is a remake/homage of Seth Able's **Legend of the Red Dragon (LoRD)** (a BBS door game). LoGD is **not** a technical dependency of this RPG (this RPG is Rust/Axum + Postgres, not PHP/MySQL); LoGD is acknowledged here as a design inspiration source and as a model for how a host ships/modularizes features.
- LoGD module catalog consulted: <http://www.lotgd.net/about.php?op=listmodules>.
- LoGD New Player Primer consulted: <http://www.lotgd.net/petition.php?op=primer>.
- Modern LoGD forks consulted: NB-Core +nb fork (<https://github.com/NB-Core/lotgd>), StephenKise revival (<https://github.com/stephenKise/Legend-of-the-Green-Dragon>), jimlunsford/lotgd + jimlunsford/lotgd-modules (<https://github.com/jimlunsford/lotgd>). DragonPrime Reborn community module archive referenced for the host/modding-ecosystem picture.

---

## 16. Pre-build probe results & remaining pre-build calls

> **Why this section exists:** a place to record the live stack probe (2026-09-08) and the pre-build info calls that come out of it, so they don't get lost before the first build. This is informational + tracking; resolve the remaining calls into §5/§6/§12 as they're decided.

### 16.1 Probe — what was checked (2026-09-08)

- **Plex** (`http://192.168.4.105:32400`, token redacted in log): reachable; `GET /library/sections` returns two sections — **Movies** (key=1, type=movie) and **TV Shows** (key=2, type=show). Counts (via `size` attrib): **Movies = 4**, **TV Shows = 8 shows** (the 8 shows map to 24 series in Sonarr). Sample movie: "Fear Street: Part One - 1994" (ratingKey 271, tmdbId not in Plex guid; Plex guid = opaque `plex://movie/...`; genres `Horror`+`Mystery` from `<Genre tag>`; rating 8.4 RT via `ratingImage="rottentomatoes://..."`; `originallyAvailableAt="2021-07-02"`; `addedAt` present). Sample show: "Gay for Play" (ratingKey 552, genres `Game Show`+`Comedy` from `<Genre tag>`; `audienceRating=6.0` with `themoviedb://image.rating` — TMDb rating source for shows; `guid="plex://show/..."`). Plex exposes **genres as top-level `<Genre tag>` only, no sub-genres**; Plex guids are opaque (no tmdbId/TVDb in guid). Plex does expose `addedAt`/`originallyAvailableAt`/`duration`/`year`/`contentRating`/`summary`/`poster`+`fanart`/`rating` (varies by type).
- **Sonarr** (`http://192.168.4.105:8989`, X-Api-Key): reachable; `GET /api/v3/health` ok; `GET /api/v3/series?includeStatistics=true` → **24 series**. Each series has **tvdbId + tmdbId**, `status`, `year`; **`genre: None`** (no genres in the v3 series response). Fetching by tvdbId (e.g. 85002) also returns `genre: None` and no genre-like keys.
- **Radarr** (`http://192.168.4.105:7878`, X-Api-Key): reachable; `GET /api/v3/health` ok; `GET /api/v3/movie` → **173 movies**. Each movie has **tmdbId + imdbId**, `year`, rich `ratings` object (imdb/tmdb/metacritic/rottenTomatoes/trakt, each with `value`+`votes`+`type`); **`genre: None`** (empty `genres` array). Fetching by tmdbId (e.g. 591275) returns `genre: None`, but the `genres` key exists (empty).
- **Host Postgres:** **not available.** No `pg_isready`/`psql` on PATH; no systemd `postgresql` service (inactive); nothing listening on port 5432. → Postgres must be installed/provisioned on the host before the RPG backend can connect.

### 16.2 Probe conclusions that touch the spec

- **Stack reachable + keys valid:** ✅ confirmed. The §2/§4 assumption holds; the probe layer (§7.3) can be written against real endpoints.
- **Library is small/new:** ✅ confirms §9.3 (no big backfill; faster updates at first; settle to 5-min cadence). 4 movies + 8 shows/24 series is a genuinely tiny library — V1 will have very few titles to work with, so the genre cascade will be thin (few genres present). Keep the genre list + cascade generic so it works on a tiny library and on a larger one later.
- **Genres come from Plex plus provider enrichment:** Plex `<Genre tag>` remains the fallback operational source; TMDb/TVDB add canonical genres and mapped sub-genre candidates. This is the selected V1 model, not an optional future path.
- **external_id (TMDb/TVDb) comes from *arrs only:** Radarr+Sonarr have `tmdbId` (movies+shows); Radarr movies also have `imdbId`. Plex guids are opaque. → the mirror's cross-source de-dup key (`external_id`) is populated from *arrs; Plex items matched to *arr items by title+year (and Plex ratingKey is the Plex-side key). → §4.5, §6.4.4.
- **Ratings and artwork use all four providers:** TMDb is the primary normalized score; OMDb supplies IMDb/Rotten Tomatoes fallback and identity validation; TVDB supplies TV ratings/metadata; Fanart.tv supplies artwork variants. Stack ratings remain fallback/audit data.
- **Postgres not on host:** ✅ correction to §6.3. Needs provisioning before build.

### 16.3 Resolved probe follow-ups

- **Provider enrichment is resolved:** TMDb, TVDB, OMDb, and Fanart.tv roles, cache boundaries, provenance, and failure behavior are defined in §4.6 and §6.4.4a. The provider-enriched sub-genre unlock model is committed for V1.
- **Legacy probe calls #15 and #16 are superseded:** the holiday-window and genre-list decisions are now governed by the resolved contracts in §5.2, §8.2, and §12 items 15–21. The fixed neutral genre order remains the source of truth; academy discipline labels are presentation metadata only.
- **Wizard rebrand follow-up:** no remaining product decision is hidden in the implementation. Any later change to an archetype effect, spell interaction, asset policy, or API/storage seam must update §§5.8–8.3 and the corresponding §12 decision before code changes.

### 16.4 Provider-enriched unlock model — implementation contract

The V1 unlock model uses TMDb/TVDB enrichment and sub-genre XP purchase:

- Completed watches add normal XP to each matching enriched sub-genre bucket (episode +10, movie +20), with parent genre derived from the provider mapping. Plex genres remain the fallback when enrichment is unavailable.
- A sub-genre is purchased at 100 XP. The first purchased sub-genre in the next fixed-cascade parent genre grants that genre access, subject to the character's level gate; only the next genre in the cascade is purchasable.
- Suggested titles filter the content mirror by enriched parent/sub-genre tags, with provider provenance retained.
- The `sub_genres`, `sub_genre_xp`, and `genre_xp_ledger` tables remain in the V1 schema. Raw provider payloads are stored in `content_provider_cache`.

### 16.5 How to use this section going forward

- §16 records the 2026-09-08 probe and its conclusions. As build progresses, move resolved calls into §5/§6/§12 and remove them from §16.
- **Call #14 is resolved in favor of TMDb/TVDB enrichment.** Provider API roles, cache boundaries, and enrichment provenance are committed in §4.6. Remaining genre-list and holiday-scope tuning is configuration/seed-data work, not a reason to drop provider enrichment.

---

*Spec end. Next step: review this wizard contract, then implement only the approved migration/API seam; do not begin frontend or asset work from unstated assumptions.*
> **Change log (2026-09-08, batch 1):** §12 Q1–Q5 resolved — Q1 host install (§6.3), Q2 Svelte (§8.4), Q3 5 min poll (§9.1), Q4 95% near-end threshold configurable (§5.1), Q5 base episode XP = 10 / level 2 at 100 XP rough anchors (§5.1, §5.2). Movie/season/series/streak XP values remain TBD.
> **Change log (2026-09-08, batch 2):** §12 Q6–Q13 resolved — Q6 genre unlock model: horror opening, everything else locked, unlock via sub-genre XP purchase, cascade one genre at a time, library-filtered sub-genre suggested titles, date-detected holiday/seasonal bonuses (§5.2); Q7 featured cases = new arrivals or all-time ranking by external rating (§5.4, §4.5); Q8 massive achievement list, categories decided, items finalized during implementation (§5.5); Q9 port 86532 (§10.2); Q10 set-a-pin gate, PIN in Postgres, V1 single-user (§7.3); Q11 common libs fine, minimal non-coupling sharing, no shared RPG state in common lib (§7.2); Q12 no big backfill (library brand new), faster updates at first, settle to 5-min cadence once caught up (§9.3); Q13 mirror everything the APIs expose, full metadata store, stack remains source of truth (§4.5).
> **Change log (2026-09-08, post-batch tightening):** §5.1/§5.2 got concrete V1 values — episode XP = 10, movie XP = 20, season bonus = 10 × episode count, series bonus = 25 × total episode count, first-completion +10, new-arrival +5 (48h window), featured +10, day-streak bonus table (§5.1.1), genre variety bonus +5/+15 (§5.1.2), level table 1→10 with cumulative XP thresholds (§5.2), genre unlock: horror opening, level-broadens-access (1 new genre per level), sub-genre XP purchase at 100 XP (+10 episode / +20 movie toward the sub-genre), fixed ordered genre list cascade, suggested titles from library mirror, holiday windows (Halloween/winter starters, +50% XP multiplier, date-gated feature modules) (§5.2); §6.4 concrete Postgres schema added (accounts, characters, character_state, genres, sub_genres, genre_access, sub_genre_xp, genre_xp_ledger, content with full metadata_blob mirror, watches, cases, featured_cases, achievements, character_achievements, sync_state, settings) + migration approach + seed data + design notes (§6.4).

### 15.8 Thanks / attribution

- **Legends of the Green Dragon (LoGD)** — https://www.lotgd.net/ — the original browser RPG
  this spec's §15 drew inspiration from (daily loop, fame/renown signal, holiday modules,
  host/module model, onboarding primer, genre/race/specialty flavor, rank ladder, new-game+
  dragon cycle). LoGD is a remake/homage of Seth Able's **Legend of the Red Dragon (LoRD)**
  (a BBS door game). LoGD is **not** a technical dependency of this RPG (this RPG is
  Rust/Axum + Postgres, not PHP/MySQL); LoGD is acknowledged here as a design inspiration
  source and as a model for how a host ships/modularizes features.
- LoGD module catalog consulted: https://www.lotgd.net/about.php?op=listmodules .
- LoGD New Player Primer consulted: https://www.lotgd.net/petition.php?op=primer .
- Modern LoGD forks consulted: NB-Core +nb fork (https://github.com/NB-Core/lotgd),
  StephenKise revival (https://github.com/stephenKise/Legend-of-the-Green-Dragon),
  jimlunsford/lotgd + jimlunsford/lotgd-modules (https://github.com/jimlunsford/lotgd).
  DragonPrime Reborn community module archive referenced for the host/modding-ecosystem picture.
> **Change log (2026-09-08, pre-build probe batch — batch 1 of info gathering):** live probe of Plex (192.168.4.105:32400) + Sonarr (:8989) + Radarr (:7878) + host Postgres → resolved §12 Q1 (Postgres NOT on host — needs provisioning before build, §6.3), §12 Q13 (Plex `<Genre tag>` is the operational genre source; external enrichment is now selected for canonical genres/sub-genres, §4.5/§4.6/§16.4), §12 Q5 (probe confirms ratings from Radarr's `ratings` object), §12 Q12 (confirmed tiny library: Plex Movies=4, TV Shows=8 shows/Sonarr=24 series → §9.3). §16 records the probe and provider-enrichment resolution.
> **Change log (2026-09-08, provider enrichment resolution):** TMDb, TVDB, OMDb, and Fanart.tv are committed V1 metadata providers. TMDb supplies canonical genres/keywords, TVDB supplies TV identity/episodes/tags, OMDb supplies IMDb/Rotten Tomatoes fallback data, and Fanart.tv supplies artwork. Responses are cached with provider IDs, timestamps, status/error metadata, raw payloads, normalized provenance, bounded retries, and stale-data fallback (§4.6, §6.4.4a, §10.3).
> **Change log (2026-09-08, during implementation):** PIN hashing finalized — the **`argon2` crate** (RustCrypto, pure Rust), **Argon2id** variant, **PHC string format**: `accounts.pin_hash` stores the full PHC string (`$argon2id$v=19$m=…,t=…,p=…$<salt>$<hash>`, self-describing salt + cost params), `accounts.pin_salts` stores the base64 salt separately for explicit access / future rotation. Version pin `argon2 = "0.5"`. (§6.4.1, §7.3)
> **Change log (2026-09-08, during implementation, bootstrap):** `settings` migration renumbered to **0006** (placement note in §6.4.11 — no dependency beyond `characters`, needed by bootstrap). Character-creation bootstrap finalized: application-level, one transaction, idempotent — ensure single character, seed `character_state` (level 1, xp 0), `genre_access` horror row, and missing `settings` V1 defaults verbatim (§6.4.11, §6.4.10). `featured_selection_mode` V1 default finalized as **`all_time_ranking`** (§6.4.10, §5.4 — robust on the tiny probe-confirmed library).
> **Change log (2026-09-08, during implementation, PIN gate):** set/verify flows finalized (§6.4.1, §7.3): set-PIN first-run-only, 4–12 digits, hashed Argon2id → account insert → character bootstrap; verify = load single `pin_hash`, missing account = locked, Argon2id re-derivation, wrong PIN rejected without side effects; malformed `pin_hash` is an operational error. Session issuance remains an implementation detail.
> **Change log (2026-09-08, during implementation, HTTP server):** session mechanics finalized (§7.3) — Axum server on 46532 (port corrected from the impossible 86532, see §12 Q9), opaque 128-bit session tokens in an `rpg_session` HttpOnly/SameSite=Lax cookie, server-side in-memory session store with 7-day expiry, logout deletes the session; public routes: `/healthz`, `/auth/status`, `/auth/set-pin`, `/auth/login`; everything else 401 without a valid session. First API surface: `GET /api/character` (character overview behind the gate). Startup runs `migrate()`; binary target `movie-rpg` added.
> **Change log (2026-09-08, during implementation, watches):** migration 0007 lands `watches` (§6.4.5) + the deferred `genre_xp_ledger` (§6.4.3 placement note corrected 0006 → 0007). `watches.featured_case_id` is created as a plain column — its FK to `featured_cases` is deferred to that table's migration (0009), which must `ALTER TABLE watches ADD CONSTRAINT watches_featured_case`.
> **Change log (2026-09-08, during implementation, cases):** migration 0008 lands `cases` (§6.4.6) with `featured_case_id` created plain (FK deferred); migration 0009 lands `featured_cases` (§6.4.7) and completes **both** deferred FKs (`watches_featured_case`, `cases_featured_case`). §6.4.7 period format finalized: ISO week label `YYYY-Www` (V1 cadence weekly, §5.4).
> **Change log (2026-09-08, during implementation, achievements):** migration 0010 lands `achievements` + `character_achievements` (§6.4.8) and seeds the full §5.5 first-cut list (101 rows) with slugs, kinds, targets, and evaluation metadata finalized per §6.4.8 implementation notes.
> **Change log (2026-09-08, during implementation, achievements engine):** §6.4.8 evaluation contract finalized (pure snapshot evaluation, kind dispatch, V1 non-evaluable set, unlock-only writes). Implemented as `achievements.rs` (pure engine) + `evaluate_achievements`/`badge_wall` store flows + gated `GET /api/achievements`.
> **Change log (2026-09-08, during implementation, watch orders):** §5.7 and §6.4.13 added and finalized — mystery watch orders (per-genre cycles, reveal derived from the §6.4.5 watch ledger, audited skip-grant ledger, algorithmic V1 generation, movies-only, pre-watched exclusion, API-enforced mystery); migration 0011 placement noted in §6.4.11.
> **Change log (2026-09-08, during implementation, game tick):** §9.1 game-tick phase order finalized (watch award → order reveals → achievement evaluation). Implemented as `game.rs` (`run_game_tick`: `refresh_watch_orders` → `evaluate_achievements`, phase 1 a documented slot) wired into `POST /api/orders/refresh`.
> **Change log (2026-09-08, during implementation, phase 1):** watch-award semantics finalized in §9.1 (viewCount/viewOffset detection, award-once rule, streak advance + §5.1.1 milestone timing, level re-evaluation from §5.2 thresholds). Implemented as `awards.rs` (pure detection/level/streak math) + tick phase 1 wired to a `PlexClient`.
> **Change log (2026-09-08, during implementation, poll loop):** §9.1 poll-loop semantics finalized (sync → tick per cycle, fixed 5-minute interval, logged-and-non-fatal sync/phase-1 failures with phases 2–3 still advancing, Ctrl-C drains both server and loop, one-line per-cycle logging). Implemented as `poll.rs` (`PollStack::from_config`, `run_poll_cycle`, `run_poll_loop` with watch-shutdown); `main.rs` spawns the loop beside the HTTP server over the shared store.
> **Change log (2026-09-10, spec-first wizard rules revision):** the product presentation is the original, cozy scholarly Lantern Academy. Neutral ledgers and stable identifiers remain authoritative; the backend-first vertical release, six pre-designed archetypes with one primary and at most one bounded secondary effect, immutable next-tick loadout changes, five spells with player-selected one-discipline affinities, newly-awarded-watch-only meters, 100-point charge thresholds, three-charge caps, overflow audit, guided UI boundary, and minimal 0012/0013 + API seam are finalized in §§2.1–2.3, 5.8, 6.4.14, 7.4, and 8.1–8.3. Implementation is deliberately deferred.
