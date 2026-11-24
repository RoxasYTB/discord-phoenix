use gpui::{AnyElement, Context, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, relative};

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

fn icon(name: &str, size: f32, color: usize) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(c(tok(color)))
}

fn text(size: f32, line: f32, weight: FontWeight, color: usize, s: SharedString) -> Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(c(tok(color))).child(s)
}

fn heading_xl(s: SharedString) -> Div {
    text(24., 32., FontWeight::MEDIUM, idx::TEXT_STRONG, s)
}

const PERKS: &[(&str, [Option<&str>; 3])] = &[
    ("sp.bo.p_emoji", [Some("100"), Some("150"), Some("250")]),
    ("sp.bo.p_stickers", [Some("15"), Some("30"), Some("60")]),
    ("sp.bo.p_soundboard", [Some("24"), Some("36"), Some("48")]),
    ("sp.bo.p_stream", [Some("sp.bo.v_720"), Some("sp.bo.v_1440"), Some("sp.bo.v_1440")]),
    ("sp.bo.p_audio", [Some("128 ko/s"), Some("256 ko/s"), Some("384 ko/s")]),
    ("sp.bo.p_upload", [Some("10 Mo"), Some("50 Mo"), Some("100 Mo")]),
    ("sp.bo.p_video", [Some("50"), Some("150"), Some("300")]),
    ("sp.bo.p_anim_icon", [None, None, None]),
    ("sp.bo.p_invite_bg", [None, None, None]),
    ("sp.bo.p_banner", [Some(""), Some("sp.bo.v_static"), Some("sp.bo.v_animated")]),
    ("sp.bo.p_role_icons", [Some(""), Some(""), None]),
    ("sp.bo.p_vanity", [Some(""), Some(""), None]),
];

const FAQ: &[&str] = &[
    "sp.bo.q1", "sp.bo.q2", "sp.bo.q3", "sp.bo.q4", "sp.bo.q5", "sp.bo.q6", "sp.bo.q7", "sp.bo.q8", "sp.bo.q9",
];

fn value(v: &str) -> SharedString {
    if v.starts_with("sp.") { tr(v) } else { SharedString::from(v.to_string()) }
}

pub(super) fn page(app: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(header())
        .child(notice())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(32.))
                .child(guild_cards(app))
                .child(perks_table())
                .child(community())
                .child(faq()),
        )
        .into_any_element()
}

fn header() -> Div {
    div()
        .flex()
        .items_center()
        .gap(z(32.))
        .mb(z(32.))
        .child(
            div()
                .w(z(364.))
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().flex().items_center().gap(z(10.)).child(icon("boost", 20., idx::GUILD_BOOSTING_PINK)).child(heading_xl(tr("sp.bo.title"))))
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(tok(idx::TEXT_SUBTLE)))
                        .child(tr("sp.bo.lead"))
                        .child(div().text_color(c(tok(idx::TEXT_LINK))).child(tr("sp.bo.learn"))),
                ),
        )
        .child(
            div()
                .flex_none()
                .relative()
                .w(z(300.))
                .h(z(169.))
                .rounded(z(10.))
                .border_1()
                .border_color(c(tok(idx::BORDER_NORMAL)))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .overflow_hidden()
                .child(div().absolute().top(z(12.)).left(z(12.)).child(text(14., 18., FontWeight::SEMIBOLD, idx::WHITE, tr("sp.bo.video"))))
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().size(z(48.)).rounded_full().bg(gpui::black().opacity(0.6)).flex().items_center().justify_center().child(icon("play", 24., idx::WHITE))),
                ),
        )
}

fn notice() -> Div {
    div().flex().justify_center().mb(z(32.)).child(
        div()
            .px(z(16.))
            .py(z(8.))
            .rounded_full()
            .border_2()
            .border_color(gpui::white())
            .bg(gpui::white().opacity(0.1))
            .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, tr("sp.bo.unavailable"))),
    )
}

fn guild_cards(app: &PhoenixApp) -> Div {
    let mut cards = div().flex().flex_col().gap(z(8.));
    for g in app.guilds.iter().take(3) {
        cards = cards.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .p(z(12.))
                .rounded(z(16.))
                .border_1()
                .border_color(c(tok(idx::BORDER_MUTED)))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .min_w_0()
                        .child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 40.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(z(2.))
                                .min_w_0()
                                .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, g.name.clone()))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(z(8.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(z(2.))
                                                .child(icon("boost", 14., idx::GUILD_BOOSTING_PINK))
                                                .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("sp.bo.count", n = g.boosts))),
                                        )
                                        .child(div().size(z(4.)).rounded_full().bg(c(tok(idx::TEXT_MUTED))))
                                        .child(text(
                                            12.,
                                            16.,
                                            FontWeight::NORMAL,
                                            idx::TEXT_MUTED,
                                            if g.boost_tier == 0 { tr("sp.bo.no_level") } else { crate::t!("sp.bo.level", n = g.boost_tier) },
                                        )),
                                ),
                        ),
                )
                .child(
                    controls::button_sm(eid(format!("boost-{}", g.id.0)), SharedString::default(), BtnKind::Secondary)
                        .gap(z(4.))
                        .child(icon("boost", 16., idx::CONTROL_SECONDARY_TEXT_DEFAULT))
                        .child(tr("sp.bo.boost_btn")),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, tr("sp.bo.could")))
        .child(cards)
}

fn perks_table() -> Div {
    let cell = || div().flex_1().min_w_0().px(z(16.));
    let tier = || cell().flex().flex_col().items_center().justify_center();
    let row = || div().flex().items_center().py(z(16.)).border_b_1().border_color(c(tok(idx::BORDER_NORMAL)));
    let mut head = row().items_start().child(cell().child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, tr("sp.bo.perks"))));
    for (lvl, n) in [("sp.bo.lvl1", "sp.bo.n2"), ("sp.bo.lvl2", "sp.bo.n7"), ("sp.bo.lvl3", "sp.bo.n14")] {
        head = head.child(
            tier()
                .child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, tr(lvl)))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE, tr(n))),
        );
    }
    let mut t = div().flex().flex_col().child(head);
    for (label, vals) in PERKS {
        let mut r = row().child(cell().child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_SUBTLE, tr(label))));
        for v in vals {
            r = r.child(match v {
                None => tier().child(icon("check", 24., idx::TEXT_STRONG)),
                Some("") => tier(),
                Some(s) => tier().child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, value(s))),
            });
        }
        t = t.child(r);
    }
    t
}

fn community() -> Div {
    let card = |ic: &'static str, k: &'static str| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .gap(z(8.))
            .p(z(12.))
            .rounded(z(16.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
            .child(icon(ic, 42., idx::ICON_MUTED))
            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE, tr(k)).text_center())
    };
    div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(heading_xl(tr("sp.bo.community")))
        .child(div().flex().gap(z(16.)).child(card("members", "sp.bo.c1")).child(card("medal", "sp.bo.c2")))
        .child(div().flex().gap(z(16.)).child(card("crown", "sp.bo.c3")).child(card("boost", "sp.bo.c4")))
}

static FAQ_OPEN: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

fn faq() -> Div {
    use std::sync::atomic::Ordering;
    let open = FAQ_OPEN.load(Ordering::Relaxed);
    let mut list = div().flex().flex_col().gap(z(4.));
    for (i, q) in FAQ.iter().enumerate() {
        let expanded = open & (1 << i) != 0;
        let (bg, border) = if expanded {
            (idx::BACKGROUND_SURFACE_HIGHEST, idx::BORDER_NORMAL)
        } else {
            (idx::BACKGROUND_SURFACE_HIGH, idx::BORDER_SUBTLE)
        };
        let mut row = div()
            .id(eid(format!("boost-faq-{i}")))
            .flex()
            .flex_col()
            .gap(z(4.))
            .p(z(12.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(border)))
            .bg(c(tok(bg)))
            .cursor_pointer()
            .on_click(move |_, window, _| {
                FAQ_OPEN.fetch_xor(1 << i, Ordering::Relaxed);
                window.refresh();
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .child(div().flex_1().min_w_0().child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_SUBTLE, tr(q))))
                    .child(icon(if expanded { "chevron-up" } else { "chevron" }, 18., idx::TEXT_DEFAULT)),
            );
        if expanded {
            row = row.child(div().pt(z(8.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, tr(&format!("{q}a")))));
        }
        list = list.child(row);
    }
    div().flex().flex_col().gap(z(16.)).w(relative(1.)).child(heading_xl(tr("sp.bo.faq"))).child(list)
}
