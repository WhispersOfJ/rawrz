use crate::{ProbeError, Result};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProbeConfig {
    pub plex_url: String,
    pub plex_token: String,
    pub sonarr_url: String,
    pub sonarr_api_key: String,
    pub radarr_url: String,
    pub radarr_api_key: String,
    pub tmdb_api_key: String,
    pub tvdb_api_key: String,
    pub omdb_api_key: String,
    pub fanart_api_key: String,
}

impl ProbeConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_values(|name| std::env::var(name).ok())
    }

    /// Load the shared Bear Cave `.env` without printing or persisting secret values.
    /// Existing process environment variables take precedence over file values.
    pub fn from_env_file(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|error| ProbeError::MissingEnvironment(error.to_string()))?;
        let file_values = contents
            .lines()
            .filter_map(parse_env_line)
            .collect::<HashMap<_, _>>();
        Self::from_values(|name| std::env::var(name).ok().or_else(|| file_values.get(name).cloned()))
    }

    fn from_values(mut get: impl FnMut(&str) -> Option<String>) -> Result<Self> {        let mut required =
            |name: &str| get(name).ok_or_else(|| ProbeError::MissingEnvironment(name.to_owned()));
        Ok(Self {
            plex_url: required("PLEX_URL")?,
            plex_token: required("PLEX_TOKEN")?,
            sonarr_url: required("SONARR_URL")?,
            sonarr_api_key: required("SONARR_API_KEY")?,
            radarr_url: required("RADARR_URL")?,
            radarr_api_key: required("RADARR_API_KEY")?,
            tmdb_api_key: required("TMDB_API_KEY")?,
            tvdb_api_key: required("TVDB_API_KEY")?,
            omdb_api_key: required("OMDB_API_KEY")?,
            fanart_api_key: required("FANART_API_KEY")?,
        })
    }
}

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let line = line.strip_prefix("export ").unwrap_or(line);
    let (name, value) = line.split_once('=')?;
    let value = value.trim().trim_matches(['\'', '"']);
    Some((name.trim().to_owned(), value.to_owned()))
}
