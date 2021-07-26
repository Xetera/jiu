use std::fmt::{Display, Write};

use chrono::{DateTime, NaiveDateTime, Utc};
use futures::future::Join;

use crate::scraper::ScopedProvider;

#[derive(Debug)]
pub struct DatabaseWebhook {
    pub id: i32,
    pub destination: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub metadata: Option<serde_json::Value>,
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
