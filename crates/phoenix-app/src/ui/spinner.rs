use std::sync::OnceLock;
use std::time::Instant;

use gpui::{AnyElement, Bounds, Hsla, IntoElement, ParentElement, Pixels, Styled, TransformationMatrix, Window, canvas, div, point, radians, size};

use crate::theme::{c, generated::idx, tok, z};

pub const CUBE_PERIOD: f32 = 1.8;
pub const CUBE: f32 = 10.;
pub const BOX: f32 = 32.;
pub const DOT_PERIOD: f32 = 1.4;

const SQUARE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#;

const FRAMES: [(f32, f32, f32, f32); 5] = [(0., 0., 0., 1.), (22., 0., -90., 0.5), (22., 22., -180., 1.), (0., 22., -270., 0.5), (0., 0., -360., 1.)];

fn clock() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

pub fn cube_at(t: f32) -> (f32, f32, f32, f32) {
    let p = (t / CUBE_PERIOD).rem_euclid(1.) * 4.;
    let i = (p.floor() as usize).min(3);
    let e = crate::ui::states::cubic_bezier(0.42, 0., 0.58, 1.)(p - i as f32);
    let (a, b) = (FRAMES[i], FRAMES[i + 1]);
    let l = |x: f32, y: f32| x + (y - x) * e;
    (l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3))
}

pub fn dot_opacity(t: f32, delay: f32) -> f32 {
    if t < delay {
        return 0.3;
    }
    let p = ((t - delay) / DOT_PERIOD).rem_euclid(1.);
    let ease = crate::ui::states::cubic_bezier(0.42, 0., 0.58, 1.);
    if p < 0.5 { 1. - 0.4 * ease(p * 2.) } else { 0.6 + 0.4 * ease(p * 2. - 1.) }
}

fn paint_cube(origin: gpui::Point<Pixels>, (tx, ty, rot, scale): (f32, f32, f32, f32), color: Hsla, window: &mut Window, cx: &mut gpui::App) {
    let b = Bounds::new(point(origin.x + z(tx), origin.y + z(ty)), size(z(CUBE), z(CUBE)));
    let sf = window.scale_factor();
    let center = b.center();
    let m = TransformationMatrix::unit().translate(center.scale(sf)).rotate(radians(rot.to_radians())).scale(size(scale, scale)).translate(center.scale(-sf));
    let _ = window.paint_svg(b, "phoenix-spinner-cube".into(), Some(SQUARE.as_bytes()), m, color, cx);
}

pub fn wandering_cubes() -> AnyElement {
    let color = c(tok(idx::BRAND_400));
    div()
        .flex()
        .items_center()
        .justify_center()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    let t = clock();
                    paint_cube(bounds.origin, cube_at(t), color, window, cx);
                    paint_cube(bounds.origin, cube_at(t + CUBE_PERIOD / 2.), color, window, cx);
                    window.request_animation_frame();
                },
            )
            .size(z(BOX)),
        )
        .into_any_element()
}

pub fn low_motion() -> AnyElement {
    let color = c(tok(idx::INTERACTIVE_TEXT_DEFAULT));
    div()
        .flex()
        .items_center()
        .justify_center()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let t = clock();
                    for i in 0..3 {
                        let x = bounds.origin.x + z(8. * i as f32);
                        let y = bounds.origin.y + (bounds.size.height - z(6.)) / 2.;
                        let q = gpui::fill(Bounds::new(point(x, y), size(z(6.), z(6.))), color.opacity(dot_opacity(t, 0.2 * i as f32))).corner_radii(z(3.));
                        window.paint_quad(q);
                    }
                    window.request_animation_frame();
                },
            )
            .w(z(22.))
            .h(z(BOX)),
        )
        .into_any_element()
}

pub fn spinner() -> AnyElement {
    if crate::ui::settings::effects::reduced_motion() { low_motion() } else { wandering_cubes() }
}

pub fn centered(pad: f32) -> AnyElement {
    div().w_full().flex().justify_center().py(z(pad)).child(spinner()).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubes_follow_the_keyframes() {
        assert_eq!(cube_at(0.), (0., 0., 0., 1.));
        let q = cube_at(CUBE_PERIOD / 4.);
        assert!((q.0 - 22.).abs() < 1e-3 && q.1.abs() < 1e-3 && (q.2 + 90.).abs() < 1e-3 && (q.3 - 0.5).abs() < 1e-3);
        let h = cube_at(CUBE_PERIOD / 2.);
        assert!((h.0 - 22.).abs() < 1e-3 && (h.1 - 22.).abs() < 1e-3);
        let mid = cube_at(CUBE_PERIOD / 8.);
        assert!((mid.0 - 11.).abs() < 1e-2);
    }

    #[test]
    fn dots_pulse_after_their_delay() {
        assert_eq!(dot_opacity(0.1, 0.2), 0.3);
        assert!((dot_opacity(0.2, 0.2) - 1.).abs() < 1e-4);
        assert!((dot_opacity(0.2 + DOT_PERIOD / 2., 0.2) - 0.6).abs() < 1e-3);
    }
}
