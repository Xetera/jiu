use crate::models::{DatabaseWebhook, PendingProvider};
use crate::request::HttpError;
use crate::scraper::scraper::{Scrape, ScraperStep};
use crate::scraper::{ProviderFailure, ScopedProvider};
use crate::webhook::dispatcher::WebhookInteraction;
use chrono::{DateTime, Offset, Utc};
use dotenv::dotenv;
use log::error;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Error, Pool, Postgres};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::env;
use std::iter::FromIterator;

type Database = Pool<Postgres>;

pub async fn connect() -> Result<Database, Error> {
    dotenv().ok();
    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL").expect("No DATABASE_URL env"))
        .await?)
}

// Grab the latest N images from a relevant provider destination
pub async fn latest_media_ids_from_provider(
    db: &Database,
    provider: &ScopedProvider,
) -> Result<HashSet<String>, sqlx::error::Error> {
    let out = sqlx::query!(
        "SELECT unique_identifier FROM media
        WHERE provider_name = $1 AND provider_destination = $2
        order by discovered_at desc, id limit 10",
        provider.name,
        provider.destination
    )
    .map(|e| e.unique_identifier)
    .fetch_all(db)
    .await?;
    Ok(HashSet::from_iter(out.into_iter()))
}

pub async fn webhooks_for_provider(
    db: &Database,
    provider_resolvable: &ScopedProvider,
) -> Result<Vec<DatabaseWebhook>, sqlx::error::Error> {
    sqlx::query_as!(
        DatabaseWebhook,
        "SELECT webhook.* FROM webhook
        JOIN webhook_source on webhook_source.webhook_id = webhook.id
        WHERE webhook_source.provider_destination = $1 AND webhook_source.provider_name = $2",
        provider_resolvable.destination,
        provider_resolvable.name
    )
    .fetch_all(db)
    .await
}

pub struct ProcessedScrape {
    scrape_id: i32,
}

pub async fn pending_scrapes(db: &Database) -> Result<Vec<PendingProvider>, Error> {
    sqlx::query!("SELECT * FROM provider_resource")
        .map(|row| PendingProvider {
            provider: ScopedProvider {
                destination: row.destination,
                name: row.name,
            },
            last_scrape: row.last_scrape.map(|r| DateTime::from_utc(r, Utc)),
        })
        .fetch_all(db)
        .await
}

pub async fn process_scrape<'a>(
    db: &Database,
    scrape: &Scrape<'a>,
) -> Result<ProcessedScrape, Error> {
    let mut tx = db.begin().await?;
    let out = sqlx::query!(
        "INSERT INTO scrape (provider_destination) VALUES ($1) returning id",
        scrape.provider.destination
    )
    .fetch_one(&mut tx)
    .await?;
    sqlx::query!(
        "UPDATE provider_resource SET last_scrape = $1 WHERE name = $2 AND destination = $3",
        // we don't really care about making sure this is completely correct
        scrape.requests.last().map(|out| out.date.naive_utc()),
        scrape.provider.name,
        scrape.provider.destination,
    )
    .fetch_one(db)
    .await?;
    let scrape_id = out.id;

    for (i, request) in scrape.requests.iter().enumerate() {
        match &request.step {
            ScraperStep::Data(provider_result) => {
                let response_code = provider_result.response_code.as_u16();
                let scrape_request_row = sqlx::query!(
                    "INSERT INTO scrape_request (scrape_id, response_code, response_delay, scraped_at, page)
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING id",
                    scrape_id,
                    response_code as u32,
                    // unsafe downcast from u128? I hope the request doesn't take 2 billion miliseconds kekw
                    provider_result.response_delay.as_millis() as u32,
                    request.date.naive_utc(),
                    i as u32
                ).fetch_one(&mut tx).await?;
                for media in provider_result.images.iter() {
                    let _media_row = sqlx::query!(
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
                        ON CONFLICT DO NOTHING returning *",
                        &scrape.provider.name,
                        &scrape.provider.destination,
                        scrape_request_row.id,
                        media.image_url,
                        media.page_url,
                        media.reference_url,
                        media.unique_identifier,
                        media.post_date.map(|date| date.naive_utc()),
                        request.date.naive_utc()
                    )
                    .fetch_optional(&mut tx)
                    .await?;
                }
            }
            ScraperStep::Error(ProviderFailure::HttpError(err)) => {
                match &err {
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
                                VALUES ($1, $2) returning id",
                                scrape_id,
                                status.as_u16() as i32,
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
                            "INSERT INTO scrape_error (scrape_id, response_code, response_body)
                            VALUES ($1, $2, $3) returning id",
                            scrape_id,
                            ctx.code.as_u16() as i32,
                            ctx.body,
                        )
                        .fetch_one(&mut tx)
                        .await?;
                    }
                }
            }
            ScraperStep::Error(ProviderFailure::Url) => {
                println!(
                    "Could not formal url properly for {}: {}",
                    scrape.provider.name, scrape.provider.destination
                );
            }
        }
    }
    tx.commit().await?;
    Ok(ProcessedScrape { scrape_id: out.id })
}

pub async fn submit_webhook_responses(
    db: &Database,
    processed_scrape: ProcessedScrape,
    interactions: Vec<WebhookInteraction>,
) -> Result<(), sqlx::Error> {
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
