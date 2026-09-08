pub mod config;
pub mod normalization;
pub mod providers;
pub mod stack;

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
}

pub type Result<T> = std::result::Result<T, ProbeError>;

#[cfg(test)]
mod tests {
    use super::normalization::NormalizedMetadata;
    use super::providers::{
        parse_fanart_payload, parse_omdb_response, parse_tmdb_details, parse_tvdb_login,
    };
    use std::collections::BTreeMap;
    use super::stack::{
        parse_plex_library_items, parse_plex_sections, SonarrSeries, RadarrMovie,
    };

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
}
