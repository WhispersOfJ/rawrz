use crate::normalization::NormalizedMetadata;
use crate::stack::{PlexLibraryItem, RadarrMovie, SonarrSeries};
use crate::{ProbeError, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ContentSource {
    Plex,
    Sonarr,
    Radarr,
}

impl ContentSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plex => "plex",
            Self::Sonarr => "sonarr",
            Self::Radarr => "radarr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum ContentType {
    Movie,
    Series,
    Episode,
}

impl ContentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Series => "series",
            Self::Episode => "episode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ContentUpsertKey {
    pub source: String,
    pub source_id: String,
}

impl ContentUpsertKey {
    pub fn new(source: ContentSource, source_id: impl Into<String>) -> Self {
        Self {
            source: source.as_str().to_owned(),
            source_id: source_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ContentIdentityKey {
    External {
        content_type: ContentType,
        external_id_type: String,
        external_id: String,
    },
    TitleYear {
        content_type: ContentType,
        title: String,
        year: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContentSyncGroup {
    pub identity: ContentIdentityKey,
    pub records: BTreeMap<ContentUpsertKey, ContentSyncRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ContentSyncBatch {
    pub groups: BTreeMap<ContentIdentityKey, ContentSyncGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContentSyncRecord {
    pub key: ContentUpsertKey,
    pub external_id: Option<String>,
    pub external_id_type: Option<String>,
    pub title: String,
    pub year: Option<i32>,
    pub content_type: ContentType,
    pub parent_key: Option<ContentUpsertKey>,
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
    pub genres: Vec<String>,
    pub sub_genres: Vec<String>,
    pub metadata_blob: Value,
    pub provider_metadata: Value,
}

impl ContentSyncRecord {
    pub fn identity_key(&self) -> ContentIdentityKey {
        match (&self.external_id, &self.external_id_type) {
            (Some(external_id), Some(external_id_type)) => ContentIdentityKey::External {
                content_type: self.content_type,
                external_id_type: external_id_type.clone(),
                external_id: external_id.clone(),
            },
            _ => ContentIdentityKey::TitleYear {
                content_type: self.content_type,
                title: normalize_title(&self.title),
                year: self.year,
            },
        }
    }

    pub fn from_plex(item: &PlexLibraryItem) -> Result<Self> {
        let source_id = required(item.rating_key.clone(), "ratingKey")?;
        let title = required(item.title.clone(), "title")?;
        let content_type = match item.item_type.as_deref() {
            Some("movie") => ContentType::Movie,
            Some("show") => ContentType::Series,
            Some(other) => {
                return Err(ProbeError::InvalidContent {
                    origin: "plex",
                    detail: format!("unsupported type {other:?}"),
                })
            }
            None => {
                return Err(ProbeError::InvalidContent {
                    origin: "plex",
                    detail: "missing type".to_owned(),
                })
            }
        };
        let attrs = attributes_to_map(&item.raw_attributes);
        let rating = first_f64(&attrs, &["audienceRating", "rating"]);
        let rating_source = attrs
            .get("ratingImage")
            .and_then(Value::as_str)
            .map(rating_source_name);

        Ok(Self {
            key: ContentUpsertKey::new(ContentSource::Plex, source_id),
            external_id: first_attr(&attrs, &["tmdbId", "tvdbId"]),
            external_id_type: if attrs.get("tmdbId").is_some() {
                Some("tmdb".to_owned())
            } else if attrs.get("tvdbId").is_some() {
                Some("tvdb".to_owned())
            } else {
                None
            },
            title,
            year: item.year,
            content_type,
            parent_key: None,
            season_number: None,
            episode_number: None,
            runtime_seconds: attrs
                .get("duration")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i64>().ok())
                .map(|value| (value / 1000) as i32),
            release_date: attrs
                .get("originallyAvailableAt")
                .and_then(Value::as_str)
                .map(str::to_owned),
            first_air_date: attrs
                .get("originallyAvailableAt")
                .and_then(Value::as_str)
                .map(str::to_owned),
            status: attrs.get("status").and_then(Value::as_str).map(str::to_owned),
            summary: attrs.get("summary").and_then(Value::as_str).map(str::to_owned),
            rating,
            rating_source,
            poster_url: attrs.get("thumb").and_then(Value::as_str).map(str::to_owned),
            fanart_url: attrs
                .get("art")
                .and_then(Value::as_str)
                .map(str::to_owned),
            section_key: attrs
                .get("librarySectionKey")
                .and_then(Value::as_str)
                .map(str::to_owned),
            section_title: attrs
                .get("librarySectionTitle")
                .and_then(Value::as_str)
                .map(str::to_owned),
            genres: item.genres.clone(),
            sub_genres: Vec::new(),
            metadata_blob: Value::Object(attrs),
            provider_metadata: json!({}),
        })
    }

    pub fn from_sonarr(series: &SonarrSeries) -> Result<Self> {
        let source_id = required(
            series.id.map(|value| value.to_string()),
            "id",
        )?;
        let title = required(series.title.clone(), "title")?;
        let external_id = series.tmdb_id.or(series.tvdb_id).map(|value| value.to_string());
        let external_id_type = if series.tmdb_id.is_some() {
            Some("tmdb".to_owned())
        } else if series.tvdb_id.is_some() {
            Some("tvdb".to_owned())
        } else {
            None
        };

        Ok(Self {
            key: ContentUpsertKey::new(ContentSource::Sonarr, source_id),
            external_id,
            external_id_type,
            title,
            year: series.year,
            content_type: ContentType::Series,
            parent_key: None,
            season_number: None,
            episode_number: None,
            runtime_seconds: json_i32(&series.raw, &["runtime"]),
            release_date: json_string(&series.raw, &["firstAired"]),
            first_air_date: json_string(&series.raw, &["firstAired"]),
            status: json_string(&series.raw, &["status"]),
            summary: json_string(&series.raw, &["overview"]),
            rating: json_f64(&series.raw, &["ratings", "value"]),
            rating_source: json_string(&series.raw, &["ratings", "type"]),
            poster_url: json_string(&series.raw, &["images", "poster"]),
            fanart_url: json_string(&series.raw, &["images", "fanart"]),
            section_key: None,
            section_title: None,
            genres: json_string_array(&series.raw, "genres"),
            sub_genres: Vec::new(),
            metadata_blob: Value::Object(series.raw.clone()),
            provider_metadata: json!({}),
        })
    }

    pub fn from_radarr(movie: &RadarrMovie) -> Result<Self> {
        let source_id = required(movie.id.map(|value| value.to_string()), "id")?;
        let title = required(movie.title.clone(), "title")?;
        let external_id = movie.tmdb_id.map(|value| value.to_string());
        let external_id_type = external_id.as_ref().map(|_| "tmdb".to_owned());

        Ok(Self {
            key: ContentUpsertKey::new(ContentSource::Radarr, source_id),
            external_id,
            external_id_type,
            title,
            year: movie.year,
            content_type: ContentType::Movie,
            parent_key: None,
            season_number: None,
            episode_number: None,
            runtime_seconds: json_i32(&movie.raw, &["runtime"]),
            release_date: json_string(&movie.raw, &["physicalRelease"]),
            first_air_date: None,
            status: json_string(&movie.raw, &["status"]),
            summary: json_string(&movie.raw, &["overview"]),
            rating: json_f64(&movie.raw, &["ratings", "tmdb", "value"]),
            rating_source: Some("radarr_tmdb".to_owned()),
            poster_url: json_string(&movie.raw, &["images", "url"]),
            fanart_url: None,
            section_key: None,
            section_title: None,
            genres: json_string_array(&movie.raw, "genres"),
            sub_genres: Vec::new(),
            metadata_blob: Value::Object(movie.raw.clone()),
            provider_metadata: json!({}),
        })
    }

    pub fn attach_normalized_metadata(&mut self, metadata: &NormalizedMetadata) -> Result<()> {
        if let Some(tmdb_id) = metadata.tmdb_id {
            self.external_id = Some(tmdb_id.to_string());
            self.external_id_type = Some("tmdb".to_owned());
        } else if self.external_id.is_none() {
            if let Some(tvdb_id) = metadata.tvdb_id {
                self.external_id = Some(tvdb_id.to_string());
                self.external_id_type = Some("tvdb".to_owned());
            }
        }

        if !metadata.genres.is_empty() {
            self.genres = tag_names(&metadata.genres);
        }
        self.sub_genres = tag_names(&metadata.sub_genres);

        if let Some(score) = &metadata.featured_score {
            self.rating = Some(score.value);
            self.rating_source = Some(score.provider.clone());
        }

        if let Some(url) = artwork_url(&metadata.artwork, |kind| kind.contains("poster")) {
            self.poster_url = Some(url);
        }
        if let Some(url) = artwork_url(&metadata.artwork, |kind| {
            kind.contains("background") || kind.contains("backdrop") || kind.contains("fanart")
        }) {
            self.fanart_url = Some(url);
        }

        self.provider_metadata = serde_json::to_value(metadata)?;
        Ok(())
    }

    pub fn deduplicate(records: impl IntoIterator<Item = Self>) -> BTreeMap<ContentUpsertKey, Self> {
        records
            .into_iter()
            .map(|record| (record.key.clone(), record))
            .collect()
    }
}

impl ContentSyncBatch {
    pub fn from_records(records: impl IntoIterator<Item = ContentSyncRecord>) -> Self {
        let mut groups = ContentSyncRecord::deduplicate(records)
            .into_values()
            .fold(BTreeMap::new(), |mut groups, record| {
                let identity = record.identity_key();
                groups
                    .entry(identity.clone())
                    .or_insert_with(|| ContentSyncGroup {
                        identity,
                        records: BTreeMap::new(),
                    })
                    .records
                    .insert(record.key.clone(), record);
                groups
            });

        let fallback_groups = groups
            .keys()
            .filter_map(|identity| match identity {
                ContentIdentityKey::TitleYear { .. } => Some(identity.clone()),
                ContentIdentityKey::External { .. } => None,
            })
            .collect::<Vec<_>>();

        for fallback_identity in fallback_groups {
            let matching_external = groups
                .keys()
                .filter(|identity| {
                    matches!(identity, ContentIdentityKey::External { .. })
                        && same_title_year(identity, groups.get(&fallback_identity).unwrap())
                })
                .cloned()
                .collect::<Vec<_>>();

            if matching_external.len() != 1 {
                continue;
            }

            let external_identity = matching_external.into_iter().next().unwrap();
            let fallback_group = groups.remove(&fallback_identity).unwrap();
            groups
                .get_mut(&external_identity)
                .expect("external identity group disappeared")
                .records
                .extend(fallback_group.records);
        }

        Self {
            groups: groups
                .into_values()
                .map(|group| (group.identity.clone(), group))
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn groups(&self) -> impl Iterator<Item = &ContentSyncGroup> {
        self.groups.values()
    }
}

fn same_title_year(identity: &ContentIdentityKey, group: &ContentSyncGroup) -> bool {
    let ContentIdentityKey::TitleYear {
        content_type,
        title,
        year,
    } = group.records.values().next().map(ContentSyncRecord::identity_key).unwrap_or_else(|| {
        ContentIdentityKey::TitleYear {
            content_type: ContentType::Movie,
            title: String::new(),
            year: None,
        }
    }) else {
        return false;
    };
    let ContentIdentityKey::External { content_type: external_type, .. } = identity else {
        return false;
    };
    if content_type != *external_type {
        return false;
    }
    group.records.values().any(|record| {
        record.content_type == *external_type
            && normalize_title(&record.title) == title
            && record.year == year
    })
}

fn normalize_title(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tag_names(tags: &[crate::normalization::NormalizedTag]) -> Vec<String> {
    let mut names = tags.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn artwork_url(
    artwork: &[crate::normalization::Artwork],
    matches_kind: impl Fn(&str) -> bool,
) -> Option<String> {
    artwork
        .iter()
        .find(|image| matches_kind(&image.kind))
        .map(|image| image.url.clone())
}

fn required<T>(value: Option<T>, field: &str) -> Result<T> {
    value.ok_or_else(|| ProbeError::InvalidContent {
        origin: "stack",
        detail: format!("missing {field}"),
    })
}

fn attributes_to_map(attributes: &[(String, String)]) -> Map<String, Value> {
    attributes
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect()
}

fn first_attr(attributes: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| attributes.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn first_f64(attributes: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        attributes
            .get(*key)
            .and_then(Value::as_str)
            .and_then(|value| value.parse().ok())
    })
}

fn rating_source_name(value: &str) -> String {
    value
        .split("://")
        .next()
        .unwrap_or(value)
        .to_owned()
}

fn json_value<'a>(raw: &'a Map<String, Value>, path: &[&str]) -> Option<&'a Value> {
    let (first, rest) = path.split_first()?;
    let mut current = raw.get(*first);
    for key in rest {
        current = current?.get(*key);
    }
    current
}

fn json_i32(raw: &Map<String, Value>, path: &[&str]) -> Option<i32> {
    json_value(raw, path)
        .and_then(Value::as_i64)
        .and_then(|value| value.try_into().ok())
}

fn json_f64(raw: &Map<String, Value>, path: &[&str]) -> Option<f64> {
    json_value(raw, path).and_then(Value::as_f64)
}

fn json_string(raw: &Map<String, Value>, path: &[&str]) -> Option<String> {
    json_value(raw, path).and_then(Value::as_str).map(str::to_owned)
}

fn json_string_array(raw: &Map<String, Value>, key: &str) -> Vec<String> {
    raw.get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        ContentIdentityKey, ContentSource, ContentSyncBatch, ContentSyncRecord, ContentType,
    };
    use crate::stack::{PlexLibraryItem, RadarrMovie, SonarrSeries};
    use serde_json::json;

    #[test]
    fn creates_stable_records_for_plex_arr_sources() {
        let plex = crate::stack::parse_plex_library_items(include_str!("../fixtures/plex_library.xml"))
            .unwrap();
        let plex_record = ContentSyncRecord::from_plex(&plex[0]).unwrap();
        assert_eq!(plex_record.key.source, ContentSource::Plex.as_str());
        assert_eq!(plex_record.key.source_id, "271");
        assert_eq!(plex_record.content_type, ContentType::Movie);
        assert_eq!(plex_record.title, "Fear Street: Part One - 1994");
        assert_eq!(plex_record.genres, vec!["Horror", "Mystery"]);
        assert_eq!(plex_record.release_date.as_deref(), Some("2021-07-02"));
        assert_eq!(plex_record.metadata_blob["ratingKey"], json!("271"));

        let series: Vec<SonarrSeries> =
            serde_json::from_str(include_str!("../fixtures/sonarr_series.json")).unwrap();
        let series_record = ContentSyncRecord::from_sonarr(&series[0]).unwrap();
        assert_eq!(series_record.key.source, ContentSource::Sonarr.as_str());
        assert_eq!(series_record.key.source_id, "24");
        assert_eq!(series_record.external_id.as_deref(), Some("12345"));
        assert_eq!(series_record.external_id_type.as_deref(), Some("tmdb"));
        assert_eq!(series_record.content_type, ContentType::Series);
        assert_eq!(series_record.metadata_blob["status"], json!("continuing"));

        let movies: Vec<RadarrMovie> =
            serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        let movie_record = ContentSyncRecord::from_radarr(&movies[0]).unwrap();
        assert_eq!(movie_record.key.source, ContentSource::Radarr.as_str());
        assert_eq!(movie_record.key.source_id, "17");
        assert_eq!(movie_record.external_id.as_deref(), Some("591275"));
        assert_eq!(movie_record.external_id_type.as_deref(), Some("tmdb"));
        assert_eq!(movie_record.content_type, ContentType::Movie);
        assert_eq!(movie_record.rating, Some(7.5));
        assert_eq!(movie_record.rating_source.as_deref(), Some("radarr_tmdb"));
    }

    #[test]
    fn attaches_normalized_provider_fields_for_gameplay_and_audit() {
        let plex = crate::stack::parse_plex_library_items(include_str!("../fixtures/plex_library.xml"))
            .unwrap();
        let radarr: Vec<RadarrMovie> =
            serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        let tmdb = crate::providers::parse_tmdb_details(include_str!("../fixtures/tmdb_movie.json"))
            .unwrap();
        let omdb = crate::providers::parse_omdb_response(include_str!("../fixtures/omdb_movie.json"))
            .unwrap();
        let fanart = crate::providers::parse_fanart_payload(
            include_str!("../fixtures/fanart_movie.json"),
        )
        .unwrap();
        let metadata = crate::normalization::NormalizedMetadata::from_sources(
            Some(&plex[0]),
            None,
            Some(&radarr[0]),
            Some(&tmdb),
            Some(&omdb),
            None,
            Some(&fanart),
            &std::collections::BTreeMap::from([(
                "dream".to_owned(),
                "Science Fiction".to_owned(),
            )]),
        );
        let mut record = ContentSyncRecord::from_radarr(&radarr[0]).unwrap();
        record.attach_normalized_metadata(&metadata).unwrap();

        assert_eq!(record.external_id.as_deref(), Some("603"));
        assert_eq!(record.external_id_type.as_deref(), Some("tmdb"));
        assert_eq!(
            record.genres,
            vec![
                "Action",
                "Drama",
                "Horror",
                "Mystery",
                "Science Fiction"
            ]
        );
        assert_eq!(record.sub_genres, vec!["dream"]);
        assert_eq!(record.rating, Some(8.2));
        assert_eq!(record.rating_source.as_deref(), Some("tmdb"));
        assert_eq!(record.poster_url.as_deref(), Some("https://assets.example/poster.jpg"));
        assert_eq!(record.fanart_url.as_deref(), Some("https://assets.example/background.jpg"));
        assert_eq!(record.provider_metadata["tmdb_id"], json!(603));
        assert!(record.provider_metadata["provenance"]["dream"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider == "tmdb"));
    }

    #[test]
    fn deduplicates_by_source_and_source_id_without_collapsing_cross_source_rows() {
        let records = vec![
            ContentSyncRecord {
                key: super::ContentUpsertKey::new(ContentSource::Plex, "271"),
                external_id: Some("591275".to_owned()),
                external_id_type: Some("tmdb".to_owned()),
                title: "Fixture Movie".to_owned(),
                year: Some(2024),
                content_type: ContentType::Movie,
                parent_key: None,
                season_number: None,
                episode_number: None,
                runtime_seconds: None,
                release_date: None,
                first_air_date: None,
                status: None,
                summary: None,
                rating: None,
                rating_source: None,
                poster_url: None,
                fanart_url: None,
                section_key: None,
                section_title: None,
                genres: Vec::new(),
                sub_genres: Vec::new(),
                metadata_blob: json!({"revision": 1}),
                provider_metadata: json!({}),
            },
            ContentSyncRecord {
                key: super::ContentUpsertKey::new(ContentSource::Plex, "271"),
                external_id: Some("591275".to_owned()),
                external_id_type: Some("tmdb".to_owned()),
                title: "Fixture Movie (updated)".to_owned(),
                year: Some(2024),
                content_type: ContentType::Movie,
                parent_key: None,
                season_number: None,
                episode_number: None,
                runtime_seconds: None,
                release_date: None,
                first_air_date: None,
                status: None,
                summary: None,
                rating: None,
                rating_source: None,
                poster_url: None,
                fanart_url: None,
                section_key: None,
                section_title: None,
                genres: Vec::new(),
                sub_genres: Vec::new(),
                metadata_blob: json!({"revision": 2}),
                provider_metadata: json!({}),
            },
            ContentSyncRecord {
                key: super::ContentUpsertKey::new(ContentSource::Radarr, "17"),
                external_id: Some("591275".to_owned()),
                external_id_type: Some("tmdb".to_owned()),
                title: "Fixture Movie".to_owned(),
                year: Some(2024),
                content_type: ContentType::Movie,
                parent_key: None,
                season_number: None,
                episode_number: None,
                runtime_seconds: None,
                release_date: None,
                first_air_date: None,
                status: None,
                summary: None,
                rating: None,
                rating_source: None,
                poster_url: None,
                fanart_url: None,
                section_key: None,
                section_title: None,
                genres: Vec::new(),
                sub_genres: Vec::new(),
                metadata_blob: json!({}),
                provider_metadata: json!({}),
            },
        ];
        let deduped = ContentSyncRecord::deduplicate(records);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[&super::ContentUpsertKey::new(ContentSource::Plex, "271")].title, "Fixture Movie (updated)");
        assert!(deduped.contains_key(&super::ContentUpsertKey::new(ContentSource::Radarr, "17")));
    }

    #[test]
    fn groups_plex_fallback_with_one_matching_arr_identity() {
        let plex = crate::stack::parse_plex_library_items(include_str!("../fixtures/plex_library.xml"))
            .unwrap();
        let movies: Vec<RadarrMovie> =
            serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        let mut plex_record = ContentSyncRecord::from_plex(&plex[0]).unwrap();
        let mut radarr_record = ContentSyncRecord::from_radarr(&movies[0]).unwrap();
        plex_record.title = "Fixture-Movie".to_owned();
        plex_record.year = Some(2024);
        radarr_record.title = "Fixture Movie".to_owned();
        radarr_record.year = Some(2024);

        let batch = ContentSyncBatch::from_records([plex_record, radarr_record]);
        assert_eq!(batch.len(), 1);
        let group = batch.groups().next().unwrap();
        assert_eq!(
            group.identity,
            ContentIdentityKey::External {
                content_type: ContentType::Movie,
                external_id_type: "tmdb".to_owned(),
                external_id: "591275".to_owned(),
            }
        );
        assert_eq!(group.records.len(), 2);
        assert!(group.records.keys().any(|key| {
            key.source == ContentSource::Plex.as_str() && key.source_id == "271"
        }));
        assert!(group.records.keys().any(|key| {
            key.source == ContentSource::Radarr.as_str() && key.source_id == "17"
        }));
    }

    #[test]
    fn leaves_ambiguous_title_year_fallbacks_unmerged() {
        let plex = crate::stack::parse_plex_library_items(include_str!("../fixtures/plex_library.xml"))
            .unwrap();
        let movies: Vec<RadarrMovie> =
            serde_json::from_str(include_str!("../fixtures/radarr_movies.json")).unwrap();
        let mut plex_record = ContentSyncRecord::from_plex(&plex[0]).unwrap();
        let radarr_record = ContentSyncRecord::from_radarr(&movies[0]).unwrap();
        let mut other_radarr_record = radarr_record.clone();
        other_radarr_record.key = super::ContentUpsertKey::new(ContentSource::Radarr, "18");
        other_radarr_record.external_id = Some("999999".to_owned());
        plex_record.title = "Fixture Movie".to_owned();
        plex_record.year = Some(2024);

        let batch = ContentSyncBatch::from_records([
            plex_record,
            radarr_record,
            other_radarr_record,
        ]);
        assert_eq!(batch.len(), 3);
        assert!(batch.groups().any(|group| {
            matches!(
                group.identity,
                ContentIdentityKey::TitleYear {
                    content_type: ContentType::Movie,
                    ..
                }
            ) && group.records.len() == 1
        }));
    }

    #[test]
    fn rejects_records_without_required_stack_identity() {
        let item = PlexLibraryItem {
            rating_key: None,
            title: Some("Untitled".to_owned()),
            year: None,
            item_type: Some("movie".to_owned()),
            genres: Vec::new(),
            raw_attributes: Vec::new(),
        };
        assert!(ContentSyncRecord::from_plex(&item).is_err());
    }
}
