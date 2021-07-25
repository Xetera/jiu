use super::{PageSize, ScrapeUrl};
use crate::request::HttpError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dyn_clone::DynClone;
use enum_map::Enum;
use log::error;
use reqwest::StatusCode;
use serde;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::Add, time::Duration};
use strum_macros;
use strum_macros::EnumString;
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
    pub last_scrape: Option<DateTime<Utc>>,
}

impl From<HttpError> for ProviderFailure {
    fn from(err: HttpError) -> Self {
        Self::HttpError(err)
    }
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

/// Providers represent a generic endpoint on a single platform that can be scraped
/// with a unique identifier for each specific resource
#[async_trait]
pub trait Provider: Sync + DynClone {
    /// a string that uniquely identifies this provider
    fn id(&self) -> AllProviders;
    /// The page size that should be used when scraping
    /// Destinations that haven't been scraped before should be using a larger
    /// page size to
    fn estimated_page_size(&self, last_scraped: Option<DateTime<Utc>>) -> PageSize;
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
    /// Provider destination are any unique identifier a provider can try to resolve into an opaque [ScrapeUrl`].
    /// This method is called after every successful scrape to resolve the next page of media
    fn from_provider_destination(
        &self,
        id: String,
        page_size: PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure>;
    /// Process a single iteration of the resource
    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure>;
}

#[derive(
    Debug, Copy, Clone, Serialize, EnumString, Enum, strum_macros::ToString, PartialEq, Eq,
)]
pub enum AllProviders {
    #[strum(serialize = "pinterest.board_feed")]
    PinterestBoardFeed,
    #[strum(serialize = "weverse.artist_feed")]
    WeverseArtistFeed,
}

// impl FromStr for AllProviders {}

// impl std::fmt::Display for AllProviders {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.write_str(&serde_json::to_string(self).unwrap())
//     }
// }
