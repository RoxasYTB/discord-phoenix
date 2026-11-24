use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, deferred, div, prelude::*};

use crate::{
    actions::close_modal,
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub type Pick = Rc<dyn Fn(usize, &mut PhoenixApp, &mut Context<PhoenixApp>)>;

pub enum SideItem {
    Header(SharedString),
    Item { label: SharedString, index: usize, danger: bool },
    Separator,
}

#[allow(clippy::too_many_arguments)]
pub fn overlay(id: &'static str, side_items: Vec<SideItem>, selected: usize, pick: Pick, title: SharedString, content: AnyElement, notice: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut side = div().w(z(218.)).flex().flex_col().gap(z(2.)).pt(z(60.)).pb(z(20.)).px(z(6.));
    for (n, it) in side_items.into_iter().enumerate() {
        side = match it {
            SideItem::Header(h) => side.child(
                div().px(z(10.)).pt(z(6.)).pb(z(6.)).text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_SUBTLE))).truncate().child(h),
            ),
            SideItem::Separator => side.child(div().mx(z(10.)).my(z(8.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))),
            SideItem::Item { label, index, danger } => {
                let on = index == selected;
                let pick = pick.clone();
                side.child(
                    div()
                        .id(("settings-side", n))
                        .px(z(10.))
                        .py(z(6.))
                        .rounded(z(8.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .text_color(c(tok(if danger {
                            idx::TEXT_FEEDBACK_CRITICAL
                        } else if on {
                            idx::TEXT_STRONG
                        } else {
                            idx::TEXT_SUBTLE
                        })))
                        .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                        .on_click(cx.listener(move |a, _, _, cx| {
                            pick(index, a, cx);
                            cx.notify();
                        }))
                        .child(label),
                )
            }
        };
    }
    let body = div()
        .flex_1()
        .min_w_0()
        .pt(z(60.))
        .pb(z(80.))
        .px(z(40.))
        .max_w(z(740.))
        .flex()
        .flex_col()
        .child(div().pb(z(20.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(title))
        .child(content);
    let close = div()
        .flex_none()
        .mt(z(60.))
        .flex()
        .flex_col()
        .items_center()
        .gap(z(8.))
        .child(
            div()
                .id("settings-close")
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
                .child(gpui::svg().path(crate::icons::path("close")).size(z(18.)).text_color(c(tok(idx::ICON_SUBTLE)))),
        )
        .child(div().text_size(z(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_SUBTLE))).child("ESC"));
    let main = div()
        .id(SharedString::from(format!("{id}-main")))
        .relative()
        .flex_1()
        .flex()
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .child(div().id(SharedString::from(format!("{id}-scroll"))).flex_1().flex().overflow_y_scroll().child(body).child(close))
        .children(notice.map(|n| div().absolute().bottom(z(20.)).left(z(40.)).w(z(700.)).child(n)));
    deferred(
        div()
            .id(id)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .child(div().id(SharedString::from(format!("{id}-side"))).flex_1().flex().justify_end().overflow_y_scroll().bg(c(tok(idx::BACKGROUND_BASE_LOWER))).child(side))
            .child(main),
    )
    .with_priority(20)
    .into_any_element()
}

pub type Act = Rc<dyn Fn(&mut PhoenixApp, &mut Window, &mut Context<PhoenixApp>)>;

pub fn unsaved_bar(reset: Act, save: Act, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .p(z(10.))
        .pl(z(16.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(4.)),
            blur_radius: z(12.),
            spread_radius: z(0.),
            inset: false,
        }])
        .flex()
        .items_center()
        .gap(z(10.))
        .child(div().flex_1().text_size(z(16.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("admin.unsaved")))
        .child(
            div()
                .id("unsaved-reset")
                .px(z(16.))
                .text_size(z(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_click(cx.listener(move |a, _, w, cx| {
                    reset(a, w, cx);
                    cx.notify();
                }))
                .child(crate::t!("admin.reset")),
        )
        .child(button("unsaved-save", crate::t!("admin.save_changes"), BtnKind::Primary).on_click(cx.listener(move |a, _, w, cx| {
            save(a, w, cx);
            cx.notify();
        })))
        .into_any_element()
}

pub fn eyebrow(t: SharedString) -> gpui::Div {
    div().pb(z(8.)).text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_SUBTLE))).child(t.to_uppercase())
}

pub fn divider() -> gpui::Div {
    div().my(z(20.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

pub fn link_btn(id: impl Into<gpui::ElementId>, t: SharedString, danger: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .px(z(8.))
        .py(z(4.))
        .rounded(z(4.))
        .text_size(z(14.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .text_color(c(tok(if danger { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_LINK })))
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .child(t)
}
