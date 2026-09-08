use crate::{ProbeError, Result};
use reqwest::Client;
use serde::Deserialize;
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
    pub title: Option<String>,
    pub year: Option<i32>,
    pub item_type: Option<String>,
    pub genres: Vec<String>,
    pub raw_attributes: Vec<(String, String)>,
}

pub fn parse_plex_sections(xml: &str) -> Result<Vec<PlexSection>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sections = Vec::new();

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Empty(event)) | Ok(quick_xml::events::Event::Start(event))
                if event.name().as_ref() == b"Directory" =>
            {
                let attrs = attributes(&event)?;
                if attrs.iter().any(|(key, value)| key == "type" && (value == "movie" || value == "show")) {
                    sections.push(PlexSection {
                        key: attr(&attrs, "key"),
                        title: attr(&attrs, "title"),
                        kind: attr(&attrs, "type"),
                        size: attr(&attrs, "size").and_then(|v| v.parse().ok()),
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

pub fn parse_plex_library_items(xml: &str) -> Result<Vec<PlexLibraryItem>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut current: Option<PlexLibraryItem> = None;
    let mut current_tag: Option<Vec<u8>> = None;
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event))
                if event.name().as_ref() == b"Video" || event.name().as_ref() == b"Directory" =>
            {
                if current.is_none() {
                    let attrs = attributes(&event)?;
                    current = Some(PlexLibraryItem {
                        rating_key: attr(&attrs, "ratingKey"),
                        title: attr(&attrs, "title"),
                        year: attr(&attrs, "year").and_then(|v| v.parse().ok()),
                        item_type: attr(&attrs, "type"),
                        genres: Vec::new(),
                        raw_attributes: attrs,
                    });
                    current_tag = Some(event.name().as_ref().to_vec());
                }
            }
            Ok(quick_xml::events::Event::Empty(event))
                if event.name().as_ref() == b"Video" || event.name().as_ref() == b"Directory" =>
            {
                let attrs = attributes(&event)?;
                if current.is_none() {
                    items.push(PlexLibraryItem {
                        rating_key: attr(&attrs, "ratingKey"),
                        title: attr(&attrs, "title"),
                        year: attr(&attrs, "year").and_then(|v| v.parse().ok()),
                        item_type: attr(&attrs, "type"),
                        genres: Vec::new(),
                        raw_attributes: attrs,
                    });
                }
            }
            Ok(quick_xml::events::Event::Empty(event)) if event.name().as_ref() == b"Genre" => {
                if let Some(item) = current.as_mut() {
                    let attrs = attributes(&event)?;
                    if let Some(tag) = attr(&attrs, "tag") {
                        item.genres.push(tag);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(event)) => {
                if current_tag.as_deref() == Some(event.name().as_ref()) {
                    if let Some(item) = current.take() {
                        items.push(item);
                    }
                    current_tag = None;
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ProbeError::Xml(error.to_string())),
        }
    }

    Ok(items)
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
    attrs.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone())
}

#[derive(Debug, Deserialize)]
pub struct SonarrSeries {
    #[serde(rename = "tmdbId")]
    pub tmdb_id: Option<i64>,
    #[serde(rename = "tvdbId")]
    pub tvdb_id: Option<i64>,
    pub title: Option<String>,
    pub year: Option<i32>,
    #[serde(flatten)]
    pub raw: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct RadarrMovie {
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
        Self { http: Client::new(), base_url: base_url.into().trim_end_matches('/').to_owned(), token: token.into() }
    }

    pub async fn sections(&self) -> Result<Vec<PlexSection>> {
        let response = self.http.get(format!("{}/library/sections", self.base_url)).header("X-Plex-Token", &self.token).send().await?;
        ensure_success("plex", &response)?;
        parse_plex_sections(&response.text().await?)
    }

    pub async fn library_items(&self, section_key: &str) -> Result<Vec<PlexLibraryItem>> {
        let response = self.http.get(format!("{}/library/sections/{}/all", self.base_url, section_key)).header("X-Plex-Token", &self.token).send().await?;
        ensure_success("plex", &response)?;
        parse_plex_library_items(&response.text().await?)
    }
}

pub struct SonarrClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl SonarrClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self { http: Client::new(), base_url: base_url.into().trim_end_matches('/').to_owned(), api_key: api_key.into() }
    }

    pub async fn series(&self) -> Result<Vec<SonarrSeries>> {
        let response = self.http.get(format!("{}/api/v3/series", self.base_url)).query(&[("includeStatistics", "true")]).header("X-Api-Key", &self.api_key).send().await?;
        ensure_success("sonarr", &response)?;
        Ok(response.json().await?)
    }
}

pub struct RadarrClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl RadarrClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self { http: Client::new(), base_url: base_url.into().trim_end_matches('/').to_owned(), api_key: api_key.into() }
    }

    pub async fn movies(&self) -> Result<Vec<RadarrMovie>> {
        let response = self.http.get(format!("{}/api/v3/movie", self.base_url)).header("X-Api-Key", &self.api_key).send().await?;
        ensure_success("radarr", &response)?;
        Ok(response.json().await?)
    }
}

fn ensure_success(provider: &'static str, response: &reqwest::Response) -> Result<()> {
    if response.status().is_success() { Ok(()) } else { Err(ProbeError::HttpStatus { provider, status: response.status().as_u16() }) }
}
