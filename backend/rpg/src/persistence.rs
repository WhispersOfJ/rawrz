use crate::enrichment::{MetadataCache, ProviderCacheEntry};
use crate::sync::{
    ContentSyncGroup, ContentSyncRecord, EnrichedSyncOutcome, ProviderSyncFailure,
    StackSyncFailure,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

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
  $10, $11, $12, $13::date, $14::date, $15, $16, $17, $18, $19, $20,
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

#[cfg(test)]
mod tests {
    use super::{
        ContentPersistencePlan, ContentUpsertParams, CONTENT_UPSERT_SQL,
        PROVIDER_CACHE_UPSERT_SQL,
    };
    use crate::enrichment::{
        EnrichmentFailure, MetadataCache, ProviderCacheEntry, ProviderCacheKey,
    };
    use crate::sync::{
        ContentSource, ContentSyncBatch, ContentSyncRecord, EnrichedSyncOutcome,
        ProviderSyncFailure, StackSyncFailure,
    };
    use serde_json::json;

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
