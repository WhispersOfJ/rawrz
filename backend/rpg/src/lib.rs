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
pub(crate) mod wizard_store;

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
    #[error("connection pool failed: {0}")]
    Pool(String),
    #[error("value out of range: {0}")]
    OutOfRange(String),
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("invalid archetype selection: {0}")]
    InvalidArchetypeSelection(String),
    #[error("archetype selection conflict: {0}")]
    ArchetypeSelectionConflict(String),
    #[error("invalid genre access: {0}")]
    InvalidGenreAccess(String),
}

pub type Result<T> = std::result::Result<T, ProbeError>;

/// Provider responses are trusted LAN services, but a misbehaving service or
/// a bad redirect must not OOM the process (F-14): bodies are streamed with
/// this hard cap.
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Reads a response body with a hard size cap (F-14).
pub(crate) async fn body_limited(response: reqwest::Response) -> Result<Vec<u8>> {
    if let Some(length) = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
    {
        if length > MAX_BODY_BYTES {
            return Err(ProbeError::Xml(format!(
                "response body of {length} bytes exceeds the {MAX_BODY_BYTES} byte cap"
            )));
        }
    }
    let mut body = Vec::new();
    let mut response = response;
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(ProbeError::Xml(format!(
                "response body exceeds the {MAX_BODY_BYTES} byte cap"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Reads a response body as text with the hard size cap (F-14).
pub(crate) async fn text_limited(response: reqwest::Response) -> Result<String> {
    let body = body_limited(response).await?;
    String::from_utf8(body)
        .map_err(|error| ProbeError::Xml(format!("response body is not UTF-8: {error}")))
}

/// Reads and decodes a JSON response body with the hard size cap (F-14).
pub(crate) async fn json_limited<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T> {
    let body = body_limited(response).await?;
    Ok(serde_json::from_slice(&body)?)
}

/// One process-wide HTTP client (F-18): all stack and provider clients share
/// a single connection pool with uniform timeouts. `reqwest::Client` is an
/// cheaply cloneable handle, so every caller gets the same pool.
pub(crate) fn shared_http_client() -> reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("shared HTTP client builds")
        })
        .clone()
}

/// 128-bit random hex string (audit event keys, collision fallbacks).
pub(crate) fn random_token_hex() -> String {
    use rand::RngCore;
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut out = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

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
        parse_plex_library_items, parse_plex_sections, parse_plex_show_items,
        parse_plex_watchable_items, PlexClient, RadarrMovie, SonarrSeries,
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
    fn parses_plex_episode_rows_with_show_parent() {
        // Nested parse: show + season + two episode leaves. The watchable
        // filter is what sync consumes — episodes only, with parent linkage.
        let items =
            parse_plex_library_items(include_str!("../fixtures/plex_episodes.xml")).unwrap();
        assert_eq!(items.len(), 4, "show + season + 2 episodes");
        let episodes =
            parse_plex_watchable_items(include_str!("../fixtures/plex_episodes.xml")).unwrap();
        assert_eq!(episodes.len(), 2, "episodes only; show/season rows filtered");
        assert_eq!(episodes[0].item_type.as_deref(), Some("episode"));
        assert_eq!(episodes[0].rating_key.as_deref(), Some("901"));
        assert_eq!(
            episodes[0].parent_rating_key.as_deref(),
            Some("900"),
            "episode rows carry their show's ratingKey"
        );
    }

    #[test]
    fn nested_same_name_tags_do_not_close_a_show_item_early() {
        // A show item whose season children share the <Directory> tag name:
        // the stack pops children first, so the show closes only at its own
        // End event — no early close, no lost attributes.
        let xml = r#"<MediaContainer size="1">
  <Directory ratingKey="900" title="Fixture Show" type="show" viewCount="3">
    <Genre tag="Comedy" />
    <Directory ratingKey="9010" title="Season 1" type="season">
      <Directory ratingKey="9011" title="Specials" type="season" />
    </Directory>
    <Directory ratingKey="9020" title="Season 2" type="season" />
  </Directory>
</MediaContainer>"#;
        let items = parse_plex_library_items(xml).unwrap();
        let shows: Vec<_> = items
            .iter()
            .filter(|item| item.item_type.as_deref() == Some("show"))
            .collect();
        assert_eq!(shows.len(), 1, "exactly one show item despite nested seasons");
        assert_eq!(shows[0].rating_key.as_deref(), Some("900"));
        assert_eq!(shows[0].genres, vec!["Comedy"]);
        assert_eq!(
            shows[0].attr("viewCount").as_deref(),
            Some("3"),
            "show attributes survive nested closes"
        );
        // The nested season fragments parse too — and never steal the show's
        // attributes or close it early.
        assert_eq!(
            items.iter().filter(|i| i.item_type.as_deref() == Some("season")).count(),
            3,
            "nested season rows are separate items"
        );
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
    async fn response_bodies_are_capped() {
        // Content-Length above the cap is rejected before reading.
        let (base_url, server) = oversize_mock_server(64 * 1024 * 1024 + 1, 0).await;
        let response = super::send_with_retry("fixture", reqwest::Client::new().get(&base_url))
            .await
            .unwrap();
        let error = super::text_limited(response).await.unwrap_err();
        assert!(error.to_string().contains("cap"), "{error}");
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

    #[allow(dead_code)]
    fn parse_plex_show_items_marker(items: &[super::stack::PlexLibraryItem]) -> usize {
        parse_plex_show_items(items)
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

    /// Serves a response whose Content-Length claims `length` bytes but sends
    /// only a stub body — the cap check must fire on the header.
    async fn oversize_mock_server(
        length: usize,
        _body_bytes: usize,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\nstub",
                length
            );
            stream.write_all(response.as_bytes()).await.unwrap();
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
