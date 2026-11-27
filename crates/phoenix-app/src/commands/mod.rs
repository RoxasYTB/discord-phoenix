pub mod builtin;
pub mod exec;
pub mod guide;
pub mod slash;
pub mod state;
pub mod store;

pub use state::{ParityEvent, ParityUi, from_gateway, on_event};

use std::sync::OnceLock;

use gpui::Context;
use parking_lot::Mutex;
use serde_json::Value;

use crate::{
    app::PhoenixApp as App,
    backend::ReadyData,
    model::{Id, Message},
};

fn session_slot() -> &'static Mutex<String> {
    static S: OnceLock<Mutex<String>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(String::new()))
}

pub fn set_session_id(id: &str) {
    *session_slot().lock() = id.to_owned();
}

pub fn session_id() -> String {
    session_slot().lock().clone()
}

#[derive(Clone, Debug, Default)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub app_name: String,
    pub app_icon: Option<String>,
    pub raw: Value,
    pub id: String,
    pub app_id: Option<Id>,
    pub bot_id: Option<Id>,
    pub kind: u8,
    pub path: Vec<(u64, String)>,
    pub leaf: Value,
    pub guild_scoped: bool,
}

impl SlashCommand {
    pub fn options(&self) -> &[Value] {
        slash::options(&self.leaf)
    }

    pub fn builtin(&self) -> bool {
        self.id.starts_with('-')
    }
}

#[derive(Clone, Debug)]
pub struct ForumPost {
    pub thread: Id,
    pub title: String,
    pub tags: Vec<Id>,
    pub message_count: u32,
    pub first_message: Option<Message>,
    pub owner: Option<Id>,
    pub last_message: Option<Id>,
    pub member_ids_preview: Vec<Id>,
    pub member_count: u32,
    pub pinned: bool,
    pub archived: bool,
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForumTag {
    pub id: Id,
    pub name: gpui::SharedString,
    pub emoji: Option<String>,
    pub moderated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FolderSync {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub color: Option<u32>,
    pub guilds: Vec<Id>,
}

#[derive(Clone, Debug)]
pub enum ModAction {
    Timeout(u64),
    Kick,
    Ban(u32),
    Unban,
}

pub type PollAnswerDraft = (String, Option<(Option<Id>, String)>);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForumSettings {
    pub sort_created: bool,
    pub gallery: bool,
    pub require_tag: bool,
    pub match_all: bool,
    pub reaction: Option<(Option<Id>, String)>,
    pub guidelines: Option<String>,
}

impl ForumSettings {
    pub fn from_channel(c: &phoenix_api::models::Channel) -> Self {
        Self {
            sort_created: c.default_sort_order == Some(1),
            gallery: c.default_forum_layout == Some(2),
            require_tag: c.flags & (1 << 4) != 0,
            match_all: c.default_tag_setting.as_deref() == Some("match_all"),
            reaction: c
                .default_reaction_emoji
                .as_ref()
                .filter(|r| r.emoji_id.is_some() || r.emoji_name.is_some())
                .map(|r| (r.emoji_id.map(|e| Id(e.0)), r.emoji_name.clone().unwrap_or_default())),
            guidelines: c.topic.clone().filter(|t| !t.trim().is_empty()),
        }
    }

    pub fn can_post(&self, tags: &[Id]) -> bool {
        !self.require_tag || !tags.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PostFile {
    pub path: std::path::PathBuf,
    pub name: Option<String>,
    pub description: String,
    pub spoiler: bool,
}

impl PostFile {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path, ..Default::default() }
    }

    pub fn filename(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into()))
    }
}

#[derive(Clone, Debug)]
pub enum ParityRequest {
    Commands { guild: Option<Id>, channel: Id },
    RunCommand { guild: Option<Id>, channel: Id, command: Value, options: Value, nonce: u64 },
    Autocomplete { guild: Option<Id>, channel: Id, command: Value, options: Value, nonce: u64 },
    ContextCommand { guild: Option<Id>, channel: Id, command: Value, target: Id, nonce: u64 },
    Component {
        guild: Option<Id>,
        channel: Id,
        message: Id,
        app: Id,
        flags: u64,
        custom_id: String,
        kind: u8,
        values: Option<Vec<String>>,
    },
    ModalSubmit { guild: Option<Id>, channel: Id, app: Id, modal: Id, custom_id: String, fields: Vec<(String, String)> },
    Mentions { before: Option<Id>, guild: Option<Id>, everyone: bool, roles: bool },
    DismissMention(Id),
    SavedMessages,
    ScheduledMessages,
    ContentInventory,
    SetSaved { channel: Id, message: Id, saved: bool },
    VotePoll { channel: Id, message: Id, answers: Vec<u32> },
    CreatePoll { channel: Id, question: String, answers: Vec<PollAnswerDraft>, hours: u32, multi: bool },
    EndPoll { channel: Id, message: Id },
    ForumPosts { forum: Id, tags: Vec<Id>, offset: u32, by_creation: bool, archived: bool, match_all: bool },
    CreatePost { forum: Id, title: String, content: String, tags: Vec<Id>, files: Vec<PostFile> },
    Moderate { guild: Id, user: Id, action: ModAction },
    BulkDelete { channel: Id, messages: Vec<Id> },
    CreateGroupDm(Vec<Id>),
    GuildFolders(Vec<FolderSync>),
    ArchivedThreads { channel: Id },
    ScheduledEvents { guild: Id },
    Onboarding { guild: Id },
    ServerGuide { guild: Id },
    FetchThread { id: Id },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OnboardingPrompt {
    pub id: Id,
    pub title: String,
    pub single_select: bool,
    pub options: Vec<OnboardingOption>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OnboardingOption {
    pub id: Id,
    pub title: String,
    pub description: String,
    pub emoji_id: Option<Id>,
    pub emoji_name: Option<String>,
    pub animated: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuildEvent {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub start: i64,
    pub end: Option<i64>,
    pub status: u8,
    pub channel: Option<Id>,
    pub location: Option<String>,
    pub user_count: u32,
    pub creator: Option<Id>,
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InventoryEntry {
    pub id: Id,
    pub author: Id,
    pub app: Option<Id>,
    pub game: Option<String>,
    pub content_type: u64,
    pub first_time: bool,
    pub resurrected: bool,
    pub trending: bool,
    pub streak_days: Option<u64>,
    pub icon: Option<gpui::SharedString>,
    pub live: bool,
    pub started_ms: Option<i64>,
    pub expires_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub enum ParityReply {
    Done,
    Commands(Vec<SlashCommand>),
    Messages(Vec<Message>),
    ForumPosts { posts: Vec<ForumPost>, has_more: bool },
    Channel(Id),
    Inventory(Vec<InventoryEntry>),
    Events(Vec<GuildEvent>),
    Onboarding(Vec<OnboardingPrompt>),
    Guide(Box<guide::GuildGuide>),
    Thread(crate::model::Channel),
}

pub struct Resync(pub ReadyData);

impl std::fmt::Debug for Resync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Resync")
    }
}

pub fn on_resync(app: &mut App, r: Resync, cx: &mut Context<App>) {
    let (guild, channel) = (app.selected_guild, app.selected_channel);
    let (home, discovery) = (app.nav.home, crate::ui::discovery::current());
    app.apply_ready(r.0, cx);
    app.forums_after_resync();
    crate::ui::quests_page::after_resync();
    if guild.is_none() || app.guilds.iter().any(|g| Some(g.id) == guild) {
        app.select_guild(guild, cx);
    }
    if let Some(c) = channel
        && app.channels.contains_key(&c)
    {
        app.select_channel(c, cx);
    }
    if guild.is_none() && channel.is_none() {
        app.nav.home = home;
    }
    if let Some(tab) = discovery {
        crate::ui::discovery::open(tab);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend::Backend, mock::MockBackend};

    #[gpui::test]
    fn resync_rebuilds_and_keeps_selection(cx: &mut gpui::TestAppContext) {
        let backend = std::sync::Arc::new(MockBackend::synchronous());
        let ready = || futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        let first = ready();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| App::new(backend.clone(), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(first, cx);
                let g = app.guilds[0].id;
                app.select_guild(Some(g), cx);
                let ch = app.guild_channels[&g]
                    .iter()
                    .copied()
                    .find(|c| app.channels[c].kind == crate::model::ChannelKind::Text)
                    .unwrap();
                app.select_channel(ch, cx);
                app.guilds.clear();
                on_resync(app, Resync(ready()), cx);
                assert!(!app.guilds.is_empty());
                assert_eq!(app.selected_guild, Some(g));
                assert_eq!(app.selected_channel, Some(ch));
            })
            .unwrap();
    }

    #[gpui::test]
    fn resync_keeps_the_open_home_page(cx: &mut gpui::TestAppContext) {
        use crate::ui::{discovery, home::data::HomeView};
        let backend = std::sync::Arc::new(MockBackend::synchronous());
        let ready = || futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        let first = ready();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| App::new(backend.clone(), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(first, cx);
                app.select_guild(None, cx);
                app.selected_channel = None;
                app.nav.home = HomeView::Quests;
                on_resync(app, Resync(ready()), cx);
                assert_eq!(app.nav.home, HomeView::Quests);
                assert_eq!(app.selected_guild, None);
                discovery::open(discovery::Tab::Apps);
                on_resync(app, Resync(ready()), cx);
                assert_eq!(discovery::current(), Some(discovery::Tab::Apps));
                discovery::close();
            })
            .unwrap();
    }

    #[gpui::test]
    fn forum_loads_again_after_resync(cx: &mut gpui::TestAppContext) {
        let backend = std::sync::Arc::new(MockBackend::synchronous());
        let ready = || futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        let first = ready();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| App::new(backend.clone(), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(first, cx);
                let forum = Id(4_242_424);
                let v = app.parity.forums.entry(forum).or_default();
                v.loading = true;
                v.archived_loading = true;
                v.error = Some("Not logged in.".into());
                on_resync(app, Resync(ready()), cx);
                let v = &app.parity.forums[&forum];
                assert!(!v.loading && !v.loaded && !v.archived_loading && v.error.is_none());
                assert_eq!(v.generation, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn voice_users_survive_ready_after_supplemental(cx: &mut gpui::TestAppContext) {
        use crate::backend::AppEvent;
        let backend = std::sync::Arc::new(MockBackend::synchronous());
        let ready = || futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        let first = ready();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| App::new(backend.clone(), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(first, cx);
                let vc = app.channels.values().find(|c| c.kind == crate::model::ChannelKind::Voice && c.guild.is_some()).map(|c| c.id).unwrap();
                let user = crate::model::Id(4242);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: None, channel: Some(vc), user, own: false }, cx);
                let flags = crate::model::VoiceFlags { self_mute: true, self_deaf: true, ..Default::default() };
                app.handle_event(AppEvent::VoiceFlags { user, flags }, cx);
                let mut live = ready();
                for c in live.channels.iter_mut().filter(|c| c.guild.is_some()) {
                    c.voice_users.clear();
                }
                on_resync(app, Resync(live), cx);
                assert!(app.channels[&vc].voice_users.contains(&user), "voice row lost by the ReadyData after READY_SUPPLEMENTAL");
                let f = crate::ui::voice_panel::flags(user);
                assert!(f.self_mute && f.self_deaf);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: app.channels[&vc].guild, channel: None, user, own: false }, cx);
                assert!(!app.channels[&vc].voice_users.contains(&user));
            })
            .unwrap();
    }

    fn voice_app(cx: &mut gpui::TestAppContext) -> gpui::WindowHandle<App> {
        let backend = std::sync::Arc::new(MockBackend::synchronous());
        let first = futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| App::new(backend.clone(), w, cx));
        window.update(cx, |app, _, cx| app.apply_ready(first, cx)).unwrap();
        window
    }

    fn guild_voice(app: &App) -> (crate::model::Id, crate::model::Id) {
        let c = app.channels.values().find(|c| c.kind == crate::model::ChannelKind::Voice && c.guild.is_some()).unwrap();
        (c.id, c.guild.unwrap())
    }

    #[gpui::test]
    fn leaving_voice_in_another_guild_or_a_dm_keeps_the_member(cx: &mut gpui::TestAppContext) {
        use crate::backend::AppEvent;
        use crate::model::Id;
        let window = voice_app(cx);
        window
            .update(cx, |app, _, cx| {
                let (vc, g) = guild_voice(app);
                let user = Id(5151);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user, own: false }, cx);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(Id(999_001)), channel: None, user, own: false }, cx);
                assert!(app.channels[&vc].voice_users.contains(&user), "a null voice state from another guild removed the member");
                app.handle_event(AppEvent::VoiceStateUpdate { guild: None, channel: None, user, own: false }, cx);
                assert!(app.channels[&vc].voice_users.contains(&user), "a DM call leave removed the guild member");
                let me = app.me.id;
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: me, own: true }, cx);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(Id(999_001)), channel: Some(Id(999_002)), user: me, own: false }, cx);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(Id(999_001)), channel: None, user: me, own: true }, cx);
                assert!(app.channels[&vc].voice_users.contains(&me), "our other session in another guild hid us");
                assert_eq!(app.voice_channel, Some(vc), "a stale leave from another guild ended our call");
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: None, user, own: false }, cx);
                assert!(!app.channels[&vc].voice_users.contains(&user));
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: None, user: me, own: true }, cx);
                assert_eq!(app.voice_channel, None);
            })
            .unwrap();
    }

    #[gpui::test]
    fn another_client_taking_voice_in_our_guild_ends_our_call(cx: &mut gpui::TestAppContext) {
        use crate::backend::AppEvent;
        let window = voice_app(cx);
        window
            .update(cx, |app, _, cx| {
                let (vc, g) = guild_voice(app);
                let me = app.me.id;
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: me, own: true }, cx);
                assert_eq!(app.voice_channel, Some(vc));
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: None, user: me, own: false }, cx);
                assert_eq!(app.voice_channel, Some(vc), "a leave of another client ended our call");
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: me, own: false }, cx);
                assert_eq!(app.voice_channel, None, "another client took our voice and we stayed in the call");
            })
            .unwrap();
    }

    #[gpui::test]
    fn members_heard_on_the_voice_gateway_join_the_list(cx: &mut gpui::TestAppContext) {
        use crate::backend::AppEvent;
        use crate::model::Id;
        use crate::voice::VoiceUiEvent;
        let window = voice_app(cx);
        window
            .update(cx, |app, _, cx| {
                let (vc, g) = guild_voice(app);
                let (a, b, c) = (Id(6161), Id(6262), Id(6363));
                for u in [a, b, c] {
                    crate::voice::roster::forget(u);
                }
                let me = app.me.id;
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: me, own: true }, cx);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: a, own: false }, cx);
                app.handle_event(AppEvent::Voice(VoiceUiEvent::Clients { users: vec![a, b, me], connected: true }), cx);
                app.handle_event(AppEvent::Voice(VoiceUiEvent::SpeakingFlags { user: c, flags: 1 }), cx);
                let users = app.channels[&vc].voice_users.clone();
                for u in [a, b, c, me] {
                    assert_eq!(users.iter().filter(|x| **x == u).count(), 1, "{u:?} missing or doubled in {users:?}");
                }
                let channels: Vec<_> = app.guild_channels[&g].iter().map(|id| {
                    let mut ch = app.channels[id].clone();
                    ch.voice_users.retain(|u| *u == a || *u == me);
                    ch
                }).collect();
                app.handle_event(AppEvent::GuildChannels { guild: g, channels }, cx);
                assert!(app.channels[&vc].voice_users.contains(&b), "a channel rebuild dropped a member only known from the voice gateway");
                app.handle_event(AppEvent::Voice(VoiceUiEvent::Clients { users: vec![b], connected: false }), cx);
                assert!(!app.channels[&vc].voice_users.contains(&b));
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: c, own: false }, cx);
                app.handle_event(AppEvent::Voice(VoiceUiEvent::Clients { users: vec![a, c], connected: false }), cx);
                assert!(app.channels[&vc].voice_users.contains(&a) && app.channels[&vc].voice_users.contains(&c), "the voice gateway overrode a voice state");
                app.handle_event(AppEvent::Voice(VoiceUiEvent::Clients { users: vec![b], connected: true }), cx);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: None, user: me, own: true }, cx);
                assert!(!app.channels[&vc].voice_users.contains(&b), "leaving kept a member only known from the voice gateway");
            })
            .unwrap();
    }

    #[gpui::test]
    fn ready_supplemental_replaces_stale_voice_rows(cx: &mut gpui::TestAppContext) {
        use crate::backend::AppEvent;
        use crate::model::Id;
        let window = voice_app(cx);
        window
            .update(cx, |app, _, cx| {
                let (vc, g) = guild_voice(app);
                let (gone, here) = (Id(7171), Id(7272));
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: gone, own: false }, cx);
                app.handle_event(AppEvent::GuildVoiceReset(g), cx);
                app.handle_event(AppEvent::VoiceStateUpdate { guild: Some(g), channel: Some(vc), user: here, own: false }, cx);
                assert!(!app.channels[&vc].voice_users.contains(&gone));
                assert!(app.channels[&vc].voice_users.contains(&here));
            })
            .unwrap();
    }
}

pub fn parse_event(v: &serde_json::Value) -> Option<GuildEvent> {
    let sid = |k: &str| v.get(k).and_then(|x| x.as_str()).and_then(|x| x.parse::<u64>().ok()).map(Id);
    let time = |k: &str| {
        v.get(k).and_then(|x| x.as_str()).and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok()).map(|d| d.timestamp())
    };
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    Some(GuildEvent {
        id: sid("id")?,
        name: s("name").unwrap_or_default(),
        description: s("description").unwrap_or_default(),
        start: time("scheduled_start_time").unwrap_or(0),
        end: time("scheduled_end_time"),
        status: v.get("status").and_then(|x| x.as_u64()).unwrap_or(1) as u8,
        channel: sid("channel_id"),
        location: v.get("entity_metadata").and_then(|m| m.get("location")).and_then(|x| x.as_str()).map(str::to_owned),
        user_count: v.get("user_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        creator: sid("creator_id"),
        image: s("image"),
    })
}

#[cfg(test)]
mod event_tests {
    #[test]
    fn parses_scheduled_event() {
        let v = serde_json::json!({
            "id": "12", "name": "Game night", "description": "", "status": 1,
            "scheduled_start_time": "2026-11-30T20:00:00+00:00", "scheduled_end_time": null,
            "channel_id": null, "entity_metadata": { "location": "Paris" }, "user_count": 3
        });
        let ev = super::parse_event(&v).unwrap();
        assert_eq!(ev.id, super::Id(12));
        assert_eq!(ev.location.as_deref(), Some("Paris"));
        assert_eq!(ev.user_count, 3);
        assert_eq!(ev.start, 1_796_068_800);
        assert!(ev.end.is_none());
    }
}

pub fn parse_onboarding(v: &serde_json::Value) -> Vec<OnboardingPrompt> {
    let sid = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).and_then(|x| x.parse::<u64>().ok()).map(Id);
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_owned();
    let Some(prompts) = v.get("prompts").and_then(|p| p.as_array()) else { return Vec::new() };
    prompts
        .iter()
        .filter_map(|p| {
            let options = p
                .get("options")
                .and_then(|o| o.as_array())
                .map(|o| {
                    o.iter()
                        .filter_map(|o| {
                            let emoji = o.get("emoji");
                            Some(OnboardingOption {
                                id: sid(o, "id")?,
                                title: s(o, "title"),
                                description: s(o, "description"),
                                emoji_id: emoji.and_then(|e| sid(e, "id")),
                                emoji_name: emoji.and_then(|e| e.get("name")).and_then(|x| x.as_str()).map(str::to_owned),
                                animated: emoji.and_then(|e| e.get("animated")).and_then(|x| x.as_bool()).unwrap_or(false),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(OnboardingPrompt {
                id: sid(p, "id")?,
                title: s(p, "title"),
                single_select: p.get("single_select").and_then(|x| x.as_bool()).unwrap_or(false),
                options,
            })
        })
        .collect()
}

#[cfg(test)]
mod onboarding_tests {
    #[test]
    fn parses_prompts() {
        let v = serde_json::json!({ "prompts": [{ "id": "1", "title": "Topics", "single_select": false, "options": [
            { "id": "2", "title": "AI", "description": "", "emoji": { "id": null, "name": "🧠" } },
            { "id": "3", "title": "Games", "description": "Fun", "emoji": { "id": "44", "name": "pad", "animated": true } }
        ]}]});
        let p = super::parse_onboarding(&v);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].options[0].emoji_name.as_deref(), Some("🧠"));
        assert_eq!(p[0].options[1].emoji_id, Some(super::Id(44)));
        assert!(p[0].options[1].animated);
    }
}
