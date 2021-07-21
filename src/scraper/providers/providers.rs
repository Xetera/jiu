use super::ScrapeUrl;
use crate::models::Media;
use async_trait::async_trait;
use futures::Future;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, ClientBuilder, Error as ReqwestError, StatusCode,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::HashSet,
    fmt,
    iter::FromIterator,
    ops::Add,
    time::{Duration, Instant},
};
use thiserror::Error;

/// Placeholder for images that may contain more metadata in the future?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMedia {
    pub url: String,
    pub unique_identifier: String,
}

impl From<&Media> for ProviderMedia {
    fn from(media: &Media) -> Self {
        Self {
            url: media.url.to_owned(),
            // ???
            unique_identifier: media.url.to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct ProviderResult {
    pub images: Vec<ProviderMedia>,
    pub response_delay: Duration,
    pub response_code: StatusCode,
}

// impl ProviderResult {
//     pub fn with_images(images: Vec<ScrapedMedia>) -> Self {
//         ProviderResult { images }
//     }
// }

// impl Add<ScrapeResult> for ScrapeResult {
//     type Output = ScrapeResult;
//     fn add(self, rhs: ScrapeResult) -> Self::Output {
//         ScrapeResult {
//             images: [self.images, rhs.images].concat(),
//         }
//     }
// }

#[derive(Debug)]
pub enum ProviderStep {
    Next(ProviderResult),
    Error(ProviderFailure),
}

#[derive(Error, Debug)]
pub enum ProviderFailure {
    #[error("Error formatting URL")]
    UrlError,
    #[error("Error from request")]
    FetchError(ReqwestError),
}

impl From<reqwest::Error> for ProviderFailure {
    fn from(err: reqwest::Error) -> Self {
        ProviderFailure::FetchError(err)
    }
}

#[derive(Clone)]
pub struct ProviderState {
    // empty if we're done with pagination
    pub url: Option<ScrapeUrl>,
    pub client: Client,
}

pub struct ScrapeRequestInput {
    pub latest_data: HashSet<String>,
}

pub fn scrape_default_headers() -> HeaderMap {
    HeaderMap::from_iter([(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_static("Jiu Scraper (https://github.com/Xetera/jiu)"),
    )])
}

pub async fn with_async_timer<T: futures::Future, F>(f: F) -> (Duration, T::Output)
where
    F: FnOnce() -> T,
{
    todo!("This doesn't work lol");
    let instant = Instant::now();
    let out = f().await;
    (instant.elapsed(), out)
}

#[async_trait]
pub trait Provider {
    type Step: Send;
    fn name(&self) -> &'static str;
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
    /// Scrape ids are any unique identifier a provider can try to resolve into an opaque ScrapeUrl
    fn from_scrape_id(
        self,
        id: String,
        previous_result: Option<Self::Step>,
    ) -> Result<ScrapeUrl, ProviderFailure>;
    /// fetch a single page of the current resource
    async fn unfold(
        &self,
        identifier: String,
        state: ProviderState,
    ) -> Result<Option<(ProviderResult, ProviderState)>, ProviderFailure>;
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Providers {
    #[serde(rename = "pinterest.board_feed")]
    PinterestBoardFeed,
}
