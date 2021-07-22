use super::discord::*;
use crate::{
    models::DatabaseWebhook,
    scraper::{scraper::Scrape, ProviderMedia},
    webhook::{webhook_type, WebhookDestination},
};
use futures::{stream, StreamExt};
use reqwest::{Client, Response};
use serde::Serialize;
use std::{cell::RefCell, cmp::min};

pub struct WebhookDispatch {
    pub webhook: DatabaseWebhook,
}

#[derive(Debug)]
pub struct WebhookInteraction {
    url: String,
    response: Result<Response, reqwest::Error>,
}

#[derive(Debug, Serialize)]
pub struct WebhookPayload<'a> {
    pub provider_type: String,
    // what even
    pub media: &'a Vec<&'a ProviderMedia>,
    pub metadata: Option<serde_json::Value>,
}

const WEBHOOK_DISPATCH_CONCURRENCY_LIMIT: usize = 8;

pub async fn dispatch_webhooks(
    scrape: &Scrape,
    dispatch: Vec<DatabaseWebhook>,
) -> Vec<WebhookInteraction> {
    let client = &Client::new();
    // request results are not guaranteed to be in order
    let mut results: Vec<WebhookInteraction> = vec![];
    let media = scrape
        .requests
        .iter()
        .flat_map(|req| &req.provider_result.images)
        .collect::<Vec<&ProviderMedia>>();
    let discord_media = &media[0..min(media.len(), DISCORD_IMAGE_DISPLAY_LIMIT)];
    let ref_cell = RefCell::new(&mut results);
    let iter = |webhook: DatabaseWebhook| async {
        let mut output = ref_cell.borrow_mut();
        let builder = client.post(&webhook.destination);
        let response = match webhook_type(&webhook.destination) {
            WebhookDestination::Custom => {
                builder
                    .header("content-type", "application/json")
                    .json(&WebhookPayload {
                        provider_type: scrape.provider_id.clone(),
                        media: &media,
                        metadata: webhook.metadata,
                    })
                    .send()
                    .await
            }
            // TODO: we should split discord webhooks into a separate stream and rate-limit it harder
            // to make sure the running ip doesn't get donked by discord
            WebhookDestination::Discord => {
                let body = discord_payload(discord_media.to_vec());
                println!("{:?}", body);
                builder
                    .header("content-type", "multipart/form-data")
                    .form(&body)
                    .send()
                    .await
            }
        };
        let url = webhook.destination;
        output.push(WebhookInteraction { url, response });
    };

    stream::iter(dispatch)
        // sadly there's no `map_concurrent` for futures
        .for_each_concurrent(WEBHOOK_DISPATCH_CONCURRENCY_LIMIT, iter)
        .await;
    results
}
