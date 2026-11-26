use gpui::{
    AnyElement, InteractiveElement, IntoElement, ListState, ParentElement, ScrollHandle, Styled, div, px,
};

use crate::theme::{c, generated::idx, tok, z};

pub const AUTO_HOVER_GROUP: &str = "auto-hover-scroller";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Thin,
    ThinFade,
    Auto,
    AutoFade,
}

thread_local! {
    static HANDLES: std::cell::RefCell<std::collections::HashMap<(&'static str, u8), ScrollHandle>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn handle(key: &'static str) -> ScrollHandle {
    handle_n(key, 0)
}

pub fn handle_n(key: &'static str, part: u8) -> ScrollHandle {
    HANDLES.with(|h| h.borrow_mut().entry((key, part)).or_default().clone())
}

pub fn bar_for(key: &'static str, bar: Bar) -> Option<AnyElement> {
    bar_for_n(key, 0, bar)
}

pub fn bar_for_n(key: &'static str, part: u8, bar: Bar) -> Option<AnyElement> {
    let h = handle_n(key, part);
    match bar {
        Bar::Thin => thin_scrollbar_div(&h),
        Bar::ThinFade => thin_fade_scrollbar_div(&h),
        Bar::Auto => auto_scrollbar_div(&h),
        Bar::AutoFade => auto_fade_scrollbar_div(&h),
    }
}

pub fn host(bar: Bar) -> gpui::Div {
    let d = div().relative().flex().flex_col();
    match bar {
        Bar::ThinFade => d.group(THIN_FADE_GROUP),
        Bar::AutoFade => d.group(AUTO_FADE_GROUP),
        _ => d,
    }
}

pub fn auto_scrollbar(state: &ListState) -> Option<AnyElement> {
    let view = state.viewport_bounds();
    let vh = f32::from(view.size.height);
    let max = f32::from(state.max_offset_for_scrollbar().y);
    let off = -f32::from(state.scroll_px_offset_for_scrollbar().y);
    auto(vh, max, off, f32::from(z(36.)), true)
}

pub const AUTO_FADE_GROUP: &str = "auto-fade-scroller";

pub fn auto_fade_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    let bar = auto_scrollbar_div(scroll)?;
    Some(div().absolute().top_0().right_0().bottom_0().w(z(16.)).opacity(0.).group_hover(AUTO_FADE_GROUP, |s| s.opacity(1.)).child(bar).into_any_element())
}

pub fn auto_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    auto(f32::from(scroll.bounds().size.height), f32::from(scroll.max_offset().y), -f32::from(scroll.offset().y), 0., false)
}

fn auto(vh: f32, max: f32, off: f32, track_margin_bottom: f32, thumb_on_hover: bool) -> Option<AnyElement> {
    if vh <= 0. {
        return None;
    }
    let track = (vh - track_margin_bottom).max(0.);
    let border = f32::from(z(4.));
    let bar = div()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(z(16.))
        .child(
            div()
                .absolute()
                .top(px(border))
                .h(px((track - 2. * border).max(0.)))
                .left(z(4.))
                .w(z(8.))
                .rounded(z(4.))
                .bg(c(tok(idx::SCROLLBAR_AUTO_TRACK))),
        );
    if max <= 0. {
        return Some(bar.into_any_element());
    }
    let off = off.clamp(0., max);
    let thumb = (track * vh / (vh + max)).max(f32::from(z(40.))).min(track);
    let top = (track - thumb) * off / max;
    let thumb_el = div()
        .absolute()
        .top(px(top + border))
        .h(px((thumb - 2. * border).max(0.)))
        .left(z(4.))
        .w(z(8.))
        .rounded(z(4.))
        .bg(c(tok(idx::SCROLLBAR_AUTO_THUMB)));
    let thumb_el = if thumb_on_hover {
        thumb_el.opacity(0.).group_hover(AUTO_HOVER_GROUP, |s| s.opacity(1.))
    } else {
        thumb_el
    };
    Some(bar.child(thumb_el).into_any_element())
}

pub fn thin_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    thin(f32::from(scroll.bounds().size.height), f32::from(scroll.max_offset().y), -f32::from(scroll.offset().y), false)
}

pub const THIN_FADE_GROUP: &str = "thin-fade-scroller";

pub fn thin_fade_scrollbar(state: &ListState) -> Option<AnyElement> {
    let vh = f32::from(state.viewport_bounds().size.height);
    let measured = f32::from(state.max_offset_for_scrollbar().y);
    thin(vh, estimated_max_offset(state, vh, measured), -f32::from(state.scroll_px_offset_for_scrollbar().y), true)
}

fn estimated_max_offset(state: &ListState, vh: f32, measured: f32) -> f32 {
    let n = state.item_count();
    let top = state.logical_scroll_top().item_ix;
    let (mut sum, mut k) = (0f32, 0usize);
    for ix in top..n.min(top + 200) {
        let Some(b) = state.bounds_for_item(ix) else { break };
        sum += f32::from(b.size.height);
        k += 1;
    }
    if k == 0 || k + top >= n {
        return measured;
    }
    measured.max(sum / k as f32 * n as f32 - vh)
}

pub fn thin_list_scrollbar(state: &ListState) -> Option<AnyElement> {
    let vh = f32::from(state.viewport_bounds().size.height);
    let measured = f32::from(state.max_offset_for_scrollbar().y);
    thin(vh, estimated_max_offset(state, vh, measured), -f32::from(state.scroll_px_offset_for_scrollbar().y), false)
}

pub fn thin_fade_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    thin(f32::from(scroll.bounds().size.height), f32::from(scroll.max_offset().y), -f32::from(scroll.offset().y), true)
}

fn thin(vh: f32, max: f32, off: f32, fade: bool) -> Option<AnyElement> {
    if vh <= 0. || max <= 0. {
        return None;
    }
    let off = off.clamp(0., max);
    let thumb = (vh * vh / (vh + max)).max(f32::from(z(40.))).min(vh);
    let top = (vh - thumb) * off / max;
    let bar = div().absolute().top_0().right_0().bottom_0().w(z(8.)).bg(c(tok(idx::SCROLLBAR_THIN_TRACK)));
    let bar = if fade { bar.opacity(0.).group_hover(THIN_FADE_GROUP, |s| s.opacity(1.)) } else { bar };
    Some(
        bar
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
