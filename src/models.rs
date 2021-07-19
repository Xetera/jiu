use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub discovered_at: DateTime<Utc>,
    /// Images are represented by the highest available quality
    pub url: String,
    pub id: String,
    // we could technically put width x height in here but we can't guarantee
    // that we can get this information from all providers
}
