use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Error as ReqwestError,
};
use std::{iter::FromIterator, ops::Add};

use crate::image::Image;

#[derive(Debug)]
pub struct ScrapeUrl(pub String);

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
pub enum ScrapeStep {
    Continue((ScrapeResult, ScrapeUrl)),
    MaxPagination(ScrapeResult),
    Stop(ScrapeResult),
}

#[derive(Debug)]
pub enum ProviderFailure {
    UrlError,
    FetchError(ReqwestError),
}

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
    fn from_scrape_id(id: &str) -> Result<ScrapeUrl, ProviderFailure>;
    async fn step(
        url: &ScrapeUrl,
        step: &ScrapeRequestStep,
        ctx: &ScrapeRequestInput,
    ) -> Result<ScrapeStep, ProviderFailure>;
    fn normalize_image_url(url: &str) -> &str {
        url
    }
}

#[derive(Debug, Hash)]
pub enum AllProviders {
    PinterestBoardFeed,
}
