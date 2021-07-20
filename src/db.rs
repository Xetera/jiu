use std::ascii::AsciiExt;
use std::collections::HashMap;
use std::error::Error;
use std::iter::FromIterator;
use std::time::Duration;

use crate::models::Media;
use crate::scraper::{Providers, ScrapedMedia};
use chrono::Utc;
use futures::TryStreamExt;
use serde_json;

const DATABASE_NAME: &'static str = "jiu";
const MEDIA_COLLECTION_NAME: &'static str = "media";
const LATEST_IMAGE_CHECK_SIZE: i64 = 5;

pub async fn connect() -> Result<DynamoDbClient, Box<dyn Error>> {
    // let mut provider = ChainProvider::new();
    // provider.set_timeout(Duration::from_millis(200));

    // let client = rusoto_dynamodb::DynamoDbClient::new_with(
    //     HttpClient::new().expect("Couldn't initialize http client"),
    //     provider,
    //     Region::EuCentral1,
    // );
    // Ok(client)
}

// Grab the latest N images from a relevant provider
pub async fn latest_media_from_provider(
    db: &DynamoDbClient,
    provider: &Providers,
) -> error::Result<Vec<Media>> {
    // let opts: BatchGetItemInput = Default::default();
    // let scan: ScanInput = Default::default();
    // scan.table_name = MEDIA_COLLECTION_NAME.to_owned();
    // scan.scan_filter = "provider = "
    // db.scan().table_name();
    // opts.request_items = HashMap::from_iter([("provider".to_owned(), provider)]);
    // let result = db.batch_get_item(opts).await?;
    // let media = db.collection::<Media>(MEDIA_COLLECTION_NAME);
    // let options = FindOptions::builder()
    //     .sort(doc! { "data.discovered_at": 1, "_id": 1 })
    //     .limit(LATEST_IMAGE_CHECK_SIZE)
    //     .build();

    // media
    //     .find(
    //         doc! { "provider": bson::to_bson(provider).unwrap() },
    //         options,
    //     )
    //     .await?
    //     .try_collect::<Vec<Media>>()
    //     .await
}

// pub async fn add_media(
//     db: &Database,
//     provider: &Providers,
//     images: Vec<ScrapedMedia>,
// ) -> error::Result<InsertManyResult> {
//     let media = db.collection::<Media>(MEDIA_COLLECTION_NAME);
//     let options = InsertManyOptions::default();
//     let out = images
//         .into_iter()
//         .map(|scraped| Media::new(scraped, provider.clone()));
//     media.insert_many(out, options).await
// }
