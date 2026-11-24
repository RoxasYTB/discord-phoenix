//! Desktop notifications for DMs and mentions (notify-rust), honoring the
//! account's per-guild / per-channel notification level and mutes from
//! `user_guild_settings` (READY + `USER_GUILD_SETTINGS_UPDATE`).

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use phoenix_api::{Cache, models as api};
use serde_json::Value;

static ENABLED: AtomicBool = AtomicBool::new(true);

/// Global switch (Settings > Notifications > Enable Desktop Notifications).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Discord `message_notifications` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    All,
    Mentions,
    Nothing,
    /// Channel override "use the server default".
    Inherit,
}

impl Level {
    fn from_value(v: Option<&Value>) -> Self {
        match v.and_then(Value::as_u64) {
            Some(0) => Self::All,
            Some(1) => Self::Mentions,
            Some(2) => Self::Nothing,
            _ => Self::Inherit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelPrefs {
    pub level: Level,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuildPrefs {
    pub level: Level,
    pub muted: bool,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    pub channels: HashMap<u64, ChannelPrefs>,
}

impl Default for GuildPrefs {
    fn default() -> Self {
        // No stored entry: Discord's default for most servers.
        Self { level: Level::Mentions, muted: false, suppress_everyone: false, suppress_roles: false, channels: HashMap::new() }
    }
}

/// Key 0 holds the DM entry (`guild_id: null`).
#[derive(Clone, Debug, Default)]
pub struct NotifyPrefs {
    pub guilds: HashMap<u64, GuildPrefs>,
}

fn snowflake(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

/// `muted` honoring `mute_config.end_time` (temporary mutes).
fn is_muted(e: &Value, now_ms: i64) -> bool {
    if !e.get("muted").and_then(Value::as_bool).unwrap_or(false) {
        return false;
    }
    let end = e.get("mute_config").and_then(|m| m.get("end_time")).and_then(Value::as_str);
    match end.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) {
        Some(t) => t.timestamp_millis() > now_ms,
        None => true,
    }
}

fn parse_entry(e: &Value, now_ms: i64) -> GuildPrefs {
    let mut g = GuildPrefs {
        level: match Level::from_value(e.get("message_notifications")) {
            Level::Inherit => Level::Mentions,
            l => l,
        },
        muted: is_muted(e, now_ms),
        suppress_everyone: e.get("suppress_everyone").and_then(Value::as_bool).unwrap_or(false),
        suppress_roles: e.get("suppress_roles").and_then(Value::as_bool).unwrap_or(false),
        channels: HashMap::new(),
    };
    let overrides = e.get("channel_overrides");
    let list: Vec<&Value> = match overrides {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Object(o)) => o.values().collect(),
        _ => Vec::new(),
    };
    for o in list {
        if let Some(id) = snowflake(o.get("channel_id")) {
            g.channels.insert(
                id,
                ChannelPrefs { level: Level::from_value(o.get("message_notifications")), muted: is_muted(o, now_ms) },
            );
        }
    }
    g
}

impl NotifyPrefs {
    /// READY `user_guild_settings` (`{entries: [...]}` or a bare array).
    pub fn from_ready(v: Option<&Value>) -> Self {
        let mut p = Self::default();
        let entries = v.and_then(|v| v.get("entries").or(Some(v))).and_then(Value::as_array);
        for e in entries.into_iter().flatten() {
            p.update(e);
        }
        p
    }

    /// One entry (also the `USER_GUILD_SETTINGS_UPDATE` payload).
    pub fn update(&mut self, e: &Value) {
        let now = chrono::Utc::now().timestamp_millis();
        let id = snowflake(e.get("guild_id")).unwrap_or(0);
        self.guilds.insert(id, parse_entry(e, now));
    }
}

/// What the message means to us.
#[derive(Clone, Copy, Debug, Default)]
pub struct MessageFacts {
    pub guild: Option<u64>,
    pub channel: u64,
    /// Parent channel for threads (overrides are inherited).
    pub parent: Option<u64>,
    pub from_me: bool,
    pub mentions_me: bool,
    pub mentions_everyone: bool,
    pub mentions_my_role: bool,
}

pub fn should_notify(p: &NotifyPrefs, f: &MessageFacts) -> bool {
    if f.from_me {
        return false;
    }
    let Some(g) = f.guild else {
        // DMs: always, unless the conversation is muted.
        let muted = p.guilds.get(&0).and_then(|d| d.channels.get(&f.channel)).is_some_and(|c| c.muted);
        return !muted;
    };
    let gp = p.guilds.get(&g).cloned().unwrap_or_default();
    let ch = gp.channels.get(&f.channel).or_else(|| f.parent.and_then(|pid| gp.channels.get(&pid)));
    if ch.is_some_and(|c| c.muted) {
        return false;
    }
    let mention = f.mentions_me
        || (f.mentions_everyone && !gp.suppress_everyone)
        || (f.mentions_my_role && !gp.suppress_roles);
    let level = match ch.map(|c| c.level) {
        Some(Level::Inherit) | None => gp.level,
        Some(l) => l,
    };
    // A muted server still notifies direct @mentions (Discord behaviour).
    if gp.muted {
        return f.mentions_me && level != Level::Nothing;
    }
    match level {
        Level::All => true,
        Level::Mentions | Level::Inherit => mention,
        Level::Nothing => false,
    }
}

pub fn facts(cache: &Cache, me: api::Snowflake, m: &api::Message) -> MessageFacts {
    let chan = cache.channel(m.channel_id);
    let guild = m.guild_id.or_else(|| chan.as_ref().and_then(|c| c.guild_id));
    let my_roles = guild.and_then(|g| cache.member(g, me)).map(|mb| mb.roles.clone()).unwrap_or_default();
    MessageFacts {
        guild: guild.map(|g| g.0),
        channel: m.channel_id.0,
        parent: chan.as_ref().and_then(|c| c.parent_id).map(|p| p.0),
        from_me: m.author.id == me,
        mentions_me: m.mentions.iter().any(|u| u.user.id == me),
        mentions_everyone: m.mention_everyone,
        mentions_my_role: m.mention_roles.iter().any(|r| my_roles.contains(r)),
    }
}

/// Gateway hook: notify for `m` unless it's in the channel being viewed.
pub fn on_message(prefs: &NotifyPrefs, cache: &Cache, me: api::Snowflake, viewing: Option<api::Snowflake>, m: &api::Message) {
    if !enabled() || viewing == Some(m.channel_id) {
        return;
    }
    // Blocked authors never notify.
    let blocked = cache.relationships().iter().any(|r| matches!(r.kind, api::RelationshipType::Blocked) && r.user.as_ref().map(|u| u.id).or(r.user_id).unwrap_or(r.id) == m.author.id);
    let f = facts(cache, me, m);
    if blocked || !should_notify(prefs, &f) {
        return;
    }
    let author = m.author.global_name.clone().unwrap_or_else(|| m.author.username.clone());
    let chan = cache.channel(m.channel_id);
    let title = match (f.guild, &chan) {
        (Some(g), Some(c)) => {
            let gname = cache.guild(api::Snowflake(g)).map(|g| g.properties.name.clone()).unwrap_or_default();
            format!("{author} (#{}, {gname})", c.name.clone().unwrap_or_default())
        }
        _ => author,
    };
    let mut body: String = m.content.chars().take(200).collect();
    if body.is_empty() && !m.attachments.is_empty() {
        body = crate::t!("notif.uploaded").to_string();
    }
    show(title, body);
}

fn show(title: String, body: String) {
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .appname("Discord Phoenix")
            .icon(crate::APP_ID)
            .summary(&title)
            .body(&body)
            .show();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn prefs() -> NotifyPrefs {
        NotifyPrefs::from_ready(Some(&json!({"entries": [
            {"guild_id": "1", "message_notifications": 0, "muted": false,
             "channel_overrides": [{"channel_id": "10", "muted": true, "message_notifications": 3},
                                   {"channel_id": "11", "muted": false, "message_notifications": 2}]},
            {"guild_id": "2", "message_notifications": 1, "muted": true, "suppress_everyone": true},
            {"guild_id": "3", "message_notifications": 1, "muted": true,
             "mute_config": {"end_time": "2000-01-01T00:00:00Z"}},
            {"guild_id": null, "channel_overrides": [{"channel_id": "50", "muted": true}]}
        ]})))
    }

    fn f(guild: Option<u64>, channel: u64) -> MessageFacts {
        MessageFacts { guild, channel, ..Default::default() }
    }

    #[test]
    fn levels_mutes_and_dms() {
        let p = prefs();
        assert!(should_notify(&p, &f(Some(1), 12)), "all messages");
        assert!(!should_notify(&p, &f(Some(1), 10)), "muted channel");
        assert!(!should_notify(&p, &MessageFacts { mentions_me: true, ..f(Some(1), 11) }), "channel set to nothing");
        assert!(!should_notify(&p, &f(Some(2), 20)), "muted guild");
        assert!(should_notify(&p, &MessageFacts { mentions_me: true, ..f(Some(2), 20) }), "mention in muted guild");
        assert!(!should_notify(&p, &MessageFacts { mentions_everyone: true, ..f(Some(9), 1) }.with_guild_default()));
        assert!(should_notify(&p, &f(None, 40)), "dm");
        assert!(!should_notify(&p, &f(None, 50)), "muted dm");
        assert!(!should_notify(&p, &MessageFacts { from_me: true, ..f(None, 40) }));
        // Expired temporary mute, mentions-only.
        assert!(!should_notify(&p, &f(Some(3), 30)));
        assert!(should_notify(&p, &MessageFacts { mentions_my_role: true, ..f(Some(3), 30) }));
        // Thread inherits parent override.
        assert!(!should_notify(&p, &MessageFacts { parent: Some(10), ..f(Some(1), 99) }));
    }

    impl MessageFacts {
        fn with_guild_default(self) -> Self {
            Self { mentions_everyone: false, ..self }
        }
    }
}
