//! Game-tick phase 1: watch award (spec §9.1, detection semantics finalized
//! 2026-09-08). Reads the Plex library, derives completed watches from
//! watch-state attributes, computes XP / streak / level outcomes as **pure
//! functions**, and lets the store apply them.
//!
//! Detection (per §12 Q4 / §9.1): `viewCount ≥ 1` → completed at 100%;
//! otherwise `viewOffset / duration ≥ 0.95` → completed at that ratio.
//! Matching: content rows are keyed `(source = 'plex', source_id =
//! ratingKey)`. Award-once: a content item earns at most one `watches` row
//! in V1 (no re-watch credit).
//!
//! XP (§5.1): movie = 20, episode = 10, written as `normal_xp`. Streak
//! (§5.1): same-day detection is neutral, a detection on the day after the
//! last watch date continues the streak, anything older restarts at 1 —
//! local calendar days via the host clock. Milestone bonuses (§5.1.1) pay
//! exactly when the streak first reaches a milestone length, which by
//! construction happens once per streak run. Levels (§5.2): re-evaluated
//! from cumulative XP against the level table on every award pass.

/// The §12 Q4 completion threshold (configurable in principle; V1 constant).
pub const COMPLETION_THRESHOLD: f64 = 0.95;

/// §5.1 base XP: a movie counts as ≈2× an episode.
pub const MOVIE_XP: i64 = 20;
pub const EPISODE_XP: i64 = 10;

/// A completed watch derived from Plex watch state.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedWatch {
    pub rating_key: String,
    pub content_id: i64,
    pub content_type: String,
    pub pct_viewed: i32,
}

/// A completed watch derived from Plex watch state.
#[derive(Debug, Clone, PartialEq)]
pub struct PlexWatchState {
    pub rating_key: String,
    pub view_count: Option<i64>,
    pub view_offset_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub item_type: String,
}

impl PlexWatchState {
    /// The §9.1 detection rule: fully watched, or ≥95% in-progress.
    pub fn completion_pct(&self) -> Option<i32> {
        if self.view_count.unwrap_or(0) >= 1 {
            return Some(100);
        }
        let (offset, duration) = (self.view_offset_ms?, self.duration_ms?);
        if duration <= 0 {
            return None;
        }
        let ratio = offset as f64 / duration as f64;
        (ratio >= COMPLETION_THRESHOLD).then(|| (ratio * 100.0).round() as i32)
    }
}

/// One detection outcome: the watch to insert plus its XP contribution.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchAward {
    pub watch: DetectedWatch,
    pub xp: i64,
}

/// Pure detection: match Plex state against known content ids and derive
/// awards. Unknown rating keys are ignored (the sync owns the catalog).
pub fn detect_watches(
    states: &[PlexWatchState],
    content_ids_by_rating_key: &std::collections::HashMap<String, i64>,
    content_types_by_content_id: &std::collections::HashMap<i64, String>,
) -> Vec<WatchAward> {
    states
        .iter()
        .filter_map(|state| {
            let pct = state.completion_pct()?;
            let content_id = *content_ids_by_rating_key.get(&state.rating_key)?;
            let content_type = content_types_by_content_id.get(&content_id)?;
            let base = match content_type.as_str() {
                "movie" => MOVIE_XP,
                "episode" => EPISODE_XP,
                _ => return None,
            };
            Some(WatchAward {
                watch: DetectedWatch {
                    rating_key: state.rating_key.clone(),
                    content_id,
                    content_type: content_type.clone(),
                    pct_viewed: pct,
                },
                xp: base,
            })
        })
        .collect()
}

/// §5.2 cumulative level table: level N is reached at `LEVEL_XP[N - 2]`.
pub const LEVEL_XP: [i64; 9] = [100, 250, 500, 900, 1400, 2000, 2700, 3500, 4400];

/// Pure level math (§5.2): the highest level whose threshold the XP reaches.
pub fn level_for_xp(xp: i64) -> i64 {
    let mut level = 1;
    for (index, threshold) in LEVEL_XP.iter().enumerate() {
        if xp >= *threshold {
            level = index as i64 + 2;
        }
    }
    level
}

/// The streak result of awarding on `today`: continuation, restart, or the
/// neutral same-day case (§5.1: a second same-day detection changes nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreakAdvance {
    Neutral,
    Continue { new_streak: i64 },
    Restart,
}

/// Pure streak advance (§5.1): `last_watch_date` is `None` before the first
/// ever watch. Days are whole calendar days (host-local clock).
pub fn streak_advance(last_watch_date: Option<chrono::NaiveDate>, today: chrono::NaiveDate) -> StreakAdvance {
    match last_watch_date {
        Some(day) if day == today => StreakAdvance::Neutral,
        Some(day) if day == today.pred_opt().unwrap_or(today) => {
            // The caller supplies the current streak; this branch only
            // signals continuation.
            StreakAdvance::Continue { new_streak: 0 }
        }
        _ => StreakAdvance::Restart,
    }
}

/// Pure milestone bonus (§5.1.1): pays exactly when the streak first
/// reaches a milestone length.
pub fn streak_milestone_bonus(new_streak: i64) -> i64 {
    match new_streak {
        2 => 5,
        3 => 10,
        5 => 25,
        7 => 50,
        10 => 100,
        14 => 200,
        21 => 400,
        30 => 800,
        60 => 1600,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ids() -> (HashMap<String, i64>, HashMap<i64, String>) {
        let mut keys = HashMap::new();
        let mut types = HashMap::new();
        keys.insert("plex-1".to_owned(), 11);
        types.insert(11, "movie".to_owned());
        keys.insert("plex-2".to_owned(), 12);
        types.insert(12, "episode".to_owned());
        (keys, types)
    }

    fn state(key: &str, view_count: Option<i64>, offset: i64, duration: i64) -> PlexWatchState {
        PlexWatchState {
            rating_key: key.to_owned(),
            view_count,
            view_offset_ms: Some(offset),
            duration_ms: Some(duration),
            item_type: "movie".to_owned(),
        }
    }

    #[test]
    fn view_count_and_threshold_derive_completions() {
        assert_eq!(state("a", Some(1), 0, 0).completion_pct(), Some(100));
        assert_eq!(state("a", None, 96, 100).completion_pct(), Some(96));
        assert_eq!(state("a", None, 94, 100).completion_pct(), None);
        assert_eq!(state("a", None, 95, 100).completion_pct(), Some(95));
        // Degenerate duration is not a completion.
        assert_eq!(state("a", None, 1000, 0).completion_pct(), None);
    }

    #[test]
    fn detection_matches_known_content_and_awards_once_worth_of_xp() {
        let (keys, types) = ids();
        let awards = detect_watches(
            &[
                state("plex-1", Some(1), 0, 0),
                state("plex-2", None, 960, 1000),
                state("unknown-key", Some(1), 0, 0),
                state("plex-3", Some(1), 0, 0),
            ],
            &keys,
            &types,
        );
        assert_eq!(awards.len(), 2);
        assert_eq!(awards[0].watch.content_id, 11);
        assert_eq!(awards[0].watch.pct_viewed, 100);
        assert_eq!(awards[0].xp, MOVIE_XP);
        assert_eq!(awards[1].watch.content_type, "episode");
        assert_eq!(awards[1].watch.pct_viewed, 96);
        assert_eq!(awards[1].xp, EPISODE_XP);
    }

    #[test]
    fn levels_follow_the_5_2_table() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(99), 1);
        assert_eq!(level_for_xp(100), 2);
        assert_eq!(level_for_xp(249), 2);
        assert_eq!(level_for_xp(250), 3);
        assert_eq!(level_for_xp(4400), 10);
        assert_eq!(level_for_xp(999_999), 10);
    }

    #[test]
    fn streaks_are_neutral_same_day_and_restart_when_cold() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
        assert_eq!(
            streak_advance(Some(today.pred_opt().unwrap()), today),
            StreakAdvance::Continue { new_streak: 0 }
        );
        assert_eq!(streak_advance(Some(today), today), StreakAdvance::Neutral);
        assert_eq!(streak_advance(None, today), StreakAdvance::Restart);
        let cold = chrono::NaiveDate::from_ymd_opt(2026, 9, 7).unwrap();
        assert_eq!(streak_advance(Some(cold), today), StreakAdvance::Restart);
    }

    #[test]
    fn milestone_bonuses_pay_once_at_exact_lengths() {
        assert_eq!(streak_milestone_bonus(1), 0);
        assert_eq!(streak_milestone_bonus(2), 5);
        assert_eq!(streak_milestone_bonus(4), 0);
        assert_eq!(streak_milestone_bonus(7), 50);
        assert_eq!(streak_milestone_bonus(60), 1600);
        assert_eq!(streak_milestone_bonus(61), 0);
    }

    #[test]
    fn detection_handles_threshold_edges() {
        // 95% exactly passes; one ms below does not.
        assert_eq!(state("a", None, 950, 1000).completion_pct(), Some(95));
        assert_eq!(state("a", None, 949, 1000).completion_pct(), None);
    }
}
