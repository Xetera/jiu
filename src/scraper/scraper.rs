use std::time::Instant;

use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderResult, ScopedProvider,
};
use crate::scraper::ProviderMedia;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use log::{debug, info};

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
    Exit,
}

#[derive(Debug)]
pub enum ScraperStep {
    Data(ProviderResult),
    // we only want to forward request related errors to the consumer
    Error(ProviderFailure),
}

pub async fn scrape<'a>(
    sp: &'a ScopedProvider,
    scrape: &dyn Provider,
    input: &ScrapeRequestInput,
) -> Result<Scrape<'a>, ProviderFailure> {
    let initial_iteration = 0;
    let page_size = scrape.next_page_size(input.last_scrape, initial_iteration);
    let url =
        scrape.from_provider_destination(sp.destination.clone(), page_size.to_owned(), None)?;
    let seed = ProviderState {
        url,
        iteration: initial_iteration,
    };
    let mut steps = futures::stream::unfold(Some(seed), |state| async {
        match state {
            None => None,
            Some(state) => {
                debug!("Scraping URL: {:?}", state.url.0);
                let iteration = state.iteration;
                Some(match scrape.unfold(state).await {
                    // we have to indicate an error to the consumer and stop iteration on the next cycle
                    Err(err) => (InternalScraperStep::Error(err), None),
                    Ok(ProviderStep::End(result)) => (InternalScraperStep::Data(result), None),
                    Ok(ProviderStep::NotInitialized) => (InternalScraperStep::Exit, None),
                    Ok(ProviderStep::Next(result, response_json)) => {
                        let page_size = scrape.next_page_size(input.last_scrape, iteration);
                        let maybe_next_url = scrape.from_provider_destination(
                            sp.destination.clone(),
                            page_size.to_owned(),
                            Some(response_json),
                        );
                        match maybe_next_url {
                            Err(err) => (InternalScraperStep::Error(err), None),
                            Ok(url) => {
                                let next_state = ProviderState {
                                    url: url.clone(),
                                    iteration: iteration + 1,
                                };
                                (InternalScraperStep::Data(result), Some(next_state))
                            }
                        }
                    }
                })
            }
        }
    })
    .boxed_local();
    let mut scrape_requests: Vec<ScrapeRequest> = vec![];
    let scrape_start = Instant::now();
    while let Some(step) = steps.next().await {
        let date = Utc::now();
        match step {
            InternalScraperStep::Exit => {}
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
                debug!("Found {} new images in {}", images.len(), sp);
                scrape_requests.push(ScrapeRequest {
                    date,
                    step: ScraperStep::Data(ProviderResult { images, ..page }),
                });
                let has_known_image = new_image_count != original_image_count;
                if has_known_image {
                    info!(
                        "[{}] has finished crawling because it's back to the last scraped data point",
                        sp
                    );
                    break;
                }
                let pagination_limit = scrape.max_pagination();
                if scrape_requests.len() as u16 > pagination_limit {
                    info!(
                        "[{}] has reached its pagination limit of {}",
                        sp, pagination_limit
                    );
                    break;
                }
                scrape.wait(&sp.destination).await;
            }
        }
    }
    let scrape_count = scrape_requests.len();
    info!(
        "[{}] finished scraping in {:?} after {} request{}",
        sp,
        scrape_start.elapsed(),
        scrape_count,
        if scrape_count != 1 { "s" } else { "" }
    );
    Ok(Scrape {
        provider: sp.to_owned(),
        requests: scrape_requests,
    })
}

// pub fn maximize_element_distance(sps: Vec<ScopedProvider>) -> Vec<ScopedProvider> {
//     sps.group_by(|a, b| a.destination == b.destination)
//         .collect::<Vec<Vec<ScopedProvider>>>();
//     sps
// }
