use reqwest::Method;
use serde_json::{Value, json};

use super::{RestClient, moderation::rfc3339};
use crate::error::Result;
use crate::models::Snowflake;

pub const VIEW_CHANNEL: u64 = 1 << 10;
pub const CONNECT: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewChannelKind {
    Text = 0,
    Voice = 2,
    Category = 4,
    Announcement = 5,
    Stage = 13,
    Forum = 15,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChannelEdit {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub slowmode: Option<u32>,
    pub nsfw: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThreadEdit {
    pub name: Option<String>,
    pub archived: Option<bool>,
    pub locked: Option<bool>,
    pub invitable: Option<bool>,
    pub auto_archive_duration: Option<u32>,
    pub slowmode: Option<u32>,
    pub flags: Option<u64>,
    pub applied_tags: Option<Vec<Snowflake>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThreadMemberPatch {
    Flags(u8),
    Muted(bool),
    MuteFor { window: i64, end_time: Option<String> },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuildEdit {
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_channel: Option<Option<Snowflake>>,
    pub afk_channel: Option<Option<Snowflake>>,
    pub afk_timeout: Option<u32>,
    pub default_notifications: Option<u8>,
    pub system_channel_flags: Option<u64>,
    pub icon: Option<Option<String>>,
    pub banner: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoleEdit {
    pub name: Option<String>,
    pub color: Option<u32>,
    pub hoist: Option<bool>,
    pub mentionable: Option<bool>,
    pub permissions: Option<u64>,
    pub colors: Option<RoleColors>,
    pub icon: Option<Option<String>>,
    pub unicode_emoji: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoleColors {
    pub primary: u32,
    pub secondary: Option<u32>,
    pub tertiary: Option<u32>,
}

pub const HOLOGRAPHIC: RoleColors = RoleColors { primary: 0xA9_C9FF, secondary: Some(0xFF_BBEC), tertiary: Some(0xFF_C3A0) };

#[derive(Debug, Clone, PartialEq)]
pub enum AdminCall {
    CreateChannel { guild: Snowflake, name: String, kind: NewChannelKind, parent: Option<Snowflake>, private: bool, me: Snowflake, members: Vec<(Snowflake, u8)> },
    EditChannel { channel: Snowflake, edit: ChannelEdit },
    DeleteChannel { channel: Snowflake },
    EditThread { thread: Snowflake, edit: ThreadEdit },
    JoinThread { thread: Snowflake, location: String },
    LeaveThread { thread: Snowflake, location: String },
    AddThreadMember { thread: Snowflake, user: Snowflake, location: String },
    ThreadMemberSettings { thread: Snowflake, patch: ThreadMemberPatch },
    Channel { channel: Snowflake },
    SetOverwrite { channel: Snowflake, target: Snowflake, kind: u8, allow: u64, deny: u64 },
    DeleteOverwrite { channel: Snowflake, target: Snowflake },
    ChannelInvites { channel: Snowflake },
    PatchChannel { channel: Snowflake, body: Value },
    ChannelWebhooks { channel: Snowflake },
    CreateWebhook { channel: Snowflake, name: String },
    EditWebhook { webhook: Snowflake, body: Value },
    DeleteWebhook { webhook: Snowflake },
    FollowChannel { channel: Snowflake, webhook_channel: Snowflake },
    GuildInvites { guild: Snowflake },
    DeleteInvite { code: String },
    Integrations { guild: Snowflake },
    EditGuild { guild: Snowflake, edit: GuildEdit },
    Roles { guild: Snowflake },
    CreateRole { guild: Snowflake, edit: RoleEdit },
    EditRole { guild: Snowflake, role: Snowflake, edit: RoleEdit },
    DeleteRole { guild: Snowflake, role: Snowflake },
    Emojis { guild: Snowflake },
    RenameEmoji { guild: Snowflake, emoji: Snowflake, name: String },
    DeleteEmoji { guild: Snowflake, emoji: Snowflake },
    Stickers { guild: Snowflake },
    DeleteSticker { guild: Snowflake, sticker: Snowflake },
    SearchMembers { guild: Snowflake, query: String, limit: u32 },
    Bans { guild: Snowflake },
    Unban { guild: Snowflake, user: Snowflake },
    AuditLog { guild: Snowflake, before: Option<Snowflake>, limit: u32 },
    IncidentActions { guild: Snowflake, invites_until: Option<u64>, dms_until: Option<u64>, lockdown_hours: Option<u32> },
    UserSettings,
    PatchUserSettings { patch: Value },
    EditGroupDm { channel: Snowflake, name: Option<String>, icon: Option<Option<String>> },
    BulkLeaveGroupDms { channels: Vec<Snowflake> },
    Server(super::guild_server::ServerCall),
    ScheduleMessage { channel: Snowflake, at: String, message: Value },
    ScheduledMessages,
    EditScheduled { id: Snowflake, at: Option<String>, content: Option<String>, flags: Option<u64> },
    DeleteScheduled { id: Snowflake },
    SendScheduledNow { id: Snowflake },
    Guild { guild: Snowflake },
    GuildProfile { guild: Snowflake },
    Applications { ids: Vec<Snowflake> },
    RoleMemberCounts { guild: Snowflake },
    RoleMemberIds { guild: Snowflake, role: Snowflake },
    ReorderRoles { guild: Snowflake, order: Vec<(Snowflake, i32)> },
    AddRoleMember { guild: Snowflake, role: Snowflake, user: Snowflake },
    RemoveRoleMember { guild: Snowflake, role: Snowflake, user: Snowflake },
    Soundboard { guild: Snowflake },
    Widget { guild: Snowflake },
    EditWidget { guild: Snowflake, enabled: bool, channel: Option<Snowflake> },
    CreateEmoji { guild: Snowflake, name: String, image: String },
    CreateSound { guild: Snowflake, name: String, sound: String, volume: f32, emoji_id: Option<Snowflake>, emoji_name: Option<String> },
    DeleteSound { guild: Snowflake, sound: Snowflake },
    Kick { guild: Snowflake, user: Snowflake },
    Ban { guild: Snowflake, user: Snowflake, delete_message_seconds: u32 },
    AuditLogFiltered { guild: Snowflake, user: Option<Snowflake>, action: Option<u32>, before: Option<Snowflake>, limit: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiReq {
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
}

fn req(method: Method, path: String, body: Option<Value>) -> ApiReq {
    ApiReq { method, path, body }
}

pub fn private_overwrites(guild: Snowflake, me: Snowflake, voice: bool) -> Value {
    let bits = if voice { VIEW_CHANNEL | CONNECT } else { VIEW_CHANNEL };
    json!([
        { "id": guild, "type": 0, "allow": "0", "deny": bits.to_string() },
        { "id": me, "type": 1, "allow": bits.to_string(), "deny": "0" },
    ])
}

pub fn toggle_list(current: &Value, guild: Snowflake, present: bool) -> Value {
    let g = guild.to_string();
    let mut v: Vec<String> = current.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).filter(|x| *x != g).collect();
    if present {
        v.push(g);
    }
    json!(v)
}

pub fn build(call: &AdminCall) -> ApiReq {
    use AdminCall as C;
    match call {
        C::CreateChannel { guild, name, kind, parent, private, me, members } => {
            let mut b = json!({ "name": name.trim(), "type": *kind as u8 });
            if let (Some(p), false) = (parent, *kind == NewChannelKind::Category) {
                b["parent_id"] = json!(p);
            }
            if *private {
                let voice = matches!(kind, NewChannelKind::Voice | NewChannelKind::Stage);
                let mut ow = private_overwrites(*guild, *me, voice);
                let bits = if voice { VIEW_CHANNEL | CONNECT } else { VIEW_CHANNEL };
                if let Some(list) = ow.as_array_mut() {
                    for (id, t) in members.iter().filter(|(id, _)| id != me) {
                        list.push(json!({ "id": id, "type": t, "allow": bits.to_string(), "deny": "0" }));
                    }
                }
                b["permission_overwrites"] = ow;
            }
            req(Method::POST, format!("/guilds/{guild}/channels"), Some(b))
        }
        C::EditChannel { channel, edit } => {
            let mut b = json!({});
            if let Some(n) = &edit.name {
                b["name"] = json!(n.trim());
            }
            if let Some(t) = &edit.topic {
                b["topic"] = json!(t);
            }
            if let Some(s) = edit.slowmode {
                b["rate_limit_per_user"] = json!(s.min(21_600));
            }
            if let Some(n) = edit.nsfw {
                b["nsfw"] = json!(n);
            }
            req(Method::PATCH, format!("/channels/{channel}"), Some(b))
        }
        C::DeleteChannel { channel } => req(Method::DELETE, format!("/channels/{channel}"), None),
        C::EditThread { thread, edit } => req(Method::PATCH, format!("/channels/{thread}"), Some(thread_body(edit))),
        C::JoinThread { thread, location } => req(Method::POST, format!("/channels/{thread}/thread-members/@me?location={}", query_value(location)), None),
        C::LeaveThread { thread, location } => req(Method::DELETE, format!("/channels/{thread}/thread-members/@me?location={}", query_value(location)), None),
        C::AddThreadMember { thread, user, location } => {
            req(Method::POST, format!("/channels/{thread}/thread-members/{user}?location={}", query_value(location)), None)
        }
        C::ThreadMemberSettings { thread, patch } => req(
            Method::PATCH,
            format!("/channels/{thread}/thread-members/@me/settings"),
            Some(match patch {
                ThreadMemberPatch::Flags(f) => json!({ "flags": f }),
                ThreadMemberPatch::Muted(m) => json!({ "muted": m }),
                ThreadMemberPatch::MuteFor { window, end_time } => {
                    json!({ "muted": true, "mute_config": { "selected_time_window": window, "end_time": end_time } })
                }
            }),
        ),
        C::Channel { channel } => req(Method::GET, format!("/channels/{channel}"), None),
        C::SetOverwrite { channel, target, kind, allow, deny } => req(
            Method::PUT,
            format!("/channels/{channel}/permissions/{target}"),
            Some(json!({ "type": kind, "allow": allow.to_string(), "deny": deny.to_string() })),
        ),
        C::DeleteOverwrite { channel, target } => req(Method::DELETE, format!("/channels/{channel}/permissions/{target}"), None),
        C::ChannelInvites { channel } => req(Method::GET, format!("/channels/{channel}/invites"), None),
        C::ScheduleMessage { channel, at, message } => {
            let mut b = json!({
                "channel_id": channel,
                "content": message.get("content").cloned().unwrap_or_else(|| json!("")),
                "scheduled_timestamp": at,
                "flags": message.get("flags").cloned().unwrap_or_else(|| json!(0)),
                "attachments": [],
            });
            for k in ["message_reference", "allowed_mentions", "sticker_ids", "poll"] {
                if let Some(v) = message.get(k) {
                    b[k] = v.clone();
                }
            }
            req(Method::POST, "/users/@me/scheduled-messages".into(), Some(b))
        }
        C::ScheduledMessages => req(Method::GET, "/users/@me/scheduled-messages".into(), None),
        C::EditScheduled { id, at, content, flags } => {
            let mut b = json!({});
            if let Some(a) = at {
                b["scheduled_timestamp"] = json!(a);
            }
            if let Some(c) = content {
                b["content"] = json!(c);
            }
            if let Some(f) = flags {
                b["flags"] = json!(f);
            }
            req(Method::PATCH, format!("/users/@me/scheduled-messages/{id}"), Some(b))
        }
        C::DeleteScheduled { id } => req(Method::DELETE, format!("/users/@me/scheduled-messages/{id}"), None),
        C::SendScheduledNow { id } => req(Method::POST, format!("/users/@me/scheduled-messages/{id}/send"), None),
        C::EditGroupDm { channel, name, icon } => {
            let mut b = json!({});
            if let Some(n) = name {
                b["name"] = json!(n);
            }
            if let Some(i) = icon {
                b["icon"] = json!(i);
            }
            req(Method::PATCH, format!("/channels/{channel}"), Some(b))
        }
        C::BulkLeaveGroupDms { channels } => req(Method::POST, "/users/@me/channels/bulk-leave".into(), Some(json!({ "channel_ids": channels }))),
        C::PatchChannel { channel, body } => req(Method::PATCH, format!("/channels/{channel}"), Some(body.clone())),
        C::ChannelWebhooks { channel } => req(Method::GET, format!("/channels/{channel}/webhooks"), None),
        C::CreateWebhook { channel, name } => req(Method::POST, format!("/channels/{channel}/webhooks"), Some(json!({ "name": name }))),
        C::EditWebhook { webhook, body } => req(Method::PATCH, format!("/webhooks/{webhook}"), Some(body.clone())),
        C::DeleteWebhook { webhook } => req(Method::DELETE, format!("/webhooks/{webhook}"), None),
        C::FollowChannel { channel, webhook_channel } => req(Method::POST, format!("/channels/{channel}/followers"), Some(json!({ "webhook_channel_id": webhook_channel }))),
        C::GuildInvites { guild } => req(Method::GET, format!("/guilds/{guild}/invites"), None),
        C::DeleteInvite { code } => req(Method::DELETE, format!("/invites/{code}"), None),
        C::Integrations { guild } => req(Method::GET, format!("/guilds/{guild}/integrations"), None),
        C::EditGuild { guild, edit } => {
            let mut b = json!({});
            if let Some(n) = &edit.name {
                b["name"] = json!(n.trim());
            }
            if let Some(d) = &edit.description {
                b["description"] = json!(d);
            }
            if let Some(s) = edit.system_channel {
                b["system_channel_id"] = json!(s);
            }
            if let Some(s) = edit.afk_channel {
                b["afk_channel_id"] = json!(s);
            }
            if let Some(t) = edit.afk_timeout {
                b["afk_timeout"] = json!(t);
            }
            if let Some(n) = edit.default_notifications {
                b["default_message_notifications"] = json!(n);
            }
            if let Some(f) = edit.system_channel_flags {
                b["system_channel_flags"] = json!(f);
            }
            if let Some(i) = &edit.icon {
                b["icon"] = json!(i);
            }
            if let Some(i) = &edit.banner {
                b["banner"] = json!(i);
            }
            req(Method::PATCH, format!("/guilds/{guild}"), Some(b))
        }
        C::Roles { guild } => req(Method::GET, format!("/guilds/{guild}/roles"), None),
        C::CreateRole { guild, edit } => req(Method::POST, format!("/guilds/{guild}/roles"), Some(role_body(edit))),
        C::EditRole { guild, role, edit } => req(Method::PATCH, format!("/guilds/{guild}/roles/{role}"), Some(role_body(edit))),
        C::DeleteRole { guild, role } => req(Method::DELETE, format!("/guilds/{guild}/roles/{role}"), None),
        C::Emojis { guild } => req(Method::GET, format!("/guilds/{guild}/emojis"), None),
        C::RenameEmoji { guild, emoji, name } => req(Method::PATCH, format!("/guilds/{guild}/emojis/{emoji}"), Some(json!({ "name": name.trim() }))),
        C::DeleteEmoji { guild, emoji } => req(Method::DELETE, format!("/guilds/{guild}/emojis/{emoji}"), None),
        C::Stickers { guild } => req(Method::GET, format!("/guilds/{guild}/stickers"), None),
        C::DeleteSticker { guild, sticker } => req(Method::DELETE, format!("/guilds/{guild}/stickers/{sticker}"), None),
        C::SearchMembers { guild, query, limit } => {
            let mut b = json!({ "limit": (*limit).clamp(1, 1000), "sort": 1, "or_query": {}, "and_query": {} });
            if !query.trim().is_empty() {
                b["and_query"] = json!({ "usernames": { "or_query": [query.trim()] } });
            }
            req(Method::POST, format!("/guilds/{guild}/members-search"), Some(b))
        }
        C::Bans { guild } => req(Method::GET, format!("/guilds/{guild}/bans?limit=1000"), None),
        C::Unban { guild, user } => req(Method::DELETE, format!("/guilds/{guild}/bans/{user}"), None),
        C::AuditLog { guild, before, limit } => {
            let mut p = format!("/guilds/{guild}/audit-logs?limit={}", (*limit).clamp(1, 100));
            if let Some(b) = before {
                p.push_str(&format!("&before={b}"));
            }
            req(Method::GET, p, None)
        }
        C::IncidentActions { guild, invites_until, dms_until, lockdown_hours } => req(
            Method::PUT,
            format!("/guilds/{guild}/incident-actions"),
            Some(json!({
                "invites_disabled_until": invites_until.map(rfc3339),
                "dms_disabled_until": dms_until.map(rfc3339),
                "lockdown_duration_hours": lockdown_hours,
            })),
        ),
        C::UserSettings => req(Method::GET, "/users/@me/settings".into(), None),
        C::PatchUserSettings { patch } => req(Method::PATCH, "/users/@me/settings".into(), Some(patch.clone())),
        C::Server(s) => super::guild_server::build(s),
        C::Guild { guild } => req(Method::GET, format!("/guilds/{guild}?with_counts=true"), None),
        C::GuildProfile { guild } => req(Method::GET, format!("/guilds/{guild}/profile"), None),
        C::Applications { ids } => {
            let q: Vec<String> = ids.iter().map(|i| format!("application_ids={i}")).collect();
            req(Method::GET, format!("/applications/public?{}", q.join("&")), None)
        }
        C::RoleMemberCounts { guild } => req(Method::GET, format!("/guilds/{guild}/roles/member-counts"), None),
        C::RoleMemberIds { guild, role } => req(Method::GET, format!("/guilds/{guild}/roles/{role}/member-ids"), None),
        C::ReorderRoles { guild, order } => {
            req(Method::PATCH, format!("/guilds/{guild}/roles"), Some(Value::Array(order.iter().map(|(id, p)| json!({ "id": id, "position": p })).collect())))
        }
        C::AddRoleMember { guild, role, user } => req(Method::PUT, format!("/guilds/{guild}/members/{user}/roles/{role}"), None),
        C::RemoveRoleMember { guild, role, user } => req(Method::DELETE, format!("/guilds/{guild}/members/{user}/roles/{role}"), None),
        C::Soundboard { guild } => req(Method::GET, format!("/guilds/{guild}/soundboard-sounds"), None),
        C::Widget { guild } => req(Method::GET, format!("/guilds/{guild}/widget"), None),
        C::EditWidget { guild, enabled, channel } => req(Method::PATCH, format!("/guilds/{guild}/widget"), Some(json!({ "enabled": enabled, "channel_id": channel }))),
        C::CreateEmoji { guild, name, image } => req(Method::POST, format!("/guilds/{guild}/emojis"), Some(json!({ "name": name.trim(), "image": image }))),
        C::CreateSound { guild, name, sound, volume, emoji_id, emoji_name } => req(
            Method::POST,
            format!("/guilds/{guild}/soundboard-sounds"),
            Some(json!({ "name": name.trim(), "sound": sound, "volume": volume.clamp(0., 1.), "emoji_id": emoji_id, "emoji_name": emoji_name })),
        ),
        C::DeleteSound { guild, sound } => req(Method::DELETE, format!("/guilds/{guild}/soundboard-sounds/{sound}"), None),
        C::Kick { guild, user } => req(Method::DELETE, format!("/guilds/{guild}/members/{user}"), None),
        C::Ban { guild, user, delete_message_seconds } => {
            req(Method::PUT, format!("/guilds/{guild}/bans/{user}"), Some(json!({ "delete_message_seconds": (*delete_message_seconds).min(604_800) })))
        }
        C::AuditLogFiltered { guild, user, action, before, limit } => {
            let mut p = format!("/guilds/{guild}/audit-logs?limit={}", (*limit).clamp(1, 100));
            if let Some(b) = before {
                p.push_str(&format!("&before={b}"));
            }
            if let Some(u) = user {
                p.push_str(&format!("&user_id={u}"));
            }
            if let Some(a) = action {
                p.push_str(&format!("&action_type={a}"));
            }
            req(Method::GET, p, None)
        }
    }
}

fn query_value(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            c => {
                let mut b = [0u8; 4];
                c.encode_utf8(&mut b).bytes().map(|x| format!("%{x:02X}")).collect()
            }
        })
        .collect()
}

pub fn thread_body(e: &ThreadEdit) -> Value {
    let mut b = json!({});
    if let Some(n) = &e.name {
        b["name"] = json!(n.trim().chars().take(100).collect::<String>());
    }
    if let Some(v) = e.archived {
        b["archived"] = json!(v);
    }
    if let Some(v) = e.locked {
        b["locked"] = json!(v);
    }
    if let Some(v) = e.invitable {
        b["invitable"] = json!(v);
    }
    if let Some(v) = e.auto_archive_duration {
        b["auto_archive_duration"] = json!(v);
    }
    if let Some(v) = e.slowmode {
        b["rate_limit_per_user"] = json!(v.min(21_600));
    }
    if let Some(v) = e.flags {
        b["flags"] = json!(v);
    }
    if let Some(t) = &e.applied_tags {
        b["applied_tags"] = json!(t);
    }
    b
}

fn role_body(e: &RoleEdit) -> Value {
    let mut b = json!({});
    if let Some(n) = &e.name {
        b["name"] = json!(n.trim());
    }
    if let Some(c) = e.color {
        b["color"] = json!(c & 0xFF_FFFF);
    }
    if let Some(h) = e.hoist {
        b["hoist"] = json!(h);
    }
    if let Some(m) = e.mentionable {
        b["mentionable"] = json!(m);
    }
    if let Some(p) = e.permissions {
        b["permissions"] = json!(p.to_string());
    }
    if let Some(c) = e.colors {
        b["colors"] = json!({ "primary_color": c.primary & 0xFF_FFFF, "secondary_color": c.secondary.map(|x| x & 0xFF_FFFF), "tertiary_color": c.tertiary.map(|x| x & 0xFF_FFFF) });
    }
    if let Some(i) = &e.icon {
        b["icon"] = json!(i);
    }
    if let Some(u) = &e.unicode_emoji {
        b["unicode_emoji"] = json!(u);
    }
    b
}

impl RestClient {
    pub async fn admin(&self, call: &AdminCall) -> Result<Value> {
        let r = build(call);
        self.request_json(r.method, &r.path, r.body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Snowflake = Snowflake(1);
    const C: Snowflake = Snowflake(2);

    #[test]
    fn scheduled_message_calls_match_the_client() {
        let r = build(&AdminCall::ScheduleMessage { channel: C, at: "2026-11-25T10:00:00.000Z".into(), message: json!({"content": "hi"}) });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/users/@me/scheduled-messages"));
        assert_eq!(r.body.unwrap(), json!({"channel_id": "2", "content": "hi", "scheduled_timestamp": "2026-11-25T10:00:00.000Z", "flags": 0, "attachments": []}));
        let r = build(&AdminCall::ScheduledMessages);
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::GET, "/users/@me/scheduled-messages", None));
        let r = build(&AdminCall::EditScheduled { id: Snowflake(9), at: Some("x".into()), content: None, flags: None });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/users/@me/scheduled-messages/9"));
        assert_eq!(r.body.unwrap(), json!({"scheduled_timestamp": "x"}));
        let r = build(&AdminCall::DeleteScheduled { id: Snowflake(9) });
        assert_eq!((r.method, r.path.as_str()), (Method::DELETE, "/users/@me/scheduled-messages/9"));
        let r = build(&AdminCall::SendScheduledNow { id: Snowflake(9) });
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::POST, "/users/@me/scheduled-messages/9/send", None));
    }

    #[test]
    fn bulk_leave_matches_the_client() {
        let r = build(&AdminCall::BulkLeaveGroupDms { channels: vec![Snowflake(2), Snowflake(3)] });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/users/@me/channels/bulk-leave"));
        assert_eq!(r.body.unwrap(), json!({"channel_ids": ["2", "3"]}));
    }

    #[test]
    fn group_dm_edit_sends_only_what_changed() {
        let r = build(&AdminCall::EditGroupDm { channel: C, name: Some("Squad".into()), icon: None });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/channels/2"));
        assert_eq!(r.body.unwrap(), json!({"name": "Squad"}));
        let r = build(&AdminCall::EditGroupDm { channel: C, name: None, icon: Some(None) });
        assert_eq!(r.body.unwrap(), json!({"icon": null}));
        let r = build(&AdminCall::EditGroupDm { channel: C, name: Some(String::new()), icon: Some(Some("data:image/png;base64,AA==".into())) });
        assert_eq!(r.body.unwrap(), json!({"name": "", "icon": "data:image/png;base64,AA=="}));
        let r = build(&AdminCall::DeleteInvite { code: "abc".into() });
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::DELETE, "/invites/abc", None));
    }

    #[test]
    fn create_private_voice() {
        let r = build(&AdminCall::CreateChannel { guild: G, name: " vc ".into(), kind: NewChannelKind::Voice, parent: Some(Snowflake(9)), private: true, me: Snowflake(5), members: vec![(Snowflake(7), 0)] });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/guilds/1/channels"));
        let b = r.body.unwrap();
        assert_eq!(b["name"], "vc");
        assert_eq!(b["type"], 2);
        assert_eq!(b["parent_id"], "9");
        assert_eq!(b["permission_overwrites"][0]["deny"], (VIEW_CHANNEL | CONNECT).to_string());
        assert_eq!(b["permission_overwrites"][1]["id"], "5");
        assert_eq!(b["permission_overwrites"][2], json!({ "id": "7", "type": 0, "allow": (VIEW_CHANNEL | CONNECT).to_string(), "deny": "0" }));
    }

    #[test]
    fn thread_calls_match_the_client() {
        let t = Snowflake(7);
        let r = build(&AdminCall::EditThread { thread: t, edit: ThreadEdit { archived: Some(true), ..Default::default() } });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/channels/7"));
        assert_eq!(r.body.unwrap(), json!({"archived": true}));
        let r = build(&AdminCall::EditThread { thread: t, edit: ThreadEdit { locked: Some(false), archived: Some(false), ..Default::default() } });
        assert_eq!(r.body.unwrap(), json!({"locked": false, "archived": false}));
        let r = build(&AdminCall::EditThread {
            thread: t,
            edit: ThreadEdit { name: Some(format!("  {}  ", "n".repeat(120))), auto_archive_duration: Some(10_080), slowmode: Some(50_000), ..Default::default() },
        });
        let b = r.body.unwrap();
        assert_eq!(b["name"].as_str().unwrap().len(), 100);
        assert_eq!(b["auto_archive_duration"], 10_080);
        assert_eq!(b["rate_limit_per_user"], 21_600);
        let r = build(&AdminCall::EditThread { thread: t, edit: ThreadEdit { flags: Some(2), applied_tags: Some(vec![Snowflake(3)]), ..Default::default() } });
        assert_eq!(r.body.unwrap(), json!({"flags": 2, "applied_tags": ["3"]}));
        let r = build(&AdminCall::JoinThread { thread: t, location: "Toolbar Overflow".into() });
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::POST, "/channels/7/thread-members/@me?location=Toolbar%20Overflow", None));
        let r = build(&AdminCall::LeaveThread { thread: t, location: "Context Menu".into() });
        assert_eq!((r.method, r.path.as_str()), (Method::DELETE, "/channels/7/thread-members/@me?location=Context%20Menu"));
        let r = build(&AdminCall::AddThreadMember { thread: t, user: Snowflake(9), location: "Members".into() });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/channels/7/thread-members/9?location=Members"));
        let r = build(&AdminCall::ThreadMemberSettings { thread: t, patch: ThreadMemberPatch::Flags(4) });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/channels/7/thread-members/@me/settings"));
        assert_eq!(r.body.unwrap(), json!({"flags": 4}));
        let r = build(&AdminCall::ThreadMemberSettings { thread: t, patch: ThreadMemberPatch::Muted(false) });
        assert_eq!(r.body.unwrap(), json!({"muted": false}));
        let end = Some("2026-11-25T12:15:00.000Z".to_string());
        let r = build(&AdminCall::ThreadMemberSettings { thread: t, patch: ThreadMemberPatch::MuteFor { window: 900, end_time: end } });
        assert_eq!(r.body.unwrap(), json!({"muted": true, "mute_config": {"selected_time_window": 900, "end_time": "2026-11-25T12:15:00.000Z"}}));
        let r = build(&AdminCall::ThreadMemberSettings { thread: t, patch: ThreadMemberPatch::MuteFor { window: -1, end_time: None } });
        assert_eq!(r.body.unwrap(), json!({"muted": true, "mute_config": {"selected_time_window": -1, "end_time": null}}));
    }

    #[test]
    fn create_category_has_no_parent() {
        let b = build(&AdminCall::CreateChannel { guild: G, name: "c".into(), kind: NewChannelKind::Category, parent: Some(C), private: false, me: G, members: Vec::new() }).body.unwrap();
        assert_eq!(b["type"], 4);
        assert!(b.get("parent_id").is_none() && b.get("permission_overwrites").is_none());
    }

    #[test]
    fn edit_and_delete_channel() {
        let r = build(&AdminCall::EditChannel { channel: C, edit: ChannelEdit { name: Some("x".into()), slowmode: Some(99_999), nsfw: Some(true), topic: None } });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/channels/2"));
        assert_eq!(r.body.unwrap(), json!({"name": "x", "rate_limit_per_user": 21_600, "nsfw": true}));
        let r = build(&AdminCall::DeleteChannel { channel: C });
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::DELETE, "/channels/2", None));
    }

    #[test]
    fn overwrites() {
        let r = build(&AdminCall::SetOverwrite { channel: C, target: Snowflake(3), kind: 0, allow: 1024, deny: 2048 });
        assert_eq!((r.method, r.path.as_str()), (Method::PUT, "/channels/2/permissions/3"));
        assert_eq!(r.body.unwrap(), json!({"type": 0, "allow": "1024", "deny": "2048"}));
        assert_eq!(build(&AdminCall::DeleteOverwrite { channel: C, target: Snowflake(3) }).method, Method::DELETE);
        let w = build(&AdminCall::CreateWebhook { channel: C, name: "Phoenix test".into() });
        assert_eq!((w.method, w.path.as_str(), w.body), (Method::POST, "/channels/2/webhooks", Some(json!({ "name": "Phoenix test" }))));
        assert_eq!(build(&AdminCall::DeleteWebhook { webhook: Snowflake(9) }).path, "/webhooks/9");
        let f = build(&AdminCall::FollowChannel { channel: C, webhook_channel: Snowflake(7) });
        assert_eq!((f.method, f.path.as_str(), f.body), (Method::POST, "/channels/2/followers", Some(json!({ "webhook_channel_id": "7" }))));
        let p = build(&AdminCall::PatchChannel { channel: C, body: json!({ "rate_limit_per_user": 5 }) });
        assert_eq!((p.method, p.body), (Method::PATCH, Some(json!({ "rate_limit_per_user": 5 }))));
    }

    #[test]
    fn guild_and_roles() {
        let edit = GuildEdit { name: Some("n".into()), system_channel: Some(None), default_notifications: Some(1), ..Default::default() };
        let b = build(&AdminCall::EditGuild { guild: G, edit }).body.unwrap();
        assert_eq!(b, json!({"name": "n", "system_channel_id": null, "default_message_notifications": 1}));
        let r = build(&AdminCall::EditRole { guild: G, role: Snowflake(4), edit: RoleEdit { color: Some(0x1FF_0000), permissions: Some(8), ..Default::default() } });
        assert_eq!(r.path, "/guilds/1/roles/4");
        assert_eq!(r.body.unwrap(), json!({"color": 0xFF_0000, "permissions": "8"}));
    }

    #[test]
    fn lists() {
        assert_eq!(build(&AdminCall::AuditLog { guild: G, before: Some(Snowflake(7)), limit: 500 }).path, "/guilds/1/audit-logs?limit=100&before=7");
        assert_eq!(build(&AdminCall::Bans { guild: G }).path, "/guilds/1/bans?limit=1000");
        assert_eq!(build(&AdminCall::DeleteInvite { code: "abc".into() }).path, "/invites/abc");
        let b = build(&AdminCall::SearchMembers { guild: G, query: "bob".into(), limit: 0 }).body.unwrap();
        assert_eq!(b["limit"], 1);
        assert_eq!(b["and_query"]["usernames"]["or_query"][0], "bob");
    }

    #[test]
    fn incidents_and_privacy() {
        let b = build(&AdminCall::IncidentActions { guild: G, invites_until: Some(3600), dms_until: None, lockdown_hours: Some(1) }).body.unwrap();
        assert_eq!(b, json!({"invites_disabled_until": "1970-01-01T01:00:00Z", "dms_disabled_until": null, "lockdown_duration_hours": 1}));
        let b = build(&AdminCall::IncidentActions { guild: G, invites_until: None, dms_until: None, lockdown_hours: None }).body.unwrap();
        assert_eq!(b, json!({"invites_disabled_until": null, "dms_disabled_until": null, "lockdown_duration_hours": null}));
        let cur = json!(["1", "8"]);
        assert_eq!(toggle_list(&cur, G, false), json!(["8"]));
        assert_eq!(toggle_list(&json!(null), G, true), json!(["1"]));
        assert_eq!(toggle_list(&cur, G, true), json!(["8", "1"]));
    }

    #[test]
    fn server_settings_reads_are_gets() {
        for c in [
            AdminCall::Guild { guild: G },
            AdminCall::GuildProfile { guild: G },
            AdminCall::Applications { ids: vec![Snowflake(3)] },
            AdminCall::RoleMemberCounts { guild: G },
            AdminCall::RoleMemberIds { guild: G, role: Snowflake(2) },
            AdminCall::Soundboard { guild: G },
            AdminCall::Widget { guild: G },
            AdminCall::AuditLogFiltered { guild: G, user: None, action: None, before: None, limit: 50 },
        ] {
            assert_eq!(build(&c).method, Method::GET);
        }
        assert_eq!(build(&AdminCall::Guild { guild: G }).path, "/guilds/1?with_counts=true");
        assert_eq!(build(&AdminCall::Applications { ids: vec![Snowflake(3), Snowflake(4)] }).path, "/applications/public?application_ids=3&application_ids=4");
        let r = build(&AdminCall::AuditLogFiltered { guild: G, user: Some(Snowflake(3)), action: Some(22), before: Some(Snowflake(9)), limit: 500 });
        assert_eq!(r.path, "/guilds/1/audit-logs?limit=100&before=9&user_id=3&action_type=22");
    }

    #[test]
    fn server_settings_writes() {
        let r = build(&AdminCall::ReorderRoles { guild: G, order: vec![(Snowflake(5), 3), (Snowflake(6), 2)] });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/guilds/1/roles"));
        assert_eq!(r.body.unwrap(), json!([{ "id": "5", "position": 3 }, { "id": "6", "position": 2 }]));
        let r = build(&AdminCall::EditWidget { guild: G, enabled: true, channel: Some(Snowflake(7)) });
        assert_eq!((r.method, r.body.unwrap()), (Method::PATCH, json!({ "enabled": true, "channel_id": "7" })));
        let r = build(&AdminCall::CreateEmoji { guild: G, name: " ok ".into(), image: "data:image/png;base64,AA".into() });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/guilds/1/emojis"));
        assert_eq!(r.body.unwrap()["name"], "ok");
        let r = build(&AdminCall::CreateSound { guild: G, name: "s".into(), sound: "data:audio/ogg;base64,AA".into(), volume: 2., emoji_id: None, emoji_name: Some("🔥".into()) });
        assert_eq!(r.body.unwrap()["volume"], 1.0);
        assert_eq!(build(&AdminCall::DeleteSound { guild: G, sound: Snowflake(4) }).path, "/guilds/1/soundboard-sounds/4");
        let r = build(&AdminCall::Ban { guild: G, user: Snowflake(2), delete_message_seconds: 9_999_999 });
        assert_eq!((r.method, r.body.unwrap()), (Method::PUT, json!({ "delete_message_seconds": 604_800 })));
        assert_eq!(build(&AdminCall::Kick { guild: G, user: Snowflake(2) }).method, Method::DELETE);
        let r = build(&AdminCall::AddRoleMember { guild: G, role: Snowflake(3), user: Snowflake(2) });
        assert_eq!((r.method, r.path.as_str()), (Method::PUT, "/guilds/1/members/2/roles/3"));
        let edit = RoleEdit { colors: Some(HOLOGRAPHIC), unicode_emoji: Some(Some("🔥".into())), icon: Some(None), ..Default::default() };
        let b = build(&AdminCall::EditRole { guild: G, role: Snowflake(4), edit }).body.unwrap();
        assert_eq!(b["colors"], json!({ "primary_color": 11_127_295, "secondary_color": 16_759_788, "tertiary_color": 16_761_760 }));
        assert_eq!(b["icon"], Value::Null);
        assert_eq!(b["unicode_emoji"], "🔥");
        let edit = GuildEdit { system_channel_flags: Some(5), banner: Some(None), ..Default::default() };
        let b = build(&AdminCall::EditGuild { guild: G, edit }).body.unwrap();
        assert_eq!(b, json!({ "system_channel_flags": 5, "banner": null }));
    }
}
