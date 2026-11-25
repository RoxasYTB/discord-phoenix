use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, ClickEvent, Context, Pixels, Point, Window};

use crate::{
    app::PhoenixApp,
    model::Id,
    ui::{
        context_menu::{self, MenuItem},
        home::data::{NavRequest, RelAction},
    },
};

const MANAGE_ROLES: u64 = 1 << 28;
const MANAGE_NICKNAMES: u64 = 1 << 27;

type Act = Rc<dyn Fn(&mut PhoenixApp, &ClickEvent, &mut Window, &mut Context<PhoenixApp>)>;

#[derive(Clone, Copy)]
struct Open {
    user: Id,
    guild: Option<Id>,
    pos: Point<Pixels>,
}

thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
}

pub fn open(user: Id, guild: Option<Id>, pos: Point<Pixels>, window: &mut Window) {
    OPEN.with(|o| *o.borrow_mut() = Some(Open { user, guild, pos }));
    window.refresh();
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let o = OPEN.with(|o| *o.borrow())?;
    let e = cx.entity();
    let me = o.user == app.me.id;
    let act = |label: gpui::SharedString, f: Act| {
        let e = e.clone();
        MenuItem::action(label, None, move |ev, w, cx: &mut App| e.update(cx, |a, cx| f(a, ev, w, cx)))
    };
    let (u, g) = (o.user, o.guild);
    let mut items = vec![
        act(crate::t!("menu.profile"), Rc::new(move |a, ev, w, cx| a.open_profile(u, g, ev.position(), w, cx))),
    ];
    if app.selected_channel.is_some() {
        items.push(act(
            crate::t!("menu.mention"),
            Rc::new(move |a, _, w, cx| {
                a.composer.update(cx, |s, cx| {
                    let v = s.value().to_string();
                    let sep = if v.is_empty() || v.ends_with(' ') { "" } else { " " };
                    s.set_value(format!("{v}{sep}<@{}> ", u.0), w, cx);
                    s.focus(w, cx);
                })
            }),
        ));
    }
    if !me {
        items.push(act(crate::t!("menu.send_message"), Rc::new(move |a, _, _, cx| a.message_user(u, cx))));
    }
    let rel = app.nav.relationship(u).map(|r| r.kind);
    let friend = rel == Some(crate::ui::home::data::RelationshipKind::Friend);
    let bot = app.users.get(&u).is_some_and(|x| x.bot);
    let stub = |key: &'static str| act(crate::t!(key), Rc::new(move |a, _, _, cx| a.stub(&crate::t!(key), cx)));
    if !me && friend {
        items.push(act(crate::t!("menu.call"), Rc::new(move |a, _, _, cx| a.open_call_ui(u, cx))));
    }
    if !me && !app.prefs.local.streamer_hides_info() {
        items.push(
            act(crate::t!(crate::actions::user::note_label(app, u)), Rc::new(move |a, ev, w, cx| a.open_profile(u, g, ev.position(), w, cx)))
                .with_subtext(crate::t!("menu.note_subtext")),
        );
    }
    let voice = if me { crate::actions::user::VoiceItems::default() } else { crate::actions::user::voice_items(app, u, g, &e) };
    items.extend(voice.watch);
    if !me && friend {
        items.push(act(crate::t!("menu.friend_nickname"), Rc::new(move |a, _, _, cx| a.open_friend_nickname(u, cx))));
    }
    items.push(MenuItem::Separator);
    if let Some(gid) = g {
        if me {
            items.push(act(crate::t!("menu.edit_server_profile"), Rc::new(move |a, _, _, cx| a.open_server_profile(gid, cx))));
        } else if crate::ui::inbox::has_perm(app.backend.guild_permissions(gid), MANAGE_NICKNAMES) {
            items.push(act(crate::t!("menu.change_nickname"), Rc::new(move |a, _, _, cx| a.open_nickname(gid, Some(u), cx))));
        }
    }
    items.push(MenuItem::submenu(crate::t!("menu.apps"), vec![stub("menu.apps_none")]));
    if !me {
        if !bot {
            let invites: Vec<MenuItem> = app
                .guilds
                .iter()
                .filter(|x| Some(x.id) != g)
                .map(|x| {
                    let gid = x.id;
                    act_owned(&e, x.name.clone(), Rc::new(move |a, _, _, cx| a.open_invite_people(gid, cx)))
                })
                .collect();
            if !invites.is_empty() {
                items.push(MenuItem::submenu(crate::t!("menu.invite_to_server"), invites));
            }
        }
        if friend {
            items.push(act(crate::t!("menu.remove_friend"), Rc::new(move |a, _, _, cx| a.relationship_action(u, RelAction::Remove, cx))));
        } else if rel.is_none() && !bot {
            items.push(act(crate::t!("friends.add"), Rc::new(move |a, _, _, cx| a.relationship_action(u, RelAction::Accept, cx))));
        }
        let ign = if crate::actions::user::is_ignored(u) { "menu.unignore" } else { "menu.ignore" };
        items.push(act(crate::t!(ign), Rc::new(move |a, _, _, cx| a.toggle_ignore(u, cx))));
        items.push(MenuItem::danger(crate::t!("menu.block"), None, {
            let e = e.clone();
            move |_, _, cx: &mut App| e.update(cx, |a, cx| a.relationship_action(u, RelAction::Block, cx))
        }));
    }
    if let Some(gid) = g.filter(|gid| crate::ui::inbox::has_perm(app.backend.guild_permissions(*gid), MANAGE_ROLES)) {
        let has: Vec<crate::model::Id> =
            app.members.get(&gid).and_then(|m| m.iter().find(|m| m.user == u)).map(|m| m.roles.clone()).unwrap_or_default();
        let mut roles: Vec<&crate::model::Role> = app.roles.get(&gid).into_iter().flatten().filter(|r| r.id != gid).collect();
        roles.sort_by_key(|r| -r.position);
        let rows: Vec<MenuItem> = roles
            .into_iter()
            .map(|r| {
                let e = e.clone();
                let (role, on) = (r.id, has.contains(&r.id));
                MenuItem::check(r.name.clone(), on, move |_, _, cx: &mut App| {
                    e.update(cx, |a, cx| a.set_member_role(gid, u, role, !on, cx))
                })
            })
            .collect();
        if !rows.is_empty() {
            items.push(MenuItem::Separator);
            items.push(MenuItem::submenu(crate::t!("menu.roles"), rows));
        }
    }
    if let Some(mv) = voice.move_to {
        if !matches!(items.last(), Some(MenuItem::Submenu { .. })) {
            items.push(MenuItem::Separator);
        }
        items.push(mv);
    }
    let in_call = app.voice_channel.is_some_and(|v| app.channels.get(&v).is_some_and(|c| c.voice_users.contains(&u)));
    if in_call {
        items.push(MenuItem::Separator);
        items.extend(crate::ui::voice_panel::user_menu_items(app, u, cx));
    }
    let mut moderation = crate::ui::inbox::moderation_items(app, u, g, &e);
    if !voice.mods.is_empty() {
        let at = if moderation.is_empty() {
            moderation.push(MenuItem::Separator);
            1
        } else {
            let has_mod_view = moderation.get(1).is_some_and(|m| matches!(m, MenuItem::Action { danger: false, .. }));
            1 + has_mod_view as usize
        };
        moderation.splice(at..at, voice.mods);
    }
    items.extend(moderation);
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        items.push(MenuItem::action(crate::t!("common.copy_user_id"), Some(crate::icons::path("id")), move |_, _, cx: &mut App| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(u.0.to_string()))
        }));
    }
    let dismiss: context_menu::Dismiss = Rc::new(|w, _| {
        OPEN.with(|o| *o.borrow_mut() = None);
        w.refresh();
    });
    Some(context_menu::render(o.pos, items, dismiss))
}

fn act_owned(e: &gpui::Entity<PhoenixApp>, label: gpui::SharedString, f: Act) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |ev, w, cx: &mut App| e.update(cx, |a, cx| f(a, ev, w, cx)))
}

impl PhoenixApp {
    pub(crate) fn relationship_action(&mut self, user: Id, action: RelAction, cx: &mut Context<Self>) {
        self.nav_request(NavRequest::Relationship { user, action }, cx, |_, _, _| {});
        cx.notify();
    }
}
