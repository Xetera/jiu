use std::iter::FromIterator;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::DateTime;
use chrono::NaiveDateTime;
use governor::Quota;
use log::error;
use num_traits::identities;
use regex::Regex;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::request::{HttpError, parse_successful_response};
use crate::scheduler::UnscopedLimiter;
use crate::scraper::providers::twitter_types::{Twitter, TwitterImageMetadata, TwitterPostMetadata, Type};

use super::*;

fn twitter_type_to_provider(media_type: &Type) -> ProviderMediaType {
    match media_type {
        AnimatedGif => ProviderMediaType::Image,
        Photo => ProviderMediaType::Image,
        Video => ProviderMediaType::Video,
    }
}

fn replace_twitter_string(s: &str) -> String {
    s.replace("\\/", "/")
}

pub struct TwitterTimeline {
    pub guest_token: SharedCredentials<String>,
    pub client: Arc<Client>,
    pub rate_limiter: UnscopedLimiter,
}

const BASE_URL: &'static str = "https://twitter.com/";
/// I have no idea where this token is coming from...
const MAGIC_BEARER_TOKEN: &'static str = "AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA";

const USER_AGENT: &'static str = "HTC Mozilla/5.0 (Linux; Android 7.0; HTC 10 Build/NRD90M) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.83 Mobile Safari/537.36";

#[async_trait]
impl RateLimitable for TwitterTimeline {
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
impl Provider for TwitterTimeline {
    fn id(&self) -> AllProviders {
        AllProviders::TwitterTimeline
    }
    fn new(input: ProviderInput) -> Self
        where
            Self: Sized,
    {
        Self {
            guest_token: create_credentials(),
            client: Arc::clone(&input.client),
            rate_limiter: Self::rate_limiter(),
        }
    }
    fn next_page_size(&self, last_scraped: Option<NaiveDateTime>, iteration: usize) -> PageSize {
        PageSize(if iteration > 1 {
            100
        } else {
            20
        })
    }
    fn from_provider_destination(
        &self,
        id: &str,
        page_size: PageSize,
        pagination: Option<Pagination>,
    ) -> Result<ScrapeUrl, ProviderFailure> {
        let mut url_fragment = UrlBuilder::from_queries(vec![
            ("include_profile_interstitial_type", "1"),
            // https://github.com/twintproject/twint/blob/master/twint/url.py
            ("include_blocking", "1"),
            ("include_blocked_by", "1"),
            ("include_followed_by", "1"),
            ("include_want_retweets", "1"),
            ("include_mute_edge", "1"),
            ("include_can_dm", "1"),
            ("include_can_media_tag", "1"),
            ("skip_status", "1"),
            ("cards_platform", "Web - 12"),
            ("include_cards", "1"),
            ("include_ext_alt_text", "true"),
            ("include_quote_count", "true"),
            ("include_reply_count", "1"),
            ("tweet_mode", "extended"),
            ("include_entities", "true"),
            ("include_user_entities", "true"),
            ("include_ext_media_color", "true"),
            ("include_ext_media_availability", "true"),
            ("send_error_codes", "true"),
            ("simple_quoted_tweet", "true"),
            ("include_tweet_replies", "true"),
            ("ext", "mediaStats%2ChighlightedLabel")]
        );
        url_fragment.page_size("count", page_size);
        url_fragment.pagination("cursor", &pagination);
        let url = url_fragment.build_scrape_url(&format!(
            "https://api.twitter.com/2/timeline/profile/{}.json",
            id
        ))?;
        Ok(url)
    }

    fn max_pagination(&self) -> u16 {
        2
    }

    async fn unfold(&self, state: ProviderState) -> Result<ProviderStep, ProviderFailure> {
        let credentials = self.guest_token.read().clone();
        let token = match credentials {
            Some(token) => token,
            None => return Ok(ProviderStep::NotInitialized),
        };
        let instant = Instant::now();
        let response = self.client.get(BASE_URL).headers(
            HeaderMap::from_iter([(
                HeaderName::from_static("user-agent"),
                //죄송합니다
                HeaderValue::from_static(USER_AGENT)
            ), (
                HeaderName::from_static("authorization"),
                HeaderValue::from_static(MAGIC_BEARER_TOKEN)
            ), (
                HeaderName::from_static("x-guest-token"),
                HeaderValue::from_str(&token).unwrap()
            )]))
            .send()
            .await?;
        let status_code = response.status();
        let response_delay = instant.elapsed();
        let response_json = parse_successful_response::<Twitter>(response).await?;
        // Twitter does some really interesting stuff with how they present API data
        let maybe_instruction = response_json.timeline.instructions.iter().find_map(|instruction| instruction.get("addEntries"));
        let tweet_db = response_json.global_objects.tweets;
        let instruction = match maybe_instruction {
            Some(instruction) => instruction,
            None => return Err(ProviderFailure::Other("Could not find an 'addEntries' in instructions".to_owned())),
        };
        let tweets = instruction.entries.iter().filter_map(|entry| {
            let sort_index = &entry.sort_index;
            // a sort index corresponds to the id of the
            // the chances of this being undefined is basically non-existent but we should be safe
            let tweet = match tweet_db.get(sort_index) {
                None => {
                    error!("Could not find the corresponding tweet id for {} in the tweet db", sort_index);
                    return None;
                }
                Some(t) => t,
            };
            Some(tweet)
        }).filter_map(|tweet| {
            let unique_identifier = tweet.id_str.clone();
            let like_count = tweet.favorite_count;
            let retweet_count = tweet.retweet_count;
            let language = tweet.lang.clone();
            let post_date = DateTime::parse_from_rfc2822(&tweet.created_at)
                .ok()
                .map(|e| e.naive_utc());
            let body = tweet.full_text.clone().map(|t| replace_twitter_string(&t));
            tweet.entities.media.as_ref().map(|media| {
                ProviderPost {
                    unique_identifier,
                    metadata: serde_json::to_value(TwitterPostMetadata {
                        like_count,
                        retweet_count,
                        language,
                    }).ok(),
                    url: None,
                    post_date,
                    images: media.into_iter().map(|media| {
                        ProviderMedia {
                            _type: twitter_type_to_provider(&media.media_type),
                            unique_identifier: media.id_str.clone(),
                            media_url: replace_twitter_string(&media.media_url_https),
                            reference_url: Some(replace_twitter_string(&media.expanded_url)),
                            metadata: serde_json::to_value(TwitterImageMetadata {
                                height: media.original_info.height,
                                width: media.original_info.width,
                            }).ok(),
                        }
                    }).collect::<Vec<_>>(),
                    body,
                }
            })
        }).collect::<Vec<_>>();


        todo!()
    }

    async fn login(&self) -> anyhow::Result<ProviderCredentials> {
        let login = self.client.get(BASE_URL)
            .headers(HeaderMap::from_iter([(HeaderName::from_static("user-agent"), HeaderValue::from_static(USER_AGENT))]))
            .send()
            .await?;
        let html = login.text().await?;
        let regex = Regex::new(r#"gt=(.*);"#).unwrap();
        let captures = regex.captures(&html).unwrap();
        let capture = captures.get(1).expect("Couldn't match a guest token in the twitter homepage, the site was changed");
        Ok(ProviderCredentials {
            access_token: capture.as_str().to_owned(),
            refresh_token: "".to_owned(),
        })
    }

    fn on_error(&self, error: &HttpError) -> anyhow::Result<ProviderErrorHandle> {
        match error {
            HttpError::FailStatus(e) | HttpError::UnexpectedBody(e) => {
                if e.code == 429 {
                    Ok(ProviderErrorHandle::Login)
                } else {
                    // unknown error at this point
                    error!("{:?}", e);
                    Ok(ProviderErrorHandle::Halt)
                }
            }
            error @ _ => {
                error!("{:?}", error);
                Ok(ProviderErrorHandle::Halt)
            }
        }
    }
}

// Example code that deserializes and serializes the model.
// extern crate serde;
// #[macro_use]
// extern crate serde_derive;
// extern crate serde_json;
//
// use generated_module::[object Object];
//
// fn main() {
//     let json = r#"{"answer": 42}"#;
//     let model: [object Object] = serde_json::from_str(&json).unwrap();
// }

