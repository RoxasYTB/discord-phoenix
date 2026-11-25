use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};

use super::card;
use crate::{
    actions::close_modal,
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub fn call_card(app: &PhoenixApp, _channel: Id, user: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let u = app.users.get(&user);
    let body = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(12.))
        .py(z(16.))
        .child(crate::ui::avatar(u, 80.))
        .child(div().text_size(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(u.map(|u| u.display_name.clone()).unwrap_or_default()))
        .child(div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("call.ringing")));
    let footer = button("call-hangup", crate::t!("call.hang_up"), BtnKind::Critical).on_click(cx.listener(|_, _, _, cx| {
        close_modal();
        cx.notify();
    }));
    card(360., crate::t!("call.title"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}
