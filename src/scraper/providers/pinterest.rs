use std::{collections::HashMap, sync::Arc, time::Instant};

use async_trait::async_trait;
use chrono::NaiveDateTime;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    request::{parse_successful_response, request_default_headers},
    scheduler::UnscopedLimiter,
    scraper::providers::ProviderMediaType,
};

use super::*;

#[derive(Debug, Deserialize)]
pub struct PinterestImage {
    pub width: u16,
    pub height: u16,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PinterestRichSummary {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PinterestPinner {
    pub full_name: String,
    // I don't know if these are really optional, but just to be safe
    pub image_xlarge_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PinterestBoard {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct PinterestImages {
    pub id: String,
    pub pinner: Option<PinterestPinner>,
    pub board: Option<PinterestBoard>,
    pub images: HashMap<String, PinterestImage>,
    pub rich_summary: Option<PinterestRichSummary>,
}

#[derive(Debug, Deserialize)]
pub struct PinterestResource {
    pub bookmark: Option<String>,
    pub data: Vec<PinterestImages>,
}

#[derive(Debug, Deserialize)]
pub struct PinterestResponse {
    pub resource_response: PinterestResource,
}

#[derive(Debug, Serialize)]
struct PinterestRequestDictOptions<'a> {
    bookmarks: &'a Option<Vec<String>>,
    board_url: &'a str,
    board_id: &'a str,
    // max accepted value by the API is 250
    page_size: usize,
}

#[derive(Debug, Serialize)]
struct PinterestRequestDict<'a> {
    options: PinterestRequestDictOptions<'a>,
}

// #[derive(Clone)]
pub struct PinterestBoardFeed {
    pub client: Arc<Client>,
    pub rate_limiter: UnscopedLimiter,
}

const PINTEREST_BOARD_SEPARATOR: &str = "|";

const URL_ROOT: &str = "https://www.pinterest.com/resource/BoardFeedResource/get";

#[allow(dead_code)]
const MAXIMUM_PAGE_SIZE: usize = 200;

/// pinterest uses a page size of 25
#[allow(dead_code)]
const PROVIDER_NATIVE_PAGE_SIZE: usize = 25;

#[async_trait]
impl RateLimitable for PinterestBoardFeed {
    async fn wait(&self, _key: &str) -> () {
        self.rate_limiter
            .until_ready_with_jitter(default_jitter())
            .await;
    }
}

// PinterestBoard ids are made up of 2 pieces, board_url and board_id formatted in this way
// "board_id|board_url"
#[async_trait]
impl Provider for PinterestBoardFeed {
    fn new(input: ProviderInput) -> Self
    where
        Self: Sized,
    {
        Self {
            client: Arc::clone(&input.client),
            rate_limiter: Self::rate_limiter(),
        }
    }
    fn id(&self) -> AllProviders {
        AllProviders::PinterestBoardFeed
    }
    fn next_page_size(&self, last_scraped: Option<NaiveDateTime>, _iteration: usize) -> PageSize {
        PageSize(match last_scraped {
            // TODO: fix
            None => 20,
            Some(_) => 20,
        })
    }

    fn from_provider_destination(
        &self,
        scrape_id: &str,
        page_size: PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        let (id, path) = scrape_id
            .split_once(PINTEREST_BOARD_SEPARATOR)
            .ok_or(ProviderFailure::Url)?;

        let data = PinterestRequestDict {
            options: PinterestRequestDictOptions {
                bookmarks: &pagination.map(|res| vec![res.next_page()]),
                board_id: id,
                board_url: path,
                page_size: page_size.0,
            },
        };
        let data_str = serde_json::to_string(&data)
            .ok()
            .ok_or(ProviderFailure::Url)?;

        let url = Url::parse_with_params(URL_ROOT, &[("source_url", path), ("data", &data_str)])
            .ok()
            .ok_or(ProviderFailure::Url)?;
        Ok(ScrapeUrl(url.as_str().to_owned()))
    }
    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure> {
        let instant = Instant::now();
        let response = self
            .client
            .get(&state.url.0)
            .headers(request_default_headers())
            .send()
            .await?;
        let response_delay = instant.elapsed();

        let status = &response.status();
        let response_json = parse_successful_response::<PinterestResponse>(response).await?;
        let posts = response_json
            .resource_response
            .data
            .iter()
            .filter_map(|pin| {
                // I imagine every image has an "orig" size but we can't know for sure
                pin.images.get("orig").map(|elem| {
                    ProviderPost {
                        account: pin
                            .pinner
                            .clone()
                            .map(|pinner| ProviderAccount {
                                name: pinner.full_name,
                                avatar_url: pinner.image_xlarge_url,
                            })
                            .unwrap_or_default(),
                        unique_identifier: pin.id.clone(),
                        url: Some(format!("https://www.pinterest.com/pin/{}", pin.id)),
                        post_date: None,
                        // There might be a body here but I don't really care, it's pinterest
                        body: None,
                        images: vec![ProviderMedia {
                            _type: ProviderMediaType::Image,
                            media_url: elem.url.to_owned(),
                            // yes, pinterest literally does not tell you when things were
                            // pinned. It's so stupid
                            reference_url: pin.rich_summary.clone().map(|sum| sum.url),
                            unique_identifier: pin.id.to_owned(),
                            metadata: None,
                        }],
                        metadata: None,
                    }
                })
            })
            .collect::<Vec<_>>();

        let result = ProviderResult {
            posts,
            response_code: status.to_owned(),
            response_delay,
        };

        let bookmark_option = response_json.resource_response.bookmark;
        // we receive a bookmark when there are more images to scrape
        Ok(match bookmark_option {
            Some(bookmark) => ProviderStep::Next(result, Pagination::NextCursor(bookmark)),
            None => ProviderStep::End(result),
        })
    }
}
