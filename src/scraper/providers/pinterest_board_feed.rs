use super::{
    scrape_default_headers, Provider, ProviderFailure, ScrapeRequestInput, ScrapeRequestStep,
    ScrapeResult, ScrapeStep, ScrapeUrl,
};
use crate::image::Image;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub data: Vec<PinterestImages>,
}

#[derive(Debug, Deserialize)]
pub struct PinterestResponse {
    pub resource_response: PinterestResource,
}

#[derive(Debug, Serialize)]
struct PinterestRequestDictOptions<'a> {
    board_url: &'a str,
    board_id: &'a str,
    // max accepted value by the API is 250
    page_size: usize,
}

#[derive(Debug, Serialize)]
struct PinterestRequestDict<'a> {
    options: PinterestRequestDictOptions<'a>,
}

pub struct PinterestBoardFeed {}

const PINTEREST_BOARD_SEPARATOR: &str = "|";

const URL_ROOT: &str = "https://www.pinterest.com/resource/BoardFeedResource/get";

const EXPECTED_PAGE_SIZE: usize = 250;

// PinterestBoard ids are made up of 2 pieces, board_url and board_id formatted in this way
// "board_id|board_url"
#[async_trait]
impl Provider for PinterestBoardFeed {
    fn from_scrape_id(scrape_id: &str) -> Result<ScrapeUrl, ProviderFailure> {
        let (id, path) = scrape_id
            .split_once(PINTEREST_BOARD_SEPARATOR)
            .ok_or(ProviderFailure::UrlError)?;

        let data = PinterestRequestDict {
            options: PinterestRequestDictOptions {
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
        Ok(ScrapeUrl(url.as_str().to_owned()))
    }
    async fn step(
        url: &ScrapeUrl,
        step: &ScrapeRequestStep,
        input: &ScrapeRequestInput,
    ) -> Result<ScrapeStep, ProviderFailure> {
        let response = step
            .client
            .get(&url.0)
            .headers(scrape_default_headers())
            .send()
            .await?
            .json::<PinterestResponse>()
            .await?;

        let images = response
            .resource_response
            .data
            .iter()
            .filter_map(|r| {
                r.images.get("orig").map(|elem| Image {
                    url: elem.url.to_owned(),
                    id: r.id.to_owned(),
                })
            })
            // this vector ideally only has 5 elements so the nested loop isn't a huge deal
            .take_while(|r| input.latest_data.iter().all(|im| im.id != r.id))
            .collect::<Vec<Image>>();

        let result = ScrapeResult {
            date: Utc::now(),
            images,
        };

        if step.iteration >= 4 {
            return Ok(ScrapeStep::MaxPagination(result));
        }

        Ok(ScrapeStep::Stop(result))
    }
}
