use async_trait::async_trait;
use bimap::{BiHashMap, BiMap};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use log::info;
use rand::rngs::OsRng;
use regex::Regex;
use reqwest::Client;
use rsa::{PaddingScheme, PublicKey, RSAPublicKey};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use std::{env, iter::FromIterator, sync::Arc, time::Instant};

use crate::{
    request::{parse_successful_response, request_default_headers},
    scraper::{providers::ProviderMediaType, ProviderMedia, ProviderResult},
};

use super::{
    AllProviders, PageSize, Pagination, Provider, ProviderFailure, ProviderState, ProviderStep,
    ScrapeUrl,
};

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
) -> anyhow::Result<WeverseLoginResponse> {
    Ok(client
        .post("https://accountapi.weverse.io/api/v1/oauth/token")
        .json(&WeverseLoginRequest {
            grant_type: "password".to_owned(),
            client_id: "weverse-test".to_owned(),
            username: email,
            password: encrypted_password,
        })
        .send()
        .await?
        .json::<WeverseLoginResponse>()
        .await?)
}
pub async fn fetch_weverse_auth_token(client: &Client) -> anyhow::Result<Option<String>> {
    match (
        env::var("WEVERSE_ACCESS_TOKEN"),
        env::var("WEVERSE_EMAIL"),
        env::var("WEVERSE_PASSWORD"),
    ) {
        (Ok(access_token), _, _) => Ok(Some(access_token)),
        (_, Ok(email), Ok(password)) => {
            info!("Detected weverse credentials, attempting to login...");
            let public_key = get_public_key(&client).await?;
            let encrypted = encrypted_password(password, public_key)?;
            let token = get_access_token(email, encrypted, &client).await?;
            Ok(Some(token.access_token))
        }
        _ => {
            info!("Weverse credentials missing, not initializing Weverse module");
            Ok(None)
        }
    }
}
#[derive(Debug, Deserialize)]
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
pub struct WeverseCommunity {
    id: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeversePost {
    id: u64,
    community: WeverseCommunity,
    photos: Vec<WeversePhoto>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WeverseMetadata {
    height: u32,
    width: u32,
    thumbnail_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeversePage {
    is_ended: bool,
    last_id: u64,
    posts: Vec<WeversePost>,
}

#[derive(Clone)]
pub struct WeverseArtistFeed {
    pub client: Arc<Client>,
    pub access_token: Option<String>,
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

#[async_trait]
impl Provider for WeverseArtistFeed {
    fn id(&self) -> AllProviders {
        AllProviders::WeverseArtistFeed
    }

    fn estimated_page_size(&self, _: Option<DateTime<Utc>>) -> PageSize {
        PageSize(0)
    }

    fn from_provider_destination(
        &self,
        id: String,
        _page_size: super::PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        let mut params = vec![];
        if let Some(page) = pagination {
            params.push(("from", page.next_page()));
        }
        let next_url = url::Url::parse_with_params(
            &format!(
                "https://weversewebapi.weverse.io/wapi/v1/communities/{}/posts/artistTab",
                id
            ),
            params.iter(),
        )
        .ok()
        .ok_or(ProviderFailure::Url)?;
        Ok(ScrapeUrl(next_url.as_str().to_owned()))
    }

    fn max_pagination(&self) -> u16 {
        2
    }

    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure> {
        match &self.access_token {
            None => {
                info!(
                    "Weverse module was not initialized, not scraping url: {}",
                    state.url.0
                );
                Ok(ProviderStep::NotInitialized)
            }
            Some(token) => {
                let instant = Instant::now();
                let response = self
                    .client
                    .get(&state.url.0)
                    .headers(request_default_headers())
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await?;

                let response_code = response.status();
                let response_json = parse_successful_response::<WeversePage>(response).await?;
                let response_delay = instant.elapsed();
                let images = response_json
                    .posts
                    .into_iter()
                    .flat_map(move |post| {
                        post.photos
                            .iter()
                            .map(|photo| {
                                let page_url = url_from_post(post.community.id, post.id, photo.id);
                                ProviderMedia {
                                    // should be unique across all of weverse
                                    _type: ProviderMediaType::Image,
                                    unique_identifier: photo.id.to_string(),
                                    post_date: Some(post.created_at),
                                    image_url: photo.org_img_url.clone(),
                                    page_url: Some(page_url.clone()),
                                    reference_url: Some(page_url.clone()),
                                    provider_metadata: serde_json::to_value(WeverseMetadata {
                                        height: photo.org_img_height,
                                        width: photo.org_img_width,
                                        thumbnail_url: photo.thumbnail_img_url.clone(),
                                    })
                                    .ok(),
                                }
                            })
                            // not sure why I have to do this here
                            .collect::<Vec<ProviderMedia>>()
                    })
                    .collect::<Vec<ProviderMedia>>();
                let has_more = !response_json.is_ended;
                let result = ProviderResult {
                    images,
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
                // todo!()
            }
        }
    }
}
