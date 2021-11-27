use std::collections::HashSet;
use std::env;
use std::iter::FromIterator;

use itertools::Itertools;
use log::error;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Error, Pool, Postgres};

use crate::dispatcher::dispatcher::WebhookInteraction;
use crate::models::{
    AMQPDestination, DatabaseWebhook, PendingProvider, ScrapeRequestMedia, ScrapeRequestWithMedia,
};
use crate::request::HttpError;
use crate::scraper::scraper::{Scrape, ScraperStep};
use crate::scraper::{ProviderFailure, ScopedProvider};

pub type Database = Pool<Postgres>;

pub async fn connect() -> Result<Database, Error> {
    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL").expect("No DATABASE_URL env"))
        .await?)
}

// Grab the latest N images from a relevant provider destination
pub async fn latest_media_ids_from_provider(
    db: &Database,
    provider: &ScopedProvider,
) -> anyhow::Result<HashSet<String>> {
    let out = sqlx::query!(
        "SELECT unique_identifier FROM media
        WHERE provider_name = $1 AND provider_destination = $2
        order by id desc, discovered_at desc limit 100",
        provider.name.to_string(),
        provider.destination
    )
    .map(|e| e.unique_identifier)
    .fetch_all(db)
    .await?;
    Ok(HashSet::from_iter(out.into_iter()))
}

pub async fn amqp_metadata(
    db: &Database,
    sp: &ScopedProvider,
) -> anyhow::Result<Option<AMQPDestination>> {
    let result = sqlx::query_as!(
        AMQPDestination,
        "SELECT id, metadata FROM amqp_source a WHERE a.provider_destination = $1 AND a.provider_name = $2 LIMIT 1",
        sp.destination,
        sp.name.to_string()
    ).fetch_optional(db).await?;
    Ok(result)
}

pub async fn webhooks_for_provider(
    db: &Database,
    provider_resolvable: &ScopedProvider,
) -> anyhow::Result<Vec<DatabaseWebhook>> {
    Ok(sqlx::query_as!(
        DatabaseWebhook,
        "SELECT webhook.*, webhook_source.metadata FROM webhook
        JOIN webhook_source on webhook_source.webhook_id = webhook.id
        WHERE webhook_source.provider_destination = $1 AND webhook_source.provider_name = $2",
        provider_resolvable.destination,
        provider_resolvable.name.to_string()
    )
    .fetch_all(db)
    .await?)
}

#[derive(Debug)]
pub struct ProcessedScrape {
    scrape_id: i32,
}

/// Adds scrapes to the db. Reverses the scrape list as a side effect
pub async fn process_scrape<'a>(
    db: &Database,
    scrape: &mut Scrape<'a>,
    pending: &PendingProvider,
) -> anyhow::Result<ProcessedScrape> {
    let mut tx = db.begin().await?;
    let out = sqlx::query!(
        "INSERT INTO scrape (provider_name, provider_destination, priority) VALUES ($1, $2, $3) returning id",
        scrape.provider.name.to_string(),
        scrape.provider.destination,
        pending.priority.level
    )
        .fetch_one(&mut tx)
        .await?;
    // we don't really care about making sure this is completely correct
    sqlx::query!(
        "UPDATE provider_resource
        SET
            last_scrape = NOW(),
            tokens = tokens - 1
        WHERE name = $1 AND destination = $2
        RETURNING *",
        scrape.provider.name.to_string(),
        scrape.provider.destination,
    )
    .fetch_one(db)
    .await?;
    let scrape_id = out.id;
    let requests = &mut scrape.requests;
    // we specifically need to reverse this list of requests/images
    // to make sure that the images that were first scraped get inserted
    // last with the highest id
    requests.reverse();

    for (i, request) in requests.iter().enumerate() {
        match &request.step {
            ScraperStep::Data(provider_result) => {
                let response_code = provider_result.response_code.as_u16();
                let scrape_request_row = sqlx::query!(
                    "INSERT INTO scrape_request (scrape_id, response_code, response_delay, scraped_at, page)
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING id",
                    scrape_id,
                    response_code as u32,
                    // unsafe downcast from u128? I hope the request doesn't take 2 billion milliseconds kekw
                    provider_result.response_delay.as_millis() as u32,
                    request.date,
                    // pages are 1-indexed
                    (i as i32) + 1
                ).fetch_one(&mut tx).await?;
                // we're not persisting post data, but that's ok
                let mut posts = provider_result.posts.clone();
                posts.reverse();
                for post in posts {
                    let mut images = post.images.clone();
                    images.reverse();
                    for media in images.iter() {
                        sqlx::query!(
                            "INSERT INTO media (
                            provider_name,
                            provider_destination,
                            scrape_request_id,
                            image_url,
                            page_url,
                            reference_url,
                            unique_identifier,
                            posted_at,
                            discovered_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                        ON CONFLICT (image_url) DO update set discovered_at = NOW() returning *",
                            // sometimes we end up re-scraping the latest known images
                            &scrape.provider.name.to_string(),
                            &scrape.provider.destination,
                            scrape_request_row.id,
                            media.media_url,
                            post.url,
                            media.reference_url,
                            media.unique_identifier,
                            post.post_date,
                            request.date
                        )
                        .fetch_optional(&mut tx)
                        .await?;
                    }
                }
            }
            ScraperStep::Error(ProviderFailure::HttpError(error)) => {
                match &error {
                    HttpError::ReqwestError(err) => {
                        // we should not be getting request related errors, only response errors
                        if err.is_request() {
                            error!(
                                "Got an error from a provider that was caused by a request\n{:?}",
                                err.url()
                            );
                            error!("{:?}", err);
                            continue;
                        }

                        if let Some(status) = err.status() {
                            sqlx::query!(
                                "INSERT INTO scrape_error (scrape_id, response_code)
                                VALUES ($1, $2)",
                                scrape_id,
                                status.as_u16() as i32
                            )
                            .fetch_one(&mut tx)
                            .await?;
                        } else {
                            error!("Got an unexpected error from a provider that doesn't have a status",);
                            error!("{:?}", err);
                            continue;
                        }
                    }
                    HttpError::FailStatus(ctx) | HttpError::UnexpectedBody(ctx) => {
                        sqlx::query!(
                            "INSERT INTO scrape_error (scrape_id, response_code, response_body, message)
                            VALUES ($1, $2, $3, $4) returning id",
                            scrape_id,
                            ctx.code.as_u16() as i32,
                            ctx.body,
                            ctx.message,
                        )
                        .fetch_one(&mut tx)
                        .await?;
                    }
                }
            }
            ScraperStep::Error(ProviderFailure::Url) => {
                println!(
                    "Could not formal url properly for {}: {}",
                    scrape.provider.name.to_string(),
                    scrape.provider.destination
                );
            }
            _ => {}
        }
    }
    tx.commit().await?;
    Ok(ProcessedScrape { scrape_id: out.id })
}

pub async fn submit_webhook_responses(
    db: &Database,
    processed_scrape: ProcessedScrape,
    interactions: Vec<WebhookInteraction>,
) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    // can't commit the invocation if we don't have a response status
    for interaction in interactions {
        let response_time = interaction.response_time.as_millis() as i32;
        let response = interaction.response;
        let status = match response {
            Ok(res) => Some(res.status()),
            Err(HttpError::UnexpectedBody(err)) | Err(HttpError::FailStatus(err)) => Some(err.code),
            Err(HttpError::ReqwestError(err)) => {
                let out = err.status();
                if out.is_none() {
                    println!("Received a response without a status code");
                    eprintln!("{:?}", err);
                }
                out
            }
        };
        if let Some(code) = status {
            sqlx::query!(
                "INSERT INTO webhook_invocation (
                    scrape_id,
                    webhook_id,
                    response_code,
                    response_delay
                ) VALUES ($1, $2, $3, $4) RETURNING *",
                processed_scrape.scrape_id,
                interaction.webhook.id,
                code.as_u16() as i32,
                response_time
            )
            .fetch_one(&mut tx)
            .await?;
        } else {
            println!(
                "Failed to persist webhook response from {}",
                interaction.webhook.destination
            )
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn latest_requests(
    db: &Database,
    _only_with_media: bool,
) -> anyhow::Result<Vec<ScrapeRequestWithMedia>> {
    let results = sqlx::query!(
        "select
                sr.id as scrape_request_id,
                s.id as scrape_id,
                pr.name,
                sr.response_delay,
                sr.response_code,
                sr.scraped_at,
                pr.url
            from scrape_request sr
            join scrape s
                on s.id = sr.scrape_id
            join provider_resource pr
                on pr.name = s.provider_name and pr.destination = s.provider_destination
            ORDER BY sr.scraped_at desc
            LIMIT 50",
    )
    .fetch_all(db)
    .await?;
    let scrape_ids = results
        .iter()
        .unique_by(|rec| rec.scrape_id)
        .map(|rec| rec.scrape_id)
        .collect::<Vec<i32>>();
    // we're using scrape_id and not scrape_request_id because users only care about individual scrapes and not requests
    let medias = sqlx::query!(
        "SELECT sr.scrape_id, scrape_request_id, page_url, image_url
        FROM media m
        join scrape_request sr
            on sr.id = m.scrape_request_id
        join scrape s
            on s.id = sr.scrape_id
        where s.id = ANY($1)",
        &scrape_ids
    )
    .fetch_all(db)
    .await?;

    let media_map = medias
        .into_iter()
        .filter(|rec| rec.scrape_id.is_some() && rec.image_url.is_some())
        .into_group_map_by(|rec| rec.scrape_id.unwrap());

    let mut out: Vec<ScrapeRequestWithMedia> = vec![];
    for result in results {
        out.push(ScrapeRequestWithMedia {
            response_code: result.response_code,
            response_delay: result.response_delay,
            provider_name: result.name.clone(),
            url: result.url.clone(),
            date: result.scraped_at,
            media: media_map
                .get(&result.scrape_id)
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|m| {
                    if m.scrape_id.unwrap() == result.scrape_id {
                        Some(ScrapeRequestMedia {
                            media_url: m.image_url.clone().unwrap(),
                            page_url: m.page_url.clone().unwrap(),
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<ScrapeRequestMedia>>(),
        })
    }
    Ok(out)
}
