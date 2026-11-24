use std::cell::RefCell;

use gpui::{AnyElement, App, ClipboardItem, Context, Pixels, Point, Window};

use crate::{
    app::PhoenixApp,
    model::Id,
    ui::{
        context_menu::{self, MenuItem},
    },
};

thread_local! {
    static OPEN: RefCell<Option<(Id, Point<Pixels>)>> = const { RefCell::new(None) };
    static POST: RefCell<Option<(crate::commands::ForumPost, Id, Point<Pixels>)>> = const { RefCell::new(None) };
}

const MANAGE_THREADS: u64 = 1 << 34;

pub fn open_post(post: crate::commands::ForumPost, forum: Id, pos: Point<Pixels>, window: &mut Window) {
    POST.with(|o| *o.borrow_mut() = Some((post, forum, pos)));
    window.refresh();
}

pub fn open(thread: Id, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = Some((thread, pos)));
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

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if let Some(menu) = render_post(app, cx) {
        return Some(menu);
    }
    let (t, pos) = OPEN.with(|o| *o.borrow())?;
    let guild = app.channels.get(&t).and_then(|c| c.guild).or(app.selected_guild);
    let mut items = Vec::new();
    if let Some(g) = guild {
        items.push(copy(crate::t!("menu.copy_link"), format!("https://discord.com/channels/{}/{}", g.0, t.0)));
    }
    if app.prefs.synced.developer_mode {
        items.push(copy(crate::t!("menu.copy_thread_id"), t.0.to_string()).with_icon("id"));
    }
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
    let manage = guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), MANAGE_THREADS));
    let owner = p.owner == Some(app.me.id);
    let stub = |key: &'static str| {
        let e = e.clone();
        MenuItem::action(crate::t!(key), None, move |_, _, cx: &mut App| e.update(cx, |a, cx| a.stub(&crate::t!(key), cx)))
    };
    let mut items = Vec::new();
    if crate::ui::forum::is_unread(app, &p) {
        let e = e.clone();
        items.push(MenuItem::action(crate::t!("menu.mark_read"), None, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.mark_nav_read(crate::ui::nav_menus::Target::Channel(t), cx))
        }));
    } else {
        items.push(MenuItem::disabled(crate::t!("menu.mark_read"), None));
    }
    items.push(MenuItem::Separator);
    items.push(crate::actions::channel::favorite_item(&e, t));
    items.push(MenuItem::Separator);
    items.push(stub("menu.post.follow"));
    if (manage || owner) && !p.archived {
        items.push(stub("menu.post.close"));
    }
    if manage {
        items.push(stub(if p.locked { "menu.post.unlock" } else { "menu.post.lock" }));
    }
    if manage || owner {
        items.push(stub("menu.post.edit"));
        let tags: Vec<MenuItem> = app
            .backend
            .forum_tags(forum)
            .into_iter()
            .filter(|x| manage || !x.moderated)
            .map(|x| {
                let on = p.tags.contains(&x.id);
                let e = e.clone();
                MenuItem::check(x.name.clone(), on, move |_, _, cx: &mut App| e.update(cx, |a, cx| a.stub(&crate::t!("menu.post.edit_tags"), cx)))
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
    items.push(MenuItem::disabled(crate::t!("menu.post.mute"), None));
    items.push(MenuItem::disabled(crate::t!("menu.notify_settings"), None));
    if manage {
        items.push(MenuItem::Separator);
        items.push(stub(if p.pinned { "menu.post.unpin" } else { "menu.post.pin" }));
        let e = e.clone();
        items.push(MenuItem::danger(crate::t!("menu.post.delete"), None, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.stub(&crate::t!("menu.post.delete"), cx))
        }));
    }
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        items.push(copy(crate::t!("menu.copy_thread_id"), t.0.to_string()).with_icon("id"));
    }
    Some(context_menu::render(pos, items, std::rc::Rc::new(|w: &mut Window, _: &mut App| close(w))))
}
