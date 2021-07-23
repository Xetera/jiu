use std::env;
use std::iter::FromIterator;

use log::error;
use reqwest;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug)]
pub struct ResponseErrorContext {
    pub body: String,
    pub code: StatusCode,
}

/// Wrapper for providing actual useful information about
/// why responses failed since reqwest throws that information
/// away when it encounters errors
#[derive(Error, Debug)]
pub enum HttpError {
    #[error("Failed response code {0:?}")]
    FailStatus(ResponseErrorContext),
    #[error("Unexpected body {0:?}")]
    UnexpectedBody(ResponseErrorContext),
    #[error("Request error")]
    ReqwestError(#[from] reqwest::Error),
}

pub async fn parse_successful_response<T: DeserializeOwned>(
    response: Response,
) -> Result<T, HttpError> {
    let response_code = response.status();
    let url = response.url().clone();
    let response_body = response.text().await?;
    if !response_code.is_success() {
        // sadly shitty reqwest doesn't give us the response body as
        // context when trying to handle invalid responses
        return Err(HttpError::FailStatus(ResponseErrorContext {
            body: response_body,
            code: response_code,
        }));
    }
    serde_json::from_str::<T>(&response_body).map_err(|_error| {
        error!("Failed to parse response from {}", url);
        HttpError::UnexpectedBody(ResponseErrorContext {
            body: response_body,
            code: response_code,
        })
    })
}

pub fn request_default_headers() -> HeaderMap {
    // TODO: change the user agent if the program has been forked to modify
    // important settings like request speed
    let user_agent: String =
        env::var("USER_AGENT").expect("Missing USER_AGENT environment variable");
    HeaderMap::from_iter([(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_str(&user_agent).unwrap(),
    )])
}
