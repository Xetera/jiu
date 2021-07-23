use crate::request::{parse_successful_response, request_default_headers};

use super::{
    Provider, ProviderFailure, ProviderMedia, ProviderResult, ProviderState, ProviderStep,
    ScrapeUrl,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};
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

#[derive(Copy, Clone)]
pub struct PinterestBoardFeed<'a> {
    pub client: &'a Client,
}

const PINTEREST_BOARD_SEPARATOR: &str = "|";

const URL_ROOT: &str = "https://www.pinterest.com/resource/BoardFeedResource/get";

const EXPECTED_PAGE_SIZE: usize = 200;

// PinterestBoard ids are made up of 2 pieces, board_url and board_id formatted in this way
// "board_id|board_url"
#[async_trait]
impl<'a> Provider for PinterestBoardFeed<'a> {
    type Step = PinterestResponse;
    fn id(&self) -> &'static str {
        "pinterest.board_feed"
    }
    fn from_provider_destination(
        self,
        scrape_id: String,
        previous_result: Option<PinterestResponse>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        let (id, path) = scrape_id
            .split_once(PINTEREST_BOARD_SEPARATOR)
            .ok_or(ProviderFailure::Url)?;

        let data = PinterestRequestDict {
            options: PinterestRequestDictOptions {
                bookmarks: &previous_result
                    .and_then(|res| res.resource_response.bookmark.map(|bm| vec![bm])),
                board_id: id,
                board_url: path,
                page_size: EXPECTED_PAGE_SIZE,
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
    async fn unfold(
        &self,
        identifier: String,
        state: ProviderState,
    ) -> Result<ProviderStep, ProviderFailure> {
        let instant = Instant::now();
        println!("Scraping pinterest...");
        let response = self
            .client
            // I'm so sorry
            .get(&state.url.0)
            .headers(request_default_headers())
            .send()
            .await?;

        let status = &response.status();
        let response_json = parse_successful_response::<PinterestResponse>(response).await?;
        let images = response_json
            .resource_response
            .data
            .iter()
            .filter_map(|pin| {
                // I imagine every image has an "orig" size but we can't know for sure
                pin.images.get("orig").map(|elem| ProviderMedia {
                    image_url: elem.url.to_owned(),
                    page_url: Some(format!("https://www.pinterest.com/pin/{}", pin.id)),
                    // yes, pinterest literally does not tell you when things were
                    // pinned. It's so stupid
                    post_date: None,
                    reference_url: pin.rich_summary.clone().map(|sum| sum.url),
                    unique_identifier: pin.id.to_owned(),
                })
            })
            .collect::<Vec<ProviderMedia>>();

        let result = ProviderResult {
            images,
            response_code: status.to_owned(),
            response_delay: instant.elapsed(),
        };

        let bookmark = response_json.resource_response.bookmark.clone();
        // we receive a bookmark when there are more images to scrape
        Ok(match bookmark {
            Some(_) => {
                let next_url = self.from_provider_destination(identifier, Some(response_json))?;
                let next_state = ProviderState { url: next_url };
                ProviderStep::Next(result, next_state)
            }
            None => ProviderStep::End(result),
        })
    }
}
