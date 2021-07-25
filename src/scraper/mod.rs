mod providers;
pub use providers::{
    fetch_weverse_auth_token, AllProviders, PinterestBoardFeed, Provider, ProviderFailure,
    ProviderMedia, ProviderResult, ScopedProvider, ScrapeRequestInput, WeverseArtistFeed,
};
pub mod scraper;
