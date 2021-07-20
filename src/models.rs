use crate::scraper::{Providers, ScrapedMedia};
use serde::{Deserialize, Serialize};

/// A Media entity builds on top of the data collected by a scraper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    /// The provider this image came from
    pub provider: Providers,
    pub data: ScrapedMedia,
}

impl Media {
    pub fn new(data: ScrapedMedia, provider: Providers) -> Self {
        Media { data, provider }
    }
}
