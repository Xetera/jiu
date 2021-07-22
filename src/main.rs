use jiu::{
    db::{connect, latest_media_ids_from_provider, process_scrape},
    scraper::{scraper::scrape, PinterestBoardFeed, ScrapeRequestInput},
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
    println!("Running...");
}
