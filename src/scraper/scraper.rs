use super::providers::{
    Provider, ProviderFailure, ScrapeRequestInput, ScrapeRequestStep, ScrapeResult, ScrapeStep,
};
use crate::scraper::providers::ScrapeUrl;
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
    let next = provider.fetch(url, step, input).await?;

    Ok(match next {
        ScrapeStep::Continue(result, response) => {
            if iteration >= provider.max_pagination() {
                return Ok(result);
            }
            tokio::time::sleep(provider.scrape_delay()).await;
            let next_url = provider.from_scrape_id(scrape_id, Some(response))?;
            let next =
                step_through_provider(&scrape_id, &next_url, provider, step, input, iteration + 1)
                    .await?;
            result + next
        }
        ScrapeStep::Stop(a) => a,
    })
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
