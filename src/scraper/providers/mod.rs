pub mod pinterest;
mod providers;
pub mod weverse;
use governor::clock::QuantaClock;
use governor::state::InMemoryState;
use governor::state::NotKeyed;
use governor::RateLimiter;
pub use pinterest::*;
pub use providers::*;
use std::fmt::Display;
pub use weverse::fetch_weverse_auth_token;
pub use weverse::WeverseArtistFeed;

/// A scrape url is only transparently available to providers
#[derive(Debug, Clone)]
pub struct ScrapeUrl(pub String);

#[derive(Debug, Copy, Clone)]
pub struct PageSize(usize);

/// Providers must be rate limited at the domain level and not at the page level
/// in order to prevent exceeding rate limits
pub type ProviderLimiter = RateLimiter<NotKeyed, InMemoryState, QuantaClock>;

/// Identifier for a specific section of a site
/// [name: pinterest.board_feed]
/// [destination: <A unique identifier scoped to pinterest>]
#[derive(Debug, PartialEq, Eq)]
pub struct ScopedProvider {
    pub name: AllProviders,
    pub destination: String,
}

impl Display for ScopedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}:{}", self.name.to_string(), self.destination))
    }
}
