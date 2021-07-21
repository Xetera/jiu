use chrono::{DateTime, NaiveDateTime, Utc};

use crate::scraper::ProviderResult;

pub struct ScrapePage {
    pub provider_resource_id: i32,
    pub result: ProviderResult,
    pub scraped_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct Media {
    pub id: i32,
    pub url: String,
    pub unique_identifier: String,
    pub posted_at: Option<NaiveDateTime>,
    pub discovered_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct ProviderResource {
    pub id: i32,
    pub destination: String,
    pub name: String,
    pub priority: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ScrapeRequest {
    pub id: i32,
    pub scrape_id: i32,
    pub date: NaiveDateTime,
    pub page: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Scrape {
    pub id: i32,
    pub provider_resource_id: i32,
}
