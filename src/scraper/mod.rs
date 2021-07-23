mod providers;
pub use providers::{
    PinterestBoardFeed, Provider, ProviderFailure, ProviderMedia, ProviderResult,
    ScrapeRequestInput,
};
pub mod scraper;
pub use scraper::Scrape;
