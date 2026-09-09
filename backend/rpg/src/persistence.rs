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

pub const CHARACTER_EXISTS_SQL: &str = "SELECT EXISTS (SELECT 1 FROM characters)";

// V1 has exactly one character per account (§6.4.1); the exists-check above
// guards the insert so repeat set-PIN calls never duplicate characters.
pub const CHARACTER_INSERT_SQL: &str = r#"
INSERT INTO characters (account_id)
SELECT accounts.id FROM accounts
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
    client: Client,
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

    /// Set-PIN flow (§6.4.1, first run only): validates the PIN, hashes it
    /// with Argon2id, inserts the single account, and — since a fresh
    /// account has no character yet — creates the default investigator plus
    /// the full bootstrap seed, all in one transaction. Returns
    /// `account_created: false` when an account already exists (V1 has no
    /// PIN change flow; bootstrap is then still run, idempotently).
    pub async fn set_account_pin(&mut self, pin: &str) -> Result<(AccountPinOutcome, BootstrapSummary)> {
        crate::auth::validate_pin(pin)?;
        let hashed = crate::auth::hash_pin(pin)?;

        let transaction = self.client.transaction().await?;

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
        let row = self.client.query_one(ACCOUNT_EXISTS_SQL, &[]).await?;
        Ok(row.get(0))
    }

    /// Verify flow (§6.4.1): loads the single account's PHC `pin_hash` and
    /// re-derives the PIN. A missing account is **locked** (`None`); a wrong
    /// PIN is `Rejected`, not an error; a malformed stored hash is an
    /// operational error.
    pub async fn verify_account_pin(&self, pin: &str) -> Result<Option<crate::auth::PinVerifyOutcome>> {
        let Some(stored) = self
            .client
            .query_opt(SINGLE_ACCOUNT_PIN_SQL, &[])
            .await?
            .map(|row| row.get::<_, String>(0))
        else {
            return Ok(None);
        };
        crate::auth::verify_pin(pin, &stored).map(Some)
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

    /// Character-sheet payload for the gated API (§7.4): the singleton
    /// character's identity, progression state, and accessed genre names.
    pub async fn character_overview(&self) -> Result<Option<CharacterOverview>> {
        let Some(row) = self
            .client
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
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(None);
        };

        let definitions = self
            .client
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
            .client
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
                    self.client
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
        for row in self.client.query(BADGE_WALL_SQL, &[&account_id]).await? {
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
            entries.push(BadgeEntry {
                slug: row.get(0),
                name: row.get(1),
                description: row.get(2),
                category: row.get(3),
                visible: row.get(4),
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
            .client
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
    pub async fn refresh_watch_orders(&mut self) -> Result<OrderRefreshSummary> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(OrderRefreshSummary {
                orders_created: Vec::new(),
                skips_granted: 0,
            });
        };
        let mut summary = OrderRefreshSummary {
            orders_created: Vec::new(),
            skips_granted: 0,
        };

        for row in self.client.query(ORDER_GENRES_SQL, &[&account_id]).await? {
            let genre_id: i64 = row.get(0);
            let genre: String = row.get(1);
            let candidates: Vec<i64> = self
                .client
                .query(
                    ORDER_CANDIDATES_SQL,
                    &[&account_id, &genre, &genre_id, &WATCH_ORDER_SIZE],
                )
                .await?
                .into_iter()
                .map(|row| row.get(0))
                .collect();
            if candidates.len() < WATCH_ORDER_SIZE as usize {
                continue;
            }

            let transaction = self.client.transaction().await?;
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
        self.client
            .execute(REVEAL_STAMP_SQL, &[&account_id])
            .await?;
        let mut granted = 0;
        for row in self.client.query(OPEN_ORDERS_SQL, &[&account_id]).await? {
            let order_id: i64 = row.get(0);
            if self
                .client
                .query_opt(ORDER_COMPLETE_SQL, &[&order_id])
                .await?
                .is_some()
                && self
                    .client
                    .query_opt(SKIP_GRANT_SQL, &[&order_id])
                    .await?
                    .is_some()
            {
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
        for row in self.client.query(ORDER_VIEW_SQL, &[&account_id]).await? {
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
            let (id, genre, cycle): (i64, String, i32) = (row.get(0), row.get(1), row.get(2));
            if current.as_ref().map(|view| view.id) != Some(id) {
                if let Some(view) = current.take() {
                    orders.push(view);
                }
                current = Some(OrderView {
                    id,
                    genre,
                    cycle_number: cycle,
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
    pub async fn skip_order_item(&mut self, order_id: i64) -> Result<SkipOutcome> {
        let Some(account_id) = self.single_account_id().await? else {
            return Ok(SkipOutcome::NothingToSkip);
        };
        let Some(current) = self
            .client
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
        let transaction = self.client.transaction().await?;
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
        // phases for it (§9.1: reveals, then achievement evaluation).
        self.advance_watch_orders(account_id).await?;
        self.evaluate_achievements().await?;
        Ok(SkipOutcome::Skipped { position })
    }

    /// The single account id, or `None` pre-set-PIN.
    async fn single_account_id(&self) -> Result<Option<i64>> {
        let row = self
            .client
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
WHERE ch.account_id = $1 AND wo.status = 'active'
ORDER BY g.list_order, wo.cycle_number, i.position
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
WHERE ch.account_id = $1 AND wo.status = 'active'
ORDER BY wo.created_at DESC
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
        ORDER_CANDIDATES_SQL, ORDER_COMPLETE_SQL, ORDER_INSERT_SQL, ORDER_VIEW_SQL,
        REVEAL_STAMP_SQL, SKIP_GRANT_SQL, SKIP_ITEM_SQL, SKIP_SPEND_SQL,
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
        assert_eq!(MIGRATIONS.len(), 11);
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
        assert!(ORDER_VIEW_SQL.contains("WHERE ch.account_id = $1 AND wo.status = 'active'"));
        // Completion requires every item resolved; the grant fires once.
        assert!(ORDER_COMPLETE_SQL.contains("i.skipped_at IS NULL AND w.id IS NULL"));
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
    fn account_pin_statements_resolve_the_single_account() {
        assert_eq!(ACCOUNT_EXISTS_SQL, "SELECT EXISTS (SELECT 1 FROM accounts)");
        assert!(ACCOUNT_INSERT_SQL.contains("INSERT INTO accounts (pin_hash, pin_salts)"));
        assert!(ACCOUNT_INSERT_SQL.contains("VALUES ($1, $2)"));
        assert!(ACCOUNT_INSERT_SQL.contains("RETURNING id"));

        assert!(SINGLE_ACCOUNT_PIN_SQL.contains("SELECT pin_hash FROM accounts"));
        assert!(SINGLE_ACCOUNT_PIN_SQL.contains("ORDER BY id"));
        assert!(SINGLE_ACCOUNT_PIN_SQL.contains("LIMIT 1"));

        assert!(CHARACTER_EXISTS_SQL.contains("SELECT EXISTS (SELECT 1 FROM characters)"));
        assert!(CHARACTER_INSERT_SQL.contains("INSERT INTO characters (account_id)"));
        assert!(CHARACTER_INSERT_SQL.contains("SELECT accounts.id FROM accounts"));
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
