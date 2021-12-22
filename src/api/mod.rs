use crate::db::Database;
use axum::body::{Bytes, Full};
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use crate::scraper::ProviderMap;

pub mod v1;

pub struct Context {
    pub db: Arc<Database>,
    pub providers: Arc<ProviderMap>
}

pub enum AppError {
    SomeError(anyhow::Error),
    SqlxError(sqlx::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(inner: anyhow::Error) -> Self {
        AppError::SomeError(inner)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(inner: sqlx::Error) -> Self {
        AppError::SqlxError(inner)
    }
}

impl IntoResponse for AppError {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        let (status, error_message) = match self {
            AppError::SomeError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::SqlxError(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
