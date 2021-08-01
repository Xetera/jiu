pub mod pinterest;
mod providers;
pub mod weverse;
use parking_lot::RwLock;
pub use pinterest::*;
pub use providers::*;
use reqwest::Client;
use std::collections::HashMap;
use std::fmt::Display;
use std::iter::FromIterator;
use std::sync::Arc;
use strum::IntoEnumIterator;
pub use weverse::*;

/// A scrape url is only transparently available to providers
#[derive(Debug, Clone)]
pub struct ScrapeUrl(pub String);

#[derive(Debug, Copy, Clone)]
pub struct PageSize(usize);

/// Identifier for a specific section of a site
/// [name: pinterest.board_feed]
/// [destination: <A unique identifier scoped to pinterest>]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ScopedProvider {
    pub name: AllProviders,
    pub destination: String,
}

impl Display for ScopedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}:{}", self.name.to_string(), self.destination))
    }
}

pub async fn providers(
    client: Arc<Client>,
) -> anyhow::Result<HashMap<AllProviders, Box<dyn Provider>>> {
    let credentials = fetch_weverse_auth_token(&client).await?;
    Ok(HashMap::from_iter(AllProviders::iter().map(
        |provider_type| {
            let client = Arc::clone(&client);
            let input = ProviderInput {
                client,
                credentials: match provider_type {
                    AllProviders::WeverseArtistFeed => {
                        Some(Arc::new(RwLock::new(credentials.clone().unwrap())))
                    }
                    _ => None,
                },
            };
            let provider: Box<dyn Provider> = match &provider_type {
                AllProviders::PinterestBoardFeed => Box::new(PinterestBoardFeed::new(input)),
                AllProviders::WeverseArtistFeed => Box::new(WeverseArtistFeed::new(input)),
            };
            (provider_type, provider)
        },
    )))
}
