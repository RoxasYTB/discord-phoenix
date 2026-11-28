use gpui::{AnyElement, Context, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

fn tr(k: &str) -> SharedString {
    crate::t!(k)
}

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

fn text(size: f32, line: f32, weight: FontWeight, color: usize, s: SharedString) -> Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(c(tok(color))).child(s)
}

pub(super) fn promo(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let item = |ic: &'static str, k: &'static str| {
        let link = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
        let (plain, hl) = super::account::bracket_runs(&tr(k), link);
        div()
            .flex()
            .items_center()
            .gap(z(6.))
            .pl(z(2.))
            .child(gpui::svg().path(crate::icons::path(ic)).flex_none().size(z(12.)).text_color(c(tok(idx::TEXT_DEFAULT))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(gpui::StyledText::new(plain).with_highlights(hl)),
            )
    };
    let store = |k: &'static str| {
        controls::button_sm(eid(format!("gift-store-{k}")), tr(k), BtnKind::Secondary)
            .bg(gpui::white())
            .text_color(gpui::black())
    };
    let card = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(z(16.))
        .min_h(z(180.))
        .p(z(24.))
        .rounded(z(16.))
        .bg(gpui::linear_gradient(
            90.,
            gpui::linear_color_stop(c(tok(idx::EXPRESSIVE_GRADIENT_BLUE_END)), 0.),
            gpui::linear_color_stop(c(tok(idx::EXPRESSIVE_GRADIENT_BLUE_START)), 1.),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(24.))
                .flex_1()
                .min_w_0()
                .max_w(z(400.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(z(12.))
                        .child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.gi.buy")))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(z(8.))
                                .child(item("server/server-grid", "sp.gi.b1"))
                                .child(item("settings/cloud", "sp.gi.b2"))
                                .child(item("gift", "sp.gi.b3"))
                                .child(item("golive/screen", "sp.gi.b4")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(z(12.))
                        .child(store("sp.gi.s1"))
                        .child(store("sp.gi.s2"))
                        .child(store("sp.gi.s3"))
                        .child(store("sp.gi.s4")),
                ),
        )
        .child(
            div()
                .w(z(282.))
                .h(z(188.))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("gift")).size(z(96.)).text_color(gpui::white().opacity(0.8))),
        );
    div().mt(z(11.)).rounded(z(16.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).child(card).into_any_element()
}

pub(super) fn redeem(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    div()
        .py(z(10.))
        .flex()
        .items_end()
        .gap(z(8.))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(z(4.))
                        .mb(z(8.))
                        .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, tr("sp.gi.code_label")))
                        .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, tr("sp.gi.code_desc"))),
                )
                .child(
                    div()
                        .h(z(40.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                        .px(z(12.))
                        .flex()
                        .items_center()
                        .child(text(16., 20., FontWeight::NORMAL, idx::INPUT_PLACEHOLDER_TEXT_DEFAULT, "WUMP-AAAAA-BBBBB-CCCCC".into())),
                ),
        )
        .child(controls::button(eid("gift-redeem"), tr("sp.gi.redeem"), BtnKind::Primary).w(z(100.)).opacity(0.5))
        .into_any_element()
}

const BADGES: &[(&str, u32)] = &[
    ("sp.gi.t1", 1),
    ("sp.gi.t2", 2),
    ("sp.gi.t3", 3),
    ("sp.gi.t4", 6),
    ("sp.gi.t5", 10),
    ("sp.gi.t6", 20),
];

pub(super) fn badges(_: &PhoenixApp, cx: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let grid = div()
        .mx_auto()
        .w_full()
        .max_w(z(520.))
        .flex()
        .flex_wrap()
        .justify_center()
        .gap_y(z(28.))
        .children(BADGES.iter().map(|&(key, count)| {
            div()
                .w(z(104.))
                .max_w_full()
                .flex_none()
                .flex()
                .flex_col()
                .items_center()
                .gap(z(12.))
                .child(gpui::svg().path(crate::icons::path("gift")).size(z(36.)).text_color(c(tok(idx::ICON_MUTED))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .text_center()
                        .child(text(14., 18., FontWeight::BOLD, idx::TEXT_MUTED, tr(key)))
                        .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!("sp.gi.gifted", n = count))),
                )
        }));
    div()
        .pt(z(3.))
        .flex()
        .flex_col()
        .child(
            div()
                .px(z(32.))
                .pt(z(44.))
                .pb(z(32.))
                .rounded_t(z(12.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .flex()
                .flex_col()
                .gap(z(24.))
                .child(grid)
                .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, tr("sp.gi.footer")).text_center()),
        )
        .child(
            div()
                .p(z(24.))
                .rounded_b(z(12.))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .flex()
                .justify_end()
                .child(controls::button_icon(eid("gift-send"), "gift", tr("sp.gi.send"), BtnKind::Primary)
                    .on_click(cx.listener(|app, _, _, cx| app.open_gift_nitro(None, cx)))),
        )
        .into_any_element()
}

pub(super) fn purchased(_: &PhoenixApp, cx: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let highlight = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let (description, highlights) = super::account::bracket_runs(&tr("sp.gi.none_desc"), highlight);
    let ranges = highlights.iter().map(|(range, _)| range.clone()).collect();
    let entity = cx.entity();
    let link = gpui::InteractiveText::new("gift-purchased-nitro", gpui::StyledText::new(description).with_highlights(highlights))
        .on_click(ranges, move |_, _, cx| {
            entity.update(cx, |app, cx| {
                app.settings = None;
                app.open_home_view(crate::ui::home::data::HomeView::Nitro, cx);
            });
        });
    div()
        .pt(z(45.))
        .pb(z(32.))
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .gap(z(8.))
        .child(text(24., 28., FontWeight::MEDIUM, idx::TEXT_STRONG, tr("sp.gi.none")))
        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(link))
        .into_any_element()
}

pub(super) fn subscriptions(app: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let loaded = super::subscriptions::get();
    let data = loaded.clone().unwrap_or_default();
    let personal = data.personal.as_deref().unwrap_or_default();
    let mut head = div()
        .flex()
        .flex_col()
        .pt(z(11.))
        .gap(z(4.))
        .child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.su.title")))
        .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr("sp.su.desc")));
    if personal.is_empty() {
        head = head.child(div().mt(z(12.)).child(super::nitro::status_card(app)));
        if loaded.is_some() && data.personal.is_none() {
            head = head.child(text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, tr("sp.su.unavailable")));
        }
    } else {
        head = head.child(div().mt(z(12.)).flex().flex_col().gap(z(12.)).children(personal.iter().enumerate().map(|(i, sub)| subscription_card(i, sub))));
    }
    div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(head)
        .when(!personal.is_empty(), |d| d.child(payment_section(personal)))
        .children(data.squad.as_ref().map(squad_card))
        .child(
            div()
                .flex()
                .flex_col()
                .child(text(20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG, tr("sp.su.credit")).mb(z(12.)))
                .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr("sp.su.credit_desc")).mt(z(-8.)))
                .when(loaded.is_some() && data.credits.is_empty(), |d| {
                    d.child(div().mt(z(24.)).min_h(z(62.)).px(z(20.)).rounded(z(8.))
                        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).flex().items_center().gap(z(10.))
                        .child(gpui::svg().path(crate::icons::path("help")).size(z(24.)).text_color(c(tok(idx::TEXT_STRONG))))
                        .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.su.no_credit"))))
                })
                .children(data.credits.iter().map(|cr| credit_row(cr, data.squad.is_some()))),
        )
        .into_any_element()
}

fn subscription_card(i: usize, sub: &super::subscriptions::Subscription) -> Div {
    let basic = sub.tier == 0;
    let (a, b) = if basic { (0x3559a0, 0x597fa6) } else { (0x8447bd, 0xa75d88) };
    let manage = controls::button_sm(("subscription-manage", i), tr("sp.su.manage"), BtnKind::Secondary)
        .bg(gpui::white()).text_color(gpui::black())
        .on_click(|_, _, cx| cx.open_url(super::subscriptions::DISCORD_SUBSCRIPTIONS));
    let cancel = controls::button_sm(("subscription-cancel", i), tr("common.cancel"), BtnKind::Secondary)
        .bg(gpui::transparent_black()).text_color(gpui::white())
        .on_click(|_, _, cx| cx.open_url(super::subscriptions::DISCORD_SUBSCRIPTIONS));
    div().min_h(z(112.)).rounded(z(4.)).p(z(16.)).flex().items_center().gap(z(8.))
        .bg(gpui::linear_gradient(90., gpui::linear_color_stop(gpui::rgb(a), 0.), gpui::linear_color_stop(gpui::rgb(b), 1.)))
        .child(gpui::img(super::art::nitro_tier()).w(z(96.)).h(z(80.)).object_fit(gpui::ObjectFit::Contain).flex_none())
        .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).text_color(gpui::white())
            .child(div().flex().items_center().justify_between().flex_wrap().gap(z(8.))
                .child(crate::fonts::nitro(div().text_size(z(28.)).line_height(z(32.)), true).child(if basic { "NITRO BASIC" } else { "NITRO" }))
                .child(div().flex().flex_wrap().items_center().gap(z(8.)).child(cancel).child(manage)))
            .child(div().text_size(z(16.)).line_height(z(20.)).child(tr(sub.plan)))
            .when(sub.canceled, |d| d.child(div().text_size(z(14.)).line_height(z(18.)).child(tr("sp.su.pending_cancel")))))
}

fn payment_section(personal: &[super::subscriptions::Subscription]) -> Div {
    let panel = || div().flex_1().min_w_0().p(z(16.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .flex().flex_col().gap(z(8.));
    div().flex().flex_col().gap(z(12.))
        .child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.su.payment")))
        .child(div().flex().gap(z(16.))
            .child(panel()
                .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.su.payment_info")))
                .children(personal.iter().map(|sub| {
                    let value = sub.period_end.map(|end| {
                        let date = crate::i18n::medium_date(end.with_timezone(&chrono::Local).date_naive());
                        crate::t!(if sub.canceled { "sp.su.ends_on" } else { "sp.su.renews_on" }, date = date)
                    }).unwrap_or_else(|| tr("sp.su.no_period"));
                    text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, value)
                })))
            .child(panel()
                .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.su.pay_with")))
                .child(controls::button("subscription-payment", tr("sp.su.open_discord"), BtnKind::Secondary)
                    .on_click(|_, _, cx| cx.open_url(super::subscriptions::DISCORD_SUBSCRIPTIONS)))))
}

fn squad_card(sq: &super::subscriptions::Squad) -> AnyElement {
    use gpui::{hsla, linear_color_stop, linear_gradient};
    let start = hsla(305.455 / 360., 0.761, 0.3118, 0.3);
    let end = hsla(240. / 360., 0.8206, 0.4373, 0.3);
    const STRIPS: usize = 16;
    let wash = div().absolute().inset_0().flex().flex_col().children((0..STRIPS).map(|k| {
        let d = 1. - (k as f32 + 0.5) / STRIPS as f32;
        let a = ((d - 0.6) / 0.4).clamp(0., 1.);
        div().flex_1().w_full().opacity(a).bg(linear_gradient(270., linear_color_stop(end, 0.), linear_color_stop(start, 1.)))
    }));
    let owner = sq.owner().map(|m| m.user.display_name.clone()).unwrap_or_default();
    let product = tr("sp.su.squad");
    let desc_plain = crate::t!("sp.su.member_desc", product = product, owner = owner);
    let learn = tr("sp.su.learn");
    let full = format!("{desc_plain}{learn}.");
    let link = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let hl = vec![(desc_plain.len()..desc_plain.len() + learn.len(), link)];
    let header = div()
        .flex()
        .items_center()
        .gap(z(11.))
        .pt(z(12.))
        .pb(z(12.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .child(crate::fonts::nitro(div().text_size(z(16.)).line_height(z(18.)), true).text_color(c(tok(idx::TEXT_STRONG))).child(tr("sp.su.squad").to_uppercase()))
        .child(
            div()
                .h(z(16.))
                .px(z(8.))
                .rounded_full()
                .bg(gpui::white())
                .flex()
                .items_center()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui::rgb(0x2f3035))
                .child(tr("sp.su.beta").to_uppercase()),
        );
    let left = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .justify_center()
        .items_start()
        .pr(z(32.))
        .py(z(24.))
        .border_r_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .child(text(24., 32., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("sp.su.member_title", product = product)))
        .child(
            div()
                .mt(z(8.))
                .mb(z(10.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(gpui::StyledText::new(full).with_highlights(hl)),
        )
        .child(div().mt(z(12.)).child(controls::button(eid("squad-leave"), tr("sp.su.leave"), BtnKind::Secondary)));
    let rows = sq.members.iter().map(|m| {
        let mut name: String = m.user.display_name.to_string();
        if m.me {
            name.push_str(&tr("sp.su.you"));
        }
        div()
            .h(z(40.))
            .pl(z(4.))
            .pr(z(8.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .child(div().flex_none().mr(z(12.)).child(crate::ui::avatar(Some(&m.user), 24.)))
            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, name.into()).truncate())
            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE, "·".into()).mx(z(4.)))
            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE, tr(if m.owner { "sp.su.owner" } else { "sp.su.member" })).flex_none())
    });
    let right = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .pl(z(32.))
        .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, tr("sp.su.members")).pt(z(12.)).px(z(4.)).mb(z(10.)))
        .children(rows);
    div()
        .relative()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .child(wash)
        .child(
            div()
                .relative()
                .pt(z(4.))
                .px(z(24.))
                .child(header)
                .child(div().flex().min_h(z(320.)).child(left).child(right)),
        )
        .into_any_element()
}

fn credit_row(cr: &super::subscriptions::Credit, paused: bool) -> AnyElement {
    use gpui::{linear_color_stop, linear_gradient};
    let (a, b) = if cr.tier == 0 {
        (idx::PREMIUM_TIER_0_PURPLE_FOR_GRADIENTS, idx::PREMIUM_TIER_0_BLUE_FOR_GRADIENTS)
    } else {
        (idx::PREMIUM_TIER_2_PURPLE_FOR_GRADIENTS, idx::PREMIUM_TIER_2_PINK_FOR_GRADIENTS)
    };
    div()
        .mt(z(16.))
        .rounded(z(5.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .p(z(16.))
        .flex()
        .items_center()
        .child(
            div()
                .flex_none()
                .size(z(32.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .justify_center()
                .bg(linear_gradient(90., linear_color_stop(c(tok(a)), 0.), linear_color_stop(c(tok(b)), 1.)))
                .child(gpui::svg().path(crate::icons::path("nitro")).size(z(24.)).text_color(gpui::white())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .mx(z(8.))
                .flex()
                .flex_col()
                .justify_center()
                .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("sp.su.credit_title", plan = tr(cr.plan))))
                .when(paused, |d| d.child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr("sp.su.paused")))),
        )
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::INTERACTIVE_TEXT_DEFAULT, super::subscriptions::months_label(cr.months)).flex_none())
        .into_any_element()
}
