use crate::theme::z;
use gpui::{AnyView, App, AppContext, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, prelude::FluentBuilder, px};

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

use gpui::{Anchor, AnchoredPositionMode, Bounds, HitboxBehavior, MouseMoveEvent, Pixels, anchored, canvas, deferred, point};

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Top,
    Bottom,
}

thread_local! {
    static TIPS: RefCell<HashMap<u64, (Bounds<Pixels>, bool)>> = RefCell::new(HashMap::new());
    static SINCE: RefCell<HashMap<u64, std::time::Instant>> = RefCell::new(HashMap::new());
}

const GAP: f32 = 11.;
const CARET: f32 = 5.;
const SPAN: f32 = 240.;
const CARD_H: f32 = 34.;
const BADGE_H: f32 = 170.;
const EDGE: f32 = 12.;

fn caret(down: bool) -> gpui::Div {
    let mut col = div().flex().flex_col().items_center();
    for i in 0..5 {
        let i = if down { 4 - i } else { i };
        col = col.child(div().w(px(1. + 2. * i as f32)).h(z(1.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))));
    }
    col
}

#[derive(Clone)]
pub enum Body {
    Plain(SharedString),
    Rich(SharedString, SharedString),
    Badge(BadgeTip),
}

#[derive(Clone)]
pub struct BadgeTip {
    pub icon: SharedString,
    pub eyebrow: Option<SharedString>,
    pub title: SharedString,
    pub body: Option<SharedString>,
    pub animated: bool,
    pub nitro: bool,
}

fn badge_card(b: BadgeTip) -> gpui::Div {
    use gpui::StyledImage as _;
    let subtle = |t: SharedString| {
        div()
            .w_full()
            .text_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(tok(idx::TEXT_SUBTLE)))
            .child(t)
    };
    let has_body = b.body.is_some();
    div()
        .w(z(200.))
        .px(z(12.))
        .pb(z(if has_body { 16. } else { 24. }))
        .flex()
        .flex_col()
        .items_center()
        .child(
            div().relative().w_full().h(z(64.)).mt(z(16.)).child(
                gpui::img(b.icon)
                    .absolute()
                    .when(b.animated, |d| d.top(z(-12.8)).left(z(-35.2)).w(z(246.4)).h(z(89.6)))
                    .when(!b.animated, |d| d.top_0().left_0().size_full())
                    .object_fit(gpui::ObjectFit::Contain),
            ),
        )
        .child(
            div()
                .mt(z(if b.eyebrow.is_some() { 12. } else { 16. }))
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(z(4.))
                .children(b.eyebrow.map(subtle))
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .font_family(crate::fonts::display_family(crate::fonts::NITRO))
                        .text_size(z(16.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::BLACK)
                        .when(b.nitro, |d| d.italic())
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(SharedString::from(b.title.to_uppercase())),
                )
                .children(b.body.map(subtle)),
        )
}

fn card(body: Body) -> gpui::Div {
    let rich = matches!(body, Body::Badge(_));
    let (max_w, content) = match body {
        Body::Badge(b) => (200., badge_card(b)),
        Body::Plain(text) => (200., div().child(text)),
        Body::Rich(title, note) => (
            272.,
            div()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .child(title)
                .child(div().font_weight(FontWeight::NORMAL).text_color(c(tok(idx::TEXT_MUTED))).child(note)),
        ),
    };
    div()
        .max_w(z(max_w))
        .when(!rich, |d| d.px(z(12.)).py(z(8.)))
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
        .child(content)
}

fn layer(text: Body, b: Bounds<Pixels>, side: Side, view_h: f32) -> gpui::AnyElement {
    let top = f32::from(b.origin.y);
    let bottom = f32::from(b.origin.y + b.size.height);
    let mid = f32::from(b.origin.x + b.size.width / 2.);
    let zz = |v: f32| f32::from(z(v));
    let (gap, caret_h, span) = (zz(GAP), zz(CARET), zz(SPAN));
    let card_h = if matches!(text, Body::Badge(_)) { BADGE_H } else { CARD_H };
    let need = zz(GAP + card_h + EDGE);
    let below = match side {
        Side::Top => top < need,
        Side::Bottom => bottom + need <= view_h || top < need,
    };
    let frame = div().w(px(span)).flex().flex_col().items_center();
    let (frame, corner, y) = if below {
        (crate::ui::states::tip_zoom("tip-zoom", (0.5, 0.), frame.child(caret(false)).child(card(text))), Anchor::TopLeft, bottom + gap - caret_h)
    } else {
        (crate::ui::states::tip_zoom("tip-zoom", (0.5, 1.), frame.child(card(text)).child(caret(true))), Anchor::BottomLeft, top - gap + caret_h)
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

fn probe(key: u64, delay: u64) -> gpui::AnyElement {
    canvas(
        move |bounds, window, _| (bounds, window.insert_hitbox(bounds, HitboxBehavior::Normal)),
        move |_, (bounds, hitbox), window, _| {
            let hovered = hitbox.is_hovered(window);
            if delay > 0 {
                SINCE.with(|s| {
                    let mut s = s.borrow_mut();
                    if !hovered {
                        s.remove(&key);
                    } else if s.entry(key).or_insert_with(std::time::Instant::now).elapsed().as_millis() < delay as u128 {
                        window.request_animation_frame();
                    }
                });
            }
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
        self.tip_body(Body::Plain(text.into()), side)
    }

    fn tip_rich(self, title: impl Into<SharedString>, note: impl Into<SharedString>, side: Side) -> Self {
        self.tip_body(Body::Rich(title.into(), note.into()), side)
    }

    fn tip_body(self, text: Body, side: Side) -> Self {
        self.tip_delayed(text, side, 0)
    }

    fn tip_badge(self, badge: BadgeTip) -> Self {
        self.tip_delayed(Body::Badge(badge), Side::Top, 300)
    }

    fn tip_delayed(self, text: Body, side: Side, delay: u64) -> Self {
        let id = match &text {
            Body::Plain(t) => t.to_string(),
            Body::Rich(t, n) => format!("{t}\n{n}"),
            Body::Badge(b) => format!("{}\n{}\n{}", b.icon, b.title, b.body.as_deref().unwrap_or_default()),
        };
        let key = crate::ui::states::key(("tip", id, side == Side::Top));
        let ready = delay == 0 || SINCE.with(|s| s.borrow().get(&key).is_some_and(|t| t.elapsed().as_millis() >= delay as u128));
        let shown = TIPS.with(|t| t.borrow().get(&key).copied()).filter(|(_, h)| *h && ready);
        let mut el = self.child(probe(key, delay));
        if let Some((b, _)) = shown {
            let view_h = VIEW_H.with(|v| v.get());
            el = el.child(crate::ui::profile_theme::scoped(None, || layer(text, b, side, view_h)));
        }
        el
    }
}

impl<E: ParentElement + Styled + Sized> Tipped for E {}

thread_local! {
    static VIEW_H: std::cell::Cell<f32> = const { std::cell::Cell::new(10_000.) };
}
