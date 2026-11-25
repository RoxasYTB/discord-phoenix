use serde::{Deserialize, Serialize};

use super::{Channel, ChannelId, EmojiId, GuildId, Member, MessageId, RoleId, Snowflake, User, null_default};

int_enum! {
    pub enum MessageType: u8 {
        Default = 0,
        RecipientAdd = 1,
        RecipientRemove = 2,
        Call = 3,
        ChannelNameChange = 4,
        ChannelIconChange = 5,
        ChannelPinnedMessage = 6,
        UserJoin = 7,
        GuildBoost = 8,
        GuildBoostTier1 = 9,
        GuildBoostTier2 = 10,
        GuildBoostTier3 = 11,
        ChannelFollowAdd = 12,
        GuildDiscoveryDisqualified = 14,
        GuildDiscoveryRequalified = 15,
        ThreadCreated = 18,
        Reply = 19,
        ChatInputCommand = 20,
        ThreadStarterMessage = 21,
        GuildInviteReminder = 22,
        ContextMenuCommand = 23,
        AutoModerationAction = 24,
        RoleSubscriptionPurchase = 25,
        StageStart = 27,
        StageEnd = 28,
        StageSpeaker = 29,
        StageTopic = 31,
        PollResult = 46,
    }
}

#[allow(clippy::derivable_impls)]
impl Default for MessageType {
    fn default() -> Self {
        Self::Default
    }
}

impl MessageType {
    pub fn is_regular(self) -> bool {
        matches!(
            self,
            Self::Default | Self::Reply | Self::ChatInputCommand | Self::ContextMenuCommand | Self::ThreadStarterMessage
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Nonce {
    Int(i64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EmbedFooter {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub proxy_icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EmbedMedia {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EmbedProvider {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EmbedAuthor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub proxy_icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EmbedField {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Embed {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub color: Option<u32>,
    #[serde(default)]
    pub footer: Option<EmbedFooter>,
    #[serde(default)]
    pub image: Option<EmbedMedia>,
    #[serde(default)]
    pub thumbnail: Option<EmbedMedia>,
    #[serde(default)]
    pub video: Option<EmbedMedia>,
    #[serde(default)]
    pub provider: Option<EmbedProvider>,
    #[serde(default)]
    pub author: Option<EmbedAuthor>,
    #[serde(default, deserialize_with = "null_default")]
    pub fields: Vec<EmbedField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Attachment {
    pub id: Snowflake,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub ephemeral: bool,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub waveform: Option<String>,
    #[serde(default)]
    pub flags: u64,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        self.content_type.as_deref().is_some_and(|c| c.starts_with("image/"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MessageSoundboardSound {
    #[serde(default)]
    pub sound_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    #[serde(default)]
    pub emoji_name: Option<String>,
}

#[cfg(test)]
mod soundboard_tests {
    use super::*;

    #[test]
    fn message_soundboard_metadata_defaults_and_ignores_extra_fields() {
        let sound: MessageSoundboardSound = serde_json::from_str(
            r#"{"sound_id":"77","guild_id":"9","name":"Chime","emoji_name":"🎵","volume":0.4,"available":true}"#,
        )
        .unwrap();
        assert_eq!(sound.sound_id, Snowflake(77));
        assert_eq!(sound.guild_id, Some(Snowflake(9)));
        assert_eq!(sound.name.as_deref(), Some("Chime"));
        assert_eq!(sound.emoji_name.as_deref(), Some("🎵"));

        let empty: MessageSoundboardSound = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, MessageSoundboardSound::default());
    }

    #[test]
    fn message_soundboard_list_accepts_null() {
        let message: Message = serde_json::from_str(r#"{"id":"1","channel_id":"2","author":{"id":"3"},"soundboard_sounds":null}"#).unwrap();
        assert!(message.soundboard_sounds.is_empty());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ReactionEmoji {
    #[serde(default)]
    pub id: Option<EmojiId>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
}

impl ReactionEmoji {
    pub fn unicode(s: impl Into<String>) -> Self {
        Self { id: None, name: Some(s.into()), animated: false }
    }

    pub fn route_form(&self) -> String {
        match (self.id, &self.name) {
            (Some(id), Some(n)) => format!("{n}:{id}"),
            (Some(id), None) => format!("_:{id}"),
            (None, Some(n)) => n.clone(),
            (None, None) => String::new(),
        }
    }

    pub fn same_as(&self, other: &ReactionEmoji) -> bool {
        match (self.id, other.id) {
            (Some(a), Some(b)) => a == b,
            (None, None) => self.name == other.name,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReactionCountDetails {
    #[serde(default)]
    pub burst: u32,
    #[serde(default)]
    pub normal: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Reaction {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub count_details: ReactionCountDetails,
    #[serde(default)]
    pub me: bool,
    #[serde(default)]
    pub me_burst: bool,
    pub emoji: ReactionEmoji,
    #[serde(default, deserialize_with = "null_default")]
    pub burst_colors: Vec<String>,
}

int_enum! {
    pub enum StickerFormat: u8 {
        Png = 1,
        Apng = 2,
        Lottie = 3,
        Gif = 4,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StickerItem {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    pub format_type: StickerFormat,
}

impl StickerItem {
    pub fn url(&self) -> String {
        let ext = match self.format_type {
            StickerFormat::Lottie => "json",
            StickerFormat::Gif => "gif",
            _ => "png",
        };
        format!("{}/stickers/{}.{ext}", super::CDN_BASE, self.id)
    }
}

int_enum! {
    pub enum MessageReferenceType: u8 {
        Default = 0,
        Forward = 1,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MessageReference {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MessageReferenceType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_if_not_exists: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelMention {
    pub id: ChannelId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(rename = "type", default)]
    pub kind: super::ChannelType,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Mention {
    #[serde(flatten)]
    pub user: User,
    #[serde(default)]
    pub member: Option<Member>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Message {
    pub id: MessageId,
    pub channel_id: ChannelId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub author: User,
    #[serde(default)]
    pub member: Option<Member>,
    #[serde(default, deserialize_with = "null_default")]
    pub content: String,
    #[serde(default, deserialize_with = "null_default")]
    pub timestamp: String,
    #[serde(default)]
    pub edited_timestamp: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub tts: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub mention_everyone: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub mentions: Vec<Mention>,
    #[serde(default, deserialize_with = "null_default")]
    pub mention_roles: Vec<RoleId>,
    #[serde(default, deserialize_with = "null_default")]
    pub mention_channels: Vec<ChannelMention>,
    #[serde(default, deserialize_with = "null_default")]
    pub attachments: Vec<Attachment>,
    #[serde(default, deserialize_with = "null_default")]
    pub embeds: Vec<Embed>,
    #[serde(default, deserialize_with = "null_default")]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub nonce: Option<Nonce>,
    #[serde(default, deserialize_with = "null_default")]
    pub pinned: bool,
    #[serde(default)]
    pub webhook_id: Option<Snowflake>,
    #[serde(default)]
    pub application_id: Option<Snowflake>,
    #[serde(rename = "type", default)]
    pub kind: MessageType,
    #[serde(default, deserialize_with = "null_default")]
    pub flags: u64,
    #[serde(default)]
    pub message_reference: Option<MessageReference>,
    #[serde(default)]
    pub referenced_message: Option<Box<Message>>,
    #[serde(default, deserialize_with = "null_default")]
    pub sticker_items: Vec<StickerItem>,
    #[serde(default, deserialize_with = "null_default")]
    pub soundboard_sounds: Vec<MessageSoundboardSound>,
    #[serde(default)]
    pub thread: Option<Box<Channel>>,
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<super::Component>,
    #[serde(default)]
    pub poll: Option<serde_json::Value>,
    #[serde(default)]
    pub message_snapshots: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_metadata: Option<InteractionMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<MessageInteraction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<MessageCall>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MessageCall {
    #[serde(default, deserialize_with = "null_default")]
    pub participants: Vec<Snowflake>,
    #[serde(default)]
    pub ended_timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InteractionMetadata {
    #[serde(default)]
    pub id: Snowflake,
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub command_type: Option<u8>,
    #[serde(default)]
    pub target_user: Option<User>,
    #[serde(default)]
    pub target_message_id: Option<MessageId>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub authorizing_integration_owners: std::collections::HashMap<String, Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MessageInteraction {
    #[serde(default)]
    pub id: Snowflake,
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub user: Option<User>,
}

impl Message {
    pub fn command_use(&self) -> Option<(Snowflake, &User, String, u8, Option<&User>)> {
        if let Some(m) = &self.interaction_metadata
            && let Some(u) = &m.user
        {
            let name = m.name.clone().or_else(|| self.interaction.as_ref().map(|i| i.name.clone())).unwrap_or_default();
            return Some((m.id, u, name, m.command_type.unwrap_or(1), m.target_user.as_ref()));
        }
        let i = self.interaction.as_ref()?;
        Some((i.id, i.user.as_ref()?, i.name.clone(), 1, None))
    }

    pub fn is_reply(&self) -> bool {
        self.kind == MessageType::Reply && self.message_reference.is_some()
    }

    pub fn mentions_user(&self, user: super::UserId) -> bool {
        self.mentions.iter().any(|m| m.user.id == user)
    }

    pub fn apply_reaction(&mut self, emoji: &ReactionEmoji, me: bool, add: bool, burst: bool) {
        let pos = self.reactions.iter().position(|r| r.emoji.same_as(emoji));
        match (pos, add) {
            (Some(i), true) => {
                let r = &mut self.reactions[i];
                r.count += 1;
                if burst { r.count_details.burst += 1 } else { r.count_details.normal += 1 }
                if me { if burst { r.me_burst = true } else { r.me = true } }
            }
            (None, true) => self.reactions.push(Reaction {
                count: 1,
                count_details: ReactionCountDetails { burst: burst as u32, normal: (!burst) as u32 },
                me: me && !burst,
                me_burst: me && burst,
                emoji: emoji.clone(),
                burst_colors: Vec::new(),
            }),
            (Some(i), false) => {
                let r = &mut self.reactions[i];
                r.count = r.count.saturating_sub(1);
                if burst {
                    r.count_details.burst = r.count_details.burst.saturating_sub(1);
                } else {
                    r.count_details.normal = r.count_details.normal.saturating_sub(1);
                }
                if me { if burst { r.me_burst = false } else { r.me = false } }
                if r.count == 0 {
                    self.reactions.remove(i);
                }
            }
            (None, false) => {}
        }
    }
}
