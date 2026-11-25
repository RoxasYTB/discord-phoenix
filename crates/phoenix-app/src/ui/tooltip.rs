use crate::theme::z;
use gpui::{AnyView, App, AppContext, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};

use crate::theme::{c, generated::idx, tok};

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let mut pointer = div().flex().flex_col().items_center();
        for i in 0..5 {
            pointer = pointer.child(div().w(px(1. + 2. * i as f32)).h(z(1.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))));
        }
        crate::fonts::ui_root(div())
            .flex()
            .flex_col()
            .items_center()
            .pt(z(4.))
            .child(pointer)
            .child(
                div()
                    .max_w(z(200.))
                    .px(z(12.))
                    .py(z(8.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                    .border_1()
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::hsla(0., 0., 0., 0.24),
                        offset: gpui::point(z(0.), z(12.)),
                        blur_radius: z(24.),
                        spread_radius: z(0.),
                        inset: false,
                    }])
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(self.0.clone()),
            )
    }
}

pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

use std::{cell::RefCell, collections::HashMap};

use gpui::{Anchor, AnchoredPositionMode, AnimationExt, Bounds, HitboxBehavior, MouseMoveEvent, Pixels, anchored, canvas, deferred, point};

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Top,
    Bottom,
}

thread_local! {
    static TIPS: RefCell<HashMap<u64, (Bounds<Pixels>, bool)>> = RefCell::new(HashMap::new());
}

const GAP: f32 = 11.;
const CARET: f32 = 5.;
const SPAN: f32 = 240.;
const CARD_H: f32 = 34.;
const EDGE: f32 = 12.;

fn caret(down: bool) -> gpui::Div {
    let mut col = div().flex().flex_col().items_center();
    for i in 0..5 {
        let i = if down { 4 - i } else { i };
        col = col.child(div().w(px(1. + 2. * i as f32)).h(z(1.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))));
    }
    col
}

fn card(text: SharedString) -> gpui::Div {
    div()
        .max_w(z(200.))
        .px(z(12.))
        .py(z(8.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(text)
}

fn zoomed(text: SharedString) -> gpui::AnimationElement<gpui::Div> {
    card(text).with_animation("tip-zoom", crate::ui::states::tip_zoom(), crate::ui::states::zoom_card)
}

fn layer(text: SharedString, b: Bounds<Pixels>, side: Side, view_h: f32) -> gpui::AnyElement {
    let top = f32::from(b.origin.y);
    let bottom = f32::from(b.origin.y + b.size.height);
    let mid = f32::from(b.origin.x + b.size.width / 2.);
    let zz = |v: f32| f32::from(z(v));
    let (gap, caret_h, span) = (zz(GAP), zz(CARET), zz(SPAN));
    let need = zz(GAP + CARD_H + EDGE);
    let below = match side {
        Side::Top => top < need,
        Side::Bottom => bottom + need <= view_h || top < need,
    };
    let frame = div().w(px(span)).flex().flex_col().items_center();
    let (frame, corner, y) = if below {
        (frame.child(caret(false)).child(zoomed(text)), Anchor::TopLeft, bottom + gap - caret_h)
    } else {
        (frame.child(zoomed(text)).child(caret(true)), Anchor::BottomLeft, top - gap + caret_h)
    };
    let tip = deferred(
        anchored()
            .position_mode(AnchoredPositionMode::Window)
            .anchor(corner)
            .snap_to_window_with_margin(z(EDGE))
            .position(point(px(mid - span / 2.), px(y)))
            .child(frame),
    )
    .with_priority(3);
    div().absolute().top_0().left_0().child(tip).into_any_element()
}

fn probe(key: u64) -> gpui::AnyElement {
    canvas(
        move |bounds, window, _| (bounds, window.insert_hitbox(bounds, HitboxBehavior::Normal)),
        move |_, (bounds, hitbox), window, _| {
            let hovered = hitbox.is_hovered(window);
            VIEW_H.with(|v| v.set(f32::from(window.viewport_size().height)));
            TIPS.with(|t| {
                let mut t = t.borrow_mut();
                if hovered || t.get(&key).is_none_or(|(b, _)| *b == bounds) {
                    t.insert(key, (bounds, hovered));
                }
            });
            window.on_mouse_event(move |_: &MouseMoveEvent, _, window, _| {
                let now = hitbox.is_hovered(window);
                if now != hovered {
                    window.refresh();
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

pub trait Tipped: ParentElement + Styled + Sized {
    fn tip(self, text: impl Into<SharedString>) -> Self {
        self.tip_side(text, Side::Top)
    }

    fn tip_side(self, text: impl Into<SharedString>, side: Side) -> Self {
        let text = text.into();
        let key = crate::ui::states::key(("tip", text.to_string(), side == Side::Top));
        let shown = TIPS.with(|t| t.borrow().get(&key).copied()).filter(|(_, h)| *h);
        let mut el = self.child(probe(key));
        if let Some((b, _)) = shown {
            let view_h = VIEW_H.with(|v| v.get());
            el = el.child(layer(text, b, side, view_h));
        }
        el
    }
}

impl<E: ParentElement + Styled + Sized> Tipped for E {}

thread_local! {
    static VIEW_H: std::cell::Cell<f32> = const { std::cell::Cell::new(10_000.) };
}
