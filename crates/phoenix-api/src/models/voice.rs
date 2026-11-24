use serde::{Deserialize, Serialize};

use super::{ChannelId, GuildId, Member, UserId, null_default};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VoiceState {
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    pub user_id: UserId,
    #[serde(default)]
    pub member: Option<Member>,
    #[serde(default, deserialize_with = "null_default")]
    pub session_id: String,
    #[serde(default, deserialize_with = "null_default")]
    pub deaf: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub mute: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub self_deaf: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub self_mute: bool,
    #[serde(default)]
    pub self_stream: Option<bool>,
    #[serde(default, deserialize_with = "null_default")]
    pub self_video: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub suppress: bool,
    #[serde(default)]
    pub request_to_speak_timestamp: Option<String>,
    #[serde(default)]
    pub discoverable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceServerUpdate {
    pub token: String,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    #[serde(default)]
    pub endpoint: Option<String>,
}
