use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::card;
use crate::{
    actions::close_modal,
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub fn snowflake_unix(id: u64) -> i64 {
    ((id >> 22) / 1000) as i64 + 1_420_070_400
}

fn row(label: SharedString, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .justify_between()
        .gap(z(16.))
        .py(z(8.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(label))
        .child(div().text_size(z(14.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(value.into()))
}

fn date(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0).map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default()
}

pub fn mod_view(app: &PhoenixApp, guild: Id, user: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let u = app.users.get(&user);
    let member = app.members.get(&guild).and_then(|m| m.iter().find(|m| m.user == user));
    let roles: Vec<&crate::model::Role> = member
        .map(|m| app.roles.get(&guild).into_iter().flatten().filter(|r| m.roles.contains(&r.id)).collect())
        .unwrap_or_default();
    let guild_name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.clone()).unwrap_or_default();
    let head = div()
        .flex()
        .items_center()
        .gap(z(12.))
        .pb(z(12.))
        .child(crate::ui::avatar(u, 56.))
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().text_size(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(u.map(|u| u.display_name.clone()).unwrap_or_default()))
                .child(div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(u.map(|u| u.name.clone()).unwrap_or_default())),
        );
    let mut chips = div().flex().flex_wrap().gap(z(4.)).py(z(8.));
    for r in &roles {
        chips = chips.child(
            div()
                .h(z(24.))
                .px(z(8.))
                .rounded(z(4.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(div().size(z(12.)).rounded_full().bg(crate::theme::role_color(r.color)))
                .child(div().text_size(z(12.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(r.name.clone())),
        );
    }
    let body = div()
        .flex()
        .flex_col()
        .child(head)
        .child(row(crate::t!("modview.user_id"), user.0.to_string()))
        .child(row(crate::t!("modview.created"), date(snowflake_unix(user.0))))
        .child(row(crate::t!("modview.server"), guild_name))
        .child(row(crate::t!("modview.roles"), roles.len().to_string()))
        .child(chips)
        .child(div().pt(z(8.)).text_size(z(12.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("modview.readonly")));
    card(480., crate::t!("menu.mod_view"), None, body.into_any_element(), None, cx)
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn snowflake_time() {
        assert_eq!(super::snowflake_unix(175_928_847_299_117_063), 1_462_015_105);
    }
}
