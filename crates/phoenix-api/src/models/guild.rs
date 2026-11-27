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
    #[serde(default)]
    pub profile: Option<GuildTagProfile>,
    #[serde(default)]
    pub incidents_data: Option<IncidentsData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GuildTagProfile {
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub badge: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct IncidentsData {
    #[serde(default)]
    pub invites_disabled_until: Option<String>,
    #[serde(default)]
    pub dms_disabled_until: Option<String>,
    #[serde(default)]
    pub raid_detected_at: Option<String>,
    #[serde(default)]
    pub dm_spam_detected_at: Option<String>,
    #[serde(default)]
    pub lockdown_duration_hours: Option<u32>,
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
    #[serde(default, deserialize_with = "null_default")]
    guild_scheduled_events: Vec<serde_json::Value>,
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
    pub guild_scheduled_events: Vec<serde_json::Value>,
}

impl From<RawGuild> for Guild {
    fn from(r: RawGuild) -> Self {
        let boosts = r.top.premium_subscription_count;
        let (profile, incidents, owner) = (r.top.profile.clone(), r.top.incidents_data.clone(), r.top.owner_id);
        let mut properties = r.properties.unwrap_or(r.top);
        properties.premium_subscription_count = properties.premium_subscription_count.or(boosts);
        properties.profile = properties.profile.or(profile);
        properties.incidents_data = properties.incidents_data.or(incidents);
        properties.owner_id = properties.owner_id.or(owner);
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
            guild_scheduled_events: r.guild_scheduled_events,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_guild_keeps_its_tag_profile_and_incidents() {
        let g: Guild = serde_json::from_value(serde_json::json!({
            "id": "5",
            "properties": {
                "name": "G",
                "profile": { "tag": "GLAD", "badge": "4f56" },
                "incidents_data": { "raid_detected_at": null, "invites_disabled_until": "2026-01-23T19:31:24.511000+00:00", "dms_disabled_until": null, "dm_spam_detected_at": null }
            }
        }))
        .unwrap();
        let p = g.properties.profile.unwrap();
        assert_eq!((p.tag.as_deref(), p.badge.as_deref()), (Some("GLAD"), Some("4f56")));
        let i = g.properties.incidents_data.unwrap();
        assert_eq!(i.invites_disabled_until.as_deref(), Some("2026-01-23T19:31:24.511000+00:00"));
        assert!(i.dms_disabled_until.is_none() && i.raid_detected_at.is_none());
    }

    #[test]
    fn top_level_profile_survives_a_properties_object() {
        let g: Guild = serde_json::from_value(serde_json::json!({
            "id": "5",
            "properties": { "name": "G", "features": ["GUILD_TAGS"] },
            "profile": { "tag": "GLAD", "badge": "4f56" },
            "incidents_data": { "dms_disabled_until": "2026-01-23T19:31:24.511000+00:00" }
        }))
        .unwrap();
        assert_eq!(g.properties.profile.and_then(|p| p.tag).as_deref(), Some("GLAD"));
        assert!(g.properties.incidents_data.is_some());
    }

    #[test]
    fn rest_guild_with_null_profile_has_no_tag() {
        let g: Guild = serde_json::from_value(serde_json::json!({ "id": "5", "name": "G", "profile": null })).unwrap();
        assert!(g.properties.profile.is_none() && g.properties.incidents_data.is_none());
    }

    #[test]
    fn guild_owner_is_preserved_when_properties_are_nested() {
        let guild: Guild = serde_json::from_value(serde_json::json!({
            "id": "1",
            "owner_id": "2",
            "properties": { "name": "Fixture" }
        })).unwrap();
        assert_eq!(guild.owner_id().map(|owner| owner.0), Some(2));
    }
}
