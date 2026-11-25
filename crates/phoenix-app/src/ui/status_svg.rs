use crate::model::Presence;

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

pub fn svg(path: &str) -> Option<String> {
    if path == MOBILE {
        return Some(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 150"><path fill="white" fill-rule="evenodd" d="M18.75 0 H81.25 A18.75 18.75 0 0 1 100 18.75 V131.25 A18.75 18.75 0 0 1 81.25 150 H18.75 A18.75 18.75 0 0 1 0 131.25 V18.75 A18.75 18.75 0 0 1 18.75 0 Z M12.5 25 V100 H87.5 V25 Z M37.5 125 A12.5 12.5 0 1 0 62.5 125 A12.5 12.5 0 1 0 37.5 125 Z"/></svg>"#
                .to_owned(),
        );
    }
    if let Some(icon) = switch_icon(path) {
        return Some(icon);
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

pub const MOBILE: &str = "status/mobile.svg";
pub const SWITCH_ON: &str = "switch/on.svg";
pub const SWITCH_OFF: &str = "switch/off.svg";

fn switch_icon(path: &str) -> Option<String> {
    let (a, b) = match path {
        SWITCH_OFF => ("M4.10585 5.3837L5.37864 4.11091L11.884 10.61632L10.6112 11.88912L4.10585 5.3837Z", "M10.61632 4.11091L11.88912 5.3837L5.3837 11.88912L4.11091 10.61632L10.61632 4.11091Z"),
        SWITCH_ON => ("M6.31649 11.88304L5.0437 10.61032L11.44792 4.20604L12.72072 5.47883L6.31649 11.88304Z", "M3.26914 8.87224L4.54194 7.59943L7.5588 10.61632L6.28601 11.88912L3.26914 8.87224Z"),
        _ => return None,
    };
    Some(format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g transform="translate(4 4)" fill="white"><path d="{a}"/><path d="{b}"/></g></svg>"#))
}

const CIRCLE: &str = "M100 50 A50 50 0 1 0 0 50 A50 50 0 1 0 100 50 Z";

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
    format!("M{p1x:.3} {p1y:.3} A{ar} {ar} 0 1 0 {p2x:.3} {p2y:.3} A{br} {br} 0 0 1 {p1x:.3} {p1y:.3} Z")
}

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

    #[test]
    fn switch_indicators_use_the_client_paths() {
        let on = svg(SWITCH_ON).unwrap();
        let off = svg(SWITCH_OFF).unwrap();
        assert!(on.contains("M6.31649 11.88304") && on.contains("translate(4 4)"));
        assert!(off.contains("M4.10585 5.3837") && off.contains("M10.61632 4.11091"));
        assert!(svg("switch/other.svg").is_none());
    }
}
