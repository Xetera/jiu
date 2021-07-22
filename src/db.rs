use std::any::Any;
use std::env;

use crate::models::Media;
use crate::models::*;
use crate::scraper::scraper::ScrapeRequest;
use crate::scraper::{ProviderMedia, Providers};
use chrono::Utc;
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

// Grab the latest N images from a relevant provider
// pub async fn latest_media_from_provider(
//     db: &Pool<Postgres>,
//     provider_name: &Providers,
// ) -> Result<Vec<Media>, sqlx::error::Error> {
//     // let out = sqlx::query_as!("SELECT id FROM media limit 5")
//     //     .fetch_all(db)
//     //     .await?;
//     // Ok(out)
//     // println!("{:?}", out);
//     // Err(Error::PoolClosed)
//     // Ok(out)
//     // let a = out;

//     // media
//     //     .inner_join(provider_resource.on())
//     //     .filter(provider_resource_id.eq(provider_name))
//     //     .load::<Media>(db)
// }

pub async fn add_media(
    db: &Pool<Postgres>,
    provider_resource: &ProviderResource,
    scrape_request: &ScrapeRequest,
    images: Vec<ProviderMedia>,
) -> Result<Scrape, Error> {
    todo!();
    // let mut tx = db.begin().await?;
    // let out = sqlx::query_as!(
    //     Scrape,
    //     "INSERT INTO scrape (provider_resource_id) VALUES ($1) returning *",
    //     provider_resource.id
    // )
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
