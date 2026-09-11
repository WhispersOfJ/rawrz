use crate::{ProbeError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbGenre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbKeyword {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbDetails {
    pub id: i64,
    pub title: Option<String>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<TmdbGenre>,
    #[serde(default)]
    pub keywords: TmdbKeywords,
    #[serde(rename = "vote_average")]
    pub vote_average: Option<f64>,
    #[serde(rename = "vote_count")]
    pub vote_count: Option<i64>,
    #[serde(rename = "imdb_id")]
    pub imdb_id: Option<String>,
    #[serde(flatten)]
    pub raw: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TmdbKeywords {
    #[serde(default)]
    pub keywords: Vec<TmdbKeyword>,
    #[serde(default)]
    pub results: Vec<TmdbKeyword>,
}

impl TmdbDetails {
    pub fn keywords(&self) -> &[TmdbKeyword] {
        if self.keywords.keywords.is_empty() {
            &self.keywords.results
        } else {
            &self.keywords.keywords
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmdbRating {
    #[serde(rename = "Source")]
    pub source: String,
    #[serde(rename = "Value")]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmdbResponse {
    #[serde(rename = "Title")]
    pub title: Option<String>,
    #[serde(rename = "Year")]
    pub year: Option<String>,
    #[serde(rename = "imdbID")]
    pub imdb_id: Option<String>,
    #[serde(rename = "imdbRating")]
    pub imdb_rating: Option<String>,
    #[serde(rename = "imdbVotes")]
    pub imdb_votes: Option<String>,
    #[serde(rename = "Ratings", default)]
    pub ratings: Vec<OmdbRating>,
    #[serde(rename = "Response")]
    pub response: Option<String>,
    #[serde(rename = "Error")]
    pub error: Option<String>,
    #[serde(flatten)]
    pub raw: BTreeMap<String, Value>,
}

impl OmdbResponse {
    pub fn rotten_tomatoes_rating(&self) -> Option<String> {
        self.ratings
            .iter()
            .find(|rating| rating.source == "Rotten Tomatoes")
            .map(|rating| rating.value.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanartImage {
    pub url: Option<String>,
    pub lang: Option<String>,
    pub likes: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FanartPayload {
    #[serde(flatten)]
    pub images: BTreeMap<String, Vec<FanartImage>>,
}

impl FanartPayload {
    pub fn images_for(&self, kind: &str) -> &[FanartImage] {
        self.images.get(kind).map(Vec::as_slice).unwrap_or(&[])
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TvdbLogin {
    pub token: String,
}

#[derive(Debug, Serialize)]
struct TvdbLoginRequest<'a> {
    apikey: &'a str,
}

pub fn parse_tmdb_details(json: &str) -> Result<TmdbDetails> {
    Ok(serde_json::from_str(json)?)
}

pub fn parse_omdb_response(json: &str) -> Result<OmdbResponse> {
    Ok(serde_json::from_str(json)?)
}

pub fn parse_fanart_payload(json: &str) -> Result<FanartPayload> {
    Ok(serde_json::from_str(json)?)
}

pub fn parse_tvdb_login(json: &str) -> Result<TvdbLogin> {
    #[derive(Deserialize)]
    struct Envelope {
        data: TvdbLogin,
    }

    Ok(serde_json::from_str::<Envelope>(json)?.data)
}

pub struct TmdbClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl TmdbClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url("https://api.themoviedb.org/3", api_key)
    }

    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            // F-18: one shared connection pool across all provider clients.
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    pub async fn movie(&self, id: i64) -> Result<TmdbDetails> {
        self.details("movie", id).await
    }

    pub async fn tv(&self, id: i64) -> Result<TmdbDetails> {
        self.details("tv", id).await
    }

    async fn details(&self, kind: &str, id: i64) -> Result<TmdbDetails> {
        let response = crate::send_with_retry(
            "tmdb",
            self.http
                .get(format!("{}/{}/{}", self.base_url, kind, id))
                .query(&[
                    ("api_key", self.api_key.as_str()),
                    ("append_to_response", "keywords,credits,images"),
                ]),
        )
        .await?;
        crate::json_limited(response).await
    }
}

pub struct OmdbClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl OmdbClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url("https://www.omdbapi.com/", api_key)
    }

    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    pub async fn by_imdb_id(&self, imdb_id: &str) -> Result<OmdbResponse> {
        self.lookup(&[("i", imdb_id)]).await
    }

    pub async fn by_title_year(&self, title: &str, year: Option<i32>) -> Result<OmdbResponse> {
        let year = year.map(|value| value.to_string());
        let mut params = vec![("t", title.to_owned())];
        if let Some(year) = year {
            params.push(("y", year));
        }
        let refs = params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        self.lookup(&refs).await
    }

    async fn lookup(&self, extra: &[(&str, &str)]) -> Result<OmdbResponse> {
        let mut params = vec![("apikey", self.api_key.as_str()), ("plot", "full")];
        params.extend(extra.iter().copied());
        let response = crate::send_with_retry(
            "omdb",
            self.http.get(&self.base_url).query(&params),
        )
        .await?;
        let parsed: OmdbResponse = crate::json_limited(response).await?;
        if parsed.response.as_deref() == Some("False") {
            return Err(ProbeError::HttpStatus {
                provider: "omdb",
                status: 404,
            });
        }
        Ok(parsed)
    }
}

pub struct FanartClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl FanartClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url("https://webservice.fanart.tv/v3", api_key)
    }

    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    pub async fn movie(&self, tmdb_id: i64) -> Result<FanartPayload> {
        self.lookup("movies", tmdb_id).await
    }

    pub async fn tv(&self, tvdb_id: i64) -> Result<FanartPayload> {
        self.lookup("tv", tvdb_id).await
    }

    async fn lookup(&self, kind: &str, id: i64) -> Result<FanartPayload> {
        let response = crate::send_with_retry(
            "fanart",
            self.http
                .get(format!("{}/{}/{}", self.base_url, kind, id))
                .header("api-key", &self.api_key),
        )
        .await?;
        crate::json_limited(response).await
    }
}

pub struct TvdbClient {
    http: Client,
    base_url: String,
    api_key: String,
    token: Arc<Mutex<Option<String>>>,
}

impl TvdbClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url("https://api4.thetvdb.com/v4", api_key)
    }

    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: crate::shared_http_client(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            token: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn ensure_login(&self) -> Result<()> {
        if self
            .token
            .lock()
            .expect("TVDB token mutex poisoned")
            .is_some()
        {
            return Ok(());
        }
        self.login().await.map(|_| ())
    }

    pub async fn login(&self) -> Result<String> {
        let response = crate::send_with_retry(
            "tvdb",
            self.http
                .post(format!("{}/login", self.base_url))
                .json(&TvdbLoginRequest {
                    apikey: &self.api_key,
                }),
        )
        .await?;
        let token = crate::json_limited::<serde_json::Value>(response)
            .await?
            .get("data")
            .and_then(|data| data.get("token"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ProbeError::Xml("TVDB login response did not contain data.token".into())
            })?
            .to_owned();
        *self.token.lock().expect("TVDB token mutex poisoned") = Some(token.clone());
        Ok(token)
    }

    /// One series lookup with a token-expiry recovery path (F-5): TVDB v4
    /// tokens expire (roughly monthly), so a 401 clears the cached token,
    /// re-logins, and retries exactly once.
    pub async fn series(&self, id: i64) -> Result<Value> {
        self.series_with_refresh(id, true).await
    }

    async fn series_with_refresh(&self, id: i64, allow_refresh: bool) -> Result<Value> {
        self.ensure_login().await?;
        let token = self
            .token
            .lock()
            .expect("TVDB token mutex poisoned")
            .clone()
            .ok_or_else(|| {
                ProbeError::MissingEnvironment("TVDB runtime token (call login first)".into())
            })?;
        // A 401 is non-retryable and comes back as an Err from the shared
        // send path — that is the token-expiry signal (F-5).
        match crate::send_with_retry(
            "tvdb",
            self.http
                .get(format!("{}/series/{}", self.base_url, id))
                .bearer_auth(token),
        )
        .await
        {
            Err(ProbeError::HttpStatus {
                provider: "tvdb",
                status: 401,
            }) if allow_refresh => {
                *self.token.lock().expect("TVDB token mutex poisoned") = None;
                // Box::pin breaks the async-fn recursion cycle for the single
                // no-refresh retry; allow_refresh=false stops it from looping.
                Box::pin(self.series_with_refresh(id, false)).await
            }
            outcome => {
                let response = outcome?;
                crate::json_limited(response).await
            }
        }
    }
}
