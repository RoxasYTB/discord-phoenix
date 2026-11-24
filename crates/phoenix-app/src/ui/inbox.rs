//! Header popouts and small dialogs of the parity work:
//! - Inbox (title-bar button): Unreads, Mentions (with filters), Saved,
//!   Reminders, Scheduled — the five tabs of the live client.
//! - Threads browser and pinned messages of the open channel (header buttons).
//! - Moderation entries of the user menu (Timeout / Kick / Ban, shown only
//!   with the matching permission) and their dialogs.
//! - crate::t!("groupdm.select") group DM picker (DM list "+").
//! - Guild folder / order sync after a drag in the guild rail.
//!
//! Layout and styles come from the live client dumps (see docs/parity/popouts.md).

use crate::theme::z;
use gpui::{AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Window, div, prelude::*};
use gpui_component::input::{Input, InputState};

use crate::{
    app::PhoenixApp,
    commands::{
        FolderSync, ModAction, ParityReply, ParityRequest,
        state::{GroupDmPicker, InboxTab, ModDialog, ModKind},
    },
    model::{Id, Message},
    theme::{
        Palette, c,
        generated::{Shadow, idx, shadow},
        m, space, tok,
    },
    ui::{avatar, context_menu::MenuItem, home::data::RelationshipKind},
};

pub const KICK_MEMBERS: u64 = 1 << 1;
pub const BAN_MEMBERS: u64 = 1 << 2;
pub const ADMINISTRATOR: u64 = 1 << 3;
pub const MODERATE_MEMBERS: u64 = 1 << 40;

/// Timeout choices of the official dialog (label, seconds).
pub const TIMEOUTS: [(&str, u64); 6] =
    [("mod.timeout.60s", 60), ("mod.timeout.5m", 300), ("mod.timeout.10m", 600), ("mod.timeout.1h", 3600), ("mod.timeout.1d", 86_400), ("mod.timeout.1w", 604_800)];

/// "Delete Message History" choices of the ban dialog.
pub const BAN_HISTORY: [(&str, u32); 7] = [
    ("mod.ban.none", 0),
    ("mod.ban.1h", 3600),
    ("mod.ban.6h", 21_600),
    ("mod.ban.12h", 43_200),
    ("mod.ban.24h", 86_400),
    ("mod.ban.3d", 259_200),
    ("mod.ban.7d", 604_800),
];

pub fn has_perm(perms: u64, bit: u64) -> bool {
    perms & ADMINISTRATOR != 0 || perms & bit != 0
}

// ------------------------------------------------------------ app actions

impl PhoenixApp {
    pub fn toggle_inbox(&mut self, cx: &mut Context<Self>) {
        let i = &mut self.parity.inbox;
        i.open = !i.open;
        self.parity.threads_open = false;
        if self.parity.inbox.open {
            self.load_inbox_tab(true, cx);
        }
        cx.notify();
    }

    pub fn toggle_threads_popout(&mut self, cx: &mut Context<Self>) {
        self.parity.threads_open = !self.parity.threads_open;
        self.parity.inbox.open = false;
        if self.parity.threads_open
            && let Some(ch) = self.selected_channel
        {
            self.parity_request(ParityRequest::ArchivedThreads { channel: ch }, cx, move |a, r, _| {
                if let Ok(ParityReply::ForumPosts { posts, .. }) = r {
                    a.parity.archived_threads.insert(ch, posts);
                }
            });
        }
        cx.notify();
    }

    /// Inbox, threads browser and pins: the header popouts. Closes them all;
    /// true when one was open (Esc, a click outside, a channel switch).
    pub fn close_header_popouts(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.header_popout_open();
        self.parity.inbox.open = false;
        self.parity.inbox.filter_menu = false;
        self.parity.threads_open = false;
        self.chat.pins = None;
        if open {
            cx.notify();
        }
        open
    }

    pub fn header_popout_open(&self) -> bool {
        self.parity.inbox.open || self.parity.threads_open || self.chat.pins.is_some()
    }

    /// Esc: the Mentions filter menu first, then the popout itself.
    pub fn escape_header_popouts(&mut self, cx: &mut Context<Self>) -> bool {
        if self.parity.inbox.open && self.parity.inbox.filter_menu {
            self.parity.inbox.filter_menu = false;
            cx.notify();
            return true;
        }
        self.close_header_popouts(cx)
    }

    /// Inbox > Unreads: the client's channel list (`ui::inbox_unreads`), built
    /// from the read states the backend holds; no request, no ack.
    fn load_unreads(&mut self, cx: &mut Context<Self>) {
        use crate::ui::inbox_unreads::Kind;
        let order = crate::ui::server_bar::flat_guild_order(self);
        let entries = self.backend.inbox_unreads(&order);
        let i = &mut self.parity.inbox;
        i.unreads_gen += 1;
        i.unreads_loading = false;
        i.unreads = entries
            .into_iter()
            .map(|entry| crate::commands::state::UnreadCard { loaded: entry.kind != Kind::Messages, entry, msgs: Vec::new(), more: false, error: None })
            .collect();
        self.load_next_unread(cx);
        cx.notify();
    }

    /// Loads the next card: `GET messages?around=<ack>&limit=30` (read-only).
    /// A card with nothing to show is dropped; the client acks it
    /// (`ACK_INBOX_CHANNEL_NO_MESSAGES`), Phoenix does not.
    pub(crate) fn load_next_unread(&mut self, cx: &mut Context<Self>) {
        use crate::ui::inbox_unreads::{FETCH_LIMIT, Kind, shown_messages};
        let i = &mut self.parity.inbox;
        if i.unreads_loading {
            return;
        }
        let Some(card) = i.unreads.iter().find(|c| !c.loaded) else { return };
        let entry = card.entry.clone();
        i.unreads_loading = true;
        let generation = i.unreads_gen;
        let fut = self.backend.load_messages_at(entry.channel, crate::backend::HistoryAt::Around(entry.oldest_read), FETCH_LIMIT);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |a, cx| {
                let i = &mut a.parity.inbox;
                if i.unreads_gen != generation {
                    return;
                }
                i.unreads_loading = false;
                if let Some(card) = i.unreads.iter_mut().find(|c| c.entry.channel == entry.channel) {
                    card.loaded = true;
                    match res {
                        Ok(msgs) => {
                            (card.msgs, card.more) = shown_messages(&entry, msgs);
                        }
                        Err(e) => card.error = Some(e.0.into()),
                    }
                }
                i.unreads.retain(|c| !c.loaded || c.entry.kind != Kind::Messages || !c.msgs.is_empty() || c.error.is_some());
                cx.notify();
            });
        })
        .detach();
    }

    /// The check button of a card (and Ctrl+Shift+E on the top one): ack it.
    pub fn mark_unread_card_read(&mut self, channel: Id, cx: &mut Context<Self>) {
        self.parity.inbox.unreads.retain(|c| c.entry.channel != channel);
        self.mark_nav_read(crate::ui::nav_menus::Target::Channel(channel), cx);
    }

    fn set_inbox_tab(&mut self, tab: InboxTab, cx: &mut Context<Self>) {
        self.parity.inbox.tab = tab;
        self.parity.inbox.filter_menu = false;
        self.load_inbox_tab(true, cx);
        cx.notify();
    }

    fn load_inbox_tab(&mut self, fresh: bool, cx: &mut Context<Self>) {
        let i = &mut self.parity.inbox;
        if i.loading {
            return;
        }
        let req = match i.tab {
            InboxTab::Mentions => {
                if fresh {
                    i.mentions.clear();
                    i.reached_end = false;
                }
                if i.reached_end {
                    return;
                }
                ParityRequest::Mentions {
                    before: i.mentions.last().map(|m| m.id),
                    guild: if i.this_server { self.selected_guild } else { None },
                    everyone: i.everyone,
                    roles: i.roles,
                }
            }
            InboxTab::Saved => ParityRequest::SavedMessages,
            InboxTab::Reminders => return self.load_reminders_tab(cx),
            InboxTab::Scheduled => ParityRequest::ScheduledMessages,
            InboxTab::Unreads => return self.load_unreads(cx),
        };
        i.loading = true;
        let tab = i.tab;
        let me = self.me.id;
        self.parity_request(req, cx, move |a, res, cx| {
            let i = &mut a.parity.inbox;
            i.loading = false;
            match res {
                Ok(ParityReply::Messages(msgs)) => match tab {
                    InboxTab::Mentions => {
                        i.reached_end = msgs.len() < 25;
                        // The endpoint applies the filters (`everyone`, `roles`,
                        // `guild_id`), like the client's fetchRecentMentions.
                        i.mentions.extend(msgs);
                    }
                    InboxTab::Scheduled => {
                        i.scheduled = msgs.into_iter().map(|m| Message { author: me, ..m }).collect();
                        i.scheduled.sort_by_key(|m| m.timestamp);
                    }
                    _ => i.saved = msgs,
                },
                Ok(_) => {}
                Err(e) => a.parity_toast(crate::t!("inbox.load_failed", e = e.0).to_string(), cx),
            }
        });
    }

    fn dismiss_mention(&mut self, id: Id, cx: &mut Context<Self>) {
        self.parity.inbox.mentions.retain(|m| m.id != id);
        self.parity_request(ParityRequest::DismissMention(id), cx, |_, _, _| {});
        cx.notify();
    }

    fn unsave(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        self.parity.inbox.saved.retain(|m| m.id != id);
        self.parity_request(ParityRequest::SetSaved { channel, message: id, saved: false }, cx, |_, _, _| {});
        cx.notify();
    }

    /// "Save Message" from the message menu (chat agent can call this).
    pub fn save_message(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        self.parity_request(ParityRequest::SetSaved { channel, message: id, saved: true }, cx, |a, r, cx| {
            if r.is_ok() {
                a.parity_toast(crate::t!("inbox.saved_toast"), cx);
            }
        });
    }

    fn jump_from_inbox(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        self.parity.inbox.open = false;
        if self.selected_channel != Some(channel) {
            self.select_channel(channel, cx);
        }
        self.jump_to_message(channel, id, cx);
    }

    pub fn open_moderation(&mut self, guild: Id, user: Id, kind: ModKind, cx: &mut Context<Self>) {
        self.parity.moderation = Some(ModDialog { guild, user, kind });
        cx.notify();
    }

    fn confirm_moderation(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.parity.moderation.take() else { return };
        let action = match d.kind {
            ModKind::Timeout(s) => ModAction::Timeout(s),
            ModKind::Kick => ModAction::Kick,
            ModKind::Ban(s) => ModAction::Ban(s),
        };
        let name = self.users.get(&d.user).map(|u| u.display_name.to_string()).unwrap_or_default();
        self.parity_request(ParityRequest::Moderate { guild: d.guild, user: d.user, action }, cx, move |a, r, cx| match r {
            Ok(_) => a.parity_toast(
                match d.kind {
                    ModKind::Timeout(0) => crate::t!("mod.timeout_removed", name = name).to_string(),
                    ModKind::Timeout(_) => crate::t!("mod.timed_out", name = name).to_string(),
                    ModKind::Kick => crate::t!("mod.kicked", name = name).to_string(),
                    ModKind::Ban(_) => crate::t!("mod.banned", name = name).to_string(),
                },
                cx,
            ),
            Err(e) => a.parity_toast(crate::t!("common.failed", e = e.0).to_string(), cx),
        });
        cx.notify();
    }

    pub fn open_group_dm_picker(&mut self, cx: &mut Context<Self>) {
        self.parity.group_dm = Some(GroupDmPicker { search: None, picked: Vec::new(), sending: false });
        cx.notify();
    }

    fn create_group_dm(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.parity.group_dm.as_mut() else { return };
        if p.picked.is_empty() || p.sending {
            return;
        }
        if p.picked.len() == 1 {
            let u = p.picked[0];
            self.parity.group_dm = None;
            self.message_user(u, cx);
            return;
        }
        p.sending = true;
        let users = p.picked.clone();
        self.parity_request(ParityRequest::CreateGroupDm(users), cx, |a, r, cx| {
            a.parity.group_dm = None;
            match r {
                Ok(ParityReply::Channel(ch)) if a.channels.contains_key(&ch) => a.select_channel(ch, cx),
                // The DM arrives through CHANNEL_CREATE; select it then.
                Ok(ParityReply::Channel(ch)) => {
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_millis(800)).await;
                        let _ = this.update(cx, |a, cx| a.select_channel(ch, cx));
                    })
                    .detach();
                }
                Ok(_) => {}
                Err(e) => a.parity_toast(crate::t!("inbox.group_failed", e = e.0).to_string(), cx),
            }
        });
        cx.notify();
    }

    /// Persist the rail order + folders (call after a drag-reorder).
    pub fn sync_guild_folders(&mut self, cx: &mut Context<Self>) {
        let folders = folder_sync(self);
        self.parity_request(ParityRequest::GuildFolders(folders), cx, |a, r, cx| {
            if let Err(e) = r {
                a.parity_toast(crate::t!("inbox.order_failed", e = e.0).to_string(), cx);
            }
        });
    }
}

/// Rail order as `guild_folders`: guilds outside folders first (one lone
/// entry each), then the folders; a lone entry that got several guilds by a
/// drag is split again.
pub fn folder_sync(app: &PhoenixApp) -> Vec<FolderSync> {
    let in_folders: std::collections::HashSet<Id> = app.nav.folders.iter().flat_map(|f| f.guilds.iter().copied()).collect();
    let lone = |g: Id| FolderSync { id: None, name: None, color: None, guilds: vec![g] };
    let mut out: Vec<FolderSync> = app.guilds.iter().filter(|g| !in_folders.contains(&g.id)).map(|g| lone(g.id)).collect();
    for f in &app.nav.folders {
        match f.id {
            Some(id) => out.push(FolderSync {
                id: Some(id),
                name: f.name.as_ref().map(|n| n.to_string()),
                color: f.color,
                guilds: f.guilds.clone(),
            }),
            None => out.extend(f.guilds.iter().copied().map(lone)),
        }
    }
    out
}

/// Moderation rows for the user context menu (empty without permissions).
pub fn moderation_items(app: &PhoenixApp, user: Id, guild: Option<Id>, entity: &Entity<PhoenixApp>) -> Vec<MenuItem> {
    // Vanilla member menu (vanilla-menu-member-* dumps): separator, "Open in
    // Mod View" (Moderate Members, also on yourself), then Timeout, Kick, Ban
    // for other members, all red.
    let Some(g) = guild else { return Vec::new() };
    let perms = app.backend.guild_permissions(g);
    let name = app.users.get(&user).map(|u| u.name.to_string()).unwrap_or_else(|| crate::t!("common.user").to_string());
    let mut items = Vec::new();
    let item = |label: String, kind: ModKind| {
        let e = entity.clone();
        MenuItem::danger(label, None, move |_, _, cx: &mut App| e.update(cx, |a, cx| a.open_moderation(g, user, kind, cx)))
    };
    if has_perm(perms, MODERATE_MEMBERS) {
        let e = entity.clone();
        items.push(MenuItem::action(crate::t!("menu.mod_view"), None, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.open_mod_view(g, user, cx))
        }));
    }
    if user != app.me.id {
        if has_perm(perms, MODERATE_MEMBERS) {
            items.push(item(crate::t!("mod.timeout_user", name = name).to_string(), ModKind::Timeout(TIMEOUTS[0].1)));
        }
        if has_perm(perms, KICK_MEMBERS) {
            items.push(item(crate::t!("mod.kick_user", name = name).to_string(), ModKind::Kick));
        }
        if has_perm(perms, BAN_MEMBERS) {
            items.push(item(crate::t!("mod.ban_user", name = name).to_string(), ModKind::Ban(0)));
        }
    }
    if !items.is_empty() {
        items.insert(0, MenuItem::Separator);
    }
    items
}

// ------------------------------------------------------------- rendering
//
// Rebuilt from the live client dumps (inbox-unreads / -mentions / -saved /
// -reminders / -scheduled, threads, threads-lrer, pins, pins-list-hover).
// Every value names its CSS source (hashed class prefix + property or token).

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

/// A generated `--shadow-*` token for the active theme.
fn shadow_tok(t: &[&[Shadow]; 4]) -> Vec<gpui::BoxShadow> {
    let ix = match crate::theme::active() {
        crate::ui::settings::AppTheme::Dark => 0,
        crate::ui::settings::AppTheme::Light => 1,
        crate::ui::settings::AppTheme::Ash => 2,
        crate::ui::settings::AppTheme::Onyx => 3,
    };
    t[ix]
        .iter()
        .map(|s| gpui::BoxShadow {
            color: gpui::rgba(s.color).into(),
            offset: gpui::point(z(s.x), z(s.y)),
            blur_radius: z(s.blur),
            spread_radius: z(s.spread),
            inset: s.inset,
        })
        .collect()
}

/// `box-shadow: var(--shadow-low)` (cards, e.g. Active Now `itemCard_`).
pub fn low_shadow() -> Vec<gpui::BoxShadow> {
    shadow_tok(&shadow::SHADOW_LOW)
}

/// `box-shadow: var(--shadow-medium)`.
pub fn shadow_medium() -> Vec<gpui::BoxShadow> {
    shadow_tok(&shadow::SHADOW_MEDIUM)
}

/// `box-shadow: var(--shadow-high)` (toasts `wrapper_O41c2-`).
pub fn shadow_high() -> Vec<gpui::BoxShadow> {
    shadow_tok(&shadow::SHADOW_HIGH)
}

/// `box-shadow: var(--shadow-border), var(--shadow-high)` (popout containers).
pub fn popout_shadow() -> Vec<gpui::BoxShadow> {
    let mut v = shadow_tok(&shadow::SHADOW_BORDER);
    v.extend(shadow_tok(&shadow::SHADOW_HIGH));
    v
}

fn named(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).size(z(size)).flex_none().text_color(color)
}

/// `button_F7qvhY sm_ secondary_` icon-only: 32px, radius 8, 1px
/// --control-secondary-border-default, --control-secondary-background-default,
/// 16px glyph --control-secondary-text-default.
fn sm_icon_button(id: impl Into<gpui::ElementId>, glyph: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
        .bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)).border_color(tk(idx::CONTROL_SECONDARY_BORDER_HOVER)))
        // secondary_F7qvhY:active
        .active(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE)).border_color(tk(idx::CONTROL_SECONDARY_BORDER_ACTIVE)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(named(glyph, 16., tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
}

/// `button_F7qvhY sm_` with text: 32px, padding 3px 11px (+1px border), radius 8,
/// label body-sm/medium 14/18.
pub fn sm_text_button(id: impl Into<gpui::ElementId>, label: SharedString, primary: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(32.))
        .flex_none()
        .px(z(12.))
        .rounded(z(8.))
        .border_1()
        .when(primary, |d| {
            // primary_F7qvhY: --control-primary-background-default / -border- / -text-default.
            d.bg(tk(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT))
                .border_color(tk(idx::CONTROL_PRIMARY_BORDER_DEFAULT))
                .text_color(tk(idx::CONTROL_PRIMARY_TEXT_DEFAULT))
                .hover(|s| s.bg(tk(idx::CONTROL_PRIMARY_BACKGROUND_HOVER)).border_color(tk(idx::CONTROL_PRIMARY_BORDER_HOVER)).text_color(tk(idx::CONTROL_PRIMARY_TEXT_HOVER)))
                .active(|s| s.bg(tk(idx::CONTROL_PRIMARY_BACKGROUND_ACTIVE)).border_color(tk(idx::CONTROL_PRIMARY_BORDER_ACTIVE)).text_color(tk(idx::CONTROL_PRIMARY_TEXT_ACTIVE)))
        })
        .when(!primary, |d| {
            d.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                .border_color(tk(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
                .text_color(tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
                .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)).border_color(tk(idx::CONTROL_SECONDARY_BORDER_HOVER)).text_color(tk(idx::CONTROL_SECONDARY_TEXT_HOVER)))
                .active(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE)).border_color(tk(idx::CONTROL_SECONDARY_BORDER_ACTIVE)).text_color(tk(idx::CONTROL_SECONDARY_TEXT_ACTIVE)))
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(label)
}

const TABS: [(InboxTab, &str, &str); 5] = [
    (InboxTab::Unreads, "inbox-tab-unreads", "inbox.tab_unreads"),
    (InboxTab::Mentions, "inbox-tab-mentions", "inbox.tab_mentions"),
    (InboxTab::Saved, "inbox-tab-saved", "inbox.tab_saved"),
    (InboxTab::Reminders, "inbox-tab-reminders", "inbox.tab_reminders"),
    (InboxTab::Scheduled, "inbox-tab-scheduled", "inbox.tab_scheduled"),
];

/// `tabBar_LSviKk top_6zZYhF`: margin-top 16, border-bottom 1px --border-subtle;
/// `tab_LSviKk item_6zZYhF brand_`: flex 1, padding 0 8 16, margin-start --space-xs,
/// body-sm/semibold 14px, --text-subtle; selected --text-brand with the 2px
/// `:after` bar (radius 2 2 0 0); hover --text-strong.
fn tab_bar(sel: InboxTab, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut bar = div()
        .mt(z(16.))
        // headerTabs_LSviKk margin 0 -16px inside the 20px header padding.
        .mx(z(-16.))
        .flex()
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE));
    for (i, (t, id, key)) in TABS.into_iter().enumerate() {
        let on = t == sel;
        bar = bar.child(
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .flex_1()
                .min_w_0()
                .when(i > 0, |d| d.ml(z(space::xs())))
                .relative()
                .px(z(8.))
                .pb(z(16.))
                .flex()
                .justify_center()
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(if on { idx::TEXT_BRAND } else { idx::TEXT_SUBTLE }))
                .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
                .on_click(cx.listener(move |a, _, _, cx| a.set_inbox_tab(t, cx)))
                .child(div().min_w_0().whitespace_nowrap().overflow_hidden().text_ellipsis().child(crate::t!(key)))
                .when(on, |d| {
                    d.child(div().absolute().left_0().right_0().bottom(z(-1.)).h(z(2.)).rounded_t(z(2.)).bg(tk(idx::TEXT_BRAND)))
                }),
        );
    }
    bar.into_any_element()
}

/// `container_FgrH4y` (Unreads / Mentions empty): min-height 360, padding 0 16,
/// centred; 80px `icon_` circle (--background-base-low, 22px padding, glyph
/// --text-muted), heading-xl/semibold 24/32 --text-strong mb 8, body-xs 12/16
/// --text-default with the `protip_` (14px medium, --text-feedback-positive).
fn caught_up(tip: Option<SharedString>) -> AnyElement {
    div()
        .min_h(z(360.))
        .px(z(16.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(
            div()
                .mb(z(16.))
                .size(z(80.))
                .rounded_full()
                .bg(tk(idx::BACKGROUND_BASE_LOW))
                .flex()
                .items_center()
                .justify_center()
                .child(named("inbox", 36., tk(idx::TEXT_MUTED))),
        )
        .child(
            div()
                .mb(z(8.))
                .text_size(z(24.))
                .line_height(z(32.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_STRONG))
                .child(crate::t!("inbox.caught_up")),
        )
        .when_some(tip, |d, tip| {
            d.child(
                div()
                    .text_center()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(
                        gpui::StyledText::new(format!("{} {tip}", crate::t!("inbox.protip"))).with_highlights([(
                            0..crate::t!("inbox.protip").len(),
                            gpui::HighlightStyle { color: Some(tk(idx::TEXT_FEEDBACK_POSITIVE)), font_weight: Some(FontWeight::MEDIUM), ..Default::default() },
                        )]),
                    ),
            )
        })
        .into_any_element()
}

/// Body text with one bold span (`body-sm/medium` 14/18 --text-subtle, `strong` 600).
fn bold_middle(a: SharedString, b: SharedString, c2: SharedString) -> gpui::StyledText {
    let start = a.len();
    let end = start + b.len();
    gpui::StyledText::new(format!("{a}{b}{c2}"))
        .with_highlights([(start..end, gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })])
}

/// A row of the demo menus (`menuRow_jMz1Ee`): padding 6 8, gap 8, radius 4,
/// 16px glyph, body-sm/medium label; highlighted: --background-mod-normal.
fn demo_menu_row(glyph: &str, label: SharedString, hi: bool) -> gpui::Div {
    div()
        .px(z(8.))
        .py(z(6.))
        .rounded(z(4.))
        .flex()
        .items_center()
        .gap(z(8.))
        .when(hi, |d| d.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
        .child(named(glyph, 16., tk(if hi { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT })))
        .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(label))
}

/// `menu_jMz1Ee`: --background-surface-highest, 1px --border-subtle, radius 8,
/// padding 4, --shadow-high.
fn demo_menu(rows: Vec<gpui::Div>) -> gpui::Div {
    div()
        .p(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .shadow(shadow_tok(&shadow::SHADOW_HIGH))
        .flex()
        .flex_col()
        .children(rows)
}

/// The Wumpus demo of `messages_jMz1Ee`: --background-base-lowest, 1px
/// --border-normal, radius 12, margin 16 0, padding 8 0; cozy rows padding
/// 2 30 2 80, 40px avatar at 20px, 16/22 text; the menu sits at inset-start 300,
/// margin-top -7 under the first line.
fn wumpus_demo(hi_second: bool) -> AnyElement {
    let line = |t: SharedString| {
        div().pl(z(80.)).pr(z(30.)).py(z(2.)).text_size(z(16.)).line_height(z(22.)).text_color(tk(idx::TEXT_DEFAULT)).whitespace_nowrap().overflow_hidden().child(t)
    };
    let time = crate::app::format_time_full(chrono::Local::now().timestamp());
    div()
        .relative()
        .my(z(16.))
        .py(z(8.))
        .w_full()
        .rounded(z(12.))
        .border_1()
        .border_color(tk(idx::BORDER_NORMAL))
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .child(
            div()
                .relative()
                .pl(z(80.))
                .pr(z(30.))
                .py(z(2.))
                .child(div().absolute().left(z(20.)).top(z(4.)).child(avatar(None, 40.)))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .child(div().mr(z(4.)).text_size(z(16.)).line_height(z(22.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child("Wumpus"))
                        // timestamp_2i8Bfm: --chat-text-muted 12px medium, margin-start 4.
                        .child(div().ml(z(4.)).text_size(z(12.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::CHAT_TEXT_MUTED)).child(time)),
                )
                .child(div().text_size(z(16.)).line_height(z(22.)).text_color(tk(idx::TEXT_DEFAULT)).whitespace_nowrap().overflow_hidden().child(crate::t!("inbox.demo_1"))),
        )
        .child(line(crate::t!("inbox.demo_2")))
        .child(line(crate::t!("inbox.demo_3")))
        .child(
            div().absolute().left(z(300.)).top(z(49.)).child(demo_menu(vec![
                demo_menu_row("inbox/bookmark", crate::t!("inbox.demo_save"), !hi_second),
                demo_menu_row("inbox/clock", crate::t!("inbox.demo_remind"), hi_second),
            ])),
        )
        .into_any_element()
}

/// `container_jMz1Ee` / `container_XzStbz` upsells (Saved, Reminders, Scheduled):
/// height 480, padding 16, centred; 140×93 image mb 24; heading-xl/semibold
/// 24/32 --text-default mb 8; body-sm/medium 14/18 --text-subtle mb 16; demo.
fn upsell(glyph: &str, title: SharedString, body: gpui::StyledText, demo: AnyElement) -> AnyElement {
    div()
        .h(z(480.))
        .p(z(16.))
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .child(
            // The illustration is a CDN asset: drawn as the matching icon in its 140×93 box.
            div().mb(z(24.)).w(z(140.)).h(z(93.)).flex().items_center().justify_center().child(named(glyph, 72., tk(idx::TEXT_BRAND))),
        )
        .child(
            div()
                .mb(z(8.))
                .text_center()
                .text_size(z(24.))
                .line_height(z(32.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(title),
        )
        .child(
            div()
                .mb(z(16.))
                .w_full()
                .text_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_SUBTLE))
                .child(body),
        )
        .child(demo)
        .into_any_element()
}

/// Scheduled demo (`demo_XzStbz` margin-top 16): attach menu (margin 0 0 -5 12)
/// over a `chatInput_` (padding 8 12, gap 8, radius 8, --background-base-lowest,
/// 1px --border-subtle) with the 24px `plusButton_` (--background-mod-strong).
fn scheduled_demo() -> AnyElement {
    div()
        .mt(z(16.))
        .w_full()
        .flex()
        .flex_col()
        .child(div().ml(z(12.)).mb(z(-5.)).flex().child(demo_menu(vec![
            demo_menu_row("chat/upload", crate::t!("inbox.demo_upload"), false),
            demo_menu_row("inbox/schedule", crate::t!("inbox.demo_schedule"), true),
        ])))
        .child(
            div()
                .px(z(12.))
                .py(z(8.))
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(div().size(z(24.)).rounded(z(8.)).bg(tk(idx::BACKGROUND_MOD_STRONG)).flex().items_center().justify_center().child(named("plus", 16., tk(idx::INTERACTIVE_TEXT_ACTIVE))))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("inbox.demo_placeholder"))),
        )
        .into_any_element()
}

fn channel_label(app: &PhoenixApp, ch: Id) -> (SharedString, Option<SharedString>) {
    // Threads (Inbox > Unreads lists joined ones) live in the nav thread cache.
    let c = app.channels.get(&ch).or_else(|| app.nav.find_thread(ch));
    let name = c.map(|c| c.name.to_string()).unwrap_or_else(|| crate::t!("common.unknown_channel").to_string());
    let guild = c.and_then(|c| c.guild).and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone());
    // `guildName_8ZcKCk` (vanilla-inbox-1 #2666): "<guild> › <category>" when the
    // channel sits in a category.
    let category = c.and_then(|c| c.parent).and_then(|p| app.channels.get(&p)).filter(|p| p.kind == crate::model::ChannelKind::Category);
    let guild = match (guild, category) {
        (Some(g), Some(cat)) => Some(format!("{g} \u{203a} {}", cat.name).into()),
        (g, _) => g,
    };
    (name.into(), guild)
}

/// `box_0JZW4Z` channel card head (`channelHeader_8ZcKCk`): padding 12, gap 8,
/// --background-surface-higher, border-bottom 1px --border-subtle; 32px guild
/// icon (radius 8), 12px chevron, name body-md/semibold 16/20 --text-default with
/// a 12px type glyph, guild line body-xs 12/16; trailing `buttonGroup_` gap 8.
fn channel_head(app: &PhoenixApp, ch: Id, buttons: Vec<gpui::Stateful<gpui::Div>>) -> gpui::Div {
    let (name, guild_name) = channel_label(app, ch);
    let channel = app.channels.get(&ch).or_else(|| app.nav.find_thread(ch));
    let guild = channel.and_then(|c| c.guild).and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let glyph = match channel.map(|c| c.kind) {
        Some(crate::model::ChannelKind::Voice) => "speaker",
        Some(crate::model::ChannelKind::Announcement) => "megaphone",
        Some(crate::model::ChannelKind::Forum) => "forum",
        _ => "hash",
    };
    div()
        .p(z(12.))
        .flex()
        .items_center()
        .gap(z(8.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(match guild {
            Some(g) => div().size(z(32.)).rounded(z(8.)).overflow_hidden().child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 32. * crate::theme::zoom())),
            None => div().size(z(32.)).rounded(z(8.)).overflow_hidden().child(avatar(channel.and_then(|c| c.voice_users.first()).and_then(|u| app.users.get(u)), 32.)),
        })
        .child(named("chevron-right", 12., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tk(idx::TEXT_DEFAULT))
                        .child(named(glyph, 12., tk(idx::TEXT_DEFAULT)))
                        .child(div().min_w_0().whitespace_nowrap().overflow_hidden().text_ellipsis().child(name)),
                )
                .when_some(guild_name, |d, g| {
                    d.child(div().text_size(z(12.)).line_height(z(16.)).text_color(tk(idx::TEXT_DEFAULT)).whitespace_nowrap().overflow_hidden().text_ellipsis().child(g))
                }),
        )
        .child(div().flex().gap(z(8.)).children(buttons))
}

/// Cozy message of the inbox cards (`singleMessage_0JZW4Z wrapper_2i8Bfm cozy_`):
/// padding 2 20 2 80, 40px avatar at 20px, username 16/22 500 --text-strong,
/// timestamp 12px 500 --chat-text-muted (margin-start 4), content 16/22 --text-default.
fn cozy_message(app: &PhoenixApp, m: &Message, pad_y: f32, cx: &Context<PhoenixApp>) -> gpui::Div {
    let u = app.users.get(&m.author);
    let name_color = app.author_color(m.author).map(|c2| crate::theme::role_color(Some(c2))).unwrap_or_else(|| tk(idx::TEXT_STRONG));
    div()
        .relative()
        .pl(z(80.))
        .pr(z(20.))
        .py(z(pad_y))
        .min_h(z(44.))
        .child(div().absolute().left(z(20.)).top(z(pad_y + 2.)).child(avatar(u, 40.)))
        .child(
            div()
                .flex()
                .items_baseline()
                .child(
                    div()
                        .mr(z(4.))
                        .text_size(z(16.))
                        .line_height(z(22.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(name_color)
                        .child(u.map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("common.unknown"))),
                )
                .when_some(u.and_then(|u| u.tag.clone()), |d, t| d.child(crate::ui::members::server_tag(&t).ml(z(2.)).mr(z(4.))))
                .child(div().ml(z(4.)).text_size(z(12.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::CHAT_TEXT_MUTED)).child(crate::app::format_time_full(m.timestamp))),
        )
        .child(div().text_size(z(16.)).line_height(z(22.)).text_color(tk(idx::TEXT_DEFAULT)).child(crate::ui::components::markdown_in(app, &m.content, &cx.entity(), m.id)))
}

/// `box_0JZW4Z`: --background-surface-higher, 1px --border-subtle, radius
/// --radius-md, margin-bottom 16.
fn inbox_box() -> gpui::Div {
    div()
        // Never shrink inside the scroller's flex column: with more entries than
        // fit, flex-shrink squashed every box to 0 (blank list over real data).
        .flex_none()
        .mb(z(16.))
        .rounded(z(12.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .overflow_hidden()
        .flex()
        .flex_col()
}

fn message_entry(
    app: &PhoenixApp,
    m: &Message,
    cx: &mut Context<PhoenixApp>,
    close_glyph: &'static str,
    close: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
) -> AnyElement {
    let (ch, id) = (m.channel, m.id);
    inbox_box()
        .child(channel_head(
            app,
            ch,
            vec![
                sm_icon_button(("inbox-jump", id.0), "inbox/jump").on_click(cx.listener(move |a, _, _, cx| a.jump_from_inbox(ch, id, cx))),
                sm_icon_button(("inbox-x", id.0), close_glyph).on_click(cx.listener(move |a, _, _, cx| close(a, cx))),
            ],
        ))
        .child(div().pt(z(12.)).pb(z(12.)).child(cozy_message(app, m, 2., cx)))
        .into_any_element()
}

/// One Inbox > Unreads card (client `n5` header + `n3` messages): the channel
/// head with its mark-read button, then up to 25 unread messages, grouped like
/// the chat (same author within 7 minutes), with day dividers unless the first
/// one is from today, and "View all" when more are unread.
fn unread_card(app: &PhoenixApp, card: &crate::commands::state::UnreadCard, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ch = card.entry.channel;
    let first = card.msgs.first().map(|m| m.id);
    let head = div()
        .id(("inbox-unread", ch.0))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| match first {
            Some(id) => a.jump_from_inbox(ch, id, cx),
            None => {
                a.parity.inbox.open = false;
                a.select_channel(ch, cx);
            }
        }))
        .child(channel_head(
            app,
            ch,
            vec![sm_icon_button(("inbox-unread-read", ch.0), "check").on_click(cx.listener(move |a, _, _, cx| {
                cx.stop_propagation();
                a.mark_unread_card_read(ch, cx);
            }))],
        ));
    let local_day = |ts: i64| chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).date_naive());
    let today = chrono::Local::now().date_naive();
    let days = card.msgs.first().is_some_and(|m| local_day(m.timestamp) != Some(today));
    let mut body = div().py(z(8.)).flex().flex_col();
    let mut prev: Option<&Message> = None;
    for m in &card.msgs {
        let day = local_day(m.timestamp);
        if days && prev.is_none_or(|p| local_day(p.timestamp) != day) {
            if let Some(d) = day {
                body = body.child(day_divider(crate::i18n::long_date(d).into()));
            }
            prev = None;
        }
        let start = prev.is_none_or(|p| p.author != m.author || m.timestamp - p.timestamp > 7 * 60);
        let id = m.id;
        let row = if start {
            cozy_message(app, m, 2., cx)
        } else {
            div()
                .pl(z(80.))
                .pr(z(20.))
                .py(z(1.))
                .text_size(z(16.))
                .line_height(z(22.))
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(crate::ui::components::markdown_in(app, &m.content, &cx.entity(), m.id))
        };
        // Hover "Jump" pill (`jumpButton_`), top-end of the message.
        let group: SharedString = format!("inbox-unread-msg-{}", id.0).into();
        body = body.child(
            div().relative().group(group.clone()).child(row).child(
                div()
                    .absolute()
                    .top(z(0.))
                    .right(z(12.))
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .child(sm_text_button(("inbox-unread-jump", id.0), crate::t!("inbox.jump"), false).on_click(cx.listener(move |a, _, _, cx| a.jump_from_inbox(ch, id, cx)))),
            ),
        );
        prev = Some(m);
    }
    if let Some(e) = &card.error {
        body = body.child(div().px(z(16.)).text_size(z(14.)).text_color(tk(idx::TEXT_FEEDBACK_CRITICAL)).child(crate::t!("inbox.load_failed", e = e)));
    }
    if card.more
        && let Some(last) = card.msgs.last().map(|m| m.id)
    {
        body = body.child(
            div()
                .pt(z(8.))
                .flex()
                .justify_center()
                .child(sm_text_button(("inbox-unread-all", ch.0), crate::t!("inbox.view_all"), true).on_click(cx.listener(move |a, _, _, cx| a.jump_from_inbox(ch, last, cx)))),
        );
    }
    inbox_box().child(head).child(body).into_any_element()
}

/// Day divider of the unread cards (`divider_` with the "LL" date).
fn day_divider(label: SharedString) -> gpui::Div {
    let line = || div().flex_1().h(z(1.)).bg(tk(idx::BORDER_SUBTLE));
    div()
        .mx(z(16.))
        .my(z(8.))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(line())
        .child(div().text_size(z(12.)).line_height(z(13.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_MUTED)).child(label))
        .child(line())
}

/// Filter dropdown of the Mentions tab (a `menu_M3qk4j` with checkbox rows).
fn filter_menu(i: &crate::commands::state::InboxState, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let toggle = |f: fn(&mut crate::commands::state::InboxState)| {
        cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<PhoenixApp>| {
            f(&mut a.parity.inbox);
            a.load_inbox_tab(true, cx);
            cx.notify();
        })
    };
    let row = |id: &'static str, label: SharedString, on: bool| {
        div()
            .id(id)
            .min_h(z(32.))
            .p(z(8.))
            .rounded(z(4.))
            .cursor_pointer()
            .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
            .flex()
            .items_center()
            .justify_between()
            .gap(z(8.))
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
            .child(label)
            .child(
                div()
                    .size(z(18.))
                    .rounded(z(4.))
                    .border_1()
                    .border_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                    .when(on, |d| d.bg(tk(idx::CONTROL_BRAND_FOREGROUND)).border_color(tk(idx::CONTROL_BRAND_FOREGROUND)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(on, |d| d.child(named("check", 14., gpui::white()))),
            )
    };
    div()
        .absolute()
        .top(z(56.))
        .right(z(56.))
        .w(z(220.))
        .p(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .shadow(shadow_tok(&shadow::SHADOW_HIGH))
        .occlude()
        .flex()
        .flex_col()
        .child(row("f-everyone", crate::t!("inbox.f_everyone"), i.everyone).on_click(toggle(|i| i.everyone = !i.everyone)))
        .child(row("f-roles", crate::t!("inbox.f_roles"), i.roles).on_click(toggle(|i| i.roles = !i.roles)))
        .child(row("f-server", crate::t!("inbox.f_server"), i.this_server).on_click(toggle(|i| i.this_server = !i.this_server)))
        .into_any_element()
}

fn inbox(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let i = &app.parity.inbox;
    if !i.open {
        return None;
    }
    let pending = app.nav.count(RelationshipKind::Incoming);
    // controls_LSviKk: gap 8, margin-inline-start auto. Unreads: "Mark All Read";
    // Mentions: "Filter"; every tab: friend requests (`friendRequestsButton_QwUPp6`).
    let mut controls = div().ml_auto().flex().items_center().gap(z(8.));
    match i.tab {
        InboxTab::Unreads => {
            // markAllRead: acks the listed channels only (user action).
            controls = controls.child(sm_icon_button("inbox-read-all", "inbox/read-all").on_click(cx.listener(|a, _, _, cx| {
                let ids: Vec<Id> = a.parity.inbox.unreads.iter().map(|c| c.entry.channel).collect();
                for id in ids {
                    a.mark_unread_card_read(id, cx);
                }
            })))
        }
        InboxTab::Mentions => {
            controls = controls.child(sm_icon_button("inbox-filter", "inbox/filter").on_click(cx.listener(|a, _, _, cx| {
                a.parity.inbox.filter_menu = !a.parity.inbox.filter_menu;
                cx.notify();
            })))
        }
        _ => {}
    }
    controls = controls.child(
        sm_icon_button("inbox-friend-requests", "friends")
            .px(z(8.))
            .w_auto()
            .min_w(z(32.))
            .gap(z(8.))
            .when(pending > 0, |d| d.child(crate::ui::badge(pending as u32)))
            .on_click(cx.listener(|a, _, _, cx| {
                a.parity.inbox.open = false;
                a.select_guild(None, cx);
                a.nav.friends_tab = crate::ui::home::data::FriendsTab::Pending;
                cx.notify();
            })),
    );
    // header_LSviKk: padding --space-md --space-md 0; headerTitle_: 20px
    // --icon-strong glyph (margin-end 8), body-lg/semibold 20/24 --text-strong.
    let header = div()
        .flex_none()
        .pt(z(space::md()))
        .px(z(space::md()))
        .child(
            div()
                .flex()
                .items_center()
                .child(named("inbox", 20., tk(idx::ICON_STRONG)).mr(z(8.)))
                .child(div().flex_1().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("inbox.title")))
                .child(controls),
        )
        .child(tab_bar(i.tab, cx));

    // scroller_0JZW4Z: padding 16 16 0.
    let mut list = div().id("inbox-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(&INBOX_SCROLL.with(|h| h.clone())).flex().flex_col();
    let scroller = |l: gpui::Stateful<gpui::Div>| l.pt(z(16.)).px(z(16.));
    match i.tab {
        InboxTab::Mentions => {
            if i.mentions.is_empty() && !i.loading {
                list = list.child(caught_up(None));
            } else {
                list = scroller(list);
            }
            for m in &i.mentions {
                let id = m.id;
                list = list.child(message_entry(app, m, cx, "check", move |a, cx| a.dismiss_mention(id, cx)));
            }
            if !i.reached_end && !i.mentions.is_empty() && !i.loading {
                list = list.child(
                    div()
                        .id("inbox-more")
                        .pb(z(16.))
                        .flex()
                        .justify_center()
                        .child(sm_text_button("inbox-more-btn", crate::t!("common.load_more"), false).on_click(cx.listener(|a, _, _, cx| a.load_inbox_tab(false, cx)))),
                );
            }
        }
        InboxTab::Saved => {
            if i.saved.is_empty() && !i.loading {
                list = list.child(upsell(
                    "inbox/bookmark",
                    crate::t!("inbox.saved_title"),
                    bold_middle(crate::t!("inbox.saved_body_a"), crate::t!("inbox.saved_body_b"), crate::t!("inbox.saved_body_c")),
                    wumpus_demo(false),
                ));
            } else {
                list = scroller(list);
            }
            for m in &i.saved {
                let (ch, id) = (m.channel, m.id);
                list = list.child(message_entry(app, m, cx, "close", move |a, cx| a.unsave(ch, id, cx)));
            }
        }
        InboxTab::Reminders if !crate::actions::message::reminders().is_empty() => {
            list = scroller(list);
            for m in crate::actions::message::reminders() {
                let (ch, id) = (m.channel, m.id);
                list = list.child(message_entry(app, &m, cx, "check", move |a, cx| a.clear_reminder(ch, id, cx)));
            }
        }
        InboxTab::Reminders => {
            list = list.child(upsell(
                "inbox/clock",
                crate::t!("inbox.reminders_title"),
                bold_middle(crate::t!("inbox.reminders_body_a"), crate::t!("inbox.reminders_body_b"), crate::t!("inbox.reminders_body_c")),
                wumpus_demo(true),
            ));
        }
        InboxTab::Scheduled if !i.scheduled.is_empty() => {
            // Read-only: the list the client shows; cancelling is not wired.
            list = scroller(list);
            for m in &i.scheduled {
                list = list.child(
                    inbox_box()
                        .child(channel_head(app, m.channel, Vec::new()))
                        .child(div().pt(z(12.)).pb(z(12.)).child(cozy_message(app, m, 2., cx))),
                );
            }
        }
        InboxTab::Scheduled => {
            list = list.child(upsell(
                "inbox/schedule",
                crate::t!("inbox.scheduled_title"),
                bold_middle(crate::t!("inbox.scheduled_body_a"), crate::t!("inbox.scheduled_body_b"), crate::t!("inbox.scheduled_body_c")),
                scheduled_demo(),
            ));
        }
        InboxTab::Unreads => {
            // Cards render once loaded (`hasLoadedAnything`), in list order; a
            // loaded card with nothing to show is dropped.
            if i.unreads.is_empty() {
                list = list.child(caught_up(Some(crate::t!("inbox.unreads_tip"))));
            } else {
                list = scroller(list);
            }
            for card in i.unreads.iter().filter(|c| c.loaded) {
                list = list.child(unread_card(app, card, cx));
            }
            if i.unreads_loading {
                list = list.child(div().pb(z(16.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("common.loading")));
            }
        }
    }
    if i.loading {
        list = list.child(div().p(z(16.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("common.loading")));
    }
    let filter = (i.filter_menu && i.tab == InboxTab::Mentions).then(|| filter_menu(i, cx));
    // container_uTMZTY: 560 wide (480–750), max-height 70vh, radius 8,
    // --background-surface-high, --shadow-border + --shadow-high; anchored under
    // the title-bar inbox button (right edges aligned, dump box x 483..1043, y 36).
    Some(
        gpui::deferred(
            div()
                .id("inbox-popout")
                .debug_selector(|| "inbox-popout".into())
                .map(|d| close_on_press_outside(d, cx))
                .absolute()
                .top(z(m::TITLE_BAR_H + 4.)) // container_uTMZTY y 36 (vanilla-inbox-1 #2620)
                .right(z(crate::ui::title_bar::inbox_right_inset())) // right edge = inbox button's
                .w(z(560.))
                // max-height 70vh; @media (max-height: 800px) 80vh, (max-height: 700px) 85vh
                .max_h(gpui::relative(match crate::ui::title_bar::viewport_h_css() {
                    h if h > 0. && h <= 700. => 0.85,
                    h if h > 0. && h <= 800. => 0.8,
                    _ => 0.7,
                }))
                .rounded(z(8.))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                .shadow(popout_shadow())
                .occlude()
                .flex()
                .flex_col()
                .child(header)
                .child(list)
                .children(filter),
        )
        .with_priority(10)
        .into_any_element(),
    )
}

/// Threads browser (`browser_l-3BKx container_c0tu5x`): 480×400, radius 8,
/// --background-surface-high, popout shadow; `header_c0tu5x` 48px, padding 0 16,
/// border-bottom --border-subtle: 24px thread glyph --interactive-text-default
/// (margin-end 8), heading-sm/semibold "Threads" (margin-end 16), 1×24 divider
/// (margin-end 16), sm search field, primary "Create". Right edge under the
/// header's threads button (dump x 408..888 of 1280, y 81).
fn threads(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !app.parity.threads_open {
        return None;
    }
    let ch = app.selected_channel?;
    let list_src = app.nav.threads.get(&ch).cloned().unwrap_or_default();
    let archived: Vec<_> = app
        .parity
        .archived_threads
        .get(&ch)
        .map(|v| v.iter().filter(|p| !list_src.iter().any(|t| t.id == p.thread)).cloned().collect())
        .unwrap_or_default();
    let header = div()
        .flex_none()
        .h(z(48.))
        .px(z(16.))
        .flex()
        .items_center()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(named("threads", 24., tk(idx::INTERACTIVE_TEXT_DEFAULT)).mr(z(8.)))
        .child(div().mr(z(16.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("threads.title")))
        .child(div().mr(z(16.)).w(z(1.)).h(z(24.)).bg(tk(idx::BORDER_SUBTLE)))
        .child(
            // input sm_0PLWhS: 32px, padding-start 4, gap 4, radius 8,
            // --input-background-default; 22px icon box.
            div()
                .w(z(248.))
                .h(z(32.))
                .pl(z(4.))
                .gap(z(4.))
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::INPUT_BORDER_ACTIVE)) // autofocused on open (vanilla-threads-lrer #1357 border = --input-border-active)
                .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                .flex()
                .items_center()
                .child(div().size(z(22.)).flex().items_center().justify_center().child(named("search", 16., tk(idx::TEXT_DEFAULT))))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT)).whitespace_nowrap().child(crate::t!("threads.search"))),
        )
        .child(div().flex_1())
        .child(div().ml(z(8.)).child(sm_text_button("threads-create", crate::t!("threads.create"), true).on_click(cx.listener(|a, _, w, cx| {
            if let Some(ch) = a.selected_channel {
                a.open_create_thread(ch, w, cx);
            }
        }))));
    // list_6_wQXw: padding 0 16.
    let mut list = div().id("threads-list").flex_1().min_h_0().overflow_y_scroll().px(z(16.)).flex().flex_col();
    if list_src.is_empty() && archived.is_empty() {
        // container_0FCMV2: min-height 400, padding 32, centred; 256×144
        // illustration mb 12; heading-xl/semibold 24/32 --text-strong mb 8;
        // body-md/normal 16/20 --text-default; cta margin-top 24 (md primary).
        list = list.child(
            div()
                .flex_1()
                .p(z(32.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .child(div().mb(z(12.)).w(z(256.)).h(z(144.)).flex().items_center().justify_center().child(named("threads", 96., tk(idx::TEXT_BRAND))))
                .child(div().mb(z(8.)).text_size(z(24.)).line_height(z(32.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("threads.empty")))
                .child(div().text_size(z(16.)).line_height(z(20.)).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("threads.empty_body")))
                .child(
                    div().mt(z(24.)).child(
                        sm_text_button("threads-create-cta", crate::t!("threads.create_thread"), true).on_click(cx.listener(|a, _, w, cx| {
                            if let Some(ch) = a.selected_channel {
                                a.open_create_thread(ch, w, cx);
                            }
                        })).h(z(40.)).px(z(16.)).text_size(z(16.)).line_height(z(20.)),
                    ),
                ),
        );
    } else if !list_src.is_empty() {
        // sectionHeader_6_wQXw (threads-lrer #1432): 24px, margin 8 0, 14/18.67
        // 500 --text-default, no uppercase: "N fils rejoints" (the gateway's
        // active threads are the joined ones; "anciens fils" needs the archive).
        list = list.child(
            div()
                .flex_none()
                .h(z(24.))
                .my(z(8.))
                .flex()
                .items_center()
                .text_size(z(14.))
                .line_height(z(18.67))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(crate::t!("threads.joined", n = list_src.len())),
        );
    }
    let now = chrono::Local::now().timestamp();
    for t in list_src {
        let r = crate::model::ThreadRef { id: t.id, name: t.name.clone(), message_count: 0 };
        let created = ((t.id.0 >> 22) / 1000 + 1_420_070_400) as i64;
        let sub = format!("{} • {}", crate::t!("threads.no_recent"), crate::ui::forum::ago(now, created));
        // container_yRVHqK: 72px, padding 15, radius 8, 1px --border-subtle,
        // --background-surface-higher, margin-bottom 8; hover --background-surface-highest
        // + --border-normal. threadName_ heading-sm/semibold 16/20 --text-default,
        // subtext_ body-sm 14/18 --text-default; 24px facepile margin-start 16.
        list = list.child(
            div()
                .id(("thread-row", t.id.0))
                .flex_none()
                .h(z(72.))
                .mb(z(8.))
                .p(z(15.))
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
                .hover(|s| s.bg(tk(idx::BACKGROUND_SURFACE_HIGHEST)).border_color(tk(idx::BORDER_NORMAL)))
                .cursor_pointer()
                .flex()
                .items_center()
                .on_click(cx.listener(move |a, _, w, cx| {
                    a.parity.threads_open = false;
                    a.open_thread(r.clone(), w, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).whitespace_nowrap().overflow_hidden().text_ellipsis().child(t.name.clone()))
                        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).whitespace_nowrap().overflow_hidden().text_ellipsis().child(sub)),
                )
                .when(!t.voice_users.is_empty(), |d| {
                    d.child(div().ml(z(16.)).flex().children(t.voice_users.iter().take(3).enumerate().map(|(k, u)| {
                        div().when(k > 0, |d| d.ml(z(-6.))).child(avatar(app.users.get(u), 24.))
                    })))
                }),
        );
    }
    if !archived.is_empty() {
        // threads-lrer #1442: same sectionHeader_6_wQXw as the joined group.
        list = list.child(section_header(crate::t!("threads.older").to_string()));
    }
    for p in archived {
        list = list.child(archived_row(app, p, cx));
    }
    Some(
        gpui::deferred(
            div()
                .id("threads-popout")
                .debug_selector(|| "threads-popout".into())
                .map(|d| close_on_press_outside(d, cx))
                .absolute()
                .top(z(m::TITLE_BAR_H + m::HEADER_H - 3.))
                .right(z(HDR_RIGHT_THREADS))
                .w(z(480.))
                .h(z(400.))
                .rounded(z(8.))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                .shadow(popout_shadow())
                .occlude()
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(header)
                .child(list),
        )
        .with_priority(10)
        .into_any_element(),
    )
}

/// Distance from the window's right edge to the right edge of a channel-header
/// button: toolbar padding-right 8 + search (244 + margin 4) + n × (32 + gap 10).
const fn hdr_right(buttons_after: f32) -> f32 {
    8. + 248. + buttons_after * 42.
}
const HDR_RIGHT_THREADS: f32 = hdr_right(3.);

/// Pinned messages popout (`messagesPopoutWrap_6bgemo`): 420 wide, min-height
/// 200, max 80vh, radius 8, --background-surface-high, popout shadow. Header
/// padding 16, border-bottom --border-subtle, 24px pin glyph + body-lg/semibold
/// 20/24 --text-strong (gap 8). Scroller padding 8 0 0 8. Right edge under the
/// header pin button (dump x 552..972 of 1280, y 81).
pub fn pins_popout(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let p = app.chat.pins.as_ref()?;
    let ch = p.channel;
    let is_dm = app.channels.get(&ch).is_some_and(|c| c.guild.is_none());
    let mut list = div().id("pins-list").flex_1().min_h_0().overflow_y_scroll().pt(z(8.)).pl(z(8.)).flex().flex_col();
    let mut footer = None;
    if p.loading {
        list = list.child(div().p(z(16.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("common.loading")));
    } else if p.msgs.is_empty() {
        // emptyPlaceholder_: margin 8 0 32, centred; 120px image; body_ margin-top
        // 20, body-md/medium 16/20 --text-default, centred (white-space: pre).
        list = list.child(
            div()
                .mt(z(8.))
                .mb(z(32.))
                .flex()
                .flex_col()
                .items_center()
                .child(div().size(z(120.)).rounded_full().bg(tk(idx::BACKGROUND_MOD_STRONG)).flex().items_center().justify_center().child(named("pin", 56., tk(idx::INTERACTIVE_TEXT_DEFAULT))))
                .child(div().mt(z(20.)).text_center().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("pins.empty_line1")))
                .child(div().text_center().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("pins.empty_line2"))),
        );
        // footer_6bgemo: padding 16, --background-mod-subtle, centred; protip
        // block padding 10 0: body-sm/semibold --text-feedback-positive, then
        // body-sm/normal 14/16 --text-default.
        footer = Some(
            div()
                .flex_none()
                .p(z(16.))
                .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
                .child(
                    div()
                        .py(z(10.))
                        .text_center()
                        .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_FEEDBACK_POSITIVE)).child(crate::t!("inbox.protip")))
                        .child(div().text_size(z(14.)).line_height(z(16.)).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("pins.tip"))),
                ),
        );
    }
    for m in &p.msgs {
        let id = m.id;
        // messageGroupWrapper_6bgemo: --background-surface-higher, 1px
        // --border-subtle, radius 8, margin 0 4 8 0; messageGroupCozy_ padding
        // 16 20 16 80 (margin-start -4). actionButtons_ on hover: top/right 8, sm
        // secondary "Jump".
        list = list.child(
            div()
                .id(("pin", id.0))
                .group("pin")
                .relative()
                .flex_none()
                .mr(z(4.))
                .mb(z(8.))
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
                .overflow_hidden()
                .child(div().ml(z(-4.)).child(cozy_message(app, m, 16., cx)))
                .child(
                    div()
                        .absolute()
                        .top(z(8.))
                        .right(z(8.))
                        .flex()
                        .gap(z(8.))
                        .invisible()
                        .group_hover("pin", |s| s.visible())
                        .child(sm_text_button(("pin-jump", id.0), crate::t!("pins.jump"), false).on_click(cx.listener(move |a, _, _, cx| a.jump_to_message(ch, id, cx))))
                        .child(sm_icon_button(("pin-unpin", id.0), "close").on_click(cx.listener(move |a, _, _, cx| {
                            cx.stop_propagation();
                            a.unpin_from_popout(ch, id, cx)
                        }))),
                ),
        );
    }
    let right = if is_dm { hdr_right(2.) } else { hdr_right(1.) };
    // Deferred: drawn over the chat like the inbox.
    Some(gpui::deferred(
        div()
            .id("pins-popout")
            .debug_selector(|| "pins-popout".into())
                .map(|d| close_on_press_outside(d, cx))
            .absolute()
            .top(z(m::HEADER_H - 3.))
            .right(z(right))
            .w(z(420.))
            .min_h(z(200.))
            .max_h(gpui::relative(0.8))
            .rounded(z(8.))
            .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
            .shadow(popout_shadow())
            .occlude()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .p(z(16.))
                    .border_b_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(named("pin", 24., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("pins.title"))),
            )
            .child(list)
            .children(footer),
    )
    .with_priority(10)
    .into_any_element())
}

thread_local! {
    static INBOX_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

/// The client puts header popouts in a `clickTrapContainer_ trapClicks_`
/// layer: a press anywhere outside closes the popout and goes no further (the
/// trigger button included, so pressing it again closes). Done here on the
/// popout itself, in the capture phase, from the bounds it was painted at.
fn close_on_press_outside(el: gpui::Stateful<gpui::Div>, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let app = cx.entity().downgrade();
    // A canvas covering the popout: its paint bounds are the popout's drawn box.
    el.child(
        gpui::canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                window.on_mouse_event(move |ev: &gpui::MouseDownEvent, phase, _, cx| {
                    if phase != gpui::DispatchPhase::Capture || bounds.contains(&ev.position) {
                        return;
                    }
                    let Some(app) = app.upgrade() else { return };
                    // A profile popout or menu opened over the popout handles its own presses.
                    let closed = app.update(cx, |a, cx| !a.people.any_overlay() && a.close_header_popouts(cx));
                    if closed {
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0(),
    )
}

pub fn popouts(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    out.extend(inbox(app, cx));
    out.extend(threads(app, cx));
    out
}

/// Our own fallbacks for the icons above, used when no local Discord install
/// provides the real ones (hand-drawn, 24×24, tinted through `text_color`).
pub fn svg_asset(path: &str) -> Option<&'static str> {
    macro_rules! st {
        ($b:expr) => {
            concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#, $b, "</svg>")
        };
    }
    Some(match path {
        "icons/inbox/filter.svg" => st!(r#"<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>"#),
        "icons/inbox/read-all.svg" => st!(r#"<path d="m2 12.5 4.5 4.5L15 8.5M11 16.5l.5.5L20 8.5"/>"#),
        "icons/inbox/jump.svg" => st!(r#"<path d="M20 11.5a8 8 0 1 0-3.2 6.4L21 19l-1-3.6M9 12h6m-2.5-2.5L15 12l-2.5 2.5"/>"#),
        "icons/inbox/bookmark.svg" => st!(r#"<path d="M6 3h12v18l-6-4.5L6 21z"/>"#),
        "icons/inbox/clock.svg" => st!(r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#),
        "icons/inbox/schedule.svg" => st!(r#"<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4M12 13v5M9.5 15.5h5"/>"#),
        "icons/search/user.svg" => st!(r#"<circle cx="12" cy="8" r="4"/><path d="M4 21a8 7 0 0 1 16 0"/>"#),
        "icons/search/attachment.svg" => st!(r#"<path d="m20 11-8.5 8.5a5 5 0 0 1-7-7L13 4a3.3 3.3 0 0 1 4.7 4.7l-8.4 8.4a1.7 1.7 0 0 1-2.4-2.4L15 7"/>"#),
        "icons/search/at.svg" => st!(r#"<circle cx="12" cy="12" r="4"/><path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/>"#),
        "icons/chat.svg" => st!(r#"<path d="M21 11.5a8.5 8.5 0 0 1-12.4 7.6L3 21l1.9-5.6A8.5 8.5 0 1 1 21 11.5z"/>"#),
        "icons/more-vertical.svg" => st!(r#"<circle cx="12" cy="5" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="12" cy="19" r="1"/>"#),
        "icons/new-group-dm.svg" => st!(r#"<path d="M20 11.5a8 8 0 1 1-3.2-6.4M12 8v7m-3.5-3.5h7M19 2v6m-3-3h6"/>"#),
        _ => return None,
    })
}

/// Build the inputs that need a `Window` (group DM search).
pub fn prepare(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    // Unreads loads the next card while the list end is within 2000px of the
    // viewport bottom (client `maybeLoadMore`).
    let i = &app.parity.inbox;
    if i.open && i.tab == InboxTab::Unreads && !i.unreads_loading && i.unreads.iter().any(|c| !c.loaded) {
        let near = INBOX_SCROLL.with(|h| h.max_offset().y + h.offset().y < gpui::px(2000.));
        if near {
            app.load_next_unread(cx);
        }
    }
    if let Some(p) = app.parity.group_dm.as_mut()
        && p.search.is_none()
    {
        let i = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("groupdm.placeholder")));
        i.update(cx, |s, cx| s.focus(window, cx));
        p.search = Some(i);
    }
}

fn radio_row(id: (&'static str, u64), label: gpui::SharedString, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(z(12.))
        .py(z(10.))
        .rounded(z(4.))
        .cursor_pointer()
        .bg(if on { c(Palette::BG_MOD_SELECTED) } else { c(Palette::BG_SECONDARY) })
        .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
        .flex()
        .items_center()
        .gap(z(10.))
        .child(
            div()
                .size(z(20.))
                .rounded_full()
                .border_2()
                .border_color(if on { c(Palette::BLURPLE) } else { c(Palette::INTERACTIVE) })
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| d.child(div().size(z(10.)).rounded_full().bg(c(Palette::BLURPLE)))),
        )
        .child(div().text_size(z(16.)).text_color(c(Palette::TEXT_NORMAL)).child(label))
}

fn moderation_dialog(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let d = app.parity.moderation?;
    let name = app.users.get(&d.user).map(|u| u.name.to_string()).unwrap_or_else(|| crate::t!("common.this_user").to_string());
    let guild = app.guilds.iter().find(|g| g.id == d.guild).map(|g| g.name.to_string()).unwrap_or_default();
    let e = cx.entity();
    let set = move |kind: ModKind| {
        let e = e.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            e.update(cx, |a, cx| {
                if let Some(m) = a.parity.moderation.as_mut() {
                    m.kind = kind;
                }
                cx.notify();
            })
        }
    };
    let (title, confirm, body): (String, gpui::SharedString, Vec<AnyElement>) = match d.kind {
        ModKind::Timeout(0) => (
            crate::t!("mod.remove_timeout_title", name = name).to_string(),
            crate::t!("mod.remove_timeout"),
            vec![div().text_color(c(Palette::TEXT_NORMAL)).child(crate::t!("mod.remove_timeout_body", name = name).to_string()).into_any_element()],
        ),
        ModKind::Timeout(secs) => {
            let mut col = div().flex().flex_col().gap(z(4.)).child(
                div().text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("mod.duration")),
            );
            for (i, (label, s)) in TIMEOUTS.iter().enumerate() {
                col = col.child(radio_row(("timeout-opt", i as u64), crate::t!(label), secs == *s).on_click(set(ModKind::Timeout(*s))));
            }
            (
                crate::t!("mod.timeout_user", name = name).to_string(),
                crate::t!("mod.timeout"),
                vec![
                    div()
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(crate::t!("mod.timeout_note"))
                        .into_any_element(),
                    col.into_any_element(),
                ],
            )
        }
        ModKind::Kick => (
            crate::t!("mod.kick_title", name = name).to_string(),
            crate::t!("mod.kick"),
            vec![
                div()
                    .text_color(c(Palette::TEXT_NORMAL))
                    .child(crate::t!("mod.kick_body", name = name, guild = guild).to_string())
                    .into_any_element(),
            ],
        ),
        ModKind::Ban(del) => {
            let mut col = div().flex().flex_col().gap(z(4.)).child(
                div().text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("mod.delete_history")),
            );
            for (i, (label, s)) in BAN_HISTORY.iter().enumerate() {
                col = col.child(radio_row(("ban-opt", i as u64), crate::t!(label), del == *s).on_click(set(ModKind::Ban(*s))));
            }
            (
                crate::t!("mod.ban_title", name = name).to_string(),
                crate::t!("mod.ban"),
                vec![
                    div()
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(crate::t!("mod.ban_body", name = name, guild = guild).to_string())
                        .into_any_element(),
                    col.into_any_element(),
                ],
            )
        }
    };
    let (e1, e2) = (cx.entity(), cx.entity());
    Some(crate::ui::modal::confirm(crate::ui::modal::Confirm {
        title: title.into(),
        body,
        confirm_label: confirm,
        danger: true,
        on_confirm: std::rc::Rc::new(move |_, cx| e1.update(cx, |a, cx| a.confirm_moderation(cx))),
        on_cancel: std::rc::Rc::new(move |_, cx| {
            e2.update(cx, |a, cx| {
                a.parity.moderation = None;
                cx.notify();
            })
        }),
    }))
}

/// Max other people in a group DM (10 members including us).
pub const GROUP_DM_MAX: usize = 9;

fn group_dm(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let p = app.parity.group_dm.as_ref()?;
    let q = p.search.as_ref().map(|s| s.read(cx).value().to_lowercase()).unwrap_or_default();
    let friends: Vec<Id> = app
        .nav
        .relationships
        .iter()
        .filter(|r| r.kind == RelationshipKind::Friend)
        .map(|r| r.user)
        .filter(|u| {
            q.is_empty()
                || app.users.get(u).is_some_and(|u| u.name.to_lowercase().contains(&q) || u.display_name.to_lowercase().contains(&q))
        })
        .collect();
    let left = GROUP_DM_MAX.saturating_sub(p.picked.len());
    let mut list = div().id("gdm-list").h(z(240.)).overflow_y_scroll().flex().flex_col();
    if friends.is_empty() {
        list = list.child(div().p(z(16.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("groupdm.none")));
    }
    for u in friends {
        let on = p.picked.contains(&u);
        let user = app.users.get(&u);
        list = list.child(
            // friend_45OTue: 48px, padding 8, --radius-sm, gap 8, --text-muted;
            // friendSelected_ (mouse/keyboard highlight): --background-mod-subtle +
            // --text-strong, background-color .2s ease.
            crate::ui::states::track(div().id(("gdm-friend", u.0)), crate::ui::states::key(("gdm-friend", u.0)))
                .relative()
                .child(crate::ui::states::fade_layer(
                    crate::ui::states::key(("gdm-friend", u.0)),
                    gpui::transparent_black(),
                    tk(idx::BACKGROUND_MOD_SUBTLE),
                    200,
                    8.,
                ))
                .h(z(48.))
                .p(z(8.))
                .rounded(z(8.))
                .cursor_pointer()
                .text_color(tk(idx::TEXT_MUTED))
                .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
                .flex()
                .items_center()
                .gap(z(8.))
                .on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(p) = a.parity.group_dm.as_mut() {
                        if let Some(i) = p.picked.iter().position(|x| *x == u) {
                            p.picked.remove(i);
                        } else if p.picked.len() < GROUP_DM_MAX {
                            p.picked.push(u);
                        }
                    }
                    cx.notify();
                }))
                .child(avatar(user, 32.))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .gap(z(6.))
                        .items_baseline()
                        // nickname_ inherits the row colour; discordTag_ --text-muted.
                        .child(div().text_size(z(16.)).child(user.map(|u| u.display_name.clone()).unwrap_or_default()))
                        .child(div().text_size(z(14.)).text_color(tk(idx::TEXT_MUTED)).child(user.map(|u| u.name.clone()).unwrap_or_default())),
                )
                .child(
                    div()
                        .size(z(22.))
                        .rounded(z(4.))
                        .border_1()
                        .border_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                        .when(on, |d| d.bg(tk(idx::CONTROL_BRAND_FOREGROUND)).border_color(tk(idx::CONTROL_BRAND_FOREGROUND)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(gpui::white())
                        .when(on, |d| d.child("✓")),
                ),
        );
    }
    let label = if p.picked.len() > 1 { crate::t!("groupdm.create_group") } else { crate::t!("groupdm.create") };
    let can = !p.picked.is_empty() && !p.sending;
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        e.update(cx, |a, cx| {
            a.parity.group_dm = None;
            cx.notify();
        })
    });
    let card = div()
        .w(z(440.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(Palette::BG_PRIMARY))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .p(z(16.))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_HEADER)).child(crate::t!("groupdm.select")))
                .child(div().text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(if left == 1 { crate::t!("groupdm.left_one") } else { crate::t!("groupdm.left_many", left = left) }))
                .child(
                    div()
                        .mt(z(12.))
                        .h(z(40.))
                        .px(z(8.))
                        .rounded(z(4.))
                        .bg(c(Palette::INPUT_BG))
                        .flex()
                        .items_center()
                        .text_size(z(16.))
                        .when_some(p.search.clone(), |d, s| d.child(Input::new(&s).appearance(false).bordered(false))),
                ),
        )
        .child(div().px(z(8.)).child(list))
        .child(
            div().p(z(16.)).border_t_1().border_color(c(Palette::DIVIDER)).child(
                div()
                    .id("gdm-create")
                    .h(z(38.))
                    .rounded(z(8.))
                    .bg(c(Palette::BLURPLE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(gpui::white())
                    .when(can, |d| d.cursor_pointer().on_click(cx.listener(|a, _, _, cx| a.create_group_dm(cx))))
                    .when(!can, |d| d.opacity(0.5))
                    .child(label),
            ),
        );
    Some(crate::ui::modal::frame("group-dm-modal", close, card))
}

pub fn dialogs(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    out.extend(moderation_dialog(app, cx));
    out.extend(crate::ui::events::modal(app, cx));
    out.extend(group_dm(app, cx));
    out.extend(crate::ui::forum::compose_modal(app, cx));
    out
}

/// sectionHeader_6_wQXw (threads-lrer #1432/#1442): 24px, margin 8 0,
/// 14/18.67 500 --text-default, no text-transform.
fn section_header(label: String) -> gpui::Div {
    div()
        .flex_none()
        .h(z(24.))
        .my(z(8.))
        .flex()
        .items_center()
        .text_size(z(14.))
        .line_height(z(18.67))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_DEFAULT))
        .child(label)
}

/// Archived thread row (threads-lrer #1443): container_yRVHqK 72px like the
/// joined rows; subtext_yRVHqK = 16px avatar_yRVHqK (margin-end 8) +
/// "Started by" + startedByName_ (margin-start 4) + bullet_yRVHqK (20px box)
/// + "Last activity: <long date>".
fn archived_row(app: &PhoenixApp, p: crate::commands::ForumPost, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let r = crate::model::ThreadRef { id: p.thread, name: p.title.clone().into(), message_count: p.message_count };
    let last = p.last_message.unwrap_or(p.thread);
    let secs = ((last.0 >> 22) / 1000 + 1_420_070_400) as i64;
    let date = chrono::DateTime::from_timestamp(secs, 0).map(|d| crate::i18n::long_date(d.with_timezone(&chrono::Local).date_naive())).unwrap_or_default();
    let owner = p.owner.and_then(|o| app.users.get(&o));
    let owner_name = owner.map(|u| if u.display_name.is_empty() { u.name.clone() } else { u.display_name.clone() }).unwrap_or_default();
    div()
        .id(("archived-thread-row", p.thread.0))
        .flex_none()
        .h(z(72.))
        .mb(z(8.))
        .p(z(15.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .hover(|s| s.bg(tk(idx::BACKGROUND_SURFACE_HIGHEST)).border_color(tk(idx::BORDER_NORMAL)))
        .cursor_pointer()
        .flex()
        .items_center()
        .on_click(cx.listener(move |a, _, w, cx| {
            a.parity.threads_open = false;
            a.open_thread(r.clone(), w, cx);
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).whitespace_nowrap().overflow_hidden().text_ellipsis().child(p.title.clone()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(tk(idx::TEXT_DEFAULT))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child(div().mr(z(8.)).child(avatar(owner, 16.)))
                        .child(crate::t!("threads.started_by"))
                        .child(div().ml(z(4.)).child(owner_name))
                        .child(div().w(z(20.)).flex().justify_center().child("•"))
                        .child(crate::t!("threads.last_activity", date = date)),
                ),
        )
        .when_some(thread_facepile(app, &p.member_ids_preview, p.member_count), |d, f| d.child(f))
}

/// What the threads-browser facepile shows: every preview user (max 50 so
/// all fit) then a pill — "50+" when count >= max, "+N" when the member
/// count exceeds the preview (Facepile renderUsers / renderMoreUsers).
fn facepile_more(preview: usize, count: u32) -> Option<String> {
    const MAX: u32 = 50; // ThreadBrowser row: Facepile max 50
    if count >= MAX {
        Some(format!("{MAX}+"))
    } else if count as usize > preview {
        Some(format!("+{}", count as usize - preview))
    } else {
        None
    }
}

/// facepile_yRVHqK container_m-EqeQ size24_m-EqeQ (threads-lrer #1392):
/// margin-start 16, flex center; avatarContainerMasked_ 24px with
/// margin-inline-end −6 except the last avatar (avatarContainer_, no margin)
/// when no pill follows. moreUsers_ size24: h 24, min-w 24, r 12, padding 0 8,
/// 12px semibold, --background-base-lowest, --text-default. Unknown users get
/// the default avatar (showDefaultAvatarsForNullUsers).
fn thread_facepile(app: &PhoenixApp, ids: &[Id], count: u32) -> Option<gpui::Div> {
    if ids.is_empty() {
        return None;
    }
    let more = facepile_more(ids.len(), count);
    let n = ids.len();
    let has_more = more.is_some();
    Some(
        div()
            .flex_none()
            .ml(z(16.))
            .flex()
            .items_center()
            .children(ids.iter().enumerate().map(|(k, u)| {
                let last = k + 1 == n && !has_more;
                div().flex_none().size(z(24.)).when(!last, |d| d.mr(z(-6.))).child(avatar(app.users.get(u), 24.))
            }))
            .when_some(more, |d, t| {
                d.child(
                    div()
                        .flex_none()
                        .h(z(24.))
                        .min_w(z(24.))
                        .px(z(8.))
                        .rounded(z(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                        .text_color(tk(idx::TEXT_DEFAULT))
                        .text_size(z(12.))
                        .line_height(z(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t),
                )
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perms() {
        assert!(has_perm(ADMINISTRATOR, BAN_MEMBERS));
        assert!(has_perm(KICK_MEMBERS, KICK_MEMBERS));
        assert!(!has_perm(KICK_MEMBERS, MODERATE_MEMBERS));
    }

    #[test]
    fn mention_filters_default_on() {
        // Client RecentMentionsStore: {guildFilter: ALL_SERVERS, everyoneFilter: true, roleFilter: true}.
        let i = crate::commands::state::InboxState::default();
        assert!(i.everyone && i.roles && !i.this_server);
    }

    #[test]
    fn facepile_pill() {
        assert_eq!(facepile_more(3, 3), None);
        assert_eq!(facepile_more(3, 0), None);
        assert_eq!(facepile_more(3, 7).as_deref(), Some("+4"));
        assert_eq!(facepile_more(3, 50).as_deref(), Some("50+"));
    }
}
