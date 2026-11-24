//! "Inventaire des cadeaux" settings page (dump vanilla-settings-16-3):
//! gift card promo, Redeem codes, the gifting badge ladder and "Gifts I
//! purchased". Phoenix handles no payments: buttons are inert.

use gpui::{AnyElement, Context, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder as _};

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

/// `container_J9WkLo`: 90deg --expressive-gradient-blue-end → -start over
/// --background-surface-high, --radius-lg, min-height 180, padding 16 16 16 28;
/// stack gap 24: heading-lg/semibold 20/24, then body-xs bullet items
/// (12px glyph, gap 6, 8 apart) and `overlay-primary` sm buttons 12 apart.
pub(super) fn promo(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let item = |ic: &'static str, k: &'static str| {
        div()
            .flex()
            .items_start()
            .gap(z(6.))
            .pl(z(2.))
            .child(gpui::svg().path(crate::icons::path(ic)).flex_none().size(z(12.)).mt(z(2.)).text_color(c(tok(idx::TEXT_DEFAULT))))
            .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr(k)))
    };
    let store = |k: &'static str| {
        // overlay-primary_F7qvhY: white fill, black text, 1px --border-normal
        controls::button_sm(eid(format!("gift-store-{k}")), tr(k), BtnKind::Secondary)
            .bg(gpui::white())
            .text_color(gpui::black())
    };
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(z(16.))
        .min_h(z(180.))
        .pt(z(16.))
        .pb(z(16.))
        .pr(z(16.))
        .pl(z(28.))
        .rounded(z(16.)) // --radius-lg
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
                .w(z(350.))
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
                                .child(item("shop", "sp.gi.b1"))
                                .child(item("nitro", "sp.gi.b2"))
                                .child(item("file", "sp.gi.b3"))
                                .child(item("info", "sp.gi.b4")),
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
            // animatedGraphic_J9WkLo (282x188 lottie): our own glyph stands in.
            div()
                .w(z(282.))
                .h(z(188.))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("gift")).size(z(96.)).text_color(gpui::white().opacity(0.8))),
        )
        .into_any_element()
}

/// Redeem field (#container_VhFsFg): label body-md/medium --text-strong,
/// description body-sm --text-muted, 8 below; 40px input (--input-background
/// -default, 1px --input-border-default, r8) + md primary "Redeem", gap 8.
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
                        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT))),
                ),
        )
        .child(controls::button(eid("gift-redeem"), tr("sp.gi.redeem"), BtnKind::Primary))
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

/// `container_qZFuGA`: content section (--background-mod-subtle, r 12 12 0 0,
/// p32) with a row of 96px `badgeItem_` (p8 gap8, 44px icon, name body-sm
/// /semibold --text-muted, "Gifted: Nx" body-xs --text-subtle), footer
/// text body-xs; footer section (--background-surface-high, r 0 0 12 12,
/// p24) with the md primary "Send a gift" button on the right.
pub(super) fn badges(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let mut grid = div().flex().justify_center().gap(z(8.));
    for (k, n) in BADGES {
        grid = grid.child(
            div()
                .w(z(96.))
                .p(z(8.))
                .flex()
                .flex_col()
                .items_center()
                .gap(z(8.))
                .child(gpui::svg().path(crate::icons::path("gift")).size(z(44.)).text_color(c(tok(idx::ICON_MUTED))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(z(2.))
                        .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_MUTED, tr(k)))
                        .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!("sp.gi.gifted", n = n))),
                ),
        );
    }
    div()
        .py(z(10.))
        .flex()
        .flex_col()
        .child(
            div()
                .p(z(32.))
                .rounded_t(z(12.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .flex()
                .flex_col()
                .gap(z(16.))
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
                .child(controls::button_icon(eid("gift-send"), "gift", tr("sp.gi.send"), BtnKind::Primary)),
        )
        .into_any_element()
}

/// Empty "Gifts I purchased": "No gifts." + the generous-soul line.
pub(super) fn purchased(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, tr("sp.gi.none")))
        .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, tr("sp.gi.none_desc")))
        .into_any_element()
}

/// "Abonnements" (dump vanilla-settings-16-2, `content_NedsCW` gap 32):
/// heading-lg/semibold 20/24 + 16px margin, `sectionDescription_4VCFtm`
/// 16/20 --text-default; then the "Subscription credit" fieldset: legend
/// body-lg/medium 20/24 (mb 12), body-sm description (mt -8). Subscription
/// cards and credit rows need billing data Phoenix doesn't fetch.
pub(super) fn subscriptions(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let data = super::subscriptions::get().unwrap_or_default();
    // content_NedsCW: column, gap 32. Section: title 20/24 600, 16, lead 16/20, 16, card.
    let mut head = div()
        .flex()
        .flex_col()
        .child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.su.title")).mb(z(16.)))
        .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr("sp.su.desc")));
    if let Some(sq) = &data.squad {
        head = head.child(div().mt(z(16.)).child(squad_card(sq)));
    }
    div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(head)
        .child(
            div()
                .flex()
                .flex_col()
                .child(text(20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG, tr("sp.su.credit")).mb(z(12.)))
                .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr("sp.su.credit_desc")).mt(z(-8.)))
                .children(data.credits.iter().map(|cr| credit_row(cr, data.squad.is_some()))),
        )
        .into_any_element()
}

/// Nitro Squad card (`container_pmhtZL nitro-pink_pmhtZL container_xFL7Bm`, live client
/// 2026-11-24): --background-surface-high, r8, padding 4 24 0, a 270deg pink→blue wash
/// (both stops at alpha .3) masked by `radial-gradient(100% 100% at 50% 100%, transparent 60%, #000)`.
fn squad_card(sq: &super::subscriptions::Squad) -> AnyElement {
    use gpui::{hsla, linear_color_stop, linear_gradient};
    let start = hsla(305.455 / 360., 0.761, 0.3118, 0.3);
    let end = hsla(240. / 360., 0.8206, 0.4373, 0.3);
    // GPUI has no masks: the radial mask is sampled on its vertical axis (x = 50%),
    // alpha = clamp((d - .6) / .4) with d = distance to the bottom edge / height.
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
        // The client draws its NITRO SQUAD wordmark (16px tall); ours is set in type.
        .child(div().text_size(z(16.)).line_height(z(18.)).font_weight(FontWeight::EXTRA_BOLD).italic().text_color(c(tok(idx::TEXT_STRONG))).child(tr("sp.su.squad").to_uppercase()))
        .child(
            // Beta pill: white, rgb(47,48,53) 12/16 600 uppercase, padding 0 8, pill radius.
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
        // "Leave Plan" is inert: it would cancel the user's membership.
        .child(div().mt(z(12.)).child(controls::button(eid("squad-leave"), tr("sp.su.leave"), BtnKind::Secondary)));
    let rows = sq.members.iter().map(|m| {
        let mut name: String = m.user.display_name.to_string();
        if m.me {
            name.push_str(&tr("sp.su.you"));
        }
        // container_-PdA9i: padding 8 8 8 4, r8; 24px avatar, 12 gap; "·" 4px each side.
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
        // usersListHeader_xFL7Bm: padding 12 4 0, margin-bottom 10.
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

/// `accountCreditRow_ysRrD_` in `premiumSubscriptionAccountCredit_ysRrD_`: --background-base-lower,
/// r5, padding 16; 32px tier gradient tile (r4, p4, 24px white glyph), details margin 0 8.
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
