use self::discord::is_discord_webhook_url;

mod discord;
pub mod dispatcher;

pub enum WebhookDestination {
    Discord,
    Custom,
}

pub fn webhook_type(url: &str) -> WebhookDestination {
    if is_discord_webhook_url(url) {
        WebhookDestination::Discord
    } else {
        WebhookDestination::Custom
    }
}
