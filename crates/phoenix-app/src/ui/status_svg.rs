//! Presence status glyphs and the avatar status cut-out, built like the
//! client's SVG masks (values measured on the live client's `<mask>` defs):
//!
//! - `svg-mask-status-online`: circle (.5, .5) r .5
//! - `svg-mask-status-idle`: that circle minus circle (.25, .25) r .375
//! - `svg-mask-status-dnd`: minus rect (.125, .375) .75 x .25, rx .125
//! - `svg-mask-status-offline`: minus circle (.5, .5) r .25
//! - `svg-mask-status-streaming`: minus triangle (.35,.25) (.783,.5) (.35,.75)
//! - `svg-mask-avatar-status-round-<size>`: avatar circle minus a circle at
//!   (cx, cx) radius r (fractions per size below), the status rect being
//!   centred on that circle (32px: rect 22,22 10x10; 80px: rect 60,60 16x16).
//!
//! The masks only remove pixels, they never paint: the status shapes here
//! are single closed paths (the idle crescent is computed from the two
//! circles' intersection), so nothing spills outside the dot.

use crate::model::Presence;

/// Asset path of the status glyph for `p` (`status/<name>.svg`).
pub fn path(p: Presence, streaming: bool) -> &'static str {
    if streaming {
        return "status/streaming.svg";
    }
    match p {
        Presence::Online => "status/online.svg",
        Presence::Idle => "status/idle.svg",
        Presence::Dnd => "status/dnd.svg",
        Presence::Offline => "status/offline.svg",
    }
}

/// SVG for a `status/*.svg` asset path, in a 0..100 view box.
pub fn svg(path: &str) -> Option<String> {
    if path == MOBILE {
        // svg-mask-status-online-mobile over its 10x15 rect (view box 100x150):
        // rect rx .1875 ry .125, minus screen (.125, .1667) .75x.5 and the
        // home button ellipse (.5, .8333) .125x.0833.
        return Some(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 150"><path fill="white" fill-rule="evenodd" d="M18.75 0 H81.25 A18.75 18.75 0 0 1 100 18.75 V131.25 A18.75 18.75 0 0 1 81.25 150 H18.75 A18.75 18.75 0 0 1 0 131.25 V18.75 A18.75 18.75 0 0 1 18.75 0 Z M12.5 25 V100 H87.5 V25 Z M37.5 125 A12.5 12.5 0 1 0 62.5 125 A12.5 12.5 0 1 0 37.5 125 Z"/></svg>"#
                .to_owned(),
        );
    }
    let body = match path.strip_prefix("status/")?.strip_suffix(".svg")? {
        "online" => CIRCLE.to_owned(),
        "offline" => format!("{CIRCLE} M75 50 A25 25 0 1 0 25 50 A25 25 0 1 0 75 50 Z"),
        "dnd" => format!(
            "{CIRCLE} M25 37.5 L75 37.5 A12.5 12.5 0 0 1 75 62.5 L25 62.5 A12.5 12.5 0 0 1 25 37.5 Z"
        ),
        "streaming" => format!("{CIRCLE} M35 25 L78.301 50 L35 75 Z"),
        "idle" => idle(),
        _ => return None,
    };
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><path fill="white" fill-rule="evenodd" d="{body}"/></svg>"#
    ))
}

/// Asset path of the "online on mobile" phone glyph.
pub const MOBILE: &str = "status/mobile.svg";

const CIRCLE: &str = "M100 50 A50 50 0 1 0 0 50 A50 50 0 1 0 100 50 Z";

/// Circle (50,50) r50 minus circle (25,25) r37.5: the outer arc between the
/// two intersection points (the long way, through the bottom right), then
/// back along the inner circle.
fn idle() -> String {
    let (ax, ay, ar) = (50f64, 50f64, 50f64);
    let (bx, by, br) = (25f64, 25f64, 37.5f64);
    let (dx, dy) = (bx - ax, by - ay);
    let d = (dx * dx + dy * dy).sqrt();
    let a = (ar * ar - br * br + d * d) / (2. * d);
    let h = (ar * ar - a * a).sqrt();
    let (mx, my) = (ax + a * dx / d, ay + a * dy / d);
    let (p1x, p1y) = (mx + h * dy / d, my - h * dx / d);
    let (p2x, p2y) = (mx - h * dy / d, my + h * dx / d);
    // SVG y points down: the long way round A (through the bottom right) is
    // sweep 0; the short arc of B back through the inside of A is sweep 1.
    format!("M{p1x:.3} {p1y:.3} A{ar} {ar} 0 1 0 {p2x:.3} {p2y:.3} A{br} {br} 0 0 1 {p1x:.3} {p1y:.3} Z")
}

/// Avatar status geometry for an avatar of `size` px:
/// `(cut centre, cut radius, status size)` from `svg-mask-avatar-status-round-<size>`.
pub fn geometry(size: f32) -> (f32, f32, f32) {
    let (c, r, s) = match size.round() as u32 {
        0..=16 => (0.8125, 0.3125, 6.),
        17..=20 => (0.85, 0.25, 6.),
        21..=24 => (0.8333, 0.2917, 8.),
        25..=32 => (0.84375, 0.25, 10.),
        33..=40 => (0.85, 0.25, 12.),
        41..=56 => (0.875, 0.2083, 12.),
        57..=80 => (0.85, 0.175, 16.),
        81..=96 => (0.8333, 0.1875, 20.),
        _ => (0.8333, 0.1667, 24.),
    };
    // Sizes between the client's steps scale the nearest step.
    let step = match size.round() as u32 {
        0..=16 => 16.,
        17..=20 => 20.,
        21..=24 => 24.,
        25..=32 => 32.,
        33..=40 => 40.,
        41..=56 => 48.,
        57..=80 => 80.,
        81..=96 => 96.,
        _ => 120.,
    };
    (c * size, r * size, s * size / step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_rects() {
        // 32px: rect 22,22 10x10; 80px: rect 60,60 16x16 (live client).
        let (c, _, s) = geometry(32.);
        assert_eq!((c - s / 2., s), (22., 10.));
        let (c, _, s) = geometry(80.);
        assert_eq!((c - s / 2., s), (60., 16.));
    }

    #[test]
    fn svgs_parse() {
        assert!(svg(MOBILE).is_some_and(|s| s.contains("150")));
        for p in ["online", "idle", "dnd", "offline", "streaming"] {
            let s = svg(&format!("status/{p}.svg")).unwrap();
            assert!(s.contains("<path"), "{p}");
            assert!(!s.contains("NaN"), "{p}");
        }
    }
}
