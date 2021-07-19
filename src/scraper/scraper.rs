use crate::scraper::providers::ScrapeUrl;

use super::providers::{PinterestBoardFeed, PinterestImage, PinterestResponse};
use super::providers::{
    Provider, ProviderFailure, ScrapeRequestInput, ScrapeRequestStep, ScrapeResult, ScrapeStep,
};
use super::AllProviders;
use async_recursion::async_recursion;
use reqwest::Client;

fn instance(provider: &AllProviders) -> impl Provider {
    match provider {
        AllProviders::PinterestBoardFeed => PinterestBoardFeed {},
    }
}

pub async fn scrape(
    scrape_id: &str,
    provider: &AllProviders,
    input: &ScrapeRequestInput,
) -> Result<ScrapeResult, ProviderFailure> {
    #[async_recursion]
    async fn step_through_provider<'a>(
        scrape_id: &'a str,
        url: &ScrapeUrl,
        provider_type: &AllProviders,
        step: &ScrapeRequestStep<'a>,
        input: &ScrapeRequestInput,
    ) -> Result<ScrapeResult, ProviderFailure> {
        let provider = instance(&provider_type);
        let next = provider.step(url, step, input).await?;

        Ok(match next {
            ScrapeStep::Continue(result, response) => {
                tokio::time::sleep(provider.scrape_delay()).await;
                let next_url = provider.from_scrape_id(scrape_id, Some(response))?;
                let next = step_through_provider(
                    &scrape_id,
                    &next_url,
                    provider_type,
                    &step.next(),
                    input,
                )
                .await?;
                result + next
            }
            ScrapeStep::Stop(a) => a,
            ScrapeStep::MaxPagination(a) => {
                println!("Reached max pagination for {:?}", provider.name());
                a
            }
        })
    }

    let url = instance(provider).from_scrape_id(scrape_id, None)?;
    let client = Client::new();
    Ok(step_through_provider(
        scrape_id,
        &url,
        provider,
        &ScrapeRequestStep {
            iteration: 1,
            client: &client,
        },
        input,
    )
    .await?)
}
