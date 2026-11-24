use crate::theme::{c, generated::idx, tok, z};
use crate::ui::Icon;
use gpui::{AnimationExt, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, svg};
use gpui::prelude::FluentBuilder as _;

pub fn composer_button(id: &'static str, i: Icon) -> gpui::Stateful<Div> {
    composer_button_ext(id, i, 20., true)
}

pub fn composer_button_ext(id: &'static str, i: Icon, glyph: f32, hover_bg: bool) -> gpui::Stateful<Div> {
    let k = key(("composer-btn", id));
    let el = track(div().id(id), k)
        .relative()
        .when(hover_bg, |d| d.child(fade_layer(k, gpui::transparent_black(), c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED)), 200, 8.)))
        .group(id)
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(8.))
        .cursor_pointer()
        .child(
            svg()
                .path(i.path())
                .size(z(glyph))
                .flex_none()
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                .id("glyph")
                .group_active(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
        );
    let dx = if id == "emoji" { -4. } else { 0. };
    crate::ui::focus_ring::ring_inset(el, k, if hover_bg { 8. } else { 4. }, dx, 4.)
}

thread_local! {
    static HOVER: std::cell::RefCell<std::collections::HashMap<u64, (bool, std::time::Instant)>> = Default::default();
}

pub fn key(id: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    h.finish()
}

pub fn state(key: u64) -> Option<(bool, std::time::Instant)> {
    HOVER.with(|h| h.borrow().get(&key).copied())
}

pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 + Copy + 'static {
    move |t: f32| {
        let t = t.clamp(0., 1.);
        let bez = |p1: f32, p2: f32, s: f32| 3. * (1. - s) * (1. - s) * s * p1 + 3. * (1. - s) * s * s * p2 + s * s * s;
        let dx = |s: f32| 3. * (1. - s) * (1. - s) * x1 + 6. * (1. - s) * s * (x2 - x1) + 3. * s * s * (1. - x2);
        let mut s = t;
        for _ in 0..8 {
            let d = dx(s);
            if d.abs() < 1e-6 {
                break;
            }
            s = (s - (bez(x1, x2, s) - t) / d).clamp(0., 1.);
        }
        let (mut lo, mut hi) = (0f32, 1f32);
        if (bez(x1, x2, s) - t).abs() > 1e-4 {
            for _ in 0..30 {
                s = (lo + hi) / 2.;
                if bez(x1, x2, s) < t { lo = s } else { hi = s }
            }
        }
        bez(y1, y2, s)
    }
}

pub fn hovered(key: u64) -> bool {
    HOVER.with(|h| h.borrow().get(&key).is_some_and(|s| s.0))
}

pub fn track(el: gpui::Stateful<Div>, key: u64) -> gpui::Stateful<Div> {
    el.on_hover(move |over, window, _| {
        let changed = HOVER.with(|h| {
            let mut h = h.borrow_mut();
            let prev = h.get(&key).is_some_and(|s| s.0);
            h.insert(key, (*over, std::time::Instant::now()));
            prev != *over
        });
        if changed {
            window.refresh();
        }
    })
}

pub fn mix(a: gpui::Hsla, b: gpui::Hsla, t: f32) -> gpui::Hsla {
    let (a, b): (gpui::Rgba, gpui::Rgba) = (a.into(), b.into());
    let l = |x: f32, y: f32| x + (y - x) * t;
    gpui::Rgba { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: l(a.a, b.a) }.into()
}

pub fn fade_layer(key: u64, rest: gpui::Hsla, hover: gpui::Hsla, ms: u64, radius: f32) -> gpui::AnyElement {
    fade_layer_pressed(key, rest, hover, None, ms, radius)
}

pub fn fade_layer_pressed(
    key: u64,
    rest: gpui::Hsla,
    hover: gpui::Hsla,
    pressed: Option<(&'static str, gpui::Hsla)>,
    ms: u64,
    radius: f32,
) -> gpui::AnyElement {
    use gpui::{AnimationExt, IntoElement};
    let state = HOVER.with(|h| h.borrow().get(&key).copied());
    let on = state.is_some_and(|s| s.0);
    let (from, to) = if on { (rest, hover) } else { (hover, rest) };
    let mut layer = div().absolute().top_0().left_0().size_full().rounded(z(radius)).id("fade-layer");
    if let Some((group, col)) = pressed {
        layer = layer.group_active(group, move |s| s.bg(col));
    }
    match state {
        Some((_, at)) if at.elapsed().as_millis() < ms as u128 => layer
            .with_animation(
                gpui::ElementId::Name(format!("fade-{key}-{on}-{at:?}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(ms)).with_easing(ease_out),
                move |d, t| d.bg(mix(from, to, t)),
            )
            .into_any_element(),
        _ => layer.bg(to).into_any_element(),
    }
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t) * (1. - t)
}

pub fn below_tip(text: gpui::SharedString, button_w: f32, button_h: f32) -> gpui::AnyElement {
    use gpui::{AnchoredPositionMode, FontWeight, IntoElement, anchored, deferred, point};
    let bg = c(tok(idx::BACKGROUND_SURFACE_HIGH));
    let mut caret = div().flex().flex_col().items_center();
    for i in 0..5 {
        caret = caret.child(div().w(z(1. + 2. * i as f32)).h(z(1.)).bg(bg));
    }
    const SPAN: f32 = 240.;
    let card = div()
        .max_w(z(200.))
        .px(z(12.))
        .py(z(8.))
        .rounded(z(8.))
        .bg(bg)
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
        .whitespace_nowrap()
        .child(text)
        .with_animation("tip-zoom", tip_zoom(), zoom_card);
    let tip = deferred(
        anchored().position_mode(AnchoredPositionMode::Local).position(point(z(button_w / 2. - SPAN / 2.), z(button_h + 6.))).child(
            div().w(z(SPAN)).flex().flex_col().items_center().child(caret).child(card),
        ),
    )
    .with_priority(2);
    div().absolute().top_0().left_0().child(tip).into_any_element()
}

pub fn header_button(id: &'static str, path: gpui::SharedString, selected: bool, label: gpui::SharedString) -> gpui::Stateful<Div> {
    let k = key(("hdr-btn", id));
    let rest = c(tok(if selected { idx::ICON_STRONG } else { idx::ICON_MUTED }));
    let hover = c(tok(if selected { idx::ICON_STRONG } else { idx::ICON_SUBTLE }));
    track(div().id(id), k)
        .group(id)
        .relative()
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            svg()
                .path(path)
                .size(z(20.))
                .flex_none()
                .text_color(rest)
                .group_hover(id, move |s| s.text_color(hover)),
        )
        .children(hovered(k).then(|| below_tip(label, 32., 32.)))
        .map(|d| crate::ui::focus_ring::ring(d, k, 4.))
}

pub fn fade_to(key: u64, on: bool, rest: gpui::Hsla, hover: gpui::Hsla, ms: u64, radius: f32) -> gpui::AnyElement {
    HOVER.with(|h| {
        let mut h = h.borrow_mut();
        match h.get(&key) {
            Some(s) if s.0 == on => {}
            None => {
                h.insert(key, (on, std::time::Instant::now() - std::time::Duration::from_secs(1)));
            }
            Some(_) => {
                h.insert(key, (on, std::time::Instant::now()));
            }
        }
    });
    fade_layer(key, rest, hover, ms, radius)
}

pub fn tip_zoom() -> gpui::Animation {
    gpui::Animation::new(crate::ui::settings::effects::motion(100)).with_easing(ease_out)
}

pub fn zoom_card(d: Div, t: f32) -> Div {
    zoom_card_sized(d, t, 14., 18.)
}

pub fn zoom_card_sized(d: Div, t: f32, font: f32, line: f32) -> Div {
    let k = 0.95 + 0.05 * t;
    d.px(z(12. * k)).py(z(8. * k)).rounded(z(8. * k)).text_size(z(font * k)).line_height(z(line * k))
}

pub fn forget(key: u64) {
    HOVER.with(|h| {
        let mut h = h.borrow_mut();
        if h.get(&key).is_none_or(|s| s.0) {
            h.insert(key, (false, std::time::Instant::now() - std::time::Duration::from_secs(1)));
        }
    });
}
