# Changelog

## 0.0.0 (2026-09-10)


### Features

* add Lantern Academy archetype selection ([9b2e4ff](https://github.com/WhispersOfJ/movie-rpg/commit/9b2e4fff2a3e3172972789058b5fffca9f63be91))
* add accounts and characters identity migration ([a285ea0](https://github.com/WhispersOfJ/movie-rpg/commit/a285ea00059ad34b09e9ba652daed2bfb93f635e))
* add achievements engine, mystery watch orders, and the game tick ([2c3c454](https://github.com/WhispersOfJ/movie-rpg/commit/2c3c454f037ff944471df94e7b250f1955c7f9f6))
* add achievements migration with the section 5.5 first-cut seed ([0327750](https://github.com/WhispersOfJ/movie-rpg/commit/03277500e020032ae234f63599b68a4606935ea1))
* add argon2 PIN gate with set and verify account flows ([524fb1f](https://github.com/WhispersOfJ/movie-rpg/commit/524fb1f9628819833e4749eba03e58ffb950c912))
* add axum server with pin gate and session cookies ([418ee47](https://github.com/WhispersOfJ/movie-rpg/commit/418ee47270f68f540bfef49711852120c123f563))
* add cache-aware metadata enrichment ([62666fc](https://github.com/WhispersOfJ/movie-rpg/commit/62666fc831c8a9610557cc60be32c3ebce429990))
* add cases and featured cases migrations ([eab5099](https://github.com/WhispersOfJ/movie-rpg/commit/eab5099f440eed3eab770720184da00027def98e))
* add content persistence upsert plan ([cbc8531](https://github.com/WhispersOfJ/movie-rpg/commit/cbc8531a14358dfc91b685130abda76370e35c4c))
* add content provider cache migration ([7728721](https://github.com/WhispersOfJ/movie-rpg/commit/77287215a6183c1a56b1b46ce3490ecac1fb899b))
* add deterministic content sync records ([5aa7295](https://github.com/WhispersOfJ/movie-rpg/commit/5aa72959a3f8e53dab12c05f88390607049c86a3))
* add end-to-end content sync pipeline ([027a22f](https://github.com/WhispersOfJ/movie-rpg/commit/027a22f26df64242d6258e6f48f2f38eae333290))
* add genre access and character state migrations ([7796c15](https://github.com/WhispersOfJ/movie-rpg/commit/7796c15132ccb3492c28aca5c9b98d7916252394))
* add idempotent schema migration runner ([3afb470](https://github.com/WhispersOfJ/movie-rpg/commit/3afb470e6bf186bda614a1c345c31fdca376c8bb))
* add metadata provider probe foundation ([dbeaf1a](https://github.com/WhispersOfJ/movie-rpg/commit/dbeaf1a7963df4a6d94398124061a2fe6a535355))
* add ordered sync state migration ([201ddc6](https://github.com/WhispersOfJ/movie-rpg/commit/201ddc64843a876f4e40e4f1d22f9c817f8408d0))
* add retry handling to provider probes ([59942a8](https://github.com/WhispersOfJ/movie-rpg/commit/59942a8704d0ed0c4981293fbc89187478717369))
* add settings migration and character bootstrap seeding ([d653210](https://github.com/WhispersOfJ/movie-rpg/commit/d653210ab2a3be63e0d440e28d05e0f7494c8e74))
* add transactional postgres content store ([560d20b](https://github.com/WhispersOfJ/movie-rpg/commit/560d20b27f3beb45caba03dd4b8d99ef6071be26))
* add unattended RPG polling loop ([acd3538](https://github.com/WhispersOfJ/movie-rpg/commit/acd3538fe05292280058ef365006309c66a04c0d))
* add watches ledger and deferred genre xp ledger migration ([feae67c](https://github.com/WhispersOfJ/movie-rpg/commit/feae67c87b0a92a622a7585625c3cc64fc45a169))
* attach provider metadata to content records ([f0c7baf](https://github.com/WhispersOfJ/movie-rpg/commit/f0c7bafa8142774fe059e0d673c4b76d06991451))
* enrich synchronized content batches ([af29a4a](https://github.com/WhispersOfJ/movie-rpg/commit/af29a4ad6bb53522e59c773e95149303bbef031c))
* group content records across sources ([1916856](https://github.com/WhispersOfJ/movie-rpg/commit/1916856dd417bd71cf6c2c599df6dc880b6dd4e8))
* hydrate provider cache on sync startup ([9e76636](https://github.com/WhispersOfJ/movie-rpg/commit/9e76636a24f742408db0f1593b9a8f7f8c7569bc))
* initial commit — Movie / TV RPG spec ([5bc23ee](https://github.com/WhispersOfJ/movie-rpg/commit/5bc23eeabf3275453734d3f6e915791810d11c0c))
* make provider probes locally testable ([3592bd9](https://github.com/WhispersOfJ/movie-rpg/commit/3592bd9bcede396996fef08ef2284574b6df118e))
* orchestrate stack content sync ([8a62eb0](https://github.com/WhispersOfJ/movie-rpg/commit/8a62eb07121c0da3c5b8600ae920b849555b84dd))


### Bug Fixes

* remediate full-codebase review findings (F-1 through F-39) ([3f7fbb2](https://github.com/WhispersOfJ/movie-rpg/commit/3f7fbb24c9eace26cb7d029be3a2f73f8c7a8b98))

### Documentation

* add handoff file for cross-session RPG work ([5a430f8](https://github.com/WhispersOfJ/movie-rpg/commit/5a430f827049e1b6408d06b77bcda70fb2873059))
* add Legends of the Green Dragon (LoGD) thanks + link-back ([11c84e1](https://github.com/WhispersOfJ/movie-rpg/commit/11c84e12ca54f48238e4b131864ac833fdc847ba))
* finalize Lantern Academy wizard contract ([f5a5f5c](https://github.com/WhispersOfJ/movie-rpg/commit/f5a5f5c31c0617b3ac1abcc9127e8b3ef5eb0030))
* finalize PIN hashing, PIN flows, and bootstrap placement in spec ([f26a945](https://github.com/WhispersOfJ/movie-rpg/commit/f26a9450af4db6d12a438db2d0ec38d8acc369eb))


### Tests

* harden provider enrichment sync ([b767906](https://github.com/WhispersOfJ/movie-rpg/commit/b7679061dc345df8f864d388359f87021f11847b))
* prove persistence surface against scratch postgres ([8b15e56](https://github.com/WhispersOfJ/movie-rpg/commit/8b15e56a1efe9bfc15f39223ce45d143cf5ee615))


### Maintenance

* add repo settings — release-please, workflows, LICENSE, CONTRIBUTING ([#1](https://github.com/WhispersOfJ/movie-rpg/issues/1)) ([ff77980](https://github.com/WhispersOfJ/movie-rpg/commit/ff779800c334572e4cf46fb4fa5f7b28e36df3db))
* add repo settings — release-please, workflows, LICENSE, CONTRIBUTING, CLAUDE.md, gitignore ([953ce64](https://github.com/WhispersOfJ/movie-rpg/commit/953ce645cd47e35a057c13b9818e3b3fe73bd3b9))
* **release-please-setup:** release 0.0.0 ([#2](https://github.com/WhispersOfJ/movie-rpg/issues/2)) ([0da8170](https://github.com/WhispersOfJ/movie-rpg/commit/0da81709b116d997730e7ed4da68cec6b774d838))

## 0.0.0 (2026-09-09)


### Features

* initial commit — Movie / TV RPG spec ([5bc23ee](https://github.com/WhispersOfJ/movie-rpg/commit/5bc23eeabf3275453734d3f6e915791810d11c0c))


### Documentation

* add Legends of the Green Dragon (LoGD) thanks + link-back ([11c84e1](https://github.com/WhispersOfJ/movie-rpg/commit/11c84e12ca54f48238e4b131864ac833fdc847ba))


### Maintenance

* add repo settings — release-please, workflows, LICENSE, CONTRIBUTING, CLAUDE.md, gitignore ([953ce64](https://github.com/WhispersOfJ/movie-rpg/commit/953ce645cd47e35a057c13b9818e3b3fe73bd3b9))
