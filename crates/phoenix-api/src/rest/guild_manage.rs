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
pub struct GuildEdit {
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_channel: Option<Option<Snowflake>>,
    pub afk_channel: Option<Option<Snowflake>>,
    pub afk_timeout: Option<u32>,
    pub default_notifications: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoleEdit {
    pub name: Option<String>,
    pub color: Option<u32>,
    pub hoist: Option<bool>,
    pub mentionable: Option<bool>,
    pub permissions: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdminCall {
    CreateChannel { guild: Snowflake, name: String, kind: NewChannelKind, parent: Option<Snowflake>, private: bool, me: Snowflake },
    EditChannel { channel: Snowflake, edit: ChannelEdit },
    DeleteChannel { channel: Snowflake },
    Channel { channel: Snowflake },
    SetOverwrite { channel: Snowflake, target: Snowflake, kind: u8, allow: u64, deny: u64 },
    DeleteOverwrite { channel: Snowflake, target: Snowflake },
    ChannelInvites { channel: Snowflake },
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
    IncidentActions { guild: Snowflake, invites_until: Option<u64>, dms_until: Option<u64> },
    UserSettings,
    PatchUserSettings { patch: Value },
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
        C::CreateChannel { guild, name, kind, parent, private, me } => {
            let mut b = json!({ "name": name.trim(), "type": *kind as u8 });
            if let (Some(p), false) = (parent, *kind == NewChannelKind::Category) {
                b["parent_id"] = json!(p);
            }
            if *private {
                b["permission_overwrites"] = private_overwrites(*guild, *me, matches!(kind, NewChannelKind::Voice | NewChannelKind::Stage));
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
        C::Channel { channel } => req(Method::GET, format!("/channels/{channel}"), None),
        C::SetOverwrite { channel, target, kind, allow, deny } => req(
            Method::PUT,
            format!("/channels/{channel}/permissions/{target}"),
            Some(json!({ "type": kind, "allow": allow.to_string(), "deny": deny.to_string() })),
        ),
        C::DeleteOverwrite { channel, target } => req(Method::DELETE, format!("/channels/{channel}/permissions/{target}"), None),
        C::ChannelInvites { channel } => req(Method::GET, format!("/channels/{channel}/invites"), None),
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
        C::IncidentActions { guild, invites_until, dms_until } => req(
            Method::PUT,
            format!("/guilds/{guild}/incident-actions"),
            Some(json!({
                "invites_disabled_until": invites_until.map(rfc3339),
                "dms_disabled_until": dms_until.map(rfc3339),
            })),
        ),
        C::UserSettings => req(Method::GET, "/users/@me/settings".into(), None),
        C::PatchUserSettings { patch } => req(Method::PATCH, "/users/@me/settings".into(), Some(patch.clone())),
    }
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
    fn create_private_voice() {
        let r = build(&AdminCall::CreateChannel { guild: G, name: " vc ".into(), kind: NewChannelKind::Voice, parent: Some(Snowflake(9)), private: true, me: Snowflake(5) });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/guilds/1/channels"));
        let b = r.body.unwrap();
        assert_eq!(b["name"], "vc");
        assert_eq!(b["type"], 2);
        assert_eq!(b["parent_id"], "9");
        assert_eq!(b["permission_overwrites"][0]["deny"], (VIEW_CHANNEL | CONNECT).to_string());
        assert_eq!(b["permission_overwrites"][1]["id"], "5");
    }

    #[test]
    fn create_category_has_no_parent() {
        let b = build(&AdminCall::CreateChannel { guild: G, name: "c".into(), kind: NewChannelKind::Category, parent: Some(C), private: false, me: G }).body.unwrap();
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
        let b = build(&AdminCall::IncidentActions { guild: G, invites_until: Some(3600), dms_until: None }).body.unwrap();
        assert_eq!(b, json!({"invites_disabled_until": "1970-01-01T01:00:00Z", "dms_disabled_until": null}));
        let cur = json!(["1", "8"]);
        assert_eq!(toggle_list(&cur, G, false), json!(["8"]));
        assert_eq!(toggle_list(&json!(null), G, true), json!(["1"]));
        assert_eq!(toggle_list(&cur, G, true), json!(["8", "1"]));
    }
}
