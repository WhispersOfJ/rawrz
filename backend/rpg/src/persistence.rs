use crate::enrichment::{MetadataCache, ProviderCacheEntry};
use crate::migrations::MIGRATIONS;
use crate::sync::{
    ContentIdentityKey, ContentSyncGroup, ContentSyncRecord, EnrichedSyncOutcome,
    ProviderSyncFailure, StackSyncFailure,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use crate::Result;
use tokio_postgres::{Client, NoTls};

pub const SCHEMA_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version    text PRIMARY KEY,
  applied_at timestamptz NOT NULL DEFAULT now()
);
"#;

pub const MIGRATION_LOOKUP_SQL: &str =
    "SELECT version FROM schema_migrations WHERE version = $1";
pub const MIGRATION_RECORD_SQL: &str =
    "INSERT INTO schema_migrations (version) VALUES ($1)";

fn migration_summary(applied: usize, already_applied: usize) -> MigrationSummary {
    MigrationSummary {
        applied,
        already_applied,
    }
}

fn bootstrap_summary(
    character_state_seeded: usize,
    genre_access_seeded: usize,
    settings_seeded: usize,
) -> BootstrapSummary {
    BootstrapSummary {
        character_state_seeded,
        genre_access_seeded,
        settings_seeded,
    }
}

/// Shared seeding body used by both the standalone bootstrap and the
/// set-PIN flow (spec §6.4.11): seeds the singleton `character_state` row,
/// the opening-genre `genre_access` row, and any missing `settings` V1
/// defaults for `character_id`. Every insert is idempotent.
async fn seed_rows_for(
    transaction: &tokio_postgres::Transaction<'_>,
    character_id: i64,
) -> Result<BootstrapSummary> {
    let character_state_seeded = transaction
        .execute(CHARACTER_STATE_SEED_SQL, &[&character_id])
        .await?;
    let genre_access_seeded = transaction
        .execute(GENRE_ACCESS_SEED_SQL, &[&character_id])
        .await?;

    let keys: Vec<&str> = SETTINGS_V1_DEFAULTS
        .iter()
        .map(|(key, _)| *key)
        .collect();
    let values: Vec<&str> = SETTINGS_V1_DEFAULTS
        .iter()
        .map(|(_, value)| *value)
        .collect();
    let settings_seeded = transaction
        .execute(SETTINGS_SEED_SQL, &[&character_id, &keys, &values])
        .await?;

    Ok(bootstrap_summary(
        character_state_seeded as usize,
        genre_access_seeded as usize,
        settings_seeded as usize,
    ))
}

pub const CONTENT_UPSERT_SQL: &str = r#"
INSERT INTO content (
  source, source_id, external_id, external_id_type, title, year, content_type,
  parent_id, season_number, episode_number, runtime_seconds, release_date,
  first_air_date, status, summary, rating, rating_source, poster_url, fanart_url,
  section_key, section_title, genres, sub_genres, metadata_blob, provider_metadata,
  last_enriched_at
)
VALUES (
  $1, $2, $3, $4, $5, $6, $7,
  (SELECT parent.id FROM content AS parent
   WHERE parent.source = $8 AND parent.source_id = $9),
  $10, $11, $12, $13::text::date, $14::text::date, $15, $16, $17, $18, $19, $20,
  $21, $22, $23::jsonb, $24::jsonb, $25::jsonb, $26::jsonb,
  CASE WHEN $26::jsonb <> '{}'::jsonb THEN now() ELSE NULL END
)
ON CONFLICT (source, source_id) DO UPDATE SET
  external_id = EXCLUDED.external_id,
  external_id_type = EXCLUDED.external_id_type,
  title = EXCLUDED.title,
  year = EXCLUDED.year,
  content_type = EXCLUDED.content_type,
  parent_id = EXCLUDED.parent_id,
  season_number = EXCLUDED.season_number,
  episode_number = EXCLUDED.episode_number,
  runtime_seconds = EXCLUDED.runtime_seconds,
  release_date = EXCLUDED.release_date,
  first_air_date = EXCLUDED.first_air_date,
  status = EXCLUDED.status,
  summary = EXCLUDED.summary,
  rating = EXCLUDED.rating,
  rating_source = EXCLUDED.rating_source,
  poster_url = EXCLUDED.poster_url,
  fanart_url = EXCLUDED.fanart_url,
  section_key = EXCLUDED.section_key,
  section_title = EXCLUDED.section_title,
  genres = EXCLUDED.genres,
  sub_genres = EXCLUDED.sub_genres,
  metadata_blob = EXCLUDED.metadata_blob,
  provider_metadata = EXCLUDED.provider_metadata,
  last_synced_at = now(),
  last_enriched_at = CASE
    WHEN EXCLUDED.provider_metadata <> '{}'::jsonb THEN now()
    ELSE content.last_enriched_at
  END
RETURNING id, source, source_id;
"#;

pub const PROVIDER_CACHE_UPSERT_SQL: &str = r#"
INSERT INTO content_provider_cache (
  content_id, provider, provider_id, payload, fetched_at, expires_at,
  http_status, error
)
VALUES (
  (SELECT content.id FROM content
   WHERE content.source = $1 AND content.source_id = $2),
  $3, $4, $5::jsonb, to_timestamp($6),
  CASE WHEN $7 IS NULL THEN NULL ELSE to_timestamp($7) END,
  $8, $9
)
ON CONFLICT (content_id, provider, provider_id) DO UPDATE SET
  payload = EXCLUDED.payload,
  fetched_at = EXCLUDED.fetched_at,
  expires_at = EXCLUDED.expires_at,
  http_status = EXCLUDED.http_status,
  error = EXCLUDED.error
RETURNING id, content_id, provider, provider_id;
"#;

pub const PROVIDER_CACHE_HYDRATE_SQL: &str = r#"
SELECT
  content.content_type,
  content.external_id_type,
  content.external_id,
  content.title,
  content.year,
  cache.provider,
  cache.provider_id,
  cache.payload,
  EXTRACT(EPOCH FROM cache.fetched_at)::bigint,
  EXTRACT(EPOCH FROM cache.expires_at)::bigint,
  cache.http_status,
  cache.error
FROM content_provider_cache AS cache
JOIN content ON content.id = cache.content_id
ORDER BY cache.id;
"#;

// Character-creation bootstrap (spec §6.4.11): V1 is a single account with a
// single investigator, so bootstrap resolves that character by account. All
// inserts are idempotent so re-running bootstrap is a no-op. The PIN-set flow
// calls this after account creation; with no account row, bootstrap is a
// no-op (§16 probe: Postgres not yet provisioned on the host).
pub const SINGLE_CHARACTER_ID_SQL: &str = r#"
SELECT characters.id
FROM characters
JOIN accounts ON accounts.id = characters.account_id
ORDER BY characters.id
LIMIT 1
"#;

pub const CHARACTER_STATE_SEED_SQL: &str = r#"
INSERT INTO character_state (character_id)
VALUES ($1)
ON CONFLICT (character_id) DO NOTHING
"#;

pub const GENRE_ACCESS_SEED_SQL: &str = r#"
INSERT INTO genre_access (character_id, genre_id)
SELECT $1, genres.id
FROM genres
WHERE genres.is_opening
ON CONFLICT (character_id, genre_id) DO NOTHING
"#;pub const SETTINGS_SEED_SQL: &str = r#"
INSERT INTO settings (character_id, key, value)
SELECT $1, key, value
FROM unnest($2::text[], $3::text[]) AS seed(key, value)
WHERE NOT EXISTS (
  SELECT 1 FROM settings
  WHERE settings.character_id = $1 AND settings.key = seed.key
)
"#;

// PIN gate account flows (spec §6.4.1 / §7.3): set-PIN creates the single
// account exactly once; verify reads the single account's PHC pin_hash.
pub const ACCOUNT_EXISTS_SQL: &str = "SELECT EXISTS (SELECT 1 FROM accounts)";

pub const ACCOUNT_INSERT_SQL: &str = r#"
INSERT INTO accounts (pin_hash, pin_salts)
VALUES ($1, $2)
RETURNING id
"#;

pub const SINGLE_ACCOUNT_PIN_SQL: &str = r#"
SELECT pin_hash FROM accounts
ORDER BY id
LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContentUpsertParams {
    pub source: String,
    pub source_id: String,
    pub external_id: Option<String>,
    pub external_id_type: Option<String>,
    pub title: String,
    pub year: Option<i32>,
    pub content_type: String,
    pub parent_source: Option<String>,
    pub parent_source_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub runtime_seconds: Option<i32>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
    pub status: Option<String>,
    pub summary: Option<String>,
    pub rating: Option<f64>,
    pub rating_source: Option<String>,
    pub poster_url: Option<String>,
    pub fanart_url: Option<String>,
    pub section_key: Option<String>,
    pub section_title: Option<String>,
    pub genres: Value,
    pub sub_genres: Value,
    pub metadata_blob: Value,
    pub provider_metadata: Value,
}

impl From<&ContentSyncRecord> for ContentUpsertParams {
    fn from(record: &ContentSyncRecord) -> Self {
        Self {
            source: record.key.source.clone(),
            source_id: record.key.source_id.clone(),
            external_id: record.external_id.clone(),
            external_id_type: record.external_id_type.clone(),
            title: record.title.clone(),
            year: record.year,
            content_type: record.content_type.as_str().to_owned(),
            parent_source: record.parent_key.as_ref().map(|key| key.source.clone()),
            parent_source_id: record
                .parent_key
                .as_ref()
                .map(|key| key.source_id.clone()),
            season_number: record.season_number,
            episode_number: record.episode_number,
            runtime_seconds: record.runtime_seconds,
            release_date: record.release_date.clone(),
            first_air_date: record.first_air_date.clone(),
            status: record.status.clone(),
            summary: record.summary.clone(),
            rating: record.rating,
            rating_source: record.rating_source.clone(),
            poster_url: record.poster_url.clone(),
            fanart_url: record.fanart_url.clone(),
            section_key: record.section_key.clone(),
            section_title: record.section_title.clone(),
            genres: serde_json::to_value(&record.genres).expect("genres are serializable"),
            sub_genres: serde_json::to_value(&record.sub_genres)
                .expect("sub-genres are serializable"),
            metadata_blob: record.metadata_blob.clone(),
            provider_metadata: record.provider_metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderCacheUpsertParams {
    pub content_source: String,
    pub content_source_id: String,
    pub provider: String,
    pub provider_id: String,
    pub payload: Value,
    pub fetched_at_epoch: u64,
    pub expires_at_epoch: Option<u64>,
    pub http_status: Option<u16>,
    pub error: Option<String>,
}

impl ProviderCacheUpsertParams {
    fn from_entry(entry: &ProviderCacheEntry, content_source: String, content_source_id: String) -> Self {
        Self {
            content_source,
            content_source_id,
            provider: entry.key.provider.clone(),
            provider_id: entry.key.provider_id.clone(),
            payload: entry.payload.clone().unwrap_or(Value::Null),
            fetched_at_epoch: entry.fetched_at.unwrap_or(entry.attempted_at),
            expires_at_epoch: entry.expires_at,
            http_status: entry.http_status,
            error: entry.error.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersistenceWarning {
    pub content_id: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContentPersistencePlan {
    pub content: Vec<ContentUpsertParams>,
    pub provider_cache: Vec<ProviderCacheUpsertParams>,
    pub stack_failures: Vec<StackSyncFailure>,
    pub provider_failures: Vec<ProviderSyncFailure>,
    pub warnings: Vec<PersistenceWarning>,
}

impl ContentPersistencePlan {
    pub fn from_outcome(outcome: &EnrichedSyncOutcome, cache: &MetadataCache) -> Self {
        let mut records = BTreeMap::new();
        let mut groups_by_cache_id = BTreeMap::new();

        for group in outcome.batch.groups.values() {
            groups_by_cache_id.insert(group.identity.cache_id(), group);
            for record in group.records.values() {
                records.insert(record.key.clone(), record);
            }
        }

        let mut content = records
            .values()
            .map(|record| ContentUpsertParams::from(*record))
            .collect::<Vec<_>>();
        content.sort_by(|left, right| {
            left.parent_source
                .is_some()
                .cmp(&right.parent_source.is_some())
                .then_with(|| left.source.cmp(&right.source))
                .then_with(|| left.source_id.cmp(&right.source_id))
        });

        let mut provider_cache = Vec::new();
        let mut warnings = Vec::new();
        for entry in cache.entries() {
            let Some(group) = groups_by_cache_id.get(&entry.key.content_id) else {
                warnings.push(PersistenceWarning {
                    content_id: entry.key.content_id.clone(),
                    detail: "provider cache entry has no content group in this sync".to_owned(),
                });
                continue;
            };
            let Some(owner) = owner_record(group) else {
                warnings.push(PersistenceWarning {
                    content_id: entry.key.content_id.clone(),
                    detail: "provider cache entry has an empty content group".to_owned(),
                });
                continue;
            };
            provider_cache.push(ProviderCacheUpsertParams::from_entry(
                entry,
                owner.key.source.clone(),
                owner.key.source_id.clone(),
            ));
        }
        provider_cache.sort_by(|left, right| {
            left.content_source
                .cmp(&right.content_source)
                .then_with(|| left.content_source_id.cmp(&right.content_source_id))
                .then_with(|| left.provider.cmp(&right.provider))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        });

        Self {
            content,
            provider_cache,
            stack_failures: outcome.stack_failures.clone(),
            provider_failures: outcome.provider_failures.clone(),
            warnings,
        }
    }
}

fn owner_record(group: &ContentSyncGroup) -> Option<&ContentSyncRecord> {
    group.records.values().min_by(|left, right| {
        source_rank(&left.key.source)
            .cmp(&source_rank(&right.key.source))
            .then_with(|| left.key.source_id.cmp(&right.key.source_id))
    })
}

fn source_rank(source: &str) -> u8 {
    match source {
        "radarr" => 0,
        "sonarr" => 1,
        "plex" => 2,
        _ => 3,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PersistedProviderCacheFields {
    content_type: String,
    external_id_type: Option<String>,
    external_id: Option<String>,
    title: String,
    year: Option<i32>,
    provider: String,
    provider_id: String,
    payload: Value,
    fetched_at_epoch: i64,
    expires_at_epoch: Option<i64>,
    http_status: Option<i32>,
    error: Option<String>,
}

fn provider_cache_entry_from_fields(
    fields: PersistedProviderCacheFields,
) -> Option<ProviderCacheEntry> {
    if fields.provider.is_empty() || fields.provider_id.is_empty() {
        return None;
    }
    let identity = ContentIdentityKey::from_persisted_fields(
        &fields.content_type,
        fields.external_id_type.as_deref(),
        fields.external_id.as_deref(),
        &fields.title,
        fields.year,
    )?;
    let fetched_at = u64::try_from(fields.fetched_at_epoch).ok()?;
    let expires_at = fields
        .expires_at_epoch
        .map(u64::try_from)
        .transpose()
        .ok()?;
    Some(ProviderCacheEntry {
        key: crate::enrichment::ProviderCacheKey::new(
            fields.provider,
            fields.provider_id,
            identity.cache_id(),
        ),
        payload: (!fields.payload.is_null()).then_some(fields.payload),
        fetched_at: Some(fetched_at),
        expires_at,
        // The schema records the successful fetch time but not a separate attempt time.
        attempted_at: fetched_at,
        http_status: fields
            .http_status
            .and_then(|status| u16::try_from(status).ok()),
        error: fields.error,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MigrationSummary {
    pub applied: usize,
    pub already_applied: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CacheHydrationSummary {
    pub loaded: usize,
    pub already_present: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PersistenceSummary {
    pub content_rows: usize,
    pub provider_cache_rows: usize,
}

pub struct PostgresContentStore {
    client: Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BootstrapSummary {
    pub character_state_seeded: usize,
    pub genre_access_seeded: usize,
    pub settings_seeded: usize,
}

/// V1 settings defaults (§6.4.10), verbatim. Missing keys are seeded at
/// character-creation bootstrap; existing values are never overwritten.
pub const SETTINGS_V1_DEFAULTS: &[(&str, &str)] = &[
    ("near_end_threshold_pct", "95"),
    ("new_arrival_window_hours", "48"),
    ("poll_interval_seconds", "300"),
    ("provider_cache_ttl_seconds", "86400"),
    ("provider_max_concurrency", "2"),
    (
        "genre_list_order",
        "[\"Horror\",\"Thriller\",\"Mystery\",\"Sci-Fi\",\"Fantasy\",\"Documentary\",\"Comedy\",\"Drama\",\"Romance\",\"Animation\"]",
    ),
    ("sub_genre_purchase_xp_threshold", "100"),
    (
        "holiday_windows",
        "[{\"name\":\"halloween\",\"start\":\"10-01\",\"end\":\"10-31\",\"genres\":[\"Horror\"],\"multiplier\":1.5},{\"name\":\"winter_holiday\",\"start\":\"12-01\",\"end\":\"12-31\",\"genres\":[\"Comedy\",\"Drama\"],\"multiplier\":1.5}]",
    ),
    ("perks_unlocked", "[]"),
    ("daily_budget_enabled", "false"),
    ("daily_budget_actions", "3"),
    ("featured_selection_mode", "all_time_ranking"),
    ("fame_enabled", "false"),
];
impl PostgresContentStore {
    pub fn validate_database_url(database_url: &str) -> Result<()> {
        if database_url.trim().is_empty() {
            return Err(crate::ProbeError::MissingEnvironment(
                "RPG_DB_URL".to_owned(),
            ));
        }
        Ok(())
    }

    pub async fn connect(database_url: &str) -> Result<Self> {
        Self::validate_database_url(database_url)?;
        let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self { client })
    }

    pub async fn migrate(&mut self) -> Result<MigrationSummary> {
        let transaction = self.client.transaction().await?;
        transaction.batch_execute(SCHEMA_MIGRATIONS_SQL).await?;
        let mut applied = 0;
        let mut already_applied = 0;

        for &(version, sql) in MIGRATIONS {
            if transaction
                .query_opt(MIGRATION_LOOKUP_SQL, &[&version])
                .await?
                .is_some()
            {
                already_applied += 1;
                continue;
            }
            transaction.batch_execute(sql).await?;
            transaction
                .execute(MIGRATION_RECORD_SQL, &[&version])
                .await?;
            applied += 1;
        }

        transaction.commit().await?;
        Ok(migration_summary(applied, already_applied))
    }

    /// Character-creation bootstrap (§6.4.11): seeds the singleton
    /// `character_state` row, the opening-genre `genre_access` row, and any
    /// missing V1 `settings` defaults for the account's single character —
    /// all in one transaction, idempotently. A no-op when no account exists
    /// yet (the PIN-set flow calls this after account creation).
    pub async fn bootstrap_single_character(&mut self) -> Result<BootstrapSummary> {
        let transaction = self.client.transaction().await?;

        let Some(character_id) = transaction
            .query_opt(SINGLE_CHARACTER_ID_SQL, &[])
            .await?
            .map(|row| row.get::<_, i64>(0))
        else {
            transaction.rollback().await?;
            return Ok(bootstrap_summary(0, 0, 0));
        };

        let summary = seed_rows_for(&transaction, character_id).await?;
        transaction.commit().await?;
        Ok(summary)
    }

    pub async fn hydrate_cache(
        &self,
        cache: &mut MetadataCache,
    ) -> Result<CacheHydrationSummary> {
        let rows = self.client.query(PROVIDER_CACHE_HYDRATE_SQL, &[]).await?;
        let mut summary = CacheHydrationSummary {
            loaded: 0,
            already_present: 0,
            skipped: 0,
        };
        for row in rows {
            let fields = PersistedProviderCacheFields {
                content_type: row.get(0),
                external_id_type: row.get(1),
                external_id: row.get(2),
                title: row.get(3),
                year: row.get(4),
                provider: row.get(5),
                provider_id: row.get(6),
                payload: row.get(7),
                fetched_at_epoch: row.get(8),
                expires_at_epoch: row.get(9),
                http_status: row.get(10),
                error: row.get(11),
            };
            let Some(entry) = provider_cache_entry_from_fields(fields) else {
                summary.skipped += 1;
                continue;
            };
            if cache.restore_if_absent(entry) {
                summary.loaded += 1;
            } else {
                summary.already_present += 1;
            }
        }
        Ok(summary)
    }

    pub async fn persist(&mut self, plan: &ContentPersistencePlan) -> Result<PersistenceSummary> {
        let transaction = self.client.transaction().await?;

        for params in &plan.content {
            let values: [&(dyn tokio_postgres::types::ToSql + Sync); 26] = [
                &params.source,
                &params.source_id,
                &params.external_id,
                &params.external_id_type,
                &params.title,
                &params.year,
                &params.content_type,
                &params.parent_source,
                &params.parent_source_id,
                &params.season_number,
                &params.episode_number,
                &params.runtime_seconds,
                &params.release_date,
                &params.first_air_date,
                &params.status,
                &params.summary,
                &params.rating,
                &params.rating_source,
                &params.poster_url,
                &params.fanart_url,
                &params.section_key,
                &params.section_title,
                &params.genres,
                &params.sub_genres,
                &params.metadata_blob,
                &params.provider_metadata,
            ];
            transaction.query_one(CONTENT_UPSERT_SQL, &values).await?;
        }

        for params in &plan.provider_cache {
            let fetched_at = params.fetched_at_epoch as f64;
            let expires_at = params.expires_at_epoch.map(|value| value as f64);
            let http_status = params.http_status.map(i32::from);
            let values: [&(dyn tokio_postgres::types::ToSql + Sync); 9] = [
                &params.content_source,
                &params.content_source_id,
                &params.provider,
                &params.provider_id,
                &params.payload,
                &fetched_at,
                &expires_at,
                &http_status,
                &params.error,
            ];
            transaction
                .query_one(PROVIDER_CACHE_UPSERT_SQL, &values)
                .await?;
        }

        transaction.commit().await?;
        Ok(PersistenceSummary {
            content_rows: plan.content.len(),
            provider_cache_rows: plan.provider_cache.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bootstrap_summary, migration_summary, provider_cache_entry_from_fields,
        BootstrapSummary, ContentPersistencePlan, ContentUpsertParams, MigrationSummary,
        PersistedProviderCacheFields, PostgresContentStore, SETTINGS_V1_DEFAULTS,
        CHARACTER_STATE_SEED_SQL, CONTENT_UPSERT_SQL, GENRE_ACCESS_SEED_SQL,
        MIGRATION_LOOKUP_SQL, MIGRATION_RECORD_SQL, PROVIDER_CACHE_HYDRATE_SQL,
        PROVIDER_CACHE_UPSERT_SQL, SCHEMA_MIGRATIONS_SQL, SETTINGS_SEED_SQL,
        SINGLE_CHARACTER_ID_SQL,
    };
    use crate::enrichment::{
        EnrichmentFailure, MetadataCache, ProviderCacheEntry, ProviderCacheKey,
    };
    use crate::migrations::MIGRATIONS;
    use crate::sync::{
        ContentSource, ContentSyncBatch, ContentSyncRecord, EnrichedSyncOutcome,
        ProviderSyncFailure, StackSyncFailure,
    };
    use serde_json::json;

    #[test]
    fn migration_runner_tracks_version_and_no_op_state() {
        assert!(SCHEMA_MIGRATIONS_SQL.contains("CREATE TABLE IF NOT EXISTS schema_migrations"));
        assert!(MIGRATION_LOOKUP_SQL.contains("SELECT version FROM schema_migrations"));
        assert!(MIGRATION_RECORD_SQL.contains("INSERT INTO schema_migrations"));
        assert_eq!(MIGRATIONS.len(), 6);
        assert_eq!(MIGRATIONS[0].0, "0001_content_provider_cache");
        assert_eq!(MIGRATIONS[1].0, "0002_sync_state");
        assert_eq!(MIGRATIONS[2].0, "0003_accounts_characters");
        assert_eq!(MIGRATIONS[3].0, "0004_genres");
        assert_eq!(MIGRATIONS[4].0, "0005_character_state");
        assert_eq!(MIGRATIONS[5].0, "0006_settings");

        assert_eq!(
            migration_summary(0, 1),
            MigrationSummary {
                applied: 0,
                already_applied: 1,
            }
        );
        assert_eq!(
            migration_summary(1, 0),
            MigrationSummary {
                applied: 1,
                already_applied: 0,
            }
        );
    }

    #[test]
    fn settings_defaults_cover_the_spec_v1_surface() {
        let defaults: std::collections::BTreeMap<&str, &str> = SETTINGS_V1_DEFAULTS
            .iter()
            .copied()
            .collect();
        assert_eq!(defaults.len(), SETTINGS_V1_DEFAULTS.len(), "duplicate setting keys");

        for (key, value) in [
            ("near_end_threshold_pct", "95"),
            ("new_arrival_window_hours", "48"),
            ("poll_interval_seconds", "300"),
            ("provider_cache_ttl_seconds", "86400"),
            ("provider_max_concurrency", "2"),
            ("sub_genre_purchase_xp_threshold", "100"),
            ("perks_unlocked", "[]"),
            ("daily_budget_enabled", "false"),
            ("daily_budget_actions", "3"),
            ("featured_selection_mode", "all_time_ranking"),
            ("fame_enabled", "false"),
        ] {
            assert_eq!(
                defaults.get(key).copied(),
                Some(value),
                "setting {key:?} has the wrong V1 default"
            );
        }

        // genre_list_order mirrors the finalized §5.2 cascade seeded by 0004.
        let genre_list: Vec<String> = serde_json::from_str(defaults["genre_list_order"])
            .expect("genre_list_order must be a JSON array");
        assert_eq!(genre_list.len(), 10);
        assert_eq!(genre_list[0], "Horror");
        assert_eq!(genre_list[9], "Animation");

        // holiday_windows: Halloween (Horror) + winter (Comedy, Drama) starters.
        let windows: Vec<serde_json::Value> =
            serde_json::from_str(defaults["holiday_windows"]).expect("holiday_windows must be JSON");
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0]["name"], "halloween");
        assert_eq!(windows[0]["multiplier"], 1.5);
        assert_eq!(windows[1]["genres"], serde_json::json!(["Comedy", "Drama"]));
    }

    #[test]
    fn bootstrap_statements_resolve_the_single_character_and_seed_idempotently() {
        assert!(SINGLE_CHARACTER_ID_SQL.contains("JOIN accounts ON accounts.id = characters.account_id"));
        assert!(SINGLE_CHARACTER_ID_SQL.contains("ORDER BY characters.id"));
        assert!(SINGLE_CHARACTER_ID_SQL.contains("LIMIT 1"));

        assert!(CHARACTER_STATE_SEED_SQL.contains("INSERT INTO character_state (character_id)"));
        assert!(CHARACTER_STATE_SEED_SQL.contains("ON CONFLICT (character_id) DO NOTHING"));

        assert!(GENRE_ACCESS_SEED_SQL.contains("INSERT INTO genre_access (character_id, genre_id)"));
        assert!(GENRE_ACCESS_SEED_SQL.contains("FROM genres"));
        assert!(GENRE_ACCESS_SEED_SQL.contains("WHERE genres.is_opening"));
        assert!(GENRE_ACCESS_SEED_SQL.contains("ON CONFLICT (character_id, genre_id) DO NOTHING"));

        assert!(SETTINGS_SEED_SQL.contains("INSERT INTO settings (character_id, key, value)"));
        assert!(SETTINGS_SEED_SQL.contains("FROM unnest($2::text[], $3::text[]) AS seed(key, value)"));
        assert!(SETTINGS_SEED_SQL.contains("WHERE NOT EXISTS ("));
        assert!(SETTINGS_SEED_SQL.contains("settings.character_id = $1 AND settings.key = seed.key"));
    }

    #[test]
    fn bootstrap_summary_counts_seed_rows() {
        assert_eq!(
            bootstrap_summary(1, 1, 13),
            BootstrapSummary {
                character_state_seeded: 1,
                genre_access_seeded: 1,
                settings_seeded: 13,
            }
        );
        assert_eq!(
            bootstrap_summary(0, 0, 0),
            BootstrapSummary {
                character_state_seeded: 0,
                genre_access_seeded: 0,
                settings_seeded: 0,
            }
        );
    }

    #[test]
    fn reconstructs_persisted_cache_identity_and_retains_failure_metadata() {
        let entry = provider_cache_entry_from_fields(PersistedProviderCacheFields {
            content_type: "movie".to_owned(),
            external_id_type: Some("tmdb".to_owned()),
            external_id: Some("603".to_owned()),
            title: "The Matrix".to_owned(),
            year: Some(1999),
            provider: "tmdb".to_owned(),
            provider_id: "603".to_owned(),
            payload: serde_json::Value::Null,
            fetched_at_epoch: 100,
            expires_at_epoch: Some(200),
            http_status: Some(503),
            error: Some("tmdb returned HTTP 503".to_owned()),
        })
        .unwrap();

        assert_eq!(entry.key.content_id, "movie:tmdb:603");
        assert_eq!(entry.payload, None);
        assert_eq!(entry.fetched_at, Some(100));
        assert_eq!(entry.expires_at, Some(200));
        assert_eq!(entry.attempted_at, 100);
        assert_eq!(entry.http_status, Some(503));
        assert_eq!(entry.error.as_deref(), Some("tmdb returned HTTP 503"));
    }

    #[test]
    fn reconstructs_title_year_identity_when_external_ids_are_missing() {
        let entry = provider_cache_entry_from_fields(PersistedProviderCacheFields {
            content_type: "series".to_owned(),
            external_id_type: None,
            external_id: None,
            title: "Fixture Series".to_owned(),
            year: Some(2024),
            provider: "omdb".to_owned(),
            provider_id: "title:fixture-series:2024".to_owned(),
            payload: serde_json::json!({"Response": "True"}),
            fetched_at_epoch: 100,
            expires_at_epoch: None,
            http_status: Some(200),
            error: None,
        })
        .unwrap();

        assert_eq!(entry.key.content_id, "series:title:fixture-series:2024");
        assert_eq!(entry.payload, Some(serde_json::json!({"Response": "True"})));
        assert_eq!(entry.expires_at, None);
    }

    #[test]
    fn restores_persisted_entries_without_overwriting_current_poll_data() {
        let key = crate::enrichment::ProviderCacheKey::new("tmdb", "603", "movie:tmdb:603");
        let mut cache = MetadataCache::default();
        cache.restore(ProviderCacheEntry {
            key: key.clone(),
            payload: Some(json!({"revision": "current"})),
            fetched_at: Some(300),
            expires_at: Some(400),
            attempted_at: 300,
            http_status: Some(200),
            error: None,
        });

        let restored = cache.restore_if_absent(ProviderCacheEntry {
            key,
            payload: Some(json!({"revision": "persisted"})),
            fetched_at: Some(100),
            expires_at: Some(200),
            attempted_at: 100,
            http_status: Some(200),
            error: None,
        });

        assert!(!restored);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.entries().next().unwrap().payload, Some(json!({"revision": "current"})));
    }

    #[test]
    fn exposes_joined_cache_hydration_sql() {
        for fragment in [
            "FROM content_provider_cache AS cache",
            "JOIN content ON content.id = cache.content_id",
            "content.external_id_type",
            "EXTRACT(EPOCH FROM cache.fetched_at)",
            "ORDER BY cache.id",
        ] {
            assert!(
                PROVIDER_CACHE_HYDRATE_SQL.contains(fragment),
                "missing {fragment:?}"
            );
        }
    }

    #[test]
    fn validates_database_url_without_opening_a_connection() {
        assert!(PostgresContentStore::validate_database_url(
            "postgresql://rpg@localhost/movie_rpg"
        )
        .is_ok());
        assert!(PostgresContentStore::validate_database_url(" ").is_err());
    }

    #[test]
    fn maps_fixture_records_to_complete_content_upsert_parameters() {
        let plex = crate::stack::parse_plex_library_items(
            include_str!("../fixtures/plex_library.xml"),
        )
        .unwrap();
        let record = ContentSyncRecord::from_plex(&plex[0]).unwrap();
        let params = ContentUpsertParams::from(&record);

        assert_eq!(params.source, "plex");
        assert_eq!(params.source_id, "271");
        assert_eq!(params.title, "Fear Street: Part One - 1994");
        assert_eq!(params.content_type, "movie");
        assert_eq!(params.release_date.as_deref(), Some("2021-07-02"));
        assert_eq!(params.genres, json!(["Horror", "Mystery"]));
        assert_eq!(params.metadata_blob["ratingKey"], json!("271"));
        assert_eq!(params.provider_metadata, json!({}));
    }

    #[test]
    fn creates_stable_upsert_sql_with_conflict_updates_and_returning_rows() {
        for fragment in [
            "INSERT INTO content",
            "ON CONFLICT (source, source_id) DO UPDATE",
            "last_synced_at = now()",
            "last_enriched_at",
            "$13::text::date",
            "RETURNING id, source, source_id",
        ] {
            assert!(CONTENT_UPSERT_SQL.contains(fragment), "missing {fragment:?}");
        }
        for fragment in [
            "INSERT INTO content_provider_cache",
            "SELECT content.id FROM content",
            "ON CONFLICT (content_id, provider, provider_id) DO UPDATE",
            "http_status = EXCLUDED.http_status",
            "RETURNING id, content_id, provider, provider_id",
        ] {
            assert!(
                PROVIDER_CACHE_UPSERT_SQL.contains(fragment),
                "missing {fragment:?}"
            );
        }
    }

    #[test]
    fn keeps_sync_failures_and_binds_provider_cache_to_radarr_owner() {
        let movies: Vec<crate::stack::RadarrMovie> =
            serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        let mut radarr = ContentSyncRecord::from_radarr(&movies[0]).unwrap();
        radarr.external_id = Some("603".to_owned());
        radarr.external_id_type = Some("tmdb".to_owned());

        let mut plex = radarr.clone();
        plex.key = super::super::sync::ContentUpsertKey::new(ContentSource::Plex, "271");
        plex.metadata_blob = json!({"ratingKey": "271"});
        let batch = ContentSyncBatch::from_records([plex, radarr]);
        let identity = batch.groups().next().unwrap().identity.clone();

        let mut cache = MetadataCache::default();
        cache.restore(ProviderCacheEntry {
            key: ProviderCacheKey::new("tmdb", "603", identity.cache_id()),
            payload: Some(json!({"id": 603})),
            fetched_at: Some(100),
            expires_at: Some(4_100),
            attempted_at: 100,
            http_status: Some(200),
            error: None,
        });
        cache.restore(ProviderCacheEntry {
            key: ProviderCacheKey::new("omdb", "tt0133093", identity.cache_id()),
            payload: None,
            fetched_at: None,
            expires_at: Some(200),
            attempted_at: 200,
            http_status: Some(503),
            error: Some("omdb returned HTTP 503".to_owned()),
        });

        let outcome = EnrichedSyncOutcome {
            batch,
            stack_failures: vec![StackSyncFailure {
                source: ContentSource::Plex,
                operation: "library_items/1".to_owned(),
                error: "fixture warning".to_owned(),
            }],
            provider_failures: vec![ProviderSyncFailure {
                identity,
                failure: EnrichmentFailure {
                    provider: "omdb".to_owned(),
                    provider_id: "tt0133093".to_owned(),
                    content_id: "movie:tmdb:603".to_owned(),
                    error: "omdb returned HTTP 503".to_owned(),
                    http_status: Some(503),
                    used_stale: false,
                },
            }],
        };

        let plan = ContentPersistencePlan::from_outcome(&outcome, &cache);
        assert_eq!(plan.content.len(), 2);
        assert_eq!(plan.provider_cache.len(), 2);
        assert!(plan
            .provider_cache
            .iter()
            .all(|entry| entry.content_source == "radarr" && entry.content_source_id == "17"));
        assert_eq!(
            plan.provider_cache
                .iter()
                .find(|entry| entry.provider == "omdb")
                .unwrap()
                .payload,
            json!(null)
        );
        assert_eq!(plan.stack_failures.len(), 1);
        assert_eq!(plan.provider_failures.len(), 1);
        assert!(plan.warnings.is_empty());
    }
}
