//! Lantern Academy archetype rules that do not require database I/O.

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

#[cfg(test)]
mod tests {
    use super::{unlock_satisfied, UnlockFacts};

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
            assert!(unlock_satisfied(slug, target, facts()), "{slug} should unlock");
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
}
