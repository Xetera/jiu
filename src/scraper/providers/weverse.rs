use std::{env, iter::FromIterator, sync::Arc, time::Instant};

use async_trait::async_trait;
use bimap::{BiHashMap, BiMap};
use chrono::{DateTime, NaiveDateTime, Utc};
use governor::Quota;
use lazy_static::lazy_static;
use log::info;
use rand::rngs::OsRng;
use regex::Regex;
use reqwest::Client;
use rsa::{PaddingScheme, PublicKey, RSAPublicKey};
use serde::{Deserialize, Serialize};
use sha1::Sha1;

use crate::{
    request::{parse_successful_response, request_default_headers, HttpError},
    scheduler::UnscopedLimiter,
    scraper::{providers::ProviderMediaType, ProviderMedia, ProviderResult},
};

use super::*;

/// https://gist.github.com/Xetera/aa59e84f3959a37c16a3309b5d9ab5a0
async fn get_public_key(client: &Client) -> anyhow::Result<RSAPublicKey> {
    let login_page = client
        .post("https://account.weverse.io/login/auth?client_id=weverse-test&hl=en")
        .send()
        .await?
        .text()
        .await?;
    let regex = Regex::new(r"/(static/js/main\..*.js)").unwrap();
    let js_bundle_captures = regex.captures(&login_page).unwrap();

    let js_name = js_bundle_captures
        .get(1)
        .expect("Couldn't match a main js bundle on account.weverse.io, the site was changed")
        .as_str();
    let js_bundle_url = format!("https://account.weverse.io/{}", js_name);
    let js_bundle = client.get(&js_bundle_url).send().await?.text().await?;
    let rsa_captures =
        Regex::new(r"(-----BEGIN RSA PUBLIC KEY-----(.|\n)+----END RSA PUBLIC KEY-----)")
            .unwrap()
            .captures(&js_bundle)
            .expect(&format!(
                "Couldn't find a hardcoded RSA key in {}",
                &js_bundle_url
            ));

    let rsa_key = rsa_captures.get(1).unwrap().as_str().to_owned();

    let der_encoded = rsa_key
        .replace("\\n", "\n")
        .lines()
        .filter(|line| !line.starts_with("-"))
        .fold(String::new(), |mut data, line| {
            data.push_str(&line);
            data
        });

    let der_bytes = base64::decode(&der_encoded).expect("failed to decode base64 content");
    let public_key = RSAPublicKey::from_pkcs8(&der_bytes).expect("failed to parse key");
    Ok(public_key)
}

fn encrypted_password(password: String, public_key: RSAPublicKey) -> anyhow::Result<String> {
    let mut rng = OsRng;
    let padding = PaddingScheme::new_oaep::<Sha1>();
    let encrypted = public_key.encrypt(&mut rng, padding, &password.as_bytes())?;
    Ok(base64::encode(encrypted))
}

#[derive(Serialize)]
struct WeverseLoginRequest {
    grant_type: String,
    client_id: String,
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct WeverseLoginResponse {
    refresh_token: String,
    access_token: String,
}

async fn get_access_token(
    email: String,
    encrypted_password: String,
    client: &Client,
) -> anyhow::Result<WeverseAuthorizeResponse> {
    Ok(client
        .post("https://accountapi.weverse.io/api/v1/oauth/token")
        .json(&WeverseAuthorizeInput::Login {
            grant_type: "password".to_owned(),
            client_id: "weverse-test".to_owned(),
            username: email,
            password: encrypted_password,
        })
        .send()
        .await?
        .json::<WeverseAuthorizeResponse>()
        .await?)
}

pub async fn fetch_weverse_auth_token(
    client: &Client,
) -> anyhow::Result<Option<ProviderCredentials>> {
    match (
        env::var("WEVERSE_ACCESS_TOKEN"),
        env::var("WEVERSE_EMAIL"),
        env::var("WEVERSE_PASSWORD"),
    ) {
        (Ok(access_token), _, _) => {
            info!("An existing weverse token was found");
            Ok(Some(ProviderCredentials {
                access_token,
                refresh_token: "".to_owned(),
            }))
        }
        (_, Ok(email), Ok(password)) => {
            info!("Detected weverse credentials, attempting to login...");
            let public_key = get_public_key(&client).await?;
            let encrypted = encrypted_password(password, public_key)?;
            let token = get_access_token(email, encrypted, &client).await?;
            Ok(Some(ProviderCredentials {
                access_token: token.access_token,
                refresh_token: token.refresh_token,
            }))
        }
        _ => {
            info!("Weverse credentials missing, not initializing Weverse module");
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeversePhoto {
    id: u64,
    org_img_url: String,
    org_img_height: u32,
    org_img_width: u32,
    thumbnail_img_url: String,
    post_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeversePost {
    id: u64,
    // community: WeverseCommunity,
    body: Option<String>,
    community_user: WeverseCommunityUser,
    photos: Option<Vec<WeversePhoto>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeverseCommunityUser {
    community_id: u32,
    artist_id: u32,
    profile_img_path: String,
    profile_nickname: String,
}

#[derive(Debug, Serialize)]
struct PostMetadata {
    author_id: u32,
    author_name: String,
}

#[derive(Debug, Serialize)]
pub struct ImageMetadata {
    height: u32,
    width: u32,
    thumbnail_url: String,
}

#[derive(Debug, Serialize)]
enum WeverseAuthorizeInput {
    TokenRefresh {
        client_id: String,
        grant_type: String,
        refresh_token: String,
    },
    Login {
        client_id: String,
        grant_type: String,
        username: String,
        password: String,
    },
}

#[derive(Debug, Deserialize)]
struct WeverseAuthorizeResponse {
    access_token: String,
    token_type: String,
    expires_in: i32,
    refresh_token: String,
}

#[derive(Debug, Serialize)]
struct WeverseTokenRefreshInput {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeversePage {
    is_ended: bool,
    last_id: u64,
    posts: Vec<WeversePost>,
}

// #[derive(Clone)]
pub struct WeverseArtistFeed {
    pub client: Arc<Client>,
    pub credentials: SharedCredentials<ProviderCredentials>,
    pub rate_limiter: UnscopedLimiter,
}

lazy_static! {
    static ref ARTIST_MAPPINGS: BiMap<u32, &'static str> =
        BiHashMap::from_iter([(14, "dreamcatcher"), (10, "sunmi")]);
}

fn url_from_post(artist_id: u32, post_id: u64, photo_id: u64) -> String {
    let artist_name = ARTIST_MAPPINGS
        .get_by_left(&artist_id)
        .expect(&format!("Weverse ID {} is not a valid mapping", artist_id));
    format!(
        "https://weverse.io/{}/artist/{}?photoId={}",
        artist_name, post_id, photo_id
    )
    .to_owned()
}

const MAX_PAGESIZE: usize = 30;
// weverse is stupid and uses a 16 page default pagesize
const DEFAULT_PAGESIZE: usize = 16;

#[async_trait]
impl RateLimitable for WeverseArtistFeed {
    fn quota() -> Quota
    where
        Self: Sized,
    {
        default_quota()
    }
    async fn wait(&self, _key: &str) -> () {
        self.rate_limiter
            .until_ready_with_jitter(default_jitter())
            .await
    }
}

#[async_trait]
impl Provider for WeverseArtistFeed {
    fn new(input: ProviderInput) -> Self
    where
        Self: Sized,
    {
        Self {
            credentials: create_credentials(),
            client: Arc::clone(&input.client),
            rate_limiter: Self::rate_limiter(),
        }
    }
    fn id(&self) -> AllProviders {
        AllProviders::WeverseArtistFeed
    }

    fn requires_auth(&self) -> bool {
        true
    }

    async fn initialize(&self) -> () {
        attempt_first_login(self, &self.credentials).await;
    }

    fn next_page_size(&self, last_scrape: Option<NaiveDateTime>, iteration: usize) -> PageSize {
        PageSize(match last_scrape {
            None => MAX_PAGESIZE,
            Some(_) => {
                if iteration > 2 {
                    MAX_PAGESIZE
                } else {
                    DEFAULT_PAGESIZE
                }
            }
        })
    }

    // async fn canonical_url_to_id(&self, url: &str) -> CanonicalUrlResolution {
    //     let res = match self.client.get("https://weverse.io").send().await {
    //         Ok(res) => res,
    //         Err(err) => {
    //             println!("{:?}", err);
    //             return CanonicalUrlResolution::Fail
    //         }
    //     };
    //     let html = match res.text().await {
    //         Ok(ok) => ok,
    //         Err(err) => return CanonicalUrlResolution::Fail,
    //     };
    //     let regex = Regex::new(r"/(communitiesInfo\s*?=\s*?(\[.*?\]))").unwrap();
    // }

    fn from_provider_destination(
        &self,
        id: &str,
        page_size: PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        let mut next_url = UrlBuilder::default();
        next_url.page_size("pageSize", page_size);
        next_url.pagination("from", &pagination);
        let url = next_url.build_scrape_url(&format!(
            "https://weversewebapi.weverse.io/wapi/v1/communities/{}/posts/artistTab",
            id
        ))?;
        Ok(url)
    }

    fn max_pagination(&self) -> u16 {
        2
    }

    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure> {
        let credentials = self.credentials.read().clone();
        // let token = "".to_owned();
        let token = match credentials {
            Some(token) => token.access_token,
            None => return Ok(ProviderStep::NotInitialized),
        };
        // .refresh_token
        // .clone();
        let instant = Instant::now();
        let response = self
            .client
            .get(&state.url.0)
            .headers(request_default_headers())
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let response_code = response.status();
        let response_delay = instant.elapsed();
        let response_json = parse_successful_response::<WeversePage>(response).await?;
        let posts = response_json
            .posts
            .into_iter()
            .map(|post| {
                let community_id = post.community_user.community_id.to_owned();
                let post_id = post.id;
                let user = post.community_user;
                let author_name = user.profile_nickname;
                let author_id = user.artist_id;
                let post_created_at = post.created_at;
                let photos = post.photos.unwrap_or(vec![]);
                let page_url = photos
                    .get(0)
                    .map(|photo| url_from_post(community_id, post_id, photo.id));
                ProviderPost {
                    account: ProviderAccount {
                        avatar_url: Some(user.profile_img_path),
                        name: author_name.clone(),
                    },
                    unique_identifier: post_id.to_string(),
                    metadata: serde_json::to_value(PostMetadata {
                        author_id,
                        author_name: author_name.clone(),
                    })
                    .ok(),
                    body: post.body,
                    url: page_url,
                    post_date: Some(post_created_at.naive_utc()),
                    images: photos
                        .into_iter()
                        .map(|photo| {
                            ProviderMedia {
                                // should be unique across all of weverse
                                _type: ProviderMediaType::Image,
                                unique_identifier: photo.id.to_string(),
                                media_url: photo.org_img_url.clone(),
                                reference_url: Some(url_from_post(community_id, post_id, photo.id)),
                                metadata: serde_json::to_value(ImageMetadata {
                                    height: photo.org_img_height,
                                    width: photo.org_img_width,
                                    thumbnail_url: photo.thumbnail_img_url.clone(),
                                })
                                .ok(),
                            }
                        })
                        // not sure why I have to do this here
                        .collect::<Vec<_>>(),
                }
            })
            .collect::<Vec<_>>();
        let has_more = !response_json.is_ended;
        let result = ProviderResult {
            posts,
            response_code,
            response_delay,
        };
        if has_more {
            return Ok(ProviderStep::Next(
                result,
                Pagination::NextCursor(response_json.last_id.to_string()),
            ));
        }
        Ok(ProviderStep::End(result))
    }

    fn on_error(&self, http_error: &HttpError) -> anyhow::Result<ProviderErrorHandle> {
        match http_error {
            HttpError::FailStatus(err) | HttpError::UnexpectedBody(err) => {
                // :) I don't actually know if weverse returns a 401 on expired tokens
                // but I can't test because their tokens last for 6 ENTIRE months!!!!
                if err.code == 401 || err.code == 403 {
                    let handle = self
                        .credentials
                        .clone()
                        .try_read()
                        .map_or(ProviderErrorHandle::Login, |creds| {
                            ProviderErrorHandle::RefreshToken(creds.clone().unwrap())
                        });
                    return Ok(handle);
                }
                Ok(ProviderErrorHandle::Halt)
            }
            _ => Ok(ProviderErrorHandle::Halt),
        }
    }
    async fn token_refresh(
        &self,
        credentials: &ProviderCredentials,
    ) -> anyhow::Result<CredentialRefresh> {
        let input = WeverseAuthorizeInput::TokenRefresh {
            grant_type: "refresh_token".to_owned(),
            client_id: "weverse-test".to_owned(),
            refresh_token: credentials.refresh_token.clone(),
        };
        let out = self
            .client
            .post("https://accountapi.weverse.io/api/v1/oauth/token")
            .json(&input)
            .send()
            .await?
            .json::<WeverseAuthorizeResponse>()
            .await?;
        let credentials_result = ProviderCredentials {
            access_token: out.access_token,
            refresh_token: out.refresh_token,
        };
        Ok(CredentialRefresh::Result(credentials_result))
    }
    async fn login(&self) -> anyhow::Result<ProviderCredentials> {
        let credentials = fetch_weverse_auth_token(&self.client)
            .await?
            .expect("Tried to authorize weverse module but the login credentials were not found");
        Ok(credentials)
    }
    fn credentials(&self) -> SharedCredentials<ProviderCredentials> {
        self.credentials.clone()
    }
}
