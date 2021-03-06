use crate::{scheduler::Priority, scraper::ScopedProvider};
use chrono::NaiveDateTime;
use serde::Serialize;
use std::fmt::Display;
use std::time::Duration;

#[derive(Debug)]
pub struct AMQPDestination {
    pub id: i32,
    pub metadata: Option<serde_json::Value>,
}

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

#[derive(Debug, Clone)]
pub struct ScrapeHistory {
    pub priority: Priority,
    pub provider: ScopedProvider,
    pub date: NaiveDateTime,
    pub result_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]

pub struct PendingProvider {
    pub id: i32,
    /// the name that is used if a more relevant name for posts cannot be found 
    pub default_name: Option<String>,
    pub priority: Priority,
    pub provider: ScopedProvider,
    pub scrape_date: Duration,
    pub last_scrape: Option<NaiveDateTime>,
}

impl Display for PendingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}", self.provider))
    }
}
