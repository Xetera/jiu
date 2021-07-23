use super::ScrapeUrl;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::error;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Error as ReqwestError, Response, StatusCode,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::HashSet, env, iter::FromIterator, ops::Add, time::Duration};
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
    #[error("Error from request")]
    Fetch(ReqwestError),
    #[error("Error processing the body")]
    Deserialization(String, StatusCode),
}

impl From<reqwest::Error> for ProviderFailure {
    fn from(err: reqwest::Error) -> Self {
        ProviderFailure::Fetch(err)
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

pub fn scrape_default_headers() -> HeaderMap {
    // TODO: change the user agent if the program has been forked to modify
    // important settings like request speed
    let user_agent: String =
        env::var("USER_AGENT").expect("Missing USER_AGENT environment variable");
    HeaderMap::from_iter([(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_str(&user_agent).unwrap(),
    )])
}

pub async fn parse_response_body<T: DeserializeOwned>(
    response: Response,
) -> Result<T, ProviderFailure> {
    let response_code = response.status();
    let url = response.url().clone();
    let response_body = response.text().await?;
    serde_json::from_str::<T>(&response_body).map_err(|_error| {
        // I hope the ToString implementation of Url is the full url otherwise it's
        // gonna spam stderr lol
        error!("Failed to parse response from {}", url);
        ProviderFailure::Deserialization(response_body, response_code)
    })
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
    /// Provider destination are any unique identifier a provider can try to resolve into an opaque ScrapeUrl
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
