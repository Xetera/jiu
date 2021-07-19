// Images are represented by the highest available quality
#[derive(Debug, Clone)]
pub struct Image {
    pub url: String,
    pub id: String,
    // we could technically put width x height in here but we can't guarantee
    // that we can get this information from all providers
}
