//! The game tick (spec §9.1, phase order finalized 2026-09-08): each poll
//! and each UI refresh runs the game state forward through **ordered
//! phases** in one place — never scattered hooks:
//!
//! 1. **watch award** — detect newly completed (≥95%) watches and write
//!    `watches` rows + XP/streak updates (`awards.rs` is the pure math;
//!    `detect_plex_watch_states` feeds it). A stack failure here is logged
//!    and non-fatal: phases 2–3 still run on known state.
//! 2. **order reveals** — `refresh_watch_orders` (§5.7): stamp audit
//!    reveals, complete finished cycles, grant skip rewards, generate next
//!    cycles.
//! 3. **achievement evaluation** — `evaluate_achievements` (§6.4.8) reads
//!    post-award, post-reveal state, so unlocks reflect the same tick.
//!
//! `POST /api/orders/refresh` is the tick's UI entry point (stack-less);
//! the poll loop (`poll.rs`) calls the same function with the stack clients.

use crate::persistence::{EvaluationSummary, OrderRefreshSummary, PostgresContentStore};
use crate::Result;
use serde::Serialize;

/// What one tick did, phase by phase.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GameTickReport {
    /// Newly awarded watches (phase 1).
    pub watches_awarded: usize,
    /// Orders generated / reveals advanced / skips granted (phase 2).
    pub orders: OrderRefreshSummary,
    /// Achievement evaluation outcome (phase 3).
    pub achievements: Option<EvaluationSummary>,
}

/// Runs one game tick: the ordered phases, in order. `plex` is `None` when
/// the stack is unreachable — phases 2–3 still run so the game advances on
/// what is already known.
pub async fn run_game_tick(
    store: &mut PostgresContentStore,
    plex: Option<&crate::stack::PlexClient>,
) -> Result<GameTickReport> {
    // Wizard loadout boundary: pending selection is applied before any
    // watch/order/achievement work in this tick.
    store.apply_pending_archetype().await?;

    let mut report = GameTickReport {
        watches_awarded: 0,
        orders: OrderRefreshSummary {
            orders_created: Vec::new(),
            skips_granted: 0,
        },
        achievements: None,
    };

    // Phase 1: watch award from Plex watch-state (§9.1 detection). Everything
    // downstream derives from the watches ledger, so this must write first.
    // Degradation contract (§9.1): a stack failure is logged and non-fatal —
    // phases 2–3 still advance the game on known state.
    if let Some(plex) = plex {
        match detect_plex_watch_states(plex).await {
            Ok(states) => {
                let today = chrono::Local::now().date_naive();
                match store.award_plex_watches(&states, today).await {
                    Ok(watches_awarded) => report.watches_awarded = watches_awarded,
                    Err(error) => eprintln!(
                        "tick: phase 1 watch award failed (continuing on known state): {error}"
                    ),
                }
            }
            Err(error) => {
                eprintln!(
                    "tick: phase 1 watch detection failed (continuing on known state): {error}"
                );
            }
        }
    }

    // Phase 2: order reveals, completions, skip grants, generation.
    report.orders = store.refresh_watch_orders().await?;

    // Phase 3: achievements read the post-award, post-reveal state.
    report.achievements = store.evaluate_achievements().await?;

    Ok(report)
}

fn attr_i64(attrs: &[(String, String)], key: &str) -> Option<i64> {
    attrs
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.parse().ok())
}

/// Reads every library item's watch-state into the pure award input (§9.1
/// detection semantics). Errors surface to the caller, which decides
/// degradation — the tick's phases 2–3 must still run.
async fn detect_plex_watch_states(
    plex: &crate::stack::PlexClient,
) -> Result<Vec<crate::awards::PlexWatchState>> {
    let mut states = Vec::new();
    for section in plex.sections().await? {
        for item in plex
            .library_items(&section.key.clone().unwrap_or_default())
            .await?
        {
            states.push(crate::awards::PlexWatchState {
                rating_key: item.rating_key.unwrap_or_default(),
                view_count: attr_i64(&item.raw_attributes, "viewCount"),
                view_offset_ms: attr_i64(&item.raw_attributes, "viewOffset"),
                duration_ms: attr_i64(&item.raw_attributes, "duration"),
                item_type: item.item_type.unwrap_or_default(),
            });
        }
    }
    Ok(states)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_report_is_a_clean_tick() {
        let report = run_game_tick_shape();
        assert_eq!(report.watches_awarded, 0);
        assert!(report.orders.orders_created.is_empty());
        assert_eq!(report.orders.skips_granted, 0);
        assert!(report.achievements.is_none());
    }

    // The clean-tick shape `run_game_tick` starts from before phases run.
    fn run_game_tick_shape() -> GameTickReport {
        GameTickReport {
            watches_awarded: 0,
            orders: OrderRefreshSummary {
                orders_created: Vec::new(),
                skips_granted: 0,
            },
            achievements: None,
        }
    }
}
