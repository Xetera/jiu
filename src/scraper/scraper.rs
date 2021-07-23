use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderResult, ScopedProvider,
};
use crate::scraper::ProviderMedia;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use log::info;

#[derive(Debug)]
pub struct Scrape<'a> {
    pub provider: &'a ScopedProvider,
    pub requests: Vec<ScrapeRequest>,
}

#[derive(Debug)]
pub struct ScrapeRequest {
    pub date: DateTime<Utc>,
    pub step: ScraperStep,
}

#[derive(Debug)]
enum InternalScraperStep {
    Data(ProviderResult),
    Error(ProviderFailure),
}

#[derive(Debug)]
pub enum ScraperStep {
    Data(ProviderResult),
    // we only want to forward request related errors to the consumer
    Error(ProviderFailure),
}

pub async fn scrape<'a, F: Sync + Copy + Provider>(
    sp: &'a ScopedProvider,
    provider: &F,
    input: &ScrapeRequestInput,
) -> Result<Scrape<'a>, ProviderFailure> {
    println!("Running");
    let url = provider.from_provider_destination(sp.destination.clone(), None)?;
    let seed = ProviderState { url };
    let mut steps = futures::stream::unfold(Some(seed), |state| async {
        match state {
            None => None,
            Some(state) => Some(match provider.unfold(sp.destination.clone(), state).await {
                // we have to indicate an error to the consumer and stop iteration on the next cycle
                Err(err) => (InternalScraperStep::Error(err), None),
                Ok(ProviderStep::End(result)) => (InternalScraperStep::Data(result), None),
                Ok(ProviderStep::Next(result, next)) => {
                    (InternalScraperStep::Data(result), Some(next))
                }
            }),
        }
    })
    .boxed();
    let mut scrape_requests: Vec<ScrapeRequest> = vec![];
    while let Some(step) = steps.next().await {
        let date = Utc::now();
        match step {
            InternalScraperStep::Error(error) => {
                scrape_requests.push(ScrapeRequest {
                    date,
                    step: ScraperStep::Error(error),
                });
            }
            InternalScraperStep::Data(page) => {
                let original_image_count = page.images.len();
                let images = page
                    .images
                    // TODO: remove this clone using Rc?
                    .clone()
                    .into_iter()
                    .take_while(|r| !input.latest_data.contains(&r.unique_identifier))
                    .collect::<Vec<ProviderMedia>>();
                let new_image_count = images.len();
                scrape_requests.push(ScrapeRequest {
                    date,
                    step: ScraperStep::Data(page),
                });
                let has_known_image = new_image_count != original_image_count;
                if has_known_image {
                    info!(
                        "'{}' has finished crawling because it reached its last known data",
                        provider.id()
                    );
                    break;
                }
                let pagination_limit = provider.max_pagination();
                if scrape_requests.len() as u16 > pagination_limit {
                    info!(
                        "'{}' has reached its pagination limit of {}",
                        provider.id(),
                        pagination_limit
                    );
                    break;
                }
                tokio::time::sleep(provider.scrape_delay()).await;
            }
        }
    }
    Ok(Scrape {
        provider: sp.to_owned(),
        requests: scrape_requests,
    })
}
