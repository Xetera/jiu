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
    scraper::{get_provider_map, scraper::scrape, Provider, ProviderMap, ScrapeRequestInput},
    webhook::dispatcher::dispatch_webhooks,
};
use tokio::join;

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
    let mut result = scrape(&sp, &*provider, &step).await?;

    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    let webhook_interactions = dispatch_webhooks(&*provider, &result, webhooks).await;
    // process scraping MUST come after webhook dispatching since it mutates the array by reversing it
    let processed_scrape = process_scrape(&ctx.db, &mut result, &pending).await?;
    submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions).await?;
    Ok(())
}

async fn job_loop(arc_db: &Arc<Database>, client: &Arc<Client>) {
    let provider_map = get_provider_map(&Arc::clone(&client))
        .await
        .expect("Could not successfully initialize a provider map");
    let running_providers: RwLock<RunningProviders> = RwLock::new(HashSet::default());
    let pendings = match pending_scrapes(&arc_db).await {
        Err(error) => {
            println!("{:?}", error);
            return;
        }
        Ok(result) => result,
    };
    println!("{:?}", pendings);
    return;
    if let Some(err) = update_priorities(&arc_db, &pendings).await.err() {
        // should an error here be preventing the scrape?
        // Could end up spamming a provider if it's stuck at a high value
        error!("{:?}", err);
    };
    // trace!("pending = {:?}", pending);
    let this_scrape = pendings.iter().map(|p| Arc::new(p)).map(|pending| async {
        let pp = pending;
        tokio::time::sleep(pp.scrape_date.clone()).await;
        // let scheduled = filter_scheduled(&pending);
        // if scheduled.len() == 0 {
        //     trace!("No providers waiting to be staged");
        //     continue;
        // }
        // trace!("scheduled = {:?}", scheduled);
        // TODO: this should be happening at the end of the scrape, not start
        if let Err(err) = mark_as_scheduled(&arc_db, &pp, &running_providers).await {
            error!("{:?}", err);
            return;
        };
        if let Err(err) = run(Arc::clone(&arc_db), &pp, &provider_map).await {
            error!("{:?}", err);
            return;
        }
        debug!("Finished scraping {}", pp.id);
        running_providers.write().remove(&pp.provider);
        ()
    });
    join_all(this_scrape).await;
}

async fn run(
    arc_db: Arc<Database>,
    pp: &PendingProvider,
    provider_map: &ProviderMap,
) -> Result<(), Box<dyn Error + Send>> {
    // let providers = pending_providers.providers();
    // // let latest = latest_requests(&*arc_db, true).await?;
    // // let pending_providers = pending_scrapes(&*arc_db).await?;
    // debug!("Pending providers = {:?}", providers);

    let ctx = &Arc::new(Context {
        db: Arc::clone(&arc_db),
    });
    let provider = provider_map.get(&pp.provider.name).expect(&format!(
        "Tried to get a provider that doesn't exist {}",
        &pp.provider,
    ));
    match iter(Arc::clone(ctx), &pp, &**provider).await {
        Err(err) => eprintln!("{:?}", err),
        Ok(_) => {}
    }
    Ok(())
}

async fn setup() -> anyhow::Result<()> {
    let db = Arc::new(connect().await?);
    let client = Arc::new(Client::new());
    info!("Starting JiU");
    loop {
        info!("Starting job loop");
        let d = Arc::clone(&db);
        let c = Arc::clone(&client);
        let data = tokio::task::spawn_local(async move {
            job_loop(&d, &c).await;
            info!("Requests finished...");
        });
        let delay = tokio::task::spawn(tokio::time::sleep(Duration::from_millis(8.64e7 as u64)));
        if let (_, Err(join_err)) = tokio::join!(delay, data) {
            println!("{:?}", join_err)
        }
        info!("Finished job loop");
    }
    // if let Err(err) = run_server(Arc::clone(&db)).await {
    //     error!("Error with the webserver");
    //     eprintln!("{:?}", err);
    // };
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
