//! Root view: owns UI state, talks to the `Backend`, applies `AppEvent`s.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{Local, TimeZone};
use futures::StreamExt;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ListAlignment, ListState, ParentElement, Render, SharedString, Styled, Subscription, Window,
    actions, div, prelude::*, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    backend::{AppEvent, Backend, Credentials, EventSink, ReadyData},
    grouping,
    markdown::MentionResolver,
    model::*,
    richtext::{self, RenderedMessage},
    theme::{FONT_UI, Palette, c},
    ui,
};

actions!(phoenix, [QuickSwitcher, Escape, EditLast, ToggleMembers]);

pub const CONTEXT: &str = "PhoenixApp";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-k", QuickSwitcher, Some(CONTEXT)),
        KeyBinding::new("ctrl-,", ui::settings::OpenSettings, Some(CONTEXT)),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
        KeyBinding::new("ctrl-u", ToggleMembers, Some(CONTEXT)),
    ]);
    crate::shortcuts::bind_keys(cx);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Main,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SidebarRow {
    Category(Id),
    Channel(Id),
    /// Active thread under its parent channel.
    Thread(Id),
    VoiceUser { user: Id },
    /// Stage channel audience strip under its speakers.
    StageAudience(Id),
}

#[derive(Clone, Debug)]
pub enum MemberRow {
    Header(SharedString),
    Member(usize),
}

#[derive(Default)]
pub struct ChannelMessages {
    pub msgs: Vec<Message>,
    pub rendered: Vec<RenderedMessage>,
    pub loading: bool,
    pub loaded_once: bool,
    pub reached_start: bool,
    /// Showing a page around a jumped-to message, not the latest (history_detached).
    pub detached: bool,
}

impl ChannelMessages {
    /// Position of message `id`. History is in snowflake order, so this is a
    /// binary search; out-of-order entries fall back to a reverse scan.
    pub fn index_of(&self, id: Id) -> Option<usize> {
        match self.msgs.binary_search_by_key(&id.0, |m| m.id.0) {
            Ok(i) if self.msgs[i].id == id => Some(i),
            _ => self.msgs.iter().rposition(|m| m.id == id),
        }
    }
}

pub use crate::ui::switcher::Switcher;

pub struct PhoenixApp {
    pub backend: Arc<dyn Backend>,
    pub screen: Screen,
    pub focus: FocusHandle,

    // Login
    pub login_email: Entity<InputState>,
    pub login_password: Entity<InputState>,
    pub login_token: Entity<InputState>,
    pub login_with_token: bool,
    /// TOTP code input, shown after a password login asked for 2FA.
    pub login_mfa: Entity<InputState>,
    pub login_mfa_step: bool,
    pub login_error: Option<SharedString>,
    pub logging_in: bool,

    // Data
    pub me: User,
    pub users: HashMap<Id, User>,
    pub guilds: Vec<Guild>,
    pub channels: HashMap<Id, Channel>,
    pub guild_channels: HashMap<Id, Vec<Id>>,
    pub dms: Vec<Id>,
    pub roles: HashMap<Id, Vec<Role>>,
    pub members: HashMap<Id, Vec<Member>>,
    /// Voice states (user -> channel) received before their channel exists (READY_SUPPLEMENTAL
    /// racing ReadyData on first login); applied by `apply_ready`.
    pub pending_voice: HashMap<Id, Id>,
    /// Presence of users outside the open guild's member list (DMs, friends).
    pub presences: HashMap<Id, Presence>,
    pub messages: HashMap<Id, ChannelMessages>,

    // Selection
    pub selected_guild: Option<Id>,
    pub selected_channel: Option<Id>,
    pub last_channel: HashMap<Option<Id>, Id>,
    pub collapsed: HashSet<Id>,
    pub nav: crate::ui::home::data::NavState,

    // Derived rows (rebuilt on change, not per frame)
    pub sidebar_rows: Vec<SidebarRow>,
    /// Guild the current `sidebar_rows` were built for (scroll anchoring).
    pub sidebar_rows_guild: Option<Id>,
    pub member_rows: Vec<MemberRow>,

    // Virtualized list states
    pub guild_list: ListState,
    pub sidebar_list: ListState,
    pub message_list: ListState,
    pub member_list: ListState,

    // Composer
    pub composer: Entity<gpui_component::input::TextareaState>,
    pub replying_to: Option<Id>,
    pub editing: Option<Id>,
    pub typing: HashMap<Id, Vec<(Id, Instant)>>,
    pub typing_label: Option<SharedString>,
    /// Server nick sent with TYPING_START, keyed by (channel, user).
    pub typing_nicks: HashMap<(Id, Id), SharedString>,

    // Self state
    pub presence: Presence,
    pub mic_muted: bool,
    pub deafened: bool,
    pub voice_channel: Option<Id>,
    pub voice: crate::voice::VoiceUi,
    pub show_members: bool,
    pub people: crate::presence::People,

    pub switcher: Option<Switcher>,
    pub settings: Option<ui::settings::SettingsState>,
    pub prefs: ui::settings::Prefs,
    pub login_ui: crate::auth::LoginUi,
    pub chat: crate::ui::chat::ChatState,
    pub parity: crate::commands::ParityUi,
    pub revealed: HashSet<(Id, usize)>,
    pub utc_offset: i64,
    _subs: Vec<Subscription>,
}

pub(crate) struct Resolver<'a> {
    users: &'a HashMap<Id, User>,
    channels: &'a HashMap<Id, Channel>,
    roles: &'a HashMap<Id, Vec<Role>>,
    members: &'a HashMap<Id, Vec<Member>>,
    soundboard_sounds: &'a [crate::model::SoundboardSound],
    me: Id,
}
impl Resolver<'_> {
    fn find_role(&self, id: u64) -> Option<&Role> {
        self.roles.values().flatten().find(|r| r.id.0 == id)
    }
}
impl MentionResolver for Resolver<'_> {
    fn role(&self, id: u64) -> Option<String> {
        self.find_role(id).map(|r| r.name.to_string())
    }
    fn role_color(&self, id: u64) -> Option<u32> {
        self.find_role(id).and_then(|r| r.color)
    }
    fn has_role(&self, id: u64) -> bool {
        let Some((g, _)) = self.roles.iter().find(|(_, rs)| rs.iter().any(|r| r.id.0 == id)) else { return false };
        self.members
            .get(g)
            .and_then(|ms| ms.iter().find(|m| m.user == self.me))
            .is_some_and(|m| m.roles.contains(&Id(id)))
    }
    fn user(&self, id: u64) -> Option<String> {
        self.users.get(&Id(id)).map(|u| u.display_name.to_string())
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.channels.get(&Id(id)).map(|c| c.name.to_string())
    }
    fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<crate::markdown::SoundboardMention> {
        let sound = self.soundboard_sounds.iter().find(|s| {
            s.id.0 == sound_id && s.guild.is_none_or(|guild| guild.0 == guild_id)
        })?;
        Some(crate::markdown::SoundboardMention {
            name: sound.name.as_ref().map(ToString::to_string).filter(|n| !n.is_empty()).unwrap_or_else(|| "Unknown".into()),
            emoji_name: sound.emoji_name.as_ref().map(ToString::to_string),
        })
    }
}

pub fn format_time_full(ts: i64) -> SharedString {
    let Some(dt) = Local.timestamp_opt(ts, 0).single() else { return "".into() };
    crate::i18n::calendar(dt, Local::now())
}

pub(crate) fn format_time_short(ts: i64) -> SharedString {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| crate::i18n::clock(d).into())
        .unwrap_or_default()
}

pub(crate) fn format_day(ts: i64) -> SharedString {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| crate::i18n::long_date(d.date_naive()).into())
        .unwrap_or_default()
}

impl PhoenixApp {
    pub fn new(backend: Arc<dyn Backend>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let login_email = cx.new(|cx| InputState::new(window, cx));
        let login_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let login_token = cx.new(|cx| InputState::new(window, cx).masked(true));
        let login_mfa = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("login.six_digit")));
        // scrollableContainer_: grows with the text up to --custom-channel-textarea-text-area-max-height
        // (50vh), then scrolls; Enter sends, Shift+Enter inserts a newline. 22px lines.
        let max_rows = ((f32::from(window.viewport_size().height) * 0.5 / 22.).floor() as usize).max(2);
        let composer = cx.new(|cx| gpui_component::input::TextareaState::new(window, cx).auto_grow(1, max_rows).submit_on_enter(true));

        let mut subs = Vec::new();
        subs.extend(ui::pickers::install(&composer, window, cx));
        subs.push(cx.subscribe_in(&composer, window, |this, _, ev: &InputEvent, window, cx| {
            match ev {
                InputEvent::PressEnter { shift: false, .. } => this.submit_composer(window, cx),
                InputEvent::Change => this.on_composer_change(cx),
                _ => {}
            }
        }));
        for input in [&login_email, &login_password, &login_token, &login_mfa] {
            subs.push(cx.subscribe_in(input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.submit_login(window, cx);
                }
            }));
        }

        // Gateway event pump: backend threads -> channel -> UI thread.
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<AppEvent>();
        let sink: EventSink = Arc::new(move |ev| {
            let _ = tx.unbounded_send(ev);
        });
        backend.set_event_sink(sink);
        cx.spawn(async move |this, cx| {
            while let Some(ev) = rx.next().await {
                // Handle every queued event in one update (one redraw per batch, not per event).
                let mut batch = vec![ev];
                while let Ok(ev) = rx.try_recv() {
                    batch.push(ev);
                }
                if this.update(cx, |this, cx| batch.into_iter().for_each(|ev| this.handle_event(ev, cx))).is_err() {
                    break;
                }
            }
        })
        .detach();

        // Typing indicator expiry tick.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| { this.expire_typing(cx); if crate::voice::start_time::ticking() || crate::ui::chat::composer::cooling() { cx.notify(); } }).is_err() {
                    break;
                }
            }
        })
        .detach();

        let message_list = ListState::new(0, ListAlignment::Bottom, px(800.));
        let weak = cx.entity().downgrade();
        message_list.set_scroll_handler(move |ev: &gpui::ListScrollEvent, _w, cx: &mut App| {
            if ev.visible_range.start == 0 {
                let _ = weak.update(cx, |this, cx| this.load_older(cx));
            }
            let _ = weak.update(cx, |this, cx| this.detached_on_scroll(ev, cx));
            let _ = weak.update(cx, |this, cx| this.chat_on_scroll(ev, cx));
        });

        // Auto-login from the stored session (keyring), if any.
        let restore = backend.restore_session();
        let logging_in = restore.is_some();
        // Draw the last session from disk while the gateway connects (docs/perf.md).
        if let Some(fut) = restore.is_some().then(|| backend.restore_cached()).flatten() {
            cx.spawn(async move |this, cx| {
                if let Ok(ready) = fut.await {
                    let _ = this.update(cx, |this, cx| {
                        if this.logging_in && this.screen == Screen::Login {
                            this.apply_ready(ready, cx);
                            crate::perf::mark("snapshot_applied");
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        if let Some(fut) = restore {
            cx.spawn(async move |this, cx| {
                let res = fut.await;
                let _ = this.update(cx, |this, cx| {
                    this.logging_in = false;
                    match res {
                        // Already showing the disk snapshot: rebuild in place, keep the selection.
                        Ok(ready) if this.screen == Screen::Main => {
                            crate::commands::on_resync(this, crate::commands::Resync(ready), cx)
                        }
                        Ok(ready) => this.apply_ready(ready, cx),
                        Err(e) => {
                            if this.screen == Screen::Main {
                                this.clear_session();
                                this.screen = Screen::Login;
                            }
                            // Empty message = nothing stored: stay quietly on the login screen.
                            if !e.0.is_empty() {
                                this.login_error = Some(e.0.into());
                            }
                        }
                    }
                    crate::perf::mark("ready_applied");
                    cx.notify();
                });
            })
            .detach();
        }

        let utc_offset = Local::now().offset().local_minus_utc() as i64;
        // One lock: two `prefs().lock()` temporaries in the same struct
        // literal deadlock (both guards live until the end of the statement).
        let (self_mute, self_deaf) = {
            let p = crate::voice::prefs::prefs().lock();
            (p.self_mute, p.self_deaf)
        };
        Self {
            backend,
            screen: Screen::Login,
            focus: cx.focus_handle(),
            login_email,
            login_password,
            login_token,
            login_with_token: false,
            login_mfa,
            login_mfa_step: false,
            login_error: None,
            logging_in,
            me: User {
                id: Id(0),
                name: "".into(),
                display_name: "".into(),
                avatar: ImageRef::Default(0),
                bot: false,
                verified_bot: false,
                tag: None,
                nameplate: None,
                name_style: None,
                decoration: None,
            },
            users: HashMap::new(),
            guilds: Vec::new(),
            channels: HashMap::new(),
            guild_channels: HashMap::new(),
            dms: Vec::new(),
            roles: HashMap::new(),
            members: HashMap::new(),
            pending_voice: HashMap::new(),
            presences: HashMap::new(),
            messages: HashMap::new(),
            selected_guild: None,
            selected_channel: None,
            last_channel: HashMap::new(),
            collapsed: HashSet::new(),
            nav: Default::default(),
            sidebar_rows: Vec::new(),
            sidebar_rows_guild: None,
            member_rows: Vec::new(),
            guild_list: ListState::new(0, ListAlignment::Top, px(200.)),
            sidebar_list: ListState::new(0, ListAlignment::Top, px(300.)),
            message_list,
            member_list: ListState::new(0, ListAlignment::Top, px(300.)),
            composer,
            replying_to: None,
            editing: None,
            typing: HashMap::new(),
            typing_label: None,
            typing_nicks: HashMap::new(),
            presence: Presence::Online,
            mic_muted: self_mute,
            deafened: self_deaf,
            voice_channel: None,
            voice: Default::default(),
            show_members: true,
            people: Default::default(),
            switcher: None,
            chat: Default::default(),
            parity: Default::default(),
            revealed: HashSet::new(),
            utc_offset,
            settings: None,
            prefs: ui::settings::Prefs::load(),
            login_ui: Default::default(),
            _subs: subs,
        }
    }

    // ---------------------------------------------------------------- login

    pub fn submit_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.logging_in {
            return;
        }
        if self.login_mfa_step && !self.login_with_token {
            return crate::auth::submit_mfa(self, cx);
        }
        let creds = if self.login_mfa_step && !self.login_with_token {
            Credentials::Mfa { code: self.login_mfa.read(cx).value().to_string() }
        } else if self.login_with_token {
            Credentials::Token(self.login_token.read(cx).value().to_string())
        } else {
            Credentials::Password {
                login: self.login_email.read(cx).value().to_string(),
                password: self.login_password.read(cx).value().to_string(),
            }
        };
        self.logging_in = true;
        self.login_error = None;
        cx.notify();
        let fut = self.backend.login(creds);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.logging_in = false;
                match res {
                    Ok(ready) => {
                        this.login_mfa_step = false;
                        this.apply_ready(ready, cx)
                    }
                    Err(e) => {
                        this.login_mfa_step = this.backend.needs_mfa();
                        this.login_error = Some(e.0.into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Drop everything tied to the logged-in account.
    fn clear_session(&mut self) {
        self.users.clear();
        self.guilds.clear();
        self.channels.clear();
        self.guild_channels.clear();
        self.dms.clear();
        self.roles.clear();
        self.members.clear();
        self.people.clear();
        self.presences.clear();
        self.messages.clear();
        self.selected_guild = None;
        self.selected_channel = None;
        self.last_channel.clear();
        self.sidebar_rows.clear();
        self.member_rows.clear();
        self.typing.clear();
        self.typing_label = None;
        self.replying_to = None;
        self.editing = None;
        self.voice_channel = None;
        self.switcher = None;
        self.chat = Default::default();
        self.guild_list.reset(0);
        self.sidebar_list.reset(0);
        self.message_list.reset(0);
        self.member_list.reset(0);
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        let fut = self.backend.logout();
        cx.spawn(async move |_, _| {
            let _ = fut.await;
        })
        .detach();
        self.clear_session();
        self.login_mfa_step = false;
        self.login_error = None;
        self.screen = Screen::Login;
        cx.notify();
    }

    pub fn apply_ready(&mut self, r: ReadyData, cx: &mut Context<Self>) {
        // Own activities are emitted during login, before this runs (convert::self_activities).
        let mine = self.people.activities.remove(&r.me.id);
        // READY_SUPPLEMENTAL voice states can land before this ReadyData, on the
        // channels of the disk snapshot (resync) or of a previous session, and
        // `clear_session` drops those channels: keep their voice users.
        let old_voice: HashMap<Id, Vec<Id>> = self
            .channels
            .values()
            .filter(|c| c.guild.is_some() && !c.voice_users.is_empty())
            .map(|c| (c.id, c.voice_users.clone()))
            .collect();
        self.clear_session();
        if let Some(a) = mine {
            self.people.custom_status = a.iter().find(|x| x.kind == crate::presence::ActivityKind::Custom).and_then(|x| x.state.clone());
            self.people.activities.insert(r.me.id, a);
        }
        self.presences = r.presences.into_iter().collect();
        self.me = r.me;
        self.users = r.users.into_iter().map(|u| (u.id, u)).collect();
        self.guilds = r.guilds;
        for (g, roles) in r.roles {
            self.roles.insert(g, roles);
        }
        for mut ch in r.channels {
            if let Some(g) = ch.guild {
                self.guild_channels.entry(g).or_default().push(ch.id);
            }
            // READY_SUPPLEMENTAL (guild voice_states) can be delivered before this ReadyData:
            // keep the voice users it already put on the channel.
            if ch.voice_users.is_empty()
                && ch.guild.is_some()
                && let Some(old) = old_voice.get(&ch.id)
            {
                ch.voice_users = old.clone();
            }
            if ch.guild.is_some() {
                for (u, _) in self.pending_voice.iter().filter(|(_, c)| **c == ch.id) {
                    if !ch.voice_users.contains(u) {
                        ch.voice_users.push(*u);
                    }
                }
            }
            self.channels.insert(ch.id, ch);
        }
        for list in self.guild_channels.values_mut() {
            list.sort_by_key(|id| self.channels[id].position);
        }
        // Our own voice state can arrive (READY_SUPPLEMENTAL) before this
        // ReadyData, while `me` was still unknown, so the event never set
        // `voice_channel`: recover it from the queued voice users.
        let me = self.me.id;
        self.voice_channel = self.pending_voice.get(&me).copied().or_else(|| {
            self.channels.values().find(|c| c.guild.is_some() && c.voice_users.contains(&me)).map(|c| c.id)
        });
        for ch in r.dm_channels {
            self.dms.push(ch.id);
            self.channels.insert(ch.id, ch);
        }
        // Home + separator + guilds + separator + add + discover
        self.guild_list.reset(self.guilds.len() + 5);
        self.screen = Screen::Main;
        self.nav_after_ready();
        let first = self.guilds.first().map(|g| g.id);
        self.select_guild(first, cx);
        ui::settings::on_ready(self, cx);
    }

    // ----------------------------------------------------------- selection

    pub fn select_guild(&mut self, guild: Option<Id>, cx: &mut Context<Self>) {
        self.selected_guild = guild;
        // Selecting a guild clears nothing: its badge is the sum of its channels' mentions.
        self.rebuild_sidebar();
        self.rebuild_members();
        self.backend.select_guild(guild).detach_ok();
        let target = self.last_channel.get(&guild).copied().or_else(|| match guild {
            Some(g) => self.guild_channels.get(&g).and_then(|l| {
                l.iter().copied().find(|id| {
                    matches!(self.channels[id].kind, ChannelKind::Text | ChannelKind::Announcement)
                        && self.channels[id].name.as_ref() == "general"
                })
                .or_else(|| {
                    l.iter().copied().find(|id| self.channels[id].kind == ChannelKind::Text)
                })
            }),
            None => None,
        });
        self.nav_home_target(guild, target.is_some());
        if let Some(t) = target {
            // Restoring the remembered chat (may be a voice channel's text chat): never joins.
            self.open_channel_chat(t, cx);
        }
        cx.notify();
    }

    /// Guild badge = sum of its channels' mentions (the client's GuildReadStateStore).
    fn refresh_guild_mentions(&mut self, guild: Option<Id>) {
        let Some(g) = guild else { return };
        let n = self.guild_channels.get(&g).map_or(0, |l| l.iter().filter_map(|c| self.channels.get(c)).map(|c| c.mentions).sum());
        if let Some(gg) = self.guilds.iter_mut().find(|x| x.id == g) {
            gg.mentions = n;
        }
    }

    pub fn select_channel(&mut self, id: Id, cx: &mut Context<Self>) {
        let Some(ch) = self.channels.get(&id) else { return };
        if matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage) {
            self.join_voice(Some(id), cx);
            self.open_channel_chat(id, cx);
            crate::ui::call_view::show(id);
            return;
        }
        self.open_channel_chat(id, cx);
    }

    /// Show a channel's text chat without any side effect on voice: for voice/stage
    /// channels this is the vanilla "Ouvrir la discussion" (voice channel text chat).
    pub fn open_channel_chat(&mut self, id: Id, cx: &mut Context<Self>) {
        crate::commands::state::remember_read_mark(self, id);
        let Some(ch) = self.channels.get_mut(&id) else { return };
        ch.unread = false;
        ch.mentions = 0;
        let guild = ch.guild;
        self.refresh_guild_mentions(guild);
        if guild != self.selected_guild {
            self.selected_guild = guild;
            self.rebuild_sidebar();
            self.rebuild_members();
        }
        self.selected_channel = Some(id);
        self.last_channel.insert(guild, id);
        self.nav_channel_selected(guild);
        self.replying_to = None;
        self.editing = None;
        self.chat_on_select(id);
        self.update_typing_label();
        self.backend.select_channel(id).detach_ok();
        let entry = self.messages.entry(id).or_default();
        self.message_list.reset(entry.msgs.len() + 1);
        crate::perf::channel_opened(id.0);
        if entry.loaded_once {
            crate::perf::channel_messages_ready(id.0, "memory");
        }
        if !entry.loaded_once && !entry.loading {
            entry.loading = true;
            // Rows are laid out on the background executor (docs/perf.md).
            crate::history_ui::load_first_page(self, id, cx);
        }
        cx.notify();
    }

    pub fn load_older(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_channel else { return };
        let Some(entry) = self.messages.get_mut(&id) else { return };
        if entry.loading || entry.reached_start || !entry.loaded_once {
            return;
        }
        let before = entry.msgs.first().map(|m| m.id);
        entry.loading = true;
        let fut = self.backend.load_messages(id, before, 50);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.insert_history(id, res.unwrap_or_default(), true, cx)
            });
        })
        .detach();
    }

    pub(crate) fn resolver(&self) -> Resolver<'_> {
        Resolver { users: &self.users, channels: &self.channels, roles: &self.roles, members: &self.members, soundboard_sounds: &[], me: self.me.id }
    }

    fn resolver_with_sounds<'a>(&'a self, sounds: &'a [crate::model::SoundboardSound]) -> Resolver<'a> {
        Resolver { users: &self.users, channels: &self.channels, roles: &self.roles, members: &self.members, soundboard_sounds: sounds, me: self.me.id }
    }

    pub(crate) fn render_msg(&self, m: &Message, prev: Option<&Message>) -> RenderedMessage {
        let revealed = |i: usize| self.revealed.contains(&(m.id, i));
        let (parsed, mut blocks, mentions_me, preview) =
            richtext::build(&m.content, &self.resolver_with_sounds(&m.soundboard_sounds), self.me.id, &revealed);
        // messageContent_.isFailed_: the whole text in --text-feedback-critical.
        if m.send_state == crate::model::SendState::Failed {
            blocks = richtext::render_blocks(&parsed, &revealed, ui::chat::tk(crate::theme::generated::idx::TEXT_FEEDBACK_CRITICAL));
        }
        RenderedMessage {
            group_start: grouping::starts_group(prev, m, self.utc_offset),
            day_divider: (prev.is_none() || grouping::day_divider(prev, m, self.utc_offset))
                .then(|| format_day(m.timestamp)),
            time_full: format_time_full(m.timestamp),
            time_short: format_time_short(m.timestamp),
            blocks,
            mentions_me,
            preview,
            parsed,
        }
    }

    pub(crate) fn insert_history(&mut self, ch: Id, page: Vec<Message>, older: bool, cx: &mut Context<Self>) {
        if !older {
            crate::perf::channel_messages_ready(ch.0, "load");
        }
        let mut entry = self.messages.remove(&ch).unwrap_or_default();
        entry.loading = false;
        entry.loaded_once = true;
        if page.len() < 50 {
            entry.reached_start = true;
        }
        let n = page.len();
        let mut all = page;
        if older {
            all.extend(std::mem::take(&mut entry.msgs));
        }
        // Re-render only the new rows plus the first old row (its grouping may change).
        let rendered: Vec<RenderedMessage> = if older {
            let mut r: Vec<_> = (0..n.min(all.len()))
                .map(|i| self.render_msg(&all[i], i.checked_sub(1).map(|p| &all[p])))
                .collect();
            let mut old = std::mem::take(&mut entry.rendered);
            if let Some(first) = old.first_mut()
                && n < all.len()
            {
                *first = self.render_msg(&all[n], n.checked_sub(1).map(|p| &all[p]));
            }
            r.extend(old);
            r
        } else {
            (0..all.len())
                .map(|i| self.render_msg(&all[i], i.checked_sub(1).map(|p| &all[p])))
                .collect()
        };
        entry.msgs = all;
        entry.rendered = rendered;
        let len = entry.msgs.len();
        self.messages.insert(ch, entry);
        if self.selected_channel == Some(ch) {
            if older {
                self.message_list.splice(1..1, n);
                if n < len {
                    // The previously-first message may have changed grouping.
                    self.message_list.splice(n + 1..n + 2, 1);
                }
            } else {
                self.message_list.reset(len + 1);
            }
        }
        cx.notify();
    }

    pub(crate) fn append_message(&mut self, m: Message, cx: &mut Context<Self>) {
        // Our own echo replaces its optimistic copy in place (ui::chat::pending).
        if self.take_echo(&m, cx) {
            return;
        }
        let ch = m.channel;
        let selected = self.selected_channel == Some(ch);
        // Blocked authors never ping (the client skips their mentions).
        let mention = m.content.contains(&format!("<@{}>", self.me.id.0)) && !self.nav.is_blocked(m.author);
        if let Some(t) = self.typing.get_mut(&ch) {
            t.retain(|(u, _)| *u != m.author);
        }
        // Detached page on screen: appending would leave a gap; count it instead.
        if self.messages.get(&ch).is_some_and(|e| e.detached) {
            if selected {
                self.chat_note_incoming(false, m.timestamp);
            }
            cx.notify();
            return;
        }
        let Some(mut entry) = self.messages.remove(&ch) else {
            if let Some(c) = self.channels.get_mut(&ch) {
                c.unread = true;
                if mention {
                    c.mentions += 1;
                }
            }
            return;
        };
        // New messages are almost always newer than the tail: skip the scan then.
        if entry.msgs.last().is_some_and(|l| l.id.0 >= m.id.0) && entry.index_of(m.id).is_some() {
            self.messages.insert(ch, entry);
            return;
        }
        let r = self.render_msg(&m, entry.msgs.last());
        let (own, ts) = (m.author == self.me.id, m.timestamp);
        entry.msgs.push(m);
        entry.rendered.push(r);
        let len = entry.msgs.len();
        self.messages.insert(ch, entry);
        if selected {
            self.message_list.splice(len..len, 1);
            self.chat_note_incoming(own, ts);
        } else if let Some(c) = self.channels.get_mut(&ch) {
            c.unread = true;
        }
        self.update_typing_label();
        cx.notify();
    }

    pub(crate) fn replace_message(&mut self, m: Message, cx: &mut Context<Self>) {
        let ch = m.channel;
        let Some(mut entry) = self.messages.remove(&ch) else { return };
        if let Some(ix) = entry.index_of(m.id) {
            entry.rendered[ix] = self.render_msg(&m, ix.checked_sub(1).map(|p| &entry.msgs[p]));
            entry.msgs[ix] = m;
            if self.selected_channel == Some(ch) {
                self.message_list.splice(ix + 1..ix + 2, 1);
            }
        }
        self.messages.insert(ch, entry);
        cx.notify();
    }

    pub fn reveal_spoiler(&mut self, msg: Id, spoiler: usize, cx: &mut Context<Self>) {
        self.revealed.insert((msg, spoiler));
        let Some(ch) = self.selected_channel else { return };
        if let Some(m) = self
            .messages
            .get(&ch)
            .and_then(|e| e.msgs.iter().find(|x| x.id == msg).cloned())
        {
            self.replace_message(m, cx);
        }
    }

    // ------------------------------------------------------------- events

    pub fn handle_event(&mut self, ev: AppEvent, cx: &mut Context<Self>) {
        match ev {
            AppEvent::Ready => {}
            AppEvent::MessageCreate(m) => self.append_message(m, cx),
            AppEvent::MessageUpdate(m) => self.replace_message(m, cx),
            AppEvent::HistoryRefresh { channel, messages } => crate::history_ui::on_refresh(self, channel, messages, cx),
            AppEvent::MessageDelete { channel, id } => {
                if let Some(mut e) = self.messages.remove(&channel) {
                    if let Some(ix) = e.index_of(id) {
                        e.msgs.remove(ix);
                        e.rendered.remove(ix);
                        let selected = self.selected_channel == Some(channel);
                        if selected {
                            self.message_list.splice(ix + 1..ix + 2, 0);
                        }
                        // The next message may now start a group / day (deleted group head).
                        if ix < e.msgs.len() {
                            e.rendered[ix] = self.render_msg(&e.msgs[ix], ix.checked_sub(1).map(|p| &e.msgs[p]));
                            if selected {
                                self.message_list.splice(ix + 1..ix + 2, 1);
                            }
                        }
                    }
                    self.messages.insert(channel, e);
                }
            }
            AppEvent::TypingStart { channel, user, nick } => {
                if user != self.me.id {
                    match nick {
                        Some(n) => { self.typing_nicks.insert((channel, user), n); }
                        None => { self.typing_nicks.remove(&(channel, user)); }
                    }
                    let list = self.typing.entry(channel).or_default();
                    list.retain(|(u, _)| *u != user);
                    list.push((user, Instant::now()));
                    self.update_typing_label();
                }
            }
            AppEvent::PresenceUpdate { user, presence } => {
                if user == self.me.id {
                    self.presence = presence; // status changed from another client
                }
                if user == self.me.id {
                    self.presence = presence; // own status dot in the user panel
                }
                self.people.on_presence(user, presence);
                self.presences.insert(user, presence);
                for ms in self.members.values_mut() {
                    for m in ms.iter_mut().filter(|m| m.user == user) {
                        m.presence = presence;
                    }
                }
                self.rebuild_members();
            }
            AppEvent::GuildMembers { guild, members } => {
                self.members.insert(guild, members);
                if self.selected_guild == Some(guild) {
                    self.rebuild_members();
                    // Voice user rows are sorted by server nick.
                    self.rebuild_sidebar();
                }
            }
            AppEvent::ChannelUnread { channel, unread, mentions } => {
                if let Some(c) = self.channels.get_mut(&channel) {
                    c.unread = unread;
                    c.mentions = mentions;
                }
                self.rebuild_sidebar();
            }
            AppEvent::VoiceFlags { user, flags } => crate::ui::voice_panel::set_flags(user, flags),
            AppEvent::VoiceStateUpdate { channel, user } => {
                // DMs keep their recipient in `voice_users` (avatar): skip them.
                for ch in self.channels.values_mut().filter(|c| c.guild.is_some()) {
                    ch.voice_users.retain(|u| *u != user);
                }
                self.pending_voice.remove(&user);
                if let Some(c) = channel.and_then(|c| self.channels.get_mut(&c)) {
                    c.voice_users.push(user);
                } else if let Some(c) = channel {
                    self.pending_voice.insert(user, c);
                }
                if user == self.me.id {
                    self.voice_channel = channel;
                }
                self.rebuild_sidebar();
            }
            AppEvent::Voice(e) => self.voice.apply(e),
            AppEvent::Users(users) => {
                for u in users {
                    self.users.insert(u.id, u);
                }
            }
            AppEvent::GuildChannels { guild, channels } => {
                self.set_guild_channels(guild, channels);
                if self.selected_guild == Some(guild) {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::GuildRoles { guild, roles } => {
                self.roles.insert(guild, roles);
                if self.selected_guild == Some(guild) {
                    self.rebuild_members();
                }
            }
            AppEvent::GuildUnread { guild, unread, mentions } => {
                if let Some(g) = self.guilds.iter_mut().find(|g| g.id == guild) {
                    g.unread = unread;
                    g.mentions = mentions;
                }
            }
            AppEvent::GuildCreate { guild, channels, roles } => {
                let gid = guild.id;
                match self.guilds.iter_mut().find(|g| g.id == gid) {
                    Some(g) => *g = guild,
                    None => {
                        self.guilds.insert(0, guild);
                        self.guild_list.reset(self.guilds.len() + 5);
                    }
                }
                self.roles.insert(gid, roles);
                self.set_guild_channels(gid, channels);
                if self.selected_guild == Some(gid) {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::GuildDelete(gid) => {
                self.guilds.retain(|g| g.id != gid);
                self.guild_list.reset(self.guilds.len() + 5);
                self.set_guild_channels(gid, Vec::new());
                self.guild_channels.remove(&gid);
                self.members.remove(&gid);
                if self.selected_guild == Some(gid) {
                    self.select_guild(None, cx);
                }
            }
            AppEvent::DmUpsert(ch) => {
                let id = ch.id;
                let keep_unread = self.selected_channel == Some(id);
                let mut ch = ch;
                if keep_unread {
                    ch.unread = false;
                    ch.mentions = 0;
                }
                self.channels.insert(id, ch);
                self.dms.retain(|d| *d != id);
                self.dms.insert(0, id);
                if self.selected_guild.is_none() {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::DmDelete(id) => {
                self.dms.retain(|d| *d != id);
                self.channels.remove(&id);
                if self.selected_guild.is_none() {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::Nav(e) => self.on_nav_event(e, cx),
            AppEvent::SettingsChanged => ui::settings::on_ready(self, cx),
            AppEvent::SoundboardSounds { guild, sounds } => ui::pickers::soundboard::on_sounds(guild, sounds, cx),
            ev @ (AppEvent::MemberListRows { .. } | AppEvent::Activities { .. }) => {
                // Own custom status changed from another client (SESSIONS_REPLACE).
                if let AppEvent::Activities { user, activities } = &ev
                    && *user == self.me.id
                {
                    self.people.custom_status = activities.iter().find(|x| x.kind == crate::presence::ActivityKind::Custom).and_then(|x| x.state.clone());
                }
                ui::members::on_event(self, ev, cx)
            }
            AppEvent::RemoteAuth(u) => crate::auth::on_remote_auth(self, u, cx),
            AppEvent::Resync(r) => crate::commands::on_resync(self, r, cx),
            AppEvent::Parity(e) => crate::commands::on_event(self, e, cx),
            AppEvent::Disconnected(reason) => {
                self.clear_session();
                self.login_error = Some(reason.into());
                self.screen = Screen::Login;
            }
        }
        crate::sounds::watch::watch(self);
        cx.notify();
    }

    /// Replace a guild's channel list (already in display order).
    fn set_guild_channels(&mut self, guild: Id, channels: Vec<crate::model::Channel>) {
        if let Some(old) = self.guild_channels.remove(&guild) {
            for id in old {
                self.channels.remove(&id);
            }
        }
        let mut ids = Vec::with_capacity(channels.len());
        for mut ch in channels {
            // The open channel is read by definition.
            if self.selected_channel == Some(ch.id) {
                ch.unread = false;
                ch.mentions = 0;
            }
            ids.push(ch.id);
            self.channels.insert(ch.id, ch);
        }
        ids.sort_by_key(|id| self.channels[id].position);
        self.guild_channels.insert(guild, ids);
        if let Some(sel) = self.selected_channel
            && !self.channels.contains_key(&sel)
        {
            self.selected_channel = None;
        }
    }

    pub fn delete_message(&mut self, id: Id, cx: &mut Context<Self>) {
        let Some(ch) = self.selected_channel else { return };
        if self.messages.get(&ch).and_then(|e| e.index_of(id).map(|i| &e.msgs[i])).is_some_and(ui::chat::pending::is_local) {
            self.discard_local(ch, id, cx);
            return;
        }
        self.backend.delete_message(ch, id).detach_ok();
        cx.notify();
    }

    fn expire_typing(&mut self, cx: &mut Context<Self>) {
        let before = self.typing.values().map(Vec::len).sum::<usize>();
        for v in self.typing.values_mut() {
            v.retain(|(_, t)| t.elapsed() < Duration::from_secs(10));
        }
        if before != self.typing.values().map(Vec::len).sum::<usize>() {
            self.update_typing_label();
            cx.notify();
        }
    }

    /// Name the client shows for a typer (`getName(guild, channel, user)`):
    /// member nick, then global display name, then username.
    pub fn typing_name(&self, channel: Id, user: Id) -> Option<SharedString> {
        // TypingUsers rejects RelationshipStore.isBlockedOrIgnored.
        if self.nav.is_blocked(user) || self.nav.is_ignored(user) {
            return None;
        }
        self.typing_nicks
            .get(&(channel, user))
            .cloned()
            .or_else(|| self.member_nick(user))
            .or_else(|| self.users.get(&user).map(|u| u.display_name.clone()))
    }

    fn update_typing_label(&mut self) {
        let names: Vec<SharedString> = self
            .selected_channel
            .and_then(|c| self.typing.get(&c).map(|v| (c, v)))
            .map(|(c, v)| v.iter().filter_map(|(u, _)| self.typing_name(c, *u)).collect())
            .unwrap_or_default();
        self.typing_label = match names.as_slice() {
            [] => None,
            [a] => Some(crate::t!("chat.typing_one", a = a)),
            [a, b] => Some(crate::t!("chat.typing_two", a = a, b = b)),
            [a, b, c] => Some(crate::t!("chat.typing_three", a = a, b = b, c = c)),
            _ => Some(crate::t!("chat.typing_many")),
        };
    }

    // ---------------------------------------------------------- composer

    fn on_composer_change(&mut self, cx: &mut Context<Self>) {
        if let Some(ch) = self.selected_channel
            && !self.composer.read(cx).value().is_empty()
        {
            self.backend.trigger_typing(ch).detach_ok();
        }
    }

    pub fn submit_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_none() && self.chat_submit(window, cx) {
            return;
        }
        let Some(ch) = self.selected_channel else { return };
        let text = self.composer.read(cx).value().trim().to_string();
        if text.is_empty() || (self.editing.is_none() && ui::chat::composer::send_held(self, ch)) {
            return;
        }
        self.composer.update(cx, |s, cx| s.set_value("", window, cx));
        let text = if self.prefs.synced.flag("emoticons") { ui::settings::emoticons::convert(&text) } else { text };
        if let Some(id) = self.editing.take() {
            let fut = self.backend.edit_message(ch, id, text);
            cx.spawn(async move |this, cx| {
                if let Ok(m) = fut.await {
                    let _ = this.update(cx, |this, cx| this.replace_message(m, cx));
                }
            })
            .detach();
        } else {
            let reply = self.replying_to.take();
            ui::chat::composer::note_sent(ch);
            let fut = self.backend.send_message(ch, text, reply);
            cx.spawn(async move |this, cx| {
                if let Ok(m) = fut.await {
                    let _ = this.update(cx, |this, cx| this.append_message(m, cx));
                }
            })
            .detach();
        }
        cx.notify();
    }

    /// Up arrow in an empty composer: edit own last message.
    pub fn edit_last(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.composer.read(cx).value().is_empty() {
            return false;
        }
        let Some(ch) = self.selected_channel else { return false };
        let Some(m) = self
            .messages
            .get(&ch)
            .and_then(|e| e.msgs.iter().rev().find(|m| m.author == self.me.id && !ui::chat::pending::is_local(m)))
        else {
            return false;
        };
        let id = m.id;
        self.start_edit(id, window, cx);
        true
    }

    pub fn start_reply(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        self.replying_to = Some(id);
        self.editing = None;
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn join_voice(&mut self, ch: Option<Id>, cx: &mut Context<Self>) {
        self.backend.join_voice(ch).detach_ok();
        crate::voice::on_join_clicked(self, ch);
        cx.notify();
    }

    // -------------------------------------------------------- derived rows

    pub fn toggle_category(&mut self, id: Id, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.rebuild_sidebar();
        cx.notify();
    }

    pub fn rebuild_sidebar(&mut self) {
        ui::sidebar::build_rows(self);
    }

    pub fn rebuild_members(&mut self) {
        if ui::members::sync_server_list(self) {
            return;
        }
        self.member_rows.clear();
        let Some(g) = self.selected_guild else {
            self.member_list.reset(0);
            return;
        };
        let roles = self.roles.get(&g).cloned().unwrap_or_default();
        let Some(members) = self.members.get(&g) else {
            self.member_list.reset(0);
            return;
        };
        let mut hoisted: Vec<&Role> = roles.iter().filter(|r| r.hoist).collect();
        hoisted.sort_by_key(|r| -r.position);
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); hoisted.len() + 2];
        for (i, m) in members.iter().enumerate() {
            let b = if m.presence == Presence::Offline {
                hoisted.len() + 1
            } else {
                hoisted
                    .iter()
                    .position(|r| m.roles.contains(&r.id))
                    .unwrap_or(hoisted.len())
            };
            buckets[b].push(i);
        }
        let users = &self.users;
        for (bi, mut b) in buckets.into_iter().enumerate() {
            if b.is_empty() {
                continue;
            }
            b.sort_by(|x, y| {
                let a = users.get(&members[*x].user).map(|u| u.display_name.to_lowercase());
                let bb = users.get(&members[*y].user).map(|u| u.display_name.to_lowercase());
                a.cmp(&bb)
            });
            let title = if bi < hoisted.len() {
                hoisted[bi].name.to_uppercase()
            } else if bi == hoisted.len() {
                crate::t!("friends.h_online").to_string()
            } else {
                crate::t!("members.h_offline").to_string()
            };
            self.member_rows
                .push(MemberRow::Header(format!("{title} — {}", b.len()).into()));
            self.member_rows.extend(b.into_iter().map(MemberRow::Member));
        }
        self.member_list.reset(self.member_rows.len() + 1); // + Activity header item (ui/members.rs)
    }

    pub fn member_color(&self, guild: Id, m: &Member) -> Option<u32> {
        // Accessibility "Role colors": only "Show in names" (0) tints names.
        if self.prefs.local.pick("roles", 0) != 0 {
            return None;
        }
        self.member_role_color(guild, m)
    }

    /// Highest coloured role of `m`, ignoring the Role colors setting.
    pub fn member_role_color(&self, guild: Id, m: &Member) -> Option<u32> {
        let roles = self.roles.get(&guild)?;
        roles
            .iter()
            .filter(|r| m.roles.contains(&r.id) && r.color.is_some())
            .max_by_key(|r| r.position)
            .and_then(|r| r.color)
    }

    /// Guild owning `channel` (regular channels and cached threads).
    pub fn channel_guild(&self, channel: Id) -> Option<Id> {
        self.channels.get(&channel).or_else(|| self.nav.find_thread(channel)).and_then(|c| c.guild)
    }

    /// Guild nickname of `user` in `guild`, from the member cache.
    pub fn member_nick_in(&self, guild: Option<Id>, user: Id) -> Option<SharedString> {
        let g = guild?;
        crate::member_index::find(self.members.get(&g)?, g, user)?.nick.clone()
    }

    /// Guild nickname of `user` in the selected guild, from the member cache.
    pub fn member_nick(&self, user: Id) -> Option<SharedString> {
        self.member_nick_in(self.selected_guild, user)
    }

    /// `user` as shown in guild `g`: the per-guild avatar, decoration and
    /// nameplate of its member replace the global ones.
    pub fn member_user(&self, g: Option<Id>, user: Id) -> Option<std::borrow::Cow<'_, crate::model::User>> {
        let u = self.users.get(&user)?;
        let m = g.and_then(|g| crate::member_index::find(self.members.get(&g)?, g, user));
        match m {
            Some(m) if m.avatar.is_some() || m.decoration.is_some() || m.nameplate.is_some() => {
                let mut u = u.clone();
                if let Some(a) = &m.avatar {
                    u.avatar = crate::model::ImageRef::Url(a.clone());
                }
                if m.decoration.is_some() {
                    u.decoration = m.decoration.clone();
                }
                if m.nameplate.is_some() {
                    u.nameplate = m.nameplate.clone();
                }
                Some(std::borrow::Cow::Owned(u))
            }
            _ => Some(std::borrow::Cow::Borrowed(u)),
        }
    }

    /// Role dot of Accessibility "Role colors: Show next to names" in `guild`.
    pub fn author_role_dot_in(&self, guild: Option<Id>, user: Id) -> Option<u32> {
        if self.prefs.local.pick("roles", 0) != 1 {
            return None;
        }
        let g = guild?;
        let m = crate::member_index::find(self.members.get(&g)?, g, user)?;
        self.member_role_color(g, m)
    }

    /// Role dot of Accessibility "Role colors: Show next to names".
    pub fn author_role_dot(&self, user: Id) -> Option<u32> {
        self.author_role_dot_in(self.selected_guild, user)
    }

    pub fn author_color_in(&self, guild: Option<Id>, user: Id) -> Option<u32> {
        let g = guild?;
        let m = crate::member_index::find(self.members.get(&g)?, g, user)?;
        self.member_color(g, m)
    }

    pub fn author_color(&self, user: Id) -> Option<u32> {
        self.author_color_in(self.selected_guild, user)
    }

    /// Gradient stops of `user`'s top coloured role in `guild` (when role colours show in names).
    pub fn author_role_gradient_in(&self, guild: Option<Id>, user: Id) -> Option<[u32; 3]> {
        if self.prefs.local.pick("roles", 0) != 0 {
            return None;
        }
        let g = guild?;
        let m = crate::member_index::find(self.members.get(&g)?, g, user)?;
        crate::ui::role_gradient::for_member(self, g, m)
    }

    /// Icon of the highest role of `user` that has one, in `guild`.
    pub fn author_role_icon_in(&self, guild: Option<Id>, user: Id) -> Option<SharedString> {
        let g = guild?;
        let m = crate::member_index::find(self.members.get(&g)?, g, user)?;
        self.roles.get(&g)?.iter().filter(|r| m.roles.contains(&r.id) && r.icon.is_some()).max_by_key(|r| r.position)?.icon.clone()
    }

    /// Icon of the highest role of `user` that has one, in the selected guild.
    pub fn author_role_icon(&self, user: Id) -> Option<SharedString> {
        self.author_role_icon_in(self.selected_guild, user)
    }

    /// Client rule for the "New member" badge (module `HU`, maxDaysOld 7): the
    /// member joined under 7 days ago, did not rejoin, is not a bot, and the
    /// guild itself is older than 7 days.
    pub fn is_new_member(&self, user: Id) -> bool {
        const WEEK_MS: i64 = 7 * 86_400_000;
        let Some(g) = self.selected_guild else { return false };
        let Some(m) = self.members.get(&g).and_then(|ms| crate::member_index::find(ms, g, user)) else { return false };
        let now = chrono::Utc::now().timestamp_millis();
        let guild_ms = (g.0 >> 22) as i64 + 1_420_070_400_000;
        let bot = self.users.get(&user).is_some_and(|u| u.bot);
        !m.rejoined && !bot && now - guild_ms >= WEEK_MS && m.joined_at.is_some_and(|j| (0..=WEEK_MS).contains(&(now - j * 1000)))
    }

    pub fn presence_of(&self, user: Id) -> Presence {
        if user == self.me.id {
            return self.presence;
        }
        let known = self.presences.get(&user).copied();
        self.selected_guild
            .and_then(|g| self.members.get(&g))
            .and_then(|ms| ms.iter().find(|m| m.user == user))
            .map(|m| m.presence)
            .or(known)
            .unwrap_or(Presence::Offline)
    }

    // ------------------------------------------------------ quick switcher

    pub fn open_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        ui::switcher::open(self, window, cx);
    }

    pub fn close_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.switcher = None;
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if ui::settings::on_escape(self, cx) {
            return;
        }
        if crate::shortcuts::on_escape() {
            cx.notify();
            return;
        }
        if ui::chat::app_launcher::close(self, window, cx) {
            return;
        }
        if crate::media::close_viewer(cx) {
        } else if self.people.any_overlay() {
            self.people.close_overlays();
            cx.notify();
        } else if self.escape_header_popouts(cx) {
            // Inbox / threads / pins: Esc closes them and never marks read.
        } else if self.switcher.is_some() {
            self.close_switcher(window, cx);
        } else if self.chat_escape(window, cx) {
        } else if self.editing.take().is_some() {
            self.composer.update(cx, |s, cx| s.set_value("", window, cx));
        } else if self.replying_to.take().is_none() {
            self.message_list.scroll_to_end();
            // Client: Esc = mark channel as read (MARK_CHANNEL_READ).
            self.mark_channel_read(cx);
        }
        cx.notify();
    }
}

trait DetachOk {
    fn detach_ok(self);
}
impl<T: Send + 'static> DetachOk for crate::backend::BoxFuture<T> {
    fn detach_ok(self) {
        // Fire-and-forget; executed on a helper thread so we don't need a
        // context. Real adapters can return immediately-ready futures.
        std::thread::spawn(move || {
            let _ = futures::executor::block_on(self);
        });
    }
}

impl Focusable for PhoenixApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PhoenixApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::perf::on_render(self.selected_channel.map(|c| c.0), true);
        crate::auth::ensure_qr(self, cx);
        crate::theme::sync(&self.prefs, window, cx);
        crate::sounds::set_focused(window.is_window_active());
        crate::media::emoji::sync_motion(self.prefs.synced.flag("anim-emoji"), window.is_window_active());
        crate::ui::settings::effects::sync(self);
        self.chat_sync_style();
        if ui::focus_ring::sync(window, cx) {
            // Controls asked for focus handles last frame: render once more to track them.
            cx.notify();
        }
        let body = match self.screen {
            Screen::Login => ui::login::render(self, window, cx).into_any_element(),
            Screen::Main => div()
                .size_full()
                .flex()
                .flex_col()
                .child(ui::title_bar::render(self, window, cx))
                .child(div().flex_1().min_h_0().flex().flex_row().child(
                    div()
                        .flex_none()
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_row()
                                .child(ui::server_bar::render(self, cx))
                                .child(ui::sidebar::render(self, cx)),
                        )
                        .child(ui::sidebar::panels(self, cx)),
                )
                .child(ui::home::main_pane(self, window, cx)))
                .into_any_element(),
        };
        div()
            .id("phoenix-root")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &QuickSwitcher, w, cx| this.open_switcher(w, cx)))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(ui::settings::open_action))
            .capture_key_down(cx.listener(|this, ev, _, cx| crate::voice::ptt::on_key_down(this, ev, cx)))
            .capture_key_up(cx.listener(|this, ev, _, cx| crate::voice::ptt::on_key_up(this, ev, cx)))
            // Bubble phase: Tab only moves focus when the composer, a picker or a menu did not take it.
            .on_key_down(cx.listener(|_, ev, w, cx| {
                if ui::focus_ring::on_key_down(ev, w, cx) {
                    cx.stop_propagation();
                }
            }))
            .map(|d| crate::shortcuts::handlers(self, d, cx))
            .on_action(cx.listener(|this, _: &ToggleMembers, _, cx| {
                this.show_members = !this.show_members;
                cx.notify();
            }))
            .size_full()
            .relative()
            .font_family(FONT_UI())
            .map(|mut el| {
                // Per-glyph fallback for characters missing from gg sans (see fonts::ui_fallbacks).
                el.text_style().font_fallbacks = Some(crate::fonts::ui_fallbacks());
                el
            })
            .text_color(c(Palette::TEXT_NORMAL))
            .bg(c(Palette::BG_TERTIARY))
            .child({
                // `SHAKE_APP` from the Raging Demon easter egg.
                let (dx, dy) = ui::easter::shake_offset(window);
                div().size_full().relative().left(gpui::px(dx)).top(gpui::px(dy)).child(body)
            })
            .children(ui::settings::render(self, window, cx))
            .children(ui::chat::panels::overlays(self, window, cx))
            .children(ui::home::overlay(self, window, cx))
            .when_some(self.switcher.as_ref(), |d, sw| d.child(ui::switcher::render(self, sw, cx)))
            .children(ui::image_viewer::render(window, cx))
            .children(ui::pickers::render_overlays(self, window, cx))
            .children(ui::chat::app_launcher::render(self, window, cx))
            .children(ui::profile::overlays(self, window, cx))
            .children(ui::components::overlays(self, window, cx))
            .children(ui::voice_panel::overlays(self, window, cx))
            .children(crate::actions::overlays(self, window, cx))
            .children(crate::shortcuts::render_help(self, window, cx))
            .when(self.screen == Screen::Login, |d| d.child(ui::title_bar::login_bar(window, cx)))
            .children(ui::window_chrome::resize_handles(window))
            .children(ui::incoming_call::render(self, cx))
    }
}
