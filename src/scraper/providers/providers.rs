use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Error as ReqwestError,
};
use std::{iter::FromIterator, ops::Add, time::Duration};

use crate::image::Image;

use super::ScrapeUrl;

#[derive(Debug)]
pub struct ScrapeResult {
    pub date: DateTime<Utc>,
    pub images: Vec<Image>,
}

impl Add<ScrapeResult> for ScrapeResult {
    type Output = ScrapeResult;
    fn add(self, rhs: ScrapeResult) -> Self::Output {
        ScrapeResult {
            date: rhs.date,
            images: [self.images, rhs.images].concat(),
        }
    }
}

#[derive(Debug)]
pub enum ScrapeStep<T> {
    Continue(ScrapeResult, T),
    MaxPagination(ScrapeResult),
    Stop(ScrapeResult),
}

#[derive(Debug)]
pub enum ProviderFailure {
    UrlError,
    FetchError(ReqwestError),
}

unsafe impl Send for ProviderFailure {}

impl From<reqwest::Error> for ProviderFailure {
    fn from(err: reqwest::Error) -> Self {
        ProviderFailure::FetchError(err)
    }
}

pub struct ScrapeRequestStep<'a> {
    pub client: &'a Client,
    pub iteration: u16,
}

impl ScrapeRequestStep<'_> {
    pub fn next(&self) -> Self {
        Self {
            iteration: self.iteration + 1,
            client: &self.client,
        }
    }
}

pub struct ScrapeRequestInput {
    pub latest_data: Vec<Image>,
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
    fn from_scrape_id(
        &self,
        id: &str,
        previous_result: Option<Self::Step>,
    ) -> Result<ScrapeUrl, ProviderFailure>;
    async fn step(
        &self,
        url: &ScrapeUrl,
        step: &ScrapeRequestStep,
        ctx: &ScrapeRequestInput,
    ) -> Result<ScrapeStep<Self::Step>, ProviderFailure>;
}

#[derive(Debug, Hash)]
pub enum AllProviders {
    PinterestBoardFeed,
}
