# Changelog

## [2.0.0](https://github.com/WhispersOfJ/rawrz/compare/v1.35.0...v2.0.0) (2026-09-12)


### ⚠ BREAKING CHANGES

* Bazarr (:6767) and its config directory are removed; existing subtitle sidecars in media/ are unaffected.

### Features

* add --auto-safe drain mode that imports only provably-correct queue items ([#135](https://github.com/WhispersOfJ/rawrz/issues/135)) ([4d70a0d](https://github.com/WhispersOfJ/rawrz/commit/4d70a0dfca1f08693d0104bc07a15fbd94b63540))
* add accounts and characters identity migration ([15b9b55](https://github.com/WhispersOfJ/rawrz/commit/15b9b55a9a21f2d881b50798b5bcccf0545d1841))
* add achievements engine, mystery watch orders, and the game tick ([377629d](https://github.com/WhispersOfJ/rawrz/commit/377629daa5edff78f72267df7377915b700a755c))
* add achievements migration with the section 5.5 first-cut seed ([63b9042](https://github.com/WhispersOfJ/rawrz/commit/63b9042df9b330349a4709e9e40da1b5b4e78d75))
* add argon2 PIN gate with set and verify account flows ([19e1b93](https://github.com/WhispersOfJ/rawrz/commit/19e1b9380122f28ed12befda89028b972ca8de96))
* add audiobook and comic servers ([2d21c89](https://github.com/WhispersOfJ/rawrz/commit/2d21c89612aab27c76481a201407c9d80dc71513))
* add axum server with pin gate and session cookies ([28b2610](https://github.com/WhispersOfJ/rawrz/commit/28b2610670f9cf125b0f10369055666d90f69c08))
* add bash port of the stack-* CLI library ([#85](https://github.com/WhispersOfJ/rawrz/issues/85)) ([adebd0d](https://github.com/WhispersOfJ/rawrz/commit/adebd0d41d974c00f7eeb227bb443ce5d09505cf))
* add Bazarr subtitle service ([8be99d4](https://github.com/WhispersOfJ/rawrz/commit/8be99d44aa3b965b90fd7113ba6d52c9e774348f))
* add bazarr.db to the maintenance digest DB gate ([#146](https://github.com/WhispersOfJ/rawrz/issues/146)) ([466e776](https://github.com/WhispersOfJ/rawrz/commit/466e776878a726b481e500fd4c3d8ffde2eb8449))
* add cache-aware metadata enrichment ([805d0d0](https://github.com/WhispersOfJ/rawrz/commit/805d0d0be1003f02d24f85a31be37bd1e9d55f08))
* add cases and featured cases migrations ([2b33861](https://github.com/WhispersOfJ/rawrz/commit/2b33861766fd67bfea94b38e305403a7c06a2b4e))
* add content persistence upsert plan ([2b34c3b](https://github.com/WhispersOfJ/rawrz/commit/2b34c3b7a7701cda1aeeddc9dfa2a737cf8dc0c2))
* add content provider cache migration ([21f17ec](https://github.com/WhispersOfJ/rawrz/commit/21f17ecbd2e2a4a28b78dc87747f63b7572aff99))
* add D11 safe-command registry and live matrix runner in tests/live ([#187](https://github.com/WhispersOfJ/rawrz/issues/187)) ([b0658c8](https://github.com/WhispersOfJ/rawrz/commit/b0658c847249af1884e1b85c2733ab987e1f2c1d))
* add deterministic content sync records ([e3a2130](https://github.com/WhispersOfJ/rawrz/commit/e3a2130b9312618ec110347e6e6ecb6cbb358316))
* add end-to-end content sync pipeline ([3cba1b4](https://github.com/WhispersOfJ/rawrz/commit/3cba1b41375cc8e48023777f3eebee4c50ef80d5))
* add Eweka as third Usenet provider slot ([#97](https://github.com/WhispersOfJ/rawrz/issues/97)) ([8a5f610](https://github.com/WhispersOfJ/rawrz/commit/8a5f610468ced58f58b1253f3cfe02e854471295))
* add genre access and character state migrations ([c025f36](https://github.com/WhispersOfJ/rawrz/commit/c025f36a0a65882f3a2ecab5b1e298779837fe52))
* add guarded radarr.db bloat prune maintenance path ([#103](https://github.com/WhispersOfJ/rawrz/issues/103)) ([836611b](https://github.com/WhispersOfJ/rawrz/commit/836611bbb040f965085189cfb593116d919a4c83))
* add idempotent nightly stack-disk-reclaim cron installer ([#108](https://github.com/WhispersOfJ/rawrz/issues/108)) ([47f3fd2](https://github.com/WhispersOfJ/rawrz/commit/47f3fd21d03343068dbde36d7c28e38adaf8ea12))
* add idempotent schema migration runner ([ce46493](https://github.com/WhispersOfJ/rawrz/commit/ce46493b499b8157fd580cdfcc19dbbd514bfb44))
* add Lantern Academy archetype selection ([cfae8c0](https://github.com/WhispersOfJ/rawrz/commit/cfae8c0a4ea6646ff3222dbc37b9ae915089a49d))
* add Lidarr and Readarr services ([871643c](https://github.com/WhispersOfJ/rawrz/commit/871643c45122341946682a1f909fc13fed556fc6))
* add MCP baseline comparison and refresh workflow ([a261051](https://github.com/WhispersOfJ/rawrz/commit/a261051de05277818e595a933d9805aacc9216b6))
* add metadata provider probe foundation ([1e29e66](https://github.com/WhispersOfJ/rawrz/commit/1e29e66913f59803cefac7157ad2303333ce3434))
* add network security services ([879282a](https://github.com/WhispersOfJ/rawrz/commit/879282a272c2ea4a2776e5a39ff23bc635c4b20f))
* add nzbdav backend wedge watchdog (probe, queue-safe restart, Discord alerts) ([0ca1aa2](https://github.com/WhispersOfJ/rawrz/commit/0ca1aa208939596b53281bfb25a307170ff28386))
* add ordered sync state migration ([9d021d4](https://github.com/WhispersOfJ/rawrz/commit/9d021d41228fdb79a6a4adb8897428473861e6e8))
* add per-call-type API timeouts so no stack-* command can hang forever ([#86](https://github.com/WhispersOfJ/rawrz/issues/86)) ([9d3c935](https://github.com/WhispersOfJ/rawrz/commit/9d3c935435387d8d951126defd095ae3222e3614))
* add pre-push git hook running the preflight gate ([#83](https://github.com/WhispersOfJ/rawrz/issues/83)) ([67498e8](https://github.com/WhispersOfJ/rawrz/commit/67498e8832debceffdd04a6e63ff0af9a75f350f))
* add recyclarr service for TRaSH-Guides profile sync ([#156](https://github.com/WhispersOfJ/rawrz/issues/156)) ([d9c30fd](https://github.com/WhispersOfJ/rawrz/commit/d9c30fd3a960861b4cf68488963d0ee1a78d4562))
* add retry handling to provider probes ([898689b](https://github.com/WhispersOfJ/rawrz/commit/898689b6ec4ecb170ee287131b398dc3752f024b))
* add settings migration and character bootstrap seeding ([02d9dad](https://github.com/WhispersOfJ/rawrz/commit/02d9dad700639e8dedfc27d373990202c5985c80))
* add stack-audit-residue for retired-service/path residue ([#116](https://github.com/WhispersOfJ/rawrz/issues/116)) ([2e65f94](https://github.com/WhispersOfJ/rawrz/commit/2e65f94c1d2728c028fb98d7129f8966ecf50f64))
* add stack-disk-reclaim for nightly Docker disk reclamation ([#106](https://github.com/WhispersOfJ/rawrz/issues/106)) ([a48d367](https://github.com/WhispersOfJ/rawrz/commit/a48d367a68ad164a400fda0608dd27136c5a5b7a))
* add stack-maintenance-digest — verify nightly maintenance ran ([#113](https://github.com/WhispersOfJ/rawrz/issues/113)) ([4c892dc](https://github.com/WhispersOfJ/rawrz/commit/4c892dc9bcb2b7866f20425c77079268b9f781ce))
* add stack-plex-markers read-only marker audit command ([#76](https://github.com/WhispersOfJ/rawrz/issues/76)) ([cf27366](https://github.com/WhispersOfJ/rawrz/commit/cf273664d6f62b82bc87129a45aba742a285ba26))
* add stack-radarr-health on-demand Radarr DB integrity check ([#73](https://github.com/WhispersOfJ/rawrz/issues/73)) ([5950b38](https://github.com/WhispersOfJ/rawrz/commit/5950b387f1184a94d19e1f42525b2513feec3235))
* add stack-sonarr-prune for sonarr.db MediaInfo bloat ([#114](https://github.com/WhispersOfJ/rawrz/issues/114)) ([7fd2009](https://github.com/WhispersOfJ/rawrz/commit/7fd200920e7b54675a0b587d0ef8200680014827))
* add stack-tui floating-window launcher for the stack-* functions ([#95](https://github.com/WhispersOfJ/rawrz/issues/95)) ([4aecdcd](https://github.com/WhispersOfJ/rawrz/commit/4aecdcdfaf590964970c4bbd67a425d3b01586fe))
* add stack-worktree helper for task-named worktrees ([#55](https://github.com/WhispersOfJ/rawrz/issues/55)) ([68c5fa2](https://github.com/WhispersOfJ/rawrz/commit/68c5fa2b32e899dc12cc96e7edcb51d22073a11b))
* add Traefik dashboard link to landing page ([ffb56ac](https://github.com/WhispersOfJ/rawrz/commit/ffb56ac4537f0dd46e62ec17ca80b86ea2740f43))
* add transactional postgres content store ([24da675](https://github.com/WhispersOfJ/rawrz/commit/24da6751ae4a6a4e40d661c3378c12bf5bd5a9e2))
* add unattended RPG polling loop ([c9e5e48](https://github.com/WhispersOfJ/rawrz/commit/c9e5e482b839c944fa5c9c9c3abe1172a5bcef39))
* add Vaultwarden and n8n services ([4e6f494](https://github.com/WhispersOfJ/rawrz/commit/4e6f494a3f0fcf42d394522901bd643e25c4a096))
* add watches ledger and deferred genre xp ledger migration ([3f85cd8](https://github.com/WhispersOfJ/rawrz/commit/3f85cd8980e03b79b291bd414bb4e3c47a71e179))
* add waybar idle-lock pause module (presentation mode) ([#164](https://github.com/WhispersOfJ/rawrz/issues/164)) ([73aa6a3](https://github.com/WhispersOfJ/rawrz/commit/73aa6a32e538c89d876302b054fbc433657f1cf0))
* add waybar recording and night-light status indicators ([#165](https://github.com/WhispersOfJ/rawrz/issues/165)) ([d32c056](https://github.com/WhispersOfJ/rawrz/commit/d32c0563eb36518d9164452add45fdf45fb07ad3))
* Alertmanager with Discord notifications + Cleanuparr setup docs ([18861dc](https://github.com/WhispersOfJ/rawrz/commit/18861dcd321cf5956908b8bce84c5328378eb419))
* Ansible playbook to auto-trust the local CA on LAN devices ([3b10053](https://github.com/WhispersOfJ/rawrz/commit/3b10053d7b6e042bb09d2d8248c07afeba4feb00))
* attach provider metadata to content records ([3e15858](https://github.com/WhispersOfJ/rawrz/commit/3e15858da376b50ecc57ad64fb3bc07c61d758be))
* **catalog:** entries 51-75 toward the 100-entry v1 target ([#18](https://github.com/WhispersOfJ/rawrz/issues/18)) ([2777f0c](https://github.com/WhispersOfJ/rawrz/commit/2777f0c928fae8519989a5c2090a6dab3e98f5d7))
* **catalog:** serve the curated catalog from backend and GUI ([#21](https://github.com/WhispersOfJ/rawrz/issues/21)) ([9b13fa3](https://github.com/WhispersOfJ/rawrz/commit/9b13fa337d95555aa48d9b82658b1476e20951d5))
* certificate setup section on the landing page ([c78f7f6](https://github.com/WhispersOfJ/rawrz/commit/c78f7f6e2dd37262e0968d0ee738c6b3bd7552e0))
* **cli:** add /api/v2/cli/ endpoints for fish functions ([32f475c](https://github.com/WhispersOfJ/rawrz/commit/32f475c04655daff966bbd2d9904673c6d8ba1a7))
* config-drift guard — running container images vs compose pins ([#124](https://github.com/WhispersOfJ/rawrz/issues/124)) ([f57a5e4](https://github.com/WhispersOfJ/rawrz/commit/f57a5e4ddea7d42b6a55f43d1c50e07001d8aa2c))
* **control-panel:** read-only TLS cert diagnostic endpoint ([33de97e](https://github.com/WhispersOfJ/rawrz/commit/33de97e60388e9cc077032b43150ac908f20abf6))
* DB growth-trend predictor turns bloat incidents into prune dates ([#147](https://github.com/WhispersOfJ/rawrz/issues/147)) ([da16551](https://github.com/WhispersOfJ/rawrz/commit/da16551c8ffd0e1ecd945fe9cf5def30865f7544))
* deploy 9 new stack services with crowdsec bouncer and nzbdav categories ([3d25969](https://github.com/WhispersOfJ/rawrz/commit/3d2596909c1daa7051bbca99e7c920d40cf7c3e2))
* digest gains a config-drift surface (running images vs pins) ([#125](https://github.com/WhispersOfJ/rawrz/issues/125)) ([7989c0f](https://github.com/WhispersOfJ/rawrz/commit/7989c0f1e7eeee50dad6508fd314e70818b7ae6a))
* digest gains a full-host residue audit surface ([#119](https://github.com/WhispersOfJ/rawrz/issues/119)) ([51c3cb9](https://github.com/WhispersOfJ/rawrz/commit/51c3cb92778d76564c07550f43e7daf70e28343b))
* digest verifies the monthly sonarr prune log (fresh + exit 0) ([#121](https://github.com/WhispersOfJ/rawrz/issues/121)) ([63539e2](https://github.com/WhispersOfJ/rawrz/commit/63539e230ea4857c5b9135bde8b989a1638ed857))
* drift reports name the recreate and how stale the pins are ([#126](https://github.com/WhispersOfJ/rawrz/issues/126)) ([6e5fe99](https://github.com/WhispersOfJ/rawrz/commit/6e5fe994d5fed8d2537d6ca6f74b21e1a8d9f601))
* ecosystem improvements — full health coverage, live Mermaid colors, registry as single source ([346cd14](https://github.com/WhispersOfJ/rawrz/commit/346cd144cfc70d5fb08169e5500120ed8a1b4a2c))
* enrich synchronized content batches ([07d4d10](https://github.com/WhispersOfJ/rawrz/commit/07d4d10f552efee68c018de48b472879543a1510))
* extend the queue drain to Radarr with an --app flag ([#132](https://github.com/WhispersOfJ/rawrz/issues/132)) ([0f36ccb](https://github.com/WhispersOfJ/rawrz/commit/0f36ccb92a754c51e9a3566546e3140ff013c87b))
* **fish:** add generated tab completions for all stack-* commands ([9a3c154](https://github.com/WhispersOfJ/rawrz/commit/9a3c1544bc14748f36df6d331f32087a674515b4))
* **fish:** complete rewrite of all fish functions — 95 API-backed + 23 host tools ([979d769](https://github.com/WhispersOfJ/rawrz/commit/979d76935df5b9429476d7250145237fc03dac25))
* **fish:** Phase 1 — direct service API helpers, bypass control panel ([a78c8bf](https://github.com/WhispersOfJ/rawrz/commit/a78c8bf0258b9777727d5a3c78ab123315e2cd1e))
* **fish:** Phase 2 — rewrite 71 fish functions to bypass control panel ([c4e12f1](https://github.com/WhispersOfJ/rawrz/commit/c4e12f1f76c691b6382d98069d34b2a96cac8aa7))
* flag *arr import-queue pile-ups in the maintenance digest ([#133](https://github.com/WhispersOfJ/rawrz/issues/133)) ([16f01f6](https://github.com/WhispersOfJ/rawrz/commit/16f01f65c58f709c0178797b6f5729a12003f43a))
* group content records across sources ([e7c78eb](https://github.com/WhispersOfJ/rawrz/commit/e7c78ebbb12106b034fb30408ede947613399f03))
* guard against bind-mount file staleness (landmine [#1](https://github.com/WhispersOfJ/rawrz/issues/1)) ([36876ea](https://github.com/WhispersOfJ/rawrz/commit/36876eacbce86ea7f8c02f37d5f5a89d200d460e))
* guard nzbdav recreate against the non-persistent queue landmine ([dbfe5a3](https://github.com/WhispersOfJ/rawrz/commit/dbfe5a3a7295bcbc5979cc0da6c36dea7d0e5097))
* hydrate provider cache on sync startup ([33b4930](https://github.com/WhispersOfJ/rawrz/commit/33b4930fdbd798fb5357d677805121968e444e28))
* initial commit — Movie / TV RPG spec ([4bd40ac](https://github.com/WhispersOfJ/rawrz/commit/4bd40ac5e613e652f1c8776d367c774b5649070f))
* initial scaffold — merge media-stack + metacacharr into The Bear Cave ([a0c976d](https://github.com/WhispersOfJ/rawrz/commit/a0c976de3b42f8d9f7ee257943e9bce55f5866ba))
* **landing-page:** CA-trust badge backed by the live TLS probe ([81d20b6](https://github.com/WhispersOfJ/rawrz/commit/81d20b6433475795238ce060072b0f9a525fee3b))
* **landing:** Phase 3 — replace control-panel health with direct service probes ([3efae51](https://github.com/WhispersOfJ/rawrz/commit/3efae5188a4b9ac551b7770fa5b940e275bffd3f))
* local CA (mkcert) with trusted wildcard cert for all nip.io hostnames ([736f8c3](https://github.com/WhispersOfJ/rawrz/commit/736f8c357c399a1631626b5029d86ef17fad10b1))
* M0 skeleton — axum backend, React frontend, catalog+parity, CI ([289b90b](https://github.com/WhispersOfJ/rawrz/commit/289b90b9f5d97fbc20e3b6e28993e293c96bbd78))
* make provider probes locally testable ([330c0b5](https://github.com/WhispersOfJ/rawrz/commit/330c0b590de0436a058ab4025ad802a74f010906))
* mine grab history for Sonarr alias candidates ([#142](https://github.com/WhispersOfJ/rawrz/issues/142)) ([91bca5c](https://github.com/WhispersOfJ/rawrz/commit/91bca5c945e1b3457ee04c42b454eb3fedafe398))
* monitoring stack — Grafana dashboards, Prometheus alert rules, landing page integration test ([edb6c5c](https://github.com/WhispersOfJ/rawrz/commit/edb6c5ca418a9d2f669577481b8cf097cb845be8))
* **monitoring:** add backup freshness, disk fill rate, and load alerts ([#39](https://github.com/WhispersOfJ/rawrz/issues/39)) ([e5f924c](https://github.com/WhispersOfJ/rawrz/commit/e5f924c9df00781b9cf00bd1edf7e5097e1cf27d))
* **nzbdav:** re-adopt Eweka as a tertiary Usenet provider ([90a76cb](https://github.com/WhispersOfJ/rawrz/commit/90a76cbb44370243eaa848001026d928af0c3cd4))
* **nzbdav:** track InfiniDysk dev tag and add queue-guarded update script ([23a4e2a](https://github.com/WhispersOfJ/rawrz/commit/23a4e2a003db6e0aa6383049366f31bacc2bab43))
* orchestrate stack content sync ([9eb62b7](https://github.com/WhispersOfJ/rawrz/commit/9eb62b77fa24f710691c2ca48e3a1a154591312e))
* Phase 4 — remove control panel, archive to archive/control-panel/ ([faf4127](https://github.com/WhispersOfJ/rawrz/commit/faf41274be6a30a610b1e812284fef526a16d338))
* **plex:** use official beta image ([e3323c3](https://github.com/WhispersOfJ/rawrz/commit/e3323c310568315cacf3650588709beb1cf2d292))
* preflight sonarr db size leg (EpisodeFiles MediaInfo gate) ([#118](https://github.com/WhispersOfJ/rawrz/issues/118)) ([ce53597](https://github.com/WhispersOfJ/rawrz/commit/ce535974ce0619d33491a3ec4d60c797c1e12406))
* promote Sonarr queue drain into scripts/drain_sonarr_queue.py ([#58](https://github.com/WhispersOfJ/rawrz/issues/58)) ([ffa4b7a](https://github.com/WhispersOfJ/rawrz/commit/ffa4b7ab84546676221f43afa2cbe3ef48f3f596))
* raise backup Usenet provider to 60 connections ([#191](https://github.com/WhispersOfJ/rawrz/issues/191)) ([5253003](https://github.com/WhispersOfJ/rawrz/commit/52530030aaa6eac0d1d9db2c65abbc608509dc61))
* re-adopt Bazarr as the 9th always-on service ([#144](https://github.com/WhispersOfJ/rawrz/issues/144)) ([c208bdd](https://github.com/WhispersOfJ/rawrz/commit/c208bddf35d8d9bced26a48ce205594c36d36e95))
* re-retire Bazarr and restore the 8-service always-on stack ([#192](https://github.com/WhispersOfJ/rawrz/issues/192)) ([5369b2f](https://github.com/WhispersOfJ/rawrz/commit/5369b2fa2f99021054050aaf5fbcabc727e1b55e))
* scale waybar bar up 40% ([#163](https://github.com/WhispersOfJ/rawrz/issues/163)) ([c2878c7](https://github.com/WhispersOfJ/rawrz/commit/c2878c786a23d9336917afdf1d072f27e8e6ea2d))
* scoped missing-search wrapper replacing blind whole-series sweeps ([#136](https://github.com/WhispersOfJ/rawrz/issues/136)) ([65cbc3e](https://github.com/WhispersOfJ/rawrz/commit/65cbc3e4622bb34d96ab32b051c8ce3b96d34ae0))
* **security:** add login rate limiting and failed-attempt logging ([523d840](https://github.com/WhispersOfJ/rawrz/commit/523d84087a9cec78bc9c51e626925efe5e7f7465))
* **security:** add resource limits to all 22 services ([#5](https://github.com/WhispersOfJ/rawrz/issues/5)) ([fb3b43a](https://github.com/WhispersOfJ/rawrz/commit/fb3b43a8ef32e3afff70c6d549a6fc39891b9fed))
* **security:** add URL credential redaction helpers ([6608169](https://github.com/WhispersOfJ/rawrz/commit/66081698869a483b108e4c546971d17c050d5188))
* **security:** harden control panel + fix no-op Trivy gate ([44f97ae](https://github.com/WhispersOfJ/rawrz/commit/44f97ae17e936bd12c58b1a9aea1e790d293d4c2))
* **setup:** sync RELEASE_PLEASE_TOKEN to GitHub Actions secrets ([178d36a](https://github.com/WhispersOfJ/rawrz/commit/178d36ad2ef7a984b5d6977ac126a5358c949704))
* slim stack and add manual Plex cache maintenance ([08d195d](https://github.com/WhispersOfJ/rawrz/commit/08d195d366629e3ab2fadd2cd77004dd5a7a671f))
* sonarr prune also slims event-table JSON payloads ([#120](https://github.com/WhispersOfJ/rawrz/issues/120)) ([c267eaf](https://github.com/WhispersOfJ/rawrz/commit/c267eaf53d8bfdc0933aea87e192deb98077ef84))
* streaming Dropbox backup of the repo (no media/metadata/secrets) ([#152](https://github.com/WhispersOfJ/rawrz/issues/152)) ([137fcec](https://github.com/WhispersOfJ/rawrz/commit/137fcec014ff144da96656d8625b10cc211f4e2b))
* switch landing page dashboard links to HTTPS nip.io hostnames ([69b18b7](https://github.com/WhispersOfJ/rawrz/commit/69b18b78c2daba25b101e03bd722d299532e3ad7))
* track the waybar stack-tui launcher module as dotfiles ([#139](https://github.com/WhispersOfJ/rawrz/issues/139)) ([0f60f1b](https://github.com/WhispersOfJ/rawrz/commit/0f60f1b90688983e5d08fd40150f0ff3f6b628b9))
* trust the local CA inside every stack container ([e2acbe0](https://github.com/WhispersOfJ/rawrz/commit/e2acbe08342c0ead95a80c665098e1a95ba09516))
* unified ecosystem — service registry, category layout, Mermaid graph, detail panels, backlinks ([06889f5](https://github.com/WhispersOfJ/rawrz/commit/06889f53265ed56d05f4b1b1c14a8256a7e396b5))
* update landing page with new services, health probes, and links ([b2bc2a8](https://github.com/WhispersOfJ/rawrz/commit/b2bc2a8651692002261854b442856a60889d9cd9))
* warn at shell load when a pre-set arr key differs from .env ([#99](https://github.com/WhispersOfJ/rawrz/issues/99)) ([4bf310d](https://github.com/WhispersOfJ/rawrz/commit/4bf310de3ce62a0da4ea547964ab33a3d1501254))
* watchable view, request arrival notifier, and activity feed (TODO [#6](https://github.com/WhispersOfJ/rawrz/issues/6)-[#8](https://github.com/WhispersOfJ/rawrz/issues/8)) ([#149](https://github.com/WhispersOfJ/rawrz/issues/149)) ([4a27606](https://github.com/WhispersOfJ/rawrz/commit/4a27606c464563c7e7278f12fc5e11724b81d70d))
* wire MCP baseline alerting, preflight gate, and unpackerr connections ([a92fe8a](https://github.com/WhispersOfJ/rawrz/commit/a92fe8af374dd2cee79ea0d6a155429e8312122e))
* wire nzbdav queue guard into fish and bash docker compose wrappers ([9eda93c](https://github.com/WhispersOfJ/rawrz/commit/9eda93c1574ee51c9294ca85a3bac64603791817))
* **wizard:** add spell resource, affinity, and charge economy ([15187f4](https://github.com/WhispersOfJ/rawrz/commit/15187f40069c71b3dcf52912132dac8062bfc6cd))


### Bug Fixes

* __arr_api_key error names the real uppercase var ([#98](https://github.com/WhispersOfJ/rawrz/issues/98)) ([44e54ed](https://github.com/WhispersOfJ/rawrz/commit/44e54ede570aa2462821795c0fb13466df249e39))
* add missing ADGUARD_ADMIN_* vars to .env.template ([64ec98c](https://github.com/WhispersOfJ/rawrz/commit/64ec98c764bbddf7544175734747128d1cdcc617))
* **alerts:** correct BackupStale metric and HostHighLoad PromQL ([39e7030](https://github.com/WhispersOfJ/rawrz/commit/39e703099437b9d3d3bf5370e8caab0490f9e44b))
* align docs and gitignore with config/ layout after Phase 4 migration ([85841e7](https://github.com/WhispersOfJ/rawrz/commit/85841e7c2804fac09602c08a9f524da7704ddef3))
* allow docker type and drop invalid input in PR title lint ([7d889e1](https://github.com/WhispersOfJ/rawrz/commit/7d889e1e63570105a4619ad51dbe96453224d7d0))
* **auth:** rate-limit PIN login attempts ([079dd54](https://github.com/WhispersOfJ/rawrz/commit/079dd544d9ef9c8ebcc370002c9322b8be7a69bf))
* auto-load stack .env into fish so stack-* commands reach the services ([f819898](https://github.com/WhispersOfJ/rawrz/commit/f819898b06426f7fe75c1f0c0d3f04bc535c03ac))
* bound install verification with a timeout on the stack-help check ([#66](https://github.com/WhispersOfJ/rawrz/issues/66)) ([026082e](https://github.com/WhispersOfJ/rawrz/commit/026082eff6720a7c1b8f6bbb41c261af78c46a08))
* bump gitleaks pre-commit hook past the Go 1.24 wasm panic ([#123](https://github.com/WhispersOfJ/rawrz/issues/123)) ([e23e31b](https://github.com/WhispersOfJ/rawrz/commit/e23e31bb3176765abe25f97c34ec46f561e808ac))
* cap __stack_containers docker call with a timeout ([#92](https://github.com/WhispersOfJ/rawrz/issues/92)) ([50b5adf](https://github.com/WhispersOfJ/rawrz/commit/50b5adf8816120bcd1bd1cfef8235fa647855782))
* **catalog:** change network constant from 'stacknet' to 'bearcave' ([d16248e](https://github.com/WhispersOfJ/rawrz/commit/d16248e50364f8cf6f9f6ab1367db558f621d549))
* **ci:** add actionlint checksum verification + pre-commit gate + docs ([aebec8d](https://github.com/WhispersOfJ/rawrz/commit/aebec8de2278e8fe50d3eb9ed9b8133397acb8d8))
* **ci:** add DISCORD_WEBHOOK_URL to secret manifest ([54dc06e](https://github.com/WhispersOfJ/rawrz/commit/54dc06e46c8c99aaad48f1c683752e5daf788a75))
* **ci:** add missing step id and fix reclaim accumulation in disk-cleanup ([3798e6c](https://github.com/WhispersOfJ/rawrz/commit/3798e6c57814240ccaf8452af4309e4a81c22e20))
* **ci:** add pytest to exporter test dependencies ([16d1ab5](https://github.com/WhispersOfJ/rawrz/commit/16d1ab53c0feb0f4eed51729961b75630686ad38))
* **ci:** correct actionlint SHA-256 checksum ([19d6fa9](https://github.com/WhispersOfJ/rawrz/commit/19d6fa953b06ad3afa9a4ce655d556d4a7d459a5))
* **ci:** correct step output references and unit parsing in new workflows ([b03c135](https://github.com/WhispersOfJ/rawrz/commit/b03c135b8bdc22d7f1d43edb49900c3f553ea25e))
* **ci:** fix actionlint errors in cert-expiry-check, disk-cleanup, codeql ([f27e200](https://github.com/WhispersOfJ/rawrz/commit/f27e200e684c785d04fd42859473a79e140fe1d0))
* **ci:** install pyyaml in api-contract job; route PR title through env var ([d2352da](https://github.com/WhispersOfJ/rawrz/commit/d2352da6263989910cda5853b66a1465673cffe8))
* **ci:** pin actions to full commit SHAs (short SHAs are rejected) ([fc623a8](https://github.com/WhispersOfJ/rawrz/commit/fc623a8483f17f8ef2ccc8c864704a20b90b8d93))
* **ci:** prevent PR title script injection ([0e90af1](https://github.com/WhispersOfJ/rawrz/commit/0e90af126b3db9ba237b60370c00eacaea42244a))
* **ci:** skip release-please until RELEASE_PLEASE_TOKEN is configured ([05c8b7e](https://github.com/WhispersOfJ/rawrz/commit/05c8b7efd6a4e711295484483f6b6be10fa67534))
* classify destructive Plex butler tasks as danger ([#101](https://github.com/WhispersOfJ/rawrz/issues/101)) ([1ffd1a1](https://github.com/WhispersOfJ/rawrz/commit/1ffd1a1b9df4e052fee6ea255fb6fce914e153a9))
* clear *arr blocklists via the ClearBlocklist command ([#100](https://github.com/WhispersOfJ/rawrz/issues/100)) ([d1916bb](https://github.com/WhispersOfJ/rawrz/commit/d1916bb4e77c9b754ac91f12b6e3e41b3a7e0d12))
* close queue and import safety gaps ([#153](https://github.com/WhispersOfJ/rawrz/issues/153)) ([7c64b2a](https://github.com/WhispersOfJ/rawrz/commit/7c64b2a36d6dd0ab0134af76b3decc70d37e881f))
* **compose:** read node-exporter healthcheck body to stop log flood ([e4ed066](https://github.com/WhispersOfJ/rawrz/commit/e4ed066271a9052ae2d2efa543769e94e5f7d7fc))
* consolidate cave-path migration and host-runtime prep ([#159](https://github.com/WhispersOfJ/rawrz/issues/159)) ([e21c6b9](https://github.com/WhispersOfJ/rawrz/commit/e21c6b9b8534e4fa2e69ebee6eae98f1e58d6071))
* digest sonarr row passes --blob-table EpisodeFiles ([#117](https://github.com/WhispersOfJ/rawrz/issues/117)) ([e8b20ca](https://github.com/WhispersOfJ/rawrz/commit/e8b20cac793d3db98ccda7bfc7a13f9ed510505c))
* disable env-managed segment cache for the symlinks import strategy ([3310219](https://github.com/WhispersOfJ/rawrz/commit/3310219e10aa4c25207f41961384f641a0fdcb90))
* disable Eweka Usenet provider slot after server-side auth rejection ([#189](https://github.com/WhispersOfJ/rawrz/issues/189)) ([86aa25d](https://github.com/WhispersOfJ/rawrz/commit/86aa25d1d37419c2d2d4c8bca99e2d57f39895a5))
* fall back to GITHUB_TOKEN when release-please PAT is unset ([1952a08](https://github.com/WhispersOfJ/rawrz/commit/1952a084695b63555e7f10b74bd10300fa8c9400))
* **fish:** drop stray bash-style do; repair stack-help description matching ([a776913](https://github.com/WhispersOfJ/rawrz/commit/a776913e47e05edcfd60e7c8c9784f015911b90f))
* **fish:** fix broken __stack_api calls after control panel removal ([79058dd](https://github.com/WhispersOfJ/rawrz/commit/79058dd9fa335e675159d5fce7a6b5b453a33736))
* **fish:** guard stack-worktree against twin and stale worktrees ([#63](https://github.com/WhispersOfJ/rawrz/issues/63)) ([41b4409](https://github.com/WhispersOfJ/rawrz/commit/41b44098c6982622a3bfe3f3d798cab8310356c9))
* **fish:** implement all 17 custom integration functions + fix Plex health ([950f925](https://github.com/WhispersOfJ/rawrz/commit/950f925d48c3c58154a87d03f4ae0b0226947c4c))
* **fish:** remediate systemic breakage across all stack-* functions ([8689b62](https://github.com/WhispersOfJ/rawrz/commit/8689b62878dc0555cb9df6a18989f05ceb2bcf12))
* **fish:** sort stack-help families and hide retired command symlinks ([#59](https://github.com/WhispersOfJ/rawrz/issues/59)) ([670741e](https://github.com/WhispersOfJ/rawrz/commit/670741e1224ea706090584188ba0616255ab9641))
* **fish:** three bugs in Phase 1 direct-service routing ([37db0a8](https://github.com/WhispersOfJ/rawrz/commit/37db0a89a23754c45e3cbfa13cd905471629761d))
* **frontend:** complete React 19 migration and align plugin-react with vite 8 ([#19](https://github.com/WhispersOfJ/rawrz/issues/19)) ([8d4c313](https://github.com/WhispersOfJ/rawrz/commit/8d4c3132316c73134d85f5a33ddbc4807abbba96))
* guard cd in preflight.sh against failure ([a34439d](https://github.com/WhispersOfJ/rawrz/commit/a34439d7cacbe9907451e671e1cc5c36a092a853))
* handle null templating and escaped dollar vars in dashboard guard ([675e088](https://github.com/WhispersOfJ/rawrz/commit/675e088751c1a477bacec08af81ce4f961b10a78))
* health view covers 19 services, backlinks use absolute URLs, registry validation, docstring accuracy ([6ab8e5c](https://github.com/WhispersOfJ/rawrz/commit/6ab8e5c5cd88febea6d1242e51290757c1234095))
* **helper:** fix image/command order, input validation, error handling ([aef7108](https://github.com/WhispersOfJ/rawrz/commit/aef71084143fb9c824d32bba31fda73daa3ddc9a))
* installer retarget via --repo, entry detection, and exit-safe cleanup ([#111](https://github.com/WhispersOfJ/rawrz/issues/111)) ([e0022c7](https://github.com/WhispersOfJ/rawrz/commit/e0022c7d35151f34088095231f257260c0bec0b6))
* JSON slim violates History.Data NOT NULL; count '' payloads as slimmed ([#122](https://github.com/WhispersOfJ/rawrz/issues/122)) ([70ad4a2](https://github.com/WhispersOfJ/rawrz/commit/70ad4a2ade6cbea91cb1ba291e241ea514ee9bb6))
* keep digests in trivy image extraction and re-key baseline to pins ([d06c333](https://github.com/WhispersOfJ/rawrz/commit/d06c3338017600d91f0bd2dc9d455331276cc1db))
* **landing-page:** correct Mermaid graph edge node IDs ([c8efb03](https://github.com/WhispersOfJ/rawrz/commit/c8efb03f77227fdaa130df7bd6484fa22ea04438))
* **landing-page:** correct pipeline flow parallelism and guard Mermaid ([2753f11](https://github.com/WhispersOfJ/rawrz/commit/2753f110d5da679d954ce0b4201f135488da85a3))
* **landing-page:** repoint doc links from media-stack to thebearcave ([ff5c3c5](https://github.com/WhispersOfJ/rawrz/commit/ff5c3c56aa99b1134305bbf371a26d4236bd0e57))
* **landing-page:** same-origin health fetch fixes HTTPS status dots ([46caf92](https://github.com/WhispersOfJ/rawrz/commit/46caf92a05e22bb8119f2c33267017d7cb1fd6dc))
* make install.sh refuse worktrees and self-verify symlinks ([#65](https://github.com/WhispersOfJ/rawrz/issues/65)) ([a99f7f9](https://github.com/WhispersOfJ/rawrz/commit/a99f7f9f214c092225238e35b8ebd5ed9cca043e))
* move recyclarr off the always-on stack to the manual maintenance profile ([#158](https://github.com/WhispersOfJ/rawrz/issues/158)) ([9ae58d3](https://github.com/WhispersOfJ/rawrz/commit/9ae58d3fc3f4812e73a67fd806833542a943dbff))
* nightly compose-path gate skips gitignored host mounts ([#129](https://github.com/WhispersOfJ/rawrz/issues/129)) ([86183aa](https://github.com/WhispersOfJ/rawrz/commit/86183aafed8a7187a72029fbae8d7fc44c9c9632))
* **nzbdav:** point library dir at the real media root ([#138](https://github.com/WhispersOfJ/rawrz/issues/138)) ([84e5005](https://github.com/WhispersOfJ/rawrz/commit/84e5005a7ac2172345bd1553711c3bdf3408a548))
* pre-push hook must validate the pushing worktree, not main ([#84](https://github.com/WhispersOfJ/rawrz/issues/84)) ([f113f7b](https://github.com/WhispersOfJ/rawrz/commit/f113f7b099ed8c39fcbb1d2f84e2871c1ff77afe))
* prune stale symlinks in fish-functions install.sh ([#60](https://github.com/WhispersOfJ/rawrz/issues/60)) ([8045400](https://github.com/WhispersOfJ/rawrz/commit/80454008e656398f98c34195e9865dd6ce64ba4c))
* raise nzbdav_rclone memory limit to stop OOM-kill stale-mount cascade ([2cecb5c](https://github.com/WhispersOfJ/rawrz/commit/2cecb5c090641bb9fedfc82878eb19a470d25259))
* remediate full-codebase review findings (F-1 through F-39) ([ba49541](https://github.com/WhispersOfJ/rawrz/commit/ba49541676c365548cf7569f3669a36e01cd72b8))
* remove invalid size expression from PR labeler workflow ([6ab944b](https://github.com/WhispersOfJ/rawrz/commit/6ab944b5446c8f056757bec84cf4228355606620))
* remove unused variables flagged by shellcheck in health checks ([623432b](https://github.com/WhispersOfJ/rawrz/commit/623432b66f7d9484e405bcdbd9fa9d364d8ed0be))
* rename ambiguous loop variable to pass ruff E741 ([f130332](https://github.com/WhispersOfJ/rawrz/commit/f1303327ef9b43a17efb17d1b6e66f54aa8f1d11))
* resolve fish conf.d env loader repo path correctly ([49cc760](https://github.com/WhispersOfJ/rawrz/commit/49cc76055f2f6b2b5afdb8c664469fe7ffb16c80))
* resolve sonarr series titles in-python in stack-arr-missing-aired ([#91](https://github.com/WhispersOfJ/rawrz/issues/91)) ([de0a5be](https://github.com/WhispersOfJ/rawrz/commit/de0a5bef6d922572481293603457374725760ae5))
* restore /api/v3 prefix so drain_sonarr_queue reaches the Sonarr API ([#130](https://github.com/WhispersOfJ/rawrz/issues/130)) ([d794815](https://github.com/WhispersOfJ/rawrz/commit/d794815e4011ff4ade361fd4e5c69953e9e1e4fe))
* restore FRONTEND_BACKEND_API_KEY to .env.template ([2f8fabe](https://github.com/WhispersOfJ/rawrz/commit/2f8fabe585a8ab0be2021d6fc42feae78ab50ed4))
* **security:** eliminate 15 CRITICAL CVEs across the stack ([db5c97b](https://github.com/WhispersOfJ/rawrz/commit/db5c97bca3a278b8e709d3e02f10a6abadf4340f))
* **security:** harden stack per potential.md audit items ([4f4255c](https://github.com/WhispersOfJ/rawrz/commit/4f4255c8baa1eaa6e2ff42ce7bfc6422ac4f487c))
* **security:** remove writable Docker socket from control-panel (item [#1](https://github.com/WhispersOfJ/rawrz/issues/1)) ([4fe6321](https://github.com/WhispersOfJ/rawrz/commit/4fe6321f1766d5d811ed65d0eae2fd65a34ccd83))
* **security:** revert wrong image pins, recreate containers with resource limits ([fae4f3c](https://github.com/WhispersOfJ/rawrz/commit/fae4f3c677acb2547418a10d839545ab20627116))
* serialize stack-tui launches against rapid-toggle races ([#140](https://github.com/WhispersOfJ/rawrz/issues/140)) ([4ad4918](https://github.com/WhispersOfJ/rawrz/commit/4ad49181200dd9be0bee6ed04c009149af12d496))
* stop comment text truncating the rclone command in compose ([#160](https://github.com/WhispersOfJ/rawrz/issues/160)) ([f2ea7e3](https://github.com/WhispersOfJ/rawrz/commit/f2ea7e3de695dee27a610fdf018ca233c2ecc993))
* stop failed Let's Encrypt attempts on LAN stack ([ef2ab3c](https://github.com/WhispersOfJ/rawrz/commit/ef2ab3c6a2da4c6e0f7226b4eca48a35bdf75c4d))
* strip --force before forwarding in bash docker-guard snippet ([d5cf3ea](https://github.com/WhispersOfJ/rawrz/commit/d5cf3eaa9db3e56bcd667afaec819f3bcb54f017))
* sync secret manifest with workflow usage ([1aeda7f](https://github.com/WhispersOfJ/rawrz/commit/1aeda7fcc950dcb6debe52807a2c7bdc7bb1085d))
* sync update-nzbdav.sh dependents with compose cascade ([293523e](https://github.com/WhispersOfJ/rawrz/commit/293523e747e3ced9b9580cba9717f4dbcdbf2f44))
* **test:** use temp file to avoid pipefail issue with large HTML pages ([94defd8](https://github.com/WhispersOfJ/rawrz/commit/94defd8ace62ec3c79a4a3d517307b974b29bd73))
* tighten nzbdav guard over-matching and env loading under the timer ([8634eb3](https://github.com/WhispersOfJ/rawrz/commit/8634eb3cc622a978df2a7da885fa769ddc298ae8))
* use swayr for the waybar window switcher ([#161](https://github.com/WhispersOfJ/rawrz/issues/161)) ([816f044](https://github.com/WhispersOfJ/rawrz/commit/816f04474d56d65005d83715733967b703315934))


### Performance Improvements

* batch docker inspect and parallelize FUSE probes in mount-drift check ([#68](https://github.com/WhispersOfJ/rawrz/issues/68)) ([ee29869](https://github.com/WhispersOfJ/rawrz/commit/ee29869f9506133a70d4a9bfc0cb70f5781a8096))
* gzip compression + lazy-load Mermaid for landing page ([ea70f4d](https://github.com/WhispersOfJ/rawrz/commit/ea70f4d57ad1b2d72d3b3045c7e7d1d494b1e015))
* plan --all missing searches per series, not wanted pagination ([#137](https://github.com/WhispersOfJ/rawrz/issues/137)) ([15b45d9](https://github.com/WhispersOfJ/rawrz/commit/15b45d96ff624aaae386541380038a8c0203fdf2))
* route rclone WebDAV directly to the internal nzbdav backend ([d223ee6](https://github.com/WhispersOfJ/rawrz/commit/d223ee69f18cc1b27afe7c194fc5ef0826f2e158))
* tune nzbdav_rclone for library-wide analysis load ([#74](https://github.com/WhispersOfJ/rawrz/issues/74)) ([43b400c](https://github.com/WhispersOfJ/rawrz/commit/43b400cdc3e6740e7d63d710486171fc5af50509))
* un-saturate the rclone vfs disk cache and drop cookie handling ([adb24a9](https://github.com/WhispersOfJ/rawrz/commit/adb24a976654e7b82ed2af00e6ce4fc4c383bec2))

## [1.35.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.34.0...v1.35.0) (2026-09-06)


### Features

* add D11 safe-command registry and live matrix runner in tests/live ([#187](https://github.com/WhispersOfJ/thebearcave/issues/187)) ([b0658c8](https://github.com/WhispersOfJ/thebearcave/commit/b0658c847249af1884e1b85c2733ab987e1f2c1d))

## [1.34.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.33.0...v1.34.0) (2026-09-05)


### Features

* add nzbdav backend wedge watchdog (probe, queue-safe restart, Discord alerts) ([0ca1aa2](https://github.com/WhispersOfJ/thebearcave/commit/0ca1aa208939596b53281bfb25a307170ff28386))

## [1.33.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.32.0...v1.33.0) (2026-09-04)


### Features

* add waybar idle-lock pause module (presentation mode) ([#164](https://github.com/WhispersOfJ/thebearcave/issues/164)) ([73aa6a3](https://github.com/WhispersOfJ/thebearcave/commit/73aa6a32e538c89d876302b054fbc433657f1cf0))
* add waybar recording and night-light status indicators ([#165](https://github.com/WhispersOfJ/thebearcave/issues/165)) ([d32c056](https://github.com/WhispersOfJ/thebearcave/commit/d32c0563eb36518d9164452add45fdf45fb07ad3))
* scale waybar bar up 40% ([#163](https://github.com/WhispersOfJ/thebearcave/issues/163)) ([c2878c7](https://github.com/WhispersOfJ/thebearcave/commit/c2878c786a23d9336917afdf1d072f27e8e6ea2d))


### Bug Fixes

* disable env-managed segment cache for the symlinks import strategy ([3310219](https://github.com/WhispersOfJ/thebearcave/commit/3310219e10aa4c25207f41961384f641a0fdcb90))
* use swayr for the waybar window switcher ([#161](https://github.com/WhispersOfJ/thebearcave/issues/161)) ([816f044](https://github.com/WhispersOfJ/thebearcave/commit/816f04474d56d65005d83715733967b703315934))


### Performance Improvements

* route rclone WebDAV directly to the internal nzbdav backend ([d223ee6](https://github.com/WhispersOfJ/thebearcave/commit/d223ee69f18cc1b27afe7c194fc5ef0826f2e158))
* un-saturate the rclone vfs disk cache and drop cookie handling ([adb24a9](https://github.com/WhispersOfJ/thebearcave/commit/adb24a976654e7b82ed2af00e6ce4fc4c383bec2))

## [1.32.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.31.1...v1.32.0) (2026-09-04)


### Features

* add recyclarr service for TRaSH-Guides profile sync ([#156](https://github.com/WhispersOfJ/thebearcave/issues/156)) ([d9c30fd](https://github.com/WhispersOfJ/thebearcave/commit/d9c30fd3a960861b4cf68488963d0ee1a78d4562))


### Bug Fixes

* consolidate cave-path migration and host-runtime prep ([#159](https://github.com/WhispersOfJ/thebearcave/issues/159)) ([e21c6b9](https://github.com/WhispersOfJ/thebearcave/commit/e21c6b9b8534e4fa2e69ebee6eae98f1e58d6071))
* move recyclarr off the always-on stack to the manual maintenance profile ([#158](https://github.com/WhispersOfJ/thebearcave/issues/158)) ([9ae58d3](https://github.com/WhispersOfJ/thebearcave/commit/9ae58d3fc3f4812e73a67fd806833542a943dbff))
* stop comment text truncating the rclone command in compose ([#160](https://github.com/WhispersOfJ/thebearcave/issues/160)) ([f2ea7e3](https://github.com/WhispersOfJ/thebearcave/commit/f2ea7e3de695dee27a610fdf018ca233c2ecc993))

## [1.31.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.31.0...v1.31.1) (2026-09-04)


### Bug Fixes

* close queue and import safety gaps ([#153](https://github.com/WhispersOfJ/thebearcave/issues/153)) ([7c64b2a](https://github.com/WhispersOfJ/thebearcave/commit/7c64b2a36d6dd0ab0134af76b3decc70d37e881f))

## [1.31.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.30.0...v1.31.0) (2026-09-04)


### Features

* streaming Dropbox backup of the repo (no media/metadata/secrets) ([#152](https://github.com/WhispersOfJ/thebearcave/issues/152)) ([137fcec](https://github.com/WhispersOfJ/thebearcave/commit/137fcec014ff144da96656d8625b10cc211f4e2b))
* watchable view, request arrival notifier, and activity feed (TODO [#6](https://github.com/WhispersOfJ/thebearcave/issues/6)-[#8](https://github.com/WhispersOfJ/thebearcave/issues/8)) ([#149](https://github.com/WhispersOfJ/thebearcave/issues/149)) ([4a27606](https://github.com/WhispersOfJ/thebearcave/commit/4a27606c464563c7e7278f12fc5e11724b81d70d))

## [1.30.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.29.0...v1.30.0) (2026-09-03)


### Features

* add bazarr.db to the maintenance digest DB gate ([#146](https://github.com/WhispersOfJ/thebearcave/issues/146)) ([466e776](https://github.com/WhispersOfJ/thebearcave/commit/466e776878a726b481e500fd4c3d8ffde2eb8449))
* DB growth-trend predictor turns bloat incidents into prune dates ([#147](https://github.com/WhispersOfJ/thebearcave/issues/147)) ([da16551](https://github.com/WhispersOfJ/thebearcave/commit/da16551c8ffd0e1ecd945fe9cf5def30865f7544))

## [1.29.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.28.0...v1.29.0) (2026-09-03)


### Features

* mine grab history for Sonarr alias candidates ([#142](https://github.com/WhispersOfJ/thebearcave/issues/142)) ([91bca5c](https://github.com/WhispersOfJ/thebearcave/commit/91bca5c945e1b3457ee04c42b454eb3fedafe398))
* re-adopt Bazarr as the 9th always-on service ([#144](https://github.com/WhispersOfJ/thebearcave/issues/144)) ([c208bdd](https://github.com/WhispersOfJ/thebearcave/commit/c208bddf35d8d9bced26a48ce205594c36d36e95))

## [1.28.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.27.0...v1.28.0) (2026-09-03)


### Features

* add --auto-safe drain mode that imports only provably-correct queue items ([#135](https://github.com/WhispersOfJ/thebearcave/issues/135)) ([4d70a0d](https://github.com/WhispersOfJ/thebearcave/commit/4d70a0dfca1f08693d0104bc07a15fbd94b63540))
* extend the queue drain to Radarr with an --app flag ([#132](https://github.com/WhispersOfJ/thebearcave/issues/132)) ([0f36ccb](https://github.com/WhispersOfJ/thebearcave/commit/0f36ccb92a754c51e9a3566546e3140ff013c87b))
* flag *arr import-queue pile-ups in the maintenance digest ([#133](https://github.com/WhispersOfJ/thebearcave/issues/133)) ([16f01f6](https://github.com/WhispersOfJ/thebearcave/commit/16f01f65c58f709c0178797b6f5729a12003f43a))
* scoped missing-search wrapper replacing blind whole-series sweeps ([#136](https://github.com/WhispersOfJ/thebearcave/issues/136)) ([65cbc3e](https://github.com/WhispersOfJ/thebearcave/commit/65cbc3e4622bb34d96ab32b051c8ce3b96d34ae0))
* track the waybar stack-tui launcher module as dotfiles ([#139](https://github.com/WhispersOfJ/thebearcave/issues/139)) ([0f60f1b](https://github.com/WhispersOfJ/thebearcave/commit/0f60f1b90688983e5d08fd40150f0ff3f6b628b9))


### Bug Fixes

* **nzbdav:** point library dir at the real media root ([#138](https://github.com/WhispersOfJ/thebearcave/issues/138)) ([84e5005](https://github.com/WhispersOfJ/thebearcave/commit/84e5005a7ac2172345bd1553711c3bdf3408a548))
* restore /api/v3 prefix so drain_sonarr_queue reaches the Sonarr API ([#130](https://github.com/WhispersOfJ/thebearcave/issues/130)) ([d794815](https://github.com/WhispersOfJ/thebearcave/commit/d794815e4011ff4ade361fd4e5c69953e9e1e4fe))
* serialize stack-tui launches against rapid-toggle races ([#140](https://github.com/WhispersOfJ/thebearcave/issues/140)) ([4ad4918](https://github.com/WhispersOfJ/thebearcave/commit/4ad49181200dd9be0bee6ed04c009149af12d496))


### Performance Improvements

* plan --all missing searches per series, not wanted pagination ([#137](https://github.com/WhispersOfJ/thebearcave/issues/137)) ([15b45d9](https://github.com/WhispersOfJ/thebearcave/commit/15b45d96ff624aaae386541380038a8c0203fdf2))

## [1.27.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.26.0...v1.27.0) (2026-09-02)


### Features

* drift reports name the recreate and how stale the pins are ([#126](https://github.com/WhispersOfJ/thebearcave/issues/126)) ([6e5fe99](https://github.com/WhispersOfJ/thebearcave/commit/6e5fe994d5fed8d2537d6ca6f74b21e1a8d9f601))


### Bug Fixes

* nightly compose-path gate skips gitignored host mounts ([#129](https://github.com/WhispersOfJ/thebearcave/issues/129)) ([86183aa](https://github.com/WhispersOfJ/thebearcave/commit/86183aafed8a7187a72029fbae8d7fc44c9c9632))

## [1.26.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.25.0...v1.26.0) (2026-09-02)


### Features

* add stack-audit-residue for retired-service/path residue ([#116](https://github.com/WhispersOfJ/thebearcave/issues/116)) ([2e65f94](https://github.com/WhispersOfJ/thebearcave/commit/2e65f94c1d2728c028fb98d7129f8966ecf50f64))
* add stack-maintenance-digest — verify nightly maintenance ran ([#113](https://github.com/WhispersOfJ/thebearcave/issues/113)) ([4c892dc](https://github.com/WhispersOfJ/thebearcave/commit/4c892dc9bcb2b7866f20425c77079268b9f781ce))
* add stack-sonarr-prune for sonarr.db MediaInfo bloat ([#114](https://github.com/WhispersOfJ/thebearcave/issues/114)) ([7fd2009](https://github.com/WhispersOfJ/thebearcave/commit/7fd200920e7b54675a0b587d0ef8200680014827))
* config-drift guard — running container images vs compose pins ([#124](https://github.com/WhispersOfJ/thebearcave/issues/124)) ([f57a5e4](https://github.com/WhispersOfJ/thebearcave/commit/f57a5e4ddea7d42b6a55f43d1c50e07001d8aa2c))
* digest gains a config-drift surface (running images vs pins) ([#125](https://github.com/WhispersOfJ/thebearcave/issues/125)) ([7989c0f](https://github.com/WhispersOfJ/thebearcave/commit/7989c0f1e7eeee50dad6508fd314e70818b7ae6a))
* digest gains a full-host residue audit surface ([#119](https://github.com/WhispersOfJ/thebearcave/issues/119)) ([51c3cb9](https://github.com/WhispersOfJ/thebearcave/commit/51c3cb92778d76564c07550f43e7daf70e28343b))
* digest verifies the monthly sonarr prune log (fresh + exit 0) ([#121](https://github.com/WhispersOfJ/thebearcave/issues/121)) ([63539e2](https://github.com/WhispersOfJ/thebearcave/commit/63539e230ea4857c5b9135bde8b989a1638ed857))
* preflight sonarr db size leg (EpisodeFiles MediaInfo gate) ([#118](https://github.com/WhispersOfJ/thebearcave/issues/118)) ([ce53597](https://github.com/WhispersOfJ/thebearcave/commit/ce535974ce0619d33491a3ec4d60c797c1e12406))
* sonarr prune also slims event-table JSON payloads ([#120](https://github.com/WhispersOfJ/thebearcave/issues/120)) ([c267eaf](https://github.com/WhispersOfJ/thebearcave/commit/c267eaf53d8bfdc0933aea87e192deb98077ef84))


### Bug Fixes

* bump gitleaks pre-commit hook past the Go 1.24 wasm panic ([#123](https://github.com/WhispersOfJ/thebearcave/issues/123)) ([e23e31b](https://github.com/WhispersOfJ/thebearcave/commit/e23e31bb3176765abe25f97c34ec46f561e808ac))
* digest sonarr row passes --blob-table EpisodeFiles ([#117](https://github.com/WhispersOfJ/thebearcave/issues/117)) ([e8b20ca](https://github.com/WhispersOfJ/thebearcave/commit/e8b20cac793d3db98ccda7bfc7a13f9ed510505c))
* JSON slim violates History.Data NOT NULL; count '' payloads as slimmed ([#122](https://github.com/WhispersOfJ/thebearcave/issues/122)) ([70ad4a2](https://github.com/WhispersOfJ/thebearcave/commit/70ad4a2ade6cbea91cb1ba291e241ea514ee9bb6))

## [1.25.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.24.0...v1.25.0) (2026-09-02)


### Features

* add idempotent nightly stack-disk-reclaim cron installer ([#108](https://github.com/WhispersOfJ/thebearcave/issues/108)) ([47f3fd2](https://github.com/WhispersOfJ/thebearcave/commit/47f3fd21d03343068dbde36d7c28e38adaf8ea12))


### Bug Fixes

* installer retarget via --repo, entry detection, and exit-safe cleanup ([#111](https://github.com/WhispersOfJ/thebearcave/issues/111)) ([e0022c7](https://github.com/WhispersOfJ/thebearcave/commit/e0022c7d35151f34088095231f257260c0bec0b6))

## [1.24.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.23.0...v1.24.0) (2026-09-02)


### Features

* add guarded radarr.db bloat prune maintenance path ([#103](https://github.com/WhispersOfJ/thebearcave/issues/103)) ([836611b](https://github.com/WhispersOfJ/thebearcave/commit/836611bbb040f965085189cfb593116d919a4c83))
* add stack-disk-reclaim for nightly Docker disk reclamation ([#106](https://github.com/WhispersOfJ/thebearcave/issues/106)) ([a48d367](https://github.com/WhispersOfJ/thebearcave/commit/a48d367a68ad164a400fda0608dd27136c5a5b7a))

## [1.23.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.22.0...v1.23.0) (2026-09-01)


### Features

* add Eweka as third Usenet provider slot ([#97](https://github.com/WhispersOfJ/thebearcave/issues/97)) ([8a5f610](https://github.com/WhispersOfJ/thebearcave/commit/8a5f610468ced58f58b1253f3cfe02e854471295))
* add stack-tui floating-window launcher for the stack-* functions ([#95](https://github.com/WhispersOfJ/thebearcave/issues/95)) ([4aecdcd](https://github.com/WhispersOfJ/thebearcave/commit/4aecdcdfaf590964970c4bbd67a425d3b01586fe))
* warn at shell load when a pre-set arr key differs from .env ([#99](https://github.com/WhispersOfJ/thebearcave/issues/99)) ([4bf310d](https://github.com/WhispersOfJ/thebearcave/commit/4bf310de3ce62a0da4ea547964ab33a3d1501254))


### Bug Fixes

* __arr_api_key error names the real uppercase var ([#98](https://github.com/WhispersOfJ/thebearcave/issues/98)) ([44e54ed](https://github.com/WhispersOfJ/thebearcave/commit/44e54ede570aa2462821795c0fb13466df249e39))
* cap __stack_containers docker call with a timeout ([#92](https://github.com/WhispersOfJ/thebearcave/issues/92)) ([50b5adf](https://github.com/WhispersOfJ/thebearcave/commit/50b5adf8816120bcd1bd1cfef8235fa647855782))
* classify destructive Plex butler tasks as danger ([#101](https://github.com/WhispersOfJ/thebearcave/issues/101)) ([1ffd1a1](https://github.com/WhispersOfJ/thebearcave/commit/1ffd1a1b9df4e052fee6ea255fb6fce914e153a9))
* clear *arr blocklists via the ClearBlocklist command ([#100](https://github.com/WhispersOfJ/thebearcave/issues/100)) ([d1916bb](https://github.com/WhispersOfJ/thebearcave/commit/d1916bb4e77c9b754ac91f12b6e3e41b3a7e0d12))
* resolve sonarr series titles in-python in stack-arr-missing-aired ([#91](https://github.com/WhispersOfJ/thebearcave/issues/91)) ([de0a5be](https://github.com/WhispersOfJ/thebearcave/commit/de0a5bef6d922572481293603457374725760ae5))

## [1.22.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.21.0...v1.22.0) (2026-09-01)


### Features

* add bash port of the stack-* CLI library ([#85](https://github.com/WhispersOfJ/thebearcave/issues/85)) ([adebd0d](https://github.com/WhispersOfJ/thebearcave/commit/adebd0d41d974c00f7eeb227bb443ce5d09505cf))
* add per-call-type API timeouts so no stack-* command can hang forever ([#86](https://github.com/WhispersOfJ/thebearcave/issues/86)) ([9d3c935](https://github.com/WhispersOfJ/thebearcave/commit/9d3c935435387d8d951126defd095ae3222e3614))

## [1.21.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.20.0...v1.21.0) (2026-08-31)


### Features

* add pre-push git hook running the preflight gate ([#83](https://github.com/WhispersOfJ/thebearcave/issues/83)) ([67498e8](https://github.com/WhispersOfJ/thebearcave/commit/67498e8832debceffdd04a6e63ff0af9a75f350f))
* add stack-plex-markers read-only marker audit command ([#76](https://github.com/WhispersOfJ/thebearcave/issues/76)) ([cf27366](https://github.com/WhispersOfJ/thebearcave/commit/cf273664d6f62b82bc87129a45aba742a285ba26))


### Bug Fixes

* pre-push hook must validate the pushing worktree, not main ([#84](https://github.com/WhispersOfJ/thebearcave/issues/84)) ([f113f7b](https://github.com/WhispersOfJ/thebearcave/commit/f113f7b099ed8c39fcbb1d2f84e2871c1ff77afe))

## [1.20.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.19.2...v1.20.0) (2026-08-31)


### Features

* add stack-radarr-health on-demand Radarr DB integrity check ([#73](https://github.com/WhispersOfJ/thebearcave/issues/73)) ([5950b38](https://github.com/WhispersOfJ/thebearcave/commit/5950b387f1184a94d19e1f42525b2513feec3235))


### Performance Improvements

* batch docker inspect and parallelize FUSE probes in mount-drift check ([#68](https://github.com/WhispersOfJ/thebearcave/issues/68)) ([ee29869](https://github.com/WhispersOfJ/thebearcave/commit/ee29869f9506133a70d4a9bfc0cb70f5781a8096))
* tune nzbdav_rclone for library-wide analysis load ([#74](https://github.com/WhispersOfJ/thebearcave/issues/74)) ([43b400c](https://github.com/WhispersOfJ/thebearcave/commit/43b400cdc3e6740e7d63d710486171fc5af50509))

## [1.19.2](https://github.com/WhispersOfJ/thebearcave/compare/v1.19.1...v1.19.2) (2026-08-31)


### Bug Fixes

* bound install verification with a timeout on the stack-help check ([#66](https://github.com/WhispersOfJ/thebearcave/issues/66)) ([026082e](https://github.com/WhispersOfJ/thebearcave/commit/026082eff6720a7c1b8f6bbb41c261af78c46a08))

## [1.19.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.19.0...v1.19.1) (2026-08-31)


### Bug Fixes

* **fish:** guard stack-worktree against twin and stale worktrees ([#63](https://github.com/WhispersOfJ/thebearcave/issues/63)) ([41b4409](https://github.com/WhispersOfJ/thebearcave/commit/41b44098c6982622a3bfe3f3d798cab8310356c9))
* make install.sh refuse worktrees and self-verify symlinks ([#65](https://github.com/WhispersOfJ/thebearcave/issues/65)) ([a99f7f9](https://github.com/WhispersOfJ/thebearcave/commit/a99f7f9f214c092225238e35b8ebd5ed9cca043e))

## [1.19.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.18.0...v1.19.0) (2026-08-31)


### Features

* add stack-worktree helper for task-named worktrees ([#55](https://github.com/WhispersOfJ/thebearcave/issues/55)) ([68c5fa2](https://github.com/WhispersOfJ/thebearcave/commit/68c5fa2b32e899dc12cc96e7edcb51d22073a11b))
* promote Sonarr queue drain into scripts/drain_sonarr_queue.py ([#58](https://github.com/WhispersOfJ/thebearcave/issues/58)) ([ffa4b7a](https://github.com/WhispersOfJ/thebearcave/commit/ffa4b7ab84546676221f43afa2cbe3ef48f3f596))


### Bug Fixes

* **fish:** sort stack-help families and hide retired command symlinks ([#59](https://github.com/WhispersOfJ/thebearcave/issues/59)) ([670741e](https://github.com/WhispersOfJ/thebearcave/commit/670741e1224ea706090584188ba0616255ab9641))

## [1.18.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.17.1...v1.18.0) (2026-08-31)


### Features

* slim stack and add manual Plex cache maintenance ([08d195d](https://github.com/WhispersOfJ/thebearcave/commit/08d195d366629e3ab2fadd2cd77004dd5a7a671f))

## [1.17.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.17.0...v1.17.1) (2026-08-30)


### Bug Fixes

* tighten nzbdav guard over-matching and env loading under the timer ([8634eb3](https://github.com/WhispersOfJ/thebearcave/commit/8634eb3cc622a978df2a7da885fa769ddc298ae8))

## [1.17.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.16.2...v1.17.0) (2026-08-30)


### Features

* guard against bind-mount file staleness (landmine [#1](https://github.com/WhispersOfJ/thebearcave/issues/1)) ([36876ea](https://github.com/WhispersOfJ/thebearcave/commit/36876eacbce86ea7f8c02f37d5f5a89d200d460e))
* guard nzbdav recreate against the non-persistent queue landmine ([dbfe5a3](https://github.com/WhispersOfJ/thebearcave/commit/dbfe5a3a7295bcbc5979cc0da6c36dea7d0e5097))
* wire nzbdav queue guard into fish and bash docker compose wrappers ([9eda93c](https://github.com/WhispersOfJ/thebearcave/commit/9eda93c1574ee51c9294ca85a3bac64603791817))


### Bug Fixes

* strip --force before forwarding in bash docker-guard snippet ([d5cf3ea](https://github.com/WhispersOfJ/thebearcave/commit/d5cf3eaa9db3e56bcd667afaec819f3bcb54f017))

## [1.16.2](https://github.com/WhispersOfJ/thebearcave/compare/v1.16.1...v1.16.2) (2026-08-30)


### Bug Fixes

* handle null templating and escaped dollar vars in dashboard guard ([675e088](https://github.com/WhispersOfJ/thebearcave/commit/675e088751c1a477bacec08af81ce4f961b10a78))

## [1.16.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.16.0...v1.16.1) (2026-08-30)


### Bug Fixes

* auto-load stack .env into fish so stack-* commands reach the services ([f819898](https://github.com/WhispersOfJ/thebearcave/commit/f819898b06426f7fe75c1f0c0d3f04bc535c03ac))
* raise nzbdav_rclone memory limit to stop OOM-kill stale-mount cascade ([2cecb5c](https://github.com/WhispersOfJ/thebearcave/commit/2cecb5c090641bb9fedfc82878eb19a470d25259))
* resolve fish conf.d env loader repo path correctly ([49cc760](https://github.com/WhispersOfJ/thebearcave/commit/49cc76055f2f6b2b5afdb8c664469fe7ffb16c80))

## [1.16.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.15.0...v1.16.0) (2026-08-29)


### Features

* wire MCP baseline alerting, preflight gate, and unpackerr connections ([a92fe8a](https://github.com/WhispersOfJ/thebearcave/commit/a92fe8af374dd2cee79ea0d6a155429e8312122e))


### Bug Fixes

* guard cd in preflight.sh against failure ([a34439d](https://github.com/WhispersOfJ/thebearcave/commit/a34439d7cacbe9907451e671e1cc5c36a092a853))

## [1.15.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.14.0...v1.15.0) (2026-08-29)


### Features

* add MCP baseline comparison and refresh workflow ([a261051](https://github.com/WhispersOfJ/thebearcave/commit/a261051de05277818e595a933d9805aacc9216b6))


### Bug Fixes

* rename ambiguous loop variable to pass ruff E741 ([f130332](https://github.com/WhispersOfJ/thebearcave/commit/f1303327ef9b43a17efb17d1b6e66f54aa8f1d11))

## [1.14.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.13.0...v1.14.0) (2026-08-29)


### Features

* add Traefik dashboard link to landing page ([ffb56ac](https://github.com/WhispersOfJ/thebearcave/commit/ffb56ac4537f0dd46e62ec17ca80b86ea2740f43))
* deploy 9 new stack services with crowdsec bouncer and nzbdav categories ([3d25969](https://github.com/WhispersOfJ/thebearcave/commit/3d2596909c1daa7051bbca99e7c920d40cf7c3e2))
* switch landing page dashboard links to HTTPS nip.io hostnames ([69b18b7](https://github.com/WhispersOfJ/thebearcave/commit/69b18b78c2daba25b101e03bd722d299532e3ad7))
* update landing page with new services, health probes, and links ([b2bc2a8](https://github.com/WhispersOfJ/thebearcave/commit/b2bc2a8651692002261854b442856a60889d9cd9))


### Bug Fixes

* add missing ADGUARD_ADMIN_* vars to .env.template ([64ec98c](https://github.com/WhispersOfJ/thebearcave/commit/64ec98c764bbddf7544175734747128d1cdcc617))
* keep digests in trivy image extraction and re-key baseline to pins ([d06c333](https://github.com/WhispersOfJ/thebearcave/commit/d06c3338017600d91f0bd2dc9d455331276cc1db))
* sync update-nzbdav.sh dependents with compose cascade ([293523e](https://github.com/WhispersOfJ/thebearcave/commit/293523e747e3ced9b9580cba9717f4dbcdbf2f44))

## [1.13.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.12.1...v1.13.0) (2026-08-29)


### Features

* add audiobook and comic servers ([2d21c89](https://github.com/WhispersOfJ/thebearcave/commit/2d21c89612aab27c76481a201407c9d80dc71513))
* add Bazarr subtitle service ([8be99d4](https://github.com/WhispersOfJ/thebearcave/commit/8be99d44aa3b965b90fd7113ba6d52c9e774348f))
* add Lidarr and Readarr services ([871643c](https://github.com/WhispersOfJ/thebearcave/commit/871643c45122341946682a1f909fc13fed556fc6))
* add network security services ([879282a](https://github.com/WhispersOfJ/thebearcave/commit/879282a272c2ea4a2776e5a39ff23bc635c4b20f))
* add Vaultwarden and n8n services ([4e6f494](https://github.com/WhispersOfJ/thebearcave/commit/4e6f494a3f0fcf42d394522901bd643e25c4a096))


### Bug Fixes

* sync secret manifest with workflow usage ([1aeda7f](https://github.com/WhispersOfJ/thebearcave/commit/1aeda7fcc950dcb6debe52807a2c7bdc7bb1085d))

## [1.12.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.12.0...v1.12.1) (2026-08-28)


### Bug Fixes

* **compose:** read node-exporter healthcheck body to stop log flood ([e4ed066](https://github.com/WhispersOfJ/thebearcave/commit/e4ed066271a9052ae2d2efa543769e94e5f7d7fc))
* **security:** eliminate 15 CRITICAL CVEs across the stack ([db5c97b](https://github.com/WhispersOfJ/thebearcave/commit/db5c97bca3a278b8e709d3e02f10a6abadf4340f))

## [1.12.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.11.0...v1.12.0) (2026-08-28)


### Features

* **fish:** add generated tab completions for all stack-* commands ([9a3c154](https://github.com/WhispersOfJ/thebearcave/commit/9a3c1544bc14748f36df6d331f32087a674515b4))


### Bug Fixes

* **fish:** drop stray bash-style do; repair stack-help description matching ([a776913](https://github.com/WhispersOfJ/thebearcave/commit/a776913e47e05edcfd60e7c8c9784f015911b90f))
* **fish:** remediate systemic breakage across all stack-* functions ([8689b62](https://github.com/WhispersOfJ/thebearcave/commit/8689b62878dc0555cb9df6a18989f05ceb2bcf12))

## [1.11.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.10.1...v1.11.0) (2026-08-28)


### Features

* **nzbdav:** track InfiniDysk dev tag and add queue-guarded update script ([23a4e2a](https://github.com/WhispersOfJ/thebearcave/commit/23a4e2a003db6e0aa6383049366f31bacc2bab43))
* **plex:** use official beta image ([e3323c3](https://github.com/WhispersOfJ/thebearcave/commit/e3323c310568315cacf3650588709beb1cf2d292))

## [1.10.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.10.0...v1.10.1) (2026-08-27)


### Bug Fixes

* **landing-page:** correct Mermaid graph edge node IDs ([c8efb03](https://github.com/WhispersOfJ/thebearcave/commit/c8efb03f77227fdaa130df7bd6484fa22ea04438))
* **security:** revert wrong image pins, recreate containers with resource limits ([fae4f3c](https://github.com/WhispersOfJ/thebearcave/commit/fae4f3c677acb2547418a10d839545ab20627116))

## [1.10.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.9.1...v1.10.0) (2026-08-27)


### Features

* **monitoring:** add backup freshness, disk fill rate, and load alerts ([#39](https://github.com/WhispersOfJ/thebearcave/issues/39)) ([e5f924c](https://github.com/WhispersOfJ/thebearcave/commit/e5f924c9df00781b9cf00bd1edf7e5097e1cf27d))
* **security:** add resource limits to all 22 services ([#5](https://github.com/WhispersOfJ/thebearcave/issues/5)) ([fb3b43a](https://github.com/WhispersOfJ/thebearcave/commit/fb3b43a8ef32e3afff70c6d549a6fc39891b9fed))


### Bug Fixes

* **alerts:** correct BackupStale metric and HostHighLoad PromQL ([39e7030](https://github.com/WhispersOfJ/thebearcave/commit/39e703099437b9d3d3bf5370e8caab0490f9e44b))

## [1.9.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.9.0...v1.9.1) (2026-08-27)


### Bug Fixes

* **ci:** add actionlint checksum verification + pre-commit gate + docs ([aebec8d](https://github.com/WhispersOfJ/thebearcave/commit/aebec8de2278e8fe50d3eb9ed9b8133397acb8d8))
* **ci:** add pytest to exporter test dependencies ([16d1ab5](https://github.com/WhispersOfJ/thebearcave/commit/16d1ab53c0feb0f4eed51729961b75630686ad38))
* **ci:** correct actionlint SHA-256 checksum ([19d6fa9](https://github.com/WhispersOfJ/thebearcave/commit/19d6fa953b06ad3afa9a4ce655d556d4a7d459a5))
* **security:** harden stack per potential.md audit items ([4f4255c](https://github.com/WhispersOfJ/thebearcave/commit/4f4255c8baa1eaa6e2ff42ce7bfc6422ac4f487c))

## [1.9.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.8.0...v1.9.0) (2026-08-27)


### Features

* **fish:** Phase 2 — rewrite 71 fish functions to bypass control panel ([c4e12f1](https://github.com/WhispersOfJ/thebearcave/commit/c4e12f1f76c691b6382d98069d34b2a96cac8aa7))
* **landing:** Phase 3 — replace control-panel health with direct service probes ([3efae51](https://github.com/WhispersOfJ/thebearcave/commit/3efae5188a4b9ac551b7770fa5b940e275bffd3f))
* Phase 4 — remove control panel, archive to archive/control-panel/ ([faf4127](https://github.com/WhispersOfJ/thebearcave/commit/faf41274be6a30a610b1e812284fef526a16d338))


### Bug Fixes

* **ci:** add DISCORD_WEBHOOK_URL to secret manifest ([54dc06e](https://github.com/WhispersOfJ/thebearcave/commit/54dc06e46c8c99aaad48f1c683752e5daf788a75))
* **ci:** add missing step id and fix reclaim accumulation in disk-cleanup ([3798e6c](https://github.com/WhispersOfJ/thebearcave/commit/3798e6c57814240ccaf8452af4309e4a81c22e20))
* **ci:** correct step output references and unit parsing in new workflows ([b03c135](https://github.com/WhispersOfJ/thebearcave/commit/b03c135b8bdc22d7f1d43edb49900c3f553ea25e))
* **ci:** fix actionlint errors in cert-expiry-check, disk-cleanup, codeql ([f27e200](https://github.com/WhispersOfJ/thebearcave/commit/f27e200e684c785d04fd42859473a79e140fe1d0))
* **fish:** fix broken __stack_api calls after control panel removal ([79058dd](https://github.com/WhispersOfJ/thebearcave/commit/79058dd9fa335e675159d5fce7a6b5b453a33736))
* **fish:** implement all 17 custom integration functions + fix Plex health ([950f925](https://github.com/WhispersOfJ/thebearcave/commit/950f925d48c3c58154a87d03f4ae0b0226947c4c))
* **fish:** three bugs in Phase 1 direct-service routing ([37db0a8](https://github.com/WhispersOfJ/thebearcave/commit/37db0a89a23754c45e3cbfa13cd905471629761d))
* restore FRONTEND_BACKEND_API_KEY to .env.template ([2f8fabe](https://github.com/WhispersOfJ/thebearcave/commit/2f8fabe585a8ab0be2021d6fc42feae78ab50ed4))

## [1.8.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.7.0...v1.8.0) (2026-08-27)


### Features

* **fish:** Phase 1 — direct service API helpers, bypass control panel ([a78c8bf](https://github.com/WhispersOfJ/thebearcave/commit/a78c8bf0258b9777727d5a3c78ab123315e2cd1e))


### Bug Fixes

* **helper:** fix image/command order, input validation, error handling ([aef7108](https://github.com/WhispersOfJ/thebearcave/commit/aef71084143fb9c824d32bba31fda73daa3ddc9a))
* **security:** remove writable Docker socket from control-panel (item [#1](https://github.com/WhispersOfJ/thebearcave/issues/1)) ([4fe6321](https://github.com/WhispersOfJ/thebearcave/commit/4fe6321f1766d5d811ed65d0eae2fd65a34ccd83))

## [1.7.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.6.0...v1.7.0) (2026-08-27)


### Features

* **security:** harden control panel + fix no-op Trivy gate ([44f97ae](https://github.com/WhispersOfJ/thebearcave/commit/44f97ae17e936bd12c58b1a9aea1e790d293d4c2))

## [1.6.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.5.0...v1.6.0) (2026-08-27)


### Features

* Alertmanager with Discord notifications + Cleanuparr setup docs ([18861dc](https://github.com/WhispersOfJ/thebearcave/commit/18861dcd321cf5956908b8bce84c5328378eb419))
* **cli:** add /api/v2/cli/ endpoints for fish functions ([32f475c](https://github.com/WhispersOfJ/thebearcave/commit/32f475c04655daff966bbd2d9904673c6d8ba1a7))
* **fish:** complete rewrite of all fish functions — 95 API-backed + 23 host tools ([979d769](https://github.com/WhispersOfJ/thebearcave/commit/979d76935df5b9429476d7250145237fc03dac25))
* monitoring stack — Grafana dashboards, Prometheus alert rules, landing page integration test ([edb6c5c](https://github.com/WhispersOfJ/thebearcave/commit/edb6c5ca418a9d2f669577481b8cf097cb845be8))
* **security:** add login rate limiting and failed-attempt logging ([523d840](https://github.com/WhispersOfJ/thebearcave/commit/523d84087a9cec78bc9c51e626925efe5e7f7465))


### Bug Fixes

* **catalog:** change network constant from 'stacknet' to 'bearcave' ([d16248e](https://github.com/WhispersOfJ/thebearcave/commit/d16248e50364f8cf6f9f6ab1367db558f621d549))
* **test:** use temp file to avoid pipefail issue with large HTML pages ([94defd8](https://github.com/WhispersOfJ/thebearcave/commit/94defd8ace62ec3c79a4a3d517307b974b29bd73))


### Performance Improvements

* gzip compression + lazy-load Mermaid for landing page ([ea70f4d](https://github.com/WhispersOfJ/thebearcave/commit/ea70f4d57ad1b2d72d3b3045c7e7d1d494b1e015))

## [1.5.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.4.1...v1.5.0) (2026-08-26)


### Features

* ecosystem improvements — full health coverage, live Mermaid colors, registry as single source ([346cd14](https://github.com/WhispersOfJ/thebearcave/commit/346cd144cfc70d5fb08169e5500120ed8a1b4a2c))

## [1.4.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.4.0...v1.4.1) (2026-08-26)


### Bug Fixes

* health view covers 19 services, backlinks use absolute URLs, registry validation, docstring accuracy ([6ab8e5c](https://github.com/WhispersOfJ/thebearcave/commit/6ab8e5c5cd88febea6d1242e51290757c1234095))
* **landing-page:** correct pipeline flow parallelism and guard Mermaid ([2753f11](https://github.com/WhispersOfJ/thebearcave/commit/2753f110d5da679d954ce0b4201f135488da85a3))

## [1.4.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.3.2...v1.4.0) (2026-08-26)


### Features

* unified ecosystem — service registry, category layout, Mermaid graph, detail panels, backlinks ([06889f5](https://github.com/WhispersOfJ/thebearcave/commit/06889f53265ed56d05f4b1b1c14a8256a7e396b5))

## [1.3.2](https://github.com/WhispersOfJ/thebearcave/compare/v1.3.1...v1.3.2) (2026-08-26)


### Bug Fixes

* **landing-page:** same-origin health fetch fixes HTTPS status dots ([46caf92](https://github.com/WhispersOfJ/thebearcave/commit/46caf92a05e22bb8119f2c33267017d7cb1fd6dc))

## [1.3.1](https://github.com/WhispersOfJ/thebearcave/compare/v1.3.0...v1.3.1) (2026-08-26)


### Bug Fixes

* **landing-page:** repoint doc links from media-stack to thebearcave ([ff5c3c5](https://github.com/WhispersOfJ/thebearcave/commit/ff5c3c56aa99b1134305bbf371a26d4236bd0e57))

## [1.3.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.2.0...v1.3.0) (2026-08-26)


### Features

* **landing-page:** CA-trust badge backed by the live TLS probe ([81d20b6](https://github.com/WhispersOfJ/thebearcave/commit/81d20b6433475795238ce060072b0f9a525fee3b))

## [1.2.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.1.0...v1.2.0) (2026-08-26)


### Features

* **control-panel:** read-only TLS cert diagnostic endpoint ([33de97e](https://github.com/WhispersOfJ/thebearcave/commit/33de97e60388e9cc077032b43150ac908f20abf6))
* **setup:** sync RELEASE_PLEASE_TOKEN to GitHub Actions secrets ([178d36a](https://github.com/WhispersOfJ/thebearcave/commit/178d36ad2ef7a984b5d6977ac126a5358c949704))

## [1.1.0](https://github.com/WhispersOfJ/thebearcave/compare/v1.0.0...v1.1.0) (2026-08-26)


### Features

* Ansible playbook to auto-trust the local CA on LAN devices ([3b10053](https://github.com/WhispersOfJ/thebearcave/commit/3b10053d7b6e042bb09d2d8248c07afeba4feb00))
* certificate setup section on the landing page ([c78f7f6](https://github.com/WhispersOfJ/thebearcave/commit/c78f7f6e2dd37262e0968d0ee738c6b3bd7552e0))
* initial scaffold — merge media-stack + metacacharr into The Bear Cave ([a0c976d](https://github.com/WhispersOfJ/thebearcave/commit/a0c976de3b42f8d9f7ee257943e9bce55f5866ba))
* local CA (mkcert) with trusted wildcard cert for all nip.io hostnames ([736f8c3](https://github.com/WhispersOfJ/thebearcave/commit/736f8c357c399a1631626b5029d86ef17fad10b1))
* trust the local CA inside every stack container ([e2acbe0](https://github.com/WhispersOfJ/thebearcave/commit/e2acbe08342c0ead95a80c665098e1a95ba09516))


### Bug Fixes

* align docs and gitignore with config/ layout after Phase 4 migration ([85841e7](https://github.com/WhispersOfJ/thebearcave/commit/85841e7c2804fac09602c08a9f524da7704ddef3))
* allow docker type and drop invalid input in PR title lint ([7d889e1](https://github.com/WhispersOfJ/thebearcave/commit/7d889e1e63570105a4619ad51dbe96453224d7d0))
* fall back to GITHUB_TOKEN when release-please PAT is unset ([1952a08](https://github.com/WhispersOfJ/thebearcave/commit/1952a084695b63555e7f10b74bd10300fa8c9400))
* remove invalid size expression from PR labeler workflow ([6ab944b](https://github.com/WhispersOfJ/thebearcave/commit/6ab944b5446c8f056757bec84cf4228355606620))
* remove unused variables flagged by shellcheck in health checks ([623432b](https://github.com/WhispersOfJ/thebearcave/commit/623432b66f7d9484e405bcdbd9fa9d364d8ed0be))
* stop failed Let's Encrypt attempts on LAN stack ([ef2ab3c](https://github.com/WhispersOfJ/thebearcave/commit/ef2ab3c6a2da4c6e0f7226b4eca48a35bdf75c4d))
