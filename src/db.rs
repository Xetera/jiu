use crate::models::Media;
use crate::scraper::{Providers, ScrapedMedia};
use futures::TryStreamExt;
use mongodb::error::{self, Error};
use mongodb::options::InsertManyOptions;
use mongodb::results::InsertManyResult;
use mongodb::{bson, Database};
use mongodb::{bson::doc, options::FindOptions};
use mongodb::{options::ClientOptions, Client};
use serde_json;

const DATABASE_NAME: &'static str = "jiu";
const MEDIA_COLLECTION_NAME: &'static str = "media";
const LATEST_IMAGE_CHECK_SIZE: i64 = 5;

pub async fn connect(url: &str) -> Result<mongodb::Database, Error> {
    let client = Client::with_options(ClientOptions::parse(url).await?)?;
    Ok(client.database(DATABASE_NAME))
}

/// Grab the latest N images from a relevant provider
pub async fn latest_media_from_provider(
    db: &Database,
    provider: &Providers,
) -> error::Result<Vec<Media>> {
    let media = db.collection::<Media>(MEDIA_COLLECTION_NAME);
    let options = FindOptions::builder()
        .sort(doc! { "data.discovered_at": 1, "_id": 1 })
        .limit(LATEST_IMAGE_CHECK_SIZE)
        .build();

    media
        .find(
            doc! { "provider": bson::to_bson(provider).unwrap() },
            options,
        )
        .await?
        .try_collect::<Vec<Media>>()
        .await
}

pub async fn add_media(
    db: &Database,
    provider: &Providers,
    images: Vec<ScrapedMedia>,
) -> error::Result<InsertManyResult> {
    let media = db.collection::<Media>(MEDIA_COLLECTION_NAME);
    let options = InsertManyOptions::default();
    let out = images
        .into_iter()
        .map(|scraped| Media::new(scraped, provider.clone()));
    media.insert_many(out, options).await
}
