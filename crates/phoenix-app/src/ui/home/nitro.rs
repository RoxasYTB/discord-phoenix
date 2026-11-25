use gpui::{AnyElement, Context, FontWeight, ParentElement, SharedString, Styled, Window, div, prelude::*};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    theme::{c, generated::idx, tok, z},
    ui::{avatar, icon},
};

const PERKS: [(&str, &str); 6] = [
    ("nitro.p_upload", "nitro.p_upload_d"),
    ("nitro.p_name_styles", "nitro.p_name_styles_d"),
    ("nitro.p_themes", "nitro.p_themes_d"),
    ("nitro.p_profile", "nitro.p_profile_d"),
    ("nitro.p_stream", "nitro.p_stream_d"),
    ("nitro.p_emoji", "nitro.p_emoji_d"),
];

fn section_title(t: SharedString) -> gpui::Div {
    crate::fonts::nitro(div(), true)
        .mb(z(24.))
        .text_size(z(24.))
        .line_height(z(26.))
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(SharedString::from(t.to_uppercase()))
}

fn card(ix: usize, pill: Option<SharedString>, title: SharedString, desc: SharedString) -> AnyElement {
    div()
        .id(("nitro-card", ix))
        .w(z(330.))
        .h(z(380.))
        .flex_none()
        .rounded(z(16.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(gpui::transparent_black())
        .hover(|s| s.border_color(c(tok(idx::BORDER_SUBTLE))))
        .overflow_hidden()
        .relative()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(icon(Icon::Nitro, 96., c(tok(idx::TEXT_BRAND)))),
        )
        .children(pill.map(|p| {
            div()
                .absolute()
                .top(z(24.))
                .left(z(24.))
                .px(z(8.))
                .py(z(2.))
                .rounded_full()
                .bg(gpui::white())
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui::black())
                .child(SharedString::from(p.to_uppercase()))
        }))
        .child(
            div()
                .p(z(20.))
                .flex()
                .flex_col()
                .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(title))
                .child(div().mt(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(desc)),
        )
        .into_any_element()
}

pub fn render(app: &mut PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let me = app.me.clone();
    let overlay_btn = |id: &'static str| {
        div()
            .id(id)
            .h(z(32.))
            .px(z(11.))
            .flex()
            .items_center()
            .gap(z(4.))
            .rounded(z(8.))
            .bg(gpui::hsla(0., 0., 0., 0.56))
            .border_1()
            .border_color(gpui::hsla(240. / 360., 0.03, 0.61, 0.16))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(gpui::white())
    };
    let nav = div()
        .h(z(49.))
        .flex_none()
        .px(z(16.))
        .flex()
        .items_center()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(6.))
                .text_color(gpui::white())
                .child(icon(Icon::Nitro, 24., gpui::white()))
                .child(crate::fonts::nitro(div().text_size(z(18.)), true).child("NITRO")),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(overlay_btn("nitro-heart").text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child("\u{2665}"))
                .child(overlay_btn("nitro-gift").child(icon(Icon::Gift, 16., gpui::white())).child(crate::t!("nitro.gift")).on_click(cx.listener(|a, _, _, cx| a.open_gift_nitro(None, cx)))),
        );
    let hello = div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(crate::fonts::nitro(div().text_size(z(32.)).line_height(z(34.)), true).text_color(gpui::white()).child(SharedString::from(crate::t!("nitro.hello").to_uppercase())))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(avatar(Some(&me), 64.))
                .child(div().text_size(z(40.)).line_height(z(58.)).font_weight(FontWeight::BOLD).text_color(gpui::white()).child(me.display_name.clone())),
        );
    let mut cards: Vec<AnyElement> = Vec::new();
    for (i, (t, d)) in PERKS.iter().enumerate() {
        let pill = (i == 0).then(|| crate::t!("nitro.new"));
        cards.push(card(i, pill, crate::t!(t), crate::t!(d)));
    }
    let grid = div().flex().flex_wrap().gap_x(z(24.)).gap_y(z(20.)).children(cards);
    div()
        .id("nitro-home")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .bg(gpui::black())
        .child(
            div()
                .id("nitro-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&crate::ui::chat::scrollbar::handle("nitro-scroll"))
                .flex()
                .flex_col()
                .child(
                    div()
                        .bg(gpui::linear_gradient(
                            160.,
                            gpui::linear_color_stop(c(tok(idx::BACKGROUND_BRAND)).opacity(0.55), 0.),
                            gpui::linear_color_stop(gpui::black(), 1.),
                        ))
                        .flex()
                        .flex_col()
                        .child(nav)
                        .child(div().px(z(32.)).pt(z(64.)).pb(z(48.)).child(hello)),
                )
                .child(
                    div()
                        .px(z(32.))
                        .pb(z(32.))
                        .flex()
                        .flex_col()
                        .child(section_title(crate::t!("nitro.whats_new")))
                        .child(grid),
                ),
        )
        .children(crate::ui::chat::scrollbar::bar_for("nitro-scroll", crate::ui::chat::scrollbar::Bar::Auto))
        .into_any_element()
}
