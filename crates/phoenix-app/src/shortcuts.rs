//! Global keyboard shortcuts of the official client.
//!
//! The list and the bindings come from the live 2026 client's keyboard
//! shortcuts modal (Ctrl+/) and its keybind table (`binds:[...]` entries in
//! the local web bundles); see docs/parity/shortcuts.md. Navigation follows the
//! client's behaviour: Alt+Up/Down cycle through the sidebar's channels with
//! wrap-around, Alt+Shift skips to unread channels, Ctrl+Shift+Alt to channels
//! with mentions, Ctrl+Alt+Up/Down cycle through the server rail (Home first).

use std::cell::RefCell;

use gpui::{
    App, ClipboardItem, Context, FontWeight, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Pixels, SharedString, Stateful, Styled, actions, div, prelude::*, px,
};

pub mod message;

use crate::{
    app::{CONTEXT, PhoenixApp},
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui,
};

actions!(
    phoenix_shortcuts,
    [
        ToggleHelp,
        PrevChannel,
        NextChannel,
        PrevUnread,
        NextUnread,
        PrevMention,
        NextMention,
        PrevGuild,
        NextGuild,
        ToggleLastGuild,
        HistoryBack,
        HistoryForward,
        ToggleInbox,
        TogglePins,
        UploadFile,
        FocusSearch,
        ToggleMute,
        ToggleDeafen,
        ToggleEmoji,
        ToggleGif,
        ToggleSticker,
        PageUp,
        PageDown,
        JumpOldestUnread,
        MarkServerRead,
        CreateGuild,
        CreateGroupDm,
        CopyChannelLink,
        GetHelp,
        MarkInboxRead,
        CallDm,
        JumpToCall,
        CreateThread,
        RecordVoiceMessage,
        ToggleSoundboard,
        ZoomIn,
        ZoomOut,
        ZoomReset,
    ]
);

/// The client's zoom steps (`ZOOM_SCALES` in the 2026 web bundle), in percent.
pub const ZOOM_SCALES: [u32; 11] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

/// Next zoom step from `cur` in `dir` (+1 in, -1 out), clamped to the ends. An
/// off-grid value goes to the nearest step in that direction.
pub fn zoom_step(cur: u32, dir: i32) -> u32 {
    if dir > 0 {
        ZOOM_SCALES.iter().copied().find(|&z| z > cur).unwrap_or(ZOOM_SCALES[ZOOM_SCALES.len() - 1])
    } else {
        ZOOM_SCALES.iter().rev().copied().find(|&z| z < cur).unwrap_or(ZOOM_SCALES[0])
    }
}

/// One row of the shortcuts modal: GPUI keystrokes, the client's group and
/// the i18n key of its description.
pub struct Shortcut {
    pub keys: &'static [&'static str],
    pub group: Group,
    pub label: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Navigation,
    Voice,
    Chat,
    Misc,
}

impl Group {
    fn title(self) -> SharedString {
        match self {
            Group::Navigation => crate::t!("shortcuts.group.navigation"),
            Group::Voice => crate::t!("shortcuts.group.voice"),
            Group::Chat => crate::t!("shortcuts.group.chat"),
            Group::Misc => crate::t!("shortcuts.group.misc"),
        }
    }
}

macro_rules! sc {
    ($group:ident, $label:literal, [$($k:literal),+]) => {
        Shortcut { keys: &[$($k),+], group: Group::$group, label: $label }
    };
}

/// Implemented shortcuts, in the order of the client's modal.
pub const SHORTCUTS: &[Shortcut] = &[
    sc!(Navigation, "shortcuts.guilds", ["ctrl-alt-up", "ctrl-alt-down"]),
    sc!(Navigation, "shortcuts.channels", ["alt-up", "alt-down"]),
    sc!(Navigation, "shortcuts.history", ["alt-left", "alt-right"]),
    sc!(Navigation, "shortcuts.unread", ["alt-shift-up", "alt-shift-down"]),
    sc!(Navigation, "shortcuts.mentions", ["ctrl-shift-alt-up", "ctrl-shift-alt-down"]),
    sc!(Navigation, "shortcuts.jump_call", ["ctrl-shift-alt-v"]),
    sc!(Navigation, "shortcuts.last_guild", ["ctrl-alt-right"]),
    sc!(Navigation, "shortcuts.switcher", ["ctrl-k"]),
    sc!(Navigation, "shortcuts.create_guild", ["ctrl-shift-n"]),
    sc!(Voice, "shortcuts.mute", ["ctrl-shift-m"]),
    sc!(Voice, "shortcuts.deafen", ["ctrl-shift-d"]),
    sc!(Voice, "shortcuts.call", ["ctrl-'"]),
    sc!(Voice, "shortcuts.soundboard", ["ctrl-shift-b"]),
    sc!(Chat, "shortcuts.mark_server_read", ["shift-escape"]),
    sc!(Chat, "shortcuts.mark_channel_read", ["escape"]),
    sc!(Chat, "shortcuts.group_dm", ["ctrl-shift-t"]),
    sc!(Chat, "shortcuts.thread", ["ctrl-shift-alt-n"]),
    sc!(Chat, "shortcuts.pins", ["ctrl-p"]),
    sc!(Chat, "shortcuts.inbox", ["ctrl-i"]),
    sc!(Chat, "shortcuts.inbox_read", ["ctrl-shift-e"]),
    sc!(Chat, "shortcuts.members", ["ctrl-u"]),
    sc!(Chat, "shortcuts.emoji", ["ctrl-e"]),
    sc!(Chat, "shortcuts.gif", ["ctrl-g"]),
    sc!(Chat, "shortcuts.sticker", ["ctrl-s"]),
    sc!(Chat, "shortcuts.scroll", ["pageup", "pagedown"]),
    sc!(Chat, "shortcuts.oldest_unread", ["shift-pageup"]),
    sc!(Chat, "shortcuts.voice_message", ["alt-r"]),
    sc!(Chat, "shortcuts.upload", ["ctrl-shift-u"]),
    sc!(Chat, "shortcuts.copy_link", ["ctrl-shift-l"]),
    sc!(Misc, "shortcuts.help", ["ctrl-shift-h"]),
    sc!(Misc, "shortcuts.search", ["ctrl-f"]),
    sc!(Misc, "shortcuts.modal", ["ctrl-/"]),
];

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("ctrl-/", ToggleHelp, ctx),
        KeyBinding::new("ctrl-shift-/", ToggleHelp, ctx),
        KeyBinding::new("alt-up", PrevChannel, ctx),
        KeyBinding::new("alt-down", NextChannel, ctx),
        KeyBinding::new("alt-shift-up", PrevUnread, ctx),
        KeyBinding::new("alt-shift-down", NextUnread, ctx),
        KeyBinding::new("ctrl-shift-alt-up", PrevMention, ctx),
        KeyBinding::new("ctrl-shift-alt-down", NextMention, ctx),
        // Client: ["mod+alt+up","mod+shift+[","ctrl+shift+tab"] and the mirror.
        KeyBinding::new("ctrl-alt-up", PrevGuild, ctx),
        KeyBinding::new("ctrl-shift-[", PrevGuild, ctx),
        KeyBinding::new("ctrl-shift-tab", PrevGuild, ctx),
        KeyBinding::new("ctrl-alt-down", NextGuild, ctx),
        KeyBinding::new("ctrl-shift-]", NextGuild, ctx),
        KeyBinding::new("ctrl-tab", NextGuild, ctx),
        KeyBinding::new("ctrl-alt-right", ToggleLastGuild, ctx),
        KeyBinding::new("alt-left", HistoryBack, ctx),
        KeyBinding::new("alt-right", HistoryForward, ctx),
        KeyBinding::new("ctrl-i", ToggleInbox, ctx),
        KeyBinding::new("ctrl-p", TogglePins, ctx),
        KeyBinding::new("ctrl-shift-u", UploadFile, ctx),
        KeyBinding::new("ctrl-f", FocusSearch, ctx),
        KeyBinding::new("ctrl-shift-f", FocusSearch, ctx),
        KeyBinding::new("ctrl-shift-m", ToggleMute, ctx),
        KeyBinding::new("ctrl-shift-d", ToggleDeafen, ctx),
        KeyBinding::new("ctrl-e", ToggleEmoji, ctx),
        KeyBinding::new("ctrl-g", ToggleGif, ctx),
        KeyBinding::new("ctrl-s", ToggleSticker, ctx),
        KeyBinding::new("pageup", PageUp, ctx),
        KeyBinding::new("pagedown", PageDown, ctx),
        KeyBinding::new("shift-pageup", JumpOldestUnread, ctx),
        KeyBinding::new("shift-escape", MarkServerRead, ctx),
        KeyBinding::new("ctrl-shift-n", CreateGuild, ctx),
        KeyBinding::new("ctrl-shift-t", CreateGroupDm, ctx),
        KeyBinding::new("ctrl-shift-l", CopyChannelLink, ctx),
        KeyBinding::new("ctrl-shift-h", GetHelp, ctx),
        KeyBinding::new("f1", GetHelp, ctx),
        KeyBinding::new("ctrl-shift-e", MarkInboxRead, ctx),
        KeyBinding::new("ctrl-'", CallDm, ctx),
        KeyBinding::new("ctrl-shift-'", CallDm, ctx),
        KeyBinding::new("ctrl-shift-alt-v", JumpToCall, ctx),
        KeyBinding::new("ctrl-shift-alt-n", CreateThread, ctx),
        KeyBinding::new("alt-r", RecordVoiceMessage, ctx),
        KeyBinding::new("ctrl-shift-b", ToggleSoundboard, ctx),
        // Zoom like the desktop client. `=` is unshifted on AZERTY, `+` needs Shift on
        // QWERTY; reset is Ctrl+0, which on AZERTY is the unshifted `à` key.
        KeyBinding::new("ctrl-=", ZoomIn, ctx),
        KeyBinding::new("ctrl-+", ZoomIn, ctx),
        KeyBinding::new("ctrl-shift-=", ZoomIn, ctx),
        KeyBinding::new("ctrl-shift-+", ZoomIn, ctx),
        KeyBinding::new("ctrl--", ZoomOut, ctx),
        KeyBinding::new("ctrl-shift--", ZoomOut, ctx),
        KeyBinding::new("ctrl-0", ZoomReset, ctx),
        KeyBinding::new("ctrl-shift-0", ZoomReset, ctx),
        KeyBinding::new("ctrl-à", ZoomReset, ctx),
    ]);
}

// ------------------------------------------------------------ pure helpers

/// Item `step` places away from `cur` in `list`, wrapping around; the first
/// (or last, going up) item when `cur` is not in the list.
pub fn cycle<T: Copy + PartialEq>(list: &[T], cur: Option<T>, step: i32) -> Option<T> {
    if list.is_empty() {
        return None;
    }
    let n = list.len() as i32;
    let i = match cur.and_then(|c| list.iter().position(|x| *x == c)) {
        Some(i) => (i as i32 + step).rem_euclid(n),
        None if step < 0 => n - 1,
        None => 0,
    };
    Some(list[i as usize])
}

/// Next item after `cur` (in `step` direction, wrapping, never `cur` itself)
/// that satisfies `pred`.
pub fn seek<T: Copy + PartialEq>(list: &[T], cur: Option<T>, step: i32, pred: impl Fn(T) -> bool) -> Option<T> {
    let n = list.len() as i32;
    let start = cur.and_then(|c| list.iter().position(|x| *x == c)).map(|i| i as i32);
    (1..=n)
        .map(|k| match start {
            Some(s) => (s + step.signum() * k).rem_euclid(n),
            None if step < 0 => n - k,
            None => k - 1,
        })
        .map(|i| list[i as usize])
        .find(|x| Some(*x) != cur && pred(*x))
}

/// Server rail order: Home, guilds outside folders, then folder contents.
pub fn rail_order(guilds: &[Id], folders: &[Vec<Id>]) -> Vec<Option<Id>> {
    let in_folder = |g: &Id| folders.iter().any(|f| f.contains(g));
    let mut out = vec![None];
    out.extend(guilds.iter().filter(|g| !in_folder(g)).map(|g| Some(*g)));
    for f in folders {
        out.extend(f.iter().filter(|g| guilds.contains(g)).map(|g| Some(*g)));
    }
    out
}

/// Back/forward stack of visited channels (the client's route history).
#[derive(Default, Debug)]
pub struct History {
    pub past: Vec<Id>,
    pub future: Vec<Id>,
    pub current: Option<Id>,
}

impl History {
    /// Records a navigation to `ch` (no-op when it is already current).
    pub fn visit(&mut self, ch: Id) {
        if self.current == Some(ch) {
            return;
        }
        if let Some(c) = self.current.replace(ch) {
            self.past.push(c);
            if self.past.len() > 50 {
                self.past.remove(0);
            }
        }
        self.future.clear();
    }

    pub fn back(&mut self) -> Option<Id> {
        let to = self.past.pop()?;
        if let Some(c) = self.current.replace(to) {
            self.future.push(c);
        }
        Some(to)
    }

    pub fn forward(&mut self) -> Option<Id> {
        let to = self.future.pop()?;
        if let Some(c) = self.current.replace(to) {
            self.past.push(c);
        }
        Some(to)
    }
}

#[derive(Default)]
struct State {
    help_open: bool,
    keys: Option<gpui::Subscription>,
    history: History,
    last_guild: Option<Id>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Opens the Ctrl+/ modal (offscreen renders).
pub fn open_help() {
    with(|s| s.help_open = true);
}

pub fn help_open() -> bool {
    with(|s| s.help_open)
}

/// Escape closes the modal first. True when consumed.
pub fn on_escape() -> bool {
    with(|s| std::mem::take(&mut s.help_open))
}

// ------------------------------------------------------------ app glue

fn texty(kind: ChannelKind) -> bool {
    !matches!(kind, ChannelKind::Voice | ChannelKind::Stage | ChannelKind::Category)
}

impl PhoenixApp {
    /// Channels Alt+Up/Down walk through: the sidebar's text channels and
    /// threads (plus the joined voice channel), or the DM list at Home.
    fn shortcut_channels(&self) -> Vec<Id> {
        let Some(g) = self.selected_guild else { return self.dms.clone() };
        let ok = |id: &Id| {
            self.channels.get(id).is_some_and(|c| c.guild == Some(g) && (texty(c.kind) || self.voice_channel == Some(*id)))
        };
        let rows: Vec<Id> = self
            .sidebar_rows
            .iter()
            .filter_map(|r| match r {
                crate::app::SidebarRow::Channel(id) | crate::app::SidebarRow::Thread(id) => Some(*id),
                _ => None,
            })
            .filter(ok)
            .collect();
        if rows.is_empty() { self.guild_channels.get(&g).into_iter().flatten().copied().filter(ok).collect() } else { rows }
    }

    fn shortcut_rail(&self) -> Vec<Option<Id>> {
        let guilds: Vec<Id> = self.guilds.iter().map(|g| g.id).collect();
        let folders: Vec<Vec<Id>> = self.nav.folders.iter().map(|f| f.guilds.clone()).collect();
        rail_order(&guilds, &folders)
    }

    /// Every navigable channel across the rail, current guild's order first.
    fn shortcut_all_channels(&self) -> Vec<Id> {
        let mut out = Vec::new();
        for g in self.shortcut_rail() {
            match g {
                _ if g == self.selected_guild => out.extend(self.shortcut_channels()),
                None => out.extend(self.dms.iter().copied()),
                Some(g) => out.extend(
                    self.guild_channels.get(&g).into_iter().flatten().copied().filter(|id| self.channels.get(id).is_some_and(|c| texty(c.kind))),
                ),
            }
        }
        out
    }

    fn goto_channel(&mut self, id: Id, cx: &mut Context<Self>) {
        // Showing the chat only: never joins voice (the client routes, it doesn't connect).
        self.open_channel_chat(id, cx);
        cx.notify();
    }

    fn step_channel(&mut self, step: i32, cx: &mut Context<Self>) {
        if let Some(id) = cycle(&self.shortcut_channels(), self.selected_channel, step) {
            self.goto_channel(id, cx);
        }
    }

    fn step_unread(&mut self, step: i32, mentions_only: bool, cx: &mut Context<Self>) {
        let list = self.shortcut_all_channels();
        let pred = |id: Id| {
            self.channels.get(&id).is_some_and(|c| if mentions_only { c.mentions > 0 } else { (c.unread && !c.muted) || c.mentions > 0 })
        };
        if let Some(id) = seek(&list, self.selected_channel, step, pred) {
            self.goto_channel(id, cx);
        }
    }

    fn step_guild(&mut self, step: i32, cx: &mut Context<Self>) {
        if let Some(g) = cycle(&self.shortcut_rail(), Some(self.selected_guild), step) {
            self.select_guild(g, cx);
        }
    }

    fn toggle_last_guild(&mut self, cx: &mut Context<Self>) {
        let target = match self.selected_guild {
            Some(g) => {
                with(|s| s.last_guild = Some(g));
                None
            }
            None => match with(|s| s.last_guild) {
                Some(g) => Some(g),
                None => return,
            },
        };
        self.select_guild(target, cx);
    }

    fn history_step(&mut self, back: bool, cx: &mut Context<Self>) {
        let to = with(|s| if back { s.history.back() } else { s.history.forward() });
        if let Some(id) = to.filter(|id| self.channels.contains_key(id)) {
            self.goto_channel(id, cx);
        }
    }

    fn page(&mut self, up: bool) {
        let h = self.message_list.viewport_bounds().size.height;
        let d: Pixels = if h > px(0.) { h - px(60.) } else { px(400.) };
        self.message_list.scroll_by(if up { -d } else { d });
    }

    fn copy_channel_link(&self, cx: &mut Context<Self>) {
        let Some(ch) = self.selected_channel else { return };
        let guild = self.channels.get(&ch).and_then(|c| c.guild).map_or("@me".to_string(), |g| g.0.to_string());
        cx.write_to_clipboard(ClipboardItem::new_string(format!("https://discord.com/channels/{guild}/{}", ch.0)));
    }
}

/// Registers every shortcut handler on the root element and records history.
pub fn handlers(app: &PhoenixApp, root: Stateful<gpui::Div>, cx: &mut Context<PhoenixApp>) -> Stateful<gpui::Div> {
    if let Some(ch) = app.selected_channel {
        with(|s| s.history.visit(ch));
    }
    if let Some(g) = app.selected_guild {
        with(|s| s.last_guild = Some(g));
    }
    if with(|s| s.keys.is_none()) {
        let weak = cx.entity().downgrade();
        let sub = cx.intercept_keystrokes(move |ev, window, cx| {
            let Some(app) = weak.upgrade() else { return };
            app.update(cx, |app, cx| {
                if app.shortcut_keystroke(&ev.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            });
        });
        with(|s| s.keys = Some(sub));
    }
    use ui::pickers::{EmojiTarget, Tab};
    root.on_action(cx.listener(|_, _: &ToggleHelp, _, cx| {
        with(|s| s.help_open = !s.help_open);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &PrevChannel, _, cx| a.step_channel(-1, cx)))
    .on_action(cx.listener(|a, _: &NextChannel, _, cx| a.step_channel(1, cx)))
    .on_action(cx.listener(|a, _: &PrevUnread, _, cx| a.step_unread(-1, false, cx)))
    .on_action(cx.listener(|a, _: &NextUnread, _, cx| a.step_unread(1, false, cx)))
    .on_action(cx.listener(|a, _: &PrevMention, _, cx| a.step_unread(-1, true, cx)))
    .on_action(cx.listener(|a, _: &NextMention, _, cx| a.step_unread(1, true, cx)))
    .on_action(cx.listener(|a, _: &PrevGuild, _, cx| a.step_guild(-1, cx)))
    .on_action(cx.listener(|a, _: &NextGuild, _, cx| a.step_guild(1, cx)))
    .on_action(cx.listener(|a, _: &ToggleLastGuild, _, cx| a.toggle_last_guild(cx)))
    .on_action(cx.listener(|a, _: &HistoryBack, _, cx| a.history_step(true, cx)))
    .on_action(cx.listener(|a, _: &HistoryForward, _, cx| a.history_step(false, cx)))
    .on_action(cx.listener(|a, _: &ToggleInbox, _, cx| a.toggle_inbox(cx)))
    .on_action(cx.listener(|a, _: &TogglePins, _, cx| a.toggle_pins(cx)))
    .on_action(cx.listener(|a, _: &UploadFile, _, cx| {
        if a.selected_channel.is_some() {
            a.prompt_attach(cx)
        }
    }))
    .on_action(cx.listener(|a, _: &FocusSearch, w, cx| ui::search::open(a, w, cx)))
    .on_action(cx.listener(|a, _: &ToggleMute, _, cx| {
        a.mic_muted = !a.mic_muted;
        crate::voice::sync_self_state(a);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &ToggleDeafen, _, cx| {
        a.deafened = !a.deafened;
        crate::voice::sync_self_state(a);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &ToggleEmoji, w, cx| ui::pickers::toggle(a, Tab::Emoji, EmojiTarget::Composer, w, cx)))
    .on_action(cx.listener(|a, _: &ToggleGif, w, cx| ui::pickers::toggle(a, Tab::Gif, EmojiTarget::Composer, w, cx)))
    .on_action(cx.listener(|a, _: &ToggleSticker, w, cx| ui::pickers::toggle(a, Tab::Sticker, EmojiTarget::Composer, w, cx)))
    .on_action(cx.listener(|a, _: &PageUp, _, cx| {
        a.page(true);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &PageDown, _, cx| {
        a.page(false);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &JumpOldestUnread, _, cx| {
        if let Some((ch, msg)) = a.chat.unread_marker {
            a.jump_to_message(ch, msg, cx);
        }
    }))
    .on_action(cx.listener(|a, _: &MarkServerRead, _, cx| {
        if let Some(g) = a.selected_guild {
            a.mark_nav_read(ui::nav_menus::Target::Guild(g), cx);
        }
    }))
    .on_action(cx.listener(|a, _: &CreateGuild, w, cx| a.open_join_server(w, cx)))
    .on_action(cx.listener(|a, _: &CreateGroupDm, _, cx| a.open_group_dm_picker(cx)))
    .on_action(cx.listener(|a, _: &CopyChannelLink, _, cx| a.copy_channel_link(cx)))
    .on_action(cx.listener(|a, _: &MarkInboxRead, _, cx| a.mark_inbox_read(cx)))
    .on_action(cx.listener(|a, _: &CallDm, _, cx| a.call_current_dm(cx)))
    .on_action(cx.listener(|a, _: &JumpToCall, _, cx| a.jump_to_call(cx)))
    .on_action(cx.listener(|a, _: &CreateThread, w, cx| a.create_thread_shortcut(w, cx)))
    .on_action(cx.listener(|a, _: &ZoomIn, _, cx| ui::settings::update_local(a, |p| p.zoom = zoom_step(p.zoom, 1), cx)))
    .on_action(cx.listener(|a, _: &ZoomOut, _, cx| ui::settings::update_local(a, |p| p.zoom = zoom_step(p.zoom, -1), cx)))
    .on_action(cx.listener(|a, _: &ZoomReset, _, cx| ui::settings::update_local(a, |p| p.zoom = 100, cx)))
    // UI-only placeholders (the same stub the voice panel's buttons open): nothing is recorded or sent.
    .on_action(cx.listener(|a, _: &RecordVoiceMessage, _, cx| a.stub(&crate::t!("shortcuts.voice_message"), cx)))
    // The live client ignores Ctrl+Shift+B outside a voice connection (checked over CDP, 2026-11-24).
    .on_action(cx.listener(|a, _: &ToggleSoundboard, _, cx| {
        if a.voice_channel.is_some() {
            a.stub(&crate::t!("voice.soundboard"), cx);
        }
    }))
    .on_action(cx.listener(|_, _: &GetHelp, _, cx| cx.open_url("https://support.discord.com")))
}

// ------------------------------------------------------------ Ctrl+/ modal
//
// Layout and values from the live modal (ref-cache shortcuts-modal-default.json,
// `keyboardShortcutsModal_jCEr5v` and friends in the 2026 CSS).

/// Focused-message combos (handled per keystroke in `message.rs`, not keymap bindings).
pub const MESSAGE_SHORTCUTS: &[(&str, &str)] = &[
    ("e", "shortcuts.msg.edit"),
    ("backspace", "shortcuts.msg.delete"),
    ("p", "shortcuts.msg.pin"),
    ("+", "shortcuts.msg.react"),
    ("r", "shortcuts.msg.reply"),
    ("f", "shortcuts.msg.forward"),
    ("s", "shortcuts.msg.speak"),
    ("ctrl-c", "shortcuts.msg.copy"),
    ("alt-enter", "shortcuts.msg.unread"),
    ("escape", "shortcuts.msg.composer"),
];

/// "ctrl-shift-u" -> ["CTRL", "MAJ", "U"] (combo_ text-transform: uppercase; localized caps).
pub fn key_caps(keys: &str) -> Vec<SharedString> {
    if keys == "+" {
        return vec!["+".into()];
    }
    keys.split('-')
        .map(|k| match k {
            "ctrl" => crate::t!("shortcuts.key.ctrl"),
            "shift" => crate::t!("shortcuts.key.shift"),
            "alt" => "ALT".into(),
            "escape" => crate::t!("shortcuts.key.escape"),
            "pageup" => crate::t!("shortcuts.key.pageup"),
            "pagedown" => crate::t!("shortcuts.key.pagedown"),
            "backspace" => crate::t!("shortcuts.key.backspace"),
            "enter" => crate::t!("shortcuts.key.enter"),
            "up" => "↑".into(),
            "down" => "↓".into(),
            "left" => "←".into(),
            "right" => "→".into(),
            k => k.to_uppercase().into(),
        })
        .collect()
}

/// `combo_dMUH0f`: 23px row of `key_dMUH0f` caps, 3px apart.
fn combo(keys: &str) -> gpui::Div {
    let caps = key_caps(keys);
    let n = caps.len();
    div()
        .flex()
        .h(z(23.)) // combo_ height 23px
        .children(caps.into_iter().enumerate().map(move |(i, k)| {
            div()
                .h(z(23.)) // key_ height 23px
                .min_w(z(14.)) // key_ min-width 14px
                .pt(z(3.)) // key_ padding 3px 6px 4px
                .px(z(6.))
                .pb(z(4.))
                .when(i + 1 < n, |d| d.mr(z(3.))) // key_:not(:last-child) margin-inline-end 3px
                .rounded(z(4.)) // key_ border-radius 4px
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE))) // key_ border --border-subtle
                .bg(c(tok(idx::BACKGROUND_MOD_MUTED))) // key_ --background-mod-muted
                .text_size(z(12.)) // combo_ 12px / 12px semibold
                .line_height(z(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))) // key_ color --interactive-text-active
                .flex()
                .justify_center()
                .child(k)
        }))
}

/// `keybindGroup_jCEr5v`: label then the combos (each `keybindKey_` 8px below).
fn keybind_card(label: SharedString, keys: &[&str], width: Pixels) -> impl IntoElement {
    let mut combos = div().flex().flex_wrap().mb(z(8.)); // keybindKey_ margin-bottom 8px
    for (i, k) in keys.iter().enumerate() {
        combos = combos.child(combo(k).when(i > 0, |d| d.ml(z(8.)))); // combo + combo margin-left 8px
    }
    div()
        .w(width) // its grid column's width (minmax(180px, max-content))
        .p(z(12.)) // keybindGroup_ padding 12px
        .rounded(z(8.)) // keybindGroup_ border-radius 8px
        .bg(c(tok(idx::BACKGROUND_MOD_MUTED))) // --background-mod-muted
        .flex()
        .flex_col()
        .gap(z(8.)) // keybindGroup_ gap 8px
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(label)) // body-sm/normal
        .child(combos)
}

/// `keyboardShortcutSection_jCEr5v`: heading, optional description, wrapped cards.
fn section(title: SharedString, desc: Option<SharedString>, rows: Vec<gpui::AnyElement>) -> impl IntoElement {
    div()
        .ml(z(16.)) // margin-inline-start 16px
        .px(z(8.)) // padding 24px 8px
        .py(z(24.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_SUBTLE))) // border-bottom 1px --border-subtle
        .child(
            div()
                .text_size(z(20.)) // heading-lg/semibold 20/24
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(title),
        )
        .when_some(desc, |d, t| {
            d.child(div().mt(z(8.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(t))
        })
        // keyboardShortcutListGroup_: margin-top 8, padding 8px 0, gap 24.
        .child(div().mt(z(8.)).py(z(8.)).flex().flex_col().gap(z(24.)).children(rows))
}

/// CSS grid `repeat(auto-fill, minmax(min, max-content))` with `gap`: the column
/// count comes from the min track size, then each column is as wide as its
/// widest item (never below `min`). Returns the column widths.
pub fn grid_columns(content: &[f32], avail: f32, min: f32, gap: f32) -> Vec<f32> {
    let n = (((avail + gap) / (min + gap)).floor() as usize).max(1);
    let mut cols = vec![min; n.min(content.len().max(1))];
    let mut limits = cols.clone();
    for (i, w) in content.iter().enumerate() {
        let c = &mut limits[i % n];
        *c = c.max(*w);
    }
    // "Maximize tracks": grow every track from its base toward its max-content
    // limit, sharing the free space equally (text wraps in capped tracks).
    let mut free = avail - gap * (cols.len() as f32 - 1.) - cols.iter().sum::<f32>();
    while free > 0.5 {
        let growing: Vec<usize> = (0..cols.len()).filter(|&i| limits[i] - cols[i] > 0.01).collect();
        if growing.is_empty() {
            break;
        }
        let share = free / growing.len() as f32;
        for i in growing {
            let d = share.min(limits[i] - cols[i]);
            cols[i] += d;
            free -= d;
        }
    }
    cols
}

fn text_width(window: &gpui::Window, text: &str, size: Pixels, weight: FontWeight) -> f32 {
    let mut font = gpui::font(crate::theme::FONT_UI());
    font.weight = weight;
    let run = gpui::TextRun { len: text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().layout_line(text, size, &[run], None).width)
}

/// max-content width of a keybind card: max(label, combos) + 2 x 12px padding.
fn card_content_width(window: &gpui::Window, label: &str, keys: &[&str]) -> f32 {
    let label_w = text_width(window, label, z(14.), FontWeight::NORMAL);
    let combos: f32 = keys
        .iter()
        .map(|k| {
            let caps = key_caps(k);
            // key_: padding-inline 6+6, border 1+1, 3px between caps.
            caps.iter().map(|c| text_width(window, c, z(12.), FontWeight::SEMIBOLD).max(2.) + 14.).sum::<f32>() + 3. * (caps.len() as f32 - 1.)
        })
        .sum::<f32>()
        + 8. * (keys.len() as f32 - 1.);
    label_w.max(combos).ceil() + 24.
}

fn grid_rows(window: &gpui::Window, items: Vec<(SharedString, &[&str])>, avail: f32) -> Vec<gpui::AnyElement> {
    let widths: Vec<f32> = items.iter().map(|(l, k)| card_content_width(window, l, k)).collect();
    let cols = grid_columns(&widths, avail, 180., 24.);
    let n = cols.len();
    let mut rows = Vec::new();
    let mut it = items.into_iter().enumerate().peekable();
    while it.peek().is_some() {
        let mut row = div().flex().gap(z(24.)); // grid gap 24px; rows stretch to the tallest card
        for _ in 0..n {
            let Some((i, (label, keys))) = it.next() else { break };
            row = row.child(keybind_card(label, keys, px(cols[i % n])));
        }
        rows.push(row.into_any_element());
    }
    rows
}

/// The modal's card (without backdrop), sized for the window's viewport.
pub fn help_card(window: &gpui::Window) -> gpui::AnyElement {
    let vp = window.viewport_size();
    // keyboardShortcutsModal_: width 80vw, min 800, max 950; max-height 80vh.
    let w = (vp.width * 0.8).clamp(px(800.), px(950.));
    // Grid width: card - list padding-right 16 - section margin 16 - section padding 2x8.
    let avail = f32::from(w) - 48.;
    let mut list = div().id("shortcuts-list").flex_1().min_h_0().overflow_y_scroll().pr(z(16.));
    let msg: Vec<(SharedString, &[&str])> = MESSAGE_SHORTCUTS.iter().map(|(k, l)| (crate::t!(l), std::slice::from_ref(k))).collect();
    list = list.child(section(crate::t!("shortcuts.group.messages"), Some(crate::t!("shortcuts.group.messages_desc")), grid_rows(window, msg, avail)));
    for g in [Group::Navigation, Group::Voice, Group::Chat, Group::Misc] {
        let items = SHORTCUTS.iter().filter(|s| s.group == g).map(|s| (crate::t!(s.label), s.keys)).collect();
        list = list.child(section(g.title(), None, grid_rows(window, items, avail)));
    }
    div()
        .rounded(z(12.)) // root_ border-radius 12px, 1px border, 0 4px 8px rgba(0,0,0,.16)
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.16),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(8.),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(
            div()
                .w(w)
                .max_h(vp.height * 0.8)
                .rounded(z(4.)) // keyboardShortcutsModal_ border-radius 4px
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER))) // --background-base-lower
                .flex()
                .flex_col()
                .child(
                    // modalTitle_: 26/34, --text-strong, padding 20px 40px 0, margin-bottom 5px.
                    div()
                        .flex()
                        .items_center()
                        .pt(z(20.))
                        .px(z(40.))
                        .mb(z(5.))
                        .text_size(z(26.))
                        .line_height(z(34.))
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(crate::t!("shortcuts.title"))
                        .child(combo("ctrl-/").ml(z(20.))),
                )
                .child(
                    // modalSubtitle_: 16/20, padding 0 40px 20px.
                    div()
                        .px(z(40.))
                        .pb(z(20.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(crate::t!("shortcuts.subtitle")),
                )
                .child(list),
        )
        .into_any_element()
}

pub fn render_help(_app: &PhoenixApp, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    if !help_open() {
        return None;
    }
    let entity = cx.entity().downgrade();
    let close: ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        with(|s| s.help_open = false);
        let _ = entity.update(cx, |_, cx| cx.notify());
    });
    Some(ui::modal::frame("shortcuts-modal", close, help_card(window)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_follow_the_client() {
        assert_eq!(zoom_step(100, 1), 110);
        assert_eq!(zoom_step(100, -1), 90);
        assert_eq!(zoom_step(200, 1), 200);
        assert_eq!(zoom_step(50, -1), 50);
        assert_eq!(zoom_step(105, 1), 110);
        assert_eq!(zoom_step(105, -1), 100);
    }

    #[test]
    fn cycle_wraps_both_ways() {
        let l = [1, 2, 3];
        assert_eq!(cycle(&l, Some(3), 1), Some(1));
        assert_eq!(cycle(&l, Some(1), -1), Some(3));
        assert_eq!(cycle(&l, Some(2), 1), Some(3));
        assert_eq!(cycle(&l, None, 1), Some(1));
        assert_eq!(cycle(&l, None, -1), Some(3));
        assert_eq!(cycle::<i32>(&[], None, 1), None);
    }

    #[test]
    fn seek_skips_current_and_wraps() {
        let l = [1, 2, 3, 4];
        let odd = |x: i32| x % 2 == 1;
        assert_eq!(seek(&l, Some(1), 1, odd), Some(3));
        assert_eq!(seek(&l, Some(3), 1, odd), Some(1));
        assert_eq!(seek(&l, Some(1), -1, odd), Some(3));
        assert_eq!(seek(&l, Some(1), 1, |x| x == 1), None);
        assert_eq!(seek(&l, None, 1, |x| x > 1), Some(2));
    }

    #[test]
    fn rail_puts_home_first_and_folders_last() {
        let (a, b, c, d) = (Id(1), Id(2), Id(3), Id(4));
        let order = rail_order(&[a, b, c, d], &[vec![c, Id(99)], vec![a]]);
        assert_eq!(order, vec![None, Some(b), Some(d), Some(c), Some(a)]);
    }

    #[test]
    fn history_back_and_forward() {
        let mut h = History::default();
        h.visit(Id(1));
        h.visit(Id(2));
        h.visit(Id(2));
        h.visit(Id(3));
        assert_eq!(h.back(), Some(Id(2)));
        assert_eq!(h.back(), Some(Id(1)));
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), Some(Id(2)));
        h.visit(Id(5));
        assert_eq!(h.forward(), None);
        assert_eq!(h.back(), Some(Id(2)));
    }

    #[test]
    fn every_listed_key_is_bound() {
        let cx = gpui::TestAppContext::single();
        cx.update(|cx| {
            bind_keys(cx);
            crate::app::bind_keys(cx);
            for s in SHORTCUTS {
                for k in s.keys {
                    let ks = gpui::Keystroke::parse(k).unwrap();
                    let keymap = cx.key_bindings();
                    let keymap = keymap.borrow();
                    assert!(
                        keymap.bindings().any(|b| b.keystrokes().len() == 1 && b.keystrokes()[0].inner().modifiers == ks.modifiers && b.keystrokes()[0].inner().key == ks.key),
                        "{k} not bound"
                    );
                }
            }
        });
    }

    #[test]
    fn grid_columns_follow_auto_fill_max_content() {
        // Live modal: 902px track area -> 4 columns of 180 / 245 / 180 / 180.
        let cols = grid_columns(&[150., 245., 120., 180., 90., 100., 170., 160.], 902., 180., 24.);
        assert_eq!(cols, vec![180., 245., 180., 180.]);
        assert_eq!(grid_columns(&[300.], 902., 180., 24.), vec![300.]);
        // Too wide for the row: tracks share the free space and stay in the box.
        let cols = grid_columns(&[220., 220., 400., 400.], 902., 180., 24.);
        assert!((cols.iter().sum::<f32>() + 72. - 902.).abs() < 1., "{cols:?}");
        assert!(cols.iter().all(|c| *c >= 180. && *c <= 400.), "{cols:?}");
    }

    #[test]
    fn caps_are_localized_upper() {
        let caps = key_caps("ctrl-shift-u");
        assert_eq!(caps.len(), 3);
        assert_eq!(caps[2].as_ref(), "U");
    }
}
