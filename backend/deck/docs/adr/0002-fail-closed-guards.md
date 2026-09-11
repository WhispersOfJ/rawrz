# ADR 0002 — Landmine guards are fail-closed

**Status:** Accepted (2026-09-06) · **Milestone:** M0

## Context

Spec §7 turns thebearcave's operational landmines (#2 FUSE mount, #4 nzbdav
queue, #13 triple health check) into runtime guards. A guard that cannot verify
must not default to "safe to proceed".

## Decision

All guards are fail-closed. Until a probe is implemented, it reports
"unverifiable", and callers refuse the operation:

- `guards::mount_healthy()` returns `false` until the rclone-RC probe exists →
  `POST /api/v1/plex/empty-trash` answers **409 `mount_unhealthy`** (landmine #2).
- `guards::nzbdav_queue_empty()` errors until the authenticated queue probe
  exists → any recreate flow refuses with `queue_guard_unavailable` (#4).
- `guards::nzbdav_triple_healthy()` returns `false` until the
  `/healthz` + authenticated queue + PROPFIND triple check exists (#13).

`backend/tests/api_contract.rs` pins the empty-trash red path at the HTTP layer.

## Consequences

- The stack can never be mutated by a guard that silently degraded.
- Probes landing in M1–M3 flip guards from "refuse" to "verify" — each flip
  must add its green-path test in the same PR.
