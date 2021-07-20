use super::ScrapeUrl;
use crate::models::Media;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::bson::Bson;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Error as ReqwestError,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashSet, fmt, iter::FromIterator, ops::Add, time::Duration};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapedMedia {
    /// The date the image was scraped
    pub discovered_at: DateTime<Utc>,
    /// Images are represented by the highest available quality
    pub url: String,
    /// An identifier unique to each provider
    pub id: String,
}

impl From<&Media> for ScrapedMedia {
    fn from(media: &Media) -> Self {
        media.data.to_owned()
    }
}

#[derive(Debug)]
pub struct ScrapeResult {
    pub images: Vec<ScrapedMedia>,
}

impl ScrapeResult {
    pub fn with_images(images: Vec<ScrapedMedia>) -> Self {
        ScrapeResult { images }
    }
}

impl Add<ScrapeResult> for ScrapeResult {
    type Output = ScrapeResult;
    fn add(self, rhs: ScrapeResult) -> Self::Output {
        ScrapeResult {
            images: [self.images, rhs.images].concat(),
        }
    }
}

#[derive(Debug)]
pub enum ScrapeStep<T> {
    Continue(ScrapeResult, T),
    Stop(ScrapeResult),
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

pub struct ScrapeRequestStep<'a> {
    pub client: &'a Client,
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
        &self,
        id: &str,
        previous_result: Option<Self::Step>,
    ) -> Result<ScrapeUrl, ProviderFailure>;
    /// fetch a single page of the current resource
    async fn fetch(
        &self,
        url: &ScrapeUrl,
        step: &ScrapeRequestStep,
    ) -> Result<ScrapeStep<Self::Step>, ProviderFailure>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Providers {
    #[serde(rename = "pinterest.board_feed")]
    PinterestBoardFeed,
}

impl Into<Bson> for Providers {
    fn into(self) -> Bson {
        Bson::String(serde_json::to_string(&self).unwrap())
    }
}
