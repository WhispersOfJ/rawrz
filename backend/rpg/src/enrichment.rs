use crate::providers::{FanartClient, OmdbClient, TmdbClient, TvdbClient};
use crate::{ProbeError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Movie,
    Series,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentRequest {
    pub content_id: String,
    pub kind: ContentKind,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub tmdb_id: Option<i64>,
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderCacheKey {
    pub provider: String,
    pub provider_id: String,
    pub content_id: String,
}

impl ProviderCacheKey {
    pub fn new(
        provider: impl Into<String>,
        provider_id: impl Into<String>,
        content_id: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            provider_id: provider_id.into(),
            content_id: content_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCacheEntry {
    pub key: ProviderCacheKey,
    pub payload: Option<Value>,
    pub fetched_at: Option<u64>,
    pub expires_at: Option<u64>,
    pub attempted_at: u64,
    pub http_status: Option<u16>,
    pub error: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct MetadataCache {
    entries: BTreeMap<ProviderCacheKey, ProviderCacheEntry>,
}

impl MetadataCache {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entry(&self, key: &ProviderCacheKey) -> Option<&ProviderCacheEntry> {
        self.entries.get(key)
    }

    pub fn entries(&self) -> impl Iterator<Item = &ProviderCacheEntry> {
        self.entries.values()
    }

    /// Restore a persisted cache entry when rebuilding the in-memory cache at startup.
    pub fn restore(&mut self, entry: ProviderCacheEntry) {
        self.entries.insert(entry.key.clone(), entry);
    }

    /// Restore a persisted entry only when the current poll has not already populated it.
    pub fn restore_if_absent(&mut self, entry: ProviderCacheEntry) -> bool {
        if self.entries.contains_key(&entry.key) {
            return false;
        }
        self.entries.insert(entry.key.clone(), entry);
        true
    }

    fn fresh(&self, key: &ProviderCacheKey, now: u64) -> Option<ProviderCacheEntry> {
        let entry = self.entries.get(key)?;
        let expires_at = entry.expires_at?;
        if entry.payload.is_some() && entry.error.is_none() && expires_at > now {
            Some(entry.clone())
        } else {
            None
        }
    }

    fn stale(&self, key: &ProviderCacheKey) -> Option<ProviderCacheEntry> {
        self.entries
            .get(key)
            .filter(|entry| entry.payload.is_some())
            .cloned()
    }

    fn store_success(&mut self, key: ProviderCacheKey, payload: Value, now: u64, ttl_seconds: u64) {
        self.entries.insert(
            key.clone(),
            ProviderCacheEntry {
                key,
                payload: Some(payload),
                fetched_at: Some(now),
                expires_at: Some(now.saturating_add(ttl_seconds)),
                attempted_at: now,
                http_status: Some(200),
                error: None,
            },
        );
    }

    fn store_failure(
        &mut self,
        key: ProviderCacheKey,
        now: u64,
        http_status: Option<u16>,
        error: String,
    ) {
        let previous = self.entries.remove(&key);
        self.entries.insert(
            key.clone(),
            ProviderCacheEntry {
                key,
                payload: previous.as_ref().and_then(|entry| entry.payload.clone()),
                fetched_at: previous.as_ref().and_then(|entry| entry.fetched_at),
                expires_at: previous
                    .as_ref()
                    .and_then(|entry| entry.expires_at)
                    .or(Some(now)),
                attempted_at: now,
                http_status,
                error: Some(error),
            },
        );
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnrichmentPayload {
    pub provider: String,
    pub provider_id: String,
    pub content_id: String,
    pub payload: Value,
    pub fetched_at: u64,
    pub expires_at: u64,
    pub from_cache: bool,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnrichmentFailure {
    pub provider: String,
    pub provider_id: String,
    pub content_id: String,
    pub error: String,
    pub http_status: Option<u16>,
    pub used_stale: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct EnrichmentBatch {
    pub payloads: BTreeMap<String, EnrichmentPayload>,
    pub failures: Vec<EnrichmentFailure>,
}

impl EnrichmentBatch {
    fn record(&mut self, outcome: ProviderOutcome) {
        if let Some(payload) = outcome.payload {
            self.payloads.insert(payload.provider.clone(), payload);
        }
        if let Some(failure) = outcome.failure {
            self.failures.push(failure);
        }
    }
}

struct ProviderOutcome {
    payload: Option<EnrichmentPayload>,
    failure: Option<EnrichmentFailure>,
}

pub struct EnrichmentCoordinator<'a> {
    tmdb: &'a TmdbClient,
    tvdb: &'a TvdbClient,
    omdb: &'a OmdbClient,
    fanart: &'a FanartClient,
    cache: &'a mut MetadataCache,
    ttl_seconds: u64,
}

impl<'a> EnrichmentCoordinator<'a> {
    pub fn new(
        tmdb: &'a TmdbClient,
        tvdb: &'a TvdbClient,
        omdb: &'a OmdbClient,
        fanart: &'a FanartClient,
        cache: &'a mut MetadataCache,
        ttl_seconds: u64,
    ) -> Self {
        Self {
            tmdb,
            tvdb,
            omdb,
            fanart,
            cache,
            ttl_seconds,
        }
    }

    pub async fn enrich(&mut self, request: &EnrichmentRequest, now: u64) -> EnrichmentBatch {
        let mut batch = EnrichmentBatch::default();

        if let Some(tmdb_id) = request.tmdb_id {
            let key = ProviderCacheKey::new("tmdb", tmdb_id.to_string(), request.content_id.clone());
            let client = self.tmdb;
            let kind = request.kind;
            let outcome = load_json(
                self.cache,
                key,
                now,
                self.ttl_seconds,
                move || async move {
                    let details = match kind {
                        ContentKind::Movie => client.movie(tmdb_id).await?,
                        ContentKind::Series => client.tv(tmdb_id).await?,
                    };
                    Ok(serde_json::to_value(details)?)
                },
            )
            .await;
            batch.record(outcome);
        }

        if request.kind == ContentKind::Series {
            if let Some(tvdb_id) = request.tvdb_id {
                let key = ProviderCacheKey::new(
                    "tvdb",
                    tvdb_id.to_string(),
                    request.content_id.clone(),
                );
                let client = self.tvdb;
                let outcome = load_json(
                    self.cache,
                    key,
                    now,
                    self.ttl_seconds,
                    move || async move {
                        client.ensure_login().await?;
                        client.series(tvdb_id).await
                    },
                )
                .await;
                batch.record(outcome);
            }
        }

        if let Some(imdb_id) = request.imdb_id.clone() {
            let key = ProviderCacheKey::new("omdb", imdb_id.clone(), request.content_id.clone());
            let client = self.omdb;
            let outcome = load_json(
                self.cache,
                key,
                now,
                self.ttl_seconds,
                move || async move { Ok(serde_json::to_value(client.by_imdb_id(&imdb_id).await?)?) },
            )
            .await;
            batch.record(outcome);
        } else if let Some(title) = request.title.clone() {
            let provider_id = format!(
                "title:{}:{}",
                slug(&title),
                request
                    .year
                    .map(|year| year.to_string())
                    .unwrap_or_else(|| "unknown".to_owned())
            );
            let key = ProviderCacheKey::new("omdb", provider_id, request.content_id.clone());
            let client = self.omdb;
            let year = request.year;
            let outcome = load_json(
                self.cache,
                key,
                now,
                self.ttl_seconds,
                move || async move {
                    Ok(serde_json::to_value(client.by_title_year(&title, year).await?)?)
                },
            )
            .await;
            batch.record(outcome);
        }

        let fanart_id = match request.kind {
            ContentKind::Movie => request.tmdb_id,
            ContentKind::Series => request.tvdb_id,
        };
        if let Some(fanart_id) = fanart_id {
            let provider_id = fanart_id.to_string();
            let key = ProviderCacheKey::new(
                "fanart",
                provider_id,
                request.content_id.clone(),
            );
            let client = self.fanart;
            let kind = request.kind;
            let outcome = load_json(
                self.cache,
                key,
                now,
                self.ttl_seconds,
                move || async move {
                    let payload = match kind {
                        ContentKind::Movie => client.movie(fanart_id).await?,
                        ContentKind::Series => client.tv(fanart_id).await?,
                    };
                    Ok(serde_json::to_value(payload)?)
                },
            )
            .await;
            batch.record(outcome);
        }

        batch
    }
}

async fn load_json<F, Fut>(
    cache: &mut MetadataCache,
    key: ProviderCacheKey,
    now: u64,
    ttl_seconds: u64,
    fetch: F,
) -> ProviderOutcome
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    if let Some(entry) = cache.fresh(&key, now) {
        return ProviderOutcome {
            payload: Some(payload_from_entry(entry, true, false, now)),
            failure: None,
        };
    }

    match fetch().await {
        Ok(payload) => {
            cache.store_success(key.clone(), payload.clone(), now, ttl_seconds);
            ProviderOutcome {
                payload: Some(EnrichmentPayload {
                    provider: key.provider,
                    provider_id: key.provider_id,
                    content_id: key.content_id,
                    payload,
                    fetched_at: now,
                    expires_at: now.saturating_add(ttl_seconds),
                    from_cache: false,
                    stale: false,
                }),
                failure: None,
            }
        }
        Err(error) => {
            let stale = cache.stale(&key);
            let used_stale = stale.is_some();
            let http_status = http_status(&error);
            let error_message = error.to_string();
            cache.store_failure(
                key.clone(),
                now,
                http_status,
                error_message.clone(),
            );
            ProviderOutcome {
                payload: stale.map(|entry| payload_from_entry(entry, true, true, now)),
                failure: Some(EnrichmentFailure {
                    provider: key.provider,
                    provider_id: key.provider_id,
                    content_id: key.content_id,
                    error: error_message,
                    http_status,
                    used_stale,
                }),
            }
        }
    }
}

fn payload_from_entry(
    entry: ProviderCacheEntry,
    from_cache: bool,
    stale: bool,
    now: u64,
) -> EnrichmentPayload {
    EnrichmentPayload {
        provider: entry.key.provider,
        provider_id: entry.key.provider_id,
        content_id: entry.key.content_id,
        payload: entry.payload.unwrap_or(Value::Null),
        fetched_at: entry.fetched_at.unwrap_or(now),
        expires_at: entry.expires_at.unwrap_or(now),
        from_cache,
        stale,
    }
}

fn http_status(error: &ProbeError) -> Option<u16> {
    match error {
        ProbeError::HttpStatus { status, .. } => Some(*status),
        _ => None,
    }
}

fn slug(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::{
        ContentKind, EnrichmentCoordinator, EnrichmentRequest, MetadataCache, ProviderCacheKey,
    };
    use crate::providers::{FanartClient, OmdbClient, TmdbClient, TvdbClient};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn uses_all_provider_clients_once_then_reads_fresh_cache() {
        let (base_url, server) = scripted_server(vec![
            (200, "GET /tv/603", include_str!("../fixtures/tmdb_movie.json")),
            (200, "POST /login", include_str!("../fixtures/tvdb_login.json")),
            (200, "GET /series/67890", include_str!("../fixtures/tvdb_series.json")),
            (200, "GET /?", include_str!("../fixtures/omdb_movie.json")),
            (200, "GET /tv/67890", include_str!("../fixtures/fanart_movie.json")),
        ])
        .await;
        let tmdb = TmdbClient::with_base_url(&base_url, "tmdb-key");
        let tvdb = TvdbClient::with_base_url(&base_url, "tvdb-key");
        let omdb = OmdbClient::with_base_url(&base_url, "omdb-key");
        let fanart = FanartClient::with_base_url(&base_url, "fanart-key");
        let request = EnrichmentRequest {
            content_id: "series:67890".to_owned(),
            kind: ContentKind::Series,
            title: Some("The Matrix".to_owned()),
            year: Some(1999),
            tmdb_id: Some(603),
            tvdb_id: Some(67890),
            imdb_id: Some("tt0133093".to_owned()),
        };
        let mut cache = MetadataCache::default();
        let (first, second) = {
            let mut coordinator = EnrichmentCoordinator::new(
                &tmdb, &tvdb, &omdb, &fanart, &mut cache, 3600,
            );
            let first = coordinator.enrich(&request, 100).await;
            let second = coordinator.enrich(&request, 101).await;
            (first, second)
        };

        assert_eq!(first.payloads.len(), 4);
        assert!(first.failures.is_empty());
        assert_eq!(second.payloads.len(), 4);
        assert!(second.failures.is_empty());
        assert!(second.payloads.values().all(|payload| payload.from_cache));
        assert!(second.payloads.values().all(|payload| !payload.stale));
        assert_eq!(cache.len(), 4);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stale_provider_failure_keeps_payload_and_does_not_block_other_providers() {
        let (base_url, server) = scripted_server(vec![
            (200, "GET /movie/603", include_str!("../fixtures/tmdb_movie.json")),
            (200, "GET /?", include_str!("../fixtures/omdb_movie.json")),
            (200, "GET /movies/603", include_str!("../fixtures/fanart_movie.json")),
            (503, "GET /movie/603", "temporary"),
            (503, "GET /movie/603", "temporary"),
            (503, "GET /movie/603", "temporary"),
            (200, "GET /?", include_str!("../fixtures/omdb_movie.json")),
            (200, "GET /movies/603", include_str!("../fixtures/fanart_movie.json")),
        ])
        .await;
        let tmdb = TmdbClient::with_base_url(&base_url, "tmdb-key");
        let tvdb = TvdbClient::with_base_url(&base_url, "tvdb-key");
        let omdb = OmdbClient::with_base_url(&base_url, "omdb-key");
        let fanart = FanartClient::with_base_url(&base_url, "fanart-key");
        let request = EnrichmentRequest {
            content_id: "movie:603".to_owned(),
            kind: ContentKind::Movie,
            title: Some("The Matrix".to_owned()),
            year: Some(1999),
            tmdb_id: Some(603),
            tvdb_id: None,
            imdb_id: Some("tt0133093".to_owned()),
        };
        let mut cache = MetadataCache::default();
        let second = {
            let mut coordinator = EnrichmentCoordinator::new(
                &tmdb, &tvdb, &omdb, &fanart, &mut cache, 10,
            );
            coordinator.enrich(&request, 100).await;
            coordinator.enrich(&request, 200).await
        };

        assert_eq!(second.failures.len(), 1);
        assert_eq!(second.failures[0].provider, "tmdb");
        assert_eq!(second.failures[0].http_status, Some(503));
        assert!(second.failures[0].used_stale);
        assert!(second.payloads["tmdb"].from_cache);
        assert!(second.payloads["tmdb"].stale);
        assert!(!second.payloads["omdb"].stale);
        assert!(!second.payloads["fanart"].stale);

        let key = ProviderCacheKey::new("tmdb", "603", "movie:603");
        let entry = cache.entry(&key).unwrap();
        assert_eq!(entry.fetched_at, Some(100));
        assert_eq!(entry.attempted_at, 200);
        assert_eq!(entry.error.as_deref(), Some("tmdb returned HTTP 503"));
        server.await.unwrap();
    }

    async fn scripted_server(
        responses: Vec<(u16, &'static str, &'static str)>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for (status, expected_request, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 8192];
                let bytes_read = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..bytes_read]);
                assert!(
                    request.contains(expected_request),
                    "request did not contain {expected_request}: {request}"
                );
                let reason = if status == 200 { "OK" } else { "Error" };
                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nRetry-After: 0\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    reason,
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (format!("http://{}", address), server)
    }
}
