use serde::{Deserialize, Serialize};

use super::{ChannelId, GuildId, Member, UserId, null_default};

/// VOICE_STATE_UPDATE payload / guild voice state entry.
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
    /// `false` when the user hides their voice activity: the client then leaves them out of
    /// the member list / profile activity (`getDiscoverableVoiceState`). Absent = `true`.
    #[serde(default)]
    pub discoverable: Option<bool>,
}

/// VOICE_SERVER_UPDATE payload: what the voice crate needs to connect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceServerUpdate {
    pub token: String,
    /// Guild id, absent for DM/group calls (then `channel_id` is set).
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    /// `None` means the voice server went away; wait for another update.
    #[serde(default)]
    pub endpoint: Option<String>,
}
