//! Achievement evaluation engine (spec §6.4.8, evaluation contract
//! finalized 2026-09-08): a **pure** function from an achievement definition
//! plus a `ProgressSnapshot` to an `Evaluation`. No I/O here — the store
//! builds the snapshot, applies the outcomes, and writes
//! `character_achievements` rows only at unlock (idempotent,
//! `ON CONFLICT DO NOTHING`).
//!
//! V1 honesty rule: `combo` achievements, metadata-qualified counters/streaks
//! (per-day windows, series/season scopes, holiday-window breakdowns, …),
//! and achievements whose inputs the V1 snapshot lacks are **not evaluable**
//! and stay unchanged — never silently unlocked. Metadata-dependent
//! achievements (`metadata_dependent: true`) are likewise skipped until the
//! metadata they need is confirmed mirrored.

use serde_json::Value;

/// The per-character inputs evaluation reads. Every field is a plain number
/// or set the store can derive with simple SQL aggregates (see
/// `PostgresContentStore::evaluate_achievements`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgressSnapshot {
    pub episode_watches: i64,
    pub movie_watches: i64,
    pub current_streak_days: i64,
    pub level: i64,
    pub distinct_genres: i64,
    pub horror_watches: i64,
    pub distinct_holiday_windows: i64,
    pub distinct_new_arrival_titles: i64,
    pub purchased_sub_genres: i64,
    pub completed_featured_cases: i64,
}

/// One achievement's evaluation outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluation {
    /// Threshold met (or condition true) — unlock now, at `progress`.
    Unlock { progress: i64 },
    /// Threshold not met — report live progress toward the target.
    InProgress { progress: i64, target: i64 },
    /// The engine cannot evaluate this achievement in V1.
    NotEvaluable,
}

/// Evaluate one achievement definition against a snapshot.
///
/// `definition` is one row of the seeded `achievements` table: `kind`,
/// `target_value`, and `metadata` (parsed json).
pub fn evaluate(
    kind: &str,
    target_value: Option<i64>,
    metadata: &Value,
    snapshot: &ProgressSnapshot,
) -> Evaluation {
    let metadata = metadata.as_object();
    let is_plain = metadata.is_some_and(|map| map.is_empty());

    // Metadata-dependent rows are never evaluated until their inputs exist.
    if metadata
        .is_some_and(|map| map.get("metadata_dependent").and_then(Value::as_bool).unwrap_or(false))
    {
        return Evaluation::NotEvaluable;
    }

    match kind {
        // Plain streak rows carry `{}` metadata and count current_streak_days.
        "streak" if is_plain => {
            let target = target_value.unwrap_or(0);
            if snapshot.current_streak_days >= target {
                Evaluation::Unlock {
                    progress: snapshot.current_streak_days,
                }
            } else {
                Evaluation::InProgress {
                    progress: snapshot.current_streak_days,
                    target,
                }
            }
        }
        // Level counters count `level` (the seeded Level Up rows) through the
        // same dispatch as every other metric.
        "counter" => {
            let Some(target) = target_value else {
                return Evaluation::NotEvaluable;
            };
            let Some(progress) = counter_progress_from(metadata, snapshot) else {
                return Evaluation::NotEvaluable;
            };
            if progress >= target {
                Evaluation::Unlock { progress }
            } else {
                Evaluation::InProgress { progress, target }
            }
        }
        // `once` and `combo` need condition machinery the V1 snapshot does
        // not carry; anything not dispatched above is honestly not evaluable.
        _ => Evaluation::NotEvaluable,
    }
}

/// Map a counter row's metadata to its snapshot metric. Only the shapes the
/// engine can honestly evaluate in V1 return a value: exactly one metadata
/// key, naming a metric the snapshot carries.
fn counter_progress_from(
    metadata: Option<&serde_json::Map<String, Value>>,
    snapshot: &ProgressSnapshot,
) -> Option<i64> {
    let metadata = metadata?;
    if metadata.len() != 1 {
        return None;
    }
    match metadata.keys().next()?.as_str() {
        "content_type" => match metadata.get("content_type")?.as_str()? {
            "episode" => Some(snapshot.episode_watches),
            "movie" => Some(snapshot.movie_watches),
            _ => None,
        },
        "distinct_genres" => Some(snapshot.distinct_genres),
        "genre" if metadata.get("genre")?.as_str()? == "Horror" => Some(snapshot.horror_watches),
        "purchases" => Some(snapshot.purchased_sub_genres),
        "distinct_windows" => Some(snapshot.distinct_holiday_windows),
        "new_arrival_titles" => Some(snapshot.distinct_new_arrival_titles),
        "case_type" if metadata.get("case_type")?.as_str()? == "featured" => {
            Some(snapshot.completed_featured_cases)
        }
        "level" => Some(snapshot.level),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snapshot() -> ProgressSnapshot {
        ProgressSnapshot {
            episode_watches: 100,
            movie_watches: 10,
            current_streak_days: 7,
            level: 5,
            distinct_genres: 3,
            horror_watches: 10,
            ..ProgressSnapshot::default()
        }
    }

    #[test]
    fn counters_unlock_at_target_and_report_progress_below() {
        let metadata = json!({"content_type": "episode"});
        assert_eq!(
            evaluate("counter", Some(100), &metadata, &snapshot()),
            Evaluation::Unlock { progress: 100 }
        );
        assert_eq!(
            evaluate("counter", Some(150), &metadata, &snapshot()),
            Evaluation::InProgress { progress: 100, target: 150 }
        );
    }

    #[test]
    fn streak_kind_counts_current_streak_for_plain_metadata() {
        assert_eq!(
            evaluate("streak", Some(7), &json!({}), &snapshot()),
            Evaluation::Unlock { progress: 7 }
        );
        assert_eq!(
            evaluate("streak", Some(30), &json!({}), &snapshot()),
            Evaluation::InProgress { progress: 7, target: 30 }
        );
    }

    #[test]
    fn level_counters_track_the_level_metric() {
        let metadata = json!({"level": 5});
        assert_eq!(
            evaluate("counter", Some(5), &metadata, &snapshot()),
            Evaluation::Unlock { progress: 5 }
        );
        assert_eq!(
            evaluate("counter", Some(10), &metadata, &snapshot()),
            Evaluation::InProgress { progress: 5, target: 10 }
        );
    }

    #[test]
    fn combo_once_and_metadata_dependent_rows_are_not_evaluable() {
        assert_eq!(
            evaluate("combo", None, &json!({"streak": 7}), &snapshot()),
            Evaluation::NotEvaluable
        );
        assert_eq!(
            evaluate("once", None, &json!({"content_type": "episode"}), &snapshot()),
            Evaluation::NotEvaluable
        );
        assert_eq!(
            evaluate("counter", Some(10), &json!({"metadata_dependent": true}), &snapshot()),
            Evaluation::NotEvaluable
        );
        // Metadata-qualified streaks (No Gap, Unbroken) stay unevaluated too.
        assert_eq!(
            evaluate("streak", Some(7), &json!({"require_new_arrival": true}), &snapshot()),
            Evaluation::NotEvaluable
        );
    }

    #[test]
    fn unknown_counter_shapes_are_not_evaluable() {
        // A two-key counter is not a shape the engine claims to understand.
        assert_eq!(
            evaluate(
                "counter",
                Some(3),
                &json!({"distinct_windows": 3, "extra": true}),
                &snapshot()
            ),
            Evaluation::NotEvaluable
        );
    }

    #[test]
    fn snapshot_metrics_dispatch_to_their_seeded_shapes() {
        assert_eq!(
            evaluate("counter", Some(3), &json!({"distinct_genres": 3}), &snapshot()),
            Evaluation::Unlock { progress: 3 }
        );
        assert_eq!(
            evaluate("counter", Some(25), &json!({"genre": "Horror"}), &snapshot()),
            Evaluation::InProgress { progress: 10, target: 25 }
        );
        // A differently-named genre is not a snapshot metric in V1.
        assert_eq!(
            evaluate("counter", Some(5), &json!({"genre": "Comedy"}), &snapshot()),
            Evaluation::NotEvaluable
        );
        assert_eq!(
            evaluate("counter", Some(8), &json!({"case_type": "featured"}), &snapshot()),
            Evaluation::InProgress { progress: 0, target: 8 }
        );
        // horror_native's "before any other purchase" condition is not a
        // single-key shape, so it stays unevaluated.
        assert_eq!(
            evaluate(
                "counter",
                Some(50),
                &json!({"genre": "Horror", "before_other_purchase": true}),
                &snapshot()
            ),
            Evaluation::NotEvaluable
        );
    }
}
