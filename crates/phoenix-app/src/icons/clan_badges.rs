use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::Mutex;
use regex::Regex;

use super::cache;

pub const DISPLAY: [u32; 10] = [6, 0, 7, 8, 1, 2, 4, 5, 9, 3];

pub const DEFAULT_BADGE: u32 = DISPLAY[0];

pub const PALETTES: [(&str, &str); 13] = [
    ("#ff1c90", "#ff7fc0"),
    ("#ff8f1c", "#ffae77"),
    ("#eebe1a", "#fffc7f"),
    ("#32a070", "#57b59e"),
    ("#32839a", "#71c2d9"),
    ("#8a43ff", "#bd95ff"),
    ("#9b3fe5", "#cc8dff"),
    ("#942e8f", "#d46cb5"),
    ("#d14242", "#ff8989"),
    ("#814300", "#a88a6c"),
    ("#717224", "#c3c3c3"),
    ("#5d1d47", "#c58fbb"),
    ("#222222", "#cccccc"),
];

const KINDS: [(&str, u32); 10] = [
    ("SWORD", 0),
    ("WATER_DROP", 1),
    ("SKULL", 2),
    ("TOADSTOOL", 3),
    ("MOON", 4),
    ("LIGHTNING", 5),
    ("LEAF", 6),
    ("HEART", 7),
    ("FIRE", 8),
    ("COMPASS", 9),
];

pub fn label_key(kind: u32) -> &'static str {
    match kind {
        0 => "gs2.badge_sword",
        1 => "gs2.badge_water",
        2 => "gs2.badge_skull",
        3 => "gs2.badge_mushroom",
        4 => "gs2.badge_moon",
        5 => "gs2.badge_bolt",
        7 => "gs2.badge_heart",
        8 => "gs2.badge_fire",
        9 => "gs2.badge_compass",
        _ => "gs2.badge_leaf",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Literal(String),
    Primary(usize),
    Secondary(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub attrs: Vec<(String, String)>,
    pub fill: Option<Fill>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ramp {
    pub base: Vec<String>,
    pub lums: Vec<f64>,
    pub weights: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub shapes: Vec<Shape>,
    pub primary: Ramp,
    pub secondary: Option<Ramp>,
}

static BADGES: OnceLock<HashMap<u32, Badge>> = OnceLock::new();
static OUT: OnceLock<PathBuf> = OnceLock::new();

fn memo() -> &'static Mutex<HashMap<String, PathBuf>> {
    static M: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

const MARK: &str = "primaryTintLuminances:";

pub(super) fn init(dir: &Path) {
    let out = dir.join("art").join("clan-badges");
    let _ = OUT.set(out.clone());
    let _ = BADGES.set(load(&out).unwrap_or_default());
}

fn load(out: &Path) -> Option<HashMap<u32, Badge>> {
    let src = out.join("badges.js");
    if let Some(m) = std::fs::read_to_string(&src).ok().map(|js| parse(&js)).filter(|m| m.len() == KINDS.len()) {
        return Some(m);
    }
    let caches = super::discord_caches();
    let (js, found) = caches.iter().flat_map(|c| cache::asset_entries(c, ".js")).find_map(|(_, p)| {
        let body = cache::read_body(&p)?;
        if !body.contains(MARK) || !body.contains(".bl.SWORD:return") {
            return None;
        }
        let m = parse(&body);
        (m.len() == KINDS.len()).then_some((body, m))
    })?;
    std::fs::create_dir_all(out).ok()?;
    std::fs::write(&src, js).ok()?;
    Some(found)
}

fn re(cell: &'static OnceLock<Regex>, src: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(src).expect("valid regex"))
}

pub fn parse(js: &str) -> HashMap<u32, Badge> {
    static SWITCH: OnceLock<Regex> = OnceLock::new();
    let switch = re(&SWITCH, r"case [\w$]+\.bl\.([A-Z_]+):return\(0,[\w$]+\.jsx\)\(([\w$]+),\{primaryTintColor");
    let mut out = HashMap::new();
    for cap in switch.captures_iter(js) {
        let Some(kind) = KINDS.iter().find(|(n, _)| *n == &cap[1]).map(|(_, k)| *k) else { continue };
        if out.contains_key(&kind) {
            continue;
        }
        let at = cap.get(0).map_or(js.len(), |m| m.start());
        if let Some(b) = badge(&js[..at], &cap[2]) {
            out.insert(kind, b);
        }
    }
    out
}

fn badge(js: &str, func: &str) -> Option<Badge> {
    static CALL: OnceLock<Regex> = OnceLock::new();
    static PATH: OnceLock<Regex> = OnceLock::new();
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let head = Regex::new(&format!(r"function {}\([\w$]+\)\{{", regex::escape(func))).ok()?;
    let m = head.find_iter(js).last()?;
    let rest = &js[m.end()..];
    let body = &rest[..rest.find("function ").unwrap_or(rest.len())];
    let call = re(&CALL, r"\{primaryColorsTransformed:([\w$]+)(?:,secondaryColorsTransformed:([\w$]+))?\}=[\w$]+\(\{([^}]*)\}\)").captures(body)?;
    let pvar = call.get(1)?.as_str();
    let svar = call.get(2).map(|x| x.as_str());
    let args: HashMap<&str, &str> = call.get(3)?.as_str().split(',').filter_map(|kv| kv.split_once(':')).collect();
    let before = &js[m.start().saturating_sub(6000)..m.start()];
    let ramp = |p: &str| -> Option<Ramp> {
        Some(Ramp {
            base: strings(array(before, args.get(format!("{p}BaseColors").as_str())?)?),
            lums: numbers(array(before, args.get(format!("{p}TintLuminances").as_str())?)?),
            weights: weights(array(before, args.get(format!("{p}LuminanceWeights").as_str())?)?),
        })
    };
    let primary = ramp("primary")?;
    let secondary = if svar.is_some() { Some(ramp("secondary")?) } else { None };
    let attr = re(&ATTR, r#"(\w+):("[^"]*"|[\w$]+\[\d+\])"#);
    let mut shapes = Vec::new();
    for p in re(&PATH, r#"\(0,[\w$]+\.jsx\)\("path",\{(.*?)\}\)"#).captures_iter(body) {
        let mut s = Shape { attrs: Vec::new(), fill: None };
        for a in attr.captures_iter(&p[1]) {
            let (k, v) = (&a[1], &a[2]);
            let value = if let Some(lit) = v.strip_prefix('"') {
                Fill::Literal(lit.trim_end_matches('"').to_owned())
            } else {
                let (var, i) = v.trim_end_matches(']').split_once('[')?;
                let i: usize = i.parse().ok()?;
                if var == pvar {
                    Fill::Primary(i)
                } else if Some(var) == svar {
                    Fill::Secondary(i)
                } else {
                    return None;
                }
            };
            match (k, value) {
                ("fill", f) => s.fill = Some(f),
                (k, Fill::Literal(l)) => s.attrs.push((svg_attr(k), l)),
                _ => return None,
            }
        }
        shapes.push(s);
    }
    (!shapes.is_empty()).then_some(Badge { shapes, primary, secondary })
}

fn svg_attr(k: &str) -> String {
    match k {
        "fillRule" => "fill-rule".into(),
        "clipRule" => "clip-rule".into(),
        k => k.into(),
    }
}

fn array<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=[");
    let mut found = None;
    let mut from = 0;
    while let Some(i) = src[from..].find(&needle) {
        let at = from + i;
        let ok = at == 0 || src[..at].ends_with(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'));
        if ok {
            let start = at + needle.len() - 1;
            let end = src[start..].find(']').map(|e| start + e + 1)?;
            found = Some(&src[start..end]);
        }
        from = at + needle.len();
    }
    found
}

fn strings(a: &str) -> Vec<String> {
    a.split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

fn numbers(a: &str) -> Vec<f64> {
    a.trim_matches(['[', ']']).split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn weights(a: &str) -> Vec<(f64, f64)> {
    static W: OnceLock<Regex> = OnceLock::new();
    re(&W, r"\{base:([\d.]+),tint:([\d.]+)\}").captures_iter(a).filter_map(|c| Some((c[1].parse().ok()?, c[2].parse().ok()?))).collect()
}

fn parse_hex(s: &str) -> Option<[f64; 3]> {
    let h = s.strip_prefix('#')?;
    let h = match h.len() {
        6 => h.to_owned(),
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        _ => return None,
    };
    let v = u32::from_str_radix(&h, 16).ok()?;
    Some([(v >> 16 & 255) as f64, (v >> 8 & 255) as f64, (v & 255) as f64])
}

fn channel_lum(c: f64) -> f64 {
    let c = c / 255.;
    if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub fn luminance(rgb: [f64; 3]) -> f64 {
    0.2126 * channel_lum(rgb[0]) + 0.7152 * channel_lum(rgb[1]) + 0.0722 * channel_lum(rgb[2])
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

pub fn with_luminance(rgb: [f64; 3], target: f64) -> [f64; 3] {
    if target == 0. {
        return [0.; 3];
    }
    if target == 1. {
        return [255.; 3];
    }
    let (mut lo, mut hi) = if luminance(rgb) > target { ([0.; 3], rgb) } else { (rgb, [255.; 3]) };
    let mut n = 20;
    loop {
        let mid = [lo[0] + 0.5 * (hi[0] - lo[0]), lo[1] + 0.5 * (hi[1] - lo[1]), lo[2] + 0.5 * (hi[2] - lo[2])];
        let l = luminance(mid);
        if (target - l).abs() < 1e-7 || n == 0 {
            return mid.map(js_round);
        }
        n -= 1;
        if l > target {
            hi = mid;
        } else {
            lo = mid;
        }
    }
}

fn hex(rgb: [f64; 3]) -> String {
    let [r, g, b] = rgb.map(|x| js_round(x).clamp(0., 255.) as u32);
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub fn tint(ramp: &Ramp, color: Option<&str>) -> Vec<String> {
    let Some(c) = color else { return ramp.base.clone() };
    let Some(rgb) = parse_hex(c) else { return ramp.lums.iter().map(|_| "#000000".to_owned()).collect() };
    let l = luminance(rgb);
    ramp.lums.iter().zip(&ramp.weights).map(|(h, (base, w))| hex(with_luminance(rgb, (h * base + l * w) / (base + w)))).collect()
}

pub fn svg(b: &Badge, primary: Option<&str>, secondary: Option<&str>) -> String {
    let p = tint(&b.primary, primary);
    let s = b.secondary.as_ref().map(|r| tint(r, secondary)).unwrap_or_default();
    let mut out = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none">"#);
    for sh in &b.shapes {
        out.push_str("<path");
        for (k, v) in &sh.attrs {
            out.push_str(&format!(r#" {k}="{v}""#));
        }
        let fill = match &sh.fill {
            Some(Fill::Literal(l)) => Some(l.clone()),
            Some(Fill::Primary(i)) => p.get(*i).cloned(),
            Some(Fill::Secondary(i)) => s.get(*i).cloned(),
            None => None,
        };
        if let Some(f) = fill {
            out.push_str(&format!(r#" fill="{f}""#));
        }
        out.push_str("/>");
    }
    out.push_str("</svg>");
    out
}

pub fn colors(kind: u32) -> usize {
    match kind {
        0 | 3 | 9 => 2,
        _ => 1,
    }
}

pub fn file(kind: u32, primary: Option<&str>, secondary: Option<&str>) -> Option<PathBuf> {
    let key = format!("{kind}-{}-{}", primary.unwrap_or("base").trim_start_matches('#'), secondary.unwrap_or("base").trim_start_matches('#'));
    if let Some(p) = memo().lock().get(&key) {
        return Some(p.clone());
    }
    let b = BADGES.get()?.get(&kind)?;
    let path = OUT.get()?.join(format!("{key}.svg"));
    if !path.is_file() {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, svg(b, primary, secondary)).ok()?;
    }
    memo().lock().insert(key, path.clone());
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"let s=["#E79418","#FAE330"],i=[.5,.75],f=[{base:5,tint:1},{base:3,tint:1}];function e(h){let{width:v=24,height:A=24,primaryTintColor:H,...V}=h,{primaryColorsTransformed:Z}=a({primaryBaseColors:s,primaryTintColor:H,primaryTintLuminances:i,primaryLuminanceWeights:f});return(0,l.jsxs)("svg",{children:[(0,l.jsx)("path",{d:"M8 5v1H5Z",fill:"#B7DBF6"}),(0,l.jsx)("path",{d:"M4 11h-1Z",fill:Z[1]}),(0,l.jsx)("path",{d:"M12 5h1Z",fill:"black",fillRule:"evenodd"})]})}function A7(h){switch(v){case A6.bl.LEAF:return(0,l.jsx)(e,{primaryTintColor:A,...M});}}"##;

    #[test]
    fn parses_a_badge_from_the_client_chunk() {
        let m = parse(SAMPLE);
        let b = &m[&6];
        assert_eq!(b.primary.base, vec!["#E79418", "#FAE330"]);
        assert_eq!(b.primary.lums, vec![0.5, 0.75]);
        assert_eq!(b.primary.weights, vec![(5., 1.), (3., 1.)]);
        assert_eq!(b.shapes.len(), 3);
        assert_eq!(b.shapes[1].fill, Some(Fill::Primary(1)));
        assert_eq!(b.shapes[2].attrs, vec![("d".to_owned(), "M12 5h1Z".to_owned()), ("fill-rule".to_owned(), "evenodd".to_owned())]);
        let base = svg(b, None, None);
        assert!(base.contains(r##"d="M4 11h-1Z" fill="#FAE330""##));
    }

    #[test]
    fn tint_follows_chroma_luminance() {
        let r = Ramp { base: vec![], lums: vec![0.1, 0.2, 0.6, 0.9], weights: vec![(4., 1.), (3., 1.), (3., 1.), (10., 1.)] };
        assert_eq!(tint(&r, Some("#ff1c90")), vec!["#c0156c", "#f01a87", "#ffa0d1", "#ffe5f3"]);
        assert_eq!(tint(&r, Some("nope")), vec!["#000000"; 4]);
        assert_eq!(with_luminance([10., 20., 30.], 0.), [0.; 3]);
    }
}
