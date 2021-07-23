use super::discord::*;
use crate::{
    models::DatabaseWebhook,
    request::{request_default_headers, HttpError},
    scraper::{scraper::ScraperStep, ProviderMedia, Scrape},
    webhook::{webhook_type, WebhookDestination},
};
use futures::{stream, StreamExt};
use log::error;
use reqwest::{Client, Response};
use serde::Serialize;
use std::{cmp::min, sync::RwLock, time::Instant};

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
pub struct WebhookPayload<'a> {
    pub provider_type: String,
    // what even
    pub media: &'a Vec<&'a ProviderMedia>,
    pub metadata: Option<serde_json::Value>,
}

const WEBHOOK_DISPATCH_CONCURRENCY_LIMIT: usize = 8;

pub async fn dispatch_webhooks<'a>(
    scrape: &Scrape<'a>,
    dispatch: Vec<DatabaseWebhook>,
) -> Vec<WebhookInteraction> {
    let client = &Client::new();
    // request results are not guaranteed to be in order
    let mut results: Vec<WebhookInteraction> = vec![];
    let media = scrape
        .requests
        .iter()
        .filter_map(|req| match &req.step {
            ScraperStep::Data(data) => Some(data),
            ScraperStep::Error(_) => None,
        })
        .flat_map(|result| &result.images)
        .collect::<Vec<&ProviderMedia>>();
    let discord_media = &media[0..min(media.len(), DISCORD_IMAGE_DISPLAY_LIMIT)];
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
                        provider_type: scrape.provider.name.clone(),
                        media: &media,
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
