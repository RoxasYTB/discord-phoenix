use serde::{Deserialize, Serialize};

use super::{EmojiId, GuildId, Snowflake, UserId};

/// A Discord Soundboard sound (`GET /soundboard-default-sounds` or Gateway
/// `SOUNDBOARD_SOUNDS`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SoundboardSound {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sound_id: Snowflake,
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    #[serde(default)]
    pub emoji_name: Option<String>,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default = "default_available")]
    pub available: bool,
    #[serde(default)]
    pub user_id: Option<UserId>,
}

fn default_volume() -> f64 {
    1.0
}

fn default_available() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundboard_sound_deserializes() {
        let s: SoundboardSound = serde_json::from_str(
            r#"{"name":"quack","sound_id":"1","volume":0.8,"emoji_id":null,"emoji_name":"🦆","guild_id":"42","available":true,"user_id":"7","extra":123}"#,
        )
        .unwrap();
        assert_eq!(s.name, "quack");
        assert_eq!(s.sound_id, Snowflake(1));
        assert_eq!(s.guild_id, Some(GuildId(42)));
        assert_eq!(s.emoji_name.as_deref(), Some("🦆"));
        assert!(s.available);
    }
}
