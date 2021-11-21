use std::env;
use std::{error::Error, sync::Arc, time::Duration};

use actix_web;
// use actix_web;
use futures::future::join_all;
use log::{debug, error, info, trace};
use reqwest::Client;
use sqlx::{Pool, Postgres};

use jiu::dispatcher::amqp::AMQPDispatcher;
use jiu::dispatcher::dispatcher::{dispatch_webhooks, DispatchablePayload};
use jiu::{
    db::*,
    models::PendingProvider,
    scheduler::*,
    scraper::{get_provider_map, scraper::scrape, Provider, ProviderMap, ScrapeRequestInput},
};

struct Context {
    db: Arc<Pool<Postgres>>,
    amqp: Arc<Option<AMQPDispatcher>>,
    client: Arc<Client>,
}

async fn iter(
    ctx: Arc<Context>,
    pending: &PendingProvider,
    provider: &dyn Provider,
) -> anyhow::Result<()> {
    let sp = pending.provider.clone();
    let latest_data = latest_media_ids_from_provider(&ctx.db, &sp).await?;
    let _is_first_scrape = !latest_data.is_empty();
    let step = ScrapeRequestInput {
        latest_data,
        default_name: pending.default_name.clone(),
        last_scrape: pending.last_scrape,
    };
    let mut result = scrape(&sp, &*provider, &step).await?;

    let webhooks = webhooks_for_provider(&ctx.db, &sp).await?;
    let webhook_interactions = if !result.requests.is_empty() {
        let dispatch = webhooks
            .into_iter()
            .map(|wh| {
                let payload = DispatchablePayload::new(&*provider, &result, wh.metadata.clone());
                (wh, payload)
            })
            .collect::<Vec<_>>();
        // we don't really care about the interactions in amqp since we have full
        // control of that environment anyways
        if let Some(amqp) = &*ctx.amqp {
            if let Ok(amqp_d) = amqp_metadata(&*ctx.db, &sp).await {
                let payload =
                    DispatchablePayload::new(&*provider, &result, amqp_d.and_then(|r| r.metadata));
                trace!("Publishing AMQP message for {}", &provider.id().to_string());
                amqp.publish(&payload).await;
            }
        }
        Some(dispatch_webhooks(dispatch).await)
    } else {
        None
    };
    // process scraping MUST come after dispatcher dispatching since it mutates the array by reversing it
    let processed_scrape = process_scrape(&ctx.db, &mut result, &pending).await?;
    if webhook_interactions.is_some() {
        submit_webhook_responses(&ctx.db, processed_scrape, webhook_interactions.unwrap()).await?
    }
    Ok(())
}

async fn job_loop(ctx: Arc<Context>) {
    let provider_map = get_provider_map(&Arc::clone(&ctx.client))
        .await
        .expect("Could not successfully initialize a provider map");
    let arc_db = Arc::clone(&ctx.db);
    let pendings = match pending_scrapes(&arc_db).await {
        Err(error) => {
            println!("{:?}", error);
            return;
        }
        Ok(result) => result,
    };
    if let Some(err) = update_priorities(&arc_db, &pendings).await.err() {
        // should an error here be preventing the scrape?
        // Could end up spamming a provider if it's stuck at a high value
        error!("{:?}", err);
    };
    println!("{:?}", pendings);

    let this_scrape = pendings.iter().map(|p| Arc::new(p)).map(|pending| async {
        let pp = pending;
        let sleep_time = pp.scrape_date.clone();
        tokio::time::sleep(sleep_time).await;
        if let Err(err) = run(Arc::clone(&ctx), &pp, &provider_map).await {
            error!("{:?}", err);
            return;
        }
        debug!("Finished scraping {}", pp.provider.name.to_string());
        ()
    });
    join_all(this_scrape).await;
}

async fn run(
    ctx: Arc<Context>,
    pp: &PendingProvider,
    provider_map: &ProviderMap,
) -> Result<(), Box<dyn Error + Send>> {
    let provider = provider_map.get(&pp.provider.name).expect(&format!(
        "Tried to get a provider that doesn't exist {}",
        &pp.provider,
    ));
    match iter(Arc::clone(&ctx), &pp, &**provider).await {
        Err(err) => eprintln!("{:?}", err),
        Ok(_) => {}
    }
    Ok(())
}

async fn setup() -> anyhow::Result<()> {
    let db = Arc::new(connect().await?);
    let client = Arc::new(Client::new());
    let amqp = Arc::new(match env::var("AMQP_URL") {
        Ok(a) => {
            println!("{:?}", a);
            Some(AMQPDispatcher::from_connection_string(&a).await.unwrap())
        }
        Err(_) => None,
    });
    info!("Starting JiU");
    loop {
        info!("Starting job loop");
        let ctx = Arc::new(Context {
            db: Arc::clone(&db),
            amqp: Arc::clone(&amqp),
            client: Arc::clone(&client),
        });
        let data = tokio::task::spawn_local(async move {
            job_loop(ctx).await;
            info!("Requests finished for the day...");
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
