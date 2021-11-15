use std::sync::Arc;
use std::{collections::HashSet, ops::Add, time::Duration};

use async_trait::async_trait;
use chrono::NaiveDateTime;
use governor::{Jitter, Quota, RateLimiter};
use log::{debug, error, info};
use nonzero_ext::nonzero;
use parking_lot::RwLock;
use reqwest::{Client, StatusCode};
use serde;
use serde::{Deserialize, Serialize};
use strum_macros;
use strum_macros::{EnumIter, EnumString};
use thiserror::Error;

use crate::request::HttpError;
use crate::scheduler::UnscopedLimiter;

use super::{PageSize, ScrapeUrl};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderMediaType {
    Image,
    Video,
}

pub type SharedCredentials<T> = Arc<RwLock<Option<T>>>;

/// Placeholder for images that may contain more metadata in the future?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMedia {
    #[serde(rename = "type")]
    pub _type: ProviderMediaType,
    pub media_url: String,
    // where the image is coming from
    pub reference_url: Option<String>,
    pub unique_identifier: String,
    /// necessary for some providers like weverse which include additional
    /// metadata that are unique to the provider being scraped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPost {
    pub unique_identifier: String,
    pub images: Vec<ProviderMedia>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub post_date: Option<NaiveDateTime>,
    /// necessary for some providers like weverse which include additional
    /// metadata that are unique to the provider being scraped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct ProviderResult {
    pub posts: Vec<ProviderPost>,
    pub response_delay: Duration,
    pub response_code: StatusCode,
}

impl Add<ProviderResult> for ProviderResult {
    type Output = ProviderResult;
    fn add(self, rhs: ProviderResult) -> Self::Output {
        ProviderResult {
            response_code: rhs.response_code,
            response_delay: rhs.response_delay,
            posts: [self.posts, rhs.posts].concat(),
        }
    }
}

#[derive(Debug)]
pub enum ProviderStep {
    Next(ProviderResult, Pagination),
    End(ProviderResult),
    // Provider exits gracefully
    NotInitialized,
}

#[derive(Error, Debug)]
pub enum ProviderFailure {
    #[error("Error formatting URL")]
    Url,
    #[error("Failed to process response from request")]
    HttpError(HttpError),
    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for ProviderFailure {
    fn from(err: reqwest::Error) -> Self {
        ProviderFailure::HttpError(HttpError::ReqwestError(err))
    }
}

#[derive(Debug, Clone)]
pub struct ProviderState {
    pub id: String,
    pub url: ScrapeUrl,
    pub pagination: Option<Pagination>,
    pub iteration: usize,
}

pub struct ScrapeRequestInput {
    pub latest_data: HashSet<String>,
    pub last_scrape: Option<NaiveDateTime>,
}

impl From<HttpError> for ProviderFailure {
    fn from(err: HttpError) -> Self {
        Self::HttpError(err)
    }
}

pub enum CanonicalUrlResolution {
    Success(String),
    Fail,
    NotImplemented,
}

pub enum CredentialRefresh {
    Result(ProviderCredentials),
    TryLogin,
    Halt,
}

pub enum ProviderErrorHandle {
    RefreshToken(ProviderCredentials),
    Login,
    Halt,
}

#[derive(Debug, Clone)]
pub enum Pagination {
    NextPage(i32),
    NextCursor(String),
}

impl Pagination {
    pub fn next_page(&self) -> String {
        match self {
            Pagination::NextPage(num) => num.to_string(),
            Pagination::NextCursor(cursor) => cursor.clone(),
        }
    }
}

impl ToString for Pagination {
    fn to_string(&self) -> String {
        self.next_page()
    }
}

#[async_trait]
pub trait RateLimitable {
    /// The available quota for this provider
    fn quota() -> Quota
    where
        Self: Sized,
    {
        default_quota()
    }
    /// The default rate limiter implementation
    /// This currently only supports global rate limiters
    /// but may need to be changed to support local ones as well
    fn rate_limiter() -> UnscopedLimiter
    where
        Self: Sized,
    {
        RateLimiter::direct(Self::quota())
    }
    /// Wait for next request if token is not available
    async fn wait(&self, key: &str) -> ();
}

pub fn default_quota() -> Quota {
    // fairly aggressive quota
    Quota::with_period(Duration::from_millis(3500u64))
        .unwrap()
        .allow_burst(nonzero!(4u32))
}

pub fn default_jitter() -> Jitter {
    Jitter::up_to(Duration::from_secs(2))
}

#[derive(Debug, Clone, Default)]
pub struct ProviderCredentials {
    pub access_token: String,
    pub refresh_token: String,
}

pub struct BareProviderInput {
    pub client: Arc<Client>,
}

pub struct ProviderInput {
    pub client: Arc<Client>,
}

pub fn create_credentials<T>() -> Arc<RwLock<Option<T>>> {
    Arc::new(RwLock::new(None))
}

/// Try to override the shared credentials after logging in one time
pub async fn attempt_first_login(
    provider: &dyn Provider,
    credentials: &SharedCredentials<ProviderCredentials>,
) -> () {
    let id = provider.id().to_string();
    info!("Attempting login to {}", &id);
    let login = provider.login().await;
    let provider_creds = match login {
        Ok(login) => {
            info!("Logged in into {}", &id);
            login
        }
        Err(err) => {
            error!("Could not log into {}, leaving it uninitialized", &id);
            eprintln!("{:?}", err);
            return;
        }
    };
    let mut writable = credentials.write();
    *writable = Some(provider_creds);
}

/// Providers represent a generic endpoint on a single platform that can be scraped
/// with a unique identifier for each specific resource
#[async_trait]
pub trait Provider: Sync + Send + RateLimitable {
    fn new(input: ProviderInput) -> Self
    where
        Self: Sized;
    async fn initialize(&self) -> () {
        ()
    }

    fn requires_auth(&self) -> bool {
        false
    }

    /// a string that uniquely identifies this provider
    fn id(&self) -> AllProviders;

    /// The page size that should be used when scraping
    /// Destinations that haven't been scraped before should be using a larger
    /// page size.
    /// iteration is 0 indexed
    fn next_page_size(&self, last_scraped: Option<NaiveDateTime>, iteration: usize) -> PageSize;

    /// The maximum number of times a resource can be paginated before exiting.
    /// This value is ignored if the context has no images aka the resource
    /// is being scraped for the first time
    fn max_pagination(&self) -> u16 {
        5
    }

    /// The amount of delay between each pagination request. Initial request is not
    /// bound by this value
    fn scrape_delay(&self) -> Duration {
        Duration::from_secs(2)
    }

    /// Attempt to resolve the data required to construct a scrape destination given a canonical URL
    /// # Example
    /// Canonical URL: https://weverse.io/dreamcatcher/artist
    /// Result:        Success("14")
    async fn canonical_url_to_id(&self, _url: &str) -> CanonicalUrlResolution {
        CanonicalUrlResolution::NotImplemented
    }

    /// Provider destination are any unique identifier a provider can try to resolve into an opaque [ScrapeUrl].
    /// This method is called after every successful scrape to resolve the next page of media
    fn from_provider_destination(
        &self,
        id: &str,
        page_size: PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure>;
    /// Process a single iteration of the resource
    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure>;

    /// Error handling branch that separates operational errors from authorization
    /// related error codes
    fn on_error(&self, _http_error: &HttpError) -> anyhow::Result<ProviderErrorHandle> {
        debug!(
            "{} ran into an unhandled error and is halting",
            self.id().to_string()
        );
        Ok(ProviderErrorHandle::Halt)
    }

    async fn token_refresh(
        &self,
        _credentials: &ProviderCredentials,
    ) -> anyhow::Result<CredentialRefresh> {
        panic!(
            "{}'s on_error branch tried to refresh credentials but it doesn't implement a token refresh flow",
            self.id().to_string()
        )
    }

    async fn login(&self) -> anyhow::Result<ProviderCredentials> {
        panic!(
            "{} tried to login but it doesn't implement a login flow",
            self.id().to_string()
        )
    }
    fn credentials(&self) -> SharedCredentials<ProviderCredentials> {
        panic!(
            "Tried to get credentials for {} which doesn't authorization",
            self.id().to_string()
        )
    }

    /// Whether the URLs generated by this scraper expire after a short amount of duration
    fn ephemeral(&self) -> bool {
        false
    }
}

#[derive(
    Debug, Hash, Copy, Clone, Serialize, EnumString, EnumIter, strum_macros::ToString, PartialEq, Eq,
)]
pub enum AllProviders {
    #[strum(serialize = "pinterest.board_feed")]
    PinterestBoardFeed,
    #[strum(serialize = "weverse.artist_feed")]
    WeverseArtistFeed,
    #[strum(serialize = "united_cube.artist_feed")]
    UnitedCubeArtistFeed,
    #[strum(serialize = "twitter.timeline")]
    TwitterTimeline,
}

pub struct UrlBuilder {
    pub params: Vec<(&'static str, String)>,
}

impl Default for UrlBuilder {
    fn default() -> Self {
        Self { params: vec![] }
    }
}

impl ToString for UrlBuilder {
    fn to_string(&self) -> String {
        todo!()
    }
}

impl UrlBuilder {
    pub fn from_queries(params: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            params: params
                .into_iter()
                .map(|(key, value)| (key, value.to_owned()))
                .collect::<Vec<_>>(),
        }
    }
    pub fn page_size(&mut self, key: &'static str, page_size: PageSize) -> &mut Self {
        self.params.push((key, page_size.0.to_string()));
        self
    }
    pub fn pagination(&mut self, key: &'static str, page_option: &Option<Pagination>) -> &mut Self {
        if let Some(page) = page_option {
            self.params.push((key, page.next_page()))
        }
        self
    }
    pub fn build(&self, base_url: &str) -> Result<url::Url, ProviderFailure> {
        Ok(url::Url::parse_with_params(base_url, self.params.iter())
            .ok()
            .ok_or(ProviderFailure::Url)?)
    }
    pub fn build_scrape_url(self, base_url: &str) -> Result<ScrapeUrl, ProviderFailure> {
        let res = self.build(base_url)?;
        Ok(ScrapeUrl(res.as_str().to_owned()))
    }
}
