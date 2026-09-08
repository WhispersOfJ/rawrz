use crate::enrichment::MetadataCache;
use crate::persistence::{
    ContentPersistencePlan, PersistenceSummary, PostgresContentStore,
};
use crate::providers::{FanartClient, OmdbClient, TmdbClient, TvdbClient};
use crate::sync::{
    EnrichedSyncOutcome, ProviderEnrichmentOrchestrator, StackSyncOrchestrator,
};
use crate::Result;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreparedSync {
    pub outcome: EnrichedSyncOutcome,
    pub plan: ContentPersistencePlan,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncRunResult {
    pub prepared: PreparedSync,
    pub persistence: PersistenceSummary,
}

pub struct SyncPipeline<'a> {
    plex: &'a crate::stack::PlexClient,
    sonarr: &'a crate::stack::SonarrClient,
    radarr: &'a crate::stack::RadarrClient,
    tmdb: &'a TmdbClient,
    tvdb: &'a TvdbClient,
    omdb: &'a OmdbClient,
    fanart: &'a FanartClient,
    cache: &'a mut MetadataCache,
    ttl_seconds: u64,
    keyword_parent_map: BTreeMap<String, String>,
}

impl<'a> SyncPipeline<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        plex: &'a crate::stack::PlexClient,
        sonarr: &'a crate::stack::SonarrClient,
        radarr: &'a crate::stack::RadarrClient,
        tmdb: &'a TmdbClient,
        tvdb: &'a TvdbClient,
        omdb: &'a OmdbClient,
        fanart: &'a FanartClient,
        cache: &'a mut MetadataCache,
        ttl_seconds: u64,
        keyword_parent_map: BTreeMap<String, String>,
    ) -> Self {
        Self {
            plex,
            sonarr,
            radarr,
            tmdb,
            tvdb,
            omdb,
            fanart,
            cache,
            ttl_seconds,
            keyword_parent_map,
        }
    }

    pub async fn prepare(&mut self, now: u64) -> PreparedSync {
        let stack_outcome = StackSyncOrchestrator::new(self.plex, self.sonarr, self.radarr)
            .collect()
            .await;
        let enriched_outcome = {
            let mut enrichment = ProviderEnrichmentOrchestrator::new(
                self.tmdb,
                self.tvdb,
                self.omdb,
                self.fanart,
                self.cache,
                self.ttl_seconds,
                self.keyword_parent_map.clone(),
            );
            enrichment.enrich(stack_outcome, now).await
        };
        let plan = ContentPersistencePlan::from_outcome(&enriched_outcome, self.cache);
        PreparedSync {
            outcome: enriched_outcome,
            plan,
        }
    }

    pub async fn run(
        &mut self,
        store: &mut PostgresContentStore,
        now: u64,
    ) -> Result<SyncRunResult> {
        let prepared = self.prepare(now).await;
        let persistence = store.persist(&prepared.plan).await?;
        Ok(SyncRunResult {
            prepared,
            persistence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SyncPipeline;
    use crate::enrichment::MetadataCache;
    use crate::providers::{FanartClient, OmdbClient, TmdbClient, TvdbClient};
    use crate::stack::{PlexClient, RadarrClient, SonarrClient};
    use std::collections::BTreeMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn prepares_healthy_stack_enrichment_and_persistence_plan() {
        let (base_url, server) = pipeline_mock_server(false, false, 15).await;
        let plex = PlexClient::with_base_url(&base_url, "plex-token");
        let sonarr = SonarrClient::with_base_url(&base_url, "sonarr-key");
        let radarr = RadarrClient::with_base_url(&base_url, "radarr-key");
        let tmdb = TmdbClient::with_base_url(format!("{base_url}/tmdb"), "tmdb-key");
        let tvdb = TvdbClient::with_base_url(format!("{base_url}/tvdb"), "tvdb-key");
        let omdb = OmdbClient::with_base_url(format!("{base_url}/omdb"), "omdb-key");
        let fanart = FanartClient::with_base_url(format!("{base_url}/fanart"), "fanart-key");
        let mut cache = MetadataCache::default();

        let prepared = SyncPipeline::new(
            &plex,
            &sonarr,
            &radarr,
            &tmdb,
            &tvdb,
            &omdb,
            &fanart,
            &mut cache,
            3600,
            BTreeMap::new(),
        )
        .prepare(100)
        .await;

        assert!(prepared.outcome.stack_failures.is_empty());
        assert!(prepared.outcome.provider_failures.is_empty());
        assert_eq!(prepared.outcome.batch.len(), 4);
        assert_eq!(prepared.plan.content.len(), 4);
        assert_eq!(prepared.plan.provider_cache.len(), 9);
        assert!(prepared.plan.warnings.is_empty());
        assert!(prepared
            .plan
            .content
            .iter()
            .any(|record| record.provider_metadata != serde_json::json!({})));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn prepares_partial_result_when_stack_and_provider_sources_fail() {
        let (base_url, server) = pipeline_mock_server(true, true, 12).await;
        let plex = PlexClient::with_base_url(&base_url, "plex-token");
        let sonarr = SonarrClient::with_base_url(&base_url, "sonarr-key");
        let radarr = RadarrClient::with_base_url(&base_url, "radarr-key");
        let tmdb = TmdbClient::with_base_url(format!("{base_url}/tmdb"), "tmdb-key");
        let tvdb = TvdbClient::with_base_url(format!("{base_url}/tvdb"), "tvdb-key");
        let omdb = OmdbClient::with_base_url(format!("{base_url}/omdb"), "omdb-key");
        let fanart = FanartClient::with_base_url(format!("{base_url}/fanart"), "fanart-key");
        let mut cache = MetadataCache::default();

        let prepared = SyncPipeline::new(
            &plex,
            &sonarr,
            &radarr,
            &tmdb,
            &tvdb,
            &omdb,
            &fanart,
            &mut cache,
            3600,
            BTreeMap::new(),
        )
        .prepare(100)
        .await;

        assert_eq!(prepared.outcome.stack_failures.len(), 1);
        assert_eq!(prepared.outcome.stack_failures[0].source.as_str(), "sonarr");
        assert_eq!(prepared.outcome.provider_failures.len(), 1);
        assert_eq!(prepared.outcome.provider_failures[0].failure.provider, "tmdb");
        assert_eq!(prepared.outcome.provider_failures[0].failure.http_status, Some(503));
        assert_eq!(prepared.outcome.batch.len(), 3);
        assert_eq!(prepared.plan.content.len(), 3);
        assert_eq!(prepared.plan.provider_cache.len(), 5);
        assert!(prepared.plan.stack_failures.len() == 1);
        assert!(prepared.plan.provider_failures.len() == 1);
        server.await.unwrap();
    }

    async fn pipeline_mock_server(
        sonarr_failure: bool,
        tmdb_movie_failure: bool,
        expected_requests: usize,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request_buffer = [0_u8; 8192];
                let bytes_read = stream.read(&mut request_buffer).await.unwrap();
                let request = String::from_utf8_lossy(&request_buffer[..bytes_read]);
                let (status, body, content_type) = if request.contains("/library/sections")
                    && !request.contains("/all")
                {
                    (
                        200,
                        include_str!("../fixtures/plex_sections.xml"),
                        "application/xml",
                    )
                } else if request.contains("/library/sections/1/all") {
                    (
                        200,
                        include_str!("../fixtures/plex_library.xml"),
                        "application/xml",
                    )
                } else if request.contains("/library/sections/2/all") {
                    (200, "<MediaContainer size=\"0\"></MediaContainer>", "application/xml")
                } else if request.contains("/api/v3/series") {
                    if sonarr_failure {
                        (401, "unauthorized", "application/json")
                    } else {
                        (
                            200,
                            include_str!("../fixtures/sonarr_series.json"),
                            "application/json",
                        )
                    }
                } else if request.contains("/api/v3/movie") {
                    (
                        200,
                        include_str!("../fixtures/radarr_movies.json"),
                        "application/json",
                    )
                } else if request.contains("/tmdb/movie/591275") && tmdb_movie_failure {
                    (503, "temporary", "text/plain")
                } else if request.contains("/tmdb/movie/") || request.contains("/tmdb/tv/") {
                    (
                        200,
                        include_str!("../fixtures/tmdb_movie.json"),
                        "application/json",
                    )
                } else if request.contains("/tvdb/login") {
                    (200, include_str!("../fixtures/tvdb_login.json"), "application/json")
                } else if request.contains("/tvdb/series/") {
                    (200, include_str!("../fixtures/tvdb_series.json"), "application/json")
                } else if request.contains("/fanart/movies/") || request.contains("/fanart/tv/") {
                    (
                        200,
                        include_str!("../fixtures/fanart_movie.json"),
                        "application/json",
                    )
                } else if request.contains("GET /omdb?") {
                    (
                        200,
                        include_str!("../fixtures/omdb_movie.json"),
                        "application/json",
                    )
                } else {
                    (404, "not found", "text/plain")
                };
                let reason = if status == 200 { "OK" } else { "Error" };
                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nRetry-After: 0\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    reason,
                    content_type,
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (format!("http://{}", address), server)
    }
}
