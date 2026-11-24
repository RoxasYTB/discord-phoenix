use crate::theme::z;
use gpui::{AnyElement, App, FontWeight, ParentElement, Styled, Window, div, img, prelude::*, px};

use crate::{
    media::{self, MediaState},
    theme::generated::idx,
    ui::chat::tk,
};

fn action(id: &'static str, icon: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(8.))
        .cursor_pointer()
        .group(id)
        .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
        .child(
            gpui::svg()
                .path(crate::icons::path(icon))
                .size(z(16.))
                .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                .group_hover(id, |s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER))),
        )
}

pub fn render(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let v = cx.try_global::<MediaState>()?.viewer.clone()?;
    let vp = window.viewport_size();
    let area_w = f32::from(vp.width) - 2. * f32::from(z(24.));
    let area_h = f32::from(vp.height) - 2. * f32::from(z(36.)) - f32::from(z(52.));
    let (w, h) = media::fit(v.width, v.height, area_w.max(1.), area_h.max(1.));
    let original = v.original.clone();
    let save = v.original.clone();

    let author = v.author.clone().map(|a| {
        div()
            .flex()
            .items_center()
            .gap(z(12.))
            .min_w_0()
            .child(crate::ui::avatar(a.user.as_ref(), 40.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(z(16.))
                            .line_height(z(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(a.color)
                            .whitespace_nowrap()
                            .child(a.name),
                    )
                    .child(
                        div()
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tk(idx::TEXT_MUTED))
                            .child(a.time),
                    ),
            )
    });

    let actions = div()
        .id("viewer-actions")
        .flex()
        .items_center()
        .gap(z(4.))
        .p(z(3.))
        .rounded(z(12.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(action("viewer-zoom", "viewer/zoom"))
        .child(action("viewer-forward", "nav/forward"))
        .child(action("viewer-save", "download").on_click(move |_, _, cx| {
            cx.stop_propagation();
            cx.open_url(&save);
        }))
        .child(action("viewer-open", "viewer/open").on_click(move |_, _, cx| {
            cx.stop_propagation();
            cx.open_url(&original);
        }))
        .child(action("viewer-more", "more-horizontal"));

    let close = div()
        .id("viewer-close")
        .absolute()
        .right_0()
        .top_0()
        .size(z(40.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(12.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .cursor_pointer()
        .hover(|s| s.bg(tk(idx::BACKGROUND_SURFACE_HIGHER)))
        .on_click(|_, _, cx| {
            cx.stop_propagation();
            media::close_viewer(cx);
        })
        .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)));

    let top_bar = div()
        .id("viewer-top")
        .relative()
        .h(z(40.))
        .flex_none()
        .flex()
        .items_center()
        .gap(z(12.))
        .pr(z(52.))
        .children(author)
        .child(div().flex_1())
        .child(actions)
        .child(close);

    Some(
        div()
            .id("image-viewer")
            .absolute()
            .inset_0()
            .bg(gpui::hsla(0., 0., 0., 0.92))
            .px(z(24.))
            .py(z(36.))
            .flex()
            .flex_col()
            .gap(z(12.))
            .occlude()
            .on_click(|_, _, cx| {
                media::close_viewer(cx);
            })
            .child(top_bar)
            .child(
                div().flex_1().min_h_0().flex().items_center().justify_center().child(
                    img(v.url.clone())
                        .id("image-viewer-img")
                        .w(px(w))
                        .h(px(h))
                        .on_click(|_, _, cx| cx.stop_propagation()),
                ),
            )
            .into_any_element(),
    )
}
