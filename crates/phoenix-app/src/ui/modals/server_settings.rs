use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, deferred, div, prelude::*};

use crate::{
    actions::{close_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

use super::Modal;

pub const SECTIONS: [&str; 21] = [
    "menu.gs.profile",
    "menu.gs.tag",
    "menu.gs.engagement",
    "menu.gs.boost_perks",
    "menu.gs.emoji",
    "menu.gs.stickers",
    "menu.gs.soundboard",
    "menu.gs.members",
    "menu.gs.roles",
    "menu.gs.invites",
    "menu.gs.access",
    "menu.gs.integrations",
    "menu.gs.app_directory",
    "menu.gs.safety",
    "menu.gs.audit_log",
    "menu.gs.bans",
    "menu.gs.community",
    "menu.gs.onboarding",
    "menu.gs.analytics",
    "menu.gs.discovery",
    "menu.gs.templates",
];

pub fn section_index(key: &str) -> usize {
    SECTIONS.iter().position(|k| *k == key).unwrap_or(0)
}

fn info(label: SharedString, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .pb(z(16.))
        .child(div().text_size(z(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_SUBTLE))).child(label))
        .child(div().text_size(z(16.)).text_color(c(tok(idx::TEXT_STRONG))).child(value.into()))
}

fn content(app: &PhoenixApp, guild: Id, section: usize) -> gpui::Div {
    let g = app.guilds.iter().find(|x| x.id == guild);
    let mut d = div().flex().flex_col();
    match SECTIONS[section] {
        "menu.gs.profile" => {
            d = d
                .child(info(crate::t!("gs.name"), g.map(|g| g.name.clone()).unwrap_or_default()))
                .child(info(crate::t!("gs.id"), guild.0.to_string()));
        }
        "menu.gs.emoji" => {
            let emojis = app.backend.guild_emojis(guild);
            d = d.child(info(crate::t!("menu.gs.emoji"), emojis.len().to_string()));
        }
        "menu.gs.stickers" => {
            d = d.child(info(crate::t!("menu.gs.stickers"), app.backend.guild_stickers(guild).len().to_string()));
        }
        "menu.gs.members" => {
            d = d.child(info(crate::t!("gs.loaded_members"), app.members.get(&guild).map_or(0, Vec::len).to_string()));
        }
        "menu.gs.roles" => {
            let mut roles: Vec<&crate::model::Role> = app.roles.get(&guild).into_iter().flatten().collect();
            roles.sort_by_key(|r| -r.position);
            for r in roles {
                d = d.child(
                    div()
                        .h(z(40.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .border_b_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .child(div().size(z(12.)).rounded_full().bg(crate::theme::role_color(r.color)))
                        .child(div().text_size(z(14.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(r.name.clone())),
                );
            }
        }
        "menu.gs.community" => {
            let feats = app.backend.guild_features(guild);
            d = d.child(info(crate::t!("gs.features"), if feats.is_empty() { "-".to_owned() } else { feats.join(", ") }));
        }
        _ => {}
    }
    d.child(div().pt(z(8.)).text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("gs.readonly")))
}

pub fn render(app: &PhoenixApp, guild: Id, section: usize, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.guilds.iter().find(|x| x.id == guild).map(|g| g.name.clone()).unwrap_or_default();
    let mut side = div().w(z(218.)).flex().flex_col().gap(z(2.)).pt(z(60.)).pb(z(20.)).px(z(6.)).child(
        div().px(z(10.)).pb(z(6.)).text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_SUBTLE))).truncate().child(name.to_uppercase()),
    );
    for (i, key) in SECTIONS.iter().enumerate() {
        let on = i == section;
        side = side.child(
            div()
                .id(("gs-item", i))
                .px(z(10.))
                .py(z(6.))
                .rounded(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(c(tok(if on { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
                .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_modal(|m| {
                        if let Some(Modal::ServerSettings { section, .. }) = m {
                            *section = i;
                        }
                    });
                    cx.notify();
                }))
                .child(crate::t!(key)),
        );
    }
    let body = div()
        .flex_1()
        .min_w_0()
        .pt(z(60.))
        .px(z(40.))
        .max_w(z(740.))
        .flex()
        .flex_col()
        .child(div().pb(z(20.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!(SECTIONS[section])))
        .child(content(app, guild, section));
    let close = div()
        .id("gs-close")
        .flex_none()
        .mr(z(20.))
        .mt(z(60.))
        .size(z(36.))
        .rounded_full()
        .border_2()
        .border_color(c(tok(idx::BORDER_STRONG)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(cx.listener(|_, _, _, cx| {
            close_modal();
            cx.notify();
        }))
        .child(gpui::svg().path(crate::icons::path("close")).size(z(18.)).text_color(c(tok(idx::ICON_SUBTLE))));
    deferred(
        div()
            .id("server-settings")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .child(div().id("gs-side").flex_1().flex().justify_end().overflow_y_scroll().bg(c(tok(idx::BACKGROUND_BASE_LOWER))).child(side))
            .child(div().id("gs-main").flex_1().flex().overflow_y_scroll().bg(c(tok(idx::BACKGROUND_BASE_LOW))).child(body).child(close)),
    )
    .with_priority(20)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn sections() {
        assert_eq!(super::SECTIONS.len(), 21);
        assert_eq!(super::section_index("menu.gs.roles"), 8);
    }
}
