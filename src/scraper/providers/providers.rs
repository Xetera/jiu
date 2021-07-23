use crate::request::HttpError;

use super::ScrapeUrl;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::error;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::Add, time::Duration};
use thiserror::Error;

/// Placeholder for images that may contain more metadata in the future?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMedia {
    pub image_url: String,
    pub page_url: Option<String>,
    pub post_date: Option<DateTime<Utc>>,
    // where the image is coming from
    pub reference_url: Option<String>,
    pub unique_identifier: String,
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
    Next(ProviderResult, ProviderState),
    End(ProviderResult),
}

#[derive(Error, Debug)]
pub enum ProviderFailure {
    #[error("Error formatting URL")]
    Url,
    #[error("Failed to process response from request")]
    HttpError(HttpError),
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
}

pub struct ScrapeRequestInput {
    pub latest_data: HashSet<String>,
}

impl From<HttpError> for ProviderFailure {
    fn from(err: HttpError) -> Self {
        Self::HttpError(err)
    }
}

#[async_trait]
pub trait Provider {
    type Step;
    fn id(&self) -> &'static str;
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
    /// Provider destination are any unique identifier a provider can try to resolve into an opaque [ScrapeUrl`]
    fn from_provider_destination(
        self,
        id: String,
        previous_result: Option<Self::Step>,
    ) -> Result<ScrapeUrl, ProviderFailure>;
    /// Process a single iteration of the resource
    async fn unfold(
        &self,
        identifier: String,
        state: ProviderState,
    ) -> Result<ProviderStep, ProviderFailure>;
}
