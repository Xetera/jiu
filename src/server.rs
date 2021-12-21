use std::convert::Infallible;
use std::net::SocketAddr;
use std::ops::Sub;
use std::sync::Arc;

use axum::body::{Bytes, Full};
use axum::extract::Extension;
use axum::http::Response;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{AddExtensionLayer, Json, Router};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use log::{debug, error, info};
use num_traits::ToPrimitive;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::json;
use sqlx::types::BigDecimal;

use crate::api::v1::{v1_scheduled_scrapes, v1_scrape_history};
use crate::api::{AppError, Context};
use crate::db::{latest_requests, Database};

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
}

#[derive(Serialize)]
struct ScheduleResponse {
    id: i32,
    provider: String,
    url: String,
    destination: String,
    wait_days: i16,
    metadata: Option<serde_json::Value>,
    name: String,
}

#[deprecated]
async fn scheduled_scrapes(
    Extension(state): Extension<Arc<Context>>,
) -> Result<Json<Vec<ScheduleResponse>>, AppError> {
    let rows = sqlx::query_as!(
        ScheduledProvider,
        "SELECT pr.id, pr.priority, pr.name, pr.destination, pr.url, pr.tokens, pr.last_queue, pr.default_name, (
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
            ScheduleResponse {
                destination: row.destination,
                provider: row.name,
                id: row.id,
                url: row.url,
                wait_days,
                metadata: row.metadata,
                name: row.default_name.unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let mut out = today
        .into_iter()
        .map(|t| ScheduleResponse {
            destination: t.destination,
            provider: t.name,
            id: t.id,
            url: t.url,
            wait_days: 0,
            metadata: t.metadata,
            name: t.default_name.unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    out.extend(labeled);
    Ok(Json(out))
}

pub async fn run_server(db: Arc<Database>, port: u16) {
    info!("Starting server");
    let ctx = Arc::new(Context {
        db: Arc::clone(&db),
    });
    let router = Router::new()
        .route("/schedule", get(scheduled_scrapes))
        .route("/v1/schedule", get(v1_scheduled_scrapes))
        .route("/v1/history", get(v1_scrape_history))
        .layer(AddExtensionLayer::new(ctx));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}
