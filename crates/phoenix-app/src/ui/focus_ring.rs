//! Keyboard focus: Tab / Shift+Tab move between controls and the focused one
//! gets the client's focus ring.
//!
//! Measured on the live client (2026-11-24, Tab from the chat, see
//! docs/parity/states.md "Focus rings"): `.focus-rings-ring` is an absolutely
//! placed box over the focused element, `box-shadow: 0 0 0 4px` in
//! `--focus-primary` (unset in the 2026 themes, so the `#00b0f4` fallback,
//! rgb(0,176,244)), with the element's own border radius. Most controls put the
//! ring on their own bounds; list rows (member rows) inset it by 4px so it
//! stays inside the scroller. It only shows when Tab moved the focus there
//! (focus-visible), never after a click. Enter / Space on a focused control clicks it.
//!
//! The helpers here are called from render code that has no `Window` / `App`,
//! so focus handles live in a thread-local registry: a control asks for its
//! key, the root render creates the missing handles (see [`sync`]) and the
//! next frame tracks them. [`sync`] also records which key holds keyboard focus
//! so the helpers can draw its ring.

use crate::theme::z;
use gpui::prelude::FluentBuilder as _;
use gpui::{App, Div, FocusHandle, InteractiveElement, KeyDownEvent, ParentElement, Stateful, Styled, Window, div, rgb};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// `--focus-primary` fallback of `.focus-rings-ring`.
pub const RING: u32 = 0x00b0f4;
/// Ring thickness (`0 0 0 4px` spread).
const WIDTH: f32 = 4.;

thread_local! {
    static HANDLES: RefCell<HashMap<u64, FocusHandle>> = Default::default();
    static WANTED: RefCell<Vec<u64>> = Default::default();
    static VISIBLE: Cell<Option<u64>> = const { Cell::new(None) };
    /// Where the last Tab / Shift+Tab put the focus (focus-visible is decided when
    /// the focus moves, so typing into a field that was clicked never shows a ring).
    static TABBED: RefCell<Option<FocusHandle>> = const { RefCell::new(None) };
}

/// Creates the handles asked for during the previous frame and records the
/// keyboard-focused key. Call at the start of the root render; returns true
/// when new handles were created (the caller re-renders once to track them).
pub fn sync(window: &Window, cx: &mut App) -> bool {
    let wanted: Vec<u64> = WANTED.with(|w| std::mem::take(&mut *w.borrow_mut()));
    let created = !wanted.is_empty();
    HANDLES.with(|h| {
        let mut h = h.borrow_mut();
        for k in wanted {
            h.entry(k).or_insert_with(|| cx.focus_handle().tab_stop(true));
        }
        let tabbed = TABBED.with(|t| t.borrow().clone()).filter(|t| t.is_focused(window));
        let focused = tabbed.and_then(|t| h.iter().find(|(_, f)| *f == &t).map(|(k, _)| *k));
        VISIBLE.with(|v| v.set(focused));
    });
    created
}

/// True when `handle` holds the focus and got it from Tab (for fields that
/// draw their own ring, like the composer).
pub fn tab_focused(handle: &FocusHandle, window: &Window) -> bool {
    handle.is_focused(window) && TABBED.with(|t| t.borrow().as_ref() == Some(handle))
}

/// True when the window focus is on one of the ring controls (the app treats
/// that like focus at the root for type-to-focus and message shortcuts).
pub fn owns_focus(window: &Window) -> bool {
    HANDLES.with(|h| h.borrow().values().any(|f| f.is_focused(window)))
}

/// The handle for `key`, if it already exists (asks for it otherwise).
fn handle(key: u64) -> Option<FocusHandle> {
    let h = HANDLES.with(|h| h.borrow().get(&key).cloned());
    if h.is_none() {
        WANTED.with(|w| w.borrow_mut().push(key));
    }
    h
}

/// Makes `el` a Tab stop and draws the ring on its bounds when it has keyboard
/// focus. `radius` is the element's border radius. `el` must be `relative()`.
pub fn ring(el: Stateful<Div>, key: u64, radius: f32) -> Stateful<Div> {
    ring_inset(el, key, radius, 0., 0.)
}

/// Like [`ring`], with the ring box inset by `dx` / `dy` from the element
/// (negative grows it), as the client does on list rows.
pub fn ring_inset(el: Stateful<Div>, key: u64, radius: f32, dx: f32, dy: f32) -> Stateful<Div> {
    let Some(h) = handle(key) else { return el };
    let shown = VISIBLE.with(|v| v.get()) == Some(key);
    el.track_focus(&h).when(shown, |el| el.child(ring_box(radius, dx, dy)))
}

/// The ring itself, for a `relative()` parent: 4px around the parent's box
/// inset by `dx` / `dy`, with `radius` the radius of that box.
pub fn ring_box(radius: f32, dx: f32, dy: f32) -> Div {
    div()
        .absolute()
        .left(z(dx - WIDTH))
        .right(z(dx - WIDTH))
        .top(z(dy - WIDTH))
        .bottom(z(dy - WIDTH))
        .border(z(WIDTH))
        .border_color(rgb(RING))
        .rounded(z(radius + WIDTH))
}

/// Root key handler: plain Tab / Shift+Tab move focus when nothing below
/// (composer autocomplete, pickers, menus) consumed the key.
pub fn on_key_down(ev: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
    let k = &ev.keystroke;
    if k.key != "tab" || k.modifiers.control || k.modifiers.alt || k.modifiers.platform {
        return false;
    }
    tab(k.modifiers.shift, window, cx);
    true
}

/// One Tab (`back` = Shift+Tab): moves the focus and marks it focus-visible.
pub fn tab(back: bool, window: &mut Window, cx: &mut App) {
    if back {
        window.focus_prev(cx);
    } else {
        window.focus_next(cx);
    }
    let now = window.focused(cx);
    TABBED.with(|t| *t.borrow_mut() = now);
    window.refresh();
}
