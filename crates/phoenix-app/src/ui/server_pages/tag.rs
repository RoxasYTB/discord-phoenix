use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::{ProfilePatch, ServerCall};
use serde_json::Value;

use super::{fetch, label, page, sf, str_of, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, z},
    ui::controls::{BtnKind, button, switch},
};

pub const BADGES: [&str; 10] = [
    "gs2.badge_leaf",
    "gs2.badge_sword",
    "gs2.badge_heart",
    "gs2.badge_fire",
    "gs2.badge_water",
    "gs2.badge_skull",
    "gs2.badge_moon",
    "gs2.badge_bolt",
    "gs2.badge_compass",
    "gs2.badge_mushroom",
];

pub const COLORS: [(u32, u32); 13] = [
    (0xff2594, 0xe06fa8),
    (0xdf7d18, 0xd28f62),
    (0xc29b15, 0xbebb5e),
    (0x319c6d, 0x4ea18d),
    (0x448ea3, 0x5fa4b7),
    (0x9b5eff, 0xa683e1),
    (0xaa5ce9, 0xb47ce1),
    (0xad5faa, 0xcd68af),
    (0xd75b5b, 0xdf7878),
    (0xa0713e, 0xa38669),
    (0x848441, 0x9f9f9f),
    (0x936884, 0xb382aa),
    (0x737373, 0xa3a3a3),
];

pub fn badge_url(guild: Id, hash: &str, size: u32) -> String {
    format!("https://cdn.discordapp.com/clan-badges/{}/{hash}.png?size={size}", guild.0)
}

pub fn valid_tag(s: &str) -> bool {
    let n = s.chars().count();
    (2..=4).contains(&n) && s.chars().all(|c| c.is_alphanumeric())
}

fn tile(id: impl Into<gpui::ElementId>, selected: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .size(z(64.))
        .flex_none()
        .rounded(z(4.))
        .border_1()
        .border_color(t(if selected { idx::BRAND_400 } else { idx::BORDER_SUBTLE }))
        .hover(move |s| if selected { s } else { s.border_color(t(idx::BORDER_STRONG)) })
        .bg(t(idx::BACKGROUND_MOD_MUTED))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
}

fn badge_img(guild: Id, hash: &str, size: f32) -> AnyElement {
    gpui::img(SharedString::from(badge_url(guild, hash, 64))).size(z(size)).into_any_element()
}

fn grid(items: Vec<gpui::Stateful<gpui::Div>>) -> gpui::Div {
    let mut g = div().flex().flex_col().gap(z(8.));
    let mut it = items.into_iter().peekable();
    while it.peek().is_some() {
        let mut row = div().flex().gap(z(8.));
        for _ in 0..5 {
            if let Some(x) = it.next() {
                row = row.child(x);
            }
        }
        g = g.child(row);
    }
    g
}

fn sample_message(name: &str, msg: SharedString, tag: Option<(&str, Option<AnyElement>)>, dim: bool) -> gpui::Div {
    div().flex().gap(z(12.)).when(dim, |d| d.opacity(0.3)).child(div().size(z(40.)).flex_none().rounded_full().bg(t(idx::BACKGROUND_MOD_STRONG))).child(
        div()
            .flex()
            .flex_col()
            .child(div().flex().items_center().gap(z(6.)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, name.to_owned())).children(tag.map(
                |(t_, b)| {
                    div().px(z(4.)).h(z(16.)).rounded(z(4.)).bg(t(idx::BACKGROUND_MOD_NORMAL)).flex().items_center().gap(z(2.)).children(b).child(text(
                        12.,
                        16.,
                        FontWeight::SEMIBOLD,
                        idx::TEXT_STRONG,
                        t_.to_owned(),
                    ))
                },
            )))
            .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, msg)),
    )
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let prof = fetch(app, guild, ServerCall::Profile { guild: sf(guild) }, cx);
    let Some(prof) = prof else { return page().child(title(crate::t!("menu.gs.tag"))).child(super::loading()).into_any_element() };
    let tag = prof.get("tag").and_then(Value::as_str).map(str::to_owned);
    let enabled = tag.is_some();
    let badge = prof.get("badge").and_then(Value::as_u64).unwrap_or(0) as usize;
    let hash = str_of(&prof, "badge_hash").to_owned();
    let primary = str_of(&prof, "badge_color_primary").trim_start_matches('#').to_owned();
    let preset = COLORS.iter().position(|(p, _)| format!("{p:06x}") == primary);
    let tag_s = tag.clone().unwrap_or_default();
    let mut badges = Vec::new();
    for (i, key) in BADGES.iter().enumerate() {
        let sel = i == badge;
        let mut tl = tile(("gs2-badge", i), sel).tooltip(crate::ui::tooltip::tip(crate::t!(key)));
        if sel && !hash.is_empty() {
            tl = tl.child(badge_img(guild, &hash, 32.));
        }
        badges.push(tl.on_click(cx.listener(move |a, _, _, cx| {
            write(a, ServerCall::SetProfile { guild: sf(guild), patch: ProfilePatch { badge: Some(i as u32), ..Default::default() } }, None, cx);
        })));
    }
    let mut colors = Vec::new();
    for (i, (p, s)) in COLORS.iter().copied().enumerate() {
        colors.push(
            tile(("gs2-color", i), preset == Some(i))
                .child(div().size(z(32.)).rounded(z(4.)).bg(gpui::linear_gradient(
                    135.,
                    gpui::linear_color_stop(c(s), 0.5),
                    gpui::linear_color_stop(c(p), 0.5),
                )))
                .on_click(cx.listener(move |a, _, _, cx| {
                    let patch =
                        ProfilePatch { badge_color_primary: Some(format!("#{p:06x}")), badge_color_secondary: Some(format!("#{s:06x}")), ..Default::default() };
                    write(a, ServerCall::SetProfile { guild: sf(guild), patch }, None, cx);
                })),
        );
    }
    colors.push(tile("gs2-color-reset", false).child(super::icon("refresh", 24., idx::INTERACTIVE_ICON_DEFAULT)));
    colors.push(tile("gs2-color-custom", preset.is_none()).child(super::icon("palette", 20., idx::INTERACTIVE_ICON_DEFAULT)));
    let left = div()
        .w(z(332.))
        .flex_none()
        .flex()
        .flex_col()
        .child(div().mt(z(24.)).child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.tag_lead"))))
        .child(
            div()
                .mt(z(30.))
                .px(z(16.))
                .py(z(20.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .flex()
                .items_center()
                .justify_between()
                .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.tag_enable")))
                .child(switch("gs2-tag-enable", enabled).on_click(cx.listener(move |a, _, _, cx| {
                    let patch = ProfilePatch { tag: Some(if enabled { None } else { Some("TAG".into()) }), ..Default::default() };
                    write(a, ServerCall::SetProfile { guild: sf(guild), patch }, None, cx);
                }))),
        )
        .child(div().mt(z(50.)).child(label(crate::t!("gs2.tag_name"))))
        .child(
            div()
                .mt(z(8.))
                .flex()
                .items_center()
                .gap(z(12.))
                .child(
                    div()
                        .w(z(166.))
                        .h(z(70.))
                        .px(z(16.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(t(idx::BRAND_400))
                        .bg(t(idx::BACKGROUND_BASE_LOWEST))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .children((!hash.is_empty()).then(|| badge_img(guild, &hash, 40.)))
                        .child(match super::input(guild, "tag", &tag_s, cx) {
                            Some(e) => div()
                                .w(z(84.))
                                .text_size(z(24.))
                                .text_color(t(idx::TEXT_DEFAULT))
                                .child(gpui_component::input::Input::new(&e).appearance(false).bordered(false))
                                .into_any_element(),
                            None => div().text_size(z(24.)).text_color(t(idx::TEXT_DEFAULT)).child(tag_s.clone()).into_any_element(),
                        }),
                )
                .child(div().w(z(154.)).child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.tag_rules")))),
        )
        .child(div().mt(z(40.)).child(label(crate::t!("gs2.tag_badge"))))
        .child(div().mt(z(4.)).mb(z(8.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!("gs2.tag_badge_desc"))))
        .child(grid(badges))
        .child(div().mt(z(22.)).flex().justify_center().child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, crate::t!("gs2.tag_all_badges"))))
        .child(div().mt(z(64.)).mb(z(8.)).child(label(crate::t!("gs2.tag_color"))))
        .child(grid(colors));
    let me = app.me.name.clone();
    let tag_el = |hash: &str| (!hash.is_empty()).then(|| badge_img(guild, hash, 12.));
    let right = div()
        .ml(z(40.))
        .w(z(288.))
        .flex_none()
        .flex()
        .flex_col()
        .gap(z(12.))
        .pt(z(12.))
        .child(sample_message("Roka", crate::t!("gs2.tag_s1"), None, true))
        .child(sample_message("hongo", crate::t!("gs2.tag_s2"), None, true))
        .child(sample_message("Lily", crate::t!("gs2.tag_s3"), Some(("GLAD", None)), false))
        .child(sample_message("sharon", crate::t!("gs2.tag_s4"), None, true))
        .child(sample_message("Lily", crate::t!("gs2.tag_s5"), None, true))
        .child(div().mt(z(32.)).child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.tag_current"))))
        .child(div().mt(z(13.)).px(z(16.)).child(sample_message(&me, crate::t!("gs2.tag_s6"), Some((tag_s.as_str(), tag_el(&hash))), false)))
        .child(div().mx(z(16.)).child(button("gs2-tag-use", crate::t!("gs2.tag_use"), BtnKind::Primary).w_full().when(enabled, |b| b.opacity(0.5))));
    let typed = super::value(guild, "tag", cx).filter(|v| *v != tag_s);
    let bar = typed.map(|v| {
        let ok = valid_tag(&v);
        let reset: crate::ui::modals::settings_layout::Act = std::rc::Rc::new(move |_, _, _| super::forget_input(guild, "tag"));
        let save: crate::ui::modals::settings_layout::Act = std::rc::Rc::new(move |a, _, cx| {
            let v = super::value(guild, "tag", cx).unwrap_or_default();
            if ok {
                write(
                    a,
                    ServerCall::SetProfile { guild: sf(guild), patch: ProfilePatch { tag: Some(Some(v.to_uppercase())), ..Default::default() } },
                    None,
                    cx,
                );
            }
        });
        div().mt(z(24.)).child(crate::ui::modals::settings_layout::unsaved_bar(reset, save, cx))
    });
    page().child(title(crate::t!("menu.gs.tag"))).child(div().flex().child(left).child(right)).children(bar).into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn tags() {
        assert!(super::valid_tag("GLAD"));
        assert!(!super::valid_tag("GLADOS"));
        assert!(!super::valid_tag("G!"));
        assert_eq!(super::badge_url(crate::model::Id(5), "ab", 16), "https://cdn.discordapp.com/clan-badges/5/ab.png?size=16");
    }
}
