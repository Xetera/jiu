use std::{cmp::min, sync::RwLock, time::Instant};

use futures::{stream, StreamExt};
use log::error;
use reqwest::{Client, Response};
use serde::Serialize;

use crate::{
    models::DatabaseWebhook,
    request::{HttpError, request_default_headers},
    scraper::{
        AllProviders,
        ProviderMedia, ProviderPost, scraper::{Scrape, ScraperStep},
    },
    webhook::{webhook_type, WebhookDestination},
};

use super::discord::*;
use super::super::scraper::Provider;

pub struct WebhookDispatch {
    pub webhook: DatabaseWebhook,
}

#[derive(Debug)]
pub struct WebhookInteraction {
    pub webhook: DatabaseWebhook,
    pub response: Result<Response, HttpError>,
    pub response_time: std::time::Duration,
}

#[derive(Debug, Serialize)]
pub struct WebhookProvider {
    #[serde(rename = "type")]
    pub _type: AllProviders,
    pub id: String,
    pub ephemeral: bool,
}

#[derive(Debug, Serialize)]
pub struct WebhookPayload<'a> {
    pub provider: WebhookProvider,
    // what even
    pub posts: &'a Vec<&'a ProviderPost>,
    pub metadata: Option<serde_json::Value>,
}

const WEBHOOK_DISPATCH_CONCURRENCY_LIMIT: usize = 8;

pub async fn dispatch_webhooks<'a>(
    provider: &dyn Provider,
    scrape: &Scrape<'a>,
    dispatch: Vec<DatabaseWebhook>,
) -> Vec<WebhookInteraction> {
    let client = &Client::new();
    // request results are not guaranteed to be in order
    let mut results: Vec<WebhookInteraction> = vec![];
    let posts = scrape
        .requests
        .iter()
        .filter_map(|req| match &req.step {
            ScraperStep::Data(data) => Some(data),
            ScraperStep::Error(_) => None,
        })
        .flat_map(|result| &result.posts)
        .collect::<Vec<&ProviderPost>>();
    let image_length =posts.iter().flat_map(|p| &p.images).collect::<Vec<_>>().len();
    let discord_media = &posts[0..min(image_length, DISCORD_IMAGE_DISPLAY_LIMIT)];
    let ref_cell = RwLock::new(&mut results);
    let iter = |webhook: DatabaseWebhook| async {
        let builder = client
            .post(&webhook.destination)
            .headers(request_default_headers());
        let instant = Instant::now();
        let response = match webhook_type(&webhook.destination) {
            WebhookDestination::Custom => {
                builder
                    .json(&WebhookPayload {
                        provider: WebhookProvider {
                            _type: scrape.provider.name.clone(),
                            id: scrape.provider.destination.clone(),
                            ephemeral: provider.ephemeral(),
                        },
                        posts: &posts,
                        metadata: webhook.metadata.clone(),
                    })
                    .send()
                    .await
            }
            // TODO: we should split discord webhooks into a separate stream and rate-limit it harder
            // to make sure the running ip doesn't get donked by discord
            WebhookDestination::Discord => {
                let body = discord_payload(discord_media.to_vec());
                match add_wait_parameter(&webhook.destination) {
                    Ok(url) => client
                        .post(url.as_str())
                        .json(&body)
                        .send()
                        .await
                        .and_then(|res| res.error_for_status()),
                    Err(error) => {
                        error!("{:?}", error);
                        panic!()
                    }
                }
            }
        }
            .map_err(|err| HttpError::ReqwestError(err));
        let response_time = instant.elapsed();
        ref_cell.write().unwrap().push(WebhookInteraction {
            webhook,
            response,
            response_time,
        });
    };

    stream::iter(dispatch)
        // sadly there's no `map_concurrent` for futures
        .for_each_concurrent(WEBHOOK_DISPATCH_CONCURRENCY_LIMIT, iter)
        .await;
    results
}
