//! Lantern Academy persistence boundary.
//!
//! `wizard.rs` owns pure rules. This module owns only their Postgres adapter:
//! loading progression facts, materializing permanent unlocks, exposing the
//! catalog, and applying the queued loadout at the game-tick boundary. The
//! neutral progression ledgers remain authoritative and are never rewritten.

use crate::persistence::PostgresContentStore;
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use tokio_postgres::Transaction;

pub const CHARACTER_WIZARD_FACTS_SQL: &str = r#"
SELECT COALESCE(cs.level, 1)::bigint,
       (SELECT count(*) FROM watch_orders wo
        WHERE wo.character_id = c.id AND wo.status = 'completed')::bigint,
       (SELECT count(*) FROM character_achievements ca
        WHERE ca.character_id = c.id)::bigint,
       COALESCE(cs.best_streak_days, 0)::bigint,
       COALESCE(cs.genres_accessed, 0)::bigint
FROM characters c
LEFT JOIN character_state cs ON cs.character_id = c.id
WHERE c.account_id = $1
"#;

pub const RESOURCE_STATE_SQL: &str = r#"
SELECT preview_tokens, streak_wards, streak_ward_last_granted_local_date::text
FROM character_wizard_resources
WHERE character_id = $1
"#;

pub const SPELL_DEFINITIONS_SQL: &str = r#"
SELECT id, slug, display_name, description, effect_type, parameters,
       unlock_kind, unlock_target, charge_cap
FROM spells
ORDER BY id
"#;

pub const SPELL_UNLOCK_FACTS_SQL: &str = r#"
SELECT c.id, s.id, s.slug, COALESCE(s.unlock_target, 0),
       COALESCE(cs.level, 1),
       (SELECT count(*) FROM watch_orders wo
        WHERE wo.character_id = c.id AND wo.status = 'completed'),
       (SELECT count(*) FROM character_achievements ca
        WHERE ca.character_id = c.id),
       COALESCE(cs.best_streak_days, 0),
       COALESCE(cs.genres_accessed, 0)
FROM characters c
CROSS JOIN spells s
LEFT JOIN character_state cs ON cs.character_id = c.id
ORDER BY c.id, s.id
"#;

pub const SPELL_STATE_SQL: &str = r#"
SELECT s.id, s.slug, s.display_name, s.description, s.effect_type,
       s.parameters, s.unlock_kind, s.unlock_target, s.charge_cap,
       EXISTS (
         SELECT 1 FROM character_spell_events unlock_event
         WHERE unlock_event.character_id = $1 AND unlock_event.spell_id = s.id
           AND unlock_event.event_type = 'unlock'
           AND unlock_event.outcome = 'unlocked'
       ) AS unlocked,
       sa.selected_genre_id, selected.name, sa.pending_genre_id,
       pending.name, sa.affinity_progress_points,
       sa.last_affinity_change_local_date::text,
       (SELECT count(*) FROM character_spell_ledger grant_row
        WHERE grant_row.character_id = $1 AND grant_row.spell_id = s.id
          AND grant_row.entry_type = 'grant'
          AND grant_row.spent_at IS NULL
          AND grant_row.reserved_at IS NULL)::bigint,
       (SELECT count(*) FROM character_spell_ledger reserved_row
        WHERE reserved_row.character_id = $1 AND reserved_row.spell_id = s.id
          AND reserved_row.entry_type = 'grant'
          AND reserved_row.spent_at IS NULL
          AND reserved_row.reserved_at IS NOT NULL)::bigint
FROM spells s
LEFT JOIN spell_affinities sa
  ON sa.character_id = $1 AND sa.spell_id = s.id
LEFT JOIN genres selected ON selected.id = sa.selected_genre_id
LEFT JOIN genres pending ON pending.id = sa.pending_genre_id
ORDER BY s.id
"#;

pub const PENDING_AFFINITIES_SQL: &str = r#"
SELECT sa.spell_id, s.slug, sa.pending_genre_id, pending.name,
       sa.pending_event_key
FROM spell_affinities sa
JOIN spells s ON s.id = sa.spell_id
JOIN genres pending ON pending.id = sa.pending_genre_id
WHERE sa.character_id = $1 AND sa.pending_genre_id IS NOT NULL
ORDER BY sa.spell_id
FOR UPDATE OF sa
"#;

pub const AVAILABLE_SPELL_GRANT_SQL: &str = r#"
SELECT id
FROM character_spell_ledger
WHERE character_id = $1 AND spell_id = $2
  AND entry_type = 'grant'
  AND spent_at IS NULL AND reserved_at IS NULL
ORDER BY id
LIMIT 1
FOR UPDATE
"#;

pub const ARMED_SPELL_CAST_SQL: &str = r#"
SELECT cast_row.id, cast_row.grant_id, grant_row.id
FROM character_spell_ledger cast_row
JOIN character_spell_ledger grant_row ON grant_row.id = cast_row.grant_id
WHERE cast_row.character_id = $1 AND cast_row.spell_id = $2
  AND cast_row.entry_type = 'cast' AND cast_row.outcome = 'armed'
  AND grant_row.spent_at IS NULL AND grant_row.reserved_at IS NOT NULL
ORDER BY cast_row.id
LIMIT 1
FOR UPDATE OF cast_row, grant_row
"#;

pub const RESOURCE_EVENT_SQL: &str = r#"
INSERT INTO character_resource_events
  (character_id, resource, event_type, source, source_event_key,
   delta, balance_after, outcome, metadata)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (character_id, resource, event_type, source_event_key) DO NOTHING
"#;

pub const SPELL_EVENT_SQL: &str = r#"
INSERT INTO character_spell_events
  (character_id, spell_id, event_type, source, source_event_key,
   outcome, reason, metadata)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (character_id, spell_id, event_type, source_event_key) DO NOTHING
"#;

pub const SPELL_LEDGER_INSERT_SQL: &str = r#"
INSERT INTO character_spell_ledger
  (character_id, spell_id, entry_type, source, source_event_key,
   affinity_points, spent_at, reserved_at, reserved_event_key,
   applied_at, target_order_id, target_item_id, grant_id, outcome, metadata)
VALUES ($1, $2, $3, $4, $5, $6,
        CASE WHEN $7 THEN now() ELSE NULL END,
        CASE WHEN $8 THEN now() ELSE NULL END,
        $9, CASE WHEN $10 THEN now() ELSE NULL END,
        $11, $12, $13, $14, $15)
RETURNING id
"#;

pub const SPELL_AFFINITY_TARGET_SQL: &str = r#"
SELECT g.id, g.name
FROM genres g
WHERE regexp_replace(lower(g.name), '[^a-z0-9]+', '-', 'g') = lower($1)
"#;

pub const SPELL_ACCESSIBLE_GENRE_SQL: &str = r#"
SELECT g.id, g.name
FROM genres g
JOIN genre_access ga ON ga.genre_id = g.id AND ga.character_id = $1
WHERE regexp_replace(lower(g.name), '[^a-z0-9]+', '-', 'g') = lower($2)
"#;

pub const SPELL_AFFINITY_ROW_SQL: &str = r#"
SELECT sa.spell_id, sa.selected_genre_id, sa.pending_genre_id,
       sa.last_affinity_change_local_date, sa.pending_event_key
FROM spell_affinities sa
WHERE sa.character_id = $1 AND sa.spell_id = $2
FOR UPDATE
"#;

pub const ORDER_NEXT_UNRESOLVED_SQL: &str = r#"
SELECT i.id, i.position,
       (SELECT max(fin.position) FROM watch_order_items fin WHERE fin.order_id = i.order_id) AS final_position
FROM watch_order_items i
JOIN watch_orders wo ON wo.id = i.order_id
JOIN characters ch ON ch.id = wo.character_id
LEFT JOIN watches w
  ON w.character_id = wo.character_id AND w.content_id = i.content_id
WHERE ch.account_id = $1 AND wo.id = $2 AND wo.status = 'active'
  AND i.skipped_at IS NULL AND w.id IS NULL
ORDER BY i.position
LIMIT 1
FOR UPDATE OF i
"#;

pub const ORDER_NEXT_SPELL_REVEAL_SQL: &str = r#"
SELECT i.id, i.position, i.order_id,
       (SELECT max(fin.position) FROM watch_order_items fin WHERE fin.order_id = i.order_id) AS final_position
FROM watch_order_items i
JOIN watch_orders wo ON wo.id = i.order_id
JOIN characters ch ON ch.id = wo.character_id
LEFT JOIN watches w
  ON w.character_id = wo.character_id AND w.content_id = i.content_id
WHERE ch.account_id = $1 AND wo.id = $2 AND wo.status = 'active'
  AND i.position > 1 AND i.skipped_at IS NULL AND w.id IS NULL
  AND i.position < (SELECT max(fin.position) FROM watch_order_items fin WHERE fin.order_id = i.order_id)
  AND NOT EXISTS (
    SELECT 1 FROM watch_order_spell_reveals sr
    WHERE sr.order_id = i.order_id AND sr.item_id = i.id
  )
  AND NOT EXISTS (
    SELECT 1
    FROM watch_order_items prev
    LEFT JOIN watches prev_watch
      ON prev_watch.character_id = wo.character_id AND prev_watch.content_id = prev.content_id
    WHERE prev.order_id = i.order_id AND prev.position = i.position - 1
      AND (prev_watch.id IS NOT NULL OR prev.skipped_at IS NOT NULL)
  )
ORDER BY i.position
LIMIT 1
FOR UPDATE OF i
"#;

pub const SPELL_ORDER_TARGET_SQL: &str = r#"
SELECT wo.id
FROM watch_orders wo
JOIN characters ch ON ch.id = wo.character_id
WHERE ch.account_id = $1 AND wo.id = $2 AND wo.status = 'active'
  AND NOT EXISTS (
    SELECT 1 FROM watch_order_items i
    LEFT JOIN watches w
      ON w.character_id = wo.character_id AND w.content_id = i.content_id
    WHERE i.order_id = wo.id AND (i.skipped_at IS NOT NULL OR w.id IS NOT NULL)
  )
FOR UPDATE OF wo
"#;

pub const SPELL_CAST_UPDATE_GRANT_SQL: &str = r#"
UPDATE character_spell_ledger
SET spent_at = now(), reserved_at = NULL, reserved_event_key = NULL,
    applied_at = now()
WHERE id = $1 AND entry_type = 'grant' AND spent_at IS NULL
RETURNING id
"#;

pub const SPELL_CAST_RESERVE_GRANT_SQL: &str = r#"
UPDATE character_spell_ledger
SET reserved_at = now(), reserved_event_key = $2
WHERE id = $1 AND entry_type = 'grant'
  AND spent_at IS NULL AND reserved_at IS NULL
RETURNING id
"#;

pub const ARMED_CASTS_FOR_WATCH_SQL: &str = r#"
SELECT cast_row.id, cast_row.spell_id, spells.slug, cast_row.grant_id
FROM character_spell_ledger cast_row
JOIN spells ON spells.id = cast_row.spell_id
JOIN character_spell_ledger grant_row ON grant_row.id = cast_row.grant_id
WHERE cast_row.character_id = $1 AND cast_row.entry_type = 'cast'
  AND cast_row.outcome = 'armed'
  AND (
    (spells.slug = 'chronicle_ward'
      AND grant_row.spent_at IS NULL AND grant_row.reserved_at IS NOT NULL)
    OR
    (spells.slug = 'focus_sigil'
      AND grant_row.spent_at IS NOT NULL)
  )
ORDER BY cast_row.id
FOR UPDATE OF cast_row, grant_row
"#;

pub const ARMED_CHRONICLE_CAST_SQL: &str = r#"
SELECT cast_row.id, cast_row.grant_id
FROM character_spell_ledger cast_row
JOIN spells ON spells.id = cast_row.spell_id
JOIN character_spell_ledger grant_row ON grant_row.id = cast_row.grant_id
WHERE cast_row.character_id = $1 AND cast_row.spell_id = spells.id
  AND spells.slug = 'chronicle_ward'
  AND cast_row.entry_type = 'cast' AND cast_row.outcome = 'armed'
  AND grant_row.spent_at IS NULL AND grant_row.reserved_at IS NOT NULL
ORDER BY cast_row.id
LIMIT 1
FOR UPDATE OF cast_row, grant_row
"#;

pub const FOCUS_CAST_FOR_WATCH_SQL: &str = r#"
SELECT cast_row.id, cast_row.grant_id
FROM character_spell_ledger cast_row
JOIN spells ON spells.id = cast_row.spell_id
JOIN character_spell_ledger grant_row ON grant_row.id = cast_row.grant_id
WHERE cast_row.character_id = $1 AND spells.slug = 'focus_sigil'
  AND cast_row.entry_type = 'cast' AND cast_row.outcome = 'armed'
  AND grant_row.spent_at IS NOT NULL
ORDER BY cast_row.id
LIMIT 1
FOR UPDATE OF cast_row, grant_row
"#;

pub const APPLY_ARMED_CAST_SQL: &str = r#"
UPDATE character_spell_ledger
SET spent_at = now(), reserved_at = NULL, reserved_event_key = NULL,
    applied_at = now()
WHERE id = $1 AND entry_type = 'grant' AND spent_at IS NULL
"#;

pub const MARK_ARMED_CAST_SQL: &str = r#"
UPDATE character_spell_ledger
SET outcome = 'applied', applied_at = now(), metadata = $2
WHERE id = $1 AND entry_type = 'cast' AND outcome = 'armed'
"#;

pub const STUDY_TARGET_CLEAR_SQL: &str = r#"
UPDATE watch_orders wo
SET study_target = false
FROM characters ch
WHERE wo.character_id = ch.id AND ch.account_id = $1
  AND wo.status = 'active'
"#;

pub const STUDY_TARGET_SET_SQL: &str = r#"
UPDATE watch_orders wo
SET study_target = true
FROM characters ch
WHERE wo.id = $2 AND wo.character_id = ch.id AND ch.account_id = $1
  AND wo.status = 'active'
"#;

pub const PREVIEW_TARGET_SQL: &str = ORDER_NEXT_SPELL_REVEAL_SQL;

pub const WATCH_AFFINITY_TARGETS_SQL: &str = r#"
SELECT sa.spell_id, s.slug, sa.affinity_progress_points,
       s.charge_cap,
       (SELECT count(*) FROM character_spell_ledger grant_row
        WHERE grant_row.character_id = sa.character_id
          AND grant_row.spell_id = sa.spell_id
          AND grant_row.entry_type = 'grant'
          AND grant_row.spent_at IS NULL
          AND grant_row.reserved_at IS NULL)::bigint
FROM spell_affinities sa
JOIN spells s ON s.id = sa.spell_id
JOIN genres g ON g.id = sa.selected_genre_id
JOIN content c ON c.id = $2
WHERE sa.character_id = $1
  AND sa.selected_genre_id IS NOT NULL
  AND c.genres ? g.name
  AND EXISTS (
    SELECT 1 FROM character_spell_events unlock_event
    WHERE unlock_event.character_id = sa.character_id
      AND unlock_event.spell_id = sa.spell_id
      AND unlock_event.event_type = 'unlock'
      AND unlock_event.outcome = 'unlocked'
  )
FOR UPDATE OF sa
"#;

pub const AFFINITY_UPDATE_SQL: &str = r#"
UPDATE spell_affinities
SET affinity_progress_points = $3, updated_at = now()
WHERE character_id = $1 AND spell_id = $2
"#;

pub const AFFINITY_LEDGER_INSERT_SQL: &str = r#"
INSERT INTO character_spell_ledger
  (character_id, spell_id, entry_type, source, source_event_key,
   affinity_points, outcome, metadata)
VALUES ($1, $2, 'affinity', 'watch', $3, $4, 'applied', $5)
ON CONFLICT (character_id, spell_id, entry_type, source_event_key) DO NOTHING
"#;

pub const SPELL_GRANT_INSERT_SQL: &str = r#"
INSERT INTO character_spell_ledger
  (character_id, spell_id, entry_type, source, source_event_key,
   affinity_points, outcome, metadata)
VALUES ($1, $2, 'grant', 'affinity', $3, $4, 'granted', $5)
ON CONFLICT (character_id, spell_id, entry_type, source_event_key) DO NOTHING
"#;

pub const SPELL_OVERFLOW_INSERT_SQL: &str = r#"
INSERT INTO character_spell_ledger
  (character_id, spell_id, entry_type, source, source_event_key,
   affinity_points, outcome, metadata)
VALUES ($1, $2, 'overflow_noop', 'affinity', $3, $4, 'dropped', $5)
ON CONFLICT (character_id, spell_id, entry_type, source_event_key) DO NOTHING
"#;

pub const RESOURCE_SEED_SQL: &str = r#"
INSERT INTO character_wizard_resources (character_id)
VALUES ($1)
ON CONFLICT (character_id) DO NOTHING
"#;

pub const RESOURCE_PREVIEW_GRANT_SQL: &str = r#"
UPDATE character_wizard_resources
SET preview_tokens = preview_tokens + 1, updated_at = now()
WHERE character_id = $1 AND preview_tokens < 1
RETURNING preview_tokens
"#;

pub const RESOURCE_WARD_GRANT_SQL: &str = r#"
UPDATE character_wizard_resources
SET streak_wards = streak_wards + 1,
    streak_ward_last_granted_local_date = $2,
    updated_at = now()
WHERE character_id = $1 AND streak_wards < 1
RETURNING streak_wards
"#;

pub const RESOURCE_WARD_COOLDOWN_GRANT_SQL: &str = r#"
UPDATE character_wizard_resources
SET streak_wards = 1,
    streak_ward_last_granted_local_date = $2,
    updated_at = now()
WHERE character_id = $1
  AND streak_wards = 0
  AND streak_ward_last_granted_local_date IS NOT NULL
  AND streak_ward_last_granted_local_date <= $2 - 30
RETURNING streak_wards
"#;

pub const RESOURCE_PREVIEW_SPEND_SQL: &str = r#"
UPDATE character_wizard_resources
SET preview_tokens = preview_tokens - 1, updated_at = now()
WHERE character_id = $1 AND preview_tokens > 0
RETURNING preview_tokens
"#;

pub const RESOURCE_WARD_SPEND_SQL: &str = r#"
UPDATE character_wizard_resources
SET streak_wards = streak_wards - 1, updated_at = now()
WHERE character_id = $1 AND streak_wards > 0
RETURNING streak_wards
"#;

pub const ARCHETYPE_BOOTSTRAP_SQL: &str = r#"
INSERT INTO character_archetypes (character_id, archetype_id, unlock_source_event_key)
SELECT $1, id, 'bootstrap:lantern_scholar'
FROM wizard_archetypes
WHERE slug = 'lantern_scholar'
ON CONFLICT (character_id, archetype_id) DO NOTHING
"#;

pub const ARCHETYPE_BOOTSTRAP_EVENT_SQL: &str = r#"
INSERT INTO character_archetype_events
  (character_id, archetype_id, event_type, source, source_event_key, outcome, reason)
SELECT $1, id, 'unlock', 'bootstrap', 'bootstrap:lantern_scholar', 'unlocked', 'starter'
FROM wizard_archetypes
WHERE slug = 'lantern_scholar'
ON CONFLICT (character_id, event_type, source_event_key) DO NOTHING
"#;

pub const ARCHETYPE_REFRESH_LOCK_SQL: &str =
    "SELECT pg_advisory_xact_lock(hashtextextended('lantern-academy:unlock-refresh', 0))";

/// Fact extraction is deliberately policy-free; `wizard::unlock_satisfied`
/// is the sole owner of the unlock decision.
pub const ARCHETYPE_UNLOCK_FACTS_SQL: &str = r#"
SELECT c.id, a.id, a.slug, COALESCE(a.unlock_target, 0),
       COALESCE(cs.level, 1),
       (SELECT count(*) FROM watch_orders wo
        WHERE wo.character_id = c.id AND wo.status = 'completed'),
       (SELECT count(*) FROM character_achievements ca
        WHERE ca.character_id = c.id),
       COALESCE(cs.best_streak_days, 0),
       COALESCE(cs.genres_accessed, 0)
FROM characters c
CROSS JOIN wizard_archetypes a
LEFT JOIN character_state cs ON cs.character_id = c.id
ORDER BY c.id, a.id
"#;

pub const ARCHETYPE_UNLOCK_INSERT_SQL: &str = r#"
INSERT INTO character_archetypes (character_id, archetype_id, unlock_source_event_key)
VALUES ($1, $2, $3)
ON CONFLICT (character_id, archetype_id) DO NOTHING
"#;

pub const ARCHETYPE_UNLOCK_EVENT_SQL: &str = r#"
INSERT INTO character_archetype_events
  (character_id, archetype_id, event_type, source, source_event_key, outcome, reason)
SELECT ca.character_id, ca.archetype_id, 'unlock', 'rule_evaluation',
       ca.unlock_source_event_key, 'unlocked', 'condition_satisfied'
FROM character_archetypes ca
WHERE NOT EXISTS (
  SELECT 1 FROM character_archetype_events e
  WHERE e.character_id = ca.character_id
    AND e.event_type = 'unlock'
    AND e.source_event_key = ca.unlock_source_event_key
)
ON CONFLICT (character_id, event_type, source_event_key) DO NOTHING
"#;

pub const ARCHETYPE_STATE_SQL: &str = r#"
SELECT a.slug, a.display_name, a.description, a.portrait_key,
       a.primary_effect, a.secondary_effect, a.unlock_kind, a.unlock_target,
       a.strengths, a.weaknesses, ca.character_id IS NOT NULL AS unlocked,
       active.slug, pending.slug, c.archetype_selected_local_date::text
FROM characters c
JOIN wizard_archetypes active ON active.id = c.active_archetype_id
LEFT JOIN wizard_archetypes pending ON pending.id = c.pending_archetype_id
CROSS JOIN wizard_archetypes a
LEFT JOIN character_archetypes ca
  ON ca.character_id = c.id AND ca.archetype_id = a.id
WHERE c.account_id = $1
ORDER BY a.id
"#;

pub const SELECT_ARCHETYPE_TARGET_SQL: &str = r#"
SELECT a.id, c.id, active.slug, pending.slug,
       c.archetype_selected_local_date, ca.character_id IS NOT NULL AS unlocked
FROM characters c
JOIN wizard_archetypes active ON active.id = c.active_archetype_id
LEFT JOIN wizard_archetypes pending ON pending.id = c.pending_archetype_id
JOIN wizard_archetypes a ON a.slug = $1
LEFT JOIN character_archetypes ca
  ON ca.character_id = c.id AND ca.archetype_id = a.id
WHERE c.account_id = (SELECT id FROM accounts ORDER BY id LIMIT 1)
FOR UPDATE OF c
"#;

pub const QUEUE_ARCHETYPE_SELECTION_SQL: &str = r#"
UPDATE characters
SET pending_archetype_id = $2,
    pending_archetype_requested_at = now(),
    pending_archetype_event_key = $3,
    archetype_selected_local_date = $4
WHERE id = $1
  AND pending_archetype_id IS NULL
"#;

pub const APPLY_PENDING_ARCHETYPE_SQL: &str = r#"
SELECT c.id, c.pending_archetype_id, c.pending_archetype_event_key
FROM characters c
WHERE c.pending_archetype_id IS NOT NULL
ORDER BY c.id
LIMIT 1
FOR UPDATE
"#;

pub const APPLY_ARCHETYPE_SQL: &str = r#"
UPDATE characters
SET active_archetype_id = $2,
    pending_archetype_id = NULL,
    pending_archetype_requested_at = NULL,
    pending_archetype_event_key = NULL
WHERE id = $1
"#;

pub const ARCHETYPE_EVENT_SQL: &str = r#"
INSERT INTO character_archetype_events
  (character_id, archetype_id, event_type, source, source_event_key, outcome, reason)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (character_id, event_type, source_event_key) DO NOTHING
"#;

pub const ACTIVE_ARCHETYPE_SQL: &str = r#"
SELECT a.slug
FROM characters c
JOIN wizard_archetypes a ON a.id = c.active_archetype_id
WHERE c.account_id = $1
"#;

pub const GENRE_ACCESS_TARGET_SQL: &str = r#"
SELECT c.id, g.id, g.name, g.list_order, g.is_opening,
       cs.level, cs.genres_accessed,
       ga.character_id IS NOT NULL AS accessed
FROM characters c
JOIN character_state cs ON cs.character_id = c.id
JOIN genres g ON g.name = $1
LEFT JOIN genre_access ga ON ga.character_id = c.id AND ga.genre_id = g.id
WHERE c.account_id = (SELECT id FROM accounts ORDER BY id LIMIT 1)
FOR UPDATE OF c, cs
"#;

pub const GENRE_ACCESS_INSERT_SQL: &str = r#"
INSERT INTO genre_access (character_id, genre_id)
VALUES ($1, $2)
ON CONFLICT (character_id, genre_id) DO NOTHING
"#;

pub const GENRE_COUNT_UPDATE_SQL: &str = r#"
UPDATE character_state
SET genres_accessed = genres_accessed + 1
WHERE character_id = $1
  AND genres_accessed = $2
  AND genres_accessed < $3
"#;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GenreAccessState {
    pub name: String,
    pub accessed: bool,
    pub level: i32,
    pub genres_accessed: i32,
}

/// Seed the starter unlock and its audit event as part of character bootstrap.
pub(crate) async fn seed_starter_archetype(
    transaction: &Transaction<'_>,
    character_id: i64,
) -> Result<()> {
    transaction
        .execute(ARCHETYPE_BOOTSTRAP_SQL, &[&character_id])
        .await?;
    transaction
        .execute(ARCHETYPE_BOOTSTRAP_EVENT_SQL, &[&character_id])
        .await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArchetypeView {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub portrait_key: String,
    pub primary_effect: Value,
    pub secondary_effect: Option<Value>,
    pub unlock_kind: String,
    pub unlock_target: Option<i64>,
    pub strengths: Value,
    pub weaknesses: Value,
    pub unlocked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArchetypeState {
    pub active_archetype: String,
    pub pending_archetype: Option<String>,
    pub archetype_selected_local_date: Option<String>,
    pub archetypes: Vec<ArchetypeView>,
}

impl PostgresContentStore {
    /// Refreshes permanent unlock rows and audit events from one consistent
    /// fact snapshot. The advisory lock serializes refreshes across callers.
    pub(crate) async fn refresh_archetype_unlocks(&self) -> Result<()> {
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;
        transaction
            .query_one(ARCHETYPE_REFRESH_LOCK_SQL, &[])
            .await?;
        let rows = transaction.query(ARCHETYPE_UNLOCK_FACTS_SQL, &[]).await?;
        for row in rows {
            let character_id: i64 = row.get(0);
            let archetype_id: i64 = row.get(1);
            let slug: String = row.get(2);
            let target: i64 = row.get(3);
            let facts = crate::wizard::UnlockFacts {
                level: row.get(4),
                completed_orders: row.get(5),
                achievements: row.get(6),
                best_streak_days: row.get(7),
                genres_accessed: row.get(8),
            };
            if crate::wizard::unlock_satisfied(&slug, target, facts) {
                let source_event_key = if slug == crate::wizard::LANTERN_SCHOLAR {
                    "bootstrap:lantern_scholar".to_owned()
                } else {
                    format!("unlock:{slug}")
                };
                transaction
                    .execute(
                        ARCHETYPE_UNLOCK_INSERT_SQL,
                        &[&character_id, &archetype_id, &source_event_key],
                    )
                    .await?;
            }
        }
        transaction.execute(ARCHETYPE_UNLOCK_EVENT_SQL, &[]).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Returns the catalog plus the current and pending loadout. Locked
    /// definitions are intentionally still included for guided UI.
    pub async fn archetype_state(&self) -> Result<Option<ArchetypeState>> {
        self.refresh_archetype_unlocks().await?;
        self.load_archetype_state().await
    }

    /// Reads the archetype state without the refresh pass — for callers that
    /// already refreshed in the same flow (F-23: one refresh, not two).
    async fn load_archetype_state(&self) -> Result<Option<ArchetypeState>> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(None);
        };
        let rows = self
            .connection()
            .await?
            .query(ARCHETYPE_STATE_SQL, &[&account_id])
            .await?;
        let Some(first) = rows.first() else {
            return Ok(None);
        };
        let active_archetype: String = first.get(11);
        let pending_archetype: Option<String> = first.get(12);
        let archetype_selected_local_date: Option<String> = first.get(13);
        let mut archetypes = Vec::with_capacity(rows.len());
        for row in rows {
            archetypes.push(ArchetypeView {
                slug: row.get(0),
                display_name: row.get(1),
                description: row.get(2),
                portrait_key: row.get(3),
                primary_effect: row.get(4),
                secondary_effect: row.get(5),
                unlock_kind: row.get(6),
                unlock_target: row.get(7),
                strengths: row.get(8),
                weaknesses: row.get(9),
                unlocked: row.get(10),
            });
        }
        Ok(Some(ArchetypeState {
            active_archetype,
            pending_archetype,
            archetype_selected_local_date,
            archetypes,
        }))
    }

    /// Queues one unlocked archetype without changing the active loadout.
    /// Database row-locking and the affected-row check make conflicts atomic.
    pub async fn select_archetype(&self, slug: &str) -> Result<ArchetypeState> {
        self.refresh_archetype_unlocks().await?;
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;
        let row = transaction
            .query_opt(SELECT_ARCHETYPE_TARGET_SQL, &[&slug])
            .await?;
        let Some(row) = row else {
            return Err(crate::ProbeError::InvalidArchetypeSelection(
                "unknown archetype".to_owned(),
            ));
        };
        let archetype_id: i64 = row.get(0);
        let character_id: i64 = row.get(1);
        let active_slug: String = row.get(2);
        let pending_slug: Option<String> = row.get(3);
        let selected_date: Option<chrono::NaiveDate> = row.get(4);
        let unlocked: bool = row.get(5);
        let today = chrono::Local::now().date_naive();

        if let Err(rejection) = crate::wizard::selection_allowed(
            unlocked,
            slug,
            &active_slug,
            pending_slug.is_some(),
            selected_date,
            today,
        ) {
            return Err(crate::ProbeError::ArchetypeSelectionConflict(
                rejection.reason().to_owned(),
            ));
        }

        // F-8: a clock failure must not collapse the event key to a
        // constant ("…:0") and silently drop the audit event via the ON
        // CONFLICT guard — fall back to a random key instead.
        let event_timestamp = chrono::Utc::now()
            .timestamp_nanos_opt()
            .map(|nanos| nanos.to_string())
            .unwrap_or_else(crate::random_token_hex);
        let event_key = format!("archetype-select:{slug}:{event_timestamp}");
        let queued = transaction
            .execute(
                QUEUE_ARCHETYPE_SELECTION_SQL,
                &[&character_id, &archetype_id, &event_key, &today],
            )
            .await?;
        if queued != 1 {
            return Err(crate::ProbeError::ArchetypeSelectionConflict(
                "another selection was accepted concurrently".to_owned(),
            ));
        }
        transaction
            .execute(
                ARCHETYPE_EVENT_SQL,
                &[
                    &character_id,
                    &archetype_id,
                    &"selection_requested",
                    &"api",
                    &event_key,
                    &"accepted",
                    &Option::<String>::None,
                ],
            )
            .await?;
        transaction.commit().await?;
        // F-23: this flow refreshed at entry, so read the state directly
        // instead of re-running the unlock pass.
        self.load_archetype_state().await?.ok_or_else(|| {
            crate::ProbeError::InvalidArchetypeSelection("character missing".to_owned())
        })
    }

    /// Applies one pending selection before watch awards in the next tick.
    pub async fn apply_pending_archetype(&self) -> Result<usize> {
        self.refresh_archetype_unlocks().await?;
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;
        let Some(row) = transaction
            .query_opt(APPLY_PENDING_ARCHETYPE_SQL, &[])
            .await?
        else {
            transaction.commit().await?;
            return Ok(0);
        };
        let character_id: i64 = row.get(0);
        let pending_id: i64 = row.get(1);
        let event_key: String = row.get(2);
        transaction
            .execute(APPLY_ARCHETYPE_SQL, &[&character_id, &pending_id])
            .await?;
        transaction
            .execute(
                ARCHETYPE_EVENT_SQL,
                &[
                    &character_id,
                    &pending_id,
                    &"selection_applied",
                    &"game_tick",
                    &event_key,
                    &"applied",
                    &Option::<String>::None,
                ],
            )
            .await?;
        transaction.commit().await?;
        Ok(1)
    }

    /// Accesses the next genre through the real level-gated transition.
    /// The row and counter are committed together; callers cannot manufacture
    /// `genres_accessed` by editing character_state alone.
    pub async fn access_genre(&self, name: &str) -> Result<GenreAccessState> {
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;
        let Some(row) = transaction
            .query_opt(GENRE_ACCESS_TARGET_SQL, &[&name])
            .await?
        else {
            return Err(crate::ProbeError::InvalidGenreAccess(
                "unknown genre".to_owned(),
            ));
        };
        let character_id: i64 = row.get(0);
        let genre_id: i64 = row.get(1);
        let genre_name: String = row.get(2);
        let list_order: i32 = row.get(3);
        let is_opening: bool = row.get(4);
        let level: i32 = row.get(5);
        let genres_accessed: i32 = row.get(6);
        let accessed: bool = row.get(7);

        if accessed || is_opening {
            return Err(crate::ProbeError::InvalidGenreAccess(
                "genre is already accessed".to_owned(),
            ));
        }
        if list_order != genres_accessed + 1 {
            return Err(crate::ProbeError::InvalidGenreAccess(
                "genre is not next in the academy cascade".to_owned(),
            ));
        }
        if list_order > level {
            return Err(crate::ProbeError::InvalidGenreAccess(
                "level has not opened this genre".to_owned(),
            ));
        }

        transaction
            .execute(GENRE_ACCESS_INSERT_SQL, &[&character_id, &genre_id])
            .await?;
        let updated = transaction
            .execute(
                GENRE_COUNT_UPDATE_SQL,
                &[&character_id, &genres_accessed, &level],
            )
            .await?;
        if updated != 1 {
            return Err(crate::ProbeError::InvalidGenreAccess(
                "genre access changed concurrently".to_owned(),
            ));
        }
        transaction.commit().await?;
        Ok(GenreAccessState {
            name: genre_name,
            accessed: true,
            level,
            genres_accessed: genres_accessed + 1,
        })
    }

    pub(crate) async fn active_archetype_slug(&self, account_id: i64) -> Result<String> {
        Ok(self
            .connection()
            .await?
            .query_one(ACTIVE_ARCHETYPE_SQL, &[&account_id])
            .await?
            .get(0))
    }
}
