use std::any::Any;
use std::collections::HashSet;
use std::env;
use std::iter::FromIterator;

use crate::models::Media;
use crate::models::*;
use crate::scraper::scraper::{Scrape, ScrapeRequest};
use crate::scraper::{Provider, ProviderMedia};
use chrono::{NaiveDateTime, Utc};
use dotenv::dotenv;
use futures::{StreamExt, TryStreamExt};
use serde_json;
use sqlx::pool::PoolOptions;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Error, Pool, Postgres};

const DATABASE_NAME: &'static str = "jiu";
const MEDIA_COLLECTION_NAME: &'static str = "media";
const LATEST_IMAGE_CHECK_SIZE: i64 = 5;

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
        "SELECT unique_identifier FROM media WHERE provider_destination = $1 order by discovered_at desc limit 5",
        destination
    )
    .map(|e| e.unique_identifier)
    .fetch_all(db)
    .await?;
    Ok(HashSet::from_iter(out.into_iter()))
}

pub async fn process_scrape(db: &Pool<Postgres>, scrape: &Scrape) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    let out = sqlx::query!(
        "INSERT INTO scrape (provider_destination) VALUES ($1) returning id",
        scrape.provider_destination
    )
    .fetch_one(&mut tx)
    .await?;
    let scrape_id = out.id;

    for (i, request) in scrape.requests.iter().enumerate() {
        let response_code = request.provider_result.response_code.as_u16();
        let out = sqlx::query!(
            "INSERT INTO scrape_request (scrape_id, response_code, response_delay, scraped_at, page) VALUES ($1, $2, $3, $4, $5) returning *",
            scrape_id,
            response_code as u32,
            // unsafe downcast from u128? I hope the request doesn't take 2 billion miliseconds kekw
            request.provider_result.response_delay.as_millis() as u32,
            request.date.naive_utc(),
            i as u32
        ).fetch_one(&mut tx).await?;
    }
    tx.commit().await?;
    Ok(())
    // out.
    // todo!()
    // .fetch_one(&mut tx)
    // .await?;
    // Ok(out)
    // let media = db.collection::<Media>(MEDIA_COLLECTION_NAME);
    // let options = InsertManyOptions::default();
    // let out = images
    //     .into_iter()
    //     .map(|scraped| Media::new(scraped, provider.clone()));
    // media.insert_many(out, options).await
}
