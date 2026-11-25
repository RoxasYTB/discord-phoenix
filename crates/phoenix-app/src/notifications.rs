use std::{
    collections::{HashMap, HashSet},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use phoenix_api::{Cache, models as api};
use serde_json::Value;

pub mod content;
#[cfg(target_os = "linux")]
pub mod dbus;

use content::Lookup;

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
    pub highlights_off: bool,
    pub mute_events: bool,
    pub mobile_push: bool,
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
            highlights_off: false,
            mute_events: false,
            mobile_push: true,
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
        highlights_off: e.get("notify_highlights").and_then(Value::as_u64) == Some(1),
        mute_events: e.get("mute_scheduled_events").and_then(Value::as_bool).unwrap_or(false),
        mobile_push: e.get("mobile_push").and_then(Value::as_bool).unwrap_or(true),
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreadFacts {
    pub joined_flags: Option<u64>,
    pub muted: bool,
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
    pub suppressed: bool,
    pub self_mentionable: bool,
    pub spammer: bool,
    pub blocked: bool,
    pub thread: Option<ThreadFacts>,
    pub grandparent: Option<u64>,
    pub voice_text: bool,
    pub in_voice: bool,
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

pub const SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;
pub const SPAMMER: u64 = 1 << 20;
const TH_ALL: u64 = 2;
const TH_MENTIONS: u64 = 4;
const TH_NONE: u64 = 8;

pub fn thread_level(p: &NotifyPrefs, f: &MessageFacts, t: ThreadFacts) -> Level {
    let Some(flags) = t.joined_flags else { return Level::Nothing };
    if flags & TH_ALL != 0 {
        return Level::All;
    }
    if flags & TH_MENTIONS != 0 {
        return Level::Mentions;
    }
    if flags & TH_NONE != 0 {
        return Level::Nothing;
    }
    let Some(parent) = f.parent else { return Level::Nothing };
    if p.is_guild_or_category_or_channel_muted(f.guild, parent, f.grandparent) {
        return Level::Nothing;
    }
    match p.resolved_level(f.guild, parent, f.grandparent) {
        Level::Nothing => Level::Nothing,
        Level::Mentions => Level::Mentions,
        _ => Level::All,
    }
}

pub fn show_message(p: &NotifyPrefs, f: &MessageFacts, c: &crate::sounds::MessageCtx) -> bool {
    if f.suppressed || f.spammer || f.blocked || c.dnd || c.quiet {
        return false;
    }
    if f.from_me && !f.self_mentionable {
        return false;
    }
    if c.focused && c.selected == Some(f.channel) {
        return false;
    }
    if p.allow_no_messages(f.guild, f.channel, f.parent) {
        return false;
    }
    if let Some(t) = f.thread {
        if t.muted {
            return false;
        }
        return match thread_level(p, f, t) {
            Level::Nothing => false,
            Level::All => true,
            _ => f.mentions_me || f.mentions_everyone || f.mentions_my_role,
        };
    }
    let all = p.resolved_level(f.guild, f.channel, f.parent) == Level::All;
    if all && (!f.voice_text || f.in_voice) {
        return true;
    }
    let gp = p.guild(f.guild);
    f.mentions_me || (f.mentions_everyone && !gp.suppress_everyone) || (f.mentions_my_role && !gp.suppress_roles)
}

fn blocked_user(cache: &Cache, user: api::Snowflake) -> bool {
    cache.relationships().iter().any(|r| matches!(r.kind, api::RelationshipType::Blocked) && r.user.as_ref().map(|u| u.id).or(r.user_id).unwrap_or(r.id) == user)
}

pub fn facts(cache: &Cache, me: api::Snowflake, m: &api::Message) -> MessageFacts {
    let mut chan = cache.channel(m.channel_id);
    if m.kind == api::MessageType::ThreadStarterMessage
        && let Some(p) = chan.as_ref().and_then(|c| c.parent_id).and_then(|p| cache.channel(p))
    {
        chan = Some(p);
    }
    let guild = m.guild_id.or_else(|| chan.as_ref().and_then(|c| c.guild_id));
    let my_roles = guild.and_then(|g| cache.member(g, me)).map(|mb| mb.roles.clone()).unwrap_or_default();
    let parent = chan.as_ref().and_then(|c| c.parent_id);
    let thread = chan.as_ref().filter(|c| c.kind.is_thread()).map(|c| ThreadFacts {
        joined_flags: c.member.as_ref().map(|v| v.get("flags").and_then(Value::as_u64).unwrap_or(0)),
        muted: c.member.as_ref().is_some_and(mute_now),
    });
    let grandparent = thread.and(parent).and_then(|p| cache.channel(p)).and_then(|p| p.parent_id).map(|p| p.0);
    let voice_text = chan.as_ref().is_some_and(|c| c.kind.is_voice());
    let in_voice = voice_text && cache.voice_states(guild).iter().any(|v| v.user_id == me && v.channel_id == Some(m.channel_id));
    MessageFacts {
        guild: guild.map(|g| g.0),
        channel: chan.as_ref().map(|c| c.id.0).unwrap_or(m.channel_id.0),
        parent: parent.map(|p| p.0),
        from_me: m.author.id == me,
        mentions_me: m.mentions.iter().any(|u| u.user.id == me),
        mentions_everyone: m.mention_everyone,
        mentions_my_role: m.mention_roles.iter().any(|r| my_roles.contains(r)),
        suppressed: m.flags & SUPPRESS_NOTIFICATIONS != 0,
        self_mentionable: m.kind == api::MessageType::PollResult,
        spammer: m.author.public_flags.unwrap_or(0) & SPAMMER != 0,
        blocked: blocked_user(cache, m.author.id),
        thread,
        grandparent,
        voice_text,
        in_voice,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Channel(u64),
    Pending(u64),
    Dm(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub tag: u64,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub target: Target,
}

pub fn build(cache: &Cache, m: &api::Message) -> Option<Toast> {
    let look = content::CacheLookup { cache };
    let ch = cache.channel(m.channel_id)?;
    let chan = look.chan(&ch);
    let guild = m.guild_id.or(ch.guild_id).map(|g| g.0);
    let author = look.member_name(guild, &m.author);
    let member_avatar = m.member.as_ref().and_then(|mb| mb.avatar.clone()).or_else(|| guild.and_then(|g| cache.member(api::Snowflake(g), m.author.id)).and_then(|mb| mb.avatar.clone()));
    Some(Toast {
        tag: chan.id,
        title: content::title(&look, &author, &chan, m),
        body: content::body(&look, &chan, m)?,
        icon: Some(content::avatar_url(guild, &m.author, member_avatar.as_deref())),
        target: Target::Channel(chan.id),
    })
}

pub fn gate_open() -> bool {
    let c = crate::sounds::message_ctx();
    enabled() && !c.dnd && !c.quiet
}

pub fn on_message(prefs: &NotifyPrefs, cache: &Cache, me: api::Snowflake, m: &api::Message) {
    if !enabled() {
        return;
    }
    let c = crate::sounds::message_ctx();
    let f = facts(cache, me, m);
    if !show_message(prefs, &f, &c) {
        return;
    }
    if let Some(t) = build(cache, m) {
        show(t);
    }
}

static OUTGOING: Mutex<Option<HashSet<u64>>> = Mutex::new(None);

fn outgoing() -> std::sync::MutexGuard<'static, Option<HashSet<u64>>> {
    OUTGOING.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn rel_user(r: &api::Relationship) -> u64 {
    r.user.as_ref().map(|u| u.id).or(r.user_id).unwrap_or(r.id).0
}

pub fn on_ready(cache: &Cache) {
    let set = cache.relationships().iter().filter(|r| matches!(r.kind, api::RelationshipType::OutgoingRequest)).map(|r| rel_user(r)).collect();
    *outgoing() = Some(set);
}

pub fn relationship_toast(r: &api::Relationship, was_outgoing: bool) -> Option<Toast> {
    let user = r.user.as_ref()?;
    let (body, target) = match r.kind {
        api::RelationshipType::IncomingRequest if !r.user_ignored => (crate::t!("notif.friend_request"), Target::Pending(user.id.0)),
        api::RelationshipType::Friend if was_outgoing => (crate::t!("notif.friend_accepted"), Target::Dm(user.id.0)),
        _ => return None,
    };
    Some(Toast { tag: user.id.0, title: user.username.clone(), body: body.to_string(), icon: Some(content::avatar_url(None, user, None)), target })
}

pub fn on_relationship_add(r: &api::Relationship) {
    let id = rel_user(r);
    let was = {
        let mut g = outgoing();
        let set = g.get_or_insert_with(HashSet::new);
        let was = set.remove(&id);
        if matches!(r.kind, api::RelationshipType::OutgoingRequest) {
            set.insert(id);
        }
        was
    };
    if gate_open()
        && let Some(t) = relationship_toast(r, was)
    {
        show(t);
    }
}

pub fn on_relationship_remove(r: &api::Relationship) {
    if let Some(s) = outgoing().as_mut() {
        s.remove(&rel_user(r));
    }
}

pub fn on_ack(channel: u64) {
    let c = crate::sounds::message_ctx();
    if c.focused || c.selected != Some(channel) {
        clear(channel);
    }
}

static WAS_FOCUSED: AtomicBool = AtomicBool::new(true);

pub fn sync_focus(focused: bool, selected: Option<u64>) {
    let was = WAS_FOCUSED.swap(focused, Ordering::Relaxed);
    if focused
        && !was
        && let Some(ch) = selected
    {
        clear(ch);
    }
}

pub fn show(t: Toast) {
    #[cfg(target_os = "linux")]
    let _ = dbus::send(dbus::Cmd::Show(t));
    #[cfg(not(target_os = "linux"))]
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new().appname("Discord Phoenix").icon(crate::APP_ID).summary(&t.title).body(&t.body).show();
    });
}

pub fn clear(tag: u64) {
    #[cfg(target_os = "linux")]
    let _ = dbus::send(dbus::Cmd::Close(tag));
    #[cfg(not(target_os = "linux"))]
    let _ = tag;
}

#[cfg(target_os = "linux")]
pub fn spawn(cx: &mut gpui::App) {
    use futures::StreamExt;
    if std::env::var_os("PHOENIX_NO_NOTIFY").is_some() {
        return;
    }
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<dbus::Click>();
    if !dbus::spawn(tx) {
        return;
    }
    cx.spawn(async move |cx| {
        while let Some(c) = rx.next().await {
            cx.update(|cx| on_click(c, cx));
        }
    })
    .detach();
}

#[cfg(not(target_os = "linux"))]
pub fn spawn(_: &mut gpui::App) {}

#[cfg(target_os = "linux")]
fn on_click(c: dbus::Click, cx: &mut gpui::App) {
    if let Some(t) = c.token {
        crate::tray::activation::provide(t);
    }
    crate::tray::activate(cx);
    let Some(app) = cx.try_global::<crate::tray::Target>().and_then(|t| t.0.upgrade()) else { return };
    app.update(cx, |a, cx| open_target(a, c.target, cx));
}

pub fn open_target(a: &mut crate::app::PhoenixApp, t: Target, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    use crate::model::Id;
    match t {
        Target::Channel(ch) => {
            if a.channels.contains_key(&Id(ch)) {
                a.open_channel_chat(Id(ch), cx);
            }
        }
        Target::Pending(user) => {
            a.parity.inbox.open = false;
            a.select_guild(None, cx);
            a.nav.friends_tab = crate::ui::home::data::FriendsTab::Pending;
            a.open_profile_modal(Id(user), cx);
        }
        Target::Dm(user) => a.open_dm_with(Id(user), cx),
    }
    cx.notify();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sounds::MessageCtx;
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

    fn ctx() -> MessageCtx {
        MessageCtx { focused: false, selected: None, dnd: false, quiet: false }
    }

    #[test]
    fn client_message_rules() {
        let p = prefs();
        let c = ctx();
        let base = f(Some(1), 12);
        assert!(show_message(&p, &base, &c));
        assert!(!show_message(&p, &MessageFacts { suppressed: true, ..base }, &c), "@silent");
        assert!(!show_message(&p, &MessageFacts { blocked: true, ..base }, &c));
        assert!(!show_message(&p, &MessageFacts { spammer: true, ..base }, &c));
        assert!(!show_message(&p, &base, &MessageCtx { dnd: true, ..c }), "do not disturb");
        assert!(!show_message(&p, &base, &MessageCtx { quiet: true, ..c }), "quiet mode");
        assert!(!show_message(&p, &base, &MessageCtx { focused: true, selected: Some(12), ..c }), "viewing it");
        assert!(show_message(&p, &base, &MessageCtx { selected: Some(12), ..c }), "selected, window in the background");
        assert!(show_message(&p, &base, &MessageCtx { focused: true, selected: Some(13), ..c }));
        assert!(!show_message(&p, &MessageFacts { from_me: true, ..base }, &c));
        assert!(show_message(&p, &MessageFacts { from_me: true, self_mentionable: true, ..base }, &c), "own poll result");
        assert!(!show_message(&p, &f(Some(1), 10), &c), "muted channel");
        assert!(!show_message(&p, &MessageFacts { parent: Some(10), ..f(Some(1), 99) }, &c), "muted category");
        assert!(!show_message(&p, &f(Some(2), 20), &c), "muted guild");
        assert!(!show_message(&p, &MessageFacts { mentions_me: true, ..f(Some(2), 20) }, &c), "mention in a muted guild");
        assert!(show_message(&p, &f(None, 40), &c), "dm");
        assert!(!show_message(&p, &f(None, 50), &c), "muted dm");
    }

    #[test]
    fn everyone_and_role_suppression() {
        let p = NotifyPrefs::from_ready(Some(&json!([
            {"guild_id": "1", "message_notifications": 1, "suppress_everyone": true, "suppress_roles": true},
            {"guild_id": "2", "message_notifications": 1}
        ])));
        let c = ctx();
        for (g, shown) in [(1, false), (2, true)] {
            let b = f(Some(g), 5);
            assert!(!show_message(&p, &b, &c), "mentions only");
            assert_eq!(show_message(&p, &MessageFacts { mentions_everyone: true, ..b }, &c), shown);
            assert_eq!(show_message(&p, &MessageFacts { mentions_my_role: true, ..b }, &c), shown);
            assert!(show_message(&p, &MessageFacts { mentions_me: true, ..b }, &c));
        }
    }

    #[test]
    fn voice_chat_and_threads() {
        let p = NotifyPrefs::from_ready(Some(&json!([{"guild_id": "1", "message_notifications": 0, "channel_overrides": [
            {"channel_id": "30", "message_notifications": 1},
            {"channel_id": "31", "muted": true}
        ]}])));
        let c = ctx();
        let v = MessageFacts { voice_text: true, ..f(Some(1), 4) };
        assert!(!show_message(&p, &v, &c), "voice chat only while connected");
        assert!(show_message(&p, &MessageFacts { in_voice: true, ..v }, &c));
        assert!(show_message(&p, &MessageFacts { mentions_me: true, ..v }, &c));
        let t = |flags: Option<u64>, parent: u64| MessageFacts { thread: Some(ThreadFacts { joined_flags: flags, muted: false }), parent: Some(parent), ..f(Some(1), 99) };
        assert!(!show_message(&p, &t(None, 20), &c), "not joined");
        assert!(show_message(&p, &t(Some(0), 20), &c), "inherits all messages");
        assert!(!show_message(&p, &t(Some(0), 30), &c), "parent set to mentions");
        assert!(show_message(&p, &MessageFacts { mentions_me: true, ..t(Some(0), 30) }, &c));
        assert!(show_message(&p, &t(Some(TH_ALL), 30), &c));
        assert!(!show_message(&p, &t(Some(TH_NONE), 20), &c));
        assert!(!show_message(&p, &t(Some(TH_ALL), 31), &c), "muted parent");
        let muted = MessageFacts { thread: Some(ThreadFacts { joined_flags: Some(TH_ALL), muted: true }), ..t(Some(TH_ALL), 20) };
        assert!(!show_message(&p, &muted, &c));
    }

    #[test]
    fn friend_request_toasts() {
        let u = api::User { id: api::Snowflake(7), username: "bob".into(), ..Default::default() };
        let r = |kind, ignored| api::Relationship { id: api::Snowflake(7), kind, user: Some(u.clone()), user_id: None, nickname: None, since: None, user_ignored: ignored, is_spam_request: false, note: None };
        let t = relationship_toast(&r(api::RelationshipType::IncomingRequest, false), false).unwrap();
        assert_eq!((t.tag, t.title.as_str(), t.target), (7, "bob", Target::Pending(7)));
        assert_eq!(t.body, crate::t!("notif.friend_request").to_string());
        assert!(relationship_toast(&r(api::RelationshipType::IncomingRequest, true), false).is_none(), "ignored user");
        assert!(relationship_toast(&r(api::RelationshipType::Friend, false), false).is_none());
        assert_eq!(relationship_toast(&r(api::RelationshipType::Friend, false), true).unwrap().target, Target::Dm(7));
        assert!(relationship_toast(&r(api::RelationshipType::OutgoingRequest, false), false).is_none());
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
