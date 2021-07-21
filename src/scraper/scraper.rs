use std::convert::TryInto;

use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderMedia, ProviderResult,
};
use crate::{models::ScrapeRequest, scraper::providers::ScrapeUrl};
use async_recursion::async_recursion;
use chrono::{Date, DateTime, Utc};
use futures::StreamExt;
use reqwest::Client;

pub struct Scrape {
    pub requests: Vec<ScrapeRequest>,
}

pub struct ScrapeRequest {
    pub date: DateTime<Utc>,
    pub provider_result: ProviderResult,
}

// #[async_recursion]
// async fn step_through_provider<F: Sync + Provider, 'a>(
//     scrape_id: &'a str,
//     url: &ScrapeUrl,
//     provider: &F,
//     step: &ProviderState<'a>,
//     input: &ScrapeRequestInput,
//     iteration: u16,
//     results: &mut Vec<ScrapeRequest>,
// ) -> Result<(), ProviderFailure> {
//     let next = provider.fetch(url, step).await?;
//     let result = match next {
//         ProviderStep::Continue(result, response) => {
//             if iteration >= provider.max_pagination() {
//                 return Ok(result);
//             }
//             let filtered_length = result.images.len();
//             let images = &result
//                 .images
//                 .into_iter()
//                 .take_while(|r| !input.latest_data.contains(&r.unique_identifier))
//                 .collect::<Vec<ProviderMedia>>();
//             // let provider_result = ProviderResult::with_images(images.to_owned());
//             results.append(scrape_request);
//             if images.len() != filtered_length {
//                 println!("Filtered out some old images");
//                 return Ok(());
//             }
//             tokio::time::sleep(provider.scrape_delay()).await;
//             let next_url = provider.from_scrape_id(scrape_id, Some(response))?;
//             // results.append(image)
//             step_through_provider(
//                 &scrape_id,
//                 &next_url,
//                 provider,
//                 step,
//                 input,
//                 iteration + 1,
//                 results,
//             )
//             .await?;
//             next_images + next
//         }
//         ProviderStep::Stop(provider_result) => [
//             input,
//             ScrapeRequest {
//                 provider_result,
//                 date,
//             },
//         ]
//         .concat(),
//     };
//     Ok(result)
// }

pub async fn scrape<F: Sync + Copy + Provider>(
    scrape_id: &str,
    provider: &F,
    input: &ScrapeRequestInput,
) -> Result<ProviderResult, ProviderFailure> {
    println!("Running");
    let url = provider.from_scrape_id(scrape_id.to_owned(), None)?;
    // haskell gods forgive me for this cringe
    let seed = ProviderState {
        client: Client::new(),
        url: Some(url),
    };
    let steps = futures::stream::unfold(Some(seed), |state| async {
        match state {
            None => None,
            Some(state) => match provider.unfold(scrape_id.to_owned(), state).await {
                // we have to indicate an error to the consumer and stop iteration on the next cycle
                Err(err) => Some((ProviderStep::Error(err), None)),
                // page end
                Ok(None) => None,
                Ok(Some((result, next))) => Some((ProviderStep::Next(result), Some(next))),
            },
        }
    });
    let scrape_requests: Vec<ScrapeRequest> = vec![];
    for step in steps.boxed().next().await {
        match step {
            ProviderStep::Error(error) => {
                println!("{:?}", error);
                break;
            }
            ProviderStep::Next(page) => {
                let date = Utc::now();
                scrape_requests.append(ScrapeRequest {
                    date,
                    provider_result: page,
                });
                // if requests.len() as u16 > provider.max_pagination() {
                //     break;
                // }
            }
        }
    }
    todo!("end of loop")
}
