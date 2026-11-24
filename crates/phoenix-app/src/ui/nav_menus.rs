//! Right-click menus on the navigation: channel rows, DM rows and guild icons.
//!
//! Item order observed in the official client:
//! - text channel: Mark As Read | Invite People, Copy Link | Mute Channel >,
//!   Notification Settings > | Copy Channel ID (developer mode)
//! - DM (live 2026 dump): Mark As Read | Pin, Favorite > | Profile, Call,
//!   Add Note, Add Friend Nickname, Close DM | Apps >, Invite to Server >,
//!   Remove Friend, Ignore, Block | Mute @user > | Copy Channel ID
//! - guild: Mark As Read | Invite People | Mute Server >, Notification
//!   Settings >, Hide Muted Channels | Privacy Settings | Leave Server |
//!   Copy Server ID
//!
//! Mute durations: 15 minutes, 1 hour, 3 hours, 8 hours, 24 hours, until
//! turned back on.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use gpui::{AnyElement, App, ClipboardItem, Context, Pixels, Point, Window};

use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    ui::{
        context_menu::{self, MenuItem},
        home::data::{NavRequest, Overlay},
    },
};

#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Channel(Id),
    /// Header bell notification menu for the open channel.
    ChannelBell(Id),
    /// Right-click menu on a sidebar thread row.
    Thread(Id),
    Guild(Id),
    /// Guild folder (`guild_folders` id).
    Folder(u64),
}

type Scope = (Option<Id>, Option<Id>);

thread_local! {
    static OPEN: RefCell<Option<(Target, Point<Pixels>)>> = const { RefCell::new(None) };
    /// Mutes and levels set from these menus this session (the READY copy
    /// lives in the backend), so radios and labels reflect the last choice.
    static MUTED_GUILDS: RefCell<HashMap<Id, bool>> = RefCell::new(HashMap::new());
    static LEVELS: RefCell<HashMap<Scope, u8>> = RefCell::new(HashMap::new());
}

pub fn open(target: Target, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = Some((target, pos)));
    window.refresh();
}

/// Toggle the channel header bell notification menu.
pub fn toggle_bell(channel: Id, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| {
        let mut cur = o.borrow_mut();
        if matches!(*cur, Some((Target::ChannelBell(c), _)) if c == channel) {
            *cur = None;
        } else {
            *cur = Some((Target::ChannelBell(channel), pos));
        }
    });
    window.refresh();
}

/// Close the header bell menu if open; returns true when it was open.
pub fn close_bell() -> bool {
    OPEN.with(|o| {
        let mut cur = o.borrow_mut();
        if matches!(*cur, Some((Target::ChannelBell(_), _))) {
            *cur = None;
            true
        } else {
            false
        }
    })
}

pub fn is_bell_open() -> bool {
    OPEN.with(|o| matches!(*o.borrow(), Some((Target::ChannelBell(_), _))))
}

const DURATIONS: [(&str, Option<u32>); 6] = [
    ("menu.mute.15m", Some(900)),
    ("menu.mute.1h", Some(3600)),
    ("menu.mute.3h", Some(10800)),
    ("menu.mute.8h", Some(28800)),
    ("menu.mute.24h", Some(86400)),
    ("menu.mute.forever", None),
];

type Ent = gpui::Entity<PhoenixApp>;

fn act(e: &Ent, label: gpui::SharedString, f: impl Fn(&mut PhoenixApp, &mut Window, &mut Context<PhoenixApp>) + 'static) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |_, w, cx: &mut App| e.update(cx, |a, cx| f(a, w, cx)))
}

fn copy(label: gpui::SharedString, text: String) -> MenuItem {
    MenuItem::action(label, None, move |_, _, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

/// "Mute X >" submenu, or a plain "Unmute X" row when already muted.
fn mute_item(e: &Ent, label: gpui::SharedString, unmute: gpui::SharedString, muted: bool, guild: Option<Id>, channel: Option<Id>) -> MenuItem {
    if muted {
        let e = e.clone();
        return MenuItem::action(unmute, None, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.set_nav_mute(guild, channel, false, None, cx))
        });
    }
    let items = DURATIONS
        .iter()
        .map(|&(l, secs)| {
            let e = e.clone();
            MenuItem::action(crate::t!(l), None, move |_, _, cx: &mut App| e.update(cx, |a, cx| a.set_nav_mute(guild, channel, true, secs, cx)))
        })
        .collect();
    MenuItem::submenu(label, items)
}

fn level_code(l: crate::notifications::Level) -> u8 {
    use crate::notifications::Level::*;
    match l {
        All => 0,
        Mentions => 1,
        Nothing => 2,
        Inherit => 3,
    }
}

/// The account's stored level for this scope (`user_guild_settings`); a
/// guild without an entry reads its server default, like the client's `W(e)`.
fn stored_level(app: &PhoenixApp, guild: Option<Id>, channel: Option<Id>) -> Option<u8> {
    let p = app.backend.notify_prefs()?;
    let g = guild.map(|g| g.0);
    Some(match channel {
        Some(ch) => p.guilds.get(&g.unwrap_or(0))?.channels.get(&ch.0).map_or(3, |c| level_code(c.level)),
        None => level_code(p.guild(g).level),
    })
}

fn guild_muted(app: &PhoenixApp, g: Id) -> bool {
    MUTED_GUILDS.with(|m| m.borrow().get(&g).copied()).unwrap_or_else(|| {
        app.backend.notify_prefs().is_some_and(|p| p.is_guild_muted(Some(g.0)))
    })
}

fn notify_radios(app: &PhoenixApp, e: &Ent, guild: Option<Id>, channel: Option<Id>) -> Vec<MenuItem> {
    let default = if channel.is_some() { 3 } else { 0 };
    let cur = LEVELS
        .with(|l| l.borrow().get(&(guild, channel)).copied())
        .or_else(|| stored_level(app, guild, channel))
        .unwrap_or(default);
    let mut opts: Vec<(&str, u8)> = vec![("menu.notify.all", 0), ("menu.notify.mentions", 1), ("menu.notify.nothing", 2)];
    // vanilla-menu-channel-notify: "Use Category Default" when the channel sits
    // in a category, else "Use Server Default"; its subtext is the inherited level.
    if let Some(ch) = channel {
        let in_category = app.channels.get(&ch).and_then(|c| c.parent).is_some();
        opts.insert(0, (if in_category { "menu.notify.default_category" } else { "menu.notify.default" }, 3));
    }
    // "Use Category Default" shows what the category resolves to.
    let parent = channel.and_then(|c| app.channels.get(&c)).and_then(|c| c.parent);
    let inherited = level_label(app, guild, parent);
    opts.into_iter()
        .map(|(l, lv)| {
            let e = e.clone();
            let item = MenuItem::radio(crate::t!(l), cur == lv, move |_, _, cx: &mut App| {
                LEVELS.with(|m| m.borrow_mut().insert((guild, channel), lv));
                e.update(cx, |a, cx| a.nav_request(NavRequest::NotifyLevel { guild, channel, level: lv }, cx, |_, _, _| {}))
            });
            if lv == 3 { item.with_subtext(inherited.clone()) } else { item }
        })
        .collect()
}

fn notify_item(app: &PhoenixApp, e: &Ent, guild: Option<Id>, channel: Option<Id>) -> MenuItem {
    MenuItem::submenu(crate::t!("menu.notify_settings"), notify_radios(app, e, guild, channel))
}

/// Label of the level a scope resolves to (the "Notification Settings" subtext):
/// `resolvedMessageNotifications`, channel then parent category then server,
/// with the levels picked in this session first.
fn level_label(app: &PhoenixApp, guild: Option<Id>, channel: Option<Id>) -> gpui::SharedString {
    let level = |c: Option<Id>| LEVELS.with(|l| l.borrow().get(&(guild, c)).copied()).or_else(|| stored_level(app, guild, c));
    let parent = channel.and_then(|c| app.channels.get(&c)).and_then(|c| c.parent);
    let lv = [channel, parent]
        .into_iter()
        .flatten()
        .map(|c| level(Some(c)))
        .find_map(|l| l.filter(|l| *l != 3))
        .or_else(|| level(None))
        .unwrap_or(0);
    crate::t!(match lv {
        0 => "menu.notify.all",
        2 => "menu.notify.nothing",
        _ => "menu.notify.mentions",
    })
}

// Permission bits used to show the admin groups like the client does.
const MANAGE_CHANNELS: u64 = 1 << 4;
const MANAGE_GUILD: u64 = 1 << 5;
const MANAGE_EVENTS: u64 = 1 << 33;

fn perms(app: &PhoenixApp, g: Id, bit: u64) -> bool {
    crate::ui::inbox::has_perm(app.backend.guild_permissions(g), bit)
}

/// Dropdown under the guild header (live `guild-header-popout`, 2026-11-24):
/// Server Boost | Invite to Server, Server Settings, Server Analytics, Create
/// Channel, Create Category, Create Event, Active Threads, App Directory | Show All
/// Channels, Notification Settings, Privacy Settings | Edit Per-server Profile,
/// Hide Muted Channels | Security Actions, Report Raid | Leave Server | Copy ID.
pub fn guild_dropdown(app: &PhoenixApp, g: Id, cx: &mut Context<PhoenixApp>) -> Vec<MenuItem> {
    let e = cx.entity();
    let mut items = Vec::new();
    let owner = app.guilds.iter().find(|x| x.id == g).and_then(|x| x.owner) == Some(app.me.id);
    let (manage_guild, manage_channels, events) = (perms(app, g, MANAGE_GUILD), perms(app, g, MANAGE_CHANNELS), perms(app, g, MANAGE_EVENTS));
    items.push(stub(&e, "settings.page.server_boost"));
    items.push(MenuItem::Separator);
    items.push(act(&e, crate::t!("menu.invite_to_server"), move |a, _, cx| a.open_invite_people(g, cx)));
    if manage_guild {
        items.push(act(&e, crate::t!("menu.server_settings"), move |a, _, cx| a.open_server_settings(g, 0, cx)));
        items.push(stub(&e, "menu.gs.analytics"));
    }
    if manage_channels {
        items.push(act(&e, crate::t!("menu.create_channel"), move |a, _, cx| a.open_create_channel(g, None, crate::model::ChannelKind::Text, cx)));
        items.push(act(&e, crate::t!("menu.create_category"), move |a, _, cx| a.open_create_channel(g, None, crate::model::ChannelKind::Category, cx)));
    }
    if events {
        items.push(act(&e, crate::t!("menu.create_event"), move |a, _, cx| a.open_create_event(g, cx)));
    }
    items.push(stub(&e, "menu.active_threads"));
    items.push(stub(&e, "menu.gs.app_directory"));
    items.push(MenuItem::Separator);
    items.extend(crate::actions::channel::show_all_item(app, &e, g));
    // A plain row that opens the notification settings modal (no submenu caret,
    // which ellipsized the label in the 220px menu).
    items.push(act(&e, crate::t!("menu.notify_settings"), move |a, _, cx| {
        let st = crate::ui::modals::guild_notify::State::new(a, g);
        crate::actions::open_modal(crate::ui::modals::Modal::GuildNotify(st), None);
        cx.notify();
    }));
    items.push(act(&e, crate::t!("menu.privacy_settings"), move |a, _, cx| a.open_privacy_settings(g, cx)));
    items.push(MenuItem::Separator);
    items.push(act(&e, crate::t!("menu.edit_server_profile"), move |a, _, cx| a.open_server_profile(g, cx)));
    items.push(MenuItem::check(crate::t!("menu.hide_muted"), app.nav.hide_muted.contains(&g), {
        let e = e.clone();
        move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| {
                if !a.nav.hide_muted.remove(&g) {
                    a.nav.hide_muted.insert(g);
                }
                a.rebuild_sidebar();
                a.save_nav_prefs();
                cx.notify();
            })
        }
    }));
    if manage_guild {
        items.push(MenuItem::Separator);
        items.push(MenuItem::danger(crate::t!("menu.security_actions"), None, {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.open_security_actions(g, cx))
        }));
        items.push(MenuItem::danger(crate::t!("menu.report_raid"), None, {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.stub(&crate::t!("menu.report_raid"), cx))
        }));
    }
    if !owner {
        items.push(MenuItem::Separator);
        items.push(MenuItem::danger(crate::t!("menu.leave_server"), None, {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.show_overlay(Overlay::LeaveGuild(g), cx))
        }));
    }
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        items.push(copy(crate::t!("menu.copy_server_id"), g.0.to_string()).with_icon("id"));
    }
    items
}

/// Stub row: the client action exists but Phoenix doesn't back it yet.
fn stub(e: &Ent, key: &'static str) -> MenuItem {
    act(e, crate::t!(key), move |a, _, cx| a.stub(&crate::t!(key), cx))
}

/// "Move To…" > [Category|Folder] > | First | Last (vanilla `*-context-move-to`).
fn move_to(e: &Ent, folder: bool) -> MenuItem {
    let (to, first, last) = if folder {
        ("menu.move_folder", "menu.move_first_folder", "menu.move_last_folder")
    } else {
        ("menu.move_category", "menu.move_first", "menu.move_last")
    };
    MenuItem::submenu(
        crate::t!("menu.move_to"),
        vec![MenuItem::submenu(crate::t!(to), vec![stub(e, "menu.apps_none")]), MenuItem::Separator, stub(e, first), stub(e, last)],
    )
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (target, pos) = OPEN.with(|o| *o.borrow())?;
    let e = cx.entity();
    let dev = app.prefs.synced.developer_mode;
    let mut items = Vec::new();
    match target {
        Target::Channel(id) => {
            let ch = app.channels.get(&id)?;
            let (guild, muted) = (ch.guild, ch.muted);
            // Live channel-context: "Mark As Read" is disabled_M3qk4j when nothing is unread.
            if ch.unread || ch.mentions > 0 {
                items.push(act(&e, crate::t!("menu.mark_read"), move |a, _, cx| a.mark_nav_read(Target::Channel(id), cx)));
            } else {
                items.push(MenuItem::disabled(crate::t!("menu.mark_read"), None));
            }
            items.push(MenuItem::Separator);
            if let Some(g) = guild {
                // Live "channel-context" (text channel) and category menus, 2026-11-23:
                // Mark As Read | Add to Favorites > | Invite to Channel, Pin Channel
                // to Top, Copy Link | Mute Channel >, Notification Settings > |
                // Move To… > | Edit, Clone, Create Text Channel, Delete | Copy ID.
                // Category: Mark As Read | Collapse Category, Collapse All | Mute
                // Category >, Notification Settings > | Move To… > | Edit, Delete | Copy ID.
                let cat = ch.kind == ChannelKind::Category;
                // Live voice-channel menu (dumps/voice-channel-context-menu, 2026-11-24):
                // same as text but "Invite to Voice Channel", then | Open Chat, Hide
                // Names (checkbox) |, no Notification Settings, "Create Voice Channel".
                let voice = ch.kind == ChannelKind::Voice;
                let manage = perms(app, g, MANAGE_CHANNELS);
                if cat {
                    let collapsed = app.collapsed.contains(&id);
                    items.push(act(
                        &e,
                        crate::t!(if collapsed { "menu.expand_category" } else { "menu.collapse_category" }),
                        move |a, _, cx| a.toggle_category_saved(id, cx),
                    ));
                    items.push(act(&e, crate::t!("menu.collapse_all"), move |a, _, cx| {
                        let cats: Vec<Id> = a
                            .guild_channels
                            .get(&g)
                            .into_iter()
                            .flatten()
                            .filter(|c| a.channels.get(c).is_some_and(|c| c.kind == ChannelKind::Category))
                            .copied()
                            .collect();
                        for c in cats {
                            if !a.collapsed.contains(&c) {
                                a.toggle_category_saved(c, cx);
                            }
                        }
                    }));
                } else {
                    items.push(crate::actions::channel::favorite_item(&e, id));
                    items.push(MenuItem::Separator);
                    let invite = if voice { "menu.invite_voice_channel" } else { "menu.invite_channel" };
                    items.push(act(&e, crate::t!(invite), move |a, _, cx| a.open_invite_people(g, cx)));
                    items.push(crate::actions::channel::pin_item(&e, id, false));
                    items.push(copy(crate::t!("menu.copy_link"), format!("https://discord.com/channels/{}/{}", g.0, id.0)));
                    if voice {
                        items.push(MenuItem::Separator);
                        items.push(act(&e, crate::t!("menu.open_voice_chat"), move |a, _, cx| a.open_channel_chat(id, cx)));
                        let hidden = crate::ui::voice_row::names_hidden(id);
                        let e2 = e.clone();
                        items.push(MenuItem::check(crate::t!("menu.hide_voice_names"), hidden, move |_, _, cx: &mut App| {
                            crate::ui::voice_row::set_names_hidden(id, !hidden);
                            e2.update(cx, |a, cx| {
                                a.rebuild_sidebar();
                                cx.notify();
                            });
                        }));
                    }
                }
                items.push(MenuItem::Separator);
                let (mute, unmute) = if cat {
                    (crate::t!("menu.mute_category"), crate::t!("menu.unmute_category"))
                } else {
                    (crate::t!("menu.mute_channel"), crate::t!("menu.unmute_channel"))
                };
                items.push(mute_item(&e, mute, unmute, muted, guild, Some(id)));
                if !voice {
                    items.push(notify_item(app, &e, guild, Some(id)).with_subtext(level_label(app, guild, Some(id))));
                }
                if manage {
                    items.push(MenuItem::Separator);
                    items.push(if cat { move_to(&e, false) } else { crate::actions::channel::move_to_item(app, &e, g, id) });
                    items.push(MenuItem::Separator);
                    if cat {
                        items.push(act(&e, crate::t!("menu.edit_category"), move |a, _, cx| a.open_channel_settings(id, cx)));
                        items.push(MenuItem::danger(crate::t!("menu.delete_category"), None, {
                            let e = e.clone();
                            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.confirm_delete_channel(id, cx))
                        }));
                    } else {
                        items.push(act(&e, crate::t!("menu.edit_channel"), move |a, _, cx| a.open_channel_settings(id, cx)));
                        items.push(act(&e, crate::t!("menu.clone_channel"), move |a, _, cx| a.open_clone_channel(g, id, cx)));
                        let parent = app.channels.get(&id).and_then(|c| c.parent);
                        let (create, kind) = if voice {
                            ("menu.create_voice_channel", ChannelKind::Voice)
                        } else {
                            ("menu.create_text_channel", ChannelKind::Text)
                        };
                        items.push(act(&e, crate::t!(create), move |a, _, cx| a.open_create_channel(g, parent, kind, cx)));
                        items.push(MenuItem::danger(crate::t!("menu.delete_channel"), None, {
                            let e = e.clone();
                            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.confirm_delete_channel(id, cx))
                        }));
                    }
                }
                if dev {
                    items.push(MenuItem::Separator);
                    let key = if cat { "menu.copy_category_id" } else { "menu.copy_channel_id" };
                    items.push(copy(crate::t!(key), id.0.to_string()).with_icon("id"));
                }
            } else {
                // Live DM row menu ("user-context", 2026-11-23): Mark As Read |
                // Pin, Add to Favorites > | Profile, Call, Add Note (subtext), Add
                // Friend Nickname, Close DM | Apps >, Invite to Server >, Remove
                // Friend, Ignore, Block | Mute @user > | Copy User ID, Copy Channel ID.
                // Group DM ("gdm-context"): Mark As Read | Pin, Add to Favorites > |
                // Invites, Edit Group | Mute Conversation > | Leave Group | Copy Channel ID.
                let peer = ch.voice_users.first().copied();
                let is_group = app.nav.extras.get(&id).is_some_and(|x| x.recipients > 1);
                items.push(crate::actions::channel::pin_item(&e, id, true).with_icon("pin"));
                items.push(crate::actions::channel::favorite_item(&e, id));
                items.push(MenuItem::Separator);
                if is_group {
                    items.push(stub(&e, "menu.invites"));
                    items.push(stub(&e, "menu.edit_group"));
                    items.push(MenuItem::Separator);
                    items.push(mute_item(&e, crate::t!("menu.mute_conversation"), crate::t!("menu.unmute_conversation"), muted, None, Some(id)));
                    items.push(MenuItem::Separator);
                    items.push(MenuItem::danger(crate::t!("menu.leave_group"), None, {
                        let e = e.clone();
                        move |_, _, cx: &mut App| e.update(cx, |a, cx| a.close_dm(id, cx))
                    }));
                } else {
                    if let Some(u) = peer {
                        items.push(act(&e, crate::t!("menu.profile"), move |a, _, cx| a.open_profile_modal(u, cx)));
                        items.push(act(&e, crate::t!("menu.call"), move |a, _, cx| a.open_call_ui(u, cx)));
                        items.push(act(&e, crate::t!(crate::actions::user::note_label(app, u)), move |a, _, cx| a.open_profile_modal(u, cx)).with_subtext(crate::t!("menu.note_subtext")));
                        if app.nav.relationship(u).is_some() {
                            items.push(act(&e, crate::t!("menu.friend_nickname"), move |a, _, cx| a.open_friend_nickname(u, cx)));
                        }
                    }
                    items.push(act(&e, crate::t!("menu.close_dm"), move |a, _, cx| a.close_dm(id, cx)));
                    if let Some(u) = peer {
                        items.push(MenuItem::Separator);
                        items.push(MenuItem::submenu(crate::t!("menu.apps"), vec![stub(&e, "menu.apps_none")]));
                        let invites: Vec<MenuItem> = app
                            .guilds
                            .iter()
                            .map(|x| {
                                let gid = x.id;
                                act(&e, x.name.clone(), move |a, _, cx| a.open_invite_people(gid, cx))
                            })
                            .collect();
                        if !invites.is_empty() {
                            items.push(MenuItem::submenu(crate::t!("menu.invite_to_server"), invites));
                        }
                        let rel = |action: fn() -> crate::ui::home::data::RelAction| {
                            move |a: &mut PhoenixApp, _: &mut Window, cx: &mut Context<PhoenixApp>| {
                                a.nav_request(NavRequest::Relationship { user: u, action: action() }, cx, |_, _, _| {});
                                cx.notify();
                            }
                        };
                        if app.nav.relationship(u).is_some() {
                            items.push(act(&e, crate::t!("menu.remove_friend"), rel(|| crate::ui::home::data::RelAction::Remove)));
                        }
                        let ign = if crate::actions::user::is_ignored(u) { "menu.unignore" } else { "menu.ignore" };
                        items.push(act(&e, crate::t!(ign), move |a, _, cx| a.toggle_ignore(u, cx)));
                        let block = rel(|| crate::ui::home::data::RelAction::Block);
                        let eb = e.clone();
                        items.push(MenuItem::danger(crate::t!("menu.block"), None, move |_, w, cx: &mut App| eb.update(cx, |a, cx| block(a, w, cx))));
                    }
                    items.push(MenuItem::Separator);
                    let who = format!("@{}", ch.name);
                    items.push(mute_item(&e, crate::t!("menu.mute_who", who = who), crate::t!("menu.unmute_who", who = who), muted, None, Some(id)));
                }
                if dev {
                    items.push(MenuItem::Separator);
                    if let Some(u) = peer.filter(|_| !is_group) {
                        items.push(copy(crate::t!("common.copy_user_id"), u.0.to_string()).with_icon("id"));
                    }
                    items.push(copy(crate::t!("menu.copy_channel_id"), id.0.to_string()).with_icon("id"));
                }
            }
        }
        Target::Guild(g) => {
            // Live "guild-context" (2026-11-23): Mark As Read | Invite to Server |
            // Mute Server >, Notification Settings >, Hide Muted Channels, Show All
            // Channels | Server Settings > (Manage Server), Privacy Settings, Edit
            // Per-server Profile | Create Channel, Create Category, Create Event
            // (permissions) | Move To… > | Security Actions (Manage Server), Leave
            // Server (not for the owner) | Copy Server ID.
            let muted = guild_muted(app, g);
            let hide = app.nav.hide_muted.contains(&g);
            let owner = app.guilds.iter().find(|x| x.id == g).and_then(|x| x.owner) == Some(app.me.id);
            let (manage_guild, manage_channels, events) =
                (perms(app, g, MANAGE_GUILD), perms(app, g, MANAGE_CHANNELS), perms(app, g, MANAGE_EVENTS));
            if app.guilds.iter().find(|x| x.id == g).is_some_and(|x| x.unread || x.mentions > 0) {
                items.push(act(&e, crate::t!("menu.mark_read"), move |a, _, cx| a.mark_nav_read(Target::Guild(g), cx)));
            } else {
                items.push(MenuItem::disabled(crate::t!("menu.mark_read"), None));
            }
            items.push(MenuItem::Separator);
            items.push(act(&e, crate::t!("menu.invite_to_server"), move |a, _, cx| a.open_invite_people(g, cx)));
            items.push(MenuItem::Separator);
            items.push(mute_item(&e, crate::t!("menu.mute_server"), crate::t!("menu.unmute_server"), muted, Some(g), None));
            items.push(notify_item(app, &e, Some(g), None).with_subtext(level_label(app, Some(g), None)));
            items.push(MenuItem::check(crate::t!("menu.hide_muted"), hide, {
                let e = e.clone();
                move |_, _, cx: &mut App| {
                    e.update(cx, |a, cx| {
                        if !a.nav.hide_muted.remove(&g) {
                            a.nav.hide_muted.insert(g);
                        }
                        a.rebuild_sidebar();
                        a.save_nav_prefs();
                        cx.notify();
                    })
                }
            }));
            items.extend(crate::actions::channel::show_all_item(app, &e, g));
            items.push(MenuItem::Separator);
            if manage_guild {
                items.push(crate::actions::channel::server_settings_item(&e, g));
            }
            items.push(act(&e, crate::t!("menu.privacy_settings"), move |a, _, cx| a.open_privacy_settings(g, cx)));
            items.push(act(&e, crate::t!("menu.edit_server_profile"), move |a, _, cx| a.open_server_profile(g, cx)));
            if manage_channels || events {
                items.push(MenuItem::Separator);
                if manage_channels {
                    items.push(act(&e, crate::t!("menu.create_channel"), move |a, _, cx| a.open_create_channel(g, None, crate::model::ChannelKind::Text, cx)));
                    items.push(act(&e, crate::t!("menu.create_category"), move |a, _, cx| a.open_create_channel(g, None, crate::model::ChannelKind::Category, cx)));
                }
                if events {
                    items.push(act(&e, crate::t!("menu.create_event"), move |a, _, cx| a.open_create_event(g, cx)));
                }
            }
            items.push(MenuItem::Separator);
            items.push(move_to(&e, true));
            items.push(MenuItem::Separator);
            if manage_guild {
                items.push(MenuItem::danger(crate::t!("menu.security_actions"), None, {
                    let e = e.clone();
                    move |_, _, cx: &mut App| e.update(cx, |a, cx| a.open_security_actions(g, cx))
                }));
            }
            if !owner {
                items.push(MenuItem::danger(crate::t!("menu.leave_server"), None, {
                    let e = e.clone();
                    move |_, _, cx: &mut App| e.update(cx, |a, cx| a.show_overlay(Overlay::LeaveGuild(g), cx))
                }));
            }
            if dev {
                items.push(MenuItem::Separator);
                items.push(copy(crate::t!("menu.copy_server_id"), g.0.to_string()).with_icon("id"));
            }
        }
        Target::ChannelBell(id) => {
            let Some(ch) = app.channels.get(&id) else { return None };
            let guild = ch.guild;
            let muted = ch.muted;
            items.push(mute_item(
                &e,
                crate::t!("menu.mute_channel"),
                crate::t!("menu.unmute_channel"),
                muted,
                guild,
                Some(id),
            ));
            items.push(MenuItem::Separator);
            items.extend(notify_radios(app, &e, guild, Some(id)));
        }
        Target::Thread(id) => {
            let live = app.channels.get(&id);
            let guild = live.and_then(|c| c.guild).or(app.selected_guild);
            let unread = live.is_some_and(|c| c.unread);
            let muted = live.is_some_and(|c| c.muted);
            if unread {
                items.push(act(&e, crate::t!("menu.mark_read"), move |a, _, cx| a.mark_nav_read(Target::Channel(id), cx)));
            } else {
                items.push(MenuItem::disabled(crate::t!("menu.mark_read"), None));
            }
            items.push(MenuItem::Separator);
            items.push(crate::actions::channel::favorite_item(&e, id));
            if let Some(g) = guild {
                items.push(MenuItem::Separator);
                items.push(copy(crate::t!("menu.copy_link"), format!("https://discord.com/channels/{}/{}", g.0, id.0)));
            }
            items.push(MenuItem::Separator);
            items.push(mute_item(
                &e,
                crate::t!("menu.mute_channel"),
                crate::t!("menu.unmute_channel"),
                muted,
                guild,
                Some(id),
            ));
            items.push(notify_item(app, &e, guild, Some(id)).with_subtext(level_label(app, guild, Some(id))));
            if dev {
                items.push(MenuItem::Separator);
                items.push(copy(crate::t!("menu.copy_thread_id"), id.0.to_string()).with_icon("id"));
            }
        }
        Target::Folder(f) => {
            // Live folder menu: Mark Folder As Read | Move To… > | Folder
            // Settings, Close All Folders.
            items.push(act(&e, crate::t!("menu.mark_folder_read"), move |a, _, cx| {
                let gs: Vec<Id> = a.nav.folders.iter().filter(|x| x.id == Some(f)).flat_map(|x| x.guilds.clone()).collect();
                for g in gs {
                    a.mark_nav_read(Target::Guild(g), cx);
                }
            }));
            items.push(MenuItem::Separator);
            items.push(move_to(&e, true));
            items.push(MenuItem::Separator);
            items.push(stub(&e, "menu.folder_settings"));
            items.push(act(&e, crate::t!("menu.close_all_folders"), |a, _, cx| {
                a.nav.expanded_folders.clear();
                a.save_nav_prefs();
                cx.notify();
            }));
        }
    }
    let dismiss: context_menu::Dismiss = Rc::new(|w, _| {
        OPEN.with(|o| *o.borrow_mut() = None);
        w.refresh();
    });
    Some(context_menu::render(pos, items, dismiss))
}

impl PhoenixApp {
    pub fn mark_nav_read(&mut self, target: Target, cx: &mut Context<Self>) {
        let req = match target {
            Target::Channel(id) | Target::ChannelBell(id) | Target::Thread(id) => {
                if let Some(ch) = self.channels.get_mut(&id) {
                    ch.unread = false;
                    ch.mentions = 0;
                }
                NavRequest::AckChannel(id)
            }
            Target::Folder(_) => return,
            Target::Guild(g) => {
                for id in self.guild_channels.get(&g).cloned().unwrap_or_default() {
                    if let Some(ch) = self.channels.get_mut(&id) {
                        ch.unread = false;
                        ch.mentions = 0;
                    }
                }
                if let Some(guild) = self.guilds.iter_mut().find(|x| x.id == g) {
                    guild.unread = false;
                    guild.mentions = 0;
                }
                NavRequest::AckGuild(g)
            }
        };
        self.rebuild_sidebar();
        self.nav_request(req, cx, |_, _, _| {});
        cx.notify();
    }

    fn set_nav_mute(&mut self, guild: Option<Id>, channel: Option<Id>, muted: bool, secs: Option<u32>, cx: &mut Context<Self>) {
        match channel {
            Some(id) => {
                if let Some(ch) = self.channels.get_mut(&id) {
                    ch.muted = muted;
                }
            }
            None => {
                if let Some(g) = guild {
                    MUTED_GUILDS.with(|m| m.borrow_mut().insert(g, muted));
                }
            }
        }
        self.rebuild_sidebar();
        self.nav_request(NavRequest::Mute { guild, channel, muted, secs }, cx, |_, _, _| {});
        cx.notify();
    }
}
