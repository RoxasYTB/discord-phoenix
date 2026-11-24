use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    CursorStyle, Decorations, InteractiveElement, IntoElement, MouseButton, ParentElement, ResizeEdge, Styled, Window, div,
    prelude::*, px,
};

const EDGE: f32 = 5.;
const CORNER: f32 = 16.;
const WIN_BUTTONS_W: f32 = 117.;

static PRESSED: AtomicBool = AtomicBool::new(false);

pub fn drag_region<E: InteractiveElement>(el: E) -> E {
    el.on_mouse_down(MouseButton::Left, |ev, w, _| {
        if ev.click_count >= 2 {
            PRESSED.store(false, Ordering::Relaxed);
            w.zoom_window();
        } else {
            PRESSED.store(true, Ordering::Relaxed);
        }
    })
    .on_mouse_up(MouseButton::Left, |_, _, _| PRESSED.store(false, Ordering::Relaxed))
    .on_mouse_move(|ev, w, _| {
        if ev.pressed_button == Some(MouseButton::Left) && PRESSED.swap(false, Ordering::Relaxed) {
            w.start_window_move();
        }
    })
}

fn strip(id: &'static str, edge: ResizeEdge, cursor: CursorStyle) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .absolute()
        .occlude()
        .cursor(cursor)
        .on_mouse_down(MouseButton::Left, move |_, w, cx| {
            cx.stop_propagation();
            w.start_window_resize(edge);
        })
}

pub fn resize_handles(window: &Window) -> Option<impl IntoElement> {
    let tiling = match window.window_decorations() {
        Decorations::Client { tiling } => tiling,
        Decorations::Server => return None,
    };
    if window.is_maximized() || window.is_fullscreen() {
        return None;
    }
    let (t, b, l, r) = (!tiling.top, !tiling.bottom, !tiling.left, !tiling.right);
    let e = px(EDGE);
    let k = px(CORNER);
    let z = crate::theme::zoom();
    let buttons_w = px(WIN_BUTTONS_W * z);
    let bar_h = px(crate::theme::m::TITLE_BAR_H * z);
    Some(
        div()
            .absolute()
            .inset_0()
            .when(t, |d| d.child(strip("rs-top", ResizeEdge::Top, CursorStyle::ResizeUpDown).top_0().left(k).right(buttons_w).h(e)))
            .when(b, |d| d.child(strip("rs-bottom", ResizeEdge::Bottom, CursorStyle::ResizeUpDown).bottom_0().left(k).right(k).h(e)))
            .when(l, |d| d.child(strip("rs-left", ResizeEdge::Left, CursorStyle::ResizeLeftRight).left_0().top(k).bottom(k).w(e)))
            .when(r, |d| d.child(strip("rs-right", ResizeEdge::Right, CursorStyle::ResizeLeftRight).right_0().top(bar_h).bottom(k).w(e)))
            .when(t && l, |d| {
                d.child(strip("rs-tl-h", ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight).top_0().left_0().w(k).h(e))
                    .child(strip("rs-tl-v", ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight).top_0().left_0().w(e).h(k))
            })
            .when(b && l, |d| {
                d.child(strip("rs-bl-h", ResizeEdge::BottomLeft, CursorStyle::ResizeUpRightDownLeft).bottom_0().left_0().w(k).h(e))
                    .child(strip("rs-bl-v", ResizeEdge::BottomLeft, CursorStyle::ResizeUpRightDownLeft).bottom_0().left_0().w(e).h(k))
            })
            .when(b && r, |d| {
                d.child(strip("rs-br-h", ResizeEdge::BottomRight, CursorStyle::ResizeUpLeftDownRight).bottom_0().right_0().w(k).h(e))
                    .child(strip("rs-br-v", ResizeEdge::BottomRight, CursorStyle::ResizeUpLeftDownRight).bottom_0().right_0().w(e).h(k))
            }),
    )
}
