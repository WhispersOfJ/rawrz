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
use tokio_postgres::NoTls;

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

    // 0012 starter loadout: the neutral archetype is permanent and seeded
    // alongside the existing character bootstrap, without changing neutral
    // progression history.
    crate::wizard_store::seed_starter_archetype(transaction, character_id).await?;

    Ok(bootstrap_summary(
        character_state_seeded as usize,
        genre_access_seeded as usize,
        settings_seeded as usize,
    ))
}

/// F-22 batched content upsert: one statement per sync instead of one per
/// row. Parameter order matches the old per-row statement exactly. Rows
/// arrive pre-sorted parents-first (see `ContentPersistencePlan::from_outcome`),
/// and parent linking happens in the trailing UPDATE pass, which re-joins
/// the content table on (source, source_id): a parent inserted earlier in
/// the same statement — and one from a previous cycle — is both visible.
pub const CONTENT_UPSERT_SQL: &str = r#"
WITH batch AS (
  SELECT
    r.source, r.source_id, r.external_id, r.external_id_type, r.title,
    r.year, r.content_type, r.parent_source, r.parent_source_id,
    r.season_number, r.episode_number, r.runtime_seconds, r.release_date,
    r.first_air_date, r.status, r.summary, r.rating, r.rating_source,
    r.poster_url, r.fanart_url, r.section_key, r.section_title, r.genres,
    r.sub_genres, r.metadata_blob, r.provider_metadata
  FROM unnest(
    $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::int[],
    $7::text[], $8::text[], $9::text[], $10::int[], $11::int[], $12::int[],
    $13::text[], $14::text[], $15::text[], $16::text[], $17::float8[],
    $18::text[], $19::text[], $20::text[], $21::text[], $22::text[],
    $23::jsonb[], $24::jsonb[], $25::jsonb[], $26::jsonb[]
  ) AS r(
    source, source_id, external_id, external_id_type, title, year,
    content_type, parent_source, parent_source_id, season_number,
    episode_number, runtime_seconds, release_date, first_air_date, status,
    summary, rating, rating_source, poster_url, fanart_url, section_key,
    section_title, genres, sub_genres, metadata_blob, provider_metadata
  )
), upserted AS (
  INSERT INTO content (
    source, source_id, external_id, external_id_type, title, year, content_type,
    parent_id, season_number, episode_number, runtime_seconds, release_date,
    first_air_date, status, summary, rating, rating_source, poster_url, fanart_url,
    section_key, section_title, genres, sub_genres, metadata_blob, provider_metadata,
    last_enriched_at
  )
  SELECT
    b.source, b.source_id, b.external_id, b.external_id_type, b.title, b.year,
    b.content_type,
    NULL, -- parent linking happens in the second pass below
    b.season_number, b.episode_number, b.runtime_seconds,
    b.release_date::text::date, b.first_air_date::text::date, b.status,
    b.summary, b.rating, b.rating_source, b.poster_url, b.fanart_url,
    b.section_key, b.section_title, b.genres, b.sub_genres, b.metadata_blob,
    b.provider_metadata,
    CASE WHEN b.provider_metadata <> '{}'::jsonb THEN now() ELSE NULL END
  FROM batch b
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
)
-- The upsert above guarantees every batch row now has a content row, so the
-- child side re-joins on the natural key. The INSERT completed before this
-- UPDATE pass runs, so parents inserted in this same batch are visible.
UPDATE content c
SET parent_id = parent.id
FROM batch b
JOIN content child ON child.source = b.source AND child.source_id = b.source_id
LEFT JOIN content parent
  ON parent.source = b.parent_source AND parent.source_id = b.parent_source_id
WHERE c.id = child.id
  AND b.parent_source IS NOT NULL
  AND parent.id IS NOT NULL
  AND c.parent_id IS DISTINCT FROM parent.id
"#;

/// F-22 batched cache upsert: one statement per sync, keyed by (source,
/// source_id) lookups against the content table (already persisted earlier
/// in the same transaction).
pub const PROVIDER_CACHE_UPSERT_SQL: &str = r#"
WITH batch AS (
  SELECT
    r.content_source, r.content_source_id, r.provider, r.provider_id,
    r.payload, r.fetched_at, r.expires_at, r.http_status, r.error
  FROM unnest(
    $1::text[], $2::text[], $3::text[], $4::text[], $5::jsonb[],
    $6::float8[], $7::float8[], $8::int[], $9::text[]
  ) WITH ORDINALITY AS r(
    content_source, content_source_id, provider, provider_id, payload,
    fetched_at, expires_at, http_status, error
  )
), resolved AS (
  SELECT
    b.*, content.id AS content_id
  FROM batch b
  JOIN content ON content.source = b.content_source
              AND content.source_id = b.content_source_id
)
INSERT INTO content_provider_cache (
  content_id, provider, provider_id, payload, fetched_at, expires_at,
  http_status, error
)
SELECT
  content_id, provider, provider_id, payload, to_timestamp(fetched_at),
  CASE WHEN expires_at IS NULL THEN NULL
       ELSE to_timestamp(expires_at) END,
  http_status, error
FROM resolved
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
WHERE cache.expires_at IS NULL OR cache.expires_at > now()
ORDER BY cache.id;
"#;

/// Cache retention (F-16): expired rows are deleted only after a grace
/// period, so the stale-fallback path (used_stale) can still read a payload
/// after its TTL lapses but the table does not grow without bound.
pub const PROVIDER_CACHE_RETENTION_SQL: &str = r#"
DELETE FROM content_provider_cache
WHERE expires_at IS NOT NULL
  AND expires_at < now() - make_interval(secs => $1::double precision)
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

pub const CHARACTER_EXISTS_SQL: &str = "SELECT EXISTS (SELECT 1 FROM characters)";

// V1 has exactly one character per account (§6.4.1); the exists-check above
// guards the insert so repeat set-PIN calls never duplicate characters.
pub const CHARACTER_INSERT_SQL: &str = r#"
INSERT INTO characters (account_id, active_archetype_id)
SELECT accounts.id, archetypes.id
FROM accounts
CROSS JOIN wizard_archetypes AS archetypes
WHERE archetypes.slug = 'lantern_scholar'
ORDER BY accounts.id
LIMIT 1
RETURNING id
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
    pool: deadpool_postgres::Pool,
}

/// One checked-out pooled connection. Dereferences to `tokio_postgres::Client`
/// so reads use it directly; `transaction()` borrows it mutably for writes.
/// The pool re-establishes connections after a Postgres restart (F-17): a
/// dead client fails its health check on recycle and the next checkout gets
/// a fresh one.
pub(crate) struct StoreConnection {
    object: deadpool_postgres::Object,
}

impl StoreConnection {
    /// `deadpool_postgres::Transaction` derefs to the tokio-postgres one, so
    /// every call site uses it identically; it also rolls back on drop, so
    /// an error return never leaves a half-applied transaction open.
    pub async fn transaction(&mut self) -> Result<deadpool_postgres::Transaction<'_>> {
        Ok(self.object.transaction().await?)
    }
}

impl std::ops::Deref for StoreConnection {
    type Target = tokio_postgres::Client;

    fn deref(&self) -> &tokio_postgres::Client {
        &self.object
    }
}

impl PostgresContentStore {
    /// Checks out one pooled connection for the duration of an await.
    pub(crate) async fn connection(&self) -> Result<StoreConnection> {
        self.pool
            .get()
            .await
            .map(|object| StoreConnection { object })
            .map_err(|error| crate::ProbeError::Pool(error.to_string()))
    }

    /// Convenience read handle for tests that want a one-shot pool checkout.
    #[allow(dead_code)]
    pub(crate) fn query_client(&self) -> &deadpool_postgres::Pool {
        &self.pool
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BootstrapSummary {
    pub character_state_seeded: usize,
    pub genre_access_seeded: usize,
    pub settings_seeded: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AccountPinOutcome {
    pub account_created: bool,
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
        let manager = deadpool_postgres::Manager::from_config(
            database_url
                .parse::<tokio_postgres::Config>()
                .map_err(|error| crate::ProbeError::Pool(error.to_string()))?,
            NoTls,
            deadpool_postgres::ManagerConfig {
                recycling_method: deadpool_postgres::RecyclingMethod::Verified,
            },
        );
        let pool = deadpool_postgres::Pool::builder(manager)
            .max_size(8)
            .build()
            .map_err(|error| crate::ProbeError::Pool(error.to_string()))?;
        Ok(Self { pool })
    }

    /// Explicit rollback helper (F-38): early returns inside open
    /// transactions roll back here instead of relying on drop semantics.
    /// Takes the deadpool transaction (all store transactions come from
    /// `StoreConnection::transaction`), which derefs to the tokio-postgres
    /// one and rolls the underlying transaction back.
    async fn rollback(
        transaction: deadpool_postgres::Transaction<'_>,
    ) {
        let _ = transaction.rollback().await;
    }

    pub async fn migrate(&self) -> Result<MigrationSummary> {
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;
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
    pub async fn bootstrap_single_character(&self) -> Result<BootstrapSummary> {
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;

        let Some(character_id) = transaction
            .query_opt(SINGLE_CHARACTER_ID_SQL, &[])
            .await?
            .map(|row| row.get::<_, i64>(0))
        else {
            Self::rollback(transaction).await;
            return Ok(bootstrap_summary(0, 0, 0));
        };

        let summary = seed_rows_for(&transaction, character_id).await?;
        transaction.commit().await?;
        Ok(summary)
    }

    /// Set-PIN flow (§6.4.1, first run only): validates the PIN, hashes it
    /// with Argon2id, inserts the single account, and — since a fresh
    /// account has no character yet — creates the default investigator plus
    /// the full bootstrap seed, all in one transaction. Returns
    /// `account_created: false` when an account already exists (V1 has no
    /// PIN change flow; bootstrap is then still run, idempotently).
    pub async fn set_account_pin(&self, pin: &str) -> Result<(AccountPinOutcome, BootstrapSummary)> {
        crate::auth::validate_pin(pin)?;
        // The Argon2id cost is paid on a blocking thread (F-20) and before
        // any store work, so an unauthenticated caller cannot hold pooled
        // connections or hash on the async runtime.
        let hashed = {
            let pin = pin.to_owned();
            tokio::task::spawn_blocking(move || crate::auth::hash_pin(&pin))
                .await
                .map_err(|error| {
                    crate::ProbeError::PinHashing(format!("hash task panicked: {error}"))
                })?
        }?;

        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;

        let existing: bool = transaction
            .query_one(ACCOUNT_EXISTS_SQL, &[])
            .await?
            .get(0);
        let account_created = !existing;
        if account_created {
            transaction
                .query_one(ACCOUNT_INSERT_SQL, &[&hashed.phc_string, &hashed.salt_b64])
                .await?;
        }

        // The default investigator: one character per account (§6.4.1).
        // Created only if the account has none yet (V1: set-PIN is the only
        // account-creation path, so this runs exactly once).
        let character_exists: bool = transaction
            .query_one(CHARACTER_EXISTS_SQL, &[])
            .await?
            .get(0);
        if !character_exists {
            transaction
                .query_one(CHARACTER_INSERT_SQL, &[])
                .await?;
        }
        let character_id: i64 = transaction
            .query_one(SINGLE_CHARACTER_ID_SQL, &[])
            .await?
            .get(0);

        let summary = seed_rows_for(&transaction, character_id).await?;
        transaction.commit().await?;
        Ok((AccountPinOutcome { account_created }, summary))
    }

    /// Whether any account exists (drives `GET /auth/status` gate state).
    pub async fn account_exists(&self) -> Result<bool> {
        let connection = self.connection().await?;
        let row = connection.query_one(ACCOUNT_EXISTS_SQL, &[]).await?;
        Ok(row.get(0))
    }

    /// Verify flow (§6.4.1): loads the single account's PHC `pin_hash` and
    /// re-derives the PIN. A missing account is **locked** (`None`); a wrong
    /// PIN is `Rejected`, not an error; a malformed stored hash is an
    /// operational error.
    pub async fn verify_account_pin(&self, pin: &str) -> Result<Option<crate::auth::PinVerifyOutcome>> {
        let connection = self.connection().await?;
        let Some(stored) = connection
            .query_opt(SINGLE_ACCOUNT_PIN_SQL, &[])
            .await?
            .map(|row| row.get::<_, String>(0))
        else {
            return Ok(None);
        };
        // The verify pass runs on a blocking thread (F-20) — it costs the
        // same Argon2id work as hashing and must not stall the runtime.
        let pin = pin.to_owned();
        tokio::task::spawn_blocking(move || crate::auth::verify_pin(&pin, &stored).map(Some))
            .await
            .map_err(|error| {
                crate::ProbeError::PinHashing(format!("verify task panicked: {error}"))
            })?
    }

    pub async fn hydrate_cache(
        &self,
        cache: &mut MetadataCache,
    ) -> Result<CacheHydrationSummary> {
        // F-16: hydrate fresh rows only; expired entries are re-fetched from
        // the provider rather than rehydrated, and retention prunes them.
        let connection = self.connection().await?;
        let rows = connection.query(PROVIDER_CACHE_HYDRATE_SQL, &[]).await?;
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

    /// Deletes expired cache rows past a grace period (F-16 retention).
    /// `grace_seconds` keeps recent payloads available for stale-fallback.
    pub async fn prune_provider_cache(&self, grace_seconds: u64) -> Result<u64> {
        let connection = self.connection().await?;
        let grace = grace_seconds as f64;
        let deleted = connection
            .execute(PROVIDER_CACHE_RETENTION_SQL, &[&grace])
            .await?;
        Ok(deleted)
    }

    pub async fn persist(&self, plan: &ContentPersistencePlan) -> Result<PersistenceSummary> {
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;

        // F-22: one batched statement per table per sync instead of one
        // round-trip per row. Content arrives parents-first, so the second
        // parent-linking pass in CONTENT_UPSERT_SQL resolves show parents.
        if !plan.content.is_empty() {
            let sources: Vec<&str> = plan.content.iter().map(|p| p.source.as_str()).collect();
            let source_ids: Vec<&str> = plan.content.iter().map(|p| p.source_id.as_str()).collect();
            let external_ids: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.external_id.as_deref()).collect();
            let external_id_types: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.external_id_type.as_deref()).collect();
            let titles: Vec<&str> = plan.content.iter().map(|p| p.title.as_str()).collect();
            let years: Vec<Option<i32>> = plan.content.iter().map(|p| p.year).collect();
            let content_types: Vec<&str> =
                plan.content.iter().map(|p| p.content_type.as_str()).collect();
            let parent_sources: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.parent_source.as_deref()).collect();
            let parent_source_ids: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.parent_source_id.as_deref()).collect();
            let season_numbers: Vec<Option<i32>> =
                plan.content.iter().map(|p| p.season_number).collect();
            let episode_numbers: Vec<Option<i32>> =
                plan.content.iter().map(|p| p.episode_number).collect();
            let runtimes: Vec<Option<i32>> =
                plan.content.iter().map(|p| p.runtime_seconds).collect();
            let release_dates: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.release_date.as_deref()).collect();
            let first_air_dates: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.first_air_date.as_deref()).collect();
            let statuses: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.status.as_deref()).collect();
            let summaries: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.summary.as_deref()).collect();
            let ratings: Vec<Option<f64>> = plan.content.iter().map(|p| p.rating).collect();
            let rating_sources: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.rating_source.as_deref()).collect();
            let poster_urls: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.poster_url.as_deref()).collect();
            let fanart_urls: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.fanart_url.as_deref()).collect();
            let section_keys: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.section_key.as_deref()).collect();
            let section_titles: Vec<Option<&str>> =
                plan.content.iter().map(|p| p.section_title.as_deref()).collect();
            let genres: Vec<Value> = plan.content.iter().map(|p| p.genres.clone()).collect();
            let sub_genres: Vec<Value> = plan.content.iter().map(|p| p.sub_genres.clone()).collect();
            let metadata_blobs: Vec<Value> =
                plan.content.iter().map(|p| p.metadata_blob.clone()).collect();
            let provider_metadata: Vec<Value> =
                plan.content.iter().map(|p| p.provider_metadata.clone()).collect();

            transaction
                .execute(
                    CONTENT_UPSERT_SQL,
                    &[
                        &sources,
                        &source_ids,
                        &external_ids,
                        &external_id_types,
                        &titles,
                        &years,
                        &content_types,
                        &parent_sources,
                        &parent_source_ids,
                        &season_numbers,
                        &episode_numbers,
                        &runtimes,
                        &release_dates,
                        &first_air_dates,
                        &statuses,
                        &summaries,
                        &ratings,
                        &rating_sources,
                        &poster_urls,
                        &fanart_urls,
                        &section_keys,
                        &section_titles,
                        &genres,
                        &sub_genres,
                        &metadata_blobs,
                        &provider_metadata,
                    ],
                )
                .await?;
        }

        if !plan.provider_cache.is_empty() {
            let content_sources: Vec<&str> =
                plan.provider_cache.iter().map(|p| p.content_source.as_str()).collect();
            let content_source_ids: Vec<&str> =
                plan.provider_cache.iter().map(|p| p.content_source_id.as_str()).collect();
            let providers: Vec<&str> =
                plan.provider_cache.iter().map(|p| p.provider.as_str()).collect();
            let provider_ids: Vec<&str> =
                plan.provider_cache.iter().map(|p| p.provider_id.as_str()).collect();
            let payloads: Vec<Value> =
                plan.provider_cache.iter().map(|p| p.payload.clone()).collect();
            let fetched_ats: Vec<f64> =
                plan.provider_cache.iter().map(|p| p.fetched_at_epoch as f64).collect();
            let expires_ats: Vec<Option<f64>> =
                plan.provider_cache.iter().map(|p| p.expires_at_epoch.map(|v| v as f64)).collect();
            let http_statuses: Vec<Option<i32>> =
                plan.provider_cache.iter().map(|p| p.http_status.map(i32::from)).collect();
            let errors: Vec<Option<&str>> =
                plan.provider_cache.iter().map(|p| p.error.as_deref()).collect();

            transaction
                .execute(
                    PROVIDER_CACHE_UPSERT_SQL,
                    &[
                        &content_sources,
                        &content_source_ids,
                        &providers,
                        &provider_ids,
                        &payloads,
                        &fetched_ats,
                        &expires_ats,
                        &http_statuses,
                        &errors,
                    ],
                )
                .await?;
        }

        transaction.commit().await?;
        Ok(PersistenceSummary {
            content_rows: plan.content.len(),
            provider_cache_rows: plan.provider_cache.len(),
        })
    }

    /// Character-sheet payload for the gated API (§7.4): the singleton
    /// character's identity, progression state, and accessed genre names.
    pub async fn character_overview(&self) -> Result<Option<CharacterOverview>> {
        let Some(row) = self
            .connection()
            .await?
            .query_opt(
                "SELECT c.name, cs.xp, cs.level, cs.total_watches, cs.episode_watches,
                        cs.movie_watches, cs.current_streak_days, cs.best_streak_days,
                        cs.genres_accessed, COALESCE(array_agg(g.name ORDER BY g.list_order)
                            FILTER (WHERE g.name IS NOT NULL), '{}')
                 FROM characters c
                 LEFT JOIN character_state cs ON cs.character_id = c.id
                 LEFT JOIN genre_access ga ON ga.character_id = c.id
                 LEFT JOIN genres g ON g.id = ga.genre_id
                 GROUP BY c.id, c.name, cs.xp, cs.level, cs.total_watches,
                          cs.episode_watches, cs.movie_watches, cs.current_streak_days,
                          cs.best_streak_days, cs.genres_accessed",
                &[],
            )
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(CharacterOverview {
            name: row.get(0),
            xp: row.get::<_, Option<i64>>(1).unwrap_or(0),
            level: row.get::<_, Option<i32>>(2).unwrap_or(1),
            total_watches: row.get::<_, Option<i32>>(3).unwrap_or(0),
            episode_watches: row.get::<_, Option<i32>>(4).unwrap_or(0),
            movie_watches: row.get::<_, Option<i32>>(5).unwrap_or(0),
            current_streak_days: row.get::<_, Option<i32>>(6).unwrap_or(0),
            best_streak_days: row.get::<_, Option<i32>>(7).unwrap_or(0),
            genres_accessed: row.get::<_, Option<i32>>(8).unwrap_or(1),
            genres: row.get(9),
        }))
    }

    /// One evaluation pass (§6.4.8 contract): load definitions, build the
    /// snapshot, run the pure engine, write only new unlocks (idempotent).
    /// Returns `None` when no character exists (pre-set-PIN).
    pub async fn evaluate_achievements(&self) -> Result<Option<EvaluationSummary>> {
        // Unlock facts are materialized before the read pass so the API and
        // the next tick see newly satisfied archetypes without a stale
        // one-cycle delay.
        self.refresh_archetype_unlocks().await?;
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(None);
        };

        let definitions = self
            .connection()
            .await?
            .query(ACHIEVEMENT_DEFINITIONS_SQL, &[])
            .await?
            .into_iter()
            .map(|row| AchievementDefinition {
                slug: row.get(0),
                kind: row.get(1),
                target_value: row.get::<_, Option<i64>>(2),
            // tokio-postgres maps jsonb → serde_json::Value directly.
                metadata: row.get(3),
            })
            .collect::<Vec<_>>();

        let unlocked_before = self
            .connection()
            .await?
            .query(UNLOCKED_SLUGS_SQL, &[&account_id])
            .await?
            .into_iter()
            .map(|row| row.get::<_, String>(0))
            .collect::<std::collections::HashSet<_>>();

        let snapshot = match self.progress_snapshot(account_id).await? {
            Some(snapshot) => snapshot,
            None => return Ok(None),
        };

        let mut summary = EvaluationSummary {
            evaluated: 0,
            unlocked: Vec::new(),
            not_evaluable: 0,
        };
        for definition in &definitions {
            let evaluation = crate::achievements::evaluate(
                &definition.kind,
                definition.target_value,
                &definition.metadata,
                &snapshot,
            );
            match evaluation {
                crate::achievements::Evaluation::Unlock { progress } => {
                    summary.evaluated += 1;
                    if unlocked_before.contains(&definition.slug) {
                        continue;
                    }
                    self.connection()
                        .await?
                        .execute(ACHIEVEMENT_UNLOCK_SQL, &[&account_id, &definition.slug, &progress])
                        .await?;
                    summary.unlocked.push(definition.slug.clone());
                }
                crate::achievements::Evaluation::InProgress { .. } => {
                    summary.evaluated += 1;
                }
                crate::achievements::Evaluation::NotEvaluable => {
                    summary.not_evaluable += 1;
                }
            }
        }
        Ok(Some(summary))
    }

    /// The badge wall: every definition with unlock state and live progress.
    /// Returns `None` when no character exists (pre-set-PIN).
    pub async fn badge_wall(&self) -> Result<Option<Vec<BadgeEntry>>> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(None);
        };
        let snapshot = self.progress_snapshot(account_id).await?;
        let Some(snapshot) = snapshot else {
            return Ok(None);
        };
        let mut entries = Vec::new();
        for row in self.connection().await?.query(BADGE_WALL_SQL, &[&account_id]).await? {
            let unlocked_at: Option<String> = row.get(6);
            let stored_progress: Option<i64> = row.get(7);
            let target: Option<i64> = row.get(8);
            let (unlocked, progress) = if unlocked_at.is_some() {
                (true, stored_progress.unwrap_or(0))
            } else {
                // Live progress for locked rows: evaluate this one definition.
                let evaluation = crate::achievements::evaluate(
                    &row.get::<_, String>(5),
                    target,
                    &row.get::<_, serde_json::Value>(9),
                    &snapshot,
                );
                match evaluation {
                    crate::achievements::Evaluation::Unlock { progress } => (true, progress),
                    crate::achievements::Evaluation::InProgress { progress, .. } => (false, progress),
                    crate::achievements::Evaluation::NotEvaluable => (false, 0),
                }
            };
            let (slug, name, description, category): (String, String, String, String) =
                (row.get(0), row.get(1), row.get(2), row.get(3));
            let visible: bool = row.get(4);
            // F-32: hidden badges are discovery content. Until unlocked,
            // only slug + category leak through; name/description/progress
            // are masked so the wall does not spoil what exists.
            let (name, description) = if !visible && !unlocked {
                ("Hidden badge".to_owned(), "Keep investigating…".to_owned())
            } else {
                (name, description)
            };
            let (progress, target) = if !visible && !unlocked { (0, None) } else { (progress, target) };
            entries.push(BadgeEntry {
                slug,
                name,
                description,
                category,
                visible,
                kind: row.get(5),
                unlocked,
                unlocked_at,
                progress,
                target,
            });
        }
        Ok(Some(entries))
    }

    /// The snapshot builder shared by both flows.
    async fn progress_snapshot(
        &self,
        account_id: i64,
    ) -> Result<Option<crate::achievements::ProgressSnapshot>> {
        let Some(row) = self
            .connection()
            .await?
            .query_opt(PROGRESS_SNAPSHOT_SQL, &[&account_id])
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(crate::achievements::ProgressSnapshot {
            episode_watches: row.get(0),
            movie_watches: row.get(1),
            current_streak_days: row.get(2),
            level: row.get(3),
            distinct_genres: row.get(4),
            horror_watches: row.get(5),
            distinct_holiday_windows: row.get(6),
            distinct_new_arrival_titles: row.get(7),
            purchased_sub_genres: row.get(8),
            completed_featured_cases: row.get(9),
        }))
    }

    /// Creates a new order cycle for each accessible genre that has none and
    /// has not exhausted its two-cycle V1 supply (§6.4.11 ORDER_GENRES), then
    /// advances reveals and completes/grants for all active orders. Safe to
    /// call on every tick and from the refresh endpoint.
    pub async fn refresh_watch_orders(&self) -> Result<OrderRefreshSummary> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(OrderRefreshSummary {
                orders_created: Vec::new(),
                skips_granted: 0,
                skipped_genres: Vec::new(),
            });
        };
        let mut summary = OrderRefreshSummary {
            orders_created: Vec::new(),
            skips_granted: 0,
            skipped_genres: Vec::new(),
        };

        for row in self.connection().await?.query(ORDER_GENRES_SQL, &[&account_id]).await? {
            let genre_id: i64 = row.get(0);
            let genre: String = row.get(1);
            let candidates: Vec<i64> = self
                .connection()
                .await?
                .query(
                    ORDER_CANDIDATES_SQL,
                    &[&account_id, &genre, &genre_id, &WATCH_ORDER_SIZE],
                )
                .await?
                .into_iter()
                .map(|row| row.get(0))
                .collect();
            if candidates.len() < WATCH_ORDER_SIZE as usize {
                // F-35: surface the skip instead of silently dropping the
                // genre — candidates are unwatched, unranked movies of that
                // genre, so a shortfall means the library is exhausted for
                // the remaining cycles.
                summary.skipped_genres.push(SkippedGenre {
                    genre,
                    reason: format!(
                        "only {} of {} needed movies are available unwatched and unranked",
                        candidates.len(), WATCH_ORDER_SIZE
                    ),
                });
                continue;
            }

            let mut connection = self.connection().await?;
            let transaction = connection.transaction().await?;
            let created = transaction
                .query_one(ORDER_INSERT_SQL, &[&account_id, &genre_id])
                .await?;
            let order_id: i64 = created.get(0);
            let cycle: i32 = created.get(1);
            let positions: Vec<i32> = (1..=WATCH_ORDER_SIZE as i32).collect();
            transaction
                .execute(
                    ORDER_ITEMS_INSERT_SQL,
                    &[&order_id, &positions, &candidates],
                )
                .await?;
            transaction.commit().await?;
            summary.orders_created.push(OrderCreated {
                order_id,
                genre,
                cycle_number: cycle,
            });
        }

        summary.skips_granted = self.advance_watch_orders(account_id).await?;
        Ok(summary)
    }

    /// Advances reveal stamps, completes finished orders, and grants their
    /// skip rewards. Returns how many skips were granted.
    async fn advance_watch_orders(&self, account_id: i64) -> Result<usize> {
        let mut connection = self.connection().await?;
        connection
            .execute(REVEAL_STAMP_SQL, &[&account_id])
            .await?;
        let mut granted = 0;
        for row in connection.query(OPEN_ORDERS_SQL, &[&account_id]).await? {
            let order_id: i64 = row.get(0);
            // Completion and its reward are one transaction. Without this,
            // a process crash after ORDER_COMPLETE_SQL commits but before
            // SKIP_GRANT_SQL runs permanently loses the player's skip because
            // completed orders are no longer in the active-order scan.
            let transaction = connection.transaction().await?;
            let completed_now = transaction
                .query_opt(ORDER_COMPLETE_SQL, &[&order_id])
                .await?
                .is_some();
            let completed_without_grant = if completed_now {
                true
            } else {
                transaction
                    .query_opt(ORDER_COMPLETED_UNGRANTED_SQL, &[&order_id])
                    .await?
                    .is_some()
            };
            let reward_inserted = completed_without_grant
                && transaction
                    .query_opt(SKIP_GRANT_SQL, &[&order_id])
                    .await?
                    .is_some();
            transaction.commit().await?;
            if reward_inserted {
                granted += 1;
            }
        }
        Ok(granted)
    }

    /// The player-facing order view. Locked items carry position + locked
    /// only — title, year, and content id are None (§5.7 mystery).
    pub async fn order_view(&self) -> Result<Option<Vec<OrderView>>> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(None);
        };
        let mut orders: Vec<OrderView> = Vec::new();
        let mut current: Option<OrderView> = None;
        for row in self
            .connection()
            .await?
            .query(ORDER_VIEW_SQL, &[&account_id])
            .await?
        {
            let position: i32 = row.get(4);
            let previous_resolved: bool = row.get(10);
            let locked = position != 1 && !previous_resolved;
            let item = OrderItemView {
                position,
                locked,
                content_id: if locked { None } else { Some(row.get(5)) },
                title: if locked { None } else { row.get(6) },
                year: if locked { None } else { row.get(7) },
                watched: row.get(9),
                skipped: row.get(8),
            };
            let (id, genre, cycle, status): (i64, String, i32, String) =
                (row.get(0), row.get(1), row.get(2), row.get(3));
            if current.as_ref().map(|view| view.id) != Some(id) {
                if let Some(view) = current.take() {
                    orders.push(view);
                }
                current = Some(OrderView {
                    id,
                    genre,
                    cycle_number: cycle,
                    status,
                    items: Vec::new(),
                });
            }
            current.as_mut().unwrap().items.push(item);
        }
        if let Some(view) = current.take() {
            orders.push(view);
        }
        Ok(Some(orders))
    }

    /// Spends a skip on the current item of one open order.
    pub async fn skip_order_item(&self, order_id: i64) -> Result<SkipOutcome> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(SkipOutcome::NothingToSkip);
        };
        let Some(current) = self
            .connection()
            .await?
            .query_opt(ORDER_CURRENT_ITEM_SQL, &[&account_id, &order_id])
            .await?
        else {
            return Ok(SkipOutcome::NothingToSkip);
        };
        let item_id: i64 = current.get(0);
        let position: i32 = current.get(1);

        // One transaction: the skip stamp and the ledger spend succeed
        // together or not at all (a dropped transaction rolls back, so a
        // missing balance can never leave a stamped-but-unpaid skip).
        let mut connection = self.connection().await?;
        let transaction = connection.transaction().await?;
        // The finale guard is the position predicate inside SKIP_ITEM_SQL.
        if transaction
            .query_opt(SKIP_ITEM_SQL, &[&account_id, &item_id])
            .await?
            .is_none()
        {
            return Ok(SkipOutcome::FinaleNotSkippable);
        }
        if transaction
            .query_opt(SKIP_SPEND_SQL, &[&account_id, &item_id])
            .await?
            .is_none()
        {
            return Ok(SkipOutcome::NoSkipsAvailable);
        }
        transaction.commit().await?;

        // The skip may have completed the order — advance the tick's later
        // phases for it (§9.1: reveals, then generation, then achievement
        // evaluation). F-31: order generation runs here too, so finishing a
        // cycle by skip yields the next mystery immediately instead of
        // waiting for the next tick/refresh.
        self.advance_watch_orders(account_id).await?;
        self.refresh_watch_orders().await?;
        self.evaluate_achievements().await?;
        Ok(SkipOutcome::Skipped { position })
    }

    /// Phase 1 (§9.1): detect completed watches from Plex watch state and
    /// award them — `watches` rows (award-once), then XP/counters/streak/
    /// level in the same pass. `today` is injected for testability. Returns
    /// how many new watches were awarded.
    pub async fn award_plex_watches(
        &self,
        states: &[crate::awards::PlexWatchState],
        today: chrono::NaiveDate,
    ) -> Result<usize> {
        use std::collections::HashMap;

        let Some(account_id) = self.single_account_id().await? else {
            return Ok(0);
        };

        // The catalog view of the Plex library: ratingKey → content id/type.
        let mut keys = HashMap::new();
        let mut types: HashMap<i64, String> = HashMap::new();
        let mut horror: HashMap<i64, bool> = HashMap::new();
        for row in self
            .connection()
            .await?
            .query(PLEX_CATALOG_SQL, &[])
            .await?
        {
            let id: i64 = row.get(1);
            keys.insert(row.get::<_, String>(0), id);
            types.insert(id, row.get(2));
            horror.insert(id, row.get(3));
        }
        let active_archetype = self.active_archetype_slug(account_id).await?;

        let awards = crate::awards::detect_watches(states, &keys, &types);
        let mut awarded = 0;
        for award in awards {
            // F-6 ledger semantics: the ledger keeps the neutral §5.1 value
            // in `normal_xp` and the active-loadout adjustment in
            // `xp_awarded`, so per-watch audit rows reconcile against the
            // pure math instead of hiding the archetype effect.
            let neutral_xp = award.xp;
            let adjusted_xp = crate::wizard::normal_xp_for_watch(
                &active_archetype,
                &award.watch.content_type,
                horror.get(&award.watch.content_id).copied().unwrap_or(false),
                neutral_xp,
            );

            // Award-once is the insert's guard: only a row that actually
            // landed costs state deltas. The insert and state update share a
            // transaction, so a partial award cannot mutate history or XP.
            let mut connection = self.connection().await?;
            let transaction = connection.transaction().await?;
            // Serialize awards for one character/content pair before the
            // NOT EXISTS guard. The 0013 UNIQUE(character_id, content_id)
            // constraint (F-7) is the database-level backstop for any second
            // writer; its violation below is treated as "already awarded".
            transaction
                .query_one(
                    WATCH_AWARD_LOCK_SQL,
                    &[&account_id.to_string(), &award.watch.content_id.to_string()],
                )
                .await?;

            let state = transaction
                .query_one(CHARACTER_AWARD_STATE_SQL, &[&account_id])
                .await?;
            let xp_so_far: i64 = state.get(0);
            let current_streak: i64 = state.get(1);
            let last_watch_date: Option<chrono::NaiveDate> = state.get(2);

            let (streak_delta, milestone) =
                match crate::awards::streak_advance(last_watch_date, today) {
                    crate::awards::StreakAdvance::Neutral => (0, 0),
                    crate::awards::StreakAdvance::Restart => {
                        (1 - current_streak, crate::awards::streak_milestone_bonus(1))
                    }
                    crate::awards::StreakAdvance::Continue { .. } => {
                        let grown = current_streak + 1;
                        (1, crate::awards::streak_milestone_bonus(grown))
                    }
                };
            let new_streak = current_streak + streak_delta;
            let total_xp = xp_so_far + adjusted_xp + milestone;
            let new_level = crate::awards::level_for_xp(total_xp);
            let date_delta = match streak_delta {
                0 => None,
                _ => Some(today),
            };
            // F-6: a paying streak milestone is recorded on the watch row so
            // the ledger shows exactly what the watch earned, bonuses
            // included (§5.1.1 audit).
            let bonuses_json = if milestone > 0 {
                serde_json::json!([
                    {"kind": "streak_milestone", "amount": milestone}
                ])
            } else {
                serde_json::json!([])
            };

            let new_streak_i32 = i32::try_from(new_streak).map_err(|_| {
                crate::ProbeError::OutOfRange(format!("streak out of int4 range: {new_streak}"))
            })?;
            let new_level_i32 = i32::try_from(new_level).map_err(|_| {
                crate::ProbeError::OutOfRange(format!("level out of int4 range: {new_level}"))
            })?;

            let insert = transaction
                .query_opt(
                    WATCH_INSERT_SQL,
                    &[
                        &account_id,
                        &award.watch.content_type,
                        &award.watch.pct_viewed,
                        &adjusted_xp,
                        &neutral_xp,
                        &bonuses_json,
                        &award.watch.rating_key,
                    ],
                )
                .await;
            match insert {
                Ok(Some(_watch_row)) => {}
                Ok(None) => {
                    // NOT EXISTS guard: this watch was already awarded.
                    transaction.commit().await?;
                    continue;
                }
                Err(error)
                    if error.code()
                        == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) =>
                {
                    // 0013 constraint backstop: a concurrent writer awarded
                    // first. Treat identically to the guard above.
                    transaction.commit().await?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            }

            transaction
                .execute(
                    AWARD_STATE_SQL,
                    &[&account_id, &(adjusted_xp + milestone), &award.watch.content_type, &new_streak_i32, &date_delta, &new_level_i32],
                )
                .await?;
            transaction.commit().await?;
            awarded += 1;
        }
        Ok(awarded)
    }

    /// The single account id, or `None` pre-set-PIN.
    pub(crate) async fn single_account_id(&self) -> Result<Option<i64>> {
        let row = self
            .connection()
            .await?
            .query_opt("SELECT id FROM accounts ORDER BY id LIMIT 1", &[])
            .await?;
        Ok(row.map(|row| row.get(0)))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CharacterOverview {
    pub name: String,
    pub xp: i64,
    pub level: i32,
    pub total_watches: i32,
    pub episode_watches: i32,
    pub movie_watches: i32,
    pub current_streak_days: i32,
    pub best_streak_days: i32,
    pub genres_accessed: i32,
    pub genres: Vec<String>,
}

// ---- Achievement evaluation (§6.4.8, evaluation contract finalized
// 2026-09-08; engine in achievements.rs, writes only at unlock) ----

/// One row per loadable achievement definition, as the store reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct AchievementDefinition {
    pub slug: String,
    pub kind: String,
    pub target_value: Option<i64>,
    pub metadata: serde_json::Value,
}

/// A badge-wall row (visible + hidden, unlocked + locked with progress).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BadgeEntry {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub visible: bool,
    pub kind: String,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
    pub progress: i64,
    pub target: Option<i64>,
}

/// The result of one evaluation pass over all definitions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvaluationSummary {
    pub evaluated: usize,
    pub unlocked: Vec<String>,
    pub not_evaluable: usize,
}

/// All definitions, ordered for deterministic evaluation.
pub const ACHIEVEMENT_DEFINITIONS_SQL: &str = r#"
SELECT slug, kind, target_value, metadata
FROM achievements
ORDER BY id
"#;

/// Existing unlocks for the single character (idempotency guard).
pub const UNLOCKED_SLUGS_SQL: &str = r#"
SELECT a.slug
FROM character_achievements ca
JOIN achievements a ON a.id = ca.achievement_id
JOIN characters c ON c.id = ca.character_id
WHERE c.account_id = $1
"#;

/// Unlock write: idempotent, records the value at unlock, server timestamp.
pub const ACHIEVEMENT_UNLOCK_SQL: &str = r#"
INSERT INTO character_achievements (character_id, achievement_id, progress)
SELECT c.id, a.id, $3
FROM characters c
CROSS JOIN achievements a
WHERE c.account_id = $1 AND a.slug = $2
ON CONFLICT DO NOTHING
"#;

/// Snapshot aggregates: character_state totals plus watch/genre/purchase
/// facts for the metrics the engine can evaluate in V1. Horror watches and
/// genre distinctness come from content metadata; holiday windows and new
/// arrivals from the watches audit flags (jsonb keys).
pub const PROGRESS_SNAPSHOT_SQL: &str = r#"
SELECT
  cs.episode_watches::bigint,
  cs.movie_watches::bigint,
  cs.current_streak_days::bigint,
  cs.level::bigint,
  (SELECT count(DISTINCT g.name)
     FROM watches w
     JOIN content c ON c.id = w.content_id,
     jsonb_array_elements_text(c.genres) AS g(name)
    WHERE w.character_id = cs.character_id)::bigint AS distinct_genres,
  (SELECT count(*)
     FROM watches w
     JOIN content c ON c.id = w.content_id
    WHERE w.character_id = cs.character_id
      AND c.genres ? 'Horror')::bigint AS horror_watches,
  (SELECT count(DISTINCT wb.holiday_window)
     FROM watches w
     CROSS JOIN LATERAL jsonb_object_keys(w.holiday_bonus) AS wb(holiday_window)
    WHERE w.character_id = cs.character_id
      AND w.holiday_bonus IS NOT NULL)::bigint AS distinct_holiday_windows,
  (SELECT count(DISTINCT w.content_id)
     FROM watches w
    WHERE w.character_id = cs.character_id
      AND w.new_arrival)::bigint AS distinct_new_arrival_titles,
  (SELECT count(*) FROM sub_genre_xp sgx
    WHERE sgx.character_id = cs.character_id AND sgx.purchased)::bigint
    AS purchased_sub_genres,
  (SELECT count(*)
     FROM cases ca
    WHERE ca.character_id = cs.character_id
      AND ca.case_type = 'featured'
      AND ca.status = 'completed')::bigint AS completed_featured_cases
FROM character_state cs
JOIN characters c ON c.id = cs.character_id
WHERE c.account_id = $1
"#;

/// Badge-wall read: every definition left-joined to the character's unlock.
pub const BADGE_WALL_SQL: &str = r#"
SELECT a.slug, a.name, a.description, a.category, a.visible, a.kind,
       ca.unlocked_at::text AS unlocked_at, ca.progress::bigint AS progress,
       a.target_value, a.metadata
FROM achievements a
LEFT JOIN (
  character_achievements ca
  JOIN characters c ON c.id = ca.character_id
) ON ca.achievement_id = a.id AND c.account_id = $1
ORDER BY a.visible DESC, a.category, a.id
"#;

// ---- Mystery watch orders (§5.7, §6.4.13). Item resolution is DERIVED
// from the watches ledger (the ≥95% award) or a spent skip — never stored. ----

/// V1 order shape: five movies per cycle (§6.4.13 generation).
pub const WATCH_ORDER_SIZE: i64 = 5;

/// Genres the character can access that have no active order and fewer than
/// two total cycles (the current cycle plus the one skips are earned for).
pub const ORDER_GENRES_SQL: &str = r#"
SELECT g.id, g.name
FROM genre_access ga
JOIN genres g ON g.id = ga.genre_id
WHERE ga.character_id = $1
  AND NOT EXISTS (
    SELECT 1 FROM watch_orders wo
    WHERE wo.character_id = ga.character_id AND wo.genre_id = g.id AND wo.status = 'active'
  )
  AND (
    SELECT count(*) FROM watch_orders wo
    WHERE wo.character_id = ga.character_id AND wo.genre_id = g.id
  ) < 2
ORDER BY g.list_order
"#;

/// Deterministic candidate pick (§6.4.13 generation): genre movies the
/// character has no awarded watch for and that no prior cycle of this genre
/// already used, ranked by provider score with id as the tiebreaker.
pub const ORDER_CANDIDATES_SQL: &str = r#"
SELECT c.id
FROM content c
WHERE c.content_type = 'movie'
  AND c.genres ? $2
  AND NOT EXISTS (
    SELECT 1 FROM watches w
    WHERE w.character_id = $1 AND w.content_id = c.id
  )
  AND NOT EXISTS (
    SELECT 1 FROM watch_order_items i
    JOIN watch_orders wo ON wo.id = i.order_id
    WHERE wo.character_id = $1 AND wo.genre_id = $3 AND i.content_id = c.id
  )
ORDER BY c.rating DESC NULLS LAST, c.id
LIMIT $4
"#;

/// Creates the next cycle for one genre; cycle_number derives from the rows
/// that exist (count + 1), not from client input.
pub const ORDER_INSERT_SQL: &str = r#"
INSERT INTO watch_orders (character_id, genre_id, cycle_number)
SELECT c.id, $2, (
  SELECT count(*) + 1 FROM watch_orders wo
  WHERE wo.character_id = c.id AND wo.genre_id = $2
)
FROM characters c
WHERE c.account_id = $1
RETURNING id, cycle_number
"#;

/// Item rows in one statement; item 1 carries the creation-time reveal.
pub const ORDER_ITEMS_INSERT_SQL: &str = r#"
INSERT INTO watch_order_items (order_id, position, content_id)
SELECT $1, position, content_id
FROM unnest($2::int[], $3::bigint[]) AS items(position, content_id)
"#;

/// The load-bearing reveal query: an item is visible when it is resolved
/// (watched per §6.4.5 or skipped) or when its predecessor is resolved — the
/// mystery is what this query does NOT return for locked items.
pub const ORDER_VIEW_SQL: &str = r#"
SELECT
  wo.id, g.name AS genre, wo.cycle_number, wo.status,
  i.position, i.content_id, c.title, c.year::text AS year,
  i.skipped_at IS NOT NULL AS skipped,
  EXISTS (
    SELECT 1 FROM watches w
    WHERE w.character_id = wo.character_id AND w.content_id = i.content_id
  ) AS watched,
  EXISTS (
    SELECT 1
    FROM watch_order_items p
    LEFT JOIN watches pw
      ON pw.character_id = wo.character_id AND pw.content_id = p.content_id
    WHERE p.order_id = wo.id AND p.position = i.position - 1
      AND (pw.id IS NOT NULL OR p.skipped_at IS NOT NULL)
  ) AS previous_resolved
FROM watch_orders wo
JOIN genres g ON g.id = wo.genre_id
JOIN characters ch ON ch.id = wo.character_id
JOIN watch_order_items i ON i.order_id = wo.id
JOIN content c ON c.id = i.content_id
WHERE ch.account_id = $1
  -- F-30: completed cycles stay on the board as history; active ones sort
  -- first so the current mystery leads per genre. (V1 caps each genre at
  -- two cycles, so the completed set stays small.)
  AND wo.status IN ('active', 'completed')
ORDER BY g.list_order,
  (wo.status = 'active') DESC,
  wo.completed_at DESC NULLS FIRST,
  wo.cycle_number, i.position
"#;

/// The current (first unresolved) item of one active order.
pub const ORDER_CURRENT_ITEM_SQL: &str = r#"
SELECT i.id, i.position
FROM watch_order_items i
JOIN watch_orders wo ON wo.id = i.order_id
JOIN characters ch ON ch.id = wo.character_id
LEFT JOIN watches w
  ON w.character_id = wo.character_id AND w.content_id = i.content_id
WHERE ch.account_id = $1 AND wo.id = $2 AND wo.status = 'active'
  AND i.skipped_at IS NULL AND w.id IS NULL
ORDER BY i.position
LIMIT 1
"#;

/// Completes an order only when every item is resolved (watched or skipped).
pub const ORDER_COMPLETE_SQL: &str = r#"
UPDATE watch_orders wo
SET status = 'completed', completed_at = now()
WHERE wo.id = $1 AND wo.status = 'active'
  AND NOT EXISTS (
    SELECT 1
    FROM watch_order_items i
    LEFT JOIN watches w
      ON w.character_id = wo.character_id AND w.content_id = i.content_id
    WHERE i.order_id = wo.id AND i.skipped_at IS NULL AND w.id IS NULL
  )
RETURNING wo.id
"#;

/// Grants the completion skip exactly once per order (V1: 1 per completion).
pub const SKIP_GRANT_SQL: &str = r#"
INSERT INTO skip_grants (character_id, source_order_id)
SELECT wo.character_id, wo.id
FROM watch_orders wo
WHERE wo.id = $1
  AND NOT EXISTS (
    SELECT 1 FROM skip_grants sg WHERE sg.source_order_id = wo.id
  )
RETURNING id
"#;

/// Spends one unspent skip; the ledger row is stamped, never deleted.
pub const SKIP_SPEND_SQL: &str = r#"
UPDATE skip_grants sg
SET spent_at = now(), spent_item_id = $2
FROM characters ch
WHERE sg.character_id = ch.id AND ch.account_id = $1
  AND sg.spent_at IS NULL
  AND NOT EXISTS (
    SELECT 1 FROM skip_grants other WHERE other.spent_item_id = $2
  )
RETURNING sg.id
"#;

/// Stamps a skip on the current item. The final item can never be skipped
/// (§5.7: the finale must be watched) — enforced by position, not by trust.
pub const SKIP_ITEM_SQL: &str = r#"
UPDATE watch_order_items i
SET skipped_at = now()
FROM watch_orders wo
JOIN characters ch ON ch.id = wo.character_id
WHERE i.order_id = wo.id AND ch.account_id = $1
  AND i.id = $2 AND wo.status = 'active'
  AND i.skipped_at IS NULL
  AND i.position < (
    SELECT max(position) FROM watch_order_items fin WHERE fin.order_id = i.order_id
  )
RETURNING i.id
"#;

/// Stamps the audit reveal time on newly revealed items (predecessor now
/// resolved). Visibility itself is always derived, never read from here.
pub const REVEAL_STAMP_SQL: &str = r#"
UPDATE watch_order_items cur
SET revealed_at = now()
FROM watch_orders wo
JOIN characters ch ON ch.id = wo.character_id
JOIN watch_order_items prev ON prev.order_id = wo.id
LEFT JOIN watches pw
  ON pw.character_id = wo.character_id AND pw.content_id = prev.content_id
WHERE ch.account_id = $1 AND wo.status = 'active'
  AND cur.order_id = wo.id
  AND prev.position = cur.position - 1
  AND (pw.id IS NOT NULL OR prev.skipped_at IS NOT NULL)
"#;

/// The player's open orders, newest first.
pub const OPEN_ORDERS_SQL: &str = r#"
SELECT wo.id, g.name
FROM watch_orders wo
JOIN genres g ON g.id = wo.genre_id
JOIN characters ch ON ch.id = wo.character_id
WHERE ch.account_id = $1
  AND (
    wo.status = 'active'
    OR (wo.status = 'completed' AND NOT EXISTS (
      SELECT 1 FROM skip_grants sg WHERE sg.source_order_id = wo.id
    ))
  )
ORDER BY wo.created_at DESC
"#;

/// Recovery guard for an order completed before its skip grant committed.
/// The completion and grant are normally one transaction; this also repairs
/// any legacy interrupted transition without creating a second grant.
pub const ORDER_COMPLETED_UNGRANTED_SQL: &str = r#"
SELECT id
FROM watch_orders wo
WHERE wo.id = $1
  AND wo.status = 'completed'
  AND NOT EXISTS (
    SELECT 1 FROM skip_grants sg WHERE sg.source_order_id = wo.id
  )
FOR UPDATE
"#;

// ---- Phase 1: watch award (§9.1 detection semantics finalized 2026-09-08;
// pure math in awards.rs, this is the application layer) ----

/// Known catalog entries keyed by their Plex ratingKey (content.source =
/// 'plex', source_id = ratingKey) with their type.
pub const PLEX_CATALOG_SQL: &str = r#"
SELECT c.source_id, c.id, c.content_type,
       (c.genres @> '["Horror"]'::jsonb) AS is_horror
FROM content c
WHERE c.source = 'plex'
"#;

/// Awarded-watch insert: the §5.7 award-once rule as a NOT EXISTS guard, so
/// a repeat detection (or a concurrent tick) cannot double-award. The 0013
/// UNIQUE(character_id, content_id) constraint is the database-level
/// backstop (F-7); `normal_xp` stays the neutral value and `xp_awarded`
/// carries the archetype adjustment (F-6 ledger semantics).
pub const WATCH_AWARD_LOCK_SQL: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text, 0))";

pub const WATCH_INSERT_SQL: &str = r#"
INSERT INTO watches (
  character_id, content_id, content_type, pct_viewed,
  xp_awarded, normal_xp, bonuses, via_plex
)
SELECT $1, c.id, $2, $3::int::numeric, $4, $5, $6::jsonb, true
FROM content c
WHERE c.source = 'plex' AND c.source_id = $7
  AND NOT EXISTS (
    SELECT 1 FROM watches w
    WHERE w.character_id = $1 AND w.content_id = c.id
  )
RETURNING id
"#;

/// The single character's award-relevant state (xp for level math, streak
/// fields for the §5.1 advance).
pub const CHARACTER_AWARD_STATE_SQL: &str = r#"
SELECT cs.xp::bigint, cs.current_streak_days::bigint, cs.streak_last_watch_date
FROM character_state cs
JOIN characters c ON c.id = cs.character_id
WHERE c.account_id = $1
"#;

/// Applies one award's state deltas atomically: XP (normal + milestone),
/// watch counters, streak fields, and level re-evaluated from §5.2 by the
/// caller-computed level. Same-day detections pass streak delta 0 + no date
/// change (§5.1 neutrality).
pub const AWARD_STATE_SQL: &str = r#"
UPDATE character_state cs
SET xp = cs.xp + $2,
    total_watches = cs.total_watches + 1,
    episode_watches = cs.episode_watches + CASE WHEN $3 = 'episode' THEN 1 ELSE 0 END,
    movie_watches = cs.movie_watches + CASE WHEN $3 = 'movie' THEN 1 ELSE 0 END,
    current_streak_days = $4,
    best_streak_days = GREATEST(cs.best_streak_days, $4),
    streak_last_watch_date = COALESCE($5, cs.streak_last_watch_date),
    level = $6
FROM characters c
WHERE cs.character_id = c.id AND c.account_id = $1
"#;

/// One item in an order view. Locked items serialize as position + locked
/// only (§5.7 mystery): the identity fields are Options, skipped entirely
/// (not null) when None so the JSON carries no hint of the hidden title.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderItemView {
    pub position: i32,
    pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    pub watched: bool,
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderView {
    pub id: i64,
    pub genre: String,
    pub cycle_number: i32,
    /// F-30: 'active' | 'completed' — completed cycles render as history.
    pub status: String,
    pub items: Vec<OrderItemView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderCreated {
    pub order_id: i64,
    pub genre: String,
    pub cycle_number: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderRefreshSummary {
    pub orders_created: Vec<OrderCreated>,
    pub skips_granted: usize,
    /// F-35: genres that had no open order but could not generate one, with
    /// the reason — the UI can explain "no new mystery available" instead of
    /// a silent no-op.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped_genres: Vec<SkippedGenre>,
}

/// One genre that could not generate its next cycle (F-35).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkippedGenre {
    pub genre: String,
    pub reason: String,
}

/// Skip action outcomes (§6.4.1-style explicitness).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipOutcome {
    Skipped { position: i32 },
    NoSkipsAvailable,
    FinaleNotSkippable,
    NothingToSkip,
}

#[cfg(test)]
mod tests {
    use super::{
        bootstrap_summary, migration_summary, provider_cache_entry_from_fields,
        BootstrapSummary, ContentPersistencePlan, ContentUpsertParams, MigrationSummary,
        PersistedProviderCacheFields, PostgresContentStore, SETTINGS_V1_DEFAULTS,
        ACCOUNT_EXISTS_SQL, ACCOUNT_INSERT_SQL, CHARACTER_EXISTS_SQL, CHARACTER_INSERT_SQL,
        CHARACTER_STATE_SEED_SQL, CONTENT_UPSERT_SQL, GENRE_ACCESS_SEED_SQL,
        MIGRATION_LOOKUP_SQL, MIGRATION_RECORD_SQL, PROVIDER_CACHE_HYDRATE_SQL,
        PROVIDER_CACHE_UPSERT_SQL, SCHEMA_MIGRATIONS_SQL, SETTINGS_SEED_SQL,
        SINGLE_ACCOUNT_PIN_SQL, SINGLE_CHARACTER_ID_SQL,
        AWARD_STATE_SQL, CHARACTER_AWARD_STATE_SQL, ORDER_CANDIDATES_SQL,
        ORDER_COMPLETE_SQL, ORDER_COMPLETED_UNGRANTED_SQL, ORDER_INSERT_SQL,
        ORDER_VIEW_SQL, PLEX_CATALOG_SQL, WATCH_AWARD_LOCK_SQL,
        REVEAL_STAMP_SQL, SKIP_GRANT_SQL, SKIP_ITEM_SQL, SKIP_SPEND_SQL,
        WATCH_INSERT_SQL,
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
        assert_eq!(MIGRATIONS.len(), 13);
        assert_eq!(MIGRATIONS[0].0, "0001_content_provider_cache");
        assert_eq!(MIGRATIONS[1].0, "0002_sync_state");
        assert_eq!(MIGRATIONS[2].0, "0003_accounts_characters");
        assert_eq!(MIGRATIONS[3].0, "0004_genres");
        assert_eq!(MIGRATIONS[4].0, "0005_character_state");
        assert_eq!(MIGRATIONS[5].0, "0006_settings");
        assert_eq!(MIGRATIONS[6].0, "0007_watches");
        assert_eq!(MIGRATIONS[7].0, "0008_cases");
        assert_eq!(MIGRATIONS[8].0, "0009_featured_cases");
        assert_eq!(MIGRATIONS[9].0, "0010_achievements");
        assert_eq!(MIGRATIONS[10].0, "0011_watch_orders");
        assert_eq!(MIGRATIONS[11].0, "0012_wizard_archetypes");
        assert_eq!(MIGRATIONS[12].0, "0013_watches_unique_award");

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
        assert!(crate::wizard_store::ARCHETYPE_BOOTSTRAP_EVENT_SQL.contains(
            "event_type, source, source_event_key"
        ));
        assert!(crate::wizard_store::ARCHETYPE_BOOTSTRAP_EVENT_SQL
            .contains("'bootstrap:lantern_scholar'"));
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
    fn watch_order_statements_derive_resolution_and_guard_the_finale() {
        // Generation is deterministic and excludes pre-watched content.
        assert!(ORDER_CANDIDATES_SQL.contains("c.content_type = 'movie'"));
        assert!(ORDER_CANDIDATES_SQL.contains("NOT EXISTS ("));
        assert!(ORDER_CANDIDATES_SQL.contains("ORDER BY c.rating DESC NULLS LAST, c.id"));
        assert!(ORDER_CANDIDATES_SQL.contains("LIMIT $4"));
        // Cycle numbers derive from rows, never from client input.
        assert!(ORDER_INSERT_SQL.contains("count(*) + 1"));
        assert!(ORDER_INSERT_SQL.contains("RETURNING id, cycle_number"));
        // The reveal derivation: previous item watched (§6.4.5 ledger) or skipped.
        assert!(ORDER_VIEW_SQL.contains("previous_resolved"));
        assert!(ORDER_VIEW_SQL.contains("p.skipped_at IS NOT NULL"));
        // F-30: completed cycles remain on the board as history.
        assert!(ORDER_VIEW_SQL.contains("wo.status IN ('active', 'completed')"));
        // Completion requires every item resolved; the grant fires once.
        assert!(ORDER_COMPLETE_SQL.contains("i.skipped_at IS NULL AND w.id IS NULL"));
        assert!(ORDER_COMPLETED_UNGRANTED_SQL.contains("status = 'completed'"));
        assert!(ORDER_COMPLETED_UNGRANTED_SQL.contains("source_order_id = wo.id"));
        assert!(SKIP_GRANT_SQL.contains("NOT EXISTS ("));
        assert!(SKIP_SPEND_SQL.contains("sg.spent_at IS NULL"));
        assert!(SKIP_SPEND_SQL.contains("spent_item_id = $2"));
        // The finale guard: position strictly before the last item.
        assert!(SKIP_ITEM_SQL.contains("i.position < ("));
        // Spend stamps the item; nothing is ever deleted from the ledger.
        assert!(SKIP_SPEND_SQL.contains("SET spent_at = now(), spent_item_id = $2"));
        // The audit stamp is bookkeeping only — the view derives visibility.
        assert!(REVEAL_STAMP_SQL.contains("prev.skipped_at IS NOT NULL"));
        assert!(!ORDER_VIEW_SQL.contains("revealed_at"));
    }

    #[test]
    fn award_statements_enforce_award_once_and_atomic_state_deltas() {
        // Detection matches Plex rows by ratingKey.
        assert!(PLEX_CATALOG_SQL.contains("WHERE c.source = 'plex'"));
        assert!(PLEX_CATALOG_SQL.contains("is_horror"));
        assert!(crate::wizard_store::ACTIVE_ARCHETYPE_SQL.contains("active_archetype_id"));
        assert!(WATCH_AWARD_LOCK_SQL.contains("pg_advisory_xact_lock"));
        assert!(crate::wizard_store::ARCHETYPE_REFRESH_LOCK_SQL.contains("pg_advisory_xact_lock"));
        // The award-once rule: NOT EXISTS guard on the single watch row.
        assert!(WATCH_INSERT_SQL.contains("NOT EXISTS ("));
        assert!(WATCH_INSERT_SQL.contains("WHERE w.character_id = $1 AND w.content_id = c.id"));
        assert!(WATCH_INSERT_SQL.contains("via_plex"));
        // State deltas carry streak fields, best-streak protection, and a
        // caller-computed level (§5.2 math stays in the pure module).
        assert!(AWARD_STATE_SQL.contains("best_streak_days = GREATEST(cs.best_streak_days, $4)"));
        assert!(AWARD_STATE_SQL.contains("level = $6"));
        assert!(AWARD_STATE_SQL.contains("streak_last_watch_date = COALESCE($5, cs.streak_last_watch_date)"));
        // The state read gives the caller everything the pure math needs.
        assert!(CHARACTER_AWARD_STATE_SQL.contains("cs.xp::bigint"));
        assert!(CHARACTER_AWARD_STATE_SQL.contains("cs.streak_last_watch_date"));
    }

    #[test]
    fn account_pin_statements_resolve_the_single_account() {
        assert_eq!(ACCOUNT_EXISTS_SQL, "SELECT EXISTS (SELECT 1 FROM accounts)");
        assert!(ACCOUNT_INSERT_SQL.contains("INSERT INTO accounts (pin_hash, pin_salts)"));
        assert!(ACCOUNT_INSERT_SQL.contains("VALUES ($1, $2)"));
        assert!(ACCOUNT_INSERT_SQL.contains("RETURNING id"));

        assert!(SINGLE_ACCOUNT_PIN_SQL.contains("SELECT pin_hash FROM accounts"));
        assert!(SINGLE_ACCOUNT_PIN_SQL.contains("ORDER BY id"));
        assert!(SINGLE_ACCOUNT_PIN_SQL.contains("LIMIT 1"));

        assert!(CHARACTER_EXISTS_SQL.contains("SELECT EXISTS (SELECT 1 FROM characters)"));
        assert!(CHARACTER_INSERT_SQL.contains("INSERT INTO characters (account_id, active_archetype_id)"));
        assert!(CHARACTER_INSERT_SQL.contains("FROM accounts"));
        assert!(CHARACTER_INSERT_SQL.contains("RETURNING id"));
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
            // F-22 batch shape: unnest arrays + parent-linking second pass.
            "FROM unnest(",
            "UPDATE content c",
            "SET parent_id = parent.id",
            // The final pass re-joins on the natural key and links parents.
            "JOIN content child ON child.source = b.source",
        ] {
            assert!(CONTENT_UPSERT_SQL.contains(fragment), "missing {fragment:?}");
        }
        for fragment in [
            "INSERT INTO content_provider_cache",
            "JOIN content ON content.source = b.content_source",
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
