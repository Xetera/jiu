use super::{
    providers::{
        Provider, ProviderFailure, ScrapeRequestInput, ScrapeRequestStep, ScrapeResult, ScrapeStep,
    },
    ScrapedMedia,
};
use crate::{models::Media, scraper::providers::ScrapeUrl};
use async_recursion::async_recursion;
use reqwest::Client;

#[async_recursion]
async fn step_through_provider<F: Sync + Provider, 'a>(
    scrape_id: &'a str,
    url: &ScrapeUrl,
    provider: &F,
    step: &ScrapeRequestStep<'a>,
    input: &ScrapeRequestInput,
    iteration: u16,
) -> Result<ScrapeResult, ProviderFailure> {
    let next = provider.fetch(url, step).await?;

    let result = match next {
        ScrapeStep::Continue(result, response) => {
            if iteration >= provider.max_pagination() {
                return Ok(result);
            }
            let filtered_length = result.images.len();
            let images = &result
                .images
                .into_iter()
                .take_while(|r| !input.latest_data.contains(&r.unique_identifier))
                .collect::<Vec<ScrapedMedia>>();

            let next_images = ScrapeResult::with_images(images.to_owned());
            if images.len() != filtered_length {
                println!("Filtered out some old images");
                return Ok(next_images);
            }
            tokio::time::sleep(provider.scrape_delay()).await;
            let next_url = provider.from_scrape_id(scrape_id, Some(response))?;
            let next =
                step_through_provider(&scrape_id, &next_url, provider, step, input, iteration + 1)
                    .await?;
            next_images + next
        }
        ScrapeStep::Stop(a) => a,
    };
    Ok(result)
}

pub async fn scrape<F: Sync + Provider>(
    scrape_id: &str,
    provider: &F,
    input: &ScrapeRequestInput,
) -> Result<ScrapeResult, ProviderFailure> {
    let url = provider.from_scrape_id(scrape_id, None)?;
    let client = Client::new();
    Ok(step_through_provider(
        scrape_id,
        &url,
        provider,
        &ScrapeRequestStep { client: &client },
        input,
        0,
    )
    .await?)
}
