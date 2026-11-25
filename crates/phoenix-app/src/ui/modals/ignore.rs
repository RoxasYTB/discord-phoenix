use gpui::{AnyElement, Context, FontWeight, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button},
        modals::close_action,
    },
};

pub const GUIDE_ARTICLE: &str = "28084948873623";

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn text(size: f32, line: f32, weight: FontWeight, color: usize, t: SharedString) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(tk(color)).child(t)
}

fn cell(icon: &str, title: SharedString, description: SharedString, last: bool) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(z(16.))
        .py(z(16.))
        .when(!last, |d| d.border_b_1().border_color(tk(idx::BORDER_MUTED)))
        .child(
            div()
                .size(z(24.))
                .flex_none()
                .flex()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).text_color(tk(idx::ICON_SUBTLE))),
        )
        .child(div().flex().flex_col().gap(z(2.)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, title)).child(text(
            12.,
            16.,
            FontWeight::MEDIUM,
            idx::TEXT_SUBTLE,
            description,
        )))
}

pub fn guide_url() -> String {
    format!("https://support.discord.com/hc/{}/articles/{GUIDE_ARTICLE}", crate::i18n::locale().to_lowercase())
}

pub fn render(app: &PhoenixApp, user: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let u = app.users.get(&user);
    let name = u.map(|u| u.display_name.to_string()).unwrap_or_default();
    let close = close_action(cx);
    let header = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(z(16.))
        .child(
            div().relative().flex().items_center().justify_center().child(crate::ui::avatar(u, 56.)).child(
                div()
                    .absolute()
                    .bottom(z(-8.))
                    .right(z(-8.))
                    .p(z(4.))
                    .rounded_full()
                    .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(gpui::svg().path(crate::icons::path("chat/eye-slash")).size(z(24.)).text_color(tk(idx::ICON_DEFAULT))),
            ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(text(24., 32., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("ignore.title", name = name)))
                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("ignore.subtitle"))),
        );
    let cells = div()
        .flex()
        .flex_col()
        .child(cell("circle-check", crate::t!("ignore.hide_title"), crate::t!("ignore.hide_desc"), false))
        .child(cell("circle-check", crate::t!("ignore.mute_title"), crate::t!("ignore.mute_desc"), false))
        .child(cell("info", crate::t!("ignore.still_title"), crate::t!("ignore.still_desc"), true));
    let block = div().flex().flex_col().gap(z(8.)).child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, crate::t!("ignore.not_enough"))).child(
        div().rounded(z(8.)).overflow_hidden().border_1().border_color(tk(idx::BORDER_MUTED)).bg(tk(idx::BACKGROUND_BASE_LOWER)).child(
            div()
                .mx(z(16.))
                .py(z(16.))
                .flex()
                .items_center()
                .justify_between()
                .gap(z(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(2.))
                        .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("ignore.block_instead")))
                        .child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_SUBTLE, crate::t!("ignore.block_desc"))),
                )
                .child(button("ignore-block", crate::t!("ignore.see_details"), BtnKind::Secondary).h(z(32.)).px(z(12.)).flex_none().on_click(cx.listener(
                    move |a, _, _, cx| {
                        crate::actions::close_modal();
                        a.confirm_block(user, cx);
                    },
                ))),
        ),
    );
    let guide = div()
        .id("ignore-guide")
        .flex()
        .justify_center()
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_LINK))
        .hover(|s| s.underline())
        .on_click(|_, _, cx| cx.open_url(&guide_url()))
        .child(crate::t!("ignore.guide"));
    let body = div().pt(z(20.)).flex().flex_col().gap(z(16.)).child(header).child(cells).child(div().flex().flex_col().gap(z(8.)).child(block).child(guide));
    let cancel = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("ignore-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| cancel(w, cx)))
        .child(button("ignore-confirm", crate::t!("menu.ignore"), BtnKind::Primary).flex_1().on_click(cx.listener(move |a, _, _, cx| {
            crate::actions::close_modal();
            a.toggle_ignore(user, cx);
        })));
    let card = div()
        .relative()
        .w(z(440.))
        .rounded(z(12.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(div().px(z(24.)).pt(z(8.)).child(body))
        .child(div().pt(z(24.)).px(z(24.)).pb(z(8.)).child(footer));
    crate::ui::modal::frame("ignore-backdrop", close, card)
}

impl PhoenixApp {
    pub fn request_ignore(&mut self, user: Id, cx: &mut Context<Self>) {
        if crate::actions::user::is_ignored(user) {
            self.toggle_ignore(user, cx);
            return;
        }
        crate::actions::open_modal(crate::ui::modals::Modal::Ignore(user), None);
        cx.notify();
    }
}
