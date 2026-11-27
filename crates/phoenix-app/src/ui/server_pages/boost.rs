use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::{SafetyEdit, ServerCall};
use serde_json::Value;

use super::{divider, fetch, icon, link, page, sf, str_of, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button, switch},
};

pub fn image_url(guild: Id, kind: &str, hash: &str) -> String {
    let ext = if hash.starts_with("a_") { "gif" } else { "png" };
    format!("https://cdn.discordapp.com/{kind}/{}/{hash}.{ext}?size=640", guild.0)
}

fn tier_pill(s: SharedString, unlocked: bool) -> gpui::Div {
    div()
        .px(z(6.))
        .py(z(1.))
        .rounded(z(12.))
        .bg(t(idx::BACKGROUND_MOD_SUBTLE))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(icon("boost", 14., if unlocked { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE }))
        .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, s))
}

fn uploader(url: Option<String>, h: f32) -> gpui::Div {
    let frame = div().relative().w(z(314.)).h(z(h)).rounded(z(8.)).overflow_hidden().bg(t(idx::BACKGROUND_MOD_STRONG)).child(match url {
        Some(u) => gpui::img(SharedString::from(u)).size_full().object_fit(gpui::ObjectFit::Cover).into_any_element(),
        None => div().size_full().flex().items_center().justify_center().child(icon("image-plus", 32., idx::ICON_SUBTLE)).into_any_element(),
    });
    div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(frame.child(div().absolute().top(z(12.)).right(z(12.)).size(z(28.)).rounded_full().bg(gpui::white()).flex().items_center().justify_center().child(icon("pen", 16., idx::BLACK))))
        .child(div().flex().justify_center().child(text(14., 14., FontWeight::NORMAL, idx::TEXT_LINK, crate::t!("gs2.remove"))))
}

fn perk_section(head: SharedString, pill: gpui::Div, lines: [SharedString; 2], cta: &'static str, side: gpui::Div) -> gpui::Div {
    div()
        .flex()
        .gap(z(32.))
        .child(
            div()
                .w(z(314.))
                .flex_none()
                .flex()
                .flex_col()
                .child(div().flex().justify_between().items_start().gap(z(8.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, head)).child(pill))
                .child(div().mt(z(8.)).child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, lines[0].clone())))
                .child(div().mt(z(8.)).child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, lines[1].clone())).child(link(crate::t!("gs2.learn_more"))))
                .child(div().mt(z(16.)).flex().child(button(cta, crate::t!("gs2.upload_bg"), BtnKind::Primary))),
        )
        .child(side)
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx);
    let vanity = fetch(app, guild, ServerCall::VanityUrl { guild: sf(guild) }, cx);
    let Some(g) = g else { return page().child(title(crate::t!("menu.gs.boost_perks"))).child(super::loading()).into_any_element() };
    let bar = g.get("premium_progress_bar_enabled").and_then(Value::as_bool).unwrap_or(false);
    let tier = g.get("premium_tier").and_then(Value::as_u64).unwrap_or(0);
    let banner = g.get("banner").and_then(Value::as_str).map(|h| image_url(guild, "banners", h));
    let splash = g.get("splash").and_then(Value::as_str).map(|h| image_url(guild, "splashes", h));
    let unlocked = |need: u64| tier_pill(if tier >= need { crate::t!("gs2.unlocked") } else { crate::t!("gs2.tier_n", n = need) }, tier >= need);
    let mut p = page()
        .child(title(crate::t!("menu.gs.boost_perks")))
        .child(
            div()
                .mt(z(40.))
                .flex()
                .gap(z(32.))
                .child(
                    div().w(z(314.)).flex_none().flex().items_start().gap(z(28.)).child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(z(4.))
                            .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("gs2.progress_bar")))
                            .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!("gs2.progress_bar_desc"))),
                    )
                    .child(switch("gs2-progress-bar", bar).on_click(cx.listener(move |a, _, _, cx| {
                        write(a, ServerCall::EditSafety { guild: sf(guild), edit: SafetyEdit { premium_progress_bar_enabled: Some(!bar), ..Default::default() } }, None, cx);
                    }))),
                )
                .child(progress_preview()),
        )
        .child(divider(32.))
        .child(perk_section(
            crate::t!("gs2.banner_bg"),
            unlocked(2),
            [crate::t!("gs2.banner_bg_desc"), crate::t!("gs2.banner_bg_size")],
            "gs2-upload-banner",
            uploader(banner, 180.),
        ))
        .child(divider(32.))
        .child(perk_section(
            crate::t!("gs2.invite_bg"),
            unlocked(1),
            [crate::t!("gs2.invite_bg_desc"), crate::t!("gs2.invite_bg_size")],
            "gs2-upload-splash",
            uploader(splash, 180.),
        ))
        .child(divider(32.));
    p = p.child(vanity_section(guild, tier, vanity.as_ref(), cx));
    p.into_any_element()
}

pub fn progress_preview_url(light: bool) -> &'static str {
    if light {
        "https://cdn.discordapp.com/assets/content/05acf6b46b4f51bf327d5eebb8c689604b37656529247b6d9984131f1a8d286c.png"
    } else {
        "https://cdn.discordapp.com/assets/content/b8d65794b7cc0b9eeca2f0842d619d0da7f31312a984da6146429bc1570995fc.png"
    }
}

fn progress_preview() -> gpui::Img {
    gpui::img(progress_preview_url(crate::theme::is_light())).flex_none().w(z(314.)).h(z(314. * 975. / 981.)).object_fit(gpui::ObjectFit::Contain)
}

fn vanity_bar(guild: Id, code: &str, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let typed = super::value(guild, "vanity", cx)?;
    if typed == code {
        return None;
    }
    let ok = vanity_ok(&typed);
    let reset: crate::ui::modals::settings_layout::Act = std::rc::Rc::new(move |_, _, _| super::forget_input(guild, "vanity"));
    let save: crate::ui::modals::settings_layout::Act = std::rc::Rc::new(move |a, _, cx| {
        let code = super::value(guild, "vanity", cx).unwrap_or_default();
        if ok {
            write(a, ServerCall::SetVanityUrl { guild: sf(guild), code }, None, cx);
        }
    });
    Some(div().mt(z(16.)).child(crate::ui::modals::settings_layout::unsaved_bar(reset, save, cx)).into_any_element())
}

pub fn next_goal(boosts: u64) -> u64 {
    match boosts {
        0..=1 => 2,
        2..=6 => 7,
        _ => 14,
    }
}

pub fn vanity_ok(code: &str) -> bool {
    !code.is_empty() && code.len() <= 25 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn vanity_section(guild: Id, tier: u64, v: Option<&Value>, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let code = v.map(|v| str_of(v, "code").to_owned()).unwrap_or_default();
    let uses = v.and_then(|v| v.get("uses")).and_then(Value::as_u64).unwrap_or(0);
    let locked = tier < 3;
    div()
        .flex()
        .flex_col()
        .child(div().flex().items_center().gap(z(8.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("gs2.vanity"))).child(tier_pill(crate::t!("gs2.tier_n", n = 3), !locked)))
        .child(div().mt(z(8.)).child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.vanity_desc"))).child(link(crate::t!("gs2.learn_more_dot"))))
        .child(
            div()
                .mt(z(16.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .when(locked, |d| d.opacity(0.5))
                .flex()
                .flex_col()
                .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.invite_url")))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.n_uses", n = uses)))
                .child(
                    div()
                        .mt(z(12.))
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, "https://discord.gg/"))
                        .child(super::field(guild, "vanity", &code, cx).flex_1()),
                )
                .child(div().mt(z(12.)).flex().child(button("gs2-vanity-remove", crate::t!("gs2.vanity_remove"), BtnKind::Critical).on_click(cx.listener(move |a, _, _, cx| {
                    write(a, ServerCall::SetVanityUrl { guild: sf(guild), code: String::new() }, None, cx);
                })))),
        )
        .children(vanity_bar(guild, &code, cx))
        .when(!code.is_empty(), |d| {
            d.child(
                div()
                    .mt(z(16.))
                    .flex()
                    .gap(z(4.))
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.vanity_info")))
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_LINK, format!("https://discord.gg/{code}"))),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boost_helpers() {
        assert_eq!(image_url(Id(1), "banners", "a_x"), "https://cdn.discordapp.com/banners/1/a_x.gif?size=640");
        assert_eq!(next_goal(9), 14);
        assert!(vanity_ok("aperture-sciences"));
        assert!(!vanity_ok("bad code"));
    }
}
