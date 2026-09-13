# RAWRZ Seerr fork prototype

This directory contains the reproducible source patch for the M2 Seerr cache
prototype. `services/seerr/patches/rawrz-redis-cache.patch` is applied to the pinned
Seerr `v3.4.1` checkout during image builds; `generate-patch.py` is the single
source-transform generator for that artifact.

The prototype is opt-in. `SEERR_CACHE_BACKEND=memory` preserves the upstream
in-process cache; `redis` enables a namespaced Redis backend and fails open to a
process-local memory cache when Redis is unavailable. Redis keys are scoped by the
configured prefix and cache ID, hashed, serialized with a versioned envelope, and
flushed with `SCAN`/`UNLINK` rather than global Redis operations. A bounded probe
returns the runtime to Redis after recovery. Cache statistics are explicitly
process-local and report the active backend and degraded state.

The side-by-side Compose service uses host port `5056` and never replaces production
Seerr on `5055`.

## Build

```bash
./services/seerr/build-prototype.sh
```

The script archives the pinned `v3.4.1` source, applies the patch, and builds
`rawrz-seerr:m2` locally. It does not publish or mutate the running production
service. To regenerate the committed patch from the pinned upstream checkout:

```bash
python3 services/seerr/generate-patch.py
```

## Acceptance

```bash
./services/seerr/test-prototype.sh
```

The acceptance script uses disposable containers and a controlled HTTP fixture. It
seeds a disposable SQLite Seerr instance, authenticates with Seerr's API key, and
exercises the actual authenticated `/api/v1/movie/550` route using a deterministic
Redis-backed TMDB envelope. It checks that the route returns the cached mapped movie,
that a repeat request is a cache hit, and that the authenticated namespace flush
removes only the TMDB cache namespace. It then exercises the compiled `ExternalAPI`
boundary for:

- first-request miss and second-request Redis hit;
- falsy cached payloads;
- TTL and persistent entries;
- Redis outage with live upstream fallback and degraded stats;
- Redis restart, probe recovery, and a new Redis-backed hit;
- cleanup after success or failure.

The script uses an isolated Docker network and configurable container names, so it
can be run beside an existing stack without replacing production Seerr.

## CI and publication

`.github/workflows/seerr-m2.yml` fetches the exact upstream `v3.4.1` tag, builds the
candidate, checks the candidate against the existing Seerr Trivy baseline, runs the
acceptance contract, and publishes only `main` builds to GHCR under an immutable
commit tag. The workflow resolves the registry digest, pulls the image by
`image@sha256:...`, and reruns acceptance against that immutable image. Production
Seerr remains unchanged; cutover is deferred to M6.
