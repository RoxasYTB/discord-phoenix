//! Full-window media viewer opened by clicking an attachment or embed image.
//! Layout of the 2026 client (`scrim_ lightbox_` → `carouselModal_`: `topBar_` with the
//! author, the `actionButtons_` pill and the close button, then `mediaArea_`), measured on
//! the vanilla-lightbox dump. Click the backdrop (or press Escape via
//! `media::close_viewer`) to close.

use crate::theme::z;
use gpui::{AnyElement, App, FontWeight, ParentElement, Styled, Window, div, img, prelude::*, px};

use crate::{
    media::{self, MediaState},
    theme::generated::idx,
    ui::chat::tk,
};

/// `actionButtons_` button: button_ sm icon-only, 32px, radius 8px, 16px icon in
/// --interactive-text-default; :hover --interactive-background-hover / --interactive-text-hover.
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

/// Overlay element, or `None` when no image is open.
pub fn render(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let v = cx.try_global::<MediaState>()?.viewer.clone()?;
    let vp = window.viewport_size();
    // carouselModal_: 24px from the sides, 36px from top and bottom (layer_ padding 24px 0
    // + 12px); topBar_ 40px + gap 12px above mediaArea_.
    let area_w = f32::from(vp.width) - 2. * f32::from(z(24.));
    let area_h = f32::from(vp.height) - 2. * f32::from(z(36.)) - f32::from(z(52.));
    let (w, h) = media::fit(v.width, v.height, area_w.max(1.), area_h.max(1.));
    let original = v.original.clone();
    let save = v.original.clone();

    let author = v.author.clone().map(|a| {
        div()
            .flex()
            .items_center()
            .gap(z(12.)) // topBar_ gap 12px
            .min_w_0()
            .child(crate::ui::avatar(a.user.as_ref(), 40.)) // authorAvatar_ 40px
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    // username_ 16px / 22px, weight 500, role colour
                    .child(
                        div()
                            .text_size(z(16.))
                            .line_height(z(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(a.color)
                            .whitespace_nowrap()
                            .child(a.name),
                    )
                    // timestamp_QGyCSa 12px / 16px, weight 500, --text-muted
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
        .gap(z(4.)) // actionButtons_ gap 4px
        .p(z(3.)) // padding 3px
        .rounded(z(12.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .on_click(|_, _, cx| cx.stop_propagation())
        // Order of the dump: Zoomer, Transférer, Enregistrer l'image, Ouvrir dans le navigateur, Plus.
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

    // closeButton_QGyCSa: 40px, radius 12px, --background-surface-highest, 1px border,
    // 20px close icon in --interactive-text-default.
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
        .pr(z(52.)) // topBar_ padding-right 52px (close button 40 + gap 12)
        .children(author)
        .child(div().flex_1())
        .child(actions)
        .child(close);

    Some(
        div()
            .id("image-viewer")
            .absolute()
            .inset_0()
            // scrim_ lightbox_: black at 92%
            .bg(gpui::hsla(0., 0., 0., 0.92))
            .px(z(24.))
            .py(z(36.))
            .flex()
            .flex_col()
            .gap(z(12.)) // carouselModal_ gap 12px
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
