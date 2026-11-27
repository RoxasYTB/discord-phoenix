use std::{cell::RefCell, time::Duration};

use gpui::{
    AnyElement, Bounds, DispatchPhase, EntityId, HitboxBehavior, InteractiveElement, IntoElement, ListState, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, ScrollHandle, Styled, Window, canvas, div, point, px,
};

use crate::theme::{c, generated::idx, tok, z};

const PAGE_FRACTION: f32 = 0.875;
const REPEAT_DELAY: Duration = Duration::from_millis(250);
const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Thin,
    ThinFade,
    Auto,
    AutoFade,
}

thread_local! {
    static HANDLES: std::cell::RefCell<std::collections::HashMap<(&'static str, u8), ScrollHandle>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    static PRESS: RefCell<Option<Press>> = const { RefCell::new(None) };
}

#[derive(Clone)]
enum Target {
    Handle(ScrollHandle),
    List(ListState),
}

impl Target {
    fn set(&self, off: f32) {
        match self {
            Target::Handle(h) => h.set_offset(point(h.offset().x, px(-off))),
            Target::List(l) => l.set_offset_from_scrollbar(point(px(0.), px(-off))),
        }
    }

    fn page(&self, distance: f32, view: EntityId, window: &mut Window) {
        match self {
            Target::Handle(h) => h.smooth_scroll_by(point(px(0.), px(-distance)), view, window),
            Target::List(l) => l.smooth_scroll_by(px(distance), view, window),
        }
    }

    fn drag_started(&self) {
        if let Target::List(l) = self {
            l.scrollbar_drag_started();
        }
    }

    fn drag_ended(&self) {
        if let Target::List(l) = self {
            l.scrollbar_drag_ended();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Geom {
    vh: f32,
    track: f32,
    thumb: f32,
    top: f32,
    max: f32,
}

impl Geom {
    fn new(vh: f32, track: f32, max: f32, off: f32) -> Self {
        let off = off.clamp(0., max.max(0.));
        let thumb = (track * vh / (vh + max)).max(f32::from(z(40.))).min(track);
        let top = if max > 0. { (track - thumb) * off / max } else { 0. };
        Self { vh, track, thumb, top, max }
    }

    fn offset_for(&self, thumb_top: f32) -> f32 {
        let span = self.track - self.thumb;
        if span <= 0. { 0. } else { (thumb_top / span).clamp(0., 1.) * self.max }
    }

    fn side(&self, y: f32) -> Option<f32> {
        if y < self.top {
            Some(-1.)
        } else if y > self.top + self.thumb {
            Some(1.)
        } else {
            None
        }
    }
}

struct Drag {
    bar: Bounds<Pixels>,
    grab: f32,
    target: Target,
}

struct Press {
    bar: Bounds<Pixels>,
    y: f32,
    dir: f32,
    geom: Geom,
    target: Target,
    view: EntityId,
}

fn interact(target: Target, geom: Geom) -> AnyElement {
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bar, hitbox, window, _| {
            let view = window.current_view();
            PRESS.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut().filter(|p| p.bar == bar) {
                    p.geom = geom;
                }
            });
            let down_target = target.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || e.button != MouseButton::Left || !hitbox.is_hovered(window) {
                    return;
                }
                cx.stop_propagation();
                if geom.max <= 0. {
                    return;
                }
                let y = f32::from(e.position.y - bar.origin.y);
                match geom.side(y) {
                    None => {
                        down_target.drag_started();
                        DRAG.with(|d| *d.borrow_mut() = Some(Drag { bar, grab: y - geom.top, target: down_target.clone() }));
                    }
                    Some(dir) => {
                        down_target.page(dir * PAGE_FRACTION * geom.vh, view, window);
                        PRESS.with(|p| {
                            *p.borrow_mut() = Some(Press { bar, y, dir, geom, target: down_target.clone(), view })
                        });
                        repeat(bar, window, cx);
                    }
                }
                window.refresh();
            });
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let y = f32::from(e.position.y - bar.origin.y);
                PRESS.with(|p| {
                    if let Some(p) = p.borrow_mut().as_mut().filter(|p| p.bar == bar) {
                        p.y = y;
                    }
                });
                let target = DRAG.with(|d| d.borrow().as_ref().filter(|d| d.bar == bar).map(|d| (d.grab, d.target.clone())));
                if let Some((grab, target)) = target {
                    target.set(geom.offset_for(y - grab));
                    window.refresh();
                }
            });
            window.on_mouse_event(move |e: &MouseUpEvent, phase, _, _| {
                if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                    return;
                }
                PRESS.with(|p| p.borrow_mut().take());
                if let Some(d) = DRAG.with(|d| d.borrow_mut().take()) {
                    d.target.drag_ended();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

fn repeat(bar: Bounds<Pixels>, window: &mut Window, cx: &mut gpui::App) {
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(REPEAT_DELAY).await;
            loop {
                let step = PRESS.with(|p| {
                    let p = p.borrow();
                    let p = p.as_ref().filter(|p| p.bar == bar)?;
                    (p.geom.side(p.y) == Some(p.dir)).then(|| (p.target.clone(), p.dir * PAGE_FRACTION * p.geom.vh, p.view))
                });
                let Some((target, distance, view)) = step else { break };
                if cx.update(|window, _| target.page(distance, view, window)).is_err() {
                    break;
                }
                cx.background_executor().timer(REPEAT_INTERVAL).await;
            }
        })
        .detach();
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
    auto(Target::List(state.clone()), vh, max, off, f32::from(z(36.)))
}

pub fn auto_thumb(state: &ListState) -> f32 {
    let vh = f32::from(state.viewport_bounds().size.height);
    let max = f32::from(state.max_offset_for_scrollbar().y);
    if vh <= 0. || max <= 0. {
        return 0.;
    }
    Geom::new(vh, (vh - f32::from(z(36.))).max(0.), max, 0.).thumb
}

pub const AUTO_FADE_GROUP: &str = "auto-fade-scroller";

pub fn auto_fade_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    let bar = auto_scrollbar_div(scroll)?;
    Some(div().absolute().top_0().right_0().bottom_0().w(z(16.)).opacity(0.).group_hover(AUTO_FADE_GROUP, |s| s.opacity(1.)).child(bar).into_any_element())
}

pub fn auto_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    auto(
        Target::Handle(scroll.clone()),
        f32::from(scroll.bounds().size.height),
        f32::from(scroll.max_offset().y),
        -f32::from(scroll.offset().y),
        0.,
    )
}

fn auto(target: Target, vh: f32, max: f32, off: f32, track_margin_bottom: f32) -> Option<AnyElement> {
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
    let geom = Geom::new(vh, track, max, off);
    let thumb_el = div()
        .absolute()
        .top(px(geom.top + border))
        .h(px((geom.thumb - 2. * border).max(0.)))
        .left(z(4.))
        .w(z(8.))
        .rounded(z(4.))
        .bg(c(tok(idx::SCROLLBAR_AUTO_THUMB)));
    Some(bar.child(thumb_el).child(interact(target, geom)).into_any_element())
}

pub fn thin_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    thin(
        Target::Handle(scroll.clone()),
        f32::from(scroll.bounds().size.height),
        f32::from(scroll.max_offset().y),
        -f32::from(scroll.offset().y),
        false,
    )
}

pub const THIN_FADE_GROUP: &str = "thin-fade-scroller";

pub fn thin_fade_scrollbar(state: &ListState) -> Option<AnyElement> {
    let vh = f32::from(state.viewport_bounds().size.height);
    let measured = f32::from(state.max_offset_for_scrollbar().y);
    thin(
        Target::List(state.clone()),
        vh,
        estimated_max_offset(state, vh, measured),
        -f32::from(state.scroll_px_offset_for_scrollbar().y),
        true,
    )
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
    thin(
        Target::List(state.clone()),
        vh,
        estimated_max_offset(state, vh, measured),
        -f32::from(state.scroll_px_offset_for_scrollbar().y),
        false,
    )
}

pub fn thin_fade_scrollbar_div(scroll: &ScrollHandle) -> Option<AnyElement> {
    thin(
        Target::Handle(scroll.clone()),
        f32::from(scroll.bounds().size.height),
        f32::from(scroll.max_offset().y),
        -f32::from(scroll.offset().y),
        true,
    )
}

fn thin(target: Target, vh: f32, max: f32, off: f32, fade: bool) -> Option<AnyElement> {
    if vh <= 0. || max <= 0. {
        return None;
    }
    let geom = Geom::new(vh, vh, max, off);
    let bar = div().absolute().top_0().right_0().bottom_0().w(z(8.)).bg(c(tok(idx::SCROLLBAR_THIN_TRACK)));
    let bar = if fade { bar.opacity(0.).group_hover(THIN_FADE_GROUP, |s| s.opacity(1.)) } else { bar };
    Some(
        bar.child(
            div()
                .absolute()
                .top(px(geom.top) + z(2.))
                .h(px(geom.thumb) - z(4.))
                .left(z(2.))
                .w(z(4.))
                .rounded(z(2.))
                .bg(c(tok(idx::SCROLLBAR_THIN_THUMB))),
        )
        .child(interact(target, geom))
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::Geom;

    #[test]
    fn thumb_follows_the_offset_and_maps_back_linearly() {
        let g = Geom::new(560., 560., 17307., 8000.);
        assert!((g.thumb - 40.).abs() < 1e-3);
        assert!((g.top - 520. * 8000. / 17307.).abs() < 1e-2);
        assert!((g.offset_for(g.top + 66.7) - 8000. - 66.7 * 17307. / 520.).abs() < 1.);
        assert_eq!(g.offset_for(-10.), 0.);
        assert_eq!(g.offset_for(10_000.), 17307.);
    }

    #[test]
    fn a_click_above_or_below_the_thumb_pages_toward_it() {
        let g = Geom::new(400., 400., 400., 200.);
        assert_eq!(g.thumb, 200.);
        assert_eq!(g.top, 100.);
        assert_eq!(g.side(50.), Some(-1.));
        assert_eq!(g.side(150.), None);
        assert_eq!(g.side(350.), Some(1.));
    }
}
