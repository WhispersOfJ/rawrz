//! The game tick (spec §9.1, phase order finalized 2026-09-08): each poll
//! and each UI refresh runs the game state forward through **ordered
//! phases** in one place — never scattered hooks:
//!
//! 1. **watch award** — detect newly completed (≥95%) watches and write
//!    `watches` rows + XP/streak updates. V1 slot: the detection query lands
//!    with the Plex watch-state integration (§9.1); the phase is reserved
//!    and reported so its wiring is additive, not a refactor.
//! 2. **order reveals** — `refresh_watch_orders` (§5.7): stamp audit
//!    reveals, complete finished cycles, grant skip rewards, generate next
//!    cycles.
//! 3. **achievement evaluation** — `evaluate_achievements` (§6.4.8) reads
//!    post-award, post-reveal state, so unlocks reflect the same tick.
//!
//! `POST /api/orders/refresh` is the tick's UI entry point today; the poll
//! loop calls the same function when the watch-award phase lands.

use crate::persistence::{EvaluationSummary, OrderRefreshSummary, PostgresContentStore};
use crate::Result;
use serde::Serialize;

/// What one tick did, phase by phase.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GameTickReport {
    /// Newly awarded watches (phase 1 — populated once Plex watch-state
    /// detection lands).
    pub watches_awarded: usize,
    /// Orders generated / reveals advanced / skips granted (phase 2).
    pub orders: OrderRefreshSummary,
    /// Achievement evaluation outcome (phase 3).
    pub achievements: Option<EvaluationSummary>,
}

/// Runs one game tick: the ordered phases, in order.
pub async fn run_game_tick(store: &mut PostgresContentStore) -> Result<GameTickReport> {
    let mut report = GameTickReport {
        watches_awarded: 0,
        orders: OrderRefreshSummary {
            orders_created: Vec::new(),
            skips_granted: 0,
        },
        achievements: None,
    };

    // Phase 1 (slot): watch award from Plex watch-state detection lands here.
    // Anything it writes must precede phase 2 — reveals derive from the
    // watches ledger (§5.7).

    // Phase 2: order reveals, completions, skip grants, generation.
    report.orders = store.refresh_watch_orders().await?;

    // Phase 3: achievements read the post-award, post-reveal state.
    report.achievements = store.evaluate_achievements().await?;

    Ok(report)
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
