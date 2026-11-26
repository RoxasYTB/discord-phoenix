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
    ChannelBell(Id),
    InboxBell(Id),
    Thread(Id),
    Guild(Id),
    Folder(u64),
    Favorites,
    FavoritesAdd,
}

type Scope = (Option<Id>, Option<Id>);

thread_local! {
    static OPEN: RefCell<Option<(Target, Point<Pixels>)>> = const { RefCell::new(None) };
    static MUTED_GUILDS: RefCell<HashMap<Id, bool>> = RefCell::new(HashMap::new());
    static LEVELS: RefCell<HashMap<Scope, u8>> = RefCell::new(HashMap::new());
}

pub fn open(target: Target, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = Some((target, pos)));
    window.refresh();
}

pub fn toggle_bell(channel: Id, pos: Point<Pixels>, window: &mut Window) {
    toggle(Target::ChannelBell(channel), pos, window);
}

pub fn toggle_inbox_bell(channel: Id, pos: Point<Pixels>, window: &mut Window) {
    toggle(Target::InboxBell(channel), pos, window);
}

fn toggle(target: Target, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| {
        let mut cur = o.borrow_mut();
        if cur.is_some_and(|(t, _)| t == target) {
            *cur = None;
        } else {
            *cur = Some((target, pos));
        }
    });
    window.refresh();
}

pub fn is_inbox_bell_open(channel: Id) -> bool {
    OPEN.with(|o| matches!(*o.borrow(), Some((Target::InboxBell(c), _)) if c == channel))
}

pub fn any_inbox_bell_open() -> bool {
    OPEN.with(|o| matches!(*o.borrow(), Some((Target::InboxBell(_), _))))
}

pub fn close_bell() -> bool {
    OPEN.with(|o| {
        let mut cur = o.borrow_mut();
        if matches!(*cur, Some((Target::ChannelBell(_) | Target::InboxBell(_), _))) {
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
    if let Some(ch) = channel {
        let in_category = app.channels.get(&ch).and_then(|c| c.parent).is_some();
        opts.insert(0, (if in_category { "menu.notify.default_category" } else { "menu.notify.default" }, 3));
    }
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

const MANAGE_CHANNELS: u64 = 1 << 4;
const MANAGE_GUILD: u64 = 1 << 5;
const MANAGE_EVENTS: u64 = 1 << 33;

fn perms(app: &PhoenixApp, g: Id, bit: u64) -> bool {
    crate::ui::inbox::has_perm(app.backend.guild_permissions(g), bit)
}

pub fn guild_dropdown(app: &PhoenixApp, g: Id, cx: &mut Context<PhoenixApp>) -> Vec<MenuItem> {
    let e = cx.entity();
    let mut items = Vec::new();
    let owner = app.guilds.iter().find(|x| x.id == g).and_then(|x| x.owner) == Some(app.me.id);
    let (manage_guild, manage_channels, events) = (perms(app, g, MANAGE_GUILD), perms(app, g, MANAGE_CHANNELS), perms(app, g, MANAGE_EVENTS));
    let community = app.guilds.iter().any(|x| x.id == g && x.community);
    items.push(
        act(&e, crate::t!("settings.page.server_boost"), move |_, _, cx| {
            crate::ui::boost_layer::open(g);
            cx.notify();
        })
        .with_icon("chat/boost"),
    );
    items.push(MenuItem::Separator);
    items.push(act(&e, crate::t!("menu.invite_to_server"), move |a, _, cx| a.open_invite_people(g, cx)).with_icon("add-to-dm"));
    if manage_guild {
        items.push(act(&e, crate::t!("menu.server_settings"), move |a, _, cx| a.open_server_settings(g, 0, cx)).with_icon("gear"));
    }
    if crate::ui::modals::server_settings::visible(app, g, "menu.gs.analytics") {
        let section = crate::ui::modals::server_settings::section_index("menu.gs.analytics");
        items.push(act(&e, crate::t!("menu.gs.analytics"), move |a, _, cx| a.open_server_settings(g, section, cx)).with_icon("picker/trending"));
    }
    if manage_channels {
        items.push(act(&e, crate::t!("menu.create_channel"), move |a, _, cx| a.open_create_channel(g, None, crate::model::ChannelKind::Text, cx)).with_icon("plus-circle"));
        items.push(
            act(&e, crate::t!("menu.create_category"), move |a, _, cx| a.open_create_channel(g, None, crate::model::ChannelKind::Category, cx))
                .with_icon("menu/folder-plus"),
        );
    }
    if events {
        items.push(act(&e, crate::t!("menu.create_event"), move |a, _, cx| a.open_create_event(g, cx)).with_icon("inbox/schedule"));
    }
    if !crate::ui::modals::active_threads::active_threads(app, g).is_empty() {
        items.push(act(&e, crate::t!("menu.active_threads"), move |a, _, cx| a.open_active_threads(g, cx)).with_icon("chat"));
    }
    items.push(act(&e, crate::t!("menu.gs.app_directory"), |_, _, cx| crate::ui::modals::server_settings::open_app_directory(cx)).with_icon("apps"));
    items.push(MenuItem::Separator);
    items.extend(crate::actions::channel::show_all_item(app, &e, g).map(|i| i.with_icon("chat/eye")));
    items.push(
        act(&e, crate::t!("menu.notify_settings"), move |a, _, cx| {
            let st = crate::ui::modals::guild_notify::State::new(a, g);
            crate::actions::open_modal(crate::ui::modals::Modal::GuildNotify(st), None);
            cx.notify();
        })
        .with_icon("bell"),
    );
    items.push(act(&e, crate::t!("menu.privacy_settings"), move |a, _, cx| a.open_privacy_settings(g, cx)).with_icon("shield"));
    items.push(MenuItem::Separator);
    items.push(act(&e, crate::t!("menu.edit_server_profile"), move |a, _, cx| a.open_server_profile(g, cx)).with_icon("pen"));
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
    })
    .with_icon("chat/eye-slash"));
    let mut danger = Vec::new();
    if manage_guild && community {
        danger.push(MenuItem::danger(crate::t!("menu.security_actions"), Some(crate::icons::path("lock")), {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.open_security_actions(g, cx))
        }));
    }
    if crate::ui::modals::report_raid::visible(app, g) {
        danger.push(MenuItem::danger(crate::t!("menu.report_raid"), Some(crate::icons::path("shield")), {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.open_report_raid(g, cx))
        }));
    }
    if !owner {
        danger.push(MenuItem::danger(crate::t!("menu.leave_server"), Some(crate::icons::path("door-exit")), {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.show_overlay(Overlay::LeaveGuild(g), cx))
        }));
    }
    if !danger.is_empty() {
        items.push(MenuItem::Separator);
        items.extend(danger);
    }
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        items.push(copy(crate::t!("menu.copy_server_id"), g.0.to_string()).with_icon("id"));
    }
    items
}

pub fn favorites_items(e: &Ent) -> Vec<MenuItem> {
    let hide = e.clone();
    vec![
        act(e, crate::t!("fav.thread_settings"), |a, _, cx| crate::ui::favorites::open_thread_settings(a, cx)),
        MenuItem::Separator,
        MenuItem::danger(crate::t!("fav.hide"), None, move |_, _, cx: &mut App| hide.update(cx, crate::ui::favorites::hide)),
    ]
}

pub fn favorites_add_items(e: &Ent) -> Vec<MenuItem> {
    vec![
        act(e, crate::t!("fav.add_title"), |_, _, cx| {
            crate::ui::favorites::open_add_modal();
            cx.notify();
        }),
        MenuItem::Separator,
        act(e, crate::t!("fav.create_category"), |_, _, cx| crate::ui::favorites::open_create_category(cx)),
    ]
}


fn rail_move(app: &PhoenixApp, e: &Ent, item: crate::ui::modals::gdm::RailRef) -> Option<MenuItem> {
    use crate::ui::modals::gdm::{Dest, RailRef, placement};
    let tree = crate::ui::inbox::folder_tree(app);
    let (in_folder, first, last) = placement(&tree, item)?;
    let go = |dest: Dest| move |a: &mut PhoenixApp, _: &mut Window, cx: &mut Context<PhoenixApp>| a.move_in_rail(item, dest, cx);
    let place = |label: &'static str, disabled: bool, dest: Dest| {
        if disabled { MenuItem::disabled(crate::t!(label), None) } else { act(e, crate::t!(label), go(dest)) }
    };
    let mut items = Vec::new();
    let folders: Vec<&crate::ui::home::data::GuildFolder> = tree.iter().filter(|f| f.id.is_some()).collect();
    if let RailRef::Guild(g) = item
        && !folders.is_empty()
    {
        let current = tree.iter().find(|f| f.guilds.contains(&g)).and_then(|f| f.id);
        let mut dests = vec![place("menu.no_folder", current.is_none(), Dest::NoFolder), MenuItem::Separator];
        for f in folders {
            let id = f.id.unwrap_or_default();
            let name = f.name.clone().unwrap_or_else(|| crate::t!("folder.unnamed"));
            dests.push(if current == Some(id) { MenuItem::disabled(name, None) } else { act(e, name, go(Dest::Folder(id))) });
        }
        items.push(MenuItem::submenu(crate::t!("menu.move_folder"), dests));
        items.push(MenuItem::Separator);
    }
    let (fl, ll) = if in_folder { ("menu.move_first_folder", "menu.move_last_folder") } else { ("menu.move_first", "menu.move_last") };
    if !(first && last) {
        items.push(place(fl, first, Dest::First));
        items.push(place(ll, last, Dest::Last));
    }
    if matches!(items.last(), Some(MenuItem::Separator)) {
        items.pop();
    }
    (!items.is_empty()).then(|| MenuItem::submenu(crate::t!("menu.move_to"), items))
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (target, pos) = OPEN.with(|o| *o.borrow()).filter(|(t, _)| !matches!(t, Target::InboxBell(_)))?;
    render_target(app, target, pos, cx)
}

pub fn render_inbox_bell(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (target, pos) = OPEN.with(|o| *o.borrow()).filter(|(t, _)| matches!(t, Target::InboxBell(_)))?;
    render_target(app, target, pos, cx)
}

fn render_target(app: &PhoenixApp, target: Target, pos: Point<Pixels>, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let e = cx.entity();
    let dev = app.prefs.synced.developer_mode;
    let mut items = Vec::new();
    match target {
        Target::Channel(id) => {
            let ch = app.channels.get(&id)?;
            let (guild, muted) = (ch.guild, ch.muted);
            if ch.unread || ch.mentions > 0 {
                items.push(act(&e, crate::t!("menu.mark_read"), move |a, _, cx| a.mark_nav_read(Target::Channel(id), cx)));
            } else {
                items.push(MenuItem::disabled(crate::t!("menu.mark_read"), None));
            }
            items.push(MenuItem::Separator);
            if let Some(g) = guild {
                let cat = ch.kind == ChannelKind::Category;
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
                    items.push(crate::actions::channel::favorite_item(app, &e, id));
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
                    if cat {
                        items.extend(crate::actions::channel::category_move_item(app, &e, g, id));
                    } else {
                        items.push(crate::actions::channel::move_to_item(app, &e, g, id));
                    }
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
                let peer = ch.voice_users.first().copied();
                let is_group = app.nav.extras.get(&id).is_some_and(|x| x.recipients > 1);
                items.push(crate::actions::channel::pin_item(&e, id, true).with_icon("pin"));
                items.push(crate::actions::channel::favorite_item(app, &e, id));
                items.push(MenuItem::Separator);
                if is_group {
                    if app.nav.extras.get(&id).and_then(|x| x.owner) == Some(app.me.id) {
                        items.push(act(&e, crate::t!("menu.invites"), move |a, _, cx| a.open_group_invites(id, cx)));
                    }
                    items.push(act(&e, crate::t!("menu.edit_group"), move |a, _, cx| a.open_edit_group(id, cx)));
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
                        if let Some(r) = app.nav.relationship(u).filter(|r| r.kind == crate::ui::home::data::RelationshipKind::Friend) {
                            items.push(act(&e, crate::ui::modals::nickname::friend_title(r.nickname.is_some()), move |a, _, cx| a.open_friend_nickname(u, cx)));
                        }
                    }
                    items.push(act(&e, crate::t!("menu.close_dm"), move |a, _, cx| a.close_dm(id, cx)));
                    if let Some(u) = peer {
                        items.push(MenuItem::Separator);
                        items.push(MenuItem::submenu(crate::t!("menu.apps"), vec![MenuItem::disabled(crate::t!("menu.apps_none"), None)]));
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
                        if app.nav.relationship(u).is_some_and(|r| r.kind == crate::ui::home::data::RelationshipKind::Friend) {
                            items.push(act(&e, crate::t!("menu.remove_friend"), move |a, _, cx| a.confirm_remove_friend(u, cx)));
                        }
                        let ign = if crate::actions::user::is_ignored(u) { "menu.unignore" } else { "menu.ignore" };
                        items.push(act(&e, crate::t!(ign), move |a, _, cx| a.request_ignore(u, cx)));
                        let eb = e.clone();
                        items.push(MenuItem::danger(crate::t!("menu.block"), None, move |_, _, cx: &mut App| eb.update(cx, |a, cx| a.confirm_block(u, cx))));
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
            if let Some(mv) = rail_move(app, &e, crate::ui::modals::gdm::RailRef::Guild(g)) {
                items.push(mv);
                items.push(MenuItem::Separator);
            }
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
        Target::ChannelBell(id) | Target::InboxBell(id) if app.backend.thread_info(id).is_some() => {
            items.push(crate::ui::thread_menu::mute_item(app, &e, id));
            items.push(MenuItem::Separator);
            if let MenuItem::Submenu { items: radios, .. } = crate::ui::thread_menu::notify_item(app, &e, id) {
                items.extend(radios);
            } else {
                items.push(crate::ui::thread_menu::notify_item(app, &e, id));
            }
        }
        Target::ChannelBell(id) | Target::InboxBell(id) => {
            let ch = app.channels.get(&id)?;
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
            if let (Target::InboxBell(_), Some(g)) = (target, guild) {
                items.push(mute_item(&e, crate::t!("menu.mute_server"), crate::t!("menu.unmute_server"), guild_muted(app, g), Some(g), None));
                items.push(MenuItem::Separator);
            }
            items.extend(notify_radios(app, &e, guild, Some(id)));
        }
        Target::Thread(id) => items.extend(crate::ui::thread_menu::items(app, &e, id, crate::actions::thread::Place::ContextMenu)),
        Target::Favorites => items = favorites_items(&e),
        Target::FavoritesAdd => items = favorites_add_items(&e),
        Target::Folder(f) => {
            let members: Vec<Id> = app.nav.folders.iter().filter(|x| x.id == Some(f)).flat_map(|x| x.guilds.clone()).collect();
            let unread = app.guilds.iter().any(|g| members.contains(&g.id) && (g.unread || g.mentions > 0));
            if unread {
                items.push(act(&e, crate::t!("menu.mark_folder_read"), move |a, _, cx| {
                    for g in members.clone() {
                        a.mark_nav_read(Target::Guild(g), cx);
                    }
                }));
            } else {
                items.push(MenuItem::disabled(crate::t!("menu.mark_folder_read"), None));
            }
            items.push(MenuItem::Separator);
            if let Some(mv) = rail_move(app, &e, crate::ui::modals::gdm::RailRef::Folder(f)) {
                items.push(mv);
                items.push(MenuItem::Separator);
            }
            items.push(act(&e, crate::t!("menu.folder_settings"), move |a, _, cx| a.open_folder_settings(f, cx)));
            if !app.nav.expanded_folders.is_empty() {
                items.push(act(&e, crate::t!("menu.close_all_folders"), |a, _, cx| {
                    a.nav.expanded_folders.clear();
                    a.save_nav_prefs();
                    cx.notify();
                }));
            }
        }
    }
    let dismiss: context_menu::Dismiss = Rc::new(|w, _| {
        OPEN.with(|o| *o.borrow_mut() = None);
        w.refresh();
    });
    Some(match target {
        Target::InboxBell(_) => context_menu::render_anchored(pos, gpui::Anchor::TopRight, items, dismiss),
        _ => context_menu::render(pos, items, dismiss),
    })
}

impl PhoenixApp {
    pub fn mark_nav_read(&mut self, target: Target, cx: &mut Context<Self>) {
        let req = match target {
            Target::Channel(id) | Target::ChannelBell(id) | Target::InboxBell(id) | Target::Thread(id) => {
                if let Some(ch) = self.channels.get_mut(&id) {
                    ch.unread = false;
                    ch.mentions = 0;
                }
                NavRequest::AckChannel(id)
            }
            Target::Folder(_) | Target::Favorites | Target::FavoritesAdd => return,
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

    pub(crate) fn set_nav_mute(&mut self, guild: Option<Id>, channel: Option<Id>, muted: bool, secs: Option<u32>, cx: &mut Context<Self>) {
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
