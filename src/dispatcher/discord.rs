use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DiscordImage {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct DiscordEmbed {
    pub image: DiscordImage,
}

#[derive(Debug, Serialize)]
pub struct DiscordPayload<'a> {
    pub username: &'a str,
    pub avatar_url: &'a str,
    pub content: String, // Vec<DiscordEmbed>,
}

pub fn is_discord_webhook_url(url: &str) -> bool {
    url.starts_with("https://discord.com/api/webhooks")
}
