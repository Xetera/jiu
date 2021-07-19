use super::providers::{
    AllProviders, Provider, ProviderFailure, ScrapeRequestInput, ScrapeRequestStep, ScrapeResult,
    ScrapeStep,
};
use super::providers::{PinterestBoardFeed, ScrapeUrl};
use async_recursion::async_recursion;
use reqwest::Client;

pub async fn scrape(
    scrape_id: &str,
    provider: &AllProviders,
    input: &ScrapeRequestInput,
) -> Result<ScrapeResult, ProviderFailure> {
    #[async_recursion]
    async fn go(
        url: &ScrapeUrl,
        step: &ScrapeRequestStep,
        input: &ScrapeRequestInput,
    ) -> Result<ScrapeResult, ProviderFailure> {
        let next = match provider {
            AllProviders::PinterestBoardFeed => PinterestBoardFeed::step(url, step, input).await?,
        };

        Ok(match next {
            ScrapeStep::Continue((result, url)) => {
                let next = go(&url, &step.next(), input).await?;
                result + next
            }
            ScrapeStep::Stop(a) => a,
            ScrapeStep::MaxPagination(a) => a,
        })
    }
    let url = match provider {
        AllProviders::PinterestBoardFeed => PinterestBoardFeed::from_scrape_id(&scrape_id)?,
    };
    let client = Client::new();
    Ok(go(
        &url,
        &ScrapeRequestStep {
            iteration: 1,
            client: &client,
        },
        input,
    )
    .await?)
}
