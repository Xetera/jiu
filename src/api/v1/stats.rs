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
    discovered_media: Option<i64>,
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
    discovered_media: i64,
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
            scrape.scraped_at as date,
            COALESCE((SELECT COUNT(*)
            from media
                     inner join public.scrape_request sr on sr.id = media.scrape_request_id
                     inner join scrape s on s.id = sr.scrape_id
               where sr.scrape_id = scrape.id), 0) as discovered_media
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
            // we shouldn't need this, but sqlx doesn't understand the semantics
            // of COALESCE for some reason
            discovered_media: row.discovered_media.unwrap_or(0),
            priority: row.priority.to_f32().unwrap(),
            default_name: row.default_name,
            official: row.official,
        })
        .collect::<Vec<_>>();
    Ok(Json(history))
}

#[derive(Serialize)]
pub struct ProviderStat {
    name: String,
    destination: String,
    enabled: bool,
    url: String,
    priority: f32,
    tokens: f32,
    // TODO: why is this nullable?
    created_at: Option<NaiveDateTime>,
    default_name: Option<String>,
    official: bool,
    last_scrape: Option<NaiveDateTime>,
    last_post: Option<NaiveDateTime>,
    discovered_images: i64,
    scrape_count: i64,
}

#[derive(Serialize)]
pub struct ProviderStatsResponse {
    stats: Vec<ProviderStat>,
}

pub async fn v1_provider_stats(
    Extension(state): Extension<Arc<Context>>,
) -> Result<Json<ProviderStatsResponse>, AppError> {
    let stats = sqlx::query!(
        "SELECT pr.id,
       pr.name,
       pr.destination,
       pr.enabled,
       pr.url,
       pr.priority,
       pr.tokens,
       pr.created_at,
       pr.default_name,
       pr.official,
       (SELECT Max(sr.scraped_at)
        FROM scrape_request sr
                 inner join scrape s on pr.destination = s.provider_destination) as last_scrape,
       (SELECT MAX(posted_at)
        FROM media
                 INNER JOIN public.scrape_request s on s.id = media.scrape_request_id
                 inner join scrape s2 on s2.id = s.scrape_id
        where s2.provider_destination = pr.destination
          and s2.provider_name = pr.name
       ) as last_post,
       (SELECT COUNT(s3.*)
        from media
                 inner join public.scrape_request r on r.id = media.scrape_request_id
                 inner join scrape s3 on s3.id = r.scrape_id
        where s3.provider_name = pr.name
          and s3.provider_destination = pr.destination
       ) as discovered_images,
       (SELECT COUNT(*) from scrape inner join scrape_request sr2 on scrape.id = sr2.scrape_id
          where scrape.provider_destination = pr.destination and scrape.provider_name = pr.name
       ) as scrape_count
    FROM provider_resource pr;"
    )
    .fetch_all(&*state.db)
    .await?;
    let data = ProviderStatsResponse {
        stats: stats
            .iter()
            .map(|stat| ProviderStat {
                name: stat.name.clone(),
                destination: stat.destination.clone(),
                enabled: stat.enabled.unwrap_or(false),
                url: stat.url.clone(),
                priority: stat.priority.to_f32().unwrap_or(0f32),
                tokens: stat.tokens.to_f32().unwrap_or(0f32),
                created_at: stat.created_at,
                default_name: stat.default_name.clone(),
                official: stat.official,
                last_scrape: stat.last_scrape,
                last_post: stat.last_post,
                discovered_images: stat.discovered_images.unwrap_or(0),
                scrape_count: stat.scrape_count.unwrap_or(0),
            })
            .collect::<Vec<_>>(),
    };
    Ok(Json(data))
}

// (SELECT Max(scraped_at) FROM scrape_request sr where sr.scrape_id = scrape.id) as last_scrape,
// (SELECT MAX(posted_at) FROM media
// INNER JOIN public.scrape_request s on s.id = media.scrape_request_id
// inner join scrape s2 on s2.id = s.scrape_id
// where s2.id = scrape.id
// ) as last_post
