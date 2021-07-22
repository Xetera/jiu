use crate::models::DatabaseWebhook;
use crate::scraper::scraper::Scrape;
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Error, Pool, Postgres};
use std::collections::HashSet;
use std::env;
use std::iter::FromIterator;

pub async fn connect() -> Result<Pool<Postgres>, Error> {
    dotenv().ok();
    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL").expect("No DATABASE_URL env"))
        .await?)
}

// Grab the latest N images from a relevant provider destination
pub async fn latest_media_ids_from_provider(
    db: &Pool<Postgres>,
    destination: &str,
) -> Result<HashSet<String>, sqlx::error::Error> {
    let out = sqlx::query!(
        "SELECT unique_identifier FROM media WHERE provider_destination = $1 order by discovered_at desc limit 10",
        destination
    )
    .map(|e| e.unique_identifier)
    .fetch_all(db)
    .await?;
    Ok(HashSet::from_iter(out.into_iter()))
}

pub async fn webhooks_for_provider(
    db: &Pool<Postgres>,
    destination: &str,
) -> Result<Vec<DatabaseWebhook>, sqlx::error::Error> {
    sqlx::query_as!(
        DatabaseWebhook,
        "SELECT webhook.* FROM webhook
        JOIN webhook_source on webhook_source.webhook_id = webhook.id
        WHERE webhook_source.provider_destination = $1",
        destination
    )
    .fetch_all(db)
    .await
}

pub async fn process_scrape(db: &Pool<Postgres>, scrape: &Scrape) -> Result<(), Error> {
    let provider_destination = &scrape.provider_destination;
    let mut tx = db.begin().await?;
    let out = sqlx::query!(
        "INSERT INTO scrape (provider_destination) VALUES ($1) returning id",
        &provider_destination
    )
    .fetch_one(&mut tx)
    .await?;
    let scrape_id = out.id;

    for (i, request) in scrape.requests.iter().enumerate() {
        let response_code = request.provider_result.response_code.as_u16();
        let scrape_request_row = sqlx::query!(
            "INSERT INTO scrape_request (scrape_id, response_code, response_delay, scraped_at, page) VALUES ($1, $2, $3, $4, $5) returning id",
            scrape_id,
            response_code as u32,
            // unsafe downcast from u128? I hope the request doesn't take 2 billion miliseconds kekw
            request.provider_result.response_delay.as_millis() as u32,
            request.date.naive_utc(),
            i as u32
        ).fetch_one(&mut tx).await?;
        for media in request.provider_result.images.iter() {
            let _media_row = sqlx::query!(
                "INSERT INTO media (
                provider_destination,
                scrape_request_id,
                image_url,
                page_url,
                reference_url,
                unique_identifier,
                posted_at,
                discovered_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT DO NOTHING returning *",
                &provider_destination,
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
    tx.commit().await?;
    Ok(())
}
