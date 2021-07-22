use super::{
    scrape_default_headers, with_async_timer, Provider, ProviderFailure, ProviderMedia,
    ProviderResult, ProviderState, ProviderStep, ScrapeRequestInput, ScrapeUrl,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Instant, SystemTime},
};
use url::Url;

#[derive(Debug, Deserialize)]
pub struct PinterestImage {
    pub width: u16,
    pub height: u16,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct PinterestImages {
    pub id: String,
    pub images: HashMap<String, PinterestImage>,
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
pub struct PinterestBoardFeed {}

const PINTEREST_BOARD_SEPARATOR: &str = "|";

const URL_ROOT: &str = "https://www.pinterest.com/resource/BoardFeedResource/get";

const EXPECTED_PAGE_SIZE: usize = 50;

// PinterestBoard ids are made up of 2 pieces, board_url and board_id formatted in this way
// "board_id|board_url"
#[async_trait]
impl Provider for PinterestBoardFeed {
    type Step = PinterestResponse;
    fn name(&self) -> &'static str {
        "Pinterest Board Feed"
    }
    fn from_scrape_id(
        self,
        scrape_id: String,
        previous_result: Option<PinterestResponse>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        let (id, path) = scrape_id
            .split_once(PINTEREST_BOARD_SEPARATOR)
            .ok_or(ProviderFailure::UrlError)?;

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
            .ok_or(ProviderFailure::UrlError)?;

        let url = Url::parse_with_params(URL_ROOT, &[("source_url", path), ("data", &data_str)])
            .ok()
            .ok_or(ProviderFailure::UrlError)?;
        Ok(ScrapeUrl(dbg!(url.as_str().to_owned())))
    }
    async fn unfold(
        &self,
        identifier: String,
        state: ProviderState,
    ) -> Result<(ProviderStep, ProviderState), ProviderFailure> {
        let instant = Instant::now();
        println!("Scraping pinterest...");
        let response = state
            .client
            // I'm so sorry
            .get(&state.url.0)
            .headers(scrape_default_headers())
            .send()
            .await?;

        let status = &response.status();
        let response_json = response.json::<PinterestResponse>().await?;

        let images = response_json
            .resource_response
            .data
            .iter()
            .filter_map(|r| {
                // I imagine every image has an "orig" size but we can't know for sure
                r.images.get("orig").map(|elem| ProviderMedia {
                    url: elem.url.to_owned(),
                    unique_identifier: r.id.to_owned(),
                })
            })
            .collect::<Vec<ProviderMedia>>();

        let result = ProviderResult {
            images,
            response_code: status.to_owned(),
            response_delay: instant.elapsed(),
        };

        let bookmark = response_json.resource_response.bookmark.clone();
        let next_url = self.from_scrape_id(identifier, Some(response_json))?;

        let next_state = ProviderState {
            client: state.client,
            url: next_url,
        };

        // we receive a bookmark when there are more images to scrape
        Ok(match bookmark {
            Some(_) => (ProviderStep::Next(result), next_state),
            None => (ProviderStep::End(result), next_state),
        })
    }
}
