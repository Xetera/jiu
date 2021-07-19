pub mod pinterest_board_feed;
mod providers;
pub use pinterest_board_feed::*;
pub use providers::*;

/// A scrape url is only transparently available to providers
#[derive(Debug)]
pub struct ScrapeUrl(String);
