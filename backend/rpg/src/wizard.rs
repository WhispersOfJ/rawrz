//! Lantern Academy rules that do not require database I/O.

use chrono::NaiveDate;

/// Stable archetype identifiers seeded by migration 0012.
pub const LANTERN_SCHOLAR: &str = "lantern_scholar";
pub const EMBER_ADEPT: &str = "ember_adept";
pub const VEIL_CARTOGRAPHER: &str = "veil_cartographer";
pub const RUNE_FORGER: &str = "rune_forger";
pub const STAR_SHEPHERD: &str = "star_shepherd";
pub const MOONLIT_MEDIATOR: &str = "moonlit_mediator";

pub const VANISHING_STEP: &str = "vanishing_step";
pub const UNSEALING_LIGHT: &str = "unsealing_light";
pub const CHRONICLE_WARD: &str = "chronicle_ward";
pub const FOCUS_SIGIL: &str = "focus_sigil";
pub const SECOND_SIGHT: &str = "second_sight";

pub const AFFINITY_THRESHOLD: i64 = 100;
pub const SPELL_CHARGE_CAP: i64 = 3;

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

/// Evaluates one spell's unlock condition from the same persisted progression
/// facts as the archetype catalog. Spell charges are never granted here.
pub fn spell_unlock_satisfied(slug: &str, target: i64, facts: UnlockFacts) -> bool {
    match slug {
        VANISHING_STEP => facts.completed_orders >= target,
        UNSEALING_LIGHT => i64::from(facts.level) >= target,
        CHRONICLE_WARD => i64::from(facts.best_streak_days) >= target,
        FOCUS_SIGIL => facts.achievements >= target,
        SECOND_SIGHT => i64::from(facts.genres_accessed) >= target,
        _ => false,
    }
}

/// Returns the additive percent applied to the neutral normal-XP component by
/// the active archetype. The Focus Sigil percentage is supplied separately so
/// the caller can apply the contract's neutral → archetype → spell order.
pub fn normal_xp_modifier_percent(
    archetype_slug: &str,
    content_type: &str,
    is_horror: bool,
) -> i64 {
    match archetype_slug {
        EMBER_ADEPT if content_type == "movie" => 10,
        EMBER_ADEPT if content_type == "episode" => -10,
        RUNE_FORGER => -10,
        MOONLIT_MEDIATOR if is_horror => -10,
        _ => 0,
    }
}

fn apply_percent_floor(amount: i64, percent: i64) -> i64 {
    amount
        .max(0)
        .saturating_mul((100 + percent).clamp(50, 200))
        .checked_div(100)
        .unwrap_or(0)
        .max(0)
}

/// Applies the active archetype's prospective normal-XP component to one new
/// watch. Historical watch rows are never recalculated.
pub fn normal_xp_for_watch(
    archetype_slug: &str,
    content_type: &str,
    is_horror: bool,
    neutral_xp: i64,
) -> i64 {
    apply_percent_floor(
        neutral_xp,
        normal_xp_modifier_percent(archetype_slug, content_type, is_horror),
    )
}

/// Applies the Focus Sigil to the already archetype-adjusted component. The
/// percentages are additive in the caller by summing component modifiers; this
/// helper is retained for small pure-rule consumers and tests.
pub fn normal_xp_with_focus(
    archetype_slug: &str,
    content_type: &str,
    is_horror: bool,
    neutral_xp: i64,
    focus_active: bool,
) -> i64 {
    let archetype_percent = normal_xp_modifier_percent(archetype_slug, content_type, is_horror);
    let spell_percent = if focus_active { 10 } else { 0 };
    apply_percent_floor(neutral_xp, archetype_percent + spell_percent)
}

/// Affinity points are based on the neutral watch component, then the single
/// active archetype's affinity modifier. Only a matching, newly inserted watch
/// may call this rule.
pub fn affinity_points_for_watch(
    archetype_slug: &str,
    content_type: &str,
    matches_selected_discipline: bool,
    neutral_xp: i64,
) -> i64 {
    if !matches_selected_discipline {
        return 0;
    }
    let base = (neutral_xp.max(0) / 2).max(0);
    let modifier = match archetype_slug {
        VEIL_CARTOGRAPHER if content_type == "movie" => -10,
        STAR_SHEPHERD if content_type == "episode" => -10,
        MOONLIT_MEDIATOR => 10,
        _ => 0,
    };
    apply_percent_floor(base, modifier)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffinityCalculation {
    pub new_progress_points: i64,
    pub charges_to_grant: i64,
    pub overflow_points: i64,
}

/// Applies one affinity increment to a spell's meter and bounded charge
/// balance. A full meter at the charge cap drops all new points; crossing the
/// cap drops only the remainder that cannot be retained. The caller persists
/// grant and overflow audit rows transactionally.
pub fn apply_affinity(
    progress_points: i64,
    unspent_charges: i64,
    incoming_points: i64,
    charge_cap: i64,
) -> AffinityCalculation {
    let progress = progress_points.clamp(0, AFFINITY_THRESHOLD - 1);
    let charges = unspent_charges.clamp(0, charge_cap.max(0));
    let incoming = incoming_points.max(0);
    if incoming == 0 {
        return AffinityCalculation {
            new_progress_points: progress,
            charges_to_grant: 0,
            overflow_points: 0,
        };
    }
    if charges >= charge_cap {
        return AffinityCalculation {
            new_progress_points: progress,
            charges_to_grant: 0,
            overflow_points: incoming,
        };
    }

    let total = progress + incoming;
    let requested_charges = total / AFFINITY_THRESHOLD;
    let remainder = total % AFFINITY_THRESHOLD;
    let capacity = charge_cap - charges;
    let minted = requested_charges.min(capacity);
    let charge_total = charges + minted;

    if minted < requested_charges {
        return AffinityCalculation {
            new_progress_points: 0,
            charges_to_grant: minted,
            overflow_points: (requested_charges - minted) * AFFINITY_THRESHOLD + remainder,
        };
    }

    // At the cap, no partial meter can be retained: it would be unspendable
    // until a charge is spent and the contract explicitly leaves the meter at
    // zero when the cap is reached.
    if charge_total >= charge_cap {
        AffinityCalculation {
            new_progress_points: 0,
            charges_to_grant: minted,
            overflow_points: remainder,
        }
    } else {
        AffinityCalculation {
            new_progress_points: remainder,
            charges_to_grant: minted,
            overflow_points: 0,
        }
    }
}

/// Returns whether a watch on `today` is a cold-gap transition.
pub fn is_cold_gap(last_watch_date: Option<NaiveDate>, today: NaiveDate) -> bool {
    match last_watch_date {
        Some(day) => day != today && day != today.pred_opt().unwrap_or(today),
        None => true,
    }
}

/// Computes the next streak. A protection effect changes only a cold-gap
/// transition; the caller still supplies a real newly awarded watch and the
/// date is still written as the date the poll detected that watch.
pub fn streak_after_watch(
    current_streak: i64,
    last_watch_date: Option<NaiveDate>,
    today: NaiveDate,
    protected_cold_gap: bool,
) -> i64 {
    match last_watch_date {
        Some(day) if day == today => current_streak,
        Some(day) if day == today.pred_opt().unwrap_or(today) => current_streak + 1,
        _ if protected_cold_gap => current_streak + 1,
        _ => 1,
    }
    .max(1)
}

/// Rune Forger affects only visible presentation progress, never unlock truth.
pub fn visible_achievement_progress(progress: i64) -> i64 {
    progress.max(0).saturating_mul(110).checked_div(100).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use super::{
        affinity_points_for_watch, apply_affinity, is_cold_gap, normal_xp_for_watch,
        normal_xp_with_focus, selection_allowed, spell_unlock_satisfied, streak_after_watch,
        unlock_satisfied, visible_achievement_progress, AffinityCalculation, SelectionRejection,
        UnlockFacts, EMBER_ADEPT, LANTERN_SCHOLAR, MOONLIT_MEDIATOR, RUNE_FORGER,
        STAR_SHEPHERD, VANISHING_STEP, VEIL_CARTOGRAPHER,
    };

    fn facts() -> UnlockFacts {
        UnlockFacts {
            level: 2,
            completed_orders: 1,
            achievements: 5,
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
    fn evaluates_all_five_spell_unlocks_without_granting_charges() {
        for (slug, target) in [
            (VANISHING_STEP, 1),
            ("unsealing_light", 2),
            ("chronicle_ward", 7),
            ("focus_sigil", 5),
            ("second_sight", 3),
        ] {
            assert!(spell_unlock_satisfied(slug, target, facts()));
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
    fn focus_sigil_stacks_additively_with_the_archetype_component() {
        assert_eq!(normal_xp_with_focus(EMBER_ADEPT, "movie", false, 20, true), 24);
        assert_eq!(normal_xp_with_focus(EMBER_ADEPT, "episode", false, 10, true), 10);
        assert_eq!(normal_xp_with_focus(LANTERN_SCHOLAR, "movie", false, 20, true), 22);
    }

    #[test]
    fn affinity_uses_neutral_half_xp_and_bounded_archetype_effects() {
        assert_eq!(affinity_points_for_watch(LANTERN_SCHOLAR, "episode", true, 10), 5);
        assert_eq!(affinity_points_for_watch(LANTERN_SCHOLAR, "movie", true, 20), 10);
        assert_eq!(affinity_points_for_watch(VEIL_CARTOGRAPHER, "movie", true, 20), 9);
        assert_eq!(affinity_points_for_watch(STAR_SHEPHERD, "episode", true, 10), 4);
        assert_eq!(affinity_points_for_watch(MOONLIT_MEDIATOR, "movie", true, 20), 11);
        assert_eq!(affinity_points_for_watch(LANTERN_SCHOLAR, "movie", false, 20), 0);
    }

    #[test]
    fn affinity_remainder_and_charge_cap_are_explicit() {
        assert_eq!(
            apply_affinity(0, 0, 95, 3),
            AffinityCalculation {
                new_progress_points: 95,
                charges_to_grant: 0,
                overflow_points: 0,
            }
        );
        assert_eq!(
            apply_affinity(95, 0, 10, 3),
            AffinityCalculation {
                new_progress_points: 5,
                charges_to_grant: 1,
                overflow_points: 0,
            }
        );
        assert_eq!(
            apply_affinity(95, 2, 10, 3),
            AffinityCalculation {
                new_progress_points: 0,
                charges_to_grant: 1,
                overflow_points: 5,
            }
        );
        assert_eq!(
            apply_affinity(50, 3, 10, 3),
            AffinityCalculation {
                new_progress_points: 50,
                charges_to_grant: 0,
                overflow_points: 10,
            }
        );
        assert_eq!(
            apply_affinity(0, 0, 350, 3),
            AffinityCalculation {
                new_progress_points: 0,
                charges_to_grant: 3,
                overflow_points: 50,
            }
        );
    }

    #[test]
    fn cold_gap_protection_changes_only_the_streak_transition() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
        let cold = NaiveDate::from_ymd_opt(2026, 9, 7).unwrap();
        assert!(!is_cold_gap(Some(yesterday), today));
        assert!(is_cold_gap(Some(cold), today));
        assert_eq!(streak_after_watch(7, Some(cold), today, false), 1);
        assert_eq!(streak_after_watch(7, Some(cold), today, true), 8);
        assert_eq!(streak_after_watch(7, Some(today), today, true), 7);
    }

    #[test]
    fn visible_progress_is_presentation_only() {
        assert_eq!(visible_achievement_progress(9), 9);
        assert_eq!(visible_achievement_progress(10), 11);
        assert_eq!(visible_achievement_progress(0), 0);
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
