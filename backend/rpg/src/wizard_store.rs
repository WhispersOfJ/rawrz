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
