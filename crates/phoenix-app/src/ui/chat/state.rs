//! Chat-view interaction state (inline edit, reply mention toggle, context
//! menu, delete confirm, pins popout, thread panel, uploads, "new messages"
//! bar) and the `PhoenixApp` actions behind it.

use std::{path::PathBuf, sync::Arc, time::Instant};

use gpui::{
    AppContext, ClipboardEntry, ClipboardItem, Context, Entity, Image, ListScrollEvent, PathPromptOptions, Pixels,
    Point, SharedString, Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    backend::Upload,
    model::{Id, Message, Reaction, ThreadRef},
};

/// Discord's non-Nitro upload cap.
pub const MAX_UPLOAD: u64 = 10 * 1024 * 1024;
pub const MAX_FILES: usize = 10;
/// Quick reactions offered in the context menu / toolbar.
pub const QUICK_REACTIONS: [&str; 4] = ["👍", "❤️", "😂", "🔥"];

pub struct EditState {
    pub id: Id,
    pub channel: Id,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

#[derive(Clone, Copy)]
pub struct MenuState {
    pub channel: Id,
    pub msg: Id,
    pub pos: Point<Pixels>,
}

pub struct PinsState {
    pub channel: Id,
    pub loading: bool,
    pub msgs: Vec<Message>,
}

pub struct ThreadPanel {
    pub thread: ThreadRef,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

#[derive(Clone)]
pub struct PendingFile {
    pub name: SharedString,
    pub bytes: Arc<Vec<u8>>,
    /// Decoded preview for images.
    pub image: Option<Arc<Image>>,
    pub spoiler: bool,
    /// Blurred preview shown while `spoiler` is on (`spoiler::blurred_image`).
    pub blurred: Option<Arc<Image>>,
    /// Alt text set in the attachment modal (sent as `attachments[].description`).
    pub description: String,
}

pub struct Sending {
    pub key: u64,
    pub channel: Id,
    pub label: SharedString,
    pub size: u64,
    pub started: Instant,
}

/// Who receives the emoji chosen in the emoji picker (owned by another
/// module). The picker reads [`ChatState::emoji_target`] to know it must
/// open, then calls [`PhoenixApp::emoji_picked`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiTarget {
    Reaction { channel: Id, msg: Id },
    Composer,
}

/// How long a jumped-to message stays highlighted.
const FLASH_HOLD: std::time::Duration = std::time::Duration::from_millis(2600); // flash_overlay timeline
/// Older pages fetched at most when jumping to a message that isn't loaded.
const JUMP_MAX_PAGES: u32 = 10;

pub struct ChatState {
    /// `theme::style_generation()` the cached rich text was built under.
    pub style_gen: u32,
    pub edit: Option<EditState>,
    /// Reply pings the author (the "@ ON" toggle in the reply bar).
    pub reply_mention: bool,
    pub menu: Option<MenuState>,
    pub confirm_delete: Option<(Id, Id)>,
    pub pins: Option<PinsState>,
    pub thread: Option<ThreadPanel>,
    pub uploads: Vec<PendingFile>,
    pub sending: Vec<Sending>,
    /// New messages received while scrolled up: (count, first timestamp).
    pub new_messages: Option<(u32, i64)>,
    pub at_bottom: bool,
    /// First unread message after "Mark Unread" (channel, msg).
    pub unread_marker: Option<(Id, Id)>,
    pub emoji_target: Option<EmojiTarget>,
    pub error: Option<SharedString>,
    /// Shift held: the hover toolbar shows Delete (and skips the confirm).
    pub shift_held: bool,
    /// Message highlighted after a jump (reply click, pins, search): flash_
    /// `[data-flash=true]` background + 2px --text-brand overlay bar.
    pub flash: Option<Id>,
    /// Blocked / ignored message runs expanded with "Montrer", keyed by the run's first message.
    pub hidden_open: std::collections::HashSet<Id>,
    next_key: u64,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            style_gen: 0,
            edit: None,
            reply_mention: true,
            menu: None,
            confirm_delete: None,
            pins: None,
            thread: None,
            uploads: Vec::new(),
            sending: Vec::new(),
            new_messages: None,
            at_bottom: true,
            unread_marker: None,
            emoji_target: None,
            error: None,
            shift_held: false,
            flash: None,
            hidden_open: Default::default(),
            next_key: 0,
        }
    }
}

/// Reaction route name: unicode as is, custom `<a:name:id>` -> `name`.
pub fn reaction_name(emoji: &str) -> String {
    let t = emoji.trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = t.split(':').collect();
    match parts.as_slice() {
        [_, name, _id] if emoji.starts_with('<') => (*name).to_string(),
        [name, _id] if emoji.starts_with('<') => (*name).to_string(),
        _ if emoji.starts_with(':') && emoji.ends_with(':') && emoji.len() > 2 => emoji[1..emoji.len() - 1].to_string(),
        _ => emoji.to_string(),
    }
}

/// Apply a reaction toggle to a message in place. Returns false when nothing changed.
pub fn apply_reaction(m: &mut Message, emoji: &str, emoji_id: Option<Id>, add: bool) -> bool {
    let same = |r: &Reaction| match (r.emoji_id, emoji_id) {
        (Some(a), Some(b)) => a == b,
        _ => r.emoji.as_ref() == emoji,
    };
    match m.reactions.iter().position(same) {
        Some(ix) => {
            let r = &mut m.reactions[ix];
            if r.me == add {
                return false;
            }
            r.me = add;
            if add {
                r.count += 1;
            } else {
                r.count = r.count.saturating_sub(1);
                if r.count == 0 {
                    m.reactions.remove(ix);
                }
            }
            true
        }
        None if add => {
            m.reactions.push(Reaction { emoji: emoji.to_string().into(), count: 1, me: true, emoji_id });
            true
        }
        None => false,
    }
}

fn is_image_name(name: &str) -> Option<gpui::ImageFormat> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => gpui::ImageFormat::Png,
        "jpg" | "jpeg" => gpui::ImageFormat::Jpeg,
        "gif" => gpui::ImageFormat::Gif,
        "webp" => gpui::ImageFormat::Webp,
        "bmp" => gpui::ImageFormat::Bmp,
        _ => return None,
    })
}

pub fn pending_file(name: String, bytes: Vec<u8>) -> PendingFile {
    let image = is_image_name(&name).map(|f| Arc::new(Image::from_bytes(f, bytes.clone())));
    PendingFile { name: name.into(), bytes: Arc::new(bytes), image, spoiler: false, blurred: None, description: String::new() }
}

impl PhoenixApp {
    /// Cached rich text holds colours resolved for the theme it was built
    /// under, and the list caches row heights: rebuild both after a theme,
    /// zoom or font size change.
    pub fn chat_sync_style(&mut self) {
        let g = crate::theme::style_generation();
        if self.chat.style_gen == g {
            return;
        }
        self.chat.style_gen = g;
        let ids: Vec<Id> = self.messages.keys().copied().collect();
        for ch in ids {
            let Some(mut entry) = self.messages.remove(&ch) else { continue };
            entry.rendered = (0..entry.msgs.len())
                .map(|i| self.render_msg(&entry.msgs[i], i.checked_sub(1).map(|p| &entry.msgs[p])))
                .collect();
            self.messages.insert(ch, entry);
        }
        self.message_list.remeasure();
    }

    fn find_msg(&self, channel: Id, id: Id) -> Option<&Message> {
        self.messages.get(&channel)?.msgs.iter().find(|m| m.id == id)
    }

    /// Hook from `select_channel`.
    pub fn chat_on_select(&mut self, channel: Id) {
        // Leaving / reopening a channel shows the present again, like the client.
        self.messages.retain(|_, e| !e.detached);
        self.chat.edit = None;
        self.chat.menu = None;
        self.chat.confirm_delete = None;
        self.chat.uploads.clear();
        self.chat.new_messages = None;
        self.chat.at_bottom = true;
        self.chat.emoji_target = None;
        self.chat.error = None;
        if self.chat.pins.as_ref().is_some_and(|p| p.channel != channel) {
            self.chat.pins = None;
        }
        if self.chat.unread_marker.is_some_and(|(c, _)| c != channel) {
            self.chat.unread_marker = None;
        }
        self.chat.thread = None;
    }

    /// Hook from the message list scroll handler.
    pub fn chat_on_scroll(&mut self, ev: &ListScrollEvent, cx: &mut Context<Self>) {
        let bottom = ev.visible_range.end >= ev.count || ev.is_following_tail;
        if bottom != self.chat.at_bottom {
            self.chat.at_bottom = bottom;
            if bottom {
                self.chat.new_messages = None;
            }
            cx.notify();
        }
    }

    /// Hook from `append_message` for the open channel.
    pub fn chat_note_incoming(&mut self, own: bool, ts: i64) {
        if own {
            self.message_list.scroll_to_end();
            self.chat.new_messages = None;
        } else if !self.chat.at_bottom {
            let e = self.chat.new_messages.get_or_insert((0, ts));
            e.0 += 1;
        }
    }

    pub fn jump_to_present(&mut self, cx: &mut Context<Self>) {
        if self.leave_detached(cx) {
            self.chat.at_bottom = true;
            self.chat.new_messages = None;
            cx.notify();
            return;
        }
        self.message_list.scroll_to_end();
        self.chat.at_bottom = true;
        self.chat.new_messages = None;
        cx.notify();
    }

    pub fn mark_channel_read(&mut self, cx: &mut Context<Self>) {
        self.chat.new_messages = None;
        self.chat.unread_marker = None;
        if let Some(ch) = self.selected_channel {
            self.backend.select_channel(ch).detach_ok_chat();
        }
        cx.notify();
    }

    /// Escape: closes the innermost chat overlay. True when consumed.
    pub fn chat_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let closed = self.chat.menu.take().is_some()
            || self.chat.confirm_delete.take().is_some()
            || self.chat.emoji_target.take().is_some();
        if !closed {
            if self.chat.edit.is_some() {
                self.cancel_edit(window, cx);
            } else if self.chat.pins.take().is_none() && self.replying_to.take().is_none() {
                return false;
            }
        }
        cx.notify();
        true
    }

    // ------------------------------------------------------------ editing

    pub fn start_edit(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        let Some(m) = self.find_msg(channel, id) else { return };
        if m.author != self.me.id {
            return;
        }
        let content = m.content.clone();
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |s, cx| {
            s.set_value(content, window, cx);
            s.focus(window, cx);
        });
        let sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.save_edit(window, cx);
            }
        });
        self.chat.menu = None;
        self.chat.edit = Some(EditState { id, channel, input, _sub: sub });
        if let Some(ix) = self.messages.get(&channel).and_then(|e| e.msgs.iter().position(|m| m.id == id)) {
            self.message_list.splice(ix + 1..ix + 2, 1);
        }
        cx.notify();
    }

    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(e) = self.chat.edit.take() {
            self.refresh_row(e.channel, e.id);
        }
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn save_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.chat.edit.as_ref() else { return };
        let (id, channel) = (e.id, e.channel);
        let text = e.input.read(cx).value().trim().to_string();
        if text.is_empty() {
            // Discord: saving an empty edit asks to delete the message.
            self.chat.edit = None;
            self.chat.confirm_delete = Some((channel, id));
            cx.notify();
            return;
        }
        let unchanged = self.find_msg(channel, id).is_some_and(|m| m.content.as_ref() == text);
        if !unchanged {
            // Optimistic local update, replaced by the server echo.
            if let Some(mut m) = self.find_msg(channel, id).cloned() {
                m.content = text.clone().into();
                m.edited = true;
                self.replace_message(m, cx);
            }
            let fut = self.backend.edit_message(channel, id, text);
            cx.spawn(async move |this, cx| {
                let res = fut.await;
                let _ = this.update(cx, |this, cx| match res {
                    Ok(m) => this.replace_message(m, cx),
                    Err(e) => this.chat.error = Some(e.0.into()),
                });
            })
            .detach();
        }
        self.cancel_edit(window, cx);
    }

    pub(crate) fn refresh_row(&mut self, channel: Id, id: Id) {
        if self.selected_channel == Some(channel)
            && let Some(ix) = self.messages.get(&channel).and_then(|e| e.msgs.iter().position(|m| m.id == id))
        {
            self.message_list.splice(ix + 1..ix + 2, 1);
        }
    }

    // ----------------------------------------------------------- reactions

    /// Click on an existing reaction pill: toggle our own reaction.
    pub fn toggle_reaction(&mut self, channel: Id, msg: Id, r: &Reaction, cx: &mut Context<Self>) {
        self.set_reaction_local(channel, msg, r.emoji.clone(), r.emoji_id, !r.me, cx);
    }

    /// Add a reaction (quick reactions, emoji picker). No-op if already reacted.
    pub fn react_with(&mut self, channel: Id, msg: Id, emoji: SharedString, emoji_id: Option<Id>, cx: &mut Context<Self>) {
        self.set_reaction_local(channel, msg, emoji, emoji_id, true, cx);
    }

    fn set_reaction_local(
        &mut self,
        channel: Id,
        msg: Id,
        emoji: SharedString,
        emoji_id: Option<Id>,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(mut m) = self.find_msg(channel, msg).cloned() else { return };
        if !apply_reaction(&mut m, &emoji, emoji_id, add) {
            return;
        }
        self.replace_message(m, cx);
        let r = Reaction { emoji, count: 0, me: add, emoji_id };
        let fut = self.backend.set_reaction(channel, msg, r, add);
        cx.spawn(async move |this, cx| {
            if let Err(e) = fut.await {
                let _ = this.update(cx, |this, cx| {
                    this.chat.error = Some(e.0.into());
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Emoji picker hook: open the picker for this target.
    pub fn open_emoji_picker(&mut self, target: EmojiTarget, cx: &mut Context<Self>) {
        self.chat.menu = None;
        self.chat.emoji_target = Some(target);
        cx.notify();
    }

    /// Emoji picker hook: an emoji was chosen (`emoji` is the unicode char or
    /// `<:name:id>` for custom emoji).
    pub fn emoji_picked(&mut self, emoji: SharedString, emoji_id: Option<Id>, window: &mut Window, cx: &mut Context<Self>) {
        match self.chat.emoji_target.take() {
            Some(EmojiTarget::Reaction { channel, msg }) => self.react_with(channel, msg, emoji, emoji_id, cx),
            Some(EmojiTarget::Composer) | None => {
                self.composer.update(cx, |s, cx| {
                    s.insert(emoji.to_string(), window, cx);
                    s.focus(window, cx);
                });
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------ delete

    /// Shift-click skips the confirmation (like Discord).
    pub fn request_delete(&mut self, channel: Id, id: Id, skip_confirm: bool, cx: &mut Context<Self>) {
        self.chat.menu = None;
        if skip_confirm {
            self.backend.delete_message(channel, id).detach_ok_chat();
        } else {
            self.chat.confirm_delete = Some((channel, id));
        }
        cx.notify();
    }

    pub fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if let Some((channel, id)) = self.chat.confirm_delete.take() {
            self.backend.delete_message(channel, id).detach_ok_chat();
        }
        cx.notify();
    }

    // ------------------------------------------------------ menu actions

    pub fn open_message_menu(&mut self, channel: Id, msg: Id, pos: Point<Pixels>, cx: &mut Context<Self>) {
        self.chat.menu = Some(MenuState { channel, msg, pos });
        super::panels::arm_menu_media();
        cx.notify();
    }

    pub fn copy_message_text(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        if let Some(m) = self.find_msg(channel, id) {
            cx.write_to_clipboard(ClipboardItem::new_string(m.content.to_string()));
        }
    }

    pub fn copy_message_link(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        let guild = self.channels.get(&channel).and_then(|c| c.guild).map(|g| g.0.to_string());
        let url = format!(
            "https://discord.com/channels/{}/{}/{}",
            guild.unwrap_or_else(|| "@me".into()),
            channel.0,
            id.0
        );
        cx.write_to_clipboard(ClipboardItem::new_string(url));
    }

    pub fn toggle_pin(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        let Some(mut m) = self.find_msg(channel, id).cloned() else { return };
        m.pinned = !m.pinned;
        let pinned = m.pinned;
        if let Some(p) = self.chat.pins.as_mut().filter(|p| p.channel == channel) {
            p.msgs.retain(|x| x.id != id);
            if pinned {
                p.msgs.insert(0, m.clone());
            }
        }
        self.replace_message(m, cx);
        self.backend.set_pinned(channel, id, pinned).detach_ok_chat();
    }

    pub fn unpin_from_popout(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        if let Some(p) = self.chat.pins.as_mut() {
            p.msgs.retain(|x| x.id != id);
        }
        if let Some(mut m) = self.find_msg(channel, id).cloned() {
            m.pinned = false;
            self.replace_message(m, cx);
        }
        self.backend.set_pinned(channel, id, false).detach_ok_chat();
        cx.notify();
    }

    pub fn mark_unread_from(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        let Some(entry) = self.messages.get(&channel) else { return };
        let Some(ix) = entry.msgs.iter().position(|m| m.id == id) else { return };
        // The read marker goes on the message just before (or id - 1).
        let last_read = ix.checked_sub(1).map(|p| entry.msgs[p].id).unwrap_or(Id(id.0.saturating_sub(1)));
        let old = self.chat.unread_marker.replace((channel, id));
        if let Some((c, m)) = old {
            self.refresh_row(c, m);
        }
        self.refresh_row(channel, id);
        if let Some(c) = self.channels.get_mut(&channel) {
            c.unread = true;
        }
        self.backend.mark_unread(channel, last_read).detach_ok_chat();
        self.rebuild_sidebar();
        cx.notify();
    }

    // ------------------------------------------------------------ pins

    pub fn toggle_pins(&mut self, cx: &mut Context<Self>) {
        if self.chat.pins.take().is_some() {
            cx.notify();
            return;
        }
        let Some(channel) = self.selected_channel else { return };
        self.chat.pins = Some(PinsState { channel, loading: true, msgs: Vec::new() });
        let fut = self.backend.load_pins(channel);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = this.chat.pins.as_mut().filter(|p| p.channel == channel) {
                    p.loading = false;
                    match res {
                        Ok(msgs) => p.msgs = msgs,
                        Err(e) => this.chat.error = Some(e.0.into()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn jump_to_message(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        self.chat.pins = None;
        if self.selected_channel != Some(channel) {
            self.select_channel(channel, cx);
        }
        self.jump_reveal(channel, id, JUMP_MAX_PAGES, cx);
        cx.notify();
    }

    /// Scroll to `id` and flash it; when it isn't loaded yet, page older history
    /// (read-only GETs) until it shows up, the channel start, or `pages` runs out.
    fn jump_reveal(&mut self, channel: Id, id: Id, pages: u32, cx: &mut Context<Self>) {
        let Some(e) = self.messages.get_mut(&channel) else { return };
        if let Some(ix) = e.msgs.iter().position(|m| m.id == id) {
            self.message_list.scroll_to_reveal_item(ix + 1);
            self.flash_message(id, cx);
            return;
        }
        if e.loading || !e.loaded_once {
            return;
        }
        // Snowflakes are time-ordered: a reply target newer than our oldest row was deleted.
        if e.msgs.first().is_some_and(|m| m.id.0 < id.0) {
            return;
        }
        // Client: jump with ?around= into a detached page (older pages stay pageable).
        if pages == JUMP_MAX_PAGES {
            self.jump_around(channel, id, cx);
            return;
        }
        if pages == 0 || e.reached_start {
            return;
        }
        let before = e.msgs.first().map(|m| m.id);
        e.loading = true;
        let fut = self.backend.load_messages(channel, before, 50);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.insert_history(channel, res.unwrap_or_default(), true, cx);
                if this.selected_channel == Some(channel) {
                    this.jump_reveal(channel, id, pages - 1, cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn flash_message(&mut self, id: Id, cx: &mut Context<Self>) {
        self.chat.flash = Some(id);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH_HOLD).await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.flash == Some(id) {
                    this.chat.flash = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    // ----------------------------------------------------------- threads

    pub fn open_thread(&mut self, thread: ThreadRef, window: &mut Window, cx: &mut Context<Self>) {
        let id = thread.id;
        let input = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.send_in_thread(window, cx);
            }
        });
        input.update(cx, |s, cx| s.focus(window, cx));
        self.chat.thread = Some(ThreadPanel { thread, input, _sub: sub });
        let loaded = self.messages.get(&id).is_some_and(|e| e.loaded_once);
        if !loaded {
            self.messages.entry(id).or_default().loading = true;
            let fut = self.backend.load_messages(id, None, 50);
            cx.spawn(async move |this, cx| {
                let res = fut.await;
                let _ = this.update(cx, |this, cx| this.insert_history(id, res.unwrap_or_default(), false, cx));
            })
            .detach();
        }
        cx.notify();
    }

    pub fn close_thread(&mut self, cx: &mut Context<Self>) {
        self.chat.thread = None;
        cx.notify();
    }

    fn send_in_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.chat.thread.as_ref() else { return };
        let id = t.thread.id;
        let text = t.input.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        t.input.update(cx, |s, cx| s.set_value("", window, cx));
        let fut = self.backend.send_message(id, text, None);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| match res {
                Ok(m) => this.append_message(m, cx),
                Err(e) => {
                    this.chat.error = Some(e.0.into());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    // ------------------------------------------------------------ uploads

    pub fn prompt_attach(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(crate::t!("chat.upload")),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update(cx, |this, cx| this.add_paths(paths, cx));
            }
        })
        .detach();
    }

    /// Drag & drop / file picker: read the files off the UI thread.
    pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move {
            paths
                .into_iter()
                .filter(|p| p.is_file())
                .map(|p| {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
                    let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if len > MAX_UPLOAD {
                        return Err(crate::t!("chat.too_large", name = name).to_string());
                    }
                    std::fs::read(&p).map(|b| (name.clone(), b)).map_err(|e| format!("{name}: {e}"))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = task.await;
            let _ = this.update(cx, |this, cx| {
                for f in files {
                    match f {
                        Ok((name, bytes)) if this.chat.uploads.len() < MAX_FILES => {
                            this.chat.uploads.push(pending_file(name, bytes))
                        }
                        Ok(_) => this.chat.error = Some(crate::t!("chat.max_files", n = MAX_FILES)),
                        Err(e) => this.chat.error = Some(e.into()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Ctrl+V: attach a pasted image. True when the clipboard held an image.
    pub fn paste_image(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else { return false };
        let mut took = false;
        for e in item.entries() {
            match e {
                ClipboardEntry::Image(img) => {
                    let ext = match img.format {
                        gpui::ImageFormat::Jpeg => "jpg",
                        gpui::ImageFormat::Gif => "gif",
                        gpui::ImageFormat::Webp => "webp",
                        gpui::ImageFormat::Bmp => "bmp",
                        _ => "png",
                    };
                    if self.chat.uploads.len() < MAX_FILES {
                        self.chat.uploads.push(pending_file(format!("image.{ext}"), img.bytes.clone()));
                    }
                    took = true;
                }
                ClipboardEntry::ExternalPaths(p) => {
                    self.add_paths(p.paths().to_vec(), cx);
                    took = true;
                }
                _ => {}
            }
        }
        if took {
            cx.notify();
        }
        took
    }

    pub fn remove_upload(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.chat.uploads.len() {
            self.chat.uploads.remove(ix);
        }
        cx.notify();
    }

    pub fn toggle_upload_spoiler(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(u) = self.chat.uploads.get_mut(ix) {
            u.spoiler = !u.spoiler;
            if u.spoiler && u.blurred.is_none() && u.image.is_some() {
                let bytes = u.bytes.clone();
                let key = u.bytes.clone();
                let task = cx.background_spawn(async move { super::spoiler::blurred_image(&bytes) });
                cx.spawn(async move |this, cx| {
                    let img = task.await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(u) = this.chat.uploads.get_mut(ix).filter(|u| Arc::ptr_eq(&u.bytes, &key)) {
                            u.blurred = img;
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------- send

    /// Hook at the top of `submit_composer`: sends text + reply (with the
    /// mention toggle) + attachments. True when handled.
    pub fn chat_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(ch) = self.selected_channel else { return true };
        if super::slash::submit(self, window, cx) {
            return true;
        }
        let text = self.composer.read(cx).value().trim().to_string();
        if text.is_empty() && self.chat.uploads.is_empty() {
            return true;
        }
        if text.len() > 2000 {
            self.chat.error = Some(crate::t!("chat.too_long"));
            cx.notify();
            return true;
        }
        self.composer.update(cx, |s, cx| s.set_value("", window, cx));
        let reply = self.replying_to.take().map(|r| (r, self.chat.reply_mention));
        self.chat.reply_mention = true;
        let files: Vec<Upload> = std::mem::take(&mut self.chat.uploads)
            .into_iter()
            .map(|f| Upload {
                filename: if f.spoiler { format!("SPOILER_{}", f.name) } else { f.name.to_string() },
                bytes: Arc::unwrap_or_clone(f.bytes),
                spoiler: f.spoiler,
                description: Some(f.description).filter(|d| !d.trim().is_empty()),
            })
            .collect();
        let key = self.chat.next_key;
        self.chat.next_key += 1;
        if !files.is_empty() {
            let size = files.iter().map(|f| f.bytes.len() as u64).sum();
            let label: SharedString = if files.len() == 1 {
                files[0].filename.clone().into()
            } else {
                crate::t!("chat.n_files", n = files.len())
            };
            self.chat.sending.push(Sending { key, channel: ch, label, size, started: Instant::now() });
        }
        let fut = self.backend.send_message_ex(ch, text, reply, files);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.chat.sending.retain(|s| s.key != key);
                match res {
                    Ok(m) => this.append_message(m, cx),
                    Err(e) => this.chat.error = Some(e.0.into()),
                }
                cx.notify();
            });
        })
        .detach();
        self.message_list.scroll_to_end();
        cx.notify();
        true
    }
}

trait DetachChat {
    fn detach_ok_chat(self);
}
impl<T: Send + 'static> DetachChat for crate::backend::BoxFuture<T> {
    fn detach_ok_chat(self) {
        std::thread::spawn(move || {
            let _ = futures::executor::block_on(self);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_names() {
        assert_eq!(reaction_name("👍"), "👍");
        assert_eq!(reaction_name("<:blob:123>"), "blob");
        assert_eq!(reaction_name("<a:party:42>"), "party");
        assert_eq!(reaction_name(":old:"), "old");
    }

    #[test]
    fn reaction_toggle_counts() {
        let mut m = Message::default();
        assert!(apply_reaction(&mut m, "👍", None, true));
        assert_eq!((m.reactions[0].count, m.reactions[0].me), (1, true));
        assert!(!apply_reaction(&mut m, "👍", None, true));
        m.reactions[0].count = 3;
        assert!(apply_reaction(&mut m, "👍", None, false));
        assert_eq!((m.reactions[0].count, m.reactions[0].me), (2, false));
        m.reactions[0].count = 1;
        m.reactions[0].me = true;
        assert!(apply_reaction(&mut m, "👍", None, false));
        assert!(m.reactions.is_empty());
    }

    #[test]
    fn custom_reaction_matches_by_id() {
        let mut m = Message::default();
        m.reactions.push(Reaction { emoji: "<:a:7>".into(), count: 2, me: false, emoji_id: Some(Id(7)) });
        assert!(apply_reaction(&mut m, "<:renamed:7>", Some(Id(7)), true));
        assert_eq!(m.reactions.len(), 1);
        assert_eq!(m.reactions[0].count, 3);
    }
}
