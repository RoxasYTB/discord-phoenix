use serde::{Deserialize, Serialize};

use super::{ChannelId, EmojiId, GuildId, MessageId, Permissions, Snowflake, User, UserId, null_default};

int_enum! {
    pub enum ChannelType: u8 {
        GuildText = 0,
        Dm = 1,
        GuildVoice = 2,
        GroupDm = 3,
        GuildCategory = 4,
        GuildAnnouncement = 5,
        AnnouncementThread = 10,
        PublicThread = 11,
        PrivateThread = 12,
        GuildStageVoice = 13,
        GuildDirectory = 14,
        GuildForum = 15,
        GuildMedia = 16,
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ChannelType {
    fn default() -> Self {
        Self::GuildText
    }
}

impl ChannelType {
    pub fn is_thread(self) -> bool {
        matches!(self, Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread)
    }
    pub fn is_private(self) -> bool {
        matches!(self, Self::Dm | Self::GroupDm)
    }
    pub fn is_voice(self) -> bool {
        matches!(self, Self::GuildVoice | Self::GuildStageVoice)
    }
    pub fn is_text_like(self) -> bool {
        matches!(
            self,
            Self::GuildText
                | Self::Dm
                | Self::GroupDm
                | Self::GuildAnnouncement
                | Self::AnnouncementThread
                | Self::PublicThread
                | Self::PrivateThread
                | Self::GuildVoice
                | Self::GuildStageVoice
        )
    }
}

int_enum! {
    pub enum OverwriteType: u8 {
        Role = 0,
        Member = 1,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionOverwrite {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: OverwriteType,
    #[serde(default)]
    pub allow: Permissions,
    #[serde(default)]
    pub deny: Permissions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ThreadMetadata {
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub auto_archive_duration: Option<u32>,
    #[serde(default)]
    pub archive_timestamp: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub invitable: Option<bool>,
    #[serde(default)]
    pub create_timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForumTag {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub moderated: bool,
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    #[serde(default)]
    pub emoji_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DefaultReaction {
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    #[serde(default)]
    pub emoji_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Channel {
    pub id: ChannelId,
    #[serde(rename = "type", default)]
    pub kind: ChannelType,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub position: Option<i32>,
    #[serde(default, deserialize_with = "null_default")]
    pub permission_overwrites: Vec<PermissionOverwrite>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub nsfw: bool,
    #[serde(default)]
    pub last_message_id: Option<MessageId>,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub user_limit: Option<u32>,
    #[serde(default)]
    pub rate_limit_per_user: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub recipients: Vec<User>,
    #[serde(default, deserialize_with = "null_default")]
    pub recipient_ids: Vec<UserId>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: Option<UserId>,
    #[serde(default)]
    pub parent_id: Option<ChannelId>,
    #[serde(default)]
    pub last_pin_timestamp: Option<String>,
    #[serde(default)]
    pub rtc_region: Option<String>,
    #[serde(default)]
    pub thread_metadata: Option<ThreadMetadata>,
    #[serde(default)]
    pub member_count: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub member_ids_preview: Vec<UserId>,
    #[serde(default)]
    pub member: Option<serde_json::Value>,
    #[serde(default)]
    pub message_count: Option<u32>,
    #[serde(default)]
    pub total_message_sent: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub flags: u64,
    #[serde(default, deserialize_with = "null_default")]
    pub available_tags: Vec<ForumTag>,
    #[serde(default, deserialize_with = "null_default")]
    pub applied_tags: Vec<Snowflake>,
    #[serde(default)]
    pub default_reaction_emoji: Option<DefaultReaction>,
    #[serde(default)]
    pub default_sort_order: Option<u8>,
    #[serde(default)]
    pub default_forum_layout: Option<u8>,
    #[serde(default, deserialize_with = "null_default")]
    pub is_message_request: bool,
}

pub const CHANNEL_FLAG_GUILD_RESOURCE: u64 = 1 << 7;

impl Channel {
    pub fn is_guild_resource(&self) -> bool {
        self.flags & CHANNEL_FLAG_GUILD_RESOURCE != 0
    }

    pub fn display_name(&self) -> String {
        if let Some(n) = self.name.as_deref().filter(|n| !n.is_empty()) {
            return n.to_owned();
        }
        let names: Vec<&str> = self.recipients.iter().map(|u| u.display_name()).collect();
        if names.is_empty() {
            "Unnamed".to_owned()
        } else {
            names.join(", ")
        }
    }

    pub fn recipient_user_ids(&self) -> Vec<UserId> {
        if self.recipients.is_empty() {
            self.recipient_ids.clone()
        } else {
            self.recipients.iter().map(|u| u.id).collect()
        }
    }
}
