mod db;
mod models;
mod scraper;

use crate::{
    db::connect,
    scraper::{scraper::scrape, PinterestBoardFeed, Providers, ScrapeRequestInput},
};
use config::Config;
use std::{collections::HashSet, error::Error};

async fn run(settings: &Config) -> Result<(), Box<dyn Error>> {
    let val = settings.get::<String>("database_url")?;
    let db = connect().await?;
    db.batch_get_item(input)
    println!("{:?}", db);

    // let latest = latest_media_from_provider(&db, &Providers::PinterestBoardFeed).await?;
    // println!("{:?}", latest);

    // let step = ScrapeRequestInput {
    //     latest_data: latest
    //         .iter()
    //         .map(|l| l.data.id.clone())
    //         .collect::<HashSet<String>>(),
    // };

    // let result = scrape(
    //     "175147941697542476|/tyrajai2003/dream-catcher/",
    //     &PinterestBoardFeed {},
    //     &step,
    // )
    // .await?;
    // println!("{:?}", result.images.len());
    // if !result.images.is_empty() {
    //     add_media(&db, &Providers::PinterestBoardFeed, result.images).await?;
    // }
    Ok(())
}

#[tokio::main]
async fn main() {
    better_panic::install();
    let mut settings = config::Config::default();
    settings
        // Add in `./Settings.toml`
        .merge(config::File::with_name("env"))
        .unwrap();

    match run(&mut settings).await {
        Ok(_) => {}
        Err(err) => println!("{:?}", err),
    };
    println!("Running...");
}
