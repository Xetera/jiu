use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderResult,
};
use crate::scraper::ProviderMedia;
use chrono::{DateTime, Duration, Utc};
use futures::StreamExt;
use log::info;
use reqwest::Client;

#[derive(Debug)]
pub struct Scrape {
    pub provider_destination: String,
    pub provider_id: String,
    pub requests: Vec<ScrapeRequest>,
}

#[derive(Debug)]
pub struct ScrapeRequest {
    pub date: DateTime<Utc>,
    pub provider_result: ProviderResult,
}

pub enum ScraperStep {
    Data(ProviderResult),
    Error(ProviderFailure),
}

pub async fn scrape<F: Sync + Copy + Provider>(
    scrape_id: &str,
    provider: &F,
    input: &ScrapeRequestInput,
) -> Result<Scrape, ProviderFailure> {
    println!("Running");
    let url = provider.from_provider_destination(scrape_id.to_owned(), None)?;
    let seed = ProviderState { url };
    let mut steps = futures::stream::unfold(Some(seed), |state| async {
        match state {
            None => None,
            Some(state) => match provider.unfold(scrape_id.to_owned(), state).await {
                // we have to indicate an error to the consumer and stop iteration on the next cycle
                Err(err) => Some((ScraperStep::Error(err), None)),
                Ok(ProviderStep::End(result)) => Some((ScraperStep::Data(result), None)),
                Ok(ProviderStep::Next(result, next)) => {
                    Some((ScraperStep::Data(result), Some(next)))
                }
            },
        }
    })
    .boxed();
    let mut scrape_requests: Vec<ScrapeRequest> = vec![];
    while let Some(step) = steps.next().await {
        match step {
            ScraperStep::Error(error) => {
                println!("{:?}", error);
                break;
            }
            ScraperStep::Data(page) => {
                let date = Utc::now();
                let response_code = page.response_code;
                let response_delay = page.response_delay;
                let original_image_count = page.images.len();
                let images = page
                    .images
                    .into_iter()
                    .take_while(|r| !input.latest_data.contains(&r.unique_identifier))
                    .collect::<Vec<ProviderMedia>>();
                let new_image_count = images.len();
                let provider_result = ProviderResult {
                    images,
                    response_code,
                    response_delay,
                };
                scrape_requests.push(ScrapeRequest {
                    date,
                    provider_result,
                });
                let has_known_image = new_image_count != original_image_count;
                if has_known_image {
                    info!(
                        "'{}' has finished crawling because it reached its last known data",
                        provider.id()
                    );
                    break;
                }
                if scrape_requests.len() as u16 > provider.max_pagination() {
                    break;
                }
                tokio::time::sleep(provider.scrape_delay()).await;
            }
        }
    }
    Ok(Scrape {
        provider_destination: scrape_id.to_owned(),
        provider_id: provider.id().to_owned(),
        requests: scrape_requests,
    })
}
