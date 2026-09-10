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
    pub rpg_db_url: String,
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
            rpg_db_url: required("RPG_DB_URL")?,
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
    let value = value.trim();
    // F-27: a quoted value may contain '#' safely; an unquoted one ends at
    // a whitespace-separated ' # ' comment marker (dotenv convention), so
    // `KEY=value # comment` keeps the comment out of the value while
    // `KEY=abc#123` stays intact.
    let value = if value.starts_with(['\'', '"']) {
        value.trim_matches(['\'', '"'])
    } else {
        match value.find(" #") {
            Some(index) => value[..index].trim_end(),
            None => value,
        }
    };
    Some((name.trim().to_owned(), value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::ProbeConfig;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_required_values_from_env_file_and_strips_quotes() {
        let path = std::env::temp_dir().join(format!(
            "movie-rpg-config-{}-{}.env",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::write(
            &path,
            "PLEX_URL=http://plex\nPLEX_TOKEN=plex-token\nSONARR_URL=http://sonarr\nSONARR_API_KEY='sonarr-key'\nRADARR_URL=http://radarr\nRADARR_API_KEY=radarr-key\nTMDB_API_KEY=tmdb-key\nTVDB_API_KEY=tvdb-key\nOMDB_API_KEY=omdb-key\nFANART_API_KEY=fanart-key\nRPG_DB_URL=postgresql://rpg@localhost/movie_rpg\n",
        )
        .unwrap();

        let config = ProbeConfig::from_env_file(&path).unwrap();
        // Process environment wins over file values by contract, so the
        // expectation for each key is ambient-env-when-set, else the file
        // value. This keeps the test hermetic even when the runner itself
        // exports one of these variables (e.g. RPG_DB_URL in CI).
        let expected = |file_value: &str, name: &str| {
            std::env::var(name).ok().unwrap_or_else(|| file_value.to_owned())
        };
        assert_eq!(
            config.sonarr_api_key,
            expected("sonarr-key", "SONARR_API_KEY")
        );
        assert_eq!(
            config.fanart_api_key,
            expected("fanart-key", "FANART_API_KEY")
        );
        assert_eq!(
            config.rpg_db_url,
            expected("postgresql://rpg@localhost/movie_rpg", "RPG_DB_URL")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn parse_env_line_strips_quotes_and_ignores_comments() {
        assert_eq!(
            super::parse_env_line("SONARR_API_KEY='sonarr-key'"),
            Some(("SONARR_API_KEY".to_owned(), "sonarr-key".to_owned()))
        );
        assert_eq!(
            super::parse_env_line("FANART_API_KEY=\"fanart-key\""),
            Some(("FANART_API_KEY".to_owned(), "fanart-key".to_owned()))
        );
        assert_eq!(
            super::parse_env_line("export RPG_DB_URL=postgresql://db"),
            Some(("RPG_DB_URL".to_owned(), "postgresql://db".to_owned()))
        );
        assert_eq!(super::parse_env_line("# comment"), None);
        assert_eq!(super::parse_env_line(""), None);
        // F-27: inline comments strip from unquoted values, survive in
        // quoted ones.
        assert_eq!(
            super::parse_env_line("PLEX_URL=http://plex:32400 # the main server"),
            Some(("PLEX_URL".to_owned(), "http://plex:32400".to_owned()))
        );
        assert_eq!(
            super::parse_env_line("OMDB_API_KEY=abc#123"),
            Some(("OMDB_API_KEY".to_owned(), "abc#123".to_owned()))
        );
    }
}
