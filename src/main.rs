use jiu::{
    db::{
        connect, latest_media_ids_from_provider, pending_scrapes, process_scrape,
        submit_webhook_responses, webhooks_for_provider,
    },
    scraper::{scraper::scrape, PinterestBoardFeed, Provider, ScrapeRequestInput},
    webhook::dispatcher::dispatch_webhooks,
};
use reqwest::Client;
use std::error::Error;

async fn run() -> Result<(), Box<dyn Error>> {
    let db = connect().await?;
    let client = Client::new();

    let pending_scrapes = pending_scrapes(&db).await?;
    let scoped_provider = pending_scrapes.get(0).unwrap();
    let pinterest = PinterestBoardFeed { client: &client };
    let latest_data = latest_media_ids_from_provider(&db, &scoped_provider.destination).await?;

    let step = ScrapeRequestInput { latest_data };

    let result = scrape(&scoped_provider.destination, &pinterest, &step).await?;
    let processed_scrape = process_scrape(&db, &result).await?;
    let webhooks = webhooks_for_provider(&db, scoped_provider).await?;
    println!("{:?}", webhooks);
    let webhook_interactions = dispatch_webhooks(&result, webhooks).await;
    submit_webhook_responses(&db, processed_scrape, webhook_interactions).await?;
    // println!("{:?}", nums);
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
