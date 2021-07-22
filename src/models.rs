use chrono::NaiveDateTime;

#[derive(Debug)]
pub struct DatabaseWebhook {
    pub id: i32,
    pub destination: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct DatabaseWebhookSource {
    pub id: i32,
    pub webhook_id: i32,
    pub provider_destination: String,
}
