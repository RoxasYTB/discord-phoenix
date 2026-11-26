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
pub const MAX_FAVORITES: usize = 20;

pub struct Forward {
    pub channel: Id,
    pub msg: Id,
    pub search: Option<Entity<InputState>>,
    pub comment: Option<Entity<InputState>>,
    pub picked: Vec<Id>,
    pub sending: bool,
    pub favorites: bool,
}

impl Forward {
    pub fn new(channel: Id, msg: Id) -> Self {
        Self { channel, msg, search: None, comment: None, picked: Vec::new(), sending: false, favorites: false }
    }

    pub fn favorites() -> Self {
        Self { favorites: true, ..Self::new(Id(0), Id(0)) }
    }

    fn max(&self) -> usize {
        if self.favorites { MAX_FAVORITES } else { MAX_DESTINATIONS }
    }
}

pub fn destinations(app: &PhoenixApp, q: &str) -> Vec<(Id, SharedString, Option<SharedString>)> {
    destinations_of(app, q, false)
}

fn destinations_of(app: &PhoenixApp, q: &str, favorites: bool) -> Vec<(Id, SharedString, Option<SharedString>)> {
    let q = q.to_lowercase();
    let mut out = Vec::new();
    for d in &app.dms {
        if let Some(ch) = app.channels.get(d) {
            out.push((*d, ch.name.clone(), None));
        }
    }
    for g in &app.guilds {
        for id in app.guild_channels.get(&g.id).into_iter().flatten() {
            let kinds = |c: &&crate::model::Channel| match c.kind {
                ChannelKind::Text | ChannelKind::Announcement => true,
                ChannelKind::Voice | ChannelKind::Stage | ChannelKind::Forum => favorites,
                _ => false,
            };
            if let Some(ch) = app.channels.get(id).filter(kinds).filter(|c| !favorites || !crate::ui::favorites::is_favorite(app, c.id)) {
                out.push((*id, ch.name.clone(), Some(g.name.clone())));
            }
        }
    }
    if !q.is_empty() {
        out.retain(|(_, l, s)| l.to_lowercase().contains(&q) || s.as_ref().is_some_and(|s| s.to_lowercase().contains(&q)));
    }
    out
}

pub fn toggle(picked: &mut Vec<Id>, id: Id) {
    toggle_max(picked, id, MAX_DESTINATIONS);
}

fn toggle_max(picked: &mut Vec<Id>, id: Id, max: usize) {
    if let Some(i) = picked.iter().position(|x| *x == id) {
        picked.remove(i);
    } else if picked.len() < max {
        picked.push(id);
    }
}

fn guild_channel_icon(app: &PhoenixApp, ch: &crate::model::Channel) -> AnyElement {
    let guild = ch.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let face: AnyElement = match guild.map(|g| &g.icon) {
        Some(crate::model::ImageRef::Url(u)) => gpui::img(u.clone()).size(z(24.)).rounded(z(8.)).flex_none().into_any_element(),
        _ => {
            let acr = guild.map(|g| g.acronym.to_string()).unwrap_or_default();
            let size = [13., 11., 10., 10., 10., 10., 10.][acr.chars().count().saturating_sub(1).min(6)];
            div()
                .size(z(24.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .font_weight(FontWeight::MEDIUM)
                .text_size(z(size))
                .line_height(gpui::relative(1.2))
                .whitespace_nowrap()
                .flex()
                .items_center()
                .justify_center()
                .child(acr)
                .into_any_element()
        }
    };
    div()
        .relative()
        .w(z(32.))
        .h(z(29.))
        .child(crate::ui::blob_mask::cut(face, [crate::ui::blob_mask::channel_type_hole(24.)]))
        .child(
            div()
                .absolute()
                .left(z(12.))
                .top(z(9.))
                .p(z(4.))
                .rounded_full()
                .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                .child(crate::ui::icon(crate::ui::sidebar::channel_icon(ch.kind), 12., c(tok(idx::TEXT_SUBTLE)))),
        )
        .into_any_element()
}

fn share_row(app: &PhoenixApp, id: Id, label: SharedString, sub: Option<SharedString>, on: bool, disabled: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ch = app.channels.get(&id);
    let group = ch.is_some_and(|c| c.guild.is_none()) && app.nav.extras.get(&id).is_some_and(|e| e.recipients > 1 || e.group_icon.is_some());
    let peer = ch.filter(|c| c.guild.is_none() && !group).and_then(|c| c.voice_users.first().copied());
    let ring = c(tok(idx::BACKGROUND_SURFACE_HIGH));
    let icon: AnyElement = match (ch, peer) {
        (Some(ch), _) if ch.guild.is_some() => guild_channel_icon(app, ch),
        (_, Some(u)) => crate::ui::avatar_with_status(app.users.get(&u), 32., app.presence_of(u), ring).into_any_element(),
        _ if group => crate::ui::group_dm::group_avatar(app, id, 32., ring),
        _ => crate::ui::avatar(None, 32.),
    };
    let sub = sub.or_else(|| peer.and_then(|u| app.users.get(&u)).map(|u| u.name.clone()));
    let label_col = if disabled { idx::TEXT_MUTED } else { idx::TEXT_STRONG };
    div()
        .id(("fwd-row", id.0))
        .flex_none()
        .min_h(z(48.))
        .mx(z(-8.))
        .px(z(8.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .when(disabled, |d| d.cursor(gpui::CursorStyle::OperationNotAllowed))
        .when(!disabled, |d| {
            d.cursor_pointer().hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))).on_click(cx.listener(move |_, _, _, cx| {
                with_modal(|m| {
                    if let Some(Modal::Forward(f)) = m {
                        let max = f.max();
                        toggle_max(&mut f.picked, id, max);
                    }
                });
                cx.notify();
            }))
        })
        .child(
            div()
                .flex()
                .flex_shrink(1.)
                .min_w_0()
                .items_center()
                .gap(z(12.))
                .overflow_hidden()
                .child(div().flex_none().when(disabled, |d| d.opacity(0.5)).child(icon))
                .child(
                    div()
                        .my(z(4.))
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .overflow_hidden()
                        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(label_col))).truncate().child(label))
                        .children(sub.map(|s| div().text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).truncate().child(s))),
                ),
        )
        .child(div().flex_none().ml(z(16.)).child(checkbox(on, disabled)))
        .into_any_element()
}

pub fn checkbox(on: bool, disabled: bool) -> gpui::Div {
    let (bg, border) = if on {
        (idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT, idx::CHECKBOX_BORDER_SELECTED_DEFAULT)
    } else {
        (idx::CHECKBOX_BACKGROUND_DEFAULT, idx::CHECKBOX_BORDER_DEFAULT)
    };
    div()
        .size(z(20.))
        .rounded(z(4.))
        .border_1()
        .border_color(c(tok(border)))
        .bg(c(tok(bg)))
        .when(disabled, |d| d.opacity(0.5))
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(c(tok(idx::CHECKBOX_ICON_ACTIVE)))))
}

pub fn render(app: &PhoenixApp, f: &Forward, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let q = f.search.as_ref().map(|s| s.read(cx).value().to_string()).unwrap_or_default();
    let mut list = div().id("fwd-list").size_full().pl(z(8.)).pr(z(16.)).overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("fwd-list")).flex().flex_col();
    let dest = destinations_of(app, &q, f.favorites);
    if dest.is_empty() {
        let none = if f.favorites { crate::t!("fav.add_none") } else { crate::t!("fwd.none") };
        list = list.child(div().p(z(16.)).text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(none));
    }
    let full = f.picked.len() >= f.max();
    for (id, name, sub) in dest.into_iter().take(200) {
        let on = f.picked.contains(&id);
        list = list.child(share_row(app, id, name, sub, on, full && !on, cx));
    }
    let list = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Auto)
        .h(z(320.))
        .child(list)
        .children(crate::ui::chat::scrollbar::bar_for("fwd-list", crate::ui::chat::scrollbar::Bar::Auto));
    let mut body = div().flex().flex_col().gap(z(12.));
    if let Some(s) = &f.search {
        body = body.child(input_box(s));
    }
    body = body.child(list);
    if let Some(cmt) = &f.comment {
        body = body.child(input_box(cmt));
    }
    let can = !f.picked.is_empty() && !f.sending;
    if f.favorites {
        let label = if f.picked.len() >= 2 { crate::t!("fav.add_n", count = f.picked.len()) } else { crate::t!("fav.add_title") };
        let add = button("fav-add-send", label, BtnKind::Primary)
            .w_full()
            .when(!can, |d| d.opacity(0.5).cursor_default())
            .on_click(cx.listener(move |a, _, _, cx| {
                let picked = with_modal(|m| match m {
                    Some(Modal::Forward(f)) if f.favorites && !f.picked.is_empty() => Some(f.picked.clone()),
                    _ => None,
                });
                let Some(picked) = picked else { return };
                close_modal();
                crate::ui::favorites::add_many(a, &picked, cx);
                cx.notify();
            }));
        return card(480., crate::t!("fav.add_title"), None, body.into_any_element(), Some(add.into_any_element()), cx);
    }
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
