use futures::{stream, StreamExt};
use reqwest::{Client, Response};
use serde::Serialize;
use std::{sync::RwLock, time::Instant};

use crate::{
    dispatcher::{webhook_type, WebhookDestination},
    models::DatabaseWebhook,
    request::{request_default_headers, HttpError},
    scraper::{
        scraper::{Scrape, ScraperStep},
        AllProviders, ProviderMedia, ProviderPost,
    },
};

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

#[derive(Debug, Serialize, Clone)]
pub struct DispatchablePayloadProviderInfo {
    #[serde(rename = "type")]
    pub _type: AllProviders,
    pub id: String,
    pub ephemeral: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct DispatchablePayload {
    pub provider: DispatchablePayloadProviderInfo,
    // what even
    pub posts: Vec<ProviderPost>,
    pub metadata: Option<serde_json::Value>,
}

impl DispatchablePayload {
    pub fn new(
        provider: &dyn Provider,
        scrape: &Scrape,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        let posts = scrape
            .requests
            .iter()
            .filter_map(|req| match &req.step {
                ScraperStep::Data(data) => Some(data),
                ScraperStep::Error(_) => None,
            })
            .flat_map(|result| result.posts.clone())
            .collect::<Vec<_>>();
        DispatchablePayload {
            provider: DispatchablePayloadProviderInfo {
                _type: scrape.provider.name.clone(),
                id: scrape.provider.destination.clone(),
                ephemeral: provider.ephemeral(),
            },
            posts,
            metadata,
        }
    }
}

const WEBHOOK_DISPATCH_CONCURRENCY_LIMIT: usize = 8;

pub async fn dispatch_webhooks<'a>(
    // provider: &dyn Provider,
    // scrape: &Scrape<'a>,
    dispatch: Vec<(DatabaseWebhook, DispatchablePayload)>,
) -> Vec<WebhookInteraction> {
    let client = &Client::new();
    // request results are not guaranteed to be in order
    let mut results: Vec<WebhookInteraction> = vec![];
    // let posts = dispatch
    //     .iter()
    //     .flat_map(|(_, p)| &p.posts.iter().flat_map(|p| p.images).collect::<Vec<_>>())
    //     .collect::<Vec<_>>();
    // let discord_media = &posts[0..min(image_length, DISCORD_IMAGE_DISPLAY_LIMIT)];
    let ref_cell = RwLock::new(&mut results);
    let iter = |(wh, payload): (DatabaseWebhook, DispatchablePayload)| {
        let pl = payload.clone();
        let f = ref_cell.write();
        async move {
            let builder = client
                .post(&wh.destination)
                .headers(request_default_headers());
            let instant = Instant::now();
            if let WebhookDestination::Custom = webhook_type(&wh.destination) {
                let response = builder
                    .json(&pl)
                    .send()
                    .await
                    .map_err(|err| HttpError::ReqwestError(err));
                let response_time = instant.elapsed();
                f.unwrap().push(WebhookInteraction {
                    webhook: wh,
                    response,
                    response_time,
                });
            } else {
                ()
            }
        }
    };

    stream::iter(dispatch)
        // sadly there's no `map_concurrent` for futures
        .for_each_concurrent(WEBHOOK_DISPATCH_CONCURRENCY_LIMIT, iter)
        .await;
    results
}
