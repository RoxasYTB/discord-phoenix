use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::*;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Ready {
    #[serde(default)]
    pub v: u8,
    pub user: User,
    pub session_id: String,
    #[serde(default)]
    pub resume_gateway_url: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub guilds: Vec<Guild>,
    #[serde(default, deserialize_with = "null_default")]
    pub private_channels: Vec<Channel>,
    #[serde(default, deserialize_with = "null_default")]
    pub connected_accounts: Vec<serde_json::Value>,
    #[serde(default, deserialize_with = "null_default")]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub game_relationships: Option<Value>,
    #[serde(default, deserialize_with = "null_default")]
    pub users: Vec<User>,
    #[serde(default, deserialize_with = "null_default")]
    pub merged_members: Vec<Vec<Member>>,
    #[serde(default)]
    pub read_state: Option<Value>,
    #[serde(default)]
    pub user_guild_settings: Option<Value>,
    #[serde(default)]
    pub notification_settings: Option<Value>,
    #[serde(default)]
    pub user_settings: Option<Value>,
    #[serde(default)]
    pub user_settings_proto: Option<String>,
    #[serde(default)]
    pub sessions: Option<Value>,
    #[serde(default)]
    pub apex_experiments: Option<Value>,
    #[serde(default)]
    pub country_code: Option<String>,
    #[serde(default)]
    pub auth_session_id_hash: Option<String>,
    #[serde(default)]
    pub ad_personalization_toggles_disabled: Option<bool>,
    #[serde(default)]
    pub swp_message_promotion_locked: Option<bool>,
    #[serde(default)]
    pub swp_message_promotion_default_opted_out: Option<bool>,
    #[serde(default)]
    pub qos_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SupplementalGuild {
    pub id: GuildId,
    #[serde(default, deserialize_with = "null_default")]
    pub voice_states: Vec<VoiceState>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReadySupplemental {
    #[serde(default, deserialize_with = "null_default")]
    pub guilds: Vec<SupplementalGuild>,
    #[serde(default)]
    pub merged_presences: Option<Value>,
    #[serde(default)]
    pub merged_members: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDelete {
    pub id: MessageId,
    pub channel_id: ChannelId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeleteBulk {
    pub ids: Vec<MessageId>,
    pub channel_id: ChannelId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionEvent {
    pub user_id: UserId,
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub member: Option<Member>,
    pub emoji: ReactionEmoji,
    #[serde(default)]
    pub burst: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub burst_colors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionBatch {
    pub emoji: ReactionEmoji,
    #[serde(default, deserialize_with = "null_default")]
    pub users: Vec<UserId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionAddMany {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default, deserialize_with = "null_default")]
    pub reactions: Vec<ReactionBatch>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionRemoveAll {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub emoji: Option<ReactionEmoji>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypingStart {
    pub channel_id: ChannelId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    pub user_id: UserId,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub member: Option<Member>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnavailableGuild {
    pub id: GuildId,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildMemberRemove {
    pub guild_id: GuildId,
    pub user: User,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildMembersChunk {
    pub guild_id: GuildId,
    #[serde(default)]
    pub members: Vec<Member>,
    #[serde(default)]
    pub chunk_index: u32,
    #[serde(default)]
    pub chunk_count: u32,
    #[serde(default)]
    pub not_found: Vec<UserId>,
    #[serde(default)]
    pub presences: Vec<Value>,
    #[serde(default)]
    pub nonce: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildRoleEvent {
    pub guild_id: GuildId,
    pub role: Role,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildRoleDelete {
    pub guild_id: GuildId,
    pub role_id: RoleId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildEmojisUpdate {
    pub guild_id: GuildId,
    pub emojis: Vec<Emoji>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageAck {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    #[serde(default)]
    pub version: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PresenceUser {
    pub id: UserId,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub discriminator: Option<String>,
}

impl PresenceUser {
    pub fn merge_into(&self, u: &mut crate::models::User) -> bool {
        let Some(name) = &self.username else { return false };
        u.username = name.clone();
        u.global_name = self.global_name.clone();
        u.avatar = self.avatar.clone();
        if let Some(d) = &self.discriminator {
            u.discriminator = d.clone();
        }
        true
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PresenceUpdate {
    pub user: PresenceUser,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub activities: Vec<Value>,
    #[serde(default)]
    pub client_status: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberListItem {
    Group { id: String, #[serde(default)] count: u32 },
    Member(Box<MemberListMember>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemberListMember {
    #[serde(flatten)]
    pub member: Member,
    #[serde(default)]
    pub presence: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "UPPERCASE")]
pub enum MemberListOp {
    Sync { range: [u32; 2], #[serde(default)] items: Vec<MemberListItem> },
    Insert { index: u32, item: MemberListItem },
    Update { index: u32, item: MemberListItem },
    Delete { index: u32 },
    Invalidate { range: [u32; 2] },
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemberListGroup {
    pub id: String,
    #[serde(default)]
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildMemberListUpdate {
    pub guild_id: GuildId,
    pub id: String,
    #[serde(default)]
    pub member_count: u32,
    #[serde(default)]
    pub online_count: u32,
    #[serde(default)]
    pub groups: Vec<MemberListGroup>,
    #[serde(default)]
    pub ops: Vec<MemberListOp>,
}

#[derive(Debug, Clone)]
pub enum GatewayEvent {
    Connected { heartbeat_interval: Duration },
    HeartbeatAck { latency: Duration },
    Disconnected { code: Option<u16>, reason: String },
    Reconnecting { attempt: u32, delay: Duration, resume: bool },
    SessionReset,
    Fatal { code: u16, reason: String },

    Ready(Box<Ready>),
    ReadySupplemental(Box<ReadySupplemental>),
    Resumed,
    UserUpdate(Box<User>),

    GuildCreate(Box<Guild>),
    GuildUpdate(Box<Guild>),
    GuildDelete(UnavailableGuild),
    GuildMemberAdd(Box<Member>),
    GuildMemberUpdate(Box<Member>),
    GuildMemberRemove(Box<GuildMemberRemove>),
    GuildMembersChunk(Box<GuildMembersChunk>),
    GuildMemberListUpdate(Box<GuildMemberListUpdate>),
    GuildRoleCreate(GuildRoleEvent),
    GuildRoleUpdate(GuildRoleEvent),
    GuildRoleDelete(GuildRoleDelete),
    GuildEmojisUpdate(GuildEmojisUpdate),

    ChannelCreate(Box<Channel>),
    ChannelUpdate(Box<Channel>),
    ChannelDelete(Box<Channel>),
    ThreadCreate(Box<Channel>),
    ThreadUpdate(Box<Channel>),
    ThreadDelete(Box<Channel>),

    MessageCreate(Box<Message>),
    MessageUpdate(Box<Value>),
    MessageDelete(MessageDelete),
    MessageDeleteBulk(MessageDeleteBulk),
    MessageReactionAdd(Box<ReactionEvent>),
    MessageReactionRemove(Box<ReactionEvent>),
    MessageReactionAddMany(Box<ReactionAddMany>),
    MessageReactionRemoveAll(ReactionRemoveAll),
    MessageReactionRemoveEmoji(ReactionRemoveAll),
    MessageAck(MessageAck),
    TypingStart(Box<TypingStart>),
    PresenceUpdate(Box<PresenceUpdate>),

    RelationshipAdd(Box<Relationship>),
    RelationshipRemove(Box<Relationship>),

    VoiceStateUpdate(Box<VoiceState>),
    VoiceServerUpdate(VoiceServerUpdate),
    SoundboardSounds(Box<SoundboardSounds>),
    VoiceChannelEffectSend(Box<VoiceChannelEffectSend>),
    StreamCreate(Box<StreamCreate>),
    StreamServerUpdate(Box<StreamServerUpdate>),
    StreamDelete(Box<StreamDelete>),

    Unknown { name: String, data: Value },
}

macro_rules! parse_events {
    ($name:expr, $raw:expr, { $($ev:literal => $variant:ident $(($wrap:path))?),* $(,)? }) => {
        match $name {
            $( $ev => serde_json::from_str($raw).map(|v| GatewayEvent::$variant($($wrap)?(v))), )*
            _ => return GatewayEvent::Unknown { name: $name.to_owned(), data: serde_json::from_str($raw).unwrap_or(Value::Null) },
        }
    };
}

pub fn parse_dispatch(name: &str, raw: &str) -> GatewayEvent {
    if name == "RESUMED" {
        return GatewayEvent::Resumed;
    }
    let res: Result<GatewayEvent, serde_json::Error> = parse_events!(name, raw, {
        "READY" => Ready(Box::new),
        "READY_SUPPLEMENTAL" => ReadySupplemental(Box::new),
        "USER_UPDATE" => UserUpdate(Box::new),
        "GUILD_CREATE" => GuildCreate(Box::new),
        "GUILD_UPDATE" => GuildUpdate(Box::new),
        "GUILD_DELETE" => GuildDelete,
        "GUILD_MEMBER_ADD" => GuildMemberAdd(Box::new),
        "GUILD_MEMBER_UPDATE" => GuildMemberUpdate(Box::new),
        "GUILD_MEMBER_REMOVE" => GuildMemberRemove,
        "GUILD_MEMBERS_CHUNK" => GuildMembersChunk(Box::new),
        "GUILD_MEMBER_LIST_UPDATE" => GuildMemberListUpdate(Box::new),
        "GUILD_ROLE_CREATE" => GuildRoleCreate,
        "GUILD_ROLE_UPDATE" => GuildRoleUpdate,
        "GUILD_ROLE_DELETE" => GuildRoleDelete,
        "GUILD_EMOJIS_UPDATE" => GuildEmojisUpdate,
        "SOUNDBOARD_SOUNDS" => SoundboardSounds(Box::new),
        "VOICE_CHANNEL_EFFECT_SEND" => VoiceChannelEffectSend(Box::new),
        "STREAM_CREATE" => StreamCreate(Box::new),
        "STREAM_SERVER_UPDATE" => StreamServerUpdate(Box::new),
        "STREAM_DELETE" => StreamDelete(Box::new),
        "CHANNEL_CREATE" => ChannelCreate(Box::new),
        "CHANNEL_UPDATE" => ChannelUpdate(Box::new),
        "CHANNEL_DELETE" => ChannelDelete(Box::new),
        "THREAD_CREATE" => ThreadCreate(Box::new),
        "THREAD_UPDATE" => ThreadUpdate(Box::new),
        "THREAD_DELETE" => ThreadDelete(Box::new),
        "MESSAGE_CREATE" => MessageCreate(Box::new),
        "MESSAGE_UPDATE" => MessageUpdate(Box::new),
        "MESSAGE_DELETE" => MessageDelete,
        "MESSAGE_DELETE_BULK" => MessageDeleteBulk,
        "MESSAGE_REACTION_ADD" => MessageReactionAdd(Box::new),
        "MESSAGE_REACTION_REMOVE" => MessageReactionRemove(Box::new),
        "MESSAGE_REACTION_ADD_MANY" => MessageReactionAddMany(Box::new),
        "MESSAGE_REACTION_REMOVE_ALL" => MessageReactionRemoveAll,
        "MESSAGE_REACTION_REMOVE_EMOJI" => MessageReactionRemoveEmoji,
        "MESSAGE_ACK" => MessageAck,
        "TYPING_START" => TypingStart(Box::new),
        "PRESENCE_UPDATE" => PresenceUpdate(Box::new),
        "RELATIONSHIP_ADD" => RelationshipAdd(Box::new),
        "RELATIONSHIP_REMOVE" => RelationshipRemove(Box::new),
        "VOICE_STATE_UPDATE" => VoiceStateUpdate(Box::new),
        "VOICE_SERVER_UPDATE" => VoiceServerUpdate,
    });
    match res {
        Ok(ev) => ev,
        Err(e) => {
            tracing::warn!(event = name, error = %e, "failed to parse dispatch; passing through raw");
            GatewayEvent::Unknown { name: name.to_owned(), data: serde_json::from_str(raw).unwrap_or(Value::Null) }
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateVoiceState {
    pub guild_id: Option<GuildId>,
    pub channel_id: Option<ChannelId>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub self_video: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdatePresence {
    pub since: u64,
    pub activities: Vec<Value>,
    pub status: String,
    pub afk: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SoundboardSounds {
    pub guild_id: GuildId,
    #[serde(default, deserialize_with = "null_default")]
    pub soundboard_sounds: Vec<SoundboardSound>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct EffectEmoji {
    #[serde(default)]
    pub id: Option<crate::models::EmojiId>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct VoiceChannelEffectSend {
    pub channel_id: ChannelId,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    pub user_id: UserId,
    #[serde(default)]
    pub emoji: Option<EffectEmoji>,
    #[serde(default)]
    pub animation_type: Option<u8>,
    #[serde(default)]
    pub animation_id: Option<u32>,
    #[serde(default)]
    pub sound_id: Option<crate::models::Snowflake>,
    #[serde(default)]
    pub sound_volume: Option<f64>,
    #[serde(default)]
    pub source_guild_id: Option<GuildId>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct StreamCreate {
    pub stream_key: String,
    #[serde(default)]
    pub rtc_server_id: Option<String>,
    #[serde(default)]
    pub viewer_ids: Vec<UserId>,
    #[serde(default)]
    pub paused: bool,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct StreamServerUpdate {
    pub stream_key: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct StreamDelete {
    pub stream_key: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct RequestSoundboardSounds {
    pub guild_ids: Vec<GuildId>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct RequestGuildMembers {
    pub guild_id: Vec<GuildId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<UserId>>,
    pub presences: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

pub type Ranges = Vec<[u32; 2]>;

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct LazyGuildRequest {
    pub guild_id: GuildId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activities: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<UserId>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub channels: std::collections::BTreeMap<ChannelId, Ranges>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub thread_member_lists: Vec<ChannelId>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct GuildSubscription {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activities: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_updates: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<UserId>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub channels: std::collections::BTreeMap<ChannelId, Ranges>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub thread_member_lists: Vec<ChannelId>,
}

impl GuildSubscription {
    pub fn focused(channel: Option<ChannelId>) -> Self {
        let mut s = Self { typing: Some(true), threads: Some(true), activities: Some(true), ..Default::default() };
        if let Some(c) = channel {
            s.channels.insert(c, vec![[0, 99]]);
        }
        s
    }
}

#[cfg(test)]
mod voice_effect_tests {
    use super::*;

    #[test]
    fn parses_soundboard_effect() {
        let raw = r#"{"user_id":"7","sound_volume":0.5,"sound_id":"42","source_guild_id":"9","guild_id":"9","emoji":{"name":"🦆","id":null},"channel_id":"5","animation_type":null,"animation_id":null}"#;
        let GatewayEvent::VoiceChannelEffectSend(e) = parse_dispatch("VOICE_CHANNEL_EFFECT_SEND", raw) else { panic!("not parsed") };
        assert_eq!(e.sound_id, Some(Snowflake(42)));
        assert_eq!(e.sound_volume, Some(0.5));
        assert_eq!(e.emoji.and_then(|e| e.name).as_deref(), Some("🦆"));
        assert_eq!(e.channel_id, Snowflake(5));
    }
}
