# Legacy stack-management backend

The former Bear Cave working tree contained a partial Rust/Axum stack-management backend
under `~/Cave/backend/src/` (`routes.rs`, `config.rs`, `docker.rs`, `executor.rs`, and
`jobs.rs`). It had no `Cargo.toml`, `main.rs`, or complete build/test surface, and its
files were not committed in the imported Bear Cave history.

M0 deliberately does not import those scratch sources as active code. Their intended
control-plane inventory is represented by the Cave Deck specification and implementation:

- specification: [`../../docs/deck/cave-deck-spec.md`](../../docs/deck/cave-deck-spec.md)
- backend: [`../../backend/deck/`](../../backend/deck/)
- API contract: [`../../scripts/check_api_contract.py`](../../scripts/check_api_contract.py)

This is a disposition record, not a promise that the target-state Deck is complete. The
root Compose runtime remains unchanged during M0; later Deck/runtime work must follow its
own milestone and rollback plan.
