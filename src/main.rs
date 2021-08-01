use actix_web;
use futures::future::join_all;
use governor::{Jitter, Quota, RateLimiter};
use jiu::{
    db::{
        connect, latest_media_ids_from_provider, process_scrape, submit_webhook_responses,
        webhooks_for_provider, Database,
    },
    models::PendingProvider,
    scheduler::{mark_as_scheduled, pending_scrapes, RunningProviders},
    scraper::{providers, scraper::scrape, Provider, ScrapeRequestInput},
    server::run_server,
    webhook::dispatcher::dispatch_webhooks,
};
use log::{debug, error, info};
use nonzero_ext::nonzero;
use parking_lot::RwLock;
use reqwest::Client;
use sqlx::{Pool, Postgres};
use std::{collections::HashSet, error::Error, sync::Arc, time::Duration};

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
    let step = ScrapeRequestInput {
        latest_data,
        last_scrape: pending.last_scrape,
    };
    let result = scrape(&sp, &*provider, &step).await?;
    let processed_scrape = process_scrape(&ctx.db, &result).await?;

    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    let webhook_interactions = dispatch_webhooks(&result, webhooks).await;
    submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions).await?;
    Ok(())
}

async fn job_loop(arc_db: Arc<Database>, client: Arc<Client>) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let running_providers: RwLock<RunningProviders> = RwLock::new(HashSet::default());
    loop {
        interval.tick().await;
        match pending_scrapes(&arc_db, &running_providers).await {
            Ok(pending) => {
                if let Err(err) = mark_as_scheduled(&arc_db, &pending, &running_providers).await {
                    error!("{:?}", err);
                    continue;
                };
                if let Err(err) = run(Arc::clone(&arc_db), Arc::clone(&client), pending).await {
                    error!("{:?}", err);
                    continue;
                }
            }
            _ => {}
        };
        ()
    }
}

async fn run(
    arc_db: Arc<Database>,
    client: Arc<Client>,
    pending_providers: Vec<PendingProvider>,
) -> Result<(), Box<dyn Error + Send>> {
    // let latest = latest_requests(&*arc_db, true).await?;
    // let pending_providers = pending_scrapes(&*arc_db).await?;
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
    let data = tokio::task::spawn_local(job_loop(Arc::clone(&db), Arc::clone(&client)));
    match run_server(Arc::clone(&db)).await {
        Err(err) => {
            eprintln!("{:?}", err);
        }
        Ok(()) => {}
    };
    // infinite
    data.await;
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
