use crate::{ProbeError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexSection {
    pub key: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexLibraryItem {
    pub rating_key: Option<String>,
    /// For episode rows, the ratingKey of the owning show (§9.1 detection
    /// links an episode watch back to the synced series).
    pub parent_rating_key: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub item_type: Option<String>,
    pub genres: Vec<String>,
    pub raw_attributes: Vec<(String, String)>,
}

impl PlexLibraryItem {
    pub fn attr(&self, name: &str) -> Option<String> {
        self.raw_attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    /// The owning show's ratingKey for an episode row (F-2). Plex sets
    /// `grandparentRatingKey` on episodes (parent = season); older servers
    /// only set `parentRatingKey`. Prefer grandparent, fall back to parent.
    pub fn parent_show_rating_key(&self) -> Option<String> {
        self.attr("grandparentRatingKey")
            .or_else(|| self.attr("parentRatingKey"))
    }
}

pub fn parse_plex_sections(xml: &str) -> Result<Vec<PlexSection>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sections = Vec::new();

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Empty(event))
            | Ok(quick_xml::events::Event::Start(event))
                if event.name().as_ref() == b"Directory" =>
            {
                let attrs = attributes(&event)?;
                if attrs.iter().any(|(key, value)| {
                    key == "type" && (value == "movie" || value == "show")
                }) {
                    sections.push(PlexSection {
                        key: attr(&attrs, "key"),
                        title: attr(&attrs, "title"),
                        kind: attr(&attrs, "type"),
                        size: attr(&attrs, "size").and_then(|value| value.parse().ok()),
                    });
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ProbeError::Xml(error.to_string())),
        }
    }

    Ok(sections)
}

/// The top-level item tag of the library scan. A show section returns
/// show-level `Directory` rows whose seasons are also `Directory` children,
/// so the item-open state machine must track tag depth (F-4) instead of
/// closing on the first same-name End event.
const PLEX_ITEM_TAGS: [&[u8]; 2] = [b"Video", b"Directory"];

fn is_item_tag(name: &[u8]) -> bool {
    PLEX_ITEM_TAGS.contains(&name)
}

fn library_item_from_attrs(attrs: Vec<(String, String)>) -> PlexLibraryItem {
    PlexLibraryItem {
        rating_key: attr(&attrs, "ratingKey"),
        parent_rating_key: attr(&attrs, "parentRatingKey"),
        title: attr(&attrs, "title"),
        year: attr(&attrs, "year").and_then(|value| value.parse().ok()),
        item_type: attr(&attrs, "type"),
        genres: Vec::new(),
        raw_attributes: attrs,
    }
}

pub fn parse_plex_library_items(xml: &str) -> Result<Vec<PlexLibraryItem>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // A stack of open items (F-4/F-2): show sections nest episodes (and
    // seasons) inside the show's Directory, so items are captured at every
    // depth — an item closes only when its own End event pops it, and its
    // children were already completed and collected.
    let mut stack: Vec<PlexLibraryItem> = Vec::new();
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event))
                if is_item_tag(event.name().as_ref()) =>
            {
                stack.push(library_item_from_attrs(attributes(&event)?));
            }
            Ok(quick_xml::events::Event::Empty(event))
                if is_item_tag(event.name().as_ref()) =>
            {
                items.push(library_item_from_attrs(attributes(&event)?));
            }
            Ok(quick_xml::events::Event::Empty(event))
                if event.name().as_ref() == b"Genre" =>
            {
                if let Some(item) = stack.last_mut() {
                    let attrs = attributes(&event)?;
                    if let Some(tag) = attr(&attrs, "tag") {
                        item.genres.push(tag);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(event))
                if is_item_tag(event.name().as_ref()) =>
            {
                if let Some(item) = stack.pop() {
                    items.push(item);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ProbeError::Xml(error.to_string())),
        }
    }

    Ok(items)
}

/// The subset of a library scan that is a playable leaf (`type="movie"` or
/// `type="episode"`): exactly the rows the sync persists as content and the
/// watch-state detector matches against ratingKeys. Show/season rows are
/// filtered out here rather than deep in the sync pipeline (F-2).
pub fn parse_plex_watchable_items(xml: &str) -> Result<Vec<PlexLibraryItem>> {
    Ok(parse_plex_library_items(xml)?
        .into_iter()
        .filter(|item| {
            matches!(
                item.item_type.as_deref(),
                Some("movie") | Some("episode")
            )
        })
        .collect())
}

/// Counts leaf episodes in a parsed library scan (used by tests and the
/// sync summary; show/season rows never count).
pub fn parse_plex_show_items(items: &[PlexLibraryItem]) -> usize {
    items
        .iter()
        .filter(|item| item.item_type.as_deref() == Some("show"))
        .count()
}

fn attributes<'a>(event: &quick_xml::events::BytesStart<'a>) -> Result<Vec<(String, String)>> {
    event
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(|error| ProbeError::Xml(error.to_string()))?;
            let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
            let value = attribute
                .unescape_value()
                .map_err(|error| ProbeError::Xml(error.to_string()))?
                .into_owned();
            Ok((key, value))
        })
        .collect()
}

fn attr(attrs: &[(String, String)], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SonarrSeries {
    pub id: Option<i64>,
    #[serde(rename = "tmdbId")]
    pub tmdb_id: Option<i64>,
    #[serde(rename = "tvdbId")]
    pub tvdb_id: Option<i64>,
    pub title: Option<String>,
    pub year: Option<i32>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RadarrMovie {
    pub id: Option<i64>,
    #[serde(rename = "tmdbId")]
    pub tmdb_id: Option<i64>,
    #[serde(rename = "imdbId")]
    pub imdb_id: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

pub struct PlexClient {
    http: Client,
    base_url: String,
    token: String,
}

impl PlexClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::with_base_url(base_url, token)
    }

    pub fn with_base_url(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            // F-18: all clients share one process-wide connection pool with
            // uniform timeouts; the local handle is only configuration.
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token: token.into(),
        }
    }

    pub async fn sections(&self) -> Result<Vec<PlexSection>> {
        let response = crate::send_with_retry(
            "plex",
            self.http
                .get(format!("{}/library/sections", self.base_url))
                .header("X-Plex-Token", &self.token),
        )
        .await?;
        parse_plex_sections(&crate::text_limited(response).await?)
    }

    pub async fn library_items(&self, section_key: &str) -> Result<Vec<PlexLibraryItem>> {
        let response = crate::send_with_retry(
            "plex",
            self.http
                .get(format!(
                    "{}/library/sections/{}/all",
                    self.base_url, section_key
                ))
                .header("X-Plex-Token", &self.token),
        )
        .await?;
        parse_plex_library_items(&crate::text_limited(response).await?)
    }

    /// The playable rows (movies + episodes) of one library section (F-2).
    pub async fn watchable_items(&self, section_key: &str) -> Result<Vec<PlexLibraryItem>> {
        let response = crate::send_with_retry(
            "plex",
            self.http
                .get(format!(
                    "{}/library/sections/{}/all",
                    self.base_url, section_key
                ))
                .header("X-Plex-Token", &self.token),
        )
        .await?;
        parse_plex_watchable_items(&crate::text_limited(response).await?)
    }
}

pub struct SonarrClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl SonarrClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self::with_base_url(base_url, api_key)
    }

    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    pub async fn series(&self) -> Result<Vec<SonarrSeries>> {
        let response = crate::send_with_retry(
            "sonarr",
            self.http
                .get(format!("{}/api/v3/series", self.base_url))
                .query(&[("includeStatistics", "true")])
                .header("X-Api-Key", &self.api_key),
        )
        .await?;
        crate::json_limited(response).await
    }
}

pub struct RadarrClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl RadarrClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self::with_base_url(base_url, api_key)
    }

    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    pub async fn movies(&self) -> Result<Vec<RadarrMovie>> {
        let response = crate::send_with_retry(
            "radarr",
            self.http
                .get(format!("{}/api/v3/movie", self.base_url))
                .header("X-Api-Key", &self.api_key),
        )
        .await?;
        crate::json_limited(response).await
    }
}
