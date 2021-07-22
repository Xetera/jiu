use chrono::{DateTime, NaiveDateTime, Utc};

#[derive(Debug)]
pub struct ProviderResource {
    pub id: i32,
    pub destination: String,
    pub name: String,
    pub priority: i32,
}
