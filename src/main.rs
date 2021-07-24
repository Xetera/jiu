use futures::{stream, StreamExt};
use jiu::{
    db::{
        connect, latest_media_ids_from_provider, pending_scrapes, process_scrape,
        submit_webhook_responses, webhooks_for_provider,
    },
    models::PendingProvider,
    scraper::{scraper::scrape, AllProviders, PinterestBoardFeed, Provider, ScrapeRequestInput},
    webhook::dispatcher::dispatch_webhooks,
};
use reqwest::Client;
use sqlx::{Pool, Postgres};
use std::{error::Error, sync::Arc};

const PROVIDER_PROCESSING_LIMIT: usize = 8;

struct Context {
    db: Pool<Postgres>,
    client: Arc<reqwest::Client>,
}

fn get_provider(p: AllProviders, client: Arc<Client>) -> Box<impl Provider> {
    Box::new(match p {
        AllProviders::PinterestBoardFeed => PinterestBoardFeed { client },
        AllProviders::TwitterTimeline => PinterestBoardFeed { client },
    })
}

async fn iter(ctx: &Context, pending: PendingProvider) -> anyhow::Result<()> {
    let sp = pending.provider;
    let provider = get_provider(sp.name, Arc::clone(&ctx.client)); // ctx.providers[sp.name];
    let latest_data = dbg!(latest_media_ids_from_provider(&ctx.db, &sp).await?);

    let step = ScrapeRequestInput {
        latest_data,
        last_scrape: pending.last_scrape,
    };
    let result = scrape(&sp, &*provider, &step).await?;
    println!("Scraped");
    let processed_scrape = process_scrape(&ctx.db, &result).await?;
    println!("Processed scrape");
    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    println!("Got webhooks");
    let webhook_interactions = dispatch_webhooks(&result, webhooks).await;
    println!("Dispatched webhooks");
    submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions).await?;
    println!("Submitted webhook reasponse");
    Ok(())
}

async fn run() -> Result<(), Box<dyn Error>> {
    let db = connect().await?;
    let backing_client = Client::new();
    let client = Arc::new(backing_client);
    let pending_providers = dbg!(pending_scrapes(&db).await?);
    // let providers: EnumMap<AllProviders, Scrapable> = enum_map! {};
    let ctx = Context {
        db,
        client: Arc::clone(&client),
        // providers,
    };
    stream::iter(pending_providers)
        .for_each_concurrent(PROVIDER_PROCESSING_LIMIT, |sp| async {
            match iter(&ctx, sp).await {
                Ok(a) => {}
                Err(error) => {
                    println!("{:?}", error)
                }
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
