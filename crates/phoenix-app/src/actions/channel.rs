use std::rc::Rc;

use gpui::{App, Context};

use super::{ActionRequest, open_modal, pins};
use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    ui::{
        context_menu::MenuItem,
        modals::{Field, Form, Modal, server_settings},
    },
};

pub fn move_entries(app: &PhoenixApp, guild: Id, channel: Id, dest: Dest) -> Vec<(Id, i32, Option<Option<Id>>)> {
    let Some(ch) = app.channels.get(&channel) else { return Vec::new() };
    let siblings = |parent: Option<Id>| -> Vec<i32> {
        app.guild_channels
            .get(&guild)
            .into_iter()
            .flatten()
            .filter_map(|id| app.channels.get(id))
            .filter(|c| c.parent == parent && c.id != channel && c.kind != ChannelKind::Category)
            .map(|c| c.position)
            .collect()
    };
    match dest {
        Dest::First => vec![(channel, siblings(ch.parent).into_iter().min().unwrap_or(0).saturating_sub(1).max(0), None)],
        Dest::Last => vec![(channel, siblings(ch.parent).into_iter().max().map_or(0, |p| p + 1), None)],
        Dest::Category(cat) => vec![(channel, siblings(cat).into_iter().max().map_or(0, |p| p + 1), Some(cat))],
    }
}

pub type ChannelSlot = (Id, ChannelKind, Option<Id>, i32);

fn deltas(order: &[ChannelSlot]) -> Vec<(Id, i32)> {
    order.iter().enumerate().filter(|(i, c)| c.3 != *i as i32).map(|(i, c)| (c.0, i as i32)).collect()
}

fn by_position(a: &ChannelSlot, b: &ChannelSlot) -> std::cmp::Ordering {
    (a.3, a.0.0).cmp(&(b.3, b.0.0))
}

pub fn category_order(chans: &[ChannelSlot]) -> Vec<ChannelSlot> {
    let mut cats: Vec<ChannelSlot> = chans.iter().copied().filter(|c| c.1 == ChannelKind::Category).collect();
    cats.sort_by(by_position);
    cats
}

fn flattened(cats: &[ChannelSlot], chans: &[ChannelSlot], keep: impl Fn(ChannelKind) -> bool) -> Vec<ChannelSlot> {
    let under = |parent: Option<Id>| {
        let mut v: Vec<ChannelSlot> = chans.iter().copied().filter(|c| c.1 != ChannelKind::Category && c.2 == parent && keep(c.1)).collect();
        v.sort_by(by_position);
        v
    };
    let mut out = under(None);
    for c in cats {
        out.extend(under(Some(c.0)));
    }
    out
}

pub fn category_move(chans: &[ChannelSlot], cat: Id, last: bool) -> Vec<(Id, i32)> {
    let mut cats = category_order(chans);
    let Some(from) = cats.iter().position(|c| c.0 == cat) else { return Vec::new() };
    let moved = cats.remove(from);
    if last {
        cats.push(moved);
    } else {
        cats.insert(0, moved);
    }
    let mut out = deltas(&cats);
    let text = |k: ChannelKind| matches!(k, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum);
    let vocal = |k: ChannelKind| matches!(k, ChannelKind::Voice | ChannelKind::Stage);
    out.extend(deltas(&flattened(&cats, chans, text)));
    out.extend(deltas(&flattened(&cats, chans, vocal)));
    out
}

pub fn guild_slots(app: &PhoenixApp, guild: Id) -> Vec<ChannelSlot> {
    app.guild_channels.get(&guild).into_iter().flatten().filter_map(|id| app.channels.get(id)).map(|c| (c.id, c.kind, c.parent, c.position)).collect()
}

pub fn category_move_item(app: &PhoenixApp, e: &Ent, guild: Id, cat: Id) -> Option<MenuItem> {
    let cats = category_order(&guild_slots(app, guild));
    if cats.len() < 2 {
        return None;
    }
    let first = cats.first().is_some_and(|c| c.0 == cat);
    let last = cats.last().is_some_and(|c| c.0 == cat);
    let place = |key: &str, at_end: bool, disabled: bool| {
        if disabled {
            MenuItem::disabled(crate::t!(key), None)
        } else {
            item(e, crate::t!(key), move |a, cx| a.move_category(guild, cat, at_end, cx))
        }
    };
    Some(MenuItem::submenu(crate::t!("menu.move_to"), vec![place("menu.move_first", false, first), place("menu.move_last", true, last)]))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dest {
    First,
    Last,
    Category(Option<Id>),
}

impl PhoenixApp {
    pub fn move_category(&mut self, guild: Id, cat: Id, last: bool, cx: &mut Context<Self>) {
        let updates = category_move(&guild_slots(self, guild), cat, last);
        if updates.is_empty() {
            return;
        }
        for (id, pos) in &updates {
            if let Some(c) = self.channels.get_mut(id) {
                c.position = *pos;
            }
        }
        let entries = updates.into_iter().map(|(id, pos)| (id, pos, None)).collect();
        self.action_toast(ActionRequest::MoveChannels { guild, entries }, None, cx);
    }

    pub fn open_clone_channel(&mut self, guild: Id, channel: Id, cx: &mut Context<Self>) {
        let name = self.channels.get(&channel).map(|c| c.name.to_string()).unwrap_or_default();
        open_modal(
            Modal::Form(Form {
                title: crate::t!("menu.clone_channel"),
                subtitle: Some(crate::t!("clone.subtitle", name = name)),
                fields: vec![Field::new(crate::t!("clone.name"), gpui::SharedString::default(), name.clone())],
                choices: Vec::new(),
                choice: None,
                check: None,
                submit: crate::t!("clone.create"),
                danger: false,
                preview: None,
                width: 440.,
                on_submit: Rc::new(move |a, v, _, cx| {
                    let name = v.into_iter().next().unwrap_or_default();
                    a.action(ActionRequest::CloneChannel { guild, channel, name }, cx, |a, r, cx| match r {
                        Ok(_) => a.parity_toast(crate::t!("clone.done"), cx),
                        Err(e) => a.parity_toast(e.0, cx),
                    });
                }),
            }),
            None,
        );
        cx.notify();
    }

    pub fn move_channel(&mut self, guild: Id, channel: Id, dest: Dest, cx: &mut Context<Self>) {
        let entries = move_entries(self, guild, channel, dest);
        if let (Some(c), Some((_, pos, parent))) = (self.channels.get_mut(&channel), entries.first().copied()) {
            c.position = pos;
            if let Some(p) = parent {
                c.parent = p;
            }
        }
        self.action_toast(ActionRequest::MoveChannels { guild, entries }, None, cx);
    }

    pub fn open_server_settings(&mut self, guild: Id, section: usize, cx: &mut Context<Self>) {
        self.nav.guild_menu = false;
        open_modal(Modal::ServerSettings(Box::new(server_settings::State::new(guild, section))), None);
        server_settings::load(self, cx);
        cx.notify();
    }
}

type Ent = gpui::Entity<PhoenixApp>;

fn item(e: &Ent, label: gpui::SharedString, f: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static) -> MenuItem {
    let e = e.clone();
    MenuItem::action(label, None, move |_, _, cx: &mut App| e.update(cx, |a, cx| f(a, cx)))
}

pub fn move_to_item(app: &PhoenixApp, e: &Ent, guild: Id, channel: Id) -> MenuItem {
    let cur = app.channels.get(&channel).and_then(|c| c.parent);
    let mut cats: Vec<MenuItem> = app
        .guild_channels
        .get(&guild)
        .into_iter()
        .flatten()
        .filter_map(|id| app.channels.get(id))
        .filter(|c| c.kind == ChannelKind::Category && Some(c.id) != cur)
        .map(|c| {
            let cat = c.id;
            item(e, c.name.clone(), move |a, cx| a.move_channel(guild, channel, Dest::Category(Some(cat)), cx))
        })
        .collect();
    if cur.is_some() {
        cats.insert(0, item(e, crate::t!("move.no_category"), move |a, cx| a.move_channel(guild, channel, Dest::Category(None), cx)));
    }
    if cats.is_empty() {
        cats.push(MenuItem::action(crate::t!("menu.apps_none"), None, |_, _, _| {}));
    }
    MenuItem::submenu(
        crate::t!("menu.move_to"),
        vec![
            MenuItem::submenu(crate::t!("menu.move_category"), cats),
            MenuItem::Separator,
            item(e, crate::t!("menu.move_first"), move |a, cx| a.move_channel(guild, channel, Dest::First, cx)),
            item(e, crate::t!("menu.move_last"), move |a, cx| a.move_channel(guild, channel, Dest::Last, cx)),
        ],
    )
}

pub fn pin_item(e: &Ent, channel: Id, dm: bool) -> MenuItem {
    let on = pins::is_pinned(channel);
    let key = match (dm, on) {
        (true, false) => "menu.pin_dm",
        (true, true) => "menu.unpin_dm",
        (false, false) => "menu.pin_channel",
        (false, true) => "menu.unpin_channel",
    };
    item(e, crate::t!(key), move |a, cx| a.toggle_channel_pin(channel, cx))
}

pub fn favorite_item(app: &PhoenixApp, e: &Ent, channel: Id) -> MenuItem {
    let fav = crate::ui::favorites::data(app);
    let current = fav.get(channel.0).map(|f| f.parent_id).map(|p| (p != 0 && fav.get(p).is_some_and(|c| c.is_category())).then_some(p));
    let cats: Vec<(u64, gpui::SharedString)> = fav.categories().into_iter().map(|(id, n)| (id, gpui::SharedString::from(n.to_owned()))).collect();
    let row = |label: gpui::SharedString, parent: Option<u64>| {
        let e = e.clone();
        let on = current == Some(parent);
        MenuItem::check(label, on, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| match (on, current) {
                (true, _) => {
                    crate::ui::favorites::remove(a, channel, cx);
                    a.parity_toast(crate::t!("fav.removed"), cx)
                }
                (false, Some(_)) => crate::ui::favorites::move_to(a, channel, parent, cx),
                (false, None) => crate::ui::favorites::add(a, channel, parent, cx),
            })
        })
    };
    let mut items = vec![row(crate::t!("menu.favorites"), None)];
    for (id, name) in cats {
        items.push(row(name, Some(id)));
    }
    MenuItem::submenu_with_icon(crate::t!("menu.favorite"), "star", items)
}

pub fn server_settings_item(app: &PhoenixApp, e: &Ent, guild: Id) -> MenuItem {
    let items = server_settings::SECTIONS.iter().enumerate().filter(|(_, key)| server_settings::visible(app, guild, key))
        .map(|(section, key)| item(e, server_settings::section_label(key), move |app, cx| {
            if server_settings::opens_app_directory(section) {
                server_settings::open_app_directory(cx);
            } else {
                app.open_server_settings(guild, section, cx);
            }
        }))
        .collect();
    let entity = e.clone();
    MenuItem::Submenu {
        label: crate::t!("menu.server_settings"),
        items,
        icon: Some(crate::icons::path("gear")),
        subtext: None,
        on_click: Some(std::rc::Rc::new(move |_, window, cx| {
            entity.update(cx, |app, cx| app.open_server_settings(guild, 0, cx));
            window.refresh();
        })),
    }
}

pub fn show_all_item(app: &PhoenixApp, e: &Ent, guild: Id) -> Option<MenuItem> {
    if !app.backend.guild_features(guild).iter().any(|f| f == "COMMUNITY") {
        return None;
    }
    let on = pins::shows_all(guild);
    let e = e.clone();
    Some(MenuItem::check(crate::t!("menu.show_all_channels"), on, move |_, _, cx: &mut App| {
        pins::toggle_show_all(guild);
        e.update(cx, |a, cx| {
            a.rebuild_sidebar();
            cx.notify();
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: u64, kind: ChannelKind, parent: Option<u64>, pos: i32) -> ChannelSlot {
        (Id(id), kind, parent.map(Id), pos)
    }

    #[test]
    fn moving_a_category_matches_the_client() {
        use ChannelKind::*;
        let chans = vec![
            slot(1, Category, None, 0),
            slot(2, Category, None, 1),
            slot(3, Category, None, 2),
            slot(10, Text, None, 0),
            slot(11, Text, Some(1), 1),
            slot(12, Text, Some(2), 2),
            slot(13, Text, Some(3), 3),
            slot(20, Voice, Some(3), 0),
            slot(21, Voice, Some(1), 1),
        ];
        assert_eq!(category_move(&chans, Id(3), false), vec![(Id(3), 0), (Id(1), 1), (Id(2), 2), (Id(13), 1), (Id(11), 2), (Id(12), 3)]);
        assert_eq!(category_move(&chans, Id(1), true), vec![(Id(2), 0), (Id(3), 1), (Id(1), 2), (Id(12), 1), (Id(13), 2), (Id(11), 3)]);
        assert_eq!(category_move(&chans, Id(3), true), vec![(Id(21), 0), (Id(20), 1)]);
    }

}
