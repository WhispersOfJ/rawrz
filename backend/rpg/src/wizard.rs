//! Lantern Academy rules that do not require database I/O.

use chrono::NaiveDate;

/// Stable archetype identifiers seeded by migration 0012.
pub const LANTERN_SCHOLAR: &str = "lantern_scholar";
pub const EMBER_ADEPT: &str = "ember_adept";
pub const VEIL_CARTOGRAPHER: &str = "veil_cartographer";
pub const RUNE_FORGER: &str = "rune_forger";
pub const STAR_SHEPHERD: &str = "star_shepherd";
pub const MOONLIT_MEDIATOR: &str = "moonlit_mediator";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockFacts {
    pub level: i32,
    pub completed_orders: i64,
    pub achievements: i64,
    pub best_streak_days: i32,
    pub genres_accessed: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionRejection {
    NotUnlocked,
    AlreadyActive,
    AnotherSelectionPending,
    SelectionAlreadyAcceptedToday,
}

impl SelectionRejection {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NotUnlocked => "archetype is not unlocked",
            Self::AlreadyActive => "archetype is already active",
            Self::AnotherSelectionPending => "another selection is already pending",
            Self::SelectionAlreadyAcceptedToday => "one selection is already accepted today",
        }
    }
}

/// The single policy decision for accepting a queued loadout change. The
/// store supplies facts and persists the resulting transition; this function
/// never reads or mutates database state.
pub fn selection_allowed(
    target_unlocked: bool,
    target_slug: &str,
    active_slug: &str,
    has_pending_selection: bool,
    selected_local_date: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<(), SelectionRejection> {
    if !target_unlocked {
        return Err(SelectionRejection::NotUnlocked);
    }
    if target_slug == active_slug {
        return Err(SelectionRejection::AlreadyActive);
    }
    if has_pending_selection {
        return Err(SelectionRejection::AnotherSelectionPending);
    }
    if selected_local_date == Some(today) {
        return Err(SelectionRejection::SelectionAlreadyAcceptedToday);
    }
    Ok(())
}

/// Evaluates the data-driven unlock condition for one seeded archetype.
/// Permanent unlock rows are written by the store when this returns true.
pub fn unlock_satisfied(slug: &str, target: i64, facts: UnlockFacts) -> bool {
    match slug {
        LANTERN_SCHOLAR => true,
        EMBER_ADEPT => i64::from(facts.level) >= target,
        VEIL_CARTOGRAPHER => facts.completed_orders >= target,
        RUNE_FORGER => facts.achievements >= target,
        STAR_SHEPHERD => i64::from(facts.best_streak_days) >= target,
        MOONLIT_MEDIATOR => i64::from(facts.genres_accessed) >= target,
        _ => false,
    }
}

/// Applies the active archetype's prospective normal-XP component to one new
/// watch. Effects are deliberately narrow, additive, and rounded down once;
/// historical watch rows are never recalculated. The fixed components are
/// all within the contract's ±10% bound.
pub fn normal_xp_for_watch(
    archetype_slug: &str,
    content_type: &str,
    is_horror: bool,
    neutral_xp: i64,
) -> i64 {
    let modifier_percent: i64 = match archetype_slug {
        EMBER_ADEPT if content_type == "movie" => 10,
        EMBER_ADEPT if content_type == "episode" => -10,
        RUNE_FORGER => -10,
        MOONLIT_MEDIATOR if is_horror => -10,
        _ => 0,
    };
    let adjusted = neutral_xp
        .saturating_mul(100 + modifier_percent)
        .checked_div(100)
        .unwrap_or(0);
    adjusted.max(0)
}

#[cfg(test)]
mod tests {
    use super::{
        normal_xp_for_watch, selection_allowed, unlock_satisfied, SelectionRejection, UnlockFacts,
        EMBER_ADEPT, LANTERN_SCHOLAR, MOONLIT_MEDIATOR, RUNE_FORGER,
    };

    fn facts() -> UnlockFacts {
        UnlockFacts {
            level: 2,
            completed_orders: 1,
            achievements: 3,
            best_streak_days: 7,
            genres_accessed: 3,
        }
    }

    #[test]
    fn evaluates_all_six_seeded_archetype_unlocks() {
        for (slug, target) in [
            ("lantern_scholar", 0),
            ("ember_adept", 2),
            ("veil_cartographer", 1),
            ("rune_forger", 3),
            ("star_shepherd", 7),
            ("moonlit_mediator", 3),
        ] {
            assert!(
                unlock_satisfied(slug, target, facts()),
                "{slug} should unlock"
            );
        }
    }

    #[test]
    fn does_not_unlock_before_each_threshold() {
        let mut current = facts();
        current.level = 1;
        assert!(!unlock_satisfied("ember_adept", 2, current));
        current.completed_orders = 0;
        assert!(!unlock_satisfied("veil_cartographer", 1, current));
        current.achievements = 2;
        assert!(!unlock_satisfied("rune_forger", 3, current));
        current.best_streak_days = 6;
        assert!(!unlock_satisfied("star_shepherd", 7, current));
        current.genres_accessed = 2;
        assert!(!unlock_satisfied("moonlit_mediator", 3, current));
    }

    #[test]
    fn selection_policy_has_one_authoritative_rejection_order() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        assert_eq!(
            selection_allowed(false, EMBER_ADEPT, LANTERN_SCHOLAR, false, None, today),
            Err(SelectionRejection::NotUnlocked)
        );
        assert_eq!(
            selection_allowed(true, LANTERN_SCHOLAR, LANTERN_SCHOLAR, false, None, today),
            Err(SelectionRejection::AlreadyActive)
        );
        assert_eq!(
            selection_allowed(true, EMBER_ADEPT, LANTERN_SCHOLAR, true, None, today),
            Err(SelectionRejection::AnotherSelectionPending)
        );
        assert_eq!(
            selection_allowed(
                true,
                EMBER_ADEPT,
                LANTERN_SCHOLAR,
                false,
                Some(today),
                today
            ),
            Err(SelectionRejection::SelectionAlreadyAcceptedToday)
        );
        assert!(selection_allowed(true, EMBER_ADEPT, LANTERN_SCHOLAR, false, None, today).is_ok());
    }

    #[test]
    fn applies_only_the_active_archetypes_named_xp_component() {
        assert_eq!(normal_xp_for_watch(EMBER_ADEPT, "movie", false, 20), 22);
        assert_eq!(normal_xp_for_watch(EMBER_ADEPT, "episode", false, 10), 9);
        assert_eq!(normal_xp_for_watch(RUNE_FORGER, "movie", false, 20), 18);
        assert_eq!(normal_xp_for_watch(MOONLIT_MEDIATOR, "movie", true, 20), 18);
        assert_eq!(
            normal_xp_for_watch(MOONLIT_MEDIATOR, "movie", false, 20),
            20
        );
        assert_eq!(normal_xp_for_watch(LANTERN_SCHOLAR, "movie", true, 20), 20);
    }

    #[test]
    fn keeps_watch_effects_bounded_and_non_negative() {
        for slug in [LANTERN_SCHOLAR, EMBER_ADEPT, RUNE_FORGER, MOONLIT_MEDIATOR] {
            let movie = normal_xp_for_watch(slug, "movie", true, 20);
            assert!((18..=22).contains(&movie), "{slug} effect escaped ±10%");
        }
        assert_eq!(normal_xp_for_watch(EMBER_ADEPT, "movie", false, -1), 0);
    }
}
