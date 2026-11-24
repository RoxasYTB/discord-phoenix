use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::InputState;

use super::{Modal, card, input_box};
use crate::{
    actions::{ActionRequest, close_modal, with_modal},
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub const MAX_DESTINATIONS: usize = 5;

pub struct Forward {
    pub channel: Id,
    pub msg: Id,
    pub search: Option<Entity<InputState>>,
    pub comment: Option<Entity<InputState>>,
    pub picked: Vec<Id>,
    pub sending: bool,
}

impl Forward {
    pub fn new(channel: Id, msg: Id) -> Self {
        Self { channel, msg, search: None, comment: None, picked: Vec::new(), sending: false }
    }
}

pub fn destinations(app: &PhoenixApp, q: &str) -> Vec<(Id, SharedString, Option<SharedString>)> {
    let q = q.to_lowercase();
    let mut out = Vec::new();
    for d in &app.dms {
        if let Some(ch) = app.channels.get(d) {
            out.push((*d, ch.name.clone(), None));
        }
    }
    for g in &app.guilds {
        for id in app.guild_channels.get(&g.id).into_iter().flatten() {
            if let Some(ch) = app.channels.get(id).filter(|c| matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement)) {
                out.push((*id, format!("#{}", ch.name).into(), Some(g.name.clone())));
            }
        }
    }
    if !q.is_empty() {
        out.retain(|(_, l, s)| l.to_lowercase().contains(&q) || s.as_ref().is_some_and(|s| s.to_lowercase().contains(&q)));
    }
    out
}

pub fn toggle(picked: &mut Vec<Id>, id: Id) {
    if let Some(i) = picked.iter().position(|x| *x == id) {
        picked.remove(i);
    } else if picked.len() < MAX_DESTINATIONS {
        picked.push(id);
    }
}

pub fn render(app: &PhoenixApp, f: &Forward, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let q = f.search.as_ref().map(|s| s.read(cx).value().to_string()).unwrap_or_default();
    let mut list = div().id("fwd-list").h(z(320.)).overflow_y_scroll().flex().flex_col().gap(z(2.));
    let dest = destinations(app, &q);
    if dest.is_empty() {
        list = list.child(div().p(z(16.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("fwd.none")));
    }
    for (id, name, sub) in dest.into_iter().take(200) {
        let on = f.picked.contains(&id);
        let peer = app.channels.get(&id).and_then(|c| if c.guild.is_none() { c.voice_users.first().copied() } else { None });
        list = list.child(
            div()
                .id(("fwd-row", id.0))
                .h(z(44.))
                .px(z(8.))
                .rounded(z(8.))
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .flex()
                .items_center()
                .gap(z(12.))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_modal(|m| {
                        if let Some(Modal::Forward(f)) = m {
                            toggle(&mut f.picked, id);
                        }
                    });
                    cx.notify();
                }))
                .child(match peer {
                    Some(u) => crate::ui::avatar(app.users.get(&u), 32.),
                    None => div()
                        .size(z(32.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("#")
                        .into_any_element(),
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_baseline()
                        .gap(z(6.))
                        .child(div().text_size(z(16.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).truncate().child(name))
                        .children(sub.map(|s| div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).truncate().child(s))),
                )
                .child(
                    div()
                        .size(z(20.))
                        .rounded(z(4.))
                        .border_1()
                        .border_color(c(tok(idx::BORDER_STRONG)))
                        .when(on, |d| d.bg(c(tok(idx::BACKGROUND_BRAND))).border_color(c(tok(idx::BACKGROUND_BRAND))))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(gpui::white())
                        .text_size(z(14.))
                        .when(on, |d| d.child("✓")),
                ),
        );
    }
    let mut body = div().flex().flex_col().gap(z(12.));
    if let Some(s) = &f.search {
        body = body.child(input_box(s));
    }
    body = body.child(list);
    if let Some(cmt) = &f.comment {
        body = body.child(input_box(cmt));
    }
    let can = !f.picked.is_empty() && !f.sending;
    let send = button("fwd-send", crate::t!("fwd.send"), BtnKind::Primary)
        .when(!can, |d| d.opacity(0.5).cursor_default())
        .on_click(cx.listener(move |a, _, _, cx| {
            let req = with_modal(|m| match m {
                Some(Modal::Forward(f)) if !f.picked.is_empty() && !f.sending => {
                    f.sending = true;
                    Some((f.channel, f.msg, f.picked.clone(), f.comment.clone()))
                }
                _ => None,
            });
            let Some((from, msg, to, comment)) = req else { return };
            let comment = comment.map(|i| i.read(cx).value().to_string()).unwrap_or_default();
            let guild = a.channels.get(&from).and_then(|c| c.guild);
            a.action(ActionRequest::Forward { from, msg, guild, to, comment }, cx, |a, r, cx| match r {
                Ok(_) => {
                    close_modal();
                    a.parity_toast(crate::t!("fwd.done"), cx);
                }
                Err(e) => {
                    with_modal(|m| {
                        if let Some(Modal::Forward(f)) = m {
                            f.sending = false;
                        }
                    });
                    a.parity_toast(e.0, cx);
                }
            });
            cx.notify();
        }));
    card(
        480.,
        crate::t!("fwd.title"),
        Some(crate::t!("fwd.subtitle", n = MAX_DESTINATIONS)),
        body.into_any_element(),
        Some(send.into_any_element()),
        cx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_at_five() {
        let mut p = Vec::new();
        for i in 0..7 {
            toggle(&mut p, Id(i));
        }
        assert_eq!(p.len(), MAX_DESTINATIONS);
        toggle(&mut p, Id(0));
        assert_eq!(p.len(), 4);
    }
}
