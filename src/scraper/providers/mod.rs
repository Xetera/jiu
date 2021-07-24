pub mod pinterest_board_feed;
mod providers;
pub mod weverse;
use chrono::{DateTime, Utc};
pub use pinterest_board_feed::*;
pub use providers::*;

/// A scrape url is only transparently available to providers
#[derive(Debug, Clone)]
pub struct ScrapeUrl(String);

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
