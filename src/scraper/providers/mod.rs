pub mod pinterest;
use futures::future::join_all;
pub use pinterest::*;
mod providers;
pub use providers::*;
pub mod weverse;
pub use weverse::*;
pub mod united_cube;
use reqwest::Client;
use std::collections::HashMap;
use std::fmt::Display;
use std::iter::FromIterator;
use std::sync::Arc;
use strum::IntoEnumIterator;
pub use united_cube::*;

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

pub type ProviderMap = HashMap<AllProviders, Box<dyn Provider>>;

pub async fn get_provider_map(client: &Arc<Client>) -> anyhow::Result<ProviderMap> {
    let handles = AllProviders::iter().map(|provider_type| async move {
        let client = Arc::clone(client);
        let input = ProviderInput { client };
        let provider: Box<dyn Provider> = match provider_type {
            AllProviders::PinterestBoardFeed => Box::new(PinterestBoardFeed::new(input)),
            AllProviders::WeverseArtistFeed => Box::new(WeverseArtistFeed::new(input)),
            AllProviders::UnitedCubeArtistFeed => Box::new(UnitedCubeArtistFeed::new(input)),
        };
        provider.initialize().await;
        (provider_type, provider)
    });
    let results = join_all(handles).await;
    Ok(HashMap::from_iter(results))
}
