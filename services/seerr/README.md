# RAWRZ Seerr fork prototype

This directory contains the source patch and build context for the M2 Seerr cache
prototype. It is intentionally kept separate from the upstream Seerr source tree:
`services/seerr/patches/rawrz-redis-cache.patch` is applied to the pinned Seerr
v3.4.1 checkout during image builds.

The prototype is opt-in. `SEERR_CACHE_BACKEND=memory` preserves upstream behavior;
`redis` enables the namespaced Redis backend and fails open to memory when Redis is
unavailable. The side-by-side Compose service uses host port `5056` and never replaces
production `seerr` on `5055`.

## Build

```bash
./services/seerr/build-prototype.sh
```

The script fetches the pinned v3.4.1 source, applies the patch, and builds
`rawrz-seerr:m2` locally. It does not publish or mutate the running production service.

## Acceptance

```bash
./services/seerr/test-prototype.sh
```

The acceptance script starts the disposable prototype container alongside Redis and
checks the health endpoint, repeat-cache behavior, namespace/TTL configuration, and
fail-open startup with Redis unavailable.
