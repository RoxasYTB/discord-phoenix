use crate::theme::z;
use gpui::prelude::FluentBuilder as _;
use gpui::{App, Div, FocusHandle, InteractiveElement, KeyDownEvent, ParentElement, Stateful, Styled, Window, div, rgb};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

pub const RING: u32 = 0x00b0f4;
const WIDTH: f32 = 4.;

const MODAL_KEY: u64 = 0x6d6f64616c5f7472;

thread_local! {
    static HANDLES: RefCell<HashMap<u64, FocusHandle>> = Default::default();
    static WANTED: RefCell<Vec<u64>> = Default::default();
    static VISIBLE: Cell<Option<u64>> = const { Cell::new(None) };
    static TABBED: RefCell<Option<FocusHandle>> = const { RefCell::new(None) };
    static MODAL_OPEN: Cell<bool> = const { Cell::new(false) };
}

pub fn sync(window: &Window, cx: &mut App) -> bool {
    MODAL_OPEN.with(|m| m.set(false));
    let wanted: Vec<u64> = WANTED.with(|w| std::mem::take(&mut *w.borrow_mut()));
    let created = !wanted.is_empty();
    HANDLES.with(|h| {
        let mut h = h.borrow_mut();
        for k in wanted {
            h.entry(k).or_insert_with(|| cx.focus_handle().tab_stop(k != MODAL_KEY));
        }
        let tabbed = TABBED.with(|t| t.borrow().clone()).filter(|t| t.is_focused(window));
        let focused = tabbed.and_then(|t| h.iter().find(|(_, f)| *f == &t).map(|(k, _)| *k));
        VISIBLE.with(|v| v.set(focused));
    });
    created
}

pub fn trap_modal(el: Stateful<Div>) -> Stateful<Div> {
    MODAL_OPEN.with(|m| m.set(true));
    let Some(h) = handle(MODAL_KEY) else { return el };
    el.track_focus(&h)
}

pub fn tab_focused(handle: &FocusHandle, window: &Window) -> bool {
    handle.is_focused(window) && TABBED.with(|t| t.borrow().as_ref() == Some(handle))
}

pub fn owns_focus(window: &Window) -> bool {
    HANDLES.with(|h| h.borrow().values().any(|f| f.is_focused(window)))
}

fn handle(key: u64) -> Option<FocusHandle> {
    let h = HANDLES.with(|h| h.borrow().get(&key).cloned());
    if h.is_none() {
        WANTED.with(|w| w.borrow_mut().push(key));
    }
    h
}

pub fn ring(el: Stateful<Div>, key: u64, radius: f32) -> Stateful<Div> {
    ring_inset(el, key, radius, 0., 0.)
}

pub fn ring_inset(el: Stateful<Div>, key: u64, radius: f32, dx: f32, dy: f32) -> Stateful<Div> {
    ring_inset_edges(el, key, radius, dx, dy, dx, dy)
}

pub fn ring_inset_edges(
    el: Stateful<Div>,
    key: u64,
    radius: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Stateful<Div> {
    let Some(h) = handle(key) else { return el };
    let shown = VISIBLE.with(|v| v.get()) == Some(key);
    el.track_focus(&h).when(shown, |el| el.child(ring_box_edges(radius, left, top, right, bottom)))
}

pub fn ring_box(radius: f32, dx: f32, dy: f32) -> Div {
    ring_box_edges(radius, dx, dy, dx, dy)
}

pub fn ring_box_edges(radius: f32, left: f32, top: f32, right: f32, bottom: f32) -> Div {
    div()
        .absolute()
        .left(z(left - WIDTH))
        .right(z(right - WIDTH))
        .top(z(top - WIDTH))
        .bottom(z(bottom - WIDTH))
        .border(z(WIDTH))
        .border_color(rgb(RING))
        .rounded(z(radius + WIDTH))
}

pub fn on_key_down(ev: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
    let k = &ev.keystroke;
    if k.key != "tab" || k.modifiers.control || k.modifiers.alt || k.modifiers.platform {
        return false;
    }
    tab(k.modifiers.shift, window, cx);
    true
}

pub fn tab(back: bool, window: &mut Window, cx: &mut App) {
    let step = |w: &mut Window, cx: &mut App| {
        if back {
            w.focus_prev(cx);
        } else {
            w.focus_next(cx);
        }
    };
    step(window, cx);
    if MODAL_OPEN.with(|m| m.get())
        && let Some(scope) = HANDLES.with(|h| h.borrow().get(&MODAL_KEY).cloned())
        && !scope.contains_focused(window, cx)
    {
        for _ in 0..64 {
            step(window, cx);
            if scope.contains_focused(window, cx) {
                break;
            }
        }
        if !scope.contains_focused(window, cx) {
            window.focus(&scope, cx);
        }
    }
    let now = window.focused(cx);
    TABBED.with(|t| *t.borrow_mut() = now);
    window.refresh();
}
