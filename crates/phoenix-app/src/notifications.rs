use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use phoenix_api::{Cache, models as api};
use serde_json::Value;

static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    All,
    Mentions,
    Nothing,
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
    pub until: Option<i64>,
    pub flags: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuildPrefs {
    pub level: Level,
    pub muted: bool,
    pub until: Option<i64>,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    pub channels: HashMap<u64, ChannelPrefs>,
    pub explicit_level: bool,
    pub flags: u64,
}

impl Default for GuildPrefs {
    fn default() -> Self {
        Self::with_level(Level::All)
    }
}

impl GuildPrefs {
    fn with_level(level: Level) -> Self {
        Self {
            level,
            muted: false,
            until: None,
            suppress_everyone: false,
            suppress_roles: false,
            channels: HashMap::new(),
            explicit_level: false,
            flags: 0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NotifyPrefs {
    pub guilds: HashMap<u64, GuildPrefs>,
    pub defaults: HashMap<u64, Level>,
}

fn snowflake(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

fn end_time(e: &Value) -> Option<i64> {
    let end = e.get("mute_config").and_then(|m| m.get("end_time")).and_then(Value::as_str)?;
    chrono::DateTime::parse_from_rfc3339(end).ok().map(|t| t.timestamp_millis())
}

fn mute_active(muted: bool, until: Option<i64>, now_ms: i64) -> bool {
    muted && until.is_none_or(|t| t >= now_ms)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn read_mute(e: &Value, now_ms: i64) -> (bool, Option<i64>) {
    let until = end_time(e);
    let muted = e.get("muted").and_then(Value::as_bool).unwrap_or(false);
    (mute_active(muted, until, now_ms), until)
}

pub fn mute_now(e: &Value) -> bool {
    read_mute(e, now_ms()).0
}

fn parse_entry(e: &Value, default: Level, now_ms: i64) -> GuildPrefs {
    let level = Level::from_value(e.get("message_notifications"));
    let (muted, until) = read_mute(e, now_ms);
    let mut g = GuildPrefs {
        explicit_level: level != Level::Inherit,
        level: if level == Level::Inherit { default } else { level },
        muted,
        until,
        suppress_everyone: e.get("suppress_everyone").and_then(Value::as_bool).unwrap_or(false),
        suppress_roles: e.get("suppress_roles").and_then(Value::as_bool).unwrap_or(false),
        channels: HashMap::new(),
        flags: e.get("flags").and_then(Value::as_u64).unwrap_or(0),
    };
    let overrides = e.get("channel_overrides");
    let list: Vec<&Value> = match overrides {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Object(o)) => o.values().collect(),
        _ => Vec::new(),
    };
    for o in list {
        if let Some(id) = snowflake(o.get("channel_id")) {
            let (muted, until) = read_mute(o, now_ms);
            g.channels.insert(id, ChannelPrefs {
                    level: Level::from_value(o.get("message_notifications")),
                    muted,
                    until,
                    flags: o.get("flags").and_then(Value::as_u64).unwrap_or(0),
                },
            );
        }
    }
    g
}

impl NotifyPrefs {
    pub fn from_ready(v: Option<&Value>) -> Self {
        Self::from_ready_with_defaults(v, HashMap::new())
    }

    pub fn from_ready_with_defaults(v: Option<&Value>, defaults: HashMap<u64, Level>) -> Self {
        let mut p = Self { defaults, ..Self::default() };
        let entries = v.and_then(|v| v.get("entries").or(Some(v))).and_then(Value::as_array);
        for e in entries.into_iter().flatten() {
            p.update(e);
        }
        p
    }

    pub fn update(&mut self, e: &Value) {
        let id = snowflake(e.get("guild_id")).unwrap_or(0);
        let entry = parse_entry(e, self.default_level(id), now_ms());
        self.guilds.insert(id, entry);
    }

    pub fn set_default(&mut self, guild: u64, level: Level) {
        self.defaults.insert(guild, level);
        if let Some(g) = self.guilds.get_mut(&guild).filter(|g| !g.explicit_level) {
            g.level = level;
        }
    }

    fn default_level(&self, guild: u64) -> Level {
        if guild == 0 {
            return Level::All;
        }
        self.defaults.get(&guild).copied().unwrap_or(Level::All)
    }

    pub fn guild(&self, guild: Option<u64>) -> GuildPrefs {
        let g = guild.unwrap_or(0);
        self.guilds.get(&g).cloned().unwrap_or_else(|| GuildPrefs::with_level(self.default_level(g)))
    }

    fn channel_prefs(&self, guild: Option<u64>, channel: u64) -> Option<&ChannelPrefs> {
        self.guilds.get(&guild.unwrap_or(0))?.channels.get(&channel)
    }

    pub fn is_guild_muted(&self, guild: Option<u64>) -> bool {
        self.guilds.get(&guild.unwrap_or(0)).is_some_and(|g| mute_active(g.muted, g.until, now_ms()))
    }

    pub fn is_channel_muted(&self, guild: Option<u64>, channel: u64) -> bool {
        self.channel_prefs(guild, channel).is_some_and(|c| mute_active(c.muted, c.until, now_ms()))
    }

    pub fn is_category_muted(&self, guild: Option<u64>, parent: Option<u64>) -> bool {
        parent.is_some_and(|p| self.is_channel_muted(guild, p))
    }

    pub fn is_guild_or_category_or_channel_muted(&self, guild: Option<u64>, channel: u64, parent: Option<u64>) -> bool {
        self.is_guild_muted(guild) || self.is_category_muted(guild, parent) || self.is_channel_muted(guild, channel)
    }

    pub fn resolved_level(&self, guild: Option<u64>, channel: u64, parent: Option<u64>) -> Level {
        let own = |c: u64| self.channel_prefs(guild, c).map(|c| c.level).filter(|l| *l != Level::Inherit);
        own(channel).or_else(|| parent.and_then(own)).unwrap_or_else(|| self.guild(guild).level)
    }

    pub fn header_bell_slashed(&self, guild: Option<u64>, channel: u64, parent: Option<u64>) -> bool {
        guild.is_some() && (self.is_channel_muted(guild, channel) || self.resolved_level(guild, channel, parent) != Level::All)
    }

    pub fn allow_no_messages(&self, guild: Option<u64>, channel: u64, parent: Option<u64>) -> bool {
        self.is_guild_or_category_or_channel_muted(guild, channel, parent) || self.resolved_level(guild, channel, parent) == Level::Nothing
    }
}

impl Level {
    pub fn from_default(v: Option<u8>) -> Self {
        if v == Some(1) { Self::Mentions } else { Self::All }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MessageFacts {
    pub guild: Option<u64>,
    pub channel: u64,
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
    if p.allow_no_messages(f.guild, f.channel, f.parent) {
        return false;
    }
    let Some(g) = f.guild else {
        return true;
    };
    if p.resolved_level(f.guild, f.channel, f.parent) == Level::All {
        return true;
    }
    let gp = p.guild(Some(g));
    f.mentions_me || (f.mentions_everyone && !gp.suppress_everyone) || (f.mentions_my_role && !gp.suppress_roles)
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

pub fn on_message(prefs: &NotifyPrefs, cache: &Cache, me: api::Snowflake, viewing: Option<api::Snowflake>, m: &api::Message) {
    if !enabled() || viewing == Some(m.channel_id) {
        return;
    }
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
        assert!(!should_notify(&p, &MessageFacts { mentions_me: true, ..f(Some(2), 20) }), "mention in muted guild");
        assert!(should_notify(&p, &f(Some(9), 1)), "unknown guild");
        assert!(should_notify(&p, &f(None, 40)), "dm");
        assert!(!should_notify(&p, &f(None, 50)), "muted dm");
        assert!(!should_notify(&p, &MessageFacts { from_me: true, ..f(None, 40) }));
        assert!(!should_notify(&p, &f(Some(3), 30)));
        assert!(should_notify(&p, &MessageFacts { mentions_my_role: true, ..f(Some(3), 30) }));
        assert!(!should_notify(&p, &MessageFacts { parent: Some(10), ..f(Some(1), 99) }));
    }

    #[test]
    fn guild_default_level_is_inherited() {
        let mut d = HashMap::new();
        d.insert(4, Level::Mentions);
        let mut p = NotifyPrefs::from_ready_with_defaults(Some(&json!([{"guild_id": "5", "muted": false}])), d);
        assert_eq!(p.resolved_level(Some(4), 1, None), Level::Mentions);
        assert!(p.header_bell_slashed(Some(4), 1, None));
        assert_eq!(p.resolved_level(Some(5), 1, None), Level::All);
        p.set_default(5, Level::Mentions);
        assert_eq!(p.resolved_level(Some(5), 1, None), Level::Mentions);
        p.set_default(4, Level::All);
        assert!(!p.header_bell_slashed(Some(4), 1, None));
        assert!(!p.header_bell_slashed(None, 1, None), "DMs have no bell");
    }

    #[test]
    fn category_override_and_mute_chain() {
        let p = NotifyPrefs::from_ready(Some(&json!({"entries": [
            {"guild_id": "1", "message_notifications": 0, "muted": false, "channel_overrides": [
                {"channel_id": "100", "muted": true, "message_notifications": 3},
                {"channel_id": "200", "muted": false, "message_notifications": 1},
                {"channel_id": "201", "muted": false, "message_notifications": 0},
                {"channel_id": "300", "muted": true, "mute_config": {"end_time": "2999-01-01T00:00:00Z"}}
            ]},
            {"guild_id": "2", "message_notifications": 0, "muted": true, "mute_config": {"end_time": null}}
        ]})));
        assert!(!p.is_channel_muted(Some(1), 101));
        assert!(p.is_category_muted(Some(1), Some(100)));
        assert!(p.is_guild_or_category_or_channel_muted(Some(1), 101, Some(100)));
        assert!(!p.header_bell_slashed(Some(1), 101, Some(100)));
        assert_eq!(p.resolved_level(Some(1), 202, Some(200)), Level::Mentions);
        assert!(p.header_bell_slashed(Some(1), 202, Some(200)));
        assert_eq!(p.resolved_level(Some(1), 201, Some(200)), Level::All);
        assert!(p.is_channel_muted(Some(1), 300));
        assert!(p.header_bell_slashed(Some(1), 300, None));
        assert!(p.is_guild_muted(Some(2)));
        assert!(p.is_guild_or_category_or_channel_muted(Some(2), 1, None));
        let mut q = p.clone();
        q.guilds.get_mut(&1).unwrap().channels.get_mut(&300).unwrap().until = Some(0);
        assert!(!q.is_channel_muted(Some(1), 300));
    }
}
