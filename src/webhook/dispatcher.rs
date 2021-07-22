use super::discord::*;
use crate::{
    models::DatabaseWebhook,
    scraper::{scraper::Scrape, ProviderMedia},
    webhook::{webhook_type, WebhookDestination},
};
use futures::{stream, StreamExt};
use reqwest::{Client, Response};
use std::{borrow::Borrow, cell::RefCell, rc::Rc, sync::Arc};

pub struct WebhookDispatch {
    pub webhook: DatabaseWebhook,
}

pub struct WebhookInteraction {
    url: String,
    response: Result<Response, reqwest::Error>,
}

const WEBHOOK_DISPATCH_CONCURRENCY_LIMIT: usize = 8;

pub async fn dispatch_webhooks(
    scrape: &Scrape,
    dispatch: Vec<DatabaseWebhook>,
) -> Vec<WebhookInteraction> {
    let client = &Client::new();
    // request results are not guaranteed to be in order
    let mut results: Vec<WebhookInteraction> = vec![];
    let media = &scrape
        .requests
        .iter()
        .flat_map(|req| &req.provider_result.images);
    let ref_cell = RefCell::new(&mut results);
    let iter = |webhook: DatabaseWebhook| async {
        let mut output = ref_cell.borrow_mut();
        let builder = client.post(&webhook.destination);
        let response = match webhook_type(&webhook.destination) {
            WebhookDestination::Custom => {
                builder
                    .header("content-type", "application/json")
                    // extra chunky unnecessary clone to get the borrow checker to stop complaining
                    // TODO: learn rust
                    .json(&media.clone().collect::<Vec<&ProviderMedia>>())
                    .send()
                    .await
            }
            // TODO: we should split discord webhooks into a separate stream and rate-limit it harder
            // to make sure the running ip doesn't get donked by discord
            WebhookDestination::Discord => {
                let payload = media
                    .clone()
                    .take(DISCORD_IMAGE_DISPLAY_LIMIT)
                    .collect::<Vec<&ProviderMedia>>();
                let body = discord_payload(&payload);
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
