use std::sync::Arc;

use axum::extract::Extension;
use axum::Json;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::api::{AppError, Context};
use crate::scraper::{CanonicalUrlResolution, ProviderFailure, WorkableDomain};

#[derive(Deserialize)]
pub struct ProviderAdd {
    url: String,
    name: String,
    official: bool,
    metadata: Option<Value>,
    add_to_amqp: Option<bool>,
}

#[derive(Serialize)]
pub enum ProviderAddResponse {
    InvalidUrl { url: String },
    InternalError,
    NotImplemented,
    Success { destination: String },
}

pub async fn v1_add_provider(
    Extension(state): Extension<Arc<Context>>,
    Json(input): Json<ProviderAdd>,
) -> Result<Json<ProviderAddResponse>, AppError> {
    let result = state
        .providers
        .values()
        .find_map(|p| p.match_domain(&input.url).map(|res| (p, res)));
    let (provider, domain) = match result {
        Some((provider, domain)) => (provider, domain),
        None => {
            debug!("Url {} was not valid", input.url);
            return Ok(Json(ProviderAddResponse::InvalidUrl { url: input.url }));
        }
    };
    let introspectable = match domain {
        WorkableDomain::ToCanonical(resource) => resource,
        _ => {
            debug!(
                "WorkableDomain {:?} from [{}] was not detected as Canonical",
                provider.id(),
                input.url
            );
            return Ok(Json(ProviderAddResponse::InvalidUrl { url: input.url }));
        }
    };
    let response = provider.introspect_resource(&introspectable).await;
    let destination = match response {
        Ok(CanonicalUrlResolution::Success { destination }) => destination,
        Ok(CanonicalUrlResolution::Fail(reason)) => {
            error!("{:?}", reason);
            return Ok(Json(ProviderAddResponse::InternalError));
        }
        Ok(CanonicalUrlResolution::NotImplemented) => {
            return Ok(Json(ProviderAddResponse::NotImplemented))
        }
        Err(ProviderFailure::Url) => {
            return Ok(Json(ProviderAddResponse::InvalidUrl { url: input.url }));
        }
        Err(other) => {
            return Ok(Json(ProviderAddResponse::InternalError));
        }
    };
    info!(
        "Successfully resolved [destination: {}] for [{:?}]",
        destination,
        provider.id()
    );
    let provider_name = provider.id().to_string();
    let db_result = sqlx::query!(
        "INSERT INTO provider_resource (destination, name, default_name, official, url) VALUES
            ($1, $2, $3, $4, $5)
            ON CONFLICT(destination, name) DO UPDATE SET enabled = True",
        destination,
        provider_name,
        input.name,
        input.official,
        input.url
    )
    .fetch_one(&*state.db)
    .await?;
    // TODO: decouple this kiyomi-specific thing out?
    if input.add_to_amqp.unwrap_or(false) {
        let source = sqlx::query!(
            "INSERT INTO amqp_source (provider_name, provider_destination, metadata)
                VALUES ($1, $2, $3)
                ON CONFLICT(provider_name, provider_destination) DO UPDATE SET metadata = $3",
            provider_name,
            destination,
            input.metadata.unwrap_or(Value::Object(Map::new()))
        )
        .fetch_one(&*state.db)
        .await?;
    }
    Ok(Json(ProviderAddResponse::Success { destination }))
}

#[derive(Deserialize)]
pub struct ProviderDelete {
    name: String,
    destination: String,
}

#[derive(Serialize)]
pub struct ProviderDeleteResponse {
    modified: bool,
}

pub async fn v1_delete_provider(
    Extension(state): Extension<Arc<Context>>,
    Json(input): Json<ProviderDelete>,
) -> Result<Json<ProviderDeleteResponse>, AppError> {
    let result = sqlx::query!(
        "UPDATE provider_resource SET enabled = False WHERE name = $1 and destination = $2 RETURNING *",
        input.name,
        input.destination,
    )
    .fetch_optional(&*state.db)
    .await?;
    return Ok(Json(ProviderDeleteResponse {
        modified: result.is_some(),
    }));
}
