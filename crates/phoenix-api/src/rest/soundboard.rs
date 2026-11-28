use reqwest::Method;

use super::RestClient;
use crate::error::Result;
use crate::models::SoundboardSound;

impl RestClient {
    pub async fn soundboard_default_sounds(&self) -> Result<Vec<SoundboardSound>> {
        self.call(Method::GET, "/soundboard-default-sounds", super::Body::None).await
    }
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub struct TopSoundboardSound {
    pub sound_id: crate::models::Snowflake,
    #[serde(default)]
    pub sound_rank: u32,
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Default)]
pub struct TopSoundboardSounds {
    #[serde(default)]
    pub items: Vec<TopSoundboardSound>,
}

impl TopSoundboardSounds {
    pub fn ranked(mut self) -> Vec<crate::models::Snowflake> {
        self.items.sort_by_key(|i| i.sound_rank);
        self.items.into_iter().map(|i| i.sound_id).collect()
    }
}

impl RestClient {
    pub async fn top_soundboard_sounds(&self, guild: crate::models::GuildId) -> Result<TopSoundboardSounds> {
        self.call(Method::GET, &format!("/guilds/{}/top-soundboard-sounds", guild.0), super::Body::None).await
    }

    pub async fn play_entrance_sound(&self, channel: crate::models::ChannelId) -> Result<()> {
        let body = serde_json::json!({"animation_type": 1, "animation_id": 0});
        self.call_empty(Method::POST, &format!("/channels/{}/custom-call-sounds", channel.0), super::Body::Json(body)).await
    }

    pub async fn send_soundboard_sound(
        &self,
        channel: crate::models::ChannelId,
        sound: crate::models::Snowflake,
        source_guild: Option<crate::models::GuildId>,
        emoji_id: Option<crate::models::EmojiId>,
        emoji_name: Option<&str>,
    ) -> Result<()> {
        let mut body = serde_json::json!({
            "sound_id": sound.0.to_string(),
            "emoji_id": emoji_id.map(|e| e.0.to_string()),
            "emoji_name": emoji_name,
        });
        if let Some(g) = source_guild.filter(|g| g.0 != 0) {
            body["source_guild_id"] = serde_json::Value::String(g.0.to_string());
        }
        self.call_empty(Method::POST, &format!("/channels/{}/send-soundboard-sound", channel.0), super::Body::Json(body)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_sounds_rank_order() {
        let t: TopSoundboardSounds = serde_json::from_str(
            r#"{"items":[{"sound_id":"3","sound_rank":2,"selections":60},{"sound_id":"9","sound_rank":1,"selections":337}]}"#,
        )
        .unwrap();
        assert_eq!(t.ranked(), vec![crate::models::Snowflake(9), crate::models::Snowflake(3)]);
    }
}
