pub mod pinterest_board_feed;
mod providers;
pub mod weverse;
pub use pinterest_board_feed::*;
pub use providers::*;
pub use weverse::fetch_weverse_auth_token;
pub use weverse::WeverseArtistFeed;

/// A scrape url is only transparently available to providers
#[derive(Debug, Clone)]
pub struct ScrapeUrl(pub String);

#[derive(Debug, Copy, Clone)]
pub struct PageSize(usize);

/// Identifier for a specific section of a site
/// [name: pinterest.board_feed]
/// [destination: <A unique identifier scoped to pinterest>]
#[derive(Debug, PartialEq, Eq)]
pub struct ScopedProvider {
    pub name: AllProviders,
    pub destination: String,
}
