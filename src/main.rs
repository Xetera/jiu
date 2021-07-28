use actix_web;
use futures::{future::join_all, stream, task, StreamExt};
use governor::{Jitter, Quota, RateLimiter};
use jiu::{
    db::{
        connect, latest_media_ids_from_provider, latest_requests, pending_scrapes, process_scrape,
        submit_webhook_responses, webhooks_for_provider, Database,
    },
    models::PendingProvider,
    scraper::{
        fetch_weverse_auth_token, scraper::scrape, AllProviders, PinterestBoardFeed, Provider,
        ProviderInput, ScrapeRequestInput, WeverseArtistFeed,
    },
    server::run_server,
    webhook::dispatcher::dispatch_webhooks,
};
use log::{debug, info};
use nonzero_ext::nonzero;
use reqwest::Client;
use sqlx::{Pool, Postgres};
use std::{collections::HashMap, error::Error, iter::FromIterator, sync::Arc, time::Duration};
use strum::IntoEnumIterator;

const PROVIDER_PROCESSING_LIMIT: u32 = 8;

struct Context {
    db: Arc<Pool<Postgres>>,
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

async fn run(arc_db: Arc<Database>) -> Result<(), Box<dyn Error + Send>> {
    // let arc_db = Arc::new(db);
    let backing_client = Client::new();
    let latest = latest_requests(&*arc_db, true).await?;
    println!("{:?}", latest);
    let client = Arc::new(backing_client);
    let access_token = fetch_weverse_auth_token(&client).await?;
    let pending_providers = pending_scrapes(&*arc_db).await?;
    debug!("Pending providers = {:?}", pending_providers);

    let ctx = Context {
        db: Arc::clone(&arc_db),
    };
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
    let rate_limiter = RateLimiter::direct(
        Quota::per_minute(nonzero!(60u32)).allow_burst(nonzero!(PROVIDER_PROCESSING_LIMIT)),
    );
    let futures = pending_providers.into_iter().map(|sp| async {
        info!("Waiting for rate limiter");
        rate_limiter
            .until_ready_with_jitter(Jitter::up_to(Duration::from_secs(4u64)))
            .await;
        let provider = provider_map.get(&sp.provider.name).expect(&format!(
            "Tried to get a provider that doesn't exist {}",
            &sp.provider,
        ));
        match iter(&ctx, sp, &**provider).await {
            Err(err) => eprintln!("{:?}", err),
            Ok(_) => {}
        }
        // info!("There are {} concurrent queries", rate_limiter.len())
    });
    join_all(futures).await;
    Ok(())
}

async fn setup() -> anyhow::Result<()> {
    let db = Arc::new(connect().await?);
    // let data = tokio::task::spawn_local(run(Arc::clone(&db)));
    match run_server(Arc::clone(&db)).await {
        Err(err) => {
            eprintln!("{:?}", err);
        }
        Ok(()) => {}
    };
    // match data.await {
    //     Err(err) => {
    //         eprintln!("{:?}", err);
    //     }
    //     Ok(Err(err)) => {
    //         eprintln!("{:?}", err);
    //     }
    //     _ => {}
    // };
    Ok(())
}

#[actix_web::main]
async fn main() {
    better_panic::install();
    env_logger::init();

    info!("Running program");
    setup().await;
    println!("Done!");
}
