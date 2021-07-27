use futures::{stream, StreamExt};
use jiu::{
    db::{
        connect, latest_media_ids_from_provider, pending_scrapes, process_scrape,
        submit_webhook_responses, webhooks_for_provider,
    },
    models::PendingProvider,
    scraper::{
        fetch_weverse_auth_token, scraper::scrape, AllProviders, PinterestBoardFeed, Provider,
        ProviderInput, ScrapeRequestInput, WeverseArtistFeed,
    },
    webhook::dispatcher::dispatch_webhooks,
};
use lazy_static::lazy_static;
use log::{debug, info};
use reqwest::Client;
use sqlx::{Pool, Postgres};
use std::{collections::HashMap, error::Error, iter::FromIterator, sync::Arc};
use strum::IntoEnumIterator;

const PROVIDER_PROCESSING_LIMIT: usize = 8;

lazy_static! {
    static ref WEVERSE_ACCESS_TOKEN: Option<String> = None;
}
struct Context {
    db: Pool<Postgres>,
}

async fn iter(
    ctx: &Context,
    pending: PendingProvider,
    provider: &dyn Provider,
) -> anyhow::Result<()> {
    let sp = pending.provider;
    let latest_data = latest_media_ids_from_provider(&ctx.db, &sp).await?;
    debug!("Providers being scraped: {:?}", latest_data);
    let step = ScrapeRequestInput {
        latest_data,
        last_scrape: pending.last_scrape,
    };
    let result = scrape(&sp, &*provider, &step).await?;
    info!("Scraped");
    let processed_scrape = process_scrape(&ctx.db, &result).await?;

    info!("Processed scrape");
    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    info!("Got webhooks");
    let webhook_interactions = dispatch_webhooks(&result, webhooks).await;
    info!("Dispatched webhooks");
    submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions).await?;
    info!("Submitted webhook responses");
    Ok(())
}

async fn run() -> Result<(), Box<dyn Error>> {
    let db = connect().await?;
    let backing_client = Client::new();
    let client = Arc::new(backing_client);
    let access_token = fetch_weverse_auth_token(&client).await?;
    let pending_providers = pending_scrapes(&db).await?;
    debug!("Pending providers = {:?}", pending_providers);

    let ctx = Context { db };
    let provider_map: HashMap<AllProviders, Box<dyn Provider>> =
        HashMap::from_iter(AllProviders::iter().map(|provider_type| {
            let client = Arc::clone(&client);
            let input = ProviderInput {
                client,
                access_token: match provider_type {
                    AllProviders::WeverseArtistFeed => access_token.clone(),
                    _ => None,
                },
            };
            let provider: Box<dyn Provider> = match &provider_type {
                AllProviders::PinterestBoardFeed => Box::new(PinterestBoardFeed::new(input)),
                AllProviders::WeverseArtistFeed => Box::new(WeverseArtistFeed::new(input)),
            };
            (provider_type, provider)
        }));
    stream::iter(pending_providers)
        .for_each_concurrent(PROVIDER_PROCESSING_LIMIT, |sp| async {
            let provider = provider_map.get(&sp.provider.name).expect(&format!(
                "Tried to get a provider that doesn't exist {}",
                &sp.provider,
            ));
            match iter(&ctx, sp, &**provider).await {
                Err(err) => eprintln!("{:?}", err),
                Ok(_) => {}
            }
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
