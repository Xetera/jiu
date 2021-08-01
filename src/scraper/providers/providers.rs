use super::{GlobalProviderLimiter, PageSize, ScrapeUrl};
use crate::request::HttpError;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use governor::{Jitter, Quota, RateLimiter};
use log::{debug, error};
use nonzero_ext::nonzero;
use parking_lot::RwLock;
use reqwest::{Client, StatusCode};
use serde;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{collections::HashSet, ops::Add, time::Duration};
use strum_macros;
use strum_macros::{EnumIter, EnumString};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderMediaType {
    Image,
    Video,
}

/// Placeholder for images that may contain more metadata in the future?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMedia {
    #[serde(rename = "type")]
    pub _type: ProviderMediaType,
    pub media_url: String,
    pub page_url: Option<String>,
    pub post_date: Option<NaiveDateTime>,
    // where the image is coming from
    pub reference_url: Option<String>,
    pub unique_identifier: String,
    /// necessary for some providers like weverse which include additional
    /// metadata that are unique to the provider being scraped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct ProviderResult {
    pub images: Vec<ProviderMedia>,
    pub response_delay: Duration,
    pub response_code: StatusCode,
}

impl ProviderResult {
    pub fn with_images(&self, images: Vec<ProviderMedia>) -> Self {
        Self {
            images,
            response_code: self.response_code,
            response_delay: self.response_delay,
        }
    }
}

impl Add<ProviderResult> for ProviderResult {
    type Output = ProviderResult;
    fn add(self, rhs: ProviderResult) -> Self::Output {
        ProviderResult {
            response_code: rhs.response_code,
            response_delay: rhs.response_delay,
            images: [self.images, rhs.images].concat(),
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
    #[error("Failed to process response from request")]
    Other(HttpError),
}

impl From<reqwest::Error> for ProviderFailure {
    fn from(err: reqwest::Error) -> Self {
        ProviderFailure::HttpError(HttpError::ReqwestError(err))
    }
}

#[derive(Debug, Clone)]
pub struct ProviderState {
    // empty if we're done with pagination
    pub url: ScrapeUrl,
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

pub enum CredentialRefresh {
    Result(ProviderCredentials),
    TryLogin,
    Halt,
}

pub enum ProviderErrorHandle {
    RefreshToken,
    Login,
    Halt,
}

#[derive(Debug)]
pub enum Pagination {
    NextPage(i32),
    NextCursor(String),
}

impl Pagination {
    pub fn next_page(self) -> String {
        match self {
            Pagination::NextPage(num) => num.to_string(),
            Pagination::NextCursor(cursor) => cursor,
        }
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
    fn rate_limiter() -> GlobalProviderLimiter
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

pub struct ProviderInput {
    pub client: Arc<Client>,
    pub credentials: Option<Arc<RwLock<ProviderCredentials>>>,
}

/// Providers represent a generic endpoint on a single platform that can be scraped
/// with a unique identifier for each specific resource
#[async_trait]
pub trait Provider: Sync + Send + RateLimitable {
    fn new(input: ProviderInput) -> Self
    where
        Self: Sized;
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
    /// Provider destination are any unique identifier a provider can try to resolve into an opaque [ScrapeUrl].
    /// This method is called after every successful scrape to resolve the next page of media
    fn from_provider_destination(
        &self,
        id: String,
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

    async fn token_refresh(&self) -> anyhow::Result<CredentialRefresh> {
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
    fn credentials(&self) -> Arc<RwLock<ProviderCredentials>> {
        panic!(
            "Tried to get credentials for {} which doesn't authorization",
            self.id().to_string()
        )
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
}
