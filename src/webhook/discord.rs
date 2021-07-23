use crate::scraper::ProviderMedia;
use reqwest::Url;
use serde::Serialize;
use std::iter::Iterator;

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

pub fn add_wait_parameter(url: &str) -> Result<Url, url::ParseError> {
    Url::parse_with_params(url, &[("wait", "true")])
}

pub const DISCORD_IMAGE_DISPLAY_LIMIT: usize = 8;

pub fn discord_payload<'a>(media: Vec<&ProviderMedia>) -> DiscordPayload<'a> {
    let media_links = media
        .iter()
        .map(|embed| embed.image_url.clone())
        .collect::<Vec<String>>()
        .join("\n");
    DiscordPayload {
        username: "Jiu",
        avatar_url: "https://i.imgur.com/GkXttv3.png",
        // content: format!(""),
        content: format!("{}", media_links),
    }
}
