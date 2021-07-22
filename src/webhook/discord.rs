use serde::Serialize;

use crate::scraper::ProviderMedia;

#[derive(Debug, Serialize)]
pub struct DiscordImage {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct DiscordEmbed<'a> {
    #[serde(rename = "type")]
    pub _type: &'a str,
    pub image: DiscordImage,
}

#[derive(Debug, Serialize)]
pub struct DiscordPayload<'a> {
    pub username: &'a str,
    pub avatar_url: &'a str,
    // pub content: &'a str,
    pub embeds: Vec<DiscordEmbed<'a>>,
}

pub fn is_discord_webhook_url(url: &str) -> bool {
    url.starts_with("https://discord.com/api/webhooks")
}

pub const DISCORD_IMAGE_DISPLAY_LIMIT: usize = 8;

pub fn discord_payload<'a>(media: &Vec<&ProviderMedia>) -> DiscordPayload<'a> {
    let size = media.len();
    DiscordPayload {
        username: "Jiu",
        avatar_url: "https://i.imgur.com/GkXttv3.png",
        // content: format!(
        //     "{} new image{} discovered",
        //     size,
        //     // can't be 0 obviously
        //     if size > 1 { "s" } else { "" }
        // ),
        embeds: media
            .iter()
            .map(|embed| DiscordEmbed {
                _type: "image",
                image: DiscordImage {
                    url: embed.image_url.clone(),
                },
            })
            .collect(),
    }
}
