use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderResult,
};
use crate::scraper::ProviderMedia;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use reqwest::Client;

#[derive(Debug)]
pub struct Scrape {
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
    let url = provider.from_scrape_id(scrape_id.to_owned(), None)?;
    let seed = ProviderState {
        client: Client::new(),
        url,
    };
    let mut steps = futures::stream::unfold(Some(seed), |state| async {
        match state {
            None => None,
            Some(state) => match provider.unfold(scrape_id.to_owned(), state).await {
                // we have to indicate an error to the consumer and stop iteration on the next cycle
                Err(err) => Some((ScraperStep::Error(err), None)),
                Ok((ProviderStep::End(result), next)) => Some((ScraperStep::Data(result), None)),
                Ok((ProviderStep::Next(result), next)) => {
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
                let images = page
                    .images
                    .into_iter()
                    .take_while(|r| !input.latest_data.contains(&r.unique_identifier))
                    .collect::<Vec<ProviderMedia>>();
                scrape_requests.push(ScrapeRequest {
                    date,
                    provider_result: ProviderResult {
                        images,
                        response_code,
                        response_delay,
                    },
                });
                if scrape_requests.len() as u16 > provider.max_pagination() {
                    break;
                }
                tokio::time::sleep(provider.scrape_delay()).await;
            }
        }
    }
    Ok(Scrape {
        requests: scrape_requests,
    })
}
