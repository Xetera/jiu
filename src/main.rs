use futures::{stream, StreamExt};
use jiu::{
    db::{
        connect, latest_media_ids_from_provider, pending_scrapes, process_scrape,
        submit_webhook_responses, webhooks_for_provider,
    },
    models::PendingProvider,
    scraper::{scraper::scrape, PinterestBoardFeed, ScopedProvider, ScrapeRequestInput},
    webhook::dispatcher::dispatch_webhooks,
};
use reqwest::Client;
use sqlx::{Pool, Postgres};
use std::error::Error;

const PROVIDER_PROCESSING_LIMIT: usize = 8;

struct Context {
    db: Pool<Postgres>,
    client: reqwest::Client,
}

async fn iter(ctx: &Context, pending: PendingProvider) -> Result<(), Box<dyn Error>> {
    let sp = pending.provider;
    let pinterest = PinterestBoardFeed {
        client: &ctx.client,
    };
    let latest_data = dbg!(latest_media_ids_from_provider(&ctx.db, &sp).await?);

    let step = ScrapeRequestInput {
        latest_data,
        last_scrape: pending.last_scrape,
    };
    let result = scrape(&sp, &pinterest, &step).await?;
    let processed_scrape = process_scrape(&ctx.db, &result).await?;
    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    let webhook_interactions = dispatch_webhooks(&result, webhooks).await;
    submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions).await?;
    Ok(())
}

async fn run() -> Result<(), Box<dyn Error>> {
    let db = connect().await?;
    let client = Client::new();
    let providers = pending_scrapes(&db).await?;
    let ctx = Context { db, client };
    stream::iter(providers)
        .for_each_concurrent(PROVIDER_PROCESSING_LIMIT, |sp| async {
            match iter(&ctx, sp).await {
                Ok(a) => {}
                Err(error) => {}
            }
            ()
        })
        .await;
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
