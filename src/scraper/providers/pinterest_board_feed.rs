use crate::{
    request::{parse_successful_response, request_default_headers},
    scraper::providers::ProviderMediaType,
};

use super::{
    AllProviders, PageSize, Pagination, Provider, ProviderFailure, ProviderMedia, ProviderResult,
    ProviderState, ProviderStep, ScrapeUrl,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Instant};
use url::Url;

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

#[derive(Debug, Deserialize)]
pub struct PinterestImages {
    pub id: String,
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

#[derive(Clone)]
pub struct PinterestBoardFeed {
    pub client: Arc<Client>,
}

const PINTEREST_BOARD_SEPARATOR: &str = "|";

const URL_ROOT: &str = "https://www.pinterest.com/resource/BoardFeedResource/get";

const MAXIMUM_PAGE_SIZE: usize = 200;

/// pinterest uses a page size of 25
const PROVIDER_NATIVE_PAGE_SIZE: usize = 25;

// PinterestBoard ids are made up of 2 pieces, board_url and board_id formatted in this way
// "board_id|board_url"
#[async_trait]
impl Provider for PinterestBoardFeed {
    fn id(&self) -> AllProviders {
        AllProviders::PinterestBoardFeed
    }
    fn estimated_page_size(
        &self,
        last_scraped: Option<DateTime<Utc>>,
        _iteration: usize,
    ) -> PageSize {
        PageSize(match last_scraped {
            // TODO: fix
            None => MAXIMUM_PAGE_SIZE,
            Some(_) => MAXIMUM_PAGE_SIZE,
        })
    }

    fn from_provider_destination(
        &self,
        scrape_id: String,
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
        let images = response_json
            .resource_response
            .data
            .iter()
            .filter_map(|pin| {
                // I imagine every image has an "orig" size but we can't know for sure
                pin.images.get("orig").map(|elem| ProviderMedia {
                    _type: ProviderMediaType::Image,
                    media_url: elem.url.to_owned(),
                    page_url: Some(format!("https://www.pinterest.com/pin/{}", pin.id)),
                    // yes, pinterest literally does not tell you when things were
                    // pinned. It's so stupid
                    post_date: None,
                    reference_url: pin.rich_summary.clone().map(|sum| sum.url),
                    unique_identifier: pin.id.to_owned(),
                    provider_metadata: None,
                })
            })
            .collect::<Vec<ProviderMedia>>();

        let result = ProviderResult {
            images,
            response_code: status.to_owned(),
            response_delay,
        };

        let bookmark_option = response_json.resource_response.bookmark.clone();
        // we receive a bookmark when there are more images to scrape
        Ok(match bookmark_option {
            Some(bookmark) => ProviderStep::Next(result, Pagination::NextCursor(bookmark)),
            None => ProviderStep::End(result),
        })
    }
}
