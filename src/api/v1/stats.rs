use std::sync::Arc;

use axum::extract::Extension;
use axum::Json;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use num_traits::ToPrimitive;
use serde::Serialize;
use sqlx::types::BigDecimal;

use crate::api::{AppError, Context};
use crate::models::ScrapeHistory;

struct ScheduledProvider {
    id: i32,
    url: String,
    name: String,
    destination: String,
    priority: BigDecimal,
    tokens: BigDecimal,
    default_name: Option<String>,
    last_queue: Option<NaiveDateTime>,
    metadata: Option<serde_json::Value>,
    official: bool,
}

#[derive(Serialize)]
pub struct ScheduledProviderRun {
    id: i32,
    provider: String,
    url: String,
    destination: String,
    wait_days: i16,
    metadata: Option<serde_json::Value>,
    name: String,
    official: bool,
}

struct PreviousScrapeRow {
    id: i32,
    name: String,
    url: String,
    destination: String,
    date: Option<NaiveDateTime>,
    // last_post: Option<NaiveDateTime>,
    priority: BigDecimal,
    default_name: Option<String>,
    official: bool,
}

#[derive(Serialize)]
pub struct PreviousScrape {
    id: i32,
    name: String,
    url: String,
    destination: String,
    // TODO: make this column not-null
    date: Option<NaiveDateTime>,
    // last_post: Option<NaiveDateTime>,
    // last_scrape: Option<NaiveDateTime>,
    // last_post: Option<NaiveDateTime>,
    priority: f32,
    default_name: Option<String>,
    official: bool,
}

#[derive(Serialize)]
pub struct ScheduleResponse {
    pub history: Vec<PreviousScrape>,
    pub scheduled: Vec<ScheduledProviderRun>,
}

pub async fn v1_scheduled_scrapes(
    Extension(state): Extension<Arc<Context>>,
) -> Result<Json<Vec<ScheduledProviderRun>>, AppError> {
    let rows = sqlx::query_as!(
        ScheduledProvider,
        "SELECT pr.id, pr.official, pr.priority, pr.name, pr.destination, pr.url, pr.tokens, pr.last_queue, pr.default_name, (
            SELECT metadata FROM amqp_source where provider_destination = pr.destination and provider_name = pr.name
        ) as metadata FROM provider_resource pr"
    )
        .fetch_all(&*state.db)
        .await?;
    let (today, later): (Vec<ScheduledProvider>, Vec<ScheduledProvider>) =
        rows.into_iter().partition(|e| {
            let now = Utc::now().naive_utc();
            // anything that was queued in the last 24 hours is already being scraped
            // it's not SUPER accurate since it's possible but
            // we only need a general idea, not precision
            e.last_queue
                .map(|last_queue| {
                    let yesterday = now - Duration::hours(24);
                    last_queue > yesterday
                })
                .unwrap_or(false)
        });
    let labeled = later
        .into_iter()
        .map(|row| {
            let wait_days = ((1f32 / (row.priority + row.tokens))
                .to_f32()
                .unwrap_or(0f32))
            .floor() as i16;
            ScheduledProviderRun {
                destination: row.destination,
                provider: row.name,
                id: row.id,
                url: row.url,
                official: row.official,
                wait_days,
                metadata: row.metadata,
                name: row.default_name.unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let mut scheduled = today
        .into_iter()
        .map(|t| ScheduledProviderRun {
            destination: t.destination,
            provider: t.name,
            official: t.official,
            id: t.id,
            url: t.url,
            wait_days: 0,
            metadata: t.metadata,
            name: t.default_name.unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    scheduled.extend(labeled);
    Ok(Json(scheduled))
}

pub async fn v1_scrape_history(
    Extension(state): Extension<Arc<Context>>,
) -> Result<Json<Vec<PreviousScrape>>, AppError> {
    let previous = sqlx::query_as!(
        PreviousScrapeRow,
        "SELECT scrape.id,
            pr.url,
            pr.default_name,
            pr.official,
            pr.name,
            pr.destination,
            scrape.priority,
            scrape.scraped_at as date
        FROM scrape
                 INNER JOIN provider_resource pr on pr.destination = scrape.provider_destination
            and scrape.provider_name = pr.name
        ORDER BY scrape.scraped_at desc
        LIMIT 100
        "
    )
    .fetch_all(&*state.db)
    .await?;
    let history = previous
        .into_iter()
        .map(|row| PreviousScrape {
            id: row.id,
            name: row.name,
            url: row.url,
            date: row.date,
            destination: row.destination,
            priority: row.priority.to_f32().unwrap(),
            default_name: row.default_name,
            official: row.official,
        })
        .collect::<Vec<_>>();
    Ok(Json(history))
}
// (SELECT Max(scraped_at) FROM scrape_request sr where sr.scrape_id = scrape.id) as last_scrape,
// (SELECT MAX(posted_at) FROM media
// INNER JOIN public.scrape_request s on s.id = media.scrape_request_id
// inner join scrape s2 on s2.id = s.scrape_id
// where s2.id = scrape.id
// ) as last_post
