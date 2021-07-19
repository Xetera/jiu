use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Image {
    /// The date the image was discovered at
    pub discovered_at: DateTime<Utc>,
    /// Images are represented by the highest available quality
    pub url: String,
    pub id: String,
    // we could technically put width x height in here but we can't guarantee
    // that we can get this information from all providers
}
