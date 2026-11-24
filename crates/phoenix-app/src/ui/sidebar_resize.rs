use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{
    Bounds, Context, DispatchPhase, InteractiveElement, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Styled, canvas, div, prelude::*,
};

use crate::app::PhoenixApp;
use crate::theme::{Palette, c, m, z, zoom};

pub const MIN_W: f32 = 184.;
pub const MAX_W: f32 = 352.;

static WIDTH: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static DRAG: Cell<bool> = const { Cell::new(false) };
    static LEFT: Cell<f32> = const { Cell::new(0.) };
}

pub fn clamp(w: f32) -> f32 {
    w.round().clamp(MIN_W, MAX_W)
}

fn path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("discord-phoenix").join("sidebar_width"))
}

fn load() -> f32 {
    std::env::var("PHX_SIDEBAR_W")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .or_else(|| std::fs::read_to_string(path()?).ok()?.trim().parse().ok())
        .map(clamp)
        .unwrap_or(m::SIDEBAR_W)
}

pub fn width() -> f32 {
    let v = WIDTH.load(Ordering::Relaxed);
    if v != 0 {
        return f32::from_bits(v);
    }
    let w = load();
    WIDTH.store(w.to_bits(), Ordering::Relaxed);
    w
}

pub fn set(w: f32, persist: bool) {
    let w = clamp(w);
    WIDTH.store(w.to_bits(), Ordering::Relaxed);
    if persist
        && std::env::var_os("PHX_SIDEBAR_W").is_none()
        && let Some(p) = path()
    {
        let _ = p.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(p, format!("{w}\n"));
    }
}

pub fn handle(cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let weak = cx.entity().downgrade();
    let probe = canvas(
        |b: Bounds<Pixels>, _, _| LEFT.with(|l| l.set(f32::from(b.origin.x))),
        move |_, _, window, _| {
            if !DRAG.with(Cell::get) {
                return;
            }
            let w1 = weak.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !DRAG.with(Cell::get) {
                    return;
                }
                set((f32::from(ev.position.x) - LEFT.with(Cell::get)) / zoom(), false);
                let _ = w1.update(cx, |_, cx| cx.notify());
            });
            let w2 = weak.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !DRAG.with(Cell::get) {
                    return;
                }
                DRAG.with(|d| d.set(false));
                set(width(), true);
                let _ = w2.update(cx, |_, cx| cx.notify());
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let dragging = DRAG.with(Cell::get);
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(probe)
        .child(
            div()
                .id("sidebar-resize")
                .absolute()
                .top(z(12.))
                .bottom_0()
                .right_0()
                .w(z(4.))
                .cursor_ew_resize()
                .when(dragging, |d| d.bg(c(Palette::BG_MOD_SELECTED)))
                .hover(|s| s.bg(c(Palette::BG_MOD_SELECTED)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    DRAG.with(|d| d.set(true));
                    cx.stop_propagation();
                }),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn clamps_to_client_range() {
        assert_eq!(super::clamp(10.), 184.);
        assert_eq!(super::clamp(1000.), 352.);
        assert_eq!(super::clamp(335.6), 336.);
    }
}
