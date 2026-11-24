use std::rc::Rc;

use gpui::{App, Context};

use super::{ActionReply, ActionRequest, EventPlace, now_unix, open_modal, pins};
use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    ui::{
        context_menu::MenuItem,
        modals::{Field, Form, Modal, server_settings},
    },
};

pub const EVENT_STARTS: [(&str, Option<u64>); 4] =
    [("event.in_1h", Some(3600)), ("event.in_3h", Some(10_800)), ("msg.remind.tomorrow", None), ("event.in_1w", Some(604_800))];

pub fn event_place(location: &str, first_voice: Option<(Id, bool)>) -> Option<EventPlace> {
    let l = location.trim();
    if !l.is_empty() {
        return Some(EventPlace::External(l.to_owned()));
    }
    first_voice.map(|(c, stage)| if stage { EventPlace::Stage(c) } else { EventPlace::Voice(c) })
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dest {
    First,
    Last,
    Category(Option<Id>),
}

impl PhoenixApp {
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

    pub fn open_create_event(&mut self, guild: Id, cx: &mut Context<Self>) {
        let voice = self
            .guild_channels
            .get(&guild)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
            .find(|c| matches!(c.kind, ChannelKind::Voice | ChannelKind::Stage))
            .map(|c| (c.id, c.kind == ChannelKind::Stage));
        open_modal(
            Modal::Form(Form {
                title: crate::t!("menu.create_event"),
                subtitle: Some(crate::t!("event.subtitle")),
                fields: vec![
                    Field::new(crate::t!("event.topic"), crate::t!("event.topic_ph"), ""),
                    Field::new(crate::t!("event.location"), crate::t!("event.location_ph"), ""),
                    Field::new(crate::t!("event.description"), gpui::SharedString::default(), ""),
                ],
                choices: EVENT_STARTS.iter().map(|(k, _)| crate::t!(k)).collect(),
                choice: Some(0),
                check: None,
                submit: crate::t!("event.create"),
                danger: false,
                on_submit: Rc::new(move |a, v, choice, cx| {
                    let (name, loc, desc) = (v[0].clone(), v[1].clone(), v[2].clone());
                    let Some(place) = event_place(&loc, voice).filter(|_| !name.trim().is_empty()) else {
                        a.parity_toast(crate::t!("event.missing"), cx);
                        return;
                    };
                    let start = match EVENT_STARTS[choice.unwrap_or(0).min(EVENT_STARTS.len() - 1)].1 {
                        Some(s) => now_unix() + s,
                        None => super::message::tomorrow_9am(chrono::Local::now()),
                    };
                    let req = ActionRequest::CreateEvent { guild, name, description: desc, start_unix: start, end_unix: None, place };
                    a.action(req, cx, |a, r, cx| match r {
                        Ok(ActionReply::Done) | Ok(_) => a.parity_toast(crate::t!("event.created"), cx),
                        Err(e) => a.parity_toast(e.0, cx),
                    });
                }),
            }),
            None,
        );
        cx.notify();
    }

    pub fn open_server_settings(&mut self, guild: Id, section: usize, cx: &mut Context<Self>) {
        open_modal(Modal::ServerSettings { guild, section }, None);
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

pub fn favorite_item(e: &Ent, channel: Id) -> MenuItem {
    let on = pins::is_favorite(channel);
    let e = e.clone();
    MenuItem::submenu_with_icon(
        crate::t!("menu.favorite"),
        "star",
        vec![MenuItem::check(crate::t!("menu.favorites"), on, move |_, _, cx: &mut App| {
            pins::toggle_favorite(channel);
            e.update(cx, |a, cx| {
                a.parity_toast(if on { crate::t!("fav.removed") } else { crate::t!("fav.added") }, cx)
            })
        })],
    )
}

pub fn server_settings_item(e: &Ent, guild: Id) -> MenuItem {
    MenuItem::submenu(
        crate::t!("menu.server_settings"),
        server_settings::SECTIONS.iter().enumerate().map(|(i, k)| item(e, crate::t!(k), move |a, cx| a.open_server_settings(guild, i, cx))).collect(),
    )
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

    #[test]
    fn places() {
        assert_eq!(event_place(" Paris ", None), Some(EventPlace::External("Paris".into())));
        assert_eq!(event_place("", Some((Id(3), false))), Some(EventPlace::Voice(Id(3))));
        assert_eq!(event_place("", Some((Id(3), true))), Some(EventPlace::Stage(Id(3))));
        assert_eq!(event_place("", None), None);
    }
}
