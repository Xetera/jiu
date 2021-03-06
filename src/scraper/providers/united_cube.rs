use std::{env, path::Path, sync::Arc, time::Instant};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use log::error;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    request::{parse_successful_response, request_default_headers, HttpError},
    scheduler::UnscopedLimiter,
    scraper::ProviderCredentials,
};

use super::*;

pub struct UnitedCubeArtistFeed {
    pub client: Arc<Client>,
    pub credentials: SharedCredentials<ProviderCredentials>,
    pub rate_limiter: UnscopedLimiter,
}

#[async_trait]
impl RateLimitable for UnitedCubeArtistFeed {
    async fn wait(&self, _key: &str) -> () {
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
    // slug: String,
    // email: String,
    // name: String,
    // language: String,
    // role_code: String,
    token: String,
    refresh_token: String,
}

/// Posts are divided between images and videos
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type_code", content = "data")]
enum MediaData {
    #[serde(rename = "601")]
    Image { path: String },
    #[serde(rename = "602")]
    Video { url: String, image: String },
    #[serde(rename = "604")]
    Post { title: String },
}

#[derive(Debug, Deserialize, Clone)]
struct Post {
    slug: String,
    content: Option<String>,
    register_datetime: DateTime<Utc>,
    media: Vec<MediaData>,
}

#[derive(Debug, Deserialize, Clone)]
struct Page {
    has_next: bool,
    // has_prev: bool,
    // prev_num: null,
    page: i32,
    next_num: Option<i32>,
    // pages: i32,
    // per_page: i32,
    // total: i32,
    items: Vec<Post>,
}

const BASE_URL: &str = "https://www.united-cube.com";

fn extract_url_and_id(path: &str, base_url: &url::Url) -> anyhow::Result<(url::Url, String)> {
    // ucube is missing a leading slash in their links lol
    let parsed_relative_url = format!("/{}", &path);
    let url = base_url.join(&parsed_relative_url)?;
    // .map_err(|result| {
    //     anyhow::anyhow!(result)
    // })?;
    // unbelievably big brain conversion
    let unique_identifier = Path::new(&parsed_relative_url)
        .file_stem()
        .and_then(|str| str.to_str().map(|result| result.to_owned()))
        .ok_or_else(|| anyhow::anyhow!("Invalid file format: {}", parsed_relative_url))?;
    Ok((url, unique_identifier))
}

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
            credentials: create_credentials(),
            rate_limiter: Self::rate_limiter(),
        }
    }

    fn requires_auth(&self) -> bool {
        true
    }

    async fn initialize(&self) -> () {
        if self.requires_auth() {
            attempt_first_login(self, &self.credentials).await;
        }
    }

    fn max_page_size(&self) -> PageSize {
        PageSize(200)
    }

    fn default_page_size(&self) -> PageSize {
        PageSize(20)
    }

    fn on_error(&self, http_error: &HttpError) -> anyhow::Result<ProviderErrorHandle> {
        let err = match http_error {
            HttpError::ReqwestError(_err) => return Ok(ProviderErrorHandle::Halt),
            HttpError::FailStatus(err) | HttpError::UnexpectedBody(err) => err,
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
            let cred = self.credentials.read().clone();
            ProviderErrorHandle::RefreshToken((cred).unwrap())
        } else {
            // I don't think there is any other response you can get if your token is expired
            // so we can probably assume that something else has gone wrong
            ProviderErrorHandle::Halt
        })
    }

    fn from_provider_destination(
        &self,
        id: &str,
        page_size: PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        // club_id|board_id
        let page_id = id.to_string();
        let parts = page_id.split('|').collect::<Vec<_>>();
        let board = parts.get(1).unwrap();
        let mut next_url: UrlBuilder = Default::default();
        next_url.params.push(("board", board.to_string()));
        next_url.page_size("per_page", page_size);
        next_url.pagination("page", &pagination);
        let url = next_url.build_scrape_url("https://united-cube.com/v1/posts")?;
        Ok(url)
    }

    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure> {
        let creds = self.credentials.read().clone();
        let credentials = match creds {
            Some(c) => c,
            None => return Ok(ProviderStep::NotInitialized),
        };

        let token = credentials.access_token.clone();
        let instant = Instant::now();

        let response = self
            .client
            .get(&state.url.0)
            .headers(request_default_headers())
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await?;
        let elapsed = instant.elapsed();
        let status = response.status();

        let cube_url = url::Url::parse(BASE_URL).unwrap();
        let response_json = parse_successful_response::<Page>(response).await?;

        let account = state
            .default_name
            .map(|name| ProviderAccount {
                name,
                avatar_url: None,
            })
            .unwrap_or_default();
        let posts = response_json
            .items
            .into_iter()
            .map(|post| {
                ProviderPost {
                    // UCube does not give us any kind of user information
                    account: account.clone(),
                    unique_identifier: post.slug,
                    // TODO: maybe add page urls to this anyways?
                    // united-cube doesn't have page-specific links, they all go to
                    // https://www.united-cube.com/club/qXmD_5exRnmZfkFIwR1cVA/board/cHTUTBaRRpqUWAL2c5nQiw#PostDetail
                    // which is controlled by JS and can't be linked to
                    url: None,
                    // This is HTML but who cares
                    body: post.content,
                    post_date: Some(post.register_datetime.naive_utc()),
                    metadata: None,
                    images:
                    post.media
                        .iter()
                        .filter_map(|media| {
                            let (_type, media_url, unique_identifier) = match &media {
                                // we don't care about posts
                                MediaData::Post { .. } => return None,
                                // Every video on ucube is (probably) a link to an external youtube video
                                // but we can't be sure
                                MediaData::Video { url, .. } => {
                                    let is_probably_external_link = url.starts_with("http");
                                    if is_probably_external_link {
                                        return None;
                                    }
                                    // assuming that a non-external link would follow the same pattern as
                                    match extract_url_and_id(url.as_str(), &cube_url) {
                                        Err(err) => {
                                            error!("Could not convert a non-external ucube video into a relative path");
                                            error!("{:?}", err);
                                            return None;
                                        }
                                        Ok((url, id)) => {
                                            (ProviderMediaType::Video, url.as_str().to_owned(), id)
                                        }
                                    }
                                }
                                MediaData::Image { path } => {
                                    match extract_url_and_id(path.as_str(), &cube_url) {
                                        Err(err) => {
                                            error!("Could not get relative path from a ucube image {}", path);
                                            error!("{:?}", err);
                                            return None;
                                        }
                                        Ok((url, id)) => {
                                            (ProviderMediaType::Image, url.as_str().to_owned(), id)
                                        }
                                    }
                                }
                            };
                            Some(ProviderMedia {
                                _type,
                                media_url,
                                // same with reference URL
                                reference_url: None,
                                metadata: None,
                                unique_identifier,
                            })
                        })
                        .collect::<Vec<_>>(),
                }
            })
            .collect::<Vec<_>>();

        let result = ProviderResult {
            posts,
            response_code: status,
            response_delay: elapsed,
        };
        match response_json.next_num {
            Some(next) => Ok(ProviderStep::Next(result, Pagination::NextPage(next))),
            None => Ok(ProviderStep::End(result)),
        }
    }

    fn credentials(&self) -> SharedCredentials<ProviderCredentials> {
        self.credentials.clone()
    }

    async fn login(&self) -> Result<ProviderCredentials, ProviderFailure> {
        let response = self
            .client
            .post("https://united-cube.com/v1/auth/login")
            .json(&LoginInput {
                refresh_token: None,
                path: "https://www.united-cube.com/signin".to_owned(),
                id: env::var("UNITED_CUBE_EMAIL")
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
        let refresh_token = credentials.refresh_token.clone();
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
