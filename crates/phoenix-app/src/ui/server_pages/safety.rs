use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::{SafetyEdit, ServerCall};
use serde_json::Value;

use super::{Sub, back, card, features, fetch, icon, id_of, label, link, page, pill, set_sub, sf, sub, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button, button_sm, radio_indicator, switch},
};

pub const SECTION: &str = "menu.gs.safety";

pub const RISKY: [u64; 11] = [1 << 3, 1 << 1, 1 << 2, 1 << 4, 1 << 5, 1 << 13, 1 << 17, 1 << 28, 1 << 29, 1 << 30, 1 << 40];

pub const LEVELS: [(&str, &str); 4] = [
    ("gs2.vl_low", "gs2.vl_low_desc"),
    ("gs2.vl_medium", "gs2.vl_medium_desc"),
    ("gs2.vl_high", "gs2.vl_high_desc"),
    ("gs2.vl_highest", "gs2.vl_highest_desc"),
];

pub fn risky_everyone(guild: &Value) -> usize {
    let gid = super::str_of(guild, "id");
    let perms = guild
        .get("roles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|r| super::str_of(r, "id") == gid)
        .and_then(|r| r.get("permissions"))
        .and_then(|p| p.as_str().and_then(|s| s.parse::<u64>().ok()).or_else(|| p.as_u64()))
        .unwrap_or(0);
    RISKY.iter().filter(|b| perms & **b != 0).count()
}

pub struct Counts {
    pub raid: (usize, usize),
    pub dm: (usize, usize),
    pub automod: (usize, usize),
    pub perms: (usize, usize),
}

pub fn counts(guild: &Value, rules: &Value) -> Counts {
    let f = features(guild);
    let has = |k: &str| f.iter().any(|x| x == k);
    let raid_alerts = !has("RAID_ALERTS_DISABLED");
    let rules = rules.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let on = rules.iter().filter(|r| r.get("enabled").and_then(Value::as_bool).unwrap_or(false)).count();
    let mfa = guild.get("mfa_level").and_then(Value::as_u64).unwrap_or(0) == 1;
    let prune = has("PRUNE_REQUIRES_ADMIN");
    let risky = risky_everyone(guild) == 0;
    Counts {
        raid: (usize::from(raid_alerts) + 2, 3),
        dm: (5, 5),
        automod: (on, rules.len()),
        perms: (usize::from(mfa) + usize::from(prune) + usize::from(risky), 3),
    }
}

fn badge(name: &'static str, from: u32, to: u32) -> AnyElement {
    let a = crate::theme::c(from);
    let b = crate::theme::c(to);
    div()
        .size(z(40.))
        .flex_none()
        .rounded_full()
        .bg(gpui::linear_gradient(135., gpui::linear_color_stop(a, 0.), gpui::linear_color_stop(b, 1.)))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path(name)).size(z(20.)).text_color(gpui::white()))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    match sub(guild, SECTION) {
        Sub::RaidCaptcha => raid(app, guild, cx),
        Sub::DmSpam => dm_spam(app, guild, cx),
        Sub::Automod => super::automod::render(app, guild, cx),
        Sub::Permissions => permissions(app, guild, cx),
        _ => overview(app, guild, cx),
    }
}

fn overview(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx);
    let rules = fetch(app, guild, ServerCall::AutomodRules { guild: sf(guild) }, cx);
    let (Some(g), Some(rules)) = (g, rules) else {
        return page().child(title(crate::t!("menu.gs.safety"))).child(super::loading()).into_any_element();
    };
    let n = counts(&g, &rules);
    let steps: [(&'static str, (usize, usize), Sub, AnyElement); 4] = [
        ("gs2.raid_title", n.raid, Sub::RaidCaptcha, badge("shield-lock", 0xFFC24A, 0xF59C1A)),
        ("gs2.dm_title", n.dm, Sub::DmSpam, badge("chat", 0x6BE3A6, 0x3AB6D9)),
        ("gs2.automod_title", n.automod, Sub::Automod, badge("moderation", 0x5865F2, 0xEB459E)),
        ("gs2.perms_title", n.perms, Sub::Permissions, badge("user-check", 0xF77FA4, 0xE8365D)),
    ];
    let mut list = card().mt(z(32.)).flex().flex_col();
    for (i, (key, (on, of), target, art)) in steps.into_iter().enumerate() {
        if i > 0 {
            list = list.child(div().mx(z(16.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)));
        }
        list = list.child(
            div()
                .p(z(16.))
                .flex()
                .items_center()
                .justify_between()
                .child(div().flex().items_center().gap(z(16.)).child(art).child(
                    div().flex().flex_col().child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!(key))).child(text(
                        12.,
                        16.,
                        FontWeight::MEDIUM,
                        idx::TEXT_DEFAULT,
                        crate::t!("gs2.enabled_of", on = on, of = of),
                    )),
                ))
                .child(button_sm(("gs2-safety-edit", i), crate::t!("gs2.edit"), BtnKind::Secondary).on_click(cx.listener(move |_, _, _, cx| {
                    set_sub(guild, SECTION, target.clone());
                    cx.notify();
                }))),
        );
    }
    page().child(title(crate::t!("menu.gs.safety"))).child(list).into_any_element()
}

fn subpage(guild: Id, key: &'static str, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    page().child(back(guild, SECTION, cx)).child(title(crate::t!(key)).mb(z(8.)))
}

fn item(id: impl Into<gpui::ElementId>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .mb(z(8.))
        .p(z(16.))
        .flex()
        .items_start()
        .justify_between()
        .gap(z(16.))
        .bg(t(idx::CARD_BACKGROUND_DEFAULT))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .rounded(z(8.))
}

fn item_text(head: SharedString, body: Option<SharedString>) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .max_w(z(538.))
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, head))
        .children(body.map(|b| text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, b)))
}

fn locked_on(id: impl Into<gpui::ElementId>) -> gpui::Div {
    div().flex_none().w(z(72.)).flex().justify_end().opacity(0.5).child(switch(id, true))
}

fn raid(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(g) = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx) else { return super::loading() };
    let f = features(&g);
    let alerts = !f.iter().any(|x| x == "RAID_ALERTS_DISABLED");
    let chan = id_of(&g, "safety_alerts_channel_id");
    let toggled = super::flag(guild, "raid-alerts-open").unwrap_or(false);
    let mut alerts_card = div()
        .id("gs2-raid-alerts")
        .mb(z(8.))
        .bg(t(if toggled { idx::BACKGROUND_BASE_LOWER } else { idx::BACKGROUND_SURFACE_HIGH }))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .rounded(z(8.))
        .cursor_pointer()
        .on_click(cx.listener(move |_, _, _, cx| {
            super::set_flag(guild, "raid-alerts-open", !toggled);
            cx.notify();
        }))
        .child(
            div()
                .p(z(16.))
                .flex()
                .items_start()
                .justify_between()
                .gap(z(16.))
                .child(item_text(crate::t!("gs2.activity_alerts"), Some(crate::t!("gs2.activity_alerts_desc"))).child(pill(super::channel_label(app, chan))))
                .child(div().w(z(72.)).flex().justify_end().child(switch("gs2-alerts-switch", alerts).on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    let mut feats = features_of(&g);
                    if alerts {
                        feats.push("RAID_ALERTS_DISABLED".into());
                    } else {
                        feats.retain(|x| x != "RAID_ALERTS_DISABLED");
                    }
                    write(a, ServerCall::EditSafety { guild: sf(guild), edit: SafetyEdit { features: Some(feats), ..Default::default() } }, None, cx);
                })))),
        );
    if toggled {
        alerts_card = alerts_card.child(
            div()
                .px(z(16.))
                .pb(z(16.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(label(crate::t!("gs2.safety_channel")))
                .child(crate::ui::controls::select_field("gs2-alerts-chan", super::channel_label(app, chan)))
                .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.safety_channel_hint"))),
        );
    }
    subpage(guild, "gs2.raid_title", cx)
        .child(alerts_card)
        .child(item("gs2-captcha-sus").child(item_text(crate::t!("gs2.captcha_suspicious"), None)).child(locked_on("gs2-captcha-sus-sw")))
        .child(item("gs2-captcha-raid").child(item_text(crate::t!("gs2.captcha_raid"), None)).child(locked_on("gs2-captcha-raid-sw")))
        .into_any_element()
}

fn features_of(g: &Value) -> Vec<String> {
    features(g)
}

fn dm_spam(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(g) = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx) else { return super::loading() };
    let level = g.get("verification_level").and_then(Value::as_u64).unwrap_or(0) as usize;
    let lv = if level == 0 { crate::t!("gs2.vl_none") } else { crate::t!(LEVELS[(level - 1).min(3)].0) };
    let mut p = subpage(guild, "gs2.dm_title", cx).child(
        item("gs2-verif")
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                super::open_modal(guild, "verification");
                cx.notify();
            }))
            .child(
                item_text(crate::t!("gs2.verification"), Some(crate::t!("gs2.verification_desc")))
                    .child(div().flex().child(pill(lv).child(icon(if level == 4 { "phone-call" } else { "inbox" }, 16., idx::TEXT_SUBTLE)).flex_row_reverse())),
            )
            .child(icon("chevron-right", 24., idx::ICON_DEFAULT)),
    );
    for (i, (h, d)) in
        [("gs2.dm_hide", Some("gs2.dm_hide_desc")), ("gs2.dm_filter", Some("gs2.dm_filter_desc")), ("gs2.links_warn", None), ("gs2.spam_delete", None)]
            .into_iter()
            .enumerate()
    {
        p = p.child(item(("gs2-dm", i)).child(item_text(crate::t!(h), d.map(|d| crate::t!(d)))).child(locked_on(("gs2-dm-sw", i))));
    }
    p.into_any_element()
}

fn permissions(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(g) = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx) else { return super::loading() };
    let mfa = g.get("mfa_level").and_then(Value::as_u64).unwrap_or(0) == 1;
    let discoverable = features(&g).iter().any(|x| x == "DISCOVERABLE");
    let prune = features(&g).iter().any(|x| x == "PRUNE_REQUIRES_ADMIN");
    let risky = risky_everyone(&g);
    let mfa_switch = if discoverable {
        locked_on("gs2-mfa-sw").into_any_element()
    } else {
        div()
            .w(z(72.))
            .flex()
            .justify_end()
            .child(switch("gs2-mfa-sw", mfa).on_click(cx.listener(move |a, _, _, cx| {
                write(a, ServerCall::SetMfaLevel { guild: sf(guild), level: u8::from(!mfa) }, None, cx);
            })))
            .into_any_element()
    };
    let g2 = g.clone();
    subpage(guild, "gs2.perms_title", cx)
        .child(item("gs2-mfa").child(item_text(crate::t!("gs2.mfa"), Some(crate::t!("gs2.mfa_desc")))).child(mfa_switch))
        .child(item("gs2-prune").child(item_text(crate::t!("gs2.prune_admin"), Some(crate::t!("gs2.prune_admin_desc")))).child(
            div().w(z(72.)).flex().justify_end().child(switch("gs2-prune-sw", prune).on_click(cx.listener(move |a, _, _, cx| {
                let mut feats = features(&g2);
                if prune {
                    feats.retain(|x| x != "PRUNE_REQUIRES_ADMIN");
                } else {
                    feats.push("PRUNE_REQUIRES_ADMIN".into());
                }
                write(a, ServerCall::EditSafety { guild: sf(guild), edit: SafetyEdit { features: Some(feats), ..Default::default() } }, None, cx);
            }))),
        ))
        .child(
            item("gs2-risky")
                .child(item_text(crate::t!("gs2.risky"), Some(crate::t!("gs2.risky_desc", n = risky))))
                .children((risky > 0).then(|| icon("warning", 24., idx::TEXT_FEEDBACK_CRITICAL))),
        )
        .into_any_element()
}

pub fn verification_modal(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let cur =
        fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx).and_then(|g| g.get("verification_level").and_then(Value::as_u64)).unwrap_or(0) as usize;
    let picked = super::with(|s| s.text.get(&(guild, "verification".to_owned())).and_then(|v| v.parse::<usize>().ok())).unwrap_or(cur.max(1));
    let e = cx.entity();
    let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
        super::close_modal();
        super::with(|s| {
            s.text.remove(&(guild, "verification".to_owned()));
        });
        e.update(cx, |_, cx| cx.notify());
    });
    let mut body = div().flex().flex_col().child(
        div()
            .pb(z(24.))
            .text_size(z(16.))
            .line_height(z(20.))
            .text_color(t(idx::TEXT_DEFAULT))
            .child(crate::t!("gs2.verification_modal_desc"))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::t!("gs2.verification_modal_reco"))),
    );
    for (i, (h, d)) in LEVELS.iter().enumerate() {
        let lvl = i + 1;
        body = body.child(
            div()
                .id(("gs2-vl", i))
                .pb(z(12.))
                .flex()
                .gap(z(12.))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    super::with(|s| {
                        s.text.insert((guild, "verification".into()), lvl.to_string());
                    });
                    cx.notify();
                }))
                .child(div().pt(z(0.)).child(radio_indicator(picked == lvl)))
                .child(div().flex().flex_col().gap(z(4.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(h))).child(text(
                    14.,
                    18.,
                    FontWeight::NORMAL,
                    idx::TEXT_SUBTLE,
                    crate::t!(d),
                ))),
        );
    }
    let close2 = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("gs2-vl-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| close2(w, cx)))
        .child(button("gs2-vl-save", crate::t!("common.save"), BtnKind::Primary).flex_1().on_click(cx.listener(move |a, _, _, cx| {
            super::close_modal();
            write(a, ServerCall::EditSafety { guild: sf(guild), edit: SafetyEdit { verification_level: Some(picked as u8), ..Default::default() } }, None, cx);
            cx.notify();
        })));
    crate::ui::modals::card_with(480., crate::t!("gs2.verification"), None, body.into_any_element(), Some(footer.into_any_element()), close)
}

pub fn content_filter(guild: Id, g: &Value, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let level = g.get("explicit_content_filter").and_then(Value::as_u64).unwrap_or(0);
    let community = features(g).iter().any(|x| x == "COMMUNITY");
    let open = super::flag(guild, "ecf-open").unwrap_or(false);
    let opts = [("gs2.ecf_all", "gs2.ecf_all_desc", 2u8), ("gs2.ecf_no_roles", "gs2.ecf_no_roles_desc", 1), ("gs2.ecf_off", "gs2.ecf_off_desc", 0)];
    let cur = opts.iter().find(|o| u64::from(o.2) == level).unwrap_or(&opts[0]);
    let row = |id: (&'static str, usize), h: &'static str, d: SharedString, on: bool, disabled: bool| {
        div()
            .id(id)
            .p(z(12.))
            .pl(z(14.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .gap(z(12.))
            .border_1()
            .border_color(t(if on { idx::BORDER_NORMAL } else { idx::BORDER_SUBTLE }))
            .when(disabled, |d| d.opacity(0.5))
            .child(div().flex_1().flex().flex_col().child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(h))).child(text(
                14.,
                18.,
                FontWeight::NORMAL,
                idx::TEXT_SUBTLE,
                d,
            )))
    };
    let mut d = div()
        .mt(z(40.))
        .flex()
        .flex_col()
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.ecf_title")))
        .child(div().mt(z(4.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.ecf_desc"))))
        .child(link(crate::t!("gs2.learn_more")).mb(z(12.)))
        .child(
            row(("gs2-ecf-head", 0), cur.0, crate::t!(cur.1), false, false)
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    super::set_flag(guild, "ecf-open", !open);
                    cx.notify();
                }))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_BRAND, crate::t!("gs2.edit"))),
        );
    if open {
        let mut group = div().mt(z(12.)).mx(z(12.)).flex().flex_col().gap(z(8.));
        for (i, (h, desc, v)) in opts.into_iter().enumerate() {
            let disabled = community && v != 2;
            let d = if disabled { crate::t!("gs2.ecf_community_only") } else { crate::t!(desc) };
            group = group.child(row(("gs2-ecf", i), h, d, u64::from(v) == level, disabled).when(!disabled, |r| {
                r.cursor_pointer().on_click(cx.listener(move |a, _, _, cx| {
                    write(
                        a,
                        ServerCall::EditSafety { guild: sf(guild), edit: SafetyEdit { explicit_content_filter: Some(v), ..Default::default() } },
                        None,
                        cx,
                    );
                }))
            }));
        }
        d = d.child(group);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_follow_the_client() {
        let g = json!({ "id": "1", "mfa_level": 1, "features": ["COMMUNITY"], "roles": [{ "id": "1", "permissions": "1024" }] });
        let rules = json!([{ "enabled": true }, { "enabled": false }, { "enabled": true }]);
        let n = counts(&g, &rules);
        assert_eq!(n.raid, (3, 3));
        assert_eq!(n.dm, (5, 5));
        assert_eq!(n.automod, (2, 3));
        assert_eq!(n.perms, (2, 3));
        let g = json!({ "id": "1", "mfa_level": 0, "features": ["RAID_ALERTS_DISABLED", "PRUNE_REQUIRES_ADMIN"], "roles": [{ "id": "1", "permissions": ((1u64 << 3) | (1 << 5)).to_string() }] });
        let n = counts(&g, &json!([]));
        assert_eq!(n.raid, (2, 3));
        assert_eq!(n.perms, (1, 3));
        assert_eq!(risky_everyone(&g), 2);
    }
}
