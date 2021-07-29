use crate::scraper::ScopedProvider;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::Serialize;
use std::fmt::Display;

#[derive(Debug)]
pub struct DatabaseWebhook {
    pub id: i32,
    pub destination: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScrapeRequestMedia {
    pub media_url: String,
    pub page_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScrapeRequestWithMedia {
    pub provider_name: String,
    pub url: String,
    pub response_code: Option<i32>,
    pub response_delay: Option<i32>,
    pub date: NaiveDateTime,
    pub media: Vec<ScrapeRequestMedia>,
}

#[derive(Debug)]
pub struct DatabaseWebhookSource {
    pub id: i32,
    pub webhook_id: i32,
    pub provider_destination: String,
}
#[derive(Debug)]
pub struct PendingProvider {
    pub provider: ScopedProvider,
    pub last_scrape: Option<DateTime<Utc>>,
}

impl Display for PendingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}", self.provider))
    }
}
