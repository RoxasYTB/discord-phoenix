use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::{SafetyEdit, ServerCall};
use serde_json::Value;

use super::{description, features, fetch, icon, id_of, label, page, sf, str_of, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button, button_sm, select_field},
};

pub const LOCALES: [(&str, &str); 6] =
    [("fr", "Français"), ("en-US", "English, US"), ("en-GB", "English, UK"), ("de", "Deutsch"), ("es-ES", "Español"), ("it", "Italiano")];

pub fn locale_name(code: &str) -> String {
    LOCALES.iter().find(|(c, _)| *c == code).map(|(_, n)| (*n).to_owned()).unwrap_or_else(|| code.to_owned())
}

pub fn disable_features(feats: &[String]) -> Vec<String> {
    feats
        .iter()
        .filter(|f| !matches!(f.as_str(), "COMMUNITY" | "DISCOVERABLE" | "NEWS" | "MEMBER_VERIFICATION_GATE_ENABLED" | "PREVIEW_ENABLED"))
        .cloned()
        .collect()
}

fn row(head: SharedString, desc: SharedString, ctrl: AnyElement, hint: Option<SharedString>, last: bool) -> gpui::Div {
    div()
        .py(z(24.))
        .when(!last, |d| d.border_b_1().border_color(t(idx::BORDER_SUBTLE)))
        .flex()
        .gap(z(24.))
        .child(div().w(z(388.)).flex_none().flex().flex_col().gap(z(4.)).child(label(head)).child(description(desc)))
        .child(
            div().flex_1().min_w_0().flex().flex_col().gap(z(8.)).child(ctrl).children(hint.map(|h| text(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, h))),
        )
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx);
    let Some(g) = g else { return page().child(title(crate::t!("gs2.community_title"))).child(super::loading()).into_any_element() };
    let feats = features(&g);
    let community = feats.iter().any(|f| f == "COMMUNITY");
    if !community {
        return enable_wizard(cx);
    }
    let dismissed = super::flag(guild, "community-banner").unwrap_or(false);
    let mut p = page().child(title(crate::t!("gs2.community_title")));
    if !dismissed {
        p = p.child(
            div()
                .mt(z(40.))
                .relative()
                .p(z(16.))
                .rounded(z(8.))
                .bg(t(idx::CARD_BACKGROUND_DEFAULT))
                .border_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .flex()
                .flex_col()
                .child(div().pr(z(40.)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.community_now"))))
                .child(div().mt(z(8.)).pr(z(40.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.community_now_desc"))))
                .child(
                    div()
                        .mt(z(28.))
                        .flex()
                        .gap(z(8.))
                        .child(button_sm("gs2-com-onboarding", crate::t!("gs2.community_setup_onboarding"), BtnKind::Primary).on_click(cx.listener(
                            |a, _, _, cx| {
                                crate::ui::modals::server_settings::pick(crate::ui::modals::server_settings::section_index("menu.gs.onboarding"), a, cx);
                                cx.notify();
                            },
                        )))
                        .child(button_sm("gs2-com-analytics", crate::t!("gs2.community_see_analytics"), BtnKind::Secondary).on_click(cx.listener(
                            |a, _, _, cx| {
                                crate::ui::modals::server_settings::pick(crate::ui::modals::server_settings::section_index("menu.gs.analytics"), a, cx);
                                cx.notify();
                            },
                        ))),
                )
                .child(
                    div()
                        .id("gs2-com-dismiss")
                        .absolute()
                        .top(z(4.))
                        .right(z(12.))
                        .size(z(40.))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(t(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER)))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            super::set_flag(guild, "community-banner", true);
                            cx.notify();
                        }))
                        .child(icon("close", 24., idx::ICON_DEFAULT)),
                ),
        );
    }
    let rules = id_of(&g, "rules_channel_id");
    let updates = id_of(&g, "public_updates_channel_id");
    let safety = id_of(&g, "safety_alerts_channel_id");
    let locale = str_of(&g, "preferred_locale").to_owned();
    let desc = str_of(&g, "description").to_owned();
    p = p
        .child(div().mt(z(16.)))
        .child(row(
            crate::t!("gs2.rules_channel"),
            crate::t!("gs2.rules_channel_desc"),
            select_field("gs2-rules", super::channel_label(app, rules)).into_any_element(),
            None,
            false,
        ))
        .child(row(
            crate::t!("gs2.updates_channel"),
            crate::t!("gs2.updates_channel_desc"),
            select_field("gs2-updates", super::channel_label(app, updates)).into_any_element(),
            Some(crate::t!("gs2.optional")),
            false,
        ))
        .child(row(
            crate::t!("gs2.safety_channel"),
            crate::t!("gs2.safety_channel_desc"),
            select_field("gs2-safety", super::channel_label(app, safety)).into_any_element(),
            None,
            false,
        ))
        .child(row(crate::t!("gs2.locale"), crate::t!("gs2.locale_desc"), select_field("gs2-locale", locale_name(&locale)).into_any_element(), None, false))
        .child(
            div()
                .py(z(24.))
                .border_b_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(label(crate::t!("gs2.server_description")))
                .child(description(crate::t!("gs2.server_description_desc")))
                .child(
                    div()
                        .mt(z(8.))
                        .h(z(88.))
                        .p(z(12.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(t(idx::INPUT_BORDER_DEFAULT))
                        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
                        .overflow_hidden()
                        .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, desc)),
                ),
        );
    let feats2 = feats.clone();
    p.child(
        div()
            .py(z(24.))
            .flex()
            .justify_between()
            .gap(z(24.))
            .child(
                div()
                    .w(z(451.))
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(label(crate::t!("gs2.disable_community")))
                    .child(description(crate::t!("gs2.disable_community_desc"))),
            )
            .child(button_sm("gs2-com-disable", crate::t!("gs2.disable_community"), BtnKind::Critical).on_click(cx.listener(move |a, _, _, cx| {
                write(
                    a,
                    ServerCall::EditSafety { guild: sf(guild), edit: SafetyEdit { features: Some(disable_features(&feats2)), ..Default::default() } },
                    None,
                    cx,
                );
            }))),
    )
    .into_any_element()
}

fn enable_wizard(cx: &mut Context<PhoenixApp>) -> AnyElement {
    page()
        .child(title(crate::t!("gs2.community_enable_title")))
        .child(div().mt(z(8.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.community_enable_lead"))))
        .child(
            div()
                .mt(z(24.))
                .flex()
                .child(button("gs2-com-enable", crate::t!("gs2.community_enable"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| cx.notify()))),
        )
        .into_any_element()
}

pub fn onboarding(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ob = fetch(app, guild, ServerCall::Onboarding { guild: sf(guild) }, cx);
    let Some(ob) = ob else { return page().child(super::loading()).into_any_element() };
    let wizard = super::flag(guild, "onboarding-wizard").unwrap_or(false);
    if wizard {
        return onboarding_wizard(app, guild, &ob, cx);
    }
    let enabled = ob.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let mut p = page()
        .items_center()
        .child(div().w(z(288.)).h(z(192.)).rounded(z(8.)).bg(t(idx::BACKGROUND_MOD_SUBTLE)))
        .child(div().mt(z(16.)).w_full().child(text(24., 32., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.ob_title")).text_center()))
        .child(div().mt(z(8.)).w_full().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.ob_lead")).text_center()))
        .child(div().mt(z(16.)).child(button("gs2-ob-start", crate::t!(if enabled { "gs2.ob_edit" } else { "gs2.ob_look" }), BtnKind::Primary).on_click(
            cx.listener(move |_, _, _, cx| {
                super::set_flag(guild, "onboarding-wizard", true);
                cx.notify();
            }),
        )));
    p = p.child(
        div()
            .mt(z(64.))
            .w_full()
            .p(z(24.))
            .rounded(z(8.))
            .bg(t(idx::CARD_BACKGROUND_DEFAULT))
            .flex()
            .flex_col()
            .items_center()
            .gap(z(8.))
            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.ob_quote")).text_center())
            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.ob_quote_by"))),
    );
    let prompts = ob.get("prompts").and_then(Value::as_array).cloned().unwrap_or_default();
    let defaults = ob.get("default_channel_ids").and_then(Value::as_array).map_or(0, Vec::len);
    p.child(
        div()
            .mt(z(64.))
            .w_full()
            .flex()
            .gap(z(48.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(12.))
                    .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.ob_prompts_title")))
                    .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.ob_prompts_desc")))
                    .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("gs2.ob_n_prompts", n = prompts.len()))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(12.))
                    .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.ob_defaults_title")))
                    .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.ob_defaults_desc")))
                    .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("gs2.ob_n_defaults", n = defaults))),
            ),
    )
    .into_any_element()
}

pub const STEPS: [&str; 5] = ["gs2.ob_step1", "gs2.ob_step2", "gs2.ob_step3", "gs2.ob_step4", "gs2.ob_step5"];

fn onboarding_wizard(app: &PhoenixApp, guild: Id, ob: &Value, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let step = super::with(|s| s.text.get(&(guild, "ob-step".to_owned())).and_then(|v| v.parse::<usize>().ok())).unwrap_or(0);
    let mut bar = div().flex().gap(z(8.));
    for (i, k) in STEPS.iter().enumerate() {
        let on = i <= step;
        bar = bar.child(
            div()
                .id(("gs2-ob-step", i))
                .flex_1()
                .h(z(44.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    if i <= step {
                        super::with(|s| {
                            s.text.insert((guild, "ob-step".into()), i.to_string());
                        });
                        cx.notify();
                    }
                }))
                .child(div().h(z(4.)).rounded(z(2.)).bg(t(if on { idx::BRAND_500 } else { idx::BACKGROUND_MOD_STRONG })))
                .child(text(12., 16., FontWeight::MEDIUM, if i == step { idx::TEXT_BRAND } else { idx::TEXT_MUTED }, crate::t!(k))),
        );
    }
    let body = match step {
        0 => super::safety::render(app, guild, cx),
        1 => {
            let mut d = div().flex().flex_col().gap(z(8.)).child(title(crate::t!("gs2.ob_step2")));
            for ch in ob.get("default_channel_ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                let id = ch.parse().ok().map(Id);
                d = d.child(
                    div()
                        .h(z(48.))
                        .px(z(16.))
                        .rounded(z(8.))
                        .bg(t(idx::CARD_BACKGROUND_DEFAULT))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(icon("hash", 20., idx::ICON_SUBTLE))
                        .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, super::channel_label(app, id)))
                        .child(div().flex_1())
                        .child(crate::ui::community::checkbox(true)),
                );
            }
            d.into_any_element()
        }
        _ => {
            let mut d = div().flex().flex_col().gap(z(16.)).child(title(crate::t!(STEPS[step])));
            for pr in ob.get("prompts").and_then(Value::as_array).into_iter().flatten() {
                let mut opts = div().flex().flex_wrap().gap(z(8.));
                for o in pr.get("options").and_then(Value::as_array).into_iter().flatten() {
                    opts = opts.child(div().px(z(16.)).py(z(12.)).rounded(z(8.)).bg(t(idx::BACKGROUND_MOD_SUBTLE)).child(text(
                        14.,
                        18.,
                        FontWeight::MEDIUM,
                        idx::TEXT_STRONG,
                        str_of(o, "title").to_owned(),
                    )));
                }
                d = d.child(
                    div()
                        .p(z(16.))
                        .rounded(z(8.))
                        .bg(t(idx::CARD_BACKGROUND_DEFAULT))
                        .flex()
                        .flex_col()
                        .gap(z(12.))
                        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, str_of(pr, "title").to_owned()))
                        .child(opts),
                );
            }
            d.into_any_element()
        }
    };
    page().child(bar).child(div().mt(z(32.)).child(body)).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_helpers() {
        assert_eq!(locale_name("fr"), "Français");
        let f = vec!["COMMUNITY".to_owned(), "NEWS".into(), "ANIMATED_ICON".into(), "DISCOVERABLE".into()];
        assert_eq!(disable_features(&f), vec!["ANIMATED_ICON"]);
    }
}
