use std::cell::RefCell;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, deferred, div, prelude::*, svg};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, m, tok, z},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewAs {
    pub guild: Id,
    pub roles: Vec<Id>,
    pub picker: bool,
}

thread_local!(static VIEW: RefCell<Option<ViewAs>> = const { RefCell::new(None) });

pub fn current() -> Option<ViewAs> {
    VIEW.with(|v| v.borrow().clone())
}

pub fn roles_for(guild: Id) -> Option<Vec<Id>> {
    current().filter(|v| v.guild == guild).map(|v| v.roles)
}

fn set(v: Option<ViewAs>) {
    VIEW.with(|s| *s.borrow_mut() = v);
}

pub fn toggle_role(roles: &mut Vec<Id>, role: Id) {
    if let Some(i) = roles.iter().position(|r| *r == role) {
        roles.remove(i);
    } else {
        roles.push(role);
    }
}

pub fn label(n: usize) -> SharedString {
    match n {
        0 => crate::t!("viewas.notice_zero"),
        1 => crate::t!("viewas.notice_one", n = 1),
        n => crate::t!("viewas.notice_many", n = n),
    }
}

pub fn start(app: &mut PhoenixApp, guild: Id, role: Id, cx: &mut Context<PhoenixApp>) {
    let roles = if role == guild { Vec::new() } else { vec![role] };
    set(Some(ViewAs { guild, roles, picker: false }));
    crate::actions::close_modal();
    if app.selected_guild != Some(guild) {
        app.select_guild(Some(guild), cx);
    }
    app.rebuild_sidebar();
    cx.notify();
}

pub fn stop(app: &mut PhoenixApp, back: bool, cx: &mut Context<PhoenixApp>) {
    let Some(v) = current() else { return };
    set(None);
    app.rebuild_sidebar();
    if back {
        app.open_server_settings(v.guild, crate::ui::modals::server_settings::section_index("menu.gs.roles"), cx);
    }
    cx.notify();
}

pub fn visible(app: &PhoenixApp, guild: Id, channel: Id) -> bool {
    match roles_for(guild) {
        Some(roles) => app.backend.roles_can_view(channel, &roles),
        None => true,
    }
}

fn button(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .ml(z(10.))
        .h(z(24.))
        .px(z(10.))
        .flex()
        .items_center()
        .gap(z(4.))
        .rounded(z(3.))
        .border_1()
        .border_color(c(tok(idx::WHITE)))
        .line_height(z(22.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
}

pub fn notice(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let v = current().filter(|v| Some(v.guild) == app.selected_guild)?;
    let white = c(tok(idx::WHITE));
    let back = button("viewas-back")
        .ml(z(0.))
        .on_click(cx.listener(|a, _, _, cx| stop(a, true, cx)))
        .child(svg().path(crate::icons::path("arrow-left")).size(z(16.)).text_color(white))
        .child(crate::t!("viewas.back"));
    let select = button("viewas-select")
        .on_click(cx.listener(|_, _, _, cx| {
            VIEW.with(|s| {
                if let Some(v) = s.borrow_mut().as_mut() {
                    v.picker = !v.picker;
                }
            });
            cx.notify();
        }))
        .child(crate::t!("viewas.select"))
        .child(svg().path(crate::icons::path("chevron")).size(z(16.)).text_color(white));
    let disable = button("viewas-disable").ml(z(20.)).on_click(cx.listener(|a, _, _, cx| stop(a, false, cx))).child(crate::t!("viewas.disable"));
    let picker = v.picker.then(|| picker(app, &v, cx));
    Some(
        div()
            .id("viewas-notice")
            .absolute()
            .top_0()
            .right_0()
            .left(z(m::server_bar_w()))
            .h(z(crate::streamer::NOTICE_H))
            .pl(z(4.))
            .pr(z(28.))
            .bg(c(tok(idx::BRAND_500)))
            .border_t_1()
            .border_l_1()
            .border_color(c(tok(idx::APP_FRAME_BORDER)))
            .rounded_tl(z(8.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(14.))
            .line_height(z(36.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(white)
            .child(back)
            .child(div().ml(z(20.)).child(label(v.roles.len())))
            .child(div().relative().child(select).children(picker))
            .child(disable)
            .into_any_element(),
    )
}

fn picker(app: &PhoenixApp, v: &ViewAs, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut roles: Vec<crate::model::Role> = app.roles.get(&v.guild).cloned().unwrap_or_default().into_iter().filter(|r| r.id != v.guild && !r.managed).collect();
    roles.sort_by_key(|r| std::cmp::Reverse(r.position));
    let mut list = div().flex().flex_col();
    for (n, r) in roles.into_iter().enumerate() {
        let on = v.roles.contains(&r.id);
        let id = r.id;
        list = list.child(
            div()
                .id(("viewas-role", n))
                .h(z(32.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    VIEW.with(|s| {
                        if let Some(v) = s.borrow_mut().as_mut() {
                            toggle_role(&mut v.roles, id);
                        }
                    });
                    a.rebuild_sidebar();
                    cx.notify();
                }))
                .child(div().size(z(12.)).flex_none().rounded_full().bg(r.color.map(c).unwrap_or_else(|| c(tok(idx::TEXT_MUTED)))))
                .child(div().flex_1().min_w_0().truncate().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(r.name.clone()))
                .child(crate::ui::community::checkbox(on)),
        );
    }
    deferred(
        div()
            .id("viewas-picker")
            .occlude()
            .absolute()
            .top(z(30.))
            .left_0()
            .w(z(280.))
            .max_h(z(360.))
            .overflow_y_scroll()
            .p(z(8.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .line_height(z(18.))
            .child(list),
    )
    .with_priority(5)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_toggle_and_label() {
        let mut r = vec![Id(1)];
        toggle_role(&mut r, Id(2));
        toggle_role(&mut r, Id(1));
        assert_eq!(r, vec![Id(2)]);
        set(Some(ViewAs { guild: Id(9), roles: r, picker: false }));
        assert_eq!(roles_for(Id(9)), Some(vec![Id(2)]));
        assert_eq!(roles_for(Id(8)), None);
        set(None);
        assert_eq!(current(), None);
    }
}
