use actix_web;
use futures::future::join_all;
use governor::{Jitter, Quota, RateLimiter};
use jiu::{
    db::{
        connect, latest_media_ids_from_provider, latest_requests, pending_scrapes, process_scrape,
        submit_webhook_responses, webhooks_for_provider, Database,
    },
    models::PendingProvider,
    scraper::{providers, scraper::scrape, Provider, ScrapeRequestInput},
    server::run_server,
    webhook::dispatcher::dispatch_webhooks,
};
use log::{debug, info};
use nonzero_ext::nonzero;
use reqwest::Client;
use sqlx::{Pool, Postgres};
use std::{error::Error, sync::Arc, time::Duration};

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

async fn job_loop(arc_db: Arc<Database>, client: Arc<Client>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1u64));
    loop {
        interval.tick().await;
        match run(Arc::clone(&arc_db), Arc::clone(&client)).await {}
        ()
    }
}

async fn run(arc_db: Arc<Database>, client: Arc<Client>) -> Result<(), Box<dyn Error + Send>> {
    // let latest = latest_requests(&*arc_db, true).await?;
    let pending_providers = pending_scrapes(&*arc_db).await?;
    debug!("Pending providers = {:?}", pending_providers);

    let ctx = Context {
        db: Arc::clone(&arc_db),
    };
    let provider_map = providers(client).await?;
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
    let client = Arc::new(Client::new());
    let data = tokio::task::spawn_local(run(Arc::clone(&db), Arc::clone(&client)));
    match run_server(Arc::clone(&db)).await {
        Err(err) => {
            eprintln!("{:?}", err);
        }
        Ok(()) => {}
    };
    match data.await {
        Err(err) => {
            eprintln!("{:?}", err);
        }
        Ok(Err(err)) => {
            eprintln!("{:?}", err);
        }
        _ => {}
    };
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
