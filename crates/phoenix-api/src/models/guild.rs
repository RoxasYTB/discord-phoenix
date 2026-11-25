use serde::{Deserialize, Serialize};

use super::{Channel, ChannelId, EmojiId, GuildId, Member, Permissions, Role, RoleId, User, UserId, VoiceState, CDN_BASE, null_default};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Emoji {
    #[serde(default)]
    pub id: Option<EmojiId>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub roles: Vec<RoleId>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default, deserialize_with = "null_default")]
    pub require_colons: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub managed: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub animated: bool,
    #[serde(default = "yes")]
    pub available: bool,
}

fn yes() -> bool {
    true
}

impl Emoji {
    pub fn url(&self) -> Option<String> {
        let ext = if self.animated { "gif" } else { "webp" };
        self.id.map(|id| format!("{CDN_BASE}/emojis/{id}.{ext}"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GuildProperties {
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub splash: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner_id: Option<UserId>,
    #[serde(default, deserialize_with = "null_default")]
    pub features: Vec<String>,
    #[serde(default)]
    pub verification_level: Option<u8>,
    #[serde(default)]
    pub mfa_level: Option<u8>,
    #[serde(default)]
    pub nsfw_level: Option<u8>,
    #[serde(default)]
    pub preferred_locale: Option<String>,
    #[serde(default)]
    pub afk_channel_id: Option<ChannelId>,
    #[serde(default)]
    pub afk_timeout: Option<u32>,
    #[serde(default)]
    pub system_channel_id: Option<ChannelId>,
    #[serde(default)]
    pub rules_channel_id: Option<ChannelId>,
    #[serde(default)]
    pub premium_tier: Option<u8>,
    #[serde(default)]
    pub premium_subscription_count: Option<u32>,
    #[serde(default)]
    pub premium_progress_bar_enabled: Option<bool>,
    #[serde(default)]
    pub vanity_url_code: Option<String>,
    #[serde(default)]
    pub max_members: Option<u32>,
    #[serde(default)]
    pub default_message_notifications: Option<u8>,
    #[serde(default)]
    pub home_header: Option<String>,
}

#[derive(Deserialize)]
struct RawGuild {
    id: GuildId,
    #[serde(default)]
    properties: Option<GuildProperties>,
    #[serde(flatten)]
    top: GuildProperties,
    #[serde(default, deserialize_with = "null_default")]
    channels: Vec<Channel>,
    #[serde(default, deserialize_with = "null_default")]
    threads: Vec<Channel>,
    #[serde(default, deserialize_with = "null_default")]
    roles: Vec<Role>,
    #[serde(default, deserialize_with = "null_default")]
    emojis: Vec<Emoji>,
    #[serde(default, deserialize_with = "null_default")]
    stickers: Vec<serde_json::Value>,
    #[serde(default, deserialize_with = "null_default")]
    members: Vec<Member>,
    #[serde(default, deserialize_with = "null_default")]
    voice_states: Vec<VoiceState>,
    #[serde(default)]
    member_count: Option<u32>,
    #[serde(default)]
    large: Option<bool>,
    #[serde(default)]
    lazy: Option<bool>,
    #[serde(default)]
    unavailable: Option<bool>,
    #[serde(default)]
    joined_at: Option<String>,
    #[serde(default)]
    data_mode: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    stage_instances: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(from = "RawGuild")]
pub struct Guild {
    pub id: GuildId,
    pub properties: GuildProperties,
    pub channels: Vec<Channel>,
    pub threads: Vec<Channel>,
    pub roles: Vec<Role>,
    pub emojis: Vec<Emoji>,
    pub stickers: Vec<serde_json::Value>,
    pub members: Vec<Member>,
    pub voice_states: Vec<VoiceState>,
    pub member_count: Option<u32>,
    pub large: bool,
    pub lazy: bool,
    pub unavailable: bool,
    pub joined_at: Option<String>,
    pub data_mode: Option<String>,
    pub stage_instances: Vec<serde_json::Value>,
}

impl From<RawGuild> for Guild {
    fn from(r: RawGuild) -> Self {
        let boosts = r.top.premium_subscription_count;
        let mut properties = r.properties.unwrap_or(r.top);
        properties.premium_subscription_count = properties.premium_subscription_count.or(boosts);
        let mut g = Guild {
            id: r.id,
            properties,
            channels: r.channels,
            threads: r.threads,
            roles: r.roles,
            emojis: r.emojis,
            stickers: r.stickers,
            members: r.members,
            voice_states: r.voice_states,
            member_count: r.member_count,
            large: r.large.unwrap_or(false),
            lazy: r.lazy.unwrap_or(false),
            unavailable: r.unavailable.unwrap_or(false),
            joined_at: r.joined_at,
            data_mode: r.data_mode,
            stage_instances: r.stage_instances,
        };
        for c in g.channels.iter_mut().chain(g.threads.iter_mut()) {
            c.guild_id.get_or_insert(g.id);
        }
        g
    }
}

impl Guild {
    pub fn name(&self) -> &str {
        &self.properties.name
    }

    pub fn owner_id(&self) -> Option<UserId> {
        self.properties.owner_id
    }

    pub fn icon_url(&self, size: u32) -> Option<String> {
        self.properties.icon.as_ref().map(|h| {
            let ext = if h.starts_with("a_") { "gif" } else { "png" };
            format!("{CDN_BASE}/icons/{}/{h}.{ext}?size={size}", self.id)
        })
    }

    pub fn everyone_role(&self) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PartialGuild {
    pub id: GuildId,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub owner: bool,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub approximate_member_count: Option<u32>,
    #[serde(default)]
    pub approximate_presence_count: Option<u32>,
}
