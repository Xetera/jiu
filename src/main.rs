use std::{collections::HashSet, error::Error, sync::Arc, time::Duration};

use actix_web;
use futures::future::join_all;
use log::{debug, error, info, trace};
use parking_lot::RwLock;
use reqwest::Client;
use sqlx::{Pool, Postgres};

use jiu::{
    db::*,
    models::PendingProvider,
    scheduler::*,
    scraper::{get_provider_map, Provider, ProviderMap, scraper::scrape, ScrapeRequestInput},
    webhook::dispatcher::dispatch_webhooks,
};

struct Context {
    db: Arc<Pool<Postgres>>,
}

async fn iter(
    ctx: Arc<Context>,
    pending: &PendingProvider,
    provider: &dyn Provider,
) -> anyhow::Result<()> {
    let sp = pending.provider.clone();
    let latest_data = latest_media_ids_from_provider(&ctx.db, &sp).await?;
    let step = ScrapeRequestInput {
        latest_data,
        last_scrape: pending.last_scrape,
    };
    let result = scrape(&sp, &*provider, &step).await?;
    let processed_scrape = process_scrape(&ctx.db, &result, &pending).await?;

    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    let webhook_interactions = dispatch_webhooks(&result, webhooks).await;
    submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions).await?;
    Ok(())
}

async fn job_loop(arc_db: Arc<Database>, client: Arc<Client>) {
    let scrape_duration = if cfg!(debug_assertions) {
        Duration::from_secs(10)
    } else {
        // release code should not be scraping as often as debug
        Duration::from_secs(120)
    };
    let mut interval = tokio::time::interval(scrape_duration);
    let provider_map = get_provider_map(&Arc::clone(&client))
        .await
        .expect("Could not successfully initialize a provider map");
    let running_providers: RwLock<RunningProviders> = RwLock::new(HashSet::default());
    loop {
        interval.tick().await;
        let pending = match pending_scrapes(&arc_db, &running_providers).await {
            Err(error) => {
                println!("{:?}", error);
                continue;
            }
            Ok(result) => result,
        };
        trace!("pending = {:?}", pending);
        let scheduled = filter_scheduled(pending);
        trace!("scheduled = {:?}", scheduled);
        if scheduled.len() == 0 {
            trace!("No providers waiting to be staged");
            continue;
        }
        // TODO: this should be happening at the end of the scrape, not start
        if let Some(err) = update_priorities(&arc_db, &scheduled).await.err() {
            // should an error here be preventing the scrape?
            // Could end up spamming a provider if it's stuck at a high value
            error!("{:?}", err);
        };
        if let Err(err) = mark_as_scheduled(&arc_db, &scheduled, &running_providers).await {
            error!("{:?}", err);
            continue;
        };
        if let Err(err) = run(Arc::clone(&arc_db), &scheduled, &provider_map).await {
            error!("{:?}", err);
            continue;
        }
        debug!(
            "Finished scraping {} providers",
            scheduled.providers().len()
        );
        for provider in scheduled.providers() {
            running_providers.write().remove(&provider.provider);
        }
        ()
    }
}

async fn run(
    arc_db: Arc<Database>,
    pending_providers: &ScheduledProviders,
    provider_map: &ProviderMap,
) -> Result<(), Box<dyn Error + Send>> {
    let providers = pending_providers.providers();
    // let latest = latest_requests(&*arc_db, true).await?;
    // let pending_providers = pending_scrapes(&*arc_db).await?;
    debug!("Pending providers = {:?}", providers);

    let ctx = &Arc::new(Context {
        db: Arc::clone(&arc_db),
    });
    let rate_limiter = &Arc::new(global_rate_limiter());
    let futures = providers.into_iter().map(|sp| async move {
        info!("Waiting for rate limiter");
        wait_provider_turn(Arc::clone(rate_limiter)).await;
        let provider = provider_map.get(&sp.provider.name).expect(&format!(
            "Tried to get a provider that doesn't exist {}",
            &sp.provider,
        ));
        match iter(Arc::clone(ctx), &sp.clone(), &**provider).await {
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
    // if let Err(err) = run_server(Arc::clone(&db)).await {
    //     error!("Error with the webserver");
    //     eprintln!("{:?}", err);
    // };
    data.await?;
    Ok(())
}

#[actix_web::main]
async fn main() {
    better_panic::install();
    env_logger::init();

    info!("Running program");
    if let Err(err) = setup().await {
        error!("{:?}", err);
    };
    info!("Shutting down...")
}
