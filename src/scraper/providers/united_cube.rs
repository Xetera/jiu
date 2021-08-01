use super::*;
use crate::{
    request::{parse_successful_response, request_default_headers, HttpError},
    scheduler::UnscopedLimiter,
    scraper::ProviderCredentials,
};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use log::error;
use parking_lot::RwLock;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, sync::Arc};

pub struct UnitedCubeArtistFeed {
    pub client: Arc<Client>,
    pub credentials: Option<SharedCredentials>,
    pub rate_limiter: UnscopedLimiter,
}

#[async_trait]
impl RateLimitable for UnitedCubeArtistFeed {
    async fn wait(&self, key: &str) -> () {
        self.rate_limiter
            .until_ready_with_jitter(default_jitter())
            .await
    }
}

#[derive(Serialize)]
struct LoginInput {
    refresh_token: Option<String>,
    path: String,
    id: String,
    pw: String,
    remember_me: bool,
}

#[derive(Serialize)]
struct RefreshInput {
    refresh_token: String,
}

#[derive(Deserialize)]
struct GenericError {
    message: String,
}

#[derive(Deserialize)]
struct RefreshResponse {
    token: String,
}

#[derive(Deserialize)]
struct LoginResponse {
    slug: String,
    email: String,
    name: String,
    language: String,
    role_code: String,
    token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct PostData {
    path: String,
    status: Option<String>,
}

#[derive(Deserialize)]
struct PostMedia {
    type_code: String,
    data: PostData,
}

#[derive(Deserialize)]
struct Post {
    slug: String,
    content: String,
    register_datetime: NaiveDateTime,
    media: Vec<PostMedia>,
}

#[derive(Deserialize)]
struct Page {
    has_next: bool,
    has_prev: bool,
    // next_num: null,
    // prev_num: null,
    page: i32,
    pages: i32,
    per_page: i32,
    total: i32,
    items: Vec<Post>,
}

const BASE_MEDIA_URL: &'static str = "https://www.united-cube.com";

fn to_absolute(path: &str) -> String {
    format!("{}/{}", BASE_MEDIA_URL, path)
}

// impl Into

#[async_trait]
impl Provider for UnitedCubeArtistFeed {
    fn id(&self) -> AllProviders {
        AllProviders::UnitedCubeArtistFeed
    }
    fn new(input: ProviderInput) -> Self
    where
        Self: Sized,
    {
        Self {
            client: input.client,
            credentials: input.credentials,
            rate_limiter: Self::rate_limiter(),
        }
    }

    fn next_page_size(&self, _last_scraped: Option<NaiveDateTime>, _iteration: usize) -> PageSize {
        PageSize(200)
    }

    fn on_error(&self, http_error: &HttpError) -> anyhow::Result<ProviderErrorHandle> {
        let err = match http_error {
            &HttpError::ReqwestError(_err) => return Ok(ProviderErrorHandle::Halt),
            &HttpError::FailStatus(err) | &HttpError::UnexpectedBody(err) => err,
        };
        let body = match serde_json::from_str::<GenericError>(&err.body) {
            Err(err) => {
                error!("Couldn't parse the response from united_cube");
                eprintln!("{:?}", err);
                return Ok(ProviderErrorHandle::Halt);
            }
            Ok(body) => body,
        };
        Ok(if body.message == "Token Expired" && err.code == 400 {
            self.credentials.map_or(ProviderErrorHandle::Login, |cred| {
                ProviderErrorHandle::RefreshToken(cred.read().clone())
            })
        } else {
            // I don't think there is any other response you can get if your token is expired
            // so we can probably assume that something else has gone wrong
            ProviderErrorHandle::Halt
        })
    }

    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure> {
        let credentials = match self.credentials {
            Some(c) => c,
            None => return Ok(ProviderStep::NotInitialized),
        };
        // if self.credentials.is_none() {
        //     return Ok(ProviderStep::NotInitialized);
        // }
        // let credentials = self.credentials.unwrap().read();
        let token = credentials.read().access_token;
        let response = self
            .client
            .get(&state.url.0)
            .headers(request_default_headers())
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await?;
        // .json::<Page>()
        // .await;
        let response_json = parse_successful_response::<Page>(response).await?;
        let a = response_json.items.iter().map(|post| {
            post.media.into_iter().flat_map(|media| ProviderMedia {
                _type: ProviderMediaType::Image,
                media_url: to_absolute(&media.data.path),
                page_url: 
            })
        });
        Err(ProviderFailure::Url)
    }

    fn credentials(&self) -> Arc<RwLock<ProviderCredentials>> {
        self.credentials.clone().unwrap()
    }

    async fn login(&self) -> anyhow::Result<ProviderCredentials> {
        let response = self
            .client
            .post("https://united-cube.com/v1/auth/login")
            .json(&LoginInput {
                refresh_token: None,
                path: "https://www.united-cube.com/signin".to_owned(),
                id: env::var("UNITED_CUBE_USERNAME")
                    .expect("Tried to login to united_cube without credentials"),
                pw: env::var("UNITED_CUBE_PASSWORD").unwrap(),
                remember_me: false,
            })
            .send()
            .await?
            .json::<LoginResponse>()
            .await?;
        Ok(ProviderCredentials {
            access_token: response.token,
            refresh_token: response.refresh_token,
        })
    }
    async fn token_refresh(
        &self,
        credentials: &ProviderCredentials,
    ) -> anyhow::Result<CredentialRefresh> {
        let refresh_token = credentials.refresh_token;
        let response = self
            .client
            .post("https://united-cube.com/v1/auth/refresh")
            .json(&RefreshInput {
                refresh_token: refresh_token.clone(),
            })
            .send()
            .await?
            .json::<RefreshResponse>()
            .await?;
        Ok(CredentialRefresh::Result(ProviderCredentials {
            access_token: response.token,
            refresh_token,
        }))
    }
}
