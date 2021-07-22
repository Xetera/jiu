use jiu::{
    db::{connect, latest_media_ids_from_provider, process_scrape, webhooks_for_provider},
    scraper::{scraper::scrape, PinterestBoardFeed, ScrapeRequestInput},
    webhook::dispatcher::dispatch_webhooks,
};
use reqwest::Client;
use std::error::Error;

async fn run() -> Result<(), Box<dyn Error>> {
    let db = connect().await?;
    let client = Client::new();

    let provider_destination = "175147941697542476|/tyrajai2003/dream-catcher/";
    let pinterest = PinterestBoardFeed { client: &client };
    let latest_data = latest_media_ids_from_provider(&db, provider_destination).await?;

    let step = ScrapeRequestInput { latest_data };

    let result = scrape(provider_destination, &pinterest, &step).await?;
    process_scrape(&db, &result).await?;
    let webhooks = webhooks_for_provider(&db, provider_destination).await?;
    println!("{:?}", webhooks);
    let nums = dispatch_webhooks(&result, webhooks).await;
    println!("{:?}", nums);
    Ok(())
}

#[tokio::main]
async fn main() {
    better_panic::install();
    env_logger::init();

    match run().await {
        Ok(_) => {}
        Err(err) => eprintln!("{:?}", err),
    };
    println!("Done!");
}
