use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::Forward,
    theme::{c, generated::idx, tok, z},
};

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn wrap(app: &PhoenixApp, f: &Forward, body: gpui::Div) -> AnyElement {
    let guild = f.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let channel = match guild {
        Some(g) if app.selected_guild != Some(g.id) => Some(g.name.clone()),
        _ => app.channels.get(&f.channel).map(|c| c.name.clone()),
    };
    let when = chrono::DateTime::from_timestamp(f.timestamp, 0).map(|t| crate::i18n::clock(t.with_timezone(&chrono::Local)));
    let footer_text = match (channel, when) {
        (Some(ch), Some(t)) => format!("{ch} \u{2022} {t}"),
        (Some(ch), None) => ch.to_string(),
        (None, Some(t)) => t,
        (None, None) => String::new(),
    };
    div()
        .flex()
        .w_full()
        .child(
            div().flex_none().w(z(4.)).rounded(z(2.)).mr(z(12.)).bg(tk(idx::BORDER_SUBTLE)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_grow(1.)
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .mt(z(2.))
                        .mb(z(4.))
                        .h(z(18.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(tk(idx::TEXT_MUTED))
                        .child(gpui::svg().path(crate::icons::path("chat/forwarded")).size(z(16.)).flex_none().text_color(tk(idx::TEXT_MUTED)))
                        .child(div().font_weight(FontWeight::SEMIBOLD).italic().child(crate::t!("chat.forwarded"))),
                )
                .child(body)
                .when(!footer_text.is_empty(), |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .mt(z(4.))
                            .py(z(1.))
                            .rounded(z(4.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tk(idx::TEXT_MUTED))
                            .when_some(guild, |d, g| d.child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 16. * crate::theme::zoom())))
                            .child(div().min_w_0().text_ellipsis().child(format!("{footer_text} \u{203A}"))),
                    )
                }),
        )
        .into_any_element()
}
