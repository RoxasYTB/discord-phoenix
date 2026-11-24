use gpui::{
    AnyElement, InteractiveElement, IntoElement, ListState, ParentElement, ScrollHandle, Styled, div, px,
};

use crate::theme::{c, generated::idx, tok, z};

pub fn auto_scrollbar(state: &ListState) -> Option<AnyElement> {
    let view = state.viewport_bounds();
    let vh = f32::from(view.size.height);
    let max = f32::from(state.max_offset_for_scrollbar().y);
    if vh <= 0. || max <= 0. {
        return None;
    }
    let off = (-f32::from(state.scroll_px_offset_for_scrollbar().y)).clamp(0., max);
    let track = vh - 2.;
    let thumb = (track * vh / (vh + max)).max(f32::from(z(40.))).min(track);
    let top = 1. + (track - thumb) * off / max;
    Some(
        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(z(16.))
            .child(
                div()
                    .absolute()
                    .top(px(1.))
                    .bottom(px(1.))
                    .left(px(1.))
                    .right(px(1.))
                    .bg(c(tok(idx::SCROLLBAR_AUTO_TRACK))),
            )
            .child(
                div()
                    .absolute()
                    .top(px(top) + z(4.))
                    .h(px(thumb) - z(8.))
                    .left(z(4.))
                    .w(z(8.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::SCROLLBAR_AUTO_THUMB))),
            )
            .into_any_element(),
    )
}

pub const THIN_FADE_GROUP: &str = "thin-fade-scroller";

pub fn thin_fade_scrollbar(state: &ListState) -> Option<AnyElement> {
    thin_fade(
        f32::from(state.viewport_bounds().size.height),
        f32::from(state.max_offset_for_scrollbar().y),
        -f32::from(state.scroll_px_offset_for_scrollbar().y),
    )
}

pub fn thin_fade_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    thin_fade(
        f32::from(scroll.bounds().size.height),
        f32::from(scroll.max_offset().y),
        -f32::from(scroll.offset().y),
    )
}

fn thin_fade(vh: f32, max: f32, off: f32) -> Option<AnyElement> {
    if vh <= 0. || max <= 0. {
        return None;
    }
    let off = off.clamp(0., max);
    let thumb = (vh * vh / (vh + max)).max(f32::from(z(40.))).min(vh);
    let top = (vh - thumb) * off / max;
    Some(
        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(z(8.))
            .opacity(0.)
            .group_hover(THIN_FADE_GROUP, |s| s.opacity(1.))
            .child(
                div()
                    .absolute()
                    .top(px(top) + z(2.))
                    .h(px(thumb) - z(4.))
                    .left(z(2.))
                    .w(z(4.))
                    .rounded(z(2.))
                    .bg(c(tok(idx::SCROLLBAR_THIN_THUMB))),
            )
            .into_any_element(),
    )
}
