use std::cell::RefCell;

use gpui::{AnyElement, App, ClipboardItem, Context, Entity, Pixels, Point, Window};

use crate::{
    actions::thread::{Place, can_manage},
    app::PhoenixApp,
    model::Id,
    ui::context_menu::{self, MenuItem},
};

thread_local! {
    static OPEN: RefCell<Option<(Id, Place, Point<Pixels>)>> = const { RefCell::new(None) };
    static POST: RefCell<Option<(crate::commands::ForumPost, Id, Point<Pixels>)>> = const { RefCell::new(None) };
}

pub const MUTE_DURATIONS: [(i64, &str); 6] = [
    (900, "menu.mute.15m"),
    (3600, "menu.mute.1h"),
    (10_800, "menu.mute.3h"),
    (28_800, "menu.mute.8h"),
    (86_400, "menu.mute.24h"),
    (-1, "menu.mute.forever"),
];
const NOTIFY: [(u8, &str); 3] = [(2, "menu.notify.all"), (4, "thread.notify_mentions"), (8, "menu.notify.nothing")];

pub fn open_post(post: crate::commands::ForumPost, forum: Id, pos: Point<Pixels>, window: &mut Window) {
    POST.with(|o| *o.borrow_mut() = Some((post, forum, pos)));
    window.refresh();
}

pub fn open(thread: Id, pos: Point<Pixels>, window: &mut Window) {
    open_at(thread, Place::Toolbar, pos, window);
}

pub fn open_at(thread: Id, place: Place, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = Some((thread, place, pos)));
    window.refresh();
}

fn close(w: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = None);
    POST.with(|o| *o.borrow_mut() = None);
    w.refresh();
}

fn copy(label: gpui::SharedString, text: String) -> MenuItem {
    MenuItem::action(label, None, move |_, _, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

fn act(e: &Entity<PhoenixApp>, label: gpui::SharedString, f: impl Fn(&mut PhoenixApp, &mut Window, &mut Context<PhoenixApp>) + 'static) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |_, w, cx: &mut App| e.update(cx, |a, cx| f(a, w, cx)))
}

fn guild_of(app: &PhoenixApp, t: Id) -> Option<Id> {
    app.channels.get(&t).and_then(|c| c.guild).or_else(|| app.nav.find_thread(t).and_then(|c| c.guild)).or(app.selected_guild)
}

fn push_group(items: &mut Vec<MenuItem>, group: Vec<MenuItem>) {
    if group.is_empty() {
        return;
    }
    if !items.is_empty() {
        items.push(MenuItem::Separator);
    }
    items.extend(group);
}

pub fn mute_item(app: &PhoenixApp, e: &Entity<PhoenixApp>, t: Id) -> MenuItem {
    let st = app.thread_state(t);
    let (mute, unmute) = if st.forum_post { ("menu.post.mute", "menu.post.unmute") } else { ("thread.mute", "thread.unmute") };
    if !st.joined {
        return MenuItem::disabled(crate::t!(mute), None);
    }
    if st.member_muted {
        let item = act(e, crate::t!(unmute), move |a, _, cx| a.unmute_thread(t, cx));
        return match st.mute_end.and_then(muted_until) {
            Some(sub) => item.with_subtext(sub),
            None => item,
        };
    }
    let rows = MUTE_DURATIONS.iter().map(|&(secs, key)| act(e, crate::t!(key), move |a, _, cx| a.mute_thread_for(t, secs, cx))).collect();
    MenuItem::submenu(crate::t!(mute), rows)
}

fn muted_until(end_ms: i64) -> Option<String> {
    use chrono::TimeZone;
    let dt = chrono::Local.timestamp_millis_opt(end_ms).single()?;
    let time = if crate::i18n::locale() == "en" { dt.format("%-m/%-d, %-I:%M %p") } else { dt.format("%d/%m %H:%M") };
    Some(crate::t!("thread.muted_until", time = time.to_string()).to_string())
}

pub fn notify_item(app: &PhoenixApp, e: &Entity<PhoenixApp>, t: Id) -> MenuItem {
    let st = app.thread_state(t);
    if !st.joined {
        return MenuItem::disabled(crate::t!("menu.notify_settings"), None);
    }
    let cur = st.notify_level();
    let radios = NOTIFY
        .iter()
        .map(|&(lv, key)| {
            let e = e.clone();
            MenuItem::radio(crate::t!(key), cur == lv, move |_, _, cx: &mut App| e.update(cx, |a, cx| a.set_thread_notify(t, lv, cx)))
        })
        .collect();
    MenuItem::submenu(crate::t!("menu.notify_settings"), radios)
}

pub fn items(app: &PhoenixApp, e: &Entity<PhoenixApp>, t: Id, place: Place) -> Vec<MenuItem> {
    let st = app.thread_state(t);
    let manage = can_manage(app, t);
    let owner = st.owner == Some(app.me.id);
    let guild = guild_of(app, t);
    let link = guild.map(|g| copy(crate::t!("menu.copy_link"), format!("https://discord.com/channels/{}/{}", g.0, t.0)));
    let id_item = app.prefs.synced.developer_mode.then(|| copy(crate::t!("menu.copy_thread_id"), t.0.to_string()).with_icon("id"));
    let archive = (manage || owner).then(|| {
        if st.archived {
            act(e, crate::t!("thread.unarchive"), move |a, _, cx| a.set_thread_archived(t, false, cx))
        } else {
            act(e, crate::t!("thread.archive"), move |a, _, cx| a.set_thread_archived(t, true, cx))
        }
    });
    let lock = manage.then(|| {
        let locked = st.locked;
        act(e, crate::t!(if locked { "thread.unlock" } else { "thread.lock" }), move |a, _, cx| a.set_thread_locked(t, !locked, cx))
    });
    let edit = (manage || owner).then(|| act(e, crate::t!("thread.edit"), move |a, _, cx| a.open_thread_settings(t, cx)));
    let delete = manage.then(|| {
        let e = e.clone();
        MenuItem::danger(crate::t!("thread.delete"), None, move |_, _, cx: &mut App| e.update(cx, |a, cx| a.confirm_delete_thread(t, false, cx)))
    });
    let membership = if st.joined {
        act(e, crate::t!("thread.leave"), move |a, _, cx| a.leave_thread(t, place, cx))
    } else {
        act(e, crate::t!("thread.join"), move |a, _, cx| a.join_thread(t, place, cx))
    };
    let mut out = Vec::new();
    match place {
        Place::ContextMenu => {
            let (unread, _) = crate::ui::sidebar::thread_unread(app, t);
            push_group(&mut out, vec![if unread {
                act(e, crate::t!("menu.mark_read"), move |a, _, cx| a.mark_nav_read(crate::ui::nav_menus::Target::Thread(t), cx))
            } else {
                MenuItem::disabled(crate::t!("menu.mark_read"), None)
            }]);
            push_group(&mut out, vec![crate::actions::channel::favorite_item(app, e, t)]);
            push_group(&mut out, [archive, lock, edit, link].into_iter().flatten().collect());
            push_group(&mut out, vec![mute_item(app, e, t), notify_item(app, e, t)]);
            push_group(&mut out, delete.into_iter().collect());
            push_group(&mut out, vec![membership]);
            push_group(&mut out, id_item.into_iter().collect());
        }
        Place::Toolbar | Place::Sidebar | Place::ForumToolbar => {
            let (join_first, leave_last) = if st.joined { (None, Some(membership)) } else { (Some(membership), None) };
            if place == Place::Sidebar {
                let full = act(e, crate::t!("thread.open_full"), move |a, _, cx| a.open_thread_full(t, cx));
                push_group(&mut out, join_first.into_iter().chain([full]).collect());
                push_group(&mut out, vec![mute_item(app, e, t), notify_item(app, e, t)]);
                push_group(&mut out, edit.into_iter().collect());
                let search = act(e, crate::t!("thread.search"), move |a, w, cx| {
                    a.open_thread_full(t, cx);
                    crate::ui::search::open(a, w, cx);
                })
                .with_icon("viewer/open");
                let pins = act(e, crate::t!("thread.pins"), move |a, _, cx| {
                    a.open_thread_full(t, cx);
                    a.toggle_pins(cx);
                })
                .with_icon("viewer/open");
                push_group(&mut out, vec![search, pins]);
                push_group(&mut out, [archive, lock, delete].into_iter().flatten().collect());
                push_group(&mut out, leave_last.into_iter().collect());
                push_group(&mut out, [link, id_item].into_iter().flatten().collect());
                return out;
            }
            let top = act(e, crate::t!("thread.jump_top"), move |a, _, cx| a.jump_thread_top(t, cx));
            push_group(&mut out, join_first.into_iter().collect());
            let split = act(e, crate::t!("thread.open_split"), move |a, w, cx| a.open_thread_split(t, w, cx));
            push_group(&mut out, [Some(top), edit, Some(split)].into_iter().flatten().collect());
            push_group(&mut out, vec![crate::actions::channel::favorite_item(app, e, t)]);
            push_group(&mut out, [archive, lock, delete].into_iter().flatten().collect());
            push_group(&mut out, leave_last.into_iter().collect());
            push_group(&mut out, [link, id_item].into_iter().flatten().collect());
        }
    }
    out
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if let Some(menu) = render_post(app, cx) {
        return Some(menu);
    }
    let (t, place, pos) = OPEN.with(|o| *o.borrow())?;
    let items = items(app, &cx.entity(), t, place);
    if items.is_empty() {
        return None;
    }
    Some(context_menu::render(pos, items, std::rc::Rc::new(|w: &mut Window, _: &mut App| close(w))))
}

fn render_post(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (p, forum, pos) = POST.with(|o| o.borrow().clone())?;
    let e = cx.entity();
    let t = p.thread;
    let guild = app.channels.get(&forum).and_then(|c| c.guild).or(app.selected_guild);
    let manage = guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 34));
    let owner = p.owner == Some(app.me.id);
    let followed = app.backend.is_thread_joined(t);
    let mut items = Vec::new();
    if crate::ui::forum::is_unread(app, &p) {
        items.push(act(&e, crate::t!("menu.mark_read"), move |a, _, cx| a.mark_nav_read(crate::ui::nav_menus::Target::Channel(t), cx)));
    } else {
        items.push(MenuItem::disabled(crate::t!("menu.mark_read"), None));
    }
    items.push(MenuItem::Separator);
    items.push(crate::actions::channel::favorite_item(app, &e, t));
    items.push(MenuItem::Separator);
    items.push(if followed {
        act(&e, crate::t!("menu.post.unfollow"), move |a, _, cx| a.leave_thread(t, Place::ContextMenu, cx))
    } else {
        act(&e, crate::t!("menu.post.follow"), move |a, _, cx| a.join_thread(t, Place::ContextMenu, cx))
    });
    if manage || owner {
        items.push(if p.archived {
            act(&e, crate::t!("menu.post.open"), move |a, _, cx| a.set_thread_archived(t, false, cx))
        } else {
            act(&e, crate::t!("menu.post.close"), move |a, _, cx| a.set_thread_archived(t, true, cx))
        });
    }
    if manage {
        let locked = p.locked;
        items.push(act(&e, crate::t!(if locked { "menu.post.unlock" } else { "menu.post.lock" }), move |a, _, cx| a.set_thread_locked(t, !locked, cx)));
    }
    if manage || owner {
        items.push(act(&e, crate::t!("menu.post.edit"), move |a, _, cx| a.open_thread_settings(t, cx)));
        let applied = p.tags.clone();
        let tags: Vec<MenuItem> = app
            .backend
            .forum_tags(forum)
            .into_iter()
            .filter(|x| manage || !x.moderated)
            .map(|x| {
                let on = applied.contains(&x.id);
                let e = e.clone();
                let applied = applied.clone();
                MenuItem::check(x.name.clone(), on, move |_, _, cx: &mut App| {
                    let next = crate::ui::forum::toggle_tag(&applied, x.id);
                    e.update(cx, |a, cx| a.set_thread_tags(t, next, cx))
                })
            })
            .collect();
        if !tags.is_empty() {
            items.push(MenuItem::submenu(crate::t!("menu.post.edit_tags"), tags));
        }
    }
    if let Some(g) = guild {
        items.push(copy(crate::t!("menu.copy_link"), format!("https://discord.com/channels/{}/{}", g.0, t.0)));
    }
    items.push(MenuItem::Separator);
    if followed {
        items.push(mute_item(app, &e, t));
        items.push(notify_item(app, &e, t));
    } else {
        items.push(MenuItem::disabled(crate::t!("menu.post.mute"), None));
        items.push(MenuItem::disabled(crate::t!("menu.notify_settings"), None));
    }
    if manage {
        items.push(MenuItem::Separator);
        let pinned = p.pinned;
        items.push(act(&e, crate::t!(if pinned { "menu.post.unpin" } else { "menu.post.pin" }), move |a, _, cx| a.set_thread_pinned(t, !pinned, cx)));
        let e2 = e.clone();
        items.push(MenuItem::danger(crate::t!("menu.post.delete"), None, move |_, _, cx: &mut App| e2.update(cx, |a, cx| a.confirm_delete_thread(t, true, cx))));
    }
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        items.push(copy(crate::t!("menu.copy_thread_id"), t.0.to_string()).with_icon("id"));
    }
    Some(context_menu::render(pos, items, std::rc::Rc::new(|w: &mut Window, _: &mut App| close(w))))
}
