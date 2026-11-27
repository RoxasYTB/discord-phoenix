use std::rc::Rc;

use gpui::Context;
use phoenix_api::{models::Snowflake, rest::guild_manage::{AdminCall, NewChannelKind}};
use serde_json::{Value, json};

use super::{ActionReply, ActionRequest, open_modal};
use crate::{
    app::PhoenixApp,
    model::{Channel, ChannelKind, Id},
    ui::modals::{Form, Modal, channel_settings, create_channel, privacy},
};

pub fn sf(id: Id) -> Snowflake {
    Snowflake(id.0)
}

pub fn new_kind(k: ChannelKind) -> NewChannelKind {
    match k {
        ChannelKind::Text => NewChannelKind::Text,
        ChannelKind::Voice => NewChannelKind::Voice,
        ChannelKind::Announcement => NewChannelKind::Announcement,
        ChannelKind::Forum => NewChannelKind::Forum,
        ChannelKind::Stage => NewChannelKind::Stage,
        ChannelKind::Category => NewChannelKind::Category,
    }
}

pub fn channel_name(kind: ChannelKind, raw: &str) -> String {
    let t = raw.trim();
    match kind {
        ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum => {
            let mut out = String::new();
            for ch in t.chars().flat_map(char::to_lowercase) {
                if ch.is_whitespace() || ch == '-' {
                    if !out.ends_with('-') && !out.is_empty() {
                        out.push('-');
                    }
                } else {
                    out.push(ch);
                }
            }
            out.trim_end_matches('-').to_owned()
        }
        _ => t.to_owned(),
    }
}

fn id_of(v: &Value) -> Option<Id> {
    v.get("id")?.as_str()?.parse().ok().map(Id)
}

pub fn mock_reply(c: &AdminCall) -> Value {
    use AdminCall as C;
    let user = |id: u64, name: &str| json!({ "id": id.to_string(), "username": name, "global_name": name, "avatar": null });
    match c {
        C::CreateChannel { name, kind, .. } => json!({ "id": (crate::actions::now_unix() * 1000 + 7).to_string(), "name": name, "type": *kind as u8 }),
        C::Channel { channel } => json!({ "id": channel, "permission_overwrites": [
            { "id": "1", "type": 0, "allow": "0", "deny": "2048" }
        ]}),
        C::ChannelInvites { .. } | C::GuildInvites { .. } => json!([
            { "code": "phoenix", "uses": 3, "max_uses": 0, "max_age": 604800, "created_at": "2026-11-20T10:00:00Z",
              "inviter": user(2, "ferris"), "channel": { "id": "10", "name": "general" } }
        ]),
        C::Integrations { .. } => json!([{ "id": "5", "name": "Phoenix Bot", "type": "discord", "enabled": true, "account": { "name": "Phoenix Bot" } }]),
        C::ChannelWebhooks { channel } => json!([
            { "id": "30", "type": 1, "name": "Captain Hook", "channel_id": channel, "avatar": null, "token": "t", "user": user(2, "ferris") },
            { "id": "31", "type": 2, "name": "Rust News #releases", "channel_id": channel, "avatar": null,
              "source_guild": { "id": "40", "name": "Rust", "icon": null }, "source_channel": { "id": "41", "name": "releases" }, "user": user(2, "ferris") }
        ]),
        C::CreateWebhook { channel, name } => json!({ "id": "32", "type": 1, "name": name, "channel_id": channel, "avatar": null, "token": "t", "user": user(2, "ferris") }),
        C::Roles { guild } => json!([
            { "id": guild, "name": "@everyone", "permissions": "104324673", "position": 0, "color": 0, "hoist": false, "mentionable": false },
            { "id": "40", "name": "Crabs", "permissions": "8", "position": 2, "color": 15_158_332, "colors": { "primary_color": 15_158_332, "secondary_color": 3_447_003, "tertiary_color": null }, "hoist": true, "mentionable": false },
            { "id": "41", "name": "Ferrises", "permissions": "0", "position": 1, "color": 3_066_993, "hoist": false, "mentionable": true }
        ]),
        C::Emojis { .. } => json!([{ "id": "11", "name": "ferris", "animated": false, "user": user(2, "ferris") }]),
        C::Stickers { .. } => json!([{ "id": "12", "name": "wave", "format_type": 1, "tags": "wave", "user": user(2, "ferris") }]),
        C::SearchMembers { .. } => json!({ "members": [
            { "member": { "user": user(2, "ferris"), "roles": [], "joined_at": "2025-01-02T03:04:05Z", "nick": null }, "join_source_type": 1 },
            { "member": { "user": user(3, "crab"), "roles": [], "joined_at": "2026-06-01T12:00:00Z", "nick": "Crabby" }, "join_source_type": 1 }
        ], "total_result_count": 2 }),
        C::Bans { .. } => json!([{ "user": user(4, "spammer"), "reason": "Spam" }]),
        C::AuditLog { .. } => json!({ "audit_log_entries": [
            { "id": "20", "user_id": "2", "action_type": 10, "target_id": "10", "changes": [] },
            { "id": "21", "user_id": "2", "action_type": 22, "target_id": "4", "reason": "Spam" }
        ], "users": [user(2, "ferris"), user(4, "spammer")] }),
        C::UserSettings => json!({ "restricted_guilds": [], "message_request_restricted_guild_ids": [] }),
        C::Server(s) => crate::ui::server_pages::mock::reply(s),
        C::Guild { guild } => json!({ "id": guild, "name": "Phoenix", "icon": null, "description": "A demo server", "system_channel_id": "10", "system_channel_flags": 13,
            "afk_channel_id": null, "afk_timeout": 300, "default_message_notifications": 1, "premium_tier": 1, "features": ["COMMUNITY"] }),
        C::GuildProfile { guild } => json!({ "id": guild, "name": "Phoenix", "description": "A demo server", "member_count": 3, "online_count": 2, "tag": "PHX", "visibility": 1,
            "traits": [{ "emoji_name": "crab", "label": "Rust", "position": 0 }], "game_application_ids": [] }),
        C::RoleMemberCounts { .. } => json!({}),
        C::RoleMemberIds { .. } => json!(["2"]),
        C::Soundboard { .. } => json!({ "items": [{ "name": "ding", "sound_id": "30", "volume": 1.0, "emoji_name": "🔔", "user": user(2, "ferris") }] }),
        C::Widget { .. } => json!({ "enabled": false, "channel_id": null }),
        C::MemberVerification { .. } => json!({ "version": "2026-01-01T00:00:00.000000+00:00", "description": null, "form_fields": [
            { "field_type": "TERMS", "label": "Read and agree to the server rules", "values": ["Be kind and respectful.", "Keep it on topic: Rust, GPUI and native clients."], "required": true }
        ]}),
        C::DiscoveryRequirements { guild } => json!({ "guild_id": guild, "sufficient": true, "safe_environment": true, "healthy": true, "size": true, "protected": true }),
        C::Applications { .. } => json!([]),
        C::AuditLogFiltered { .. } => mock_reply(&C::AuditLog { guild: Snowflake(0), before: None, limit: 50 }),
        _ => Value::Null,
    }
}

impl PhoenixApp {
    pub fn admin(&mut self, call: AdminCall, cx: &mut Context<Self>, done: impl FnOnce(&mut Self, Result<Value, String>, &mut Context<Self>) + 'static) {
        self.action(ActionRequest::Admin(call), cx, move |a, r, cx| {
            done(
                a,
                match r {
                    Ok(ActionReply::Json(v)) => Ok(v),
                    Ok(_) => Ok(Value::Null),
                    Err(e) => Err(e.0),
                },
                cx,
            )
        });
    }

    pub fn admin_mfa(&mut self, call: AdminCall, code: String, cx: &mut Context<Self>, done: impl FnOnce(&mut Self, Result<Value, String>, &mut Context<Self>) + 'static) {
        self.action(ActionRequest::AdminMfa(call, code), cx, move |a, r, cx| {
            done(
                a,
                match r {
                    Ok(ActionReply::Json(v)) => Ok(v),
                    Ok(_) => Ok(Value::Null),
                    Err(e) => Err(e.0),
                },
                cx,
            )
        });
    }

    pub fn admin_toast(&mut self, call: AdminCall, ok: Option<gpui::SharedString>, cx: &mut Context<Self>) {
        self.admin(call, cx, move |a, r, cx| match r {
            Ok(_) => {
                if let Some(ok) = ok {
                    a.parity_toast(ok, cx)
                }
            }
            Err(e) => a.parity_toast(e, cx),
        });
    }

    fn put_channel_local(&mut self, ch: Channel) {
        let Some(g) = ch.guild else { return };
        let list = self.guild_channels.entry(g).or_default();
        if !list.contains(&ch.id) {
            list.push(ch.id);
        }
        self.channels.insert(ch.id, ch);
        self.rebuild_sidebar();
    }

    fn drop_channel_local(&mut self, channel: Id) {
        if let Some(ch) = self.channels.remove(&channel)
            && let Some(list) = ch.guild.and_then(|g| self.guild_channels.get_mut(&g))
        {
            list.retain(|c| *c != channel);
        }
        for c in self.channels.values_mut() {
            if c.parent == Some(channel) {
                c.parent = None;
            }
        }
        if self.selected_channel == Some(channel) {
            self.selected_channel = None;
        }
        self.rebuild_sidebar();
    }

    pub fn open_create_channel(&mut self, guild: Id, parent: Option<Id>, kind: ChannelKind, cx: &mut Context<Self>) {
        open_modal(Modal::CreateChannel(create_channel::State::new(guild, parent, kind)), None);
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_channel(&mut self, guild: Id, parent: Option<Id>, kind: ChannelKind, name: String, private: bool, members: Vec<(Id, u8)>, cx: &mut Context<Self>) {
        let name = channel_name(kind, &name);
        if name.is_empty() {
            return;
        }
        let me = self.me.id;
        let call = AdminCall::CreateChannel { guild: sf(guild), name: name.clone(), kind: new_kind(kind), parent: parent.map(sf), private, me: sf(me), members: members.into_iter().map(|(i, t)| (sf(i), t)).collect() };
        self.admin(call, cx, move |a, r, cx| match r {
            Ok(v) => {
                if let Some(id) = id_of(&v) {
                    let position = a.guild_channels.get(&guild).map_or(0, |l| l.len() as i32);
                    let parent = if kind == ChannelKind::Category { None } else { parent };
                    a.put_channel_local(Channel {
                        id,
                        guild: Some(guild),
                        parent,
                        name: name.into(),
                        topic: None,
                        kind,
                        position,
                        unread: false,
                        muted: false,
                        mentions: 0,
                        voice_users: Vec::new(),
                    });
                    if kind != ChannelKind::Category && kind != ChannelKind::Voice && kind != ChannelKind::Stage {
                        a.selected_channel = Some(id);
                    }
                }
                cx.notify();
            }
            Err(e) => a.parity_toast(e, cx),
        });
    }

    pub fn confirm_delete_channel(&mut self, channel: Id, cx: &mut Context<Self>) {
        let Some(ch) = self.channels.get(&channel) else { return };
        let cat = ch.kind == ChannelKind::Category;
        let name = ch.name.to_string();
        open_modal(
            Modal::Form(Form {
                title: crate::t!(if cat { "admin.delete_category_title" } else { "admin.delete_channel_title" }),
                subtitle: Some(crate::t!(if cat { "admin.delete_category_body" } else { "admin.delete_channel_body" }, name = name)),
                fields: Vec::new(),
                choices: Vec::new(),
                choice: None,
                check: None,
                submit: crate::t!(if cat { "menu.delete_category" } else { "menu.delete_channel" }),
                danger: true,
                preview: None,
                width: 400.,
                on_submit: Rc::new(move |a, _, _, cx| a.delete_channel(channel, cx)),
            }),
            None,
        );
        cx.notify();
    }

    pub fn delete_channel(&mut self, channel: Id, cx: &mut Context<Self>) {
        self.admin(AdminCall::DeleteChannel { channel: sf(channel) }, cx, move |a, r, cx| match r {
            Ok(_) => {
                if crate::actions::with_modal(|m| matches!(m, Some(Modal::ChannelSettings(s)) if s.channel == channel)) {
                    crate::actions::close_modal();
                }
                a.drop_channel_local(channel);
                cx.notify();
            }
            Err(e) => a.parity_toast(e, cx),
        });
    }

    pub fn open_channel_settings(&mut self, channel: Id, cx: &mut Context<Self>) {
        let Some(ch) = self.channels.get(&channel) else { return };
        open_modal(Modal::ChannelSettings(Box::new(channel_settings::State::new(ch))), None);
        self.admin(AdminCall::Channel { channel: sf(channel) }, cx, move |_, r, cx| {
            if let Ok(v) = r {
                crate::actions::with_modal(|m| {
                    if let Some(Modal::ChannelSettings(s)) = m
                        && s.channel == channel
                    {
                        s.load_channel(&v);
                    }
                });
            }
            cx.notify();
        });
        cx.notify();
    }

    pub fn apply_channel_overview(&mut self, channel: Id, name: &str, topic: &str) {
        if let Some(c) = self.channels.get_mut(&channel) {
            c.name = name.to_owned().into();
            c.topic = (!topic.is_empty()).then(|| topic.to_owned().into());
        }
        self.rebuild_sidebar();
    }

    pub fn open_privacy_settings(&mut self, guild: Id, cx: &mut Context<Self>) {
        open_modal(Modal::Privacy(privacy::Privacy::new(guild)), None);
        self.admin(AdminCall::UserSettings, cx, move |a, r, cx| {
            match r {
                Ok(v) => crate::actions::with_modal(|m| {
                    if let Some(Modal::Privacy(p)) = m {
                        p.load(&v);
                    }
                }),
                Err(e) => a.parity_toast(e, cx),
            }
            cx.notify();
        });
        cx.notify();
    }

    pub fn set_guild_privacy(&mut self, guild: Id, key: &'static str, restricted: bool, current: Value, cx: &mut Context<Self>) {
        let list = phoenix_api::rest::guild_manage::toggle_list(&current, sf(guild), restricted);
        self.admin_toast(AdminCall::PatchUserSettings { patch: json!({ key: list }) }, None, cx);
    }

    pub fn security_state(&self, guild: Id) -> privacy::Security {
        let feature = self.backend.guild_features(guild).iter().any(|f| f == "INVITES_DISABLED");
        let now = chrono::Utc::now();
        let inc = self.backend.guild_incidents(guild);
        let invites = feature || inc.as_ref().is_some_and(|i| incident_active(i.invites_disabled_until.as_deref(), now));
        let dms = inc.as_ref().is_some_and(|i| incident_active(i.dms_disabled_until.as_deref(), now));
        privacy::Security::with_state(guild, invites, dms, inc.and_then(|i| i.lockdown_duration_hours), false)
    }

    pub fn under_lockdown(&self, guild: Id) -> bool {
        self.backend.guild_incidents(guild).is_some_and(|i| lockdown(&i, chrono::Utc::now()))
    }

    pub fn open_security_actions(&mut self, guild: Id, cx: &mut Context<Self>) {
        open_modal(Modal::Security(self.security_state(guild)), None);
        cx.notify();
    }
}

pub fn incident_active(until: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> bool {
    until.and_then(|u| chrono::DateTime::parse_from_rfc3339(u).ok()).is_some_and(|t| t > now)
}

pub fn lockdown(i: &phoenix_api::models::IncidentsData, now: chrono::DateTime<chrono::Utc>) -> bool {
    incident_active(i.dms_disabled_until.as_deref(), now) || incident_active(i.invites_disabled_until.as_deref(), now)
}

pub const INCIDENT_PERMS: u64 = super::ADMINISTRATOR | super::MANAGE_GUILD | (1 << 2) | (1 << 1) | super::MODERATE_MEMBERS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockdown_follows_the_future_pause_dates() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-26T12:00:00+00:00").unwrap().with_timezone(&chrono::Utc);
        let mut i = phoenix_api::models::IncidentsData { invites_disabled_until: Some("2026-01-23T19:31:24.511000+00:00".into()), ..Default::default() };
        assert!(!lockdown(&i, now));
        i.dms_disabled_until = Some("2026-11-26T14:00:00.000000+00:00".into());
        assert!(lockdown(&i, now));
        assert!(!incident_active(Some("garbage"), now) && !incident_active(None, now));
    }

    #[test]
    fn names() {
        assert_eq!(channel_name(ChannelKind::Text, "  Hello  World -x "), "hello-world-x");
        assert_eq!(channel_name(ChannelKind::Voice, " Hello World "), "Hello World");
        assert_eq!(channel_name(ChannelKind::Forum, "A"), "a");
    }

    #[test]
    fn mock_lists() {
        assert!(mock_reply(&AdminCall::Bans { guild: Snowflake(1) }).as_array().is_some_and(|a| !a.is_empty()));
        let v = mock_reply(&AdminCall::CreateChannel { guild: Snowflake(1), name: "x".into(), kind: NewChannelKind::Text, parent: None, private: false, me: Snowflake(2), members: Vec::new() });
        assert!(id_of(&v).is_some());
    }
}
