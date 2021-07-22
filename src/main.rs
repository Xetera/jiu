use jiu::{
    db::connect,
    scraper::{scraper::scrape, PinterestBoardFeed, ScrapeRequestInput},
};
use std::{collections::HashSet, error::Error};

async fn run() -> Result<(), Box<dyn Error>> {
    // let val = settings.get::<String>("database_url")?;
    // let db = connect().await?;
    // println!("{:?}", db);

    // latest_media_from_provider(&db, &Providers::PinterestBoardFeed).await?;
    // println!("{:?}", latest);

    let step = ScrapeRequestInput {
        latest_data: HashSet::new(),
        // latest
        //     .iter()
        //     .map(|l| l.data.id.clone())
        //     .collect::<HashSet<String>>(),
    };

    let result = scrape(
        "175147941697542476|/tyrajai2003/dream-catcher/",
        &PinterestBoardFeed {},
        &step,
    )
    .await?;
    println!("{:?}", result.requests.len());
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
    env_logger::init();

    match run().await {
        Ok(_) => {}
        Err(err) => println!("{:?}", err),
    };
    println!("Running...");
}
