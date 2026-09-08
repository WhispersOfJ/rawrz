use crate::providers::{FanartPayload, OmdbResponse, TmdbDetails};
use crate::stack::{PlexLibraryItem, RadarrMovie, SonarrSeries};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NormalizedTag {
    pub name: String,
    pub slug: String,
    pub parent_genre: Option<String>,
    pub provider: String,
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderRating {
    pub provider: String,
    pub value: f64,
    pub scale: f64,
    pub votes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Artwork {
    pub kind: String,
    pub url: String,
    pub provider: String,
    pub language: Option<String>,
    pub likes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeaturedScore {
    pub provider: String,
    pub value: f64,
    pub scale: f64,
    pub votes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NormalizedMetadata {
    pub title: Option<String>,
    pub year: Option<i32>,
    pub tmdb_id: Option<i64>,
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub genres: Vec<NormalizedTag>,
    pub sub_genres: Vec<NormalizedTag>,
    pub ratings: Vec<ProviderRating>,
    pub featured_score: Option<FeaturedScore>,
    pub artwork: Vec<Artwork>,
    pub provenance: BTreeMap<String, Vec<String>>,
}

impl NormalizedMetadata {
    pub fn from_sources(
        plex: Option<&PlexLibraryItem>,
        sonarr: Option<&SonarrSeries>,
        radarr: Option<&RadarrMovie>,
        tmdb: Option<&TmdbDetails>,
        omdb: Option<&OmdbResponse>,
        tvdb: Option<&Value>,
        fanart: Option<&FanartPayload>,
        keyword_parent_map: &BTreeMap<String, String>,
    ) -> Self {
        let tmdb_id = tmdb
            .map(|details| details.id)
            .or_else(|| radarr.and_then(|movie| movie.tmdb_id))
            .or_else(|| sonarr.and_then(|series| series.tmdb_id));
        let tvdb_id = sonarr
            .and_then(|series| series.tvdb_id)
            .or_else(|| json_i64(tvdb, &["data", "id"]));
        let imdb_id = tmdb
            .and_then(|details| details.imdb_id.clone())
            .or_else(|| radarr.and_then(|movie| movie.imdb_id.clone()))
            .or_else(|| omdb.and_then(|details| details.imdb_id.clone()));

        let title = tmdb
            .and_then(|details| details.title.clone().or_else(|| details.name.clone()))
            .or_else(|| omdb.and_then(|details| details.title.clone()))
            .or_else(|| radarr.and_then(|movie| movie.title.clone()))
            .or_else(|| sonarr.and_then(|series| series.title.clone()))
            .or_else(|| plex.and_then(|item| item.title.clone()));
        let year = omdb
            .and_then(|details| details.year.as_deref().and_then(parse_year))
            .or_else(|| radarr.and_then(|movie| movie.year))
            .or_else(|| sonarr.and_then(|series| series.year))
            .or_else(|| plex.and_then(|item| item.year));

        let mut normalized = Self {
            title,
            year,
            tmdb_id,
            tvdb_id,
            imdb_id,
            genres: Vec::new(),
            sub_genres: Vec::new(),
            ratings: Vec::new(),
            featured_score: None,
            artwork: Vec::new(),
            provenance: BTreeMap::new(),
        };

        if let Some(item) = plex {
            for genre in &item.genres {
                Self::push_tag(
                    &mut normalized.genres,
                    &mut normalized.provenance,
                    NormalizedTag::new(genre, genre, "plex", None),
                );
            }
        }
        if let Some(details) = tmdb {
            for genre in &details.genres {
                Self::push_tag(
                    &mut normalized.genres,
                    &mut normalized.provenance,
                    NormalizedTag::new(&genre.name, &genre.name, "tmdb", Some(genre.id.to_string())),
                );
            }
            for keyword in details.keywords() {
                let keyword_slug = slug(&keyword.name);
                if let Some(parent) = keyword_parent_map.get(&keyword_slug) {
                    Self::push_tag(
                        &mut normalized.sub_genres,
                        &mut normalized.provenance,
                        NormalizedTag {
                            name: keyword.name.clone(),
                            slug: keyword_slug,
                            parent_genre: Some(parent.clone()),
                            provider: "tmdb".to_owned(),
                            provider_id: Some(keyword.id.to_string()),
                        },
                    );
                }
            }
        }
        if let Some(value) = tvdb {
            for (name, id) in json_tags(value, &["data", "genres"]) {
                Self::push_tag(
                    &mut normalized.genres,
                    &mut normalized.provenance,
                    NormalizedTag::new(&name, &name, "tvdb", id.map(|value| value.to_string())),
                );
            }
            for (name, id) in json_tags(value, &["data", "tags"]) {
                let tag_slug = slug(&name);
                if let Some(parent) = keyword_parent_map.get(&tag_slug) {
                    Self::push_tag(
                        &mut normalized.sub_genres,
                        &mut normalized.provenance,
                        NormalizedTag {
                            name,
                            slug: tag_slug,
                            parent_genre: Some(parent.clone()),
                            provider: "tvdb".to_owned(),
                            provider_id: id.map(|value| value.to_string()),
                        },
                    );
                }
            }
        }

        if let Some(details) = tmdb {
            if let Some(value) = details.vote_average {
                normalized.push_rating("tmdb", value, 10.0, details.vote_count);
            }
        }
        if let Some(details) = omdb {
            if let Some(value) = parse_rating(details.imdb_rating.as_deref()) {
                normalized.push_rating("omdb_imdb", value, 10.0, parse_votes(details.imdb_votes.as_deref()));
            }
            if let Some(value) = details.rotten_tomatoes_rating().and_then(|value| parse_rating(Some(&value))) {
                normalized.push_rating("omdb_rotten_tomatoes", value, 10.0, None);
            }
        }
        if let Some(value) = json_f64(tvdb, &["data", "score"]) {
            normalized.push_rating("tvdb", value, 10.0, json_i64(tvdb, &["data", "scoreVotes"]));
        }
        if let Some(movie) = radarr {
            for (provider, key) in [("radarr_tmdb", "tmdb"), ("radarr_imdb", "imdb")] {
                if let Some(value) = movie.raw.get("ratings").and_then(|ratings| ratings.get(key)).and_then(|rating| rating.get("value")).and_then(Value::as_f64) {
                    let votes = movie.raw.get("ratings").and_then(|ratings| ratings.get(key)).and_then(|rating| rating.get("votes")).and_then(Value::as_i64);
                    normalized.push_rating(provider, value, 10.0, votes);
                }
            }
        }
        if let Some(item) = plex {
            if let Some(value) = attr_f64(&item.raw_attributes, "audienceRating").or_else(|| attr_f64(&item.raw_attributes, "rating")) {
                normalized.push_rating("plex", value, 10.0, None);
            }
        }

        if let Some(payload) = fanart {
            for (kind, images) in &payload.images {
                for image in images {
                    if let Some(url) = &image.url {
                        normalized.artwork.push(Artwork {
                            kind: kind.clone(),
                            url: url.clone(),
                            provider: "fanart".to_owned(),
                            language: image.lang.clone(),
                            likes: image.likes.as_deref().and_then(|likes| likes.parse().ok()),
                        });
                    }
                }
            }
            normalized.artwork.sort_by(|left, right| {
                (&left.kind, &left.url).cmp(&(&right.kind, &right.url))
            });
        }

        normalized.featured_score = normalized.ratings.iter().find_map(|rating| {
            ["tmdb", "omdb_imdb", "tvdb", "radarr_tmdb", "radarr_imdb", "plex"]
                .contains(&rating.provider.as_str())
                .then(|| FeaturedScore {
                    provider: rating.provider.clone(),
                    value: rating.value,
                    scale: rating.scale,
                    votes: rating.votes,
                })
        });
        normalized
    }

    fn push_tag(
        tags: &mut Vec<NormalizedTag>,
        provenance: &mut BTreeMap<String, Vec<String>>,
        tag: NormalizedTag,
    ) {
        let key = tag.slug.clone();
        if let Some(existing) = tags.iter_mut().find(|existing| existing.slug == key) {
            let existing_provider = existing.provider.clone();
            let providers = provenance.entry(existing.slug.clone()).or_default();
            if !providers.contains(&tag.provider) {
                providers.push(tag.provider.clone());
            }
            if provider_rank(&tag.provider) > provider_rank(&existing_provider) {
                *existing = tag;
            }
            return;
        }
        provenance
            .entry(tag.slug.clone())
            .or_default()
            .push(tag.provider.clone());
        tags.push(tag);
        tags.sort_by(|left, right| left.slug.cmp(&right.slug));
    }

    fn push_rating(&mut self, provider: &str, value: f64, scale: f64, votes: Option<i64>) {
        if !value.is_finite() || value < 0.0 || value > scale {
            return;
        }
        if self.ratings.iter().any(|rating| rating.provider == provider) {
            return;
        }
        self.ratings.push(ProviderRating {
            provider: provider.to_owned(),
            value,
            scale,
            votes,
        });
    }
}

impl NormalizedTag {
    fn new(name: &str, slug_source: &str, provider: &str, provider_id: Option<String>) -> Self {
        Self {
            name: name.to_owned(),
            slug: slug(slug_source),
            parent_genre: None,
            provider: provider.to_owned(),
            provider_id,
        }
    }
}

fn slug(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn provider_rank(provider: &str) -> u8 {
    match provider {
        "tmdb" => 3,
        "tvdb" => 2,
        "plex" => 1,
        _ => 0,
    }
}

fn parse_year(value: &str) -> Option<i32> {
    value.chars().take(4).collect::<String>().parse().ok()
}

fn parse_rating(value: Option<&str>) -> Option<f64> {
    let value = value?.trim();
    let is_percent = value.ends_with('%');
    let parsed = value.trim_end_matches('%').parse::<f64>().ok()?;
    Some(if is_percent { parsed / 10.0 } else { parsed })
}

fn parse_votes(value: Option<&str>) -> Option<i64> {
    value?.replace(',', "").parse().ok()
}

fn attr_f64(attributes: &[(String, String)], key: &str) -> Option<f64> {
    attributes.iter().find(|(name, _)| name == key)?.1.parse().ok()
}

fn json_f64(value: Option<&Value>, path: &[&str]) -> Option<f64> {
    json_value(value, path)?.as_f64()
}

fn json_i64(value: Option<&Value>, path: &[&str]) -> Option<i64> {
    json_value(value, path)?.as_i64()
}

fn json_value<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value?, |current, key| current.get(*key))
}

fn json_tags(value: &Value, path: &[&str]) -> Vec<(String, Option<i64>)> {
    let Some(items) = json_value(Some(value), path).and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(name) = item.as_str() {
                return Some((name.to_owned(), None));
            }
            let name = item.get("name").and_then(Value::as_str)?.to_owned();
            Some((name, item.get("id").and_then(Value::as_i64)))
        })
        .collect()
}
