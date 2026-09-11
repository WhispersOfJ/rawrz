# ADR 0001 — Feature-ID contract enforcement

**Status:** Accepted (2026-09-06) · **Milestone:** M0

## Context

The spec (thebearcave `cave-deck-spec.md` §D.4/D.6) mandates that every GUI
feature ID from the parity table is implemented, and every `retire:` row stays
unimplemented. Hand-maintained tables drift the moment someone adds a route.

## Decision

1. Every backend route group carries `#[doc = "features: [...]"]`.
2. Every frontend area page exports `FEATURE_IDS = [...]`.
3. `scripts/check_api_contract.py` (CI job `api-contract`) scrapes both and
   asserts, against `parity/parity.yaml`:
   - no parity ID unserved (backend ∪ frontend),
   - no route/page referencing an unknown ID,
   - `retire:` rows never appear as served IDs.
4. `backend/tests/api_contract.rs` pins the HTTP behaviors (landmine red paths)
   that the ID declarations cannot express.

## Consequences

- Adding a route without declaring IDs fails CI (drift is impossible to merge).
- M0 covers 48/48 IDs with honest stubs; per-milestone work fills handlers, not
  declarations.
- The header layer (`x-cave-deck-features`) from the spec's §D.2 sketch is
  intentionally deferred to M1 to keep the M0 router single-layered; the doc
  attributes are the source of truth.
