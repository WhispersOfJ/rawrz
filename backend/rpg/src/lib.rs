pub mod achievements;
pub mod awards;
pub mod auth;
pub mod game;
pub mod config;
pub mod enrichment;
pub mod migrations;
pub mod normalization;
pub mod persistence;
pub mod pipeline;
pub mod poll;
pub mod providers;
pub mod server;
pub mod stack;
pub mod sync;
pub mod wizard;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("{provider} returned HTTP {status}")]
    HttpStatus { provider: &'static str, status: u16 },
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON decoding failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("XML decoding failed: {0}")]
    Xml(String),
    #[error("missing required environment variable {0}")]
    MissingEnvironment(String),
    #[error("invalid {origin} content record: {detail}")]
    InvalidContent {
        origin: &'static str,
        detail: String,
    },
    #[error("PIN must be 4-12 digits")]
    InvalidPin,
    #[error("PIN hashing failed: {0}")]
    PinHashing(String),
    #[error("database operation failed: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("invalid archetype selection: {0}")]
    InvalidArchetypeSelection(String),
    #[error("archetype selection conflict: {0}")]
    ArchetypeSelectionConflict(String),
}

pub type Result<T> = std::result::Result<T, ProbeError>;

pub(crate) async fn send_with_retry(
    provider: &'static str,
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    const MAX_ATTEMPTS: usize = 3;
    let retry_template = request.try_clone().ok_or_else(|| ProbeError::HttpStatus {
        provider,
        status: 400,
    })?;
    let mut request = Some(request);

    for attempt in 1..=MAX_ATTEMPTS {
        let response = request
            .take()
            .expect("retry request missing")
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(response);
        }

        let retryable = response.status().as_u16() == 429 || response.status().is_server_error();
        if !retryable || attempt == MAX_ATTEMPTS {
            return Err(ProbeError::HttpStatus {
                provider,
                status: response.status().as_u16(),
            });
        }

        let delay_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1_u64 << (attempt - 1));
        tokio::time::sleep(std::time::Duration::from_secs(delay_seconds.min(4))).await;
        request = Some(retry_template.try_clone().ok_or_else(|| ProbeError::HttpStatus {
            provider,
            status: response.status().as_u16(),
        })?);
    }

    unreachable!("retry loop always returns")
}

#[cfg(test)]
mod tests {
    use super::normalization::NormalizedMetadata;
    use super::ProbeError;
    use super::providers::{
        parse_fanart_payload, parse_omdb_response, parse_tmdb_details, parse_tvdb_login,
        TmdbClient,
    };
    use super::stack::{
        parse_plex_library_items, parse_plex_sections, PlexClient, RadarrMovie, SonarrSeries,
    };
    use std::collections::BTreeMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn parses_plex_sections_and_library_genres() {
        let sections = parse_plex_sections(include_str!("../fixtures/plex_sections.xml")).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title.as_deref(), Some("Movies"));
        assert_eq!(sections[1].kind.as_deref(), Some("show"));

        let items = parse_plex_library_items(include_str!("../fixtures/plex_library.xml")).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title.as_deref(), Some("Fear Street: Part One - 1994"));
        assert_eq!(items[0].genres, vec!["Horror", "Mystery"]);
        assert_eq!(items[1].item_type.as_deref(), Some("show"));
    }

    #[test]
    fn parses_arr_payloads_with_external_ids_and_raw_fields() {
        let series: Vec<SonarrSeries> = serde_json::from_str(include_str!("../fixtures/sonarr_series.json")).unwrap();
        assert_eq!(series[0].tmdb_id, Some(12345));
        assert_eq!(series[0].tvdb_id, Some(67890));
        assert_eq!(series[0].raw.get("status").and_then(|v| v.as_str()), Some("continuing"));

        let movies: Vec<RadarrMovie> = serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        assert_eq!(movies[0].tmdb_id, Some(591275));
        assert_eq!(movies[0].imdb_id.as_deref(), Some("tt1234567"));
        assert_eq!(movies[0].raw.get("ratings").and_then(|v| v.get("tmdb")).and_then(|v| v.get("value")).and_then(|v| v.as_f64()), Some(7.5));
    }

    #[test]
    fn parses_tmdb_omdb_fanart_and_tvdb_enrichment() {
        let tmdb = parse_tmdb_details(include_str!("../fixtures/tmdb_movie.json")).unwrap();
        assert_eq!(tmdb.id, 603);
        assert!(tmdb.genres.iter().any(|genre| genre.name == "Drama"));
        assert!(tmdb.keywords().iter().any(|keyword| keyword.name == "dream"));

        let omdb = parse_omdb_response(include_str!("../fixtures/omdb_movie.json")).unwrap();
        assert_eq!(omdb.imdb_id.as_deref(), Some("tt0133093"));
        assert_eq!(omdb.imdb_rating.as_deref(), Some("8.7"));
        assert_eq!(omdb.rotten_tomatoes_rating().as_deref(), Some("83%"));

        let fanart = parse_fanart_payload(include_str!("../fixtures/fanart_movie.json")).unwrap();
        assert_eq!(fanart.images_for("movieposter").len(), 1);
        assert_eq!(fanart.images_for("movieposter")[0].url.as_deref(), Some("https://assets.example/poster.jpg"));

        let login = parse_tvdb_login(include_str!("../fixtures/tvdb_login.json")).unwrap();
        assert_eq!(login.token, "fixture-token");
    }

    #[test]
    fn normalizes_provider_metadata_with_precedence_and_provenance() {
        let plex = parse_plex_library_items(include_str!("../fixtures/plex_library.xml")).unwrap();
        let radarr: Vec<RadarrMovie> =
            serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        let tmdb = parse_tmdb_details(include_str!("../fixtures/tmdb_movie.json")).unwrap();
        let omdb = parse_omdb_response(include_str!("../fixtures/omdb_movie.json")).unwrap();
        let fanart = parse_fanart_payload(include_str!("../fixtures/fanart_movie.json")).unwrap();
        let tvdb: serde_json::Value = serde_json::from_str(include_str!("../fixtures/tvdb_series.json")).unwrap();
        let metadata = NormalizedMetadata::from_sources(
            Some(&plex[0]),
            None,
            Some(&radarr[0]),
            Some(&tmdb),
            Some(&omdb),
            Some(&tvdb),
            Some(&fanart),
            &BTreeMap::from([
                (String::from("dream"), String::from("Science Fiction")),
                (String::from("mystery"), String::from("Mystery")),
            ]),
        );

        assert_eq!(metadata.title.as_deref(), Some("The Matrix"));
        assert_eq!(metadata.tmdb_id, Some(603));
        assert_eq!(metadata.imdb_id.as_deref(), Some("tt0133093"));
        assert_eq!(metadata.featured_score.as_ref().map(|score| score.provider.as_str()), Some("tmdb"));
        assert!(metadata.sub_genres.iter().any(|tag| tag.slug == "dream"));
        assert_eq!(metadata.artwork.len(), 2);
        assert_eq!(metadata.provenance["horror"], vec!["plex"]);
    }

    #[tokio::test]
    async fn retries_transient_responses_but_not_client_errors() {
        let (base_url, server) = retry_mock_server(vec![503, 429, 200], "ok").await;
        let response = super::send_with_retry("fixture", reqwest::Client::new().get(&base_url))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        server.await.unwrap();

        let (base_url, server) = retry_mock_server(vec![401], "unauthorized").await;
        let error = super::send_with_retry("fixture", reqwest::Client::new().get(&base_url))
            .await
            .unwrap_err();
        assert!(matches!(error, ProbeError::HttpStatus { status: 401, .. }));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn clients_use_injected_base_urls_without_live_services() {
        let (base_url, server) = mock_server(
            "/library/sections",
            "<MediaContainer size=\"0\"></MediaContainer>",
        )
        .await;
        assert!(PlexClient::with_base_url(base_url, "fixture-token")
            .sections()
            .await
            .unwrap()
            .is_empty());
        server.await.unwrap();

        let (base_url, server) = mock_server(
            "/movie/603",
            include_str!("../fixtures/tmdb_movie.json"),
        )
        .await;
        let details = TmdbClient::with_base_url(base_url, "fixture-key")
            .movie(603)
            .await
            .unwrap();
        assert_eq!(details.id, 603);
        server.await.unwrap();
    }

    async fn retry_mock_server(
        statuses: Vec<u16>,
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for status in statuses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).await.unwrap();
                let reason = if status == 200 { "OK" } else { "Error" };
                let response = format!(
                    "HTTP/1.1 {} {}\r\nRetry-After: 0\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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

    async fn mock_server(expected_path: &'static str, body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let bytes_read = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..bytes_read]);
            assert!(request.contains(expected_path), "request did not contain {expected_path}: {request}");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        (format!("http://{}", address), server)
    }
}
