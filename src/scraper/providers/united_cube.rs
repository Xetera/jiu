use super::{default_jitter, Provider, RateLimitable, SharedCredentials};
use crate::{scheduler::UnscopedLimiter, scraper::ProviderCredentials};
use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, sync::Arc};

struct UnitedCubeArtistFeed {
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
struct UnitedCubeLogin {
    refresh_token: Option<String>,
    path: String,
    id: String,
    pw: String,
    remember_me: bool,
}

#[derive(Deserialize)]
struct UnitedCubeLoginResponse {
    slug: String,
    email: String,
    name: String,
    language: String,
    role_code: String,
    token: String,
    refresh_token: String,
}

#[async_trait]
impl Provider for UnitedCubeArtistFeed {
    fn id(&self) -> super::AllProviders {
        super::AllProviders::UnitedCubeArtistFeed
    }
    fn new(input: super::ProviderInput) -> Self
    where
        Self: Sized,
    {
        Self {
            client: input.client,
            credentials: input.credentials,
            rate_limiter: Self::rate_limiter(),
        }
    }

    fn credentials(&self) -> Arc<RwLock<ProviderCredentials>> {
        self.credentials.clone().unwrap()
    }
    async fn login(&self) -> anyhow::Result<ProviderCredentials> {
        let response = self
            .client
            .post("https://united-cube.com/v1/auth/login")
            .json(&UnitedCubeLogin {
                refresh_token: None,
                path: "https://www.united-cube.com/signin".to_owned(),
                id: env::var("UNITED_CUBE_USERNAME").unwrap(),
                pw: env::var("UNITED_CUBE_PASSWORD").unwrap(),
                remember_me: false,
            })
            .send()
            .await?
            .json::<UnitedCubeLoginResponse>()
            .await?;
        Ok(ProviderCredentials {
            access_token: response.token,
            refresh_token: response.refresh_token,
        })
        // todo!()
    }
}
