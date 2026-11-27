use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;

use super::client_mix;
use super::generated;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub id: u32,
    pub key: &'static str,
    pub light: bool,
    pub angle: f32,
    pub stops: &'static [(u32, f32)],
    pub overlay: f32,
}

const fn p(id: u32, key: &'static str, light: bool, angle: f32, stops: &'static [(u32, f32)]) -> Preset {
    Preset { id, key, light, angle, stops, overlay: 1. }
}

pub static PRESETS: &[Preset] = &[
    p(0, "MINT_APPLE", true, 180., &[(0x56B69F, 6.15), (0x63BC61, 48.7), (0x9ECA67, 93.07)]),
    p(1, "CITRUS_SHERBERT", true, 180., &[(0xF3B336, 31.1), (0xEE8558, 67.09)]),
    p(2, "RETRO_RAINCLOUD", true, 148.71, &[(0x3A7CA1, 5.64), (0x7F7EB9, 26.38), (0x7F7EB9, 49.92), (0x3A7CA1, 73.12)]),
    p(3, "HANAMI", true, 38.08, &[(0xEFAAB3, 3.56), (0xEFD696, 35.49), (0xA6DAA2, 68.78)]),
    p(4, "SUNRISE", true, 154.19, &[(0x9F4175, 8.62), (0xC49064, 48.07), (0xA6953D, 76.04)]),
    p(5, "COTTON_CANDY", true, 180.14, &[(0xF4ABB8, 8.5), (0xB1C2FC, 94.28)]),
    p(6, "LOFI_VIBES", true, 179.52, &[(0xA4C0F7, 7.08), (0xA9E4E8, 34.94), (0xB0E2B8, 65.12), (0xCFDFA2, 96.23)]),
    p(7, "DESERT_KHAKI", true, 38.99, &[(0xE7DBD0, 12.92), (0xDFD0B2, 32.92), (0xE0D6A3, 52.11)]),
    p(8, "SUNSET", false, 141.68, &[(0x48288C, 27.57), (0xDB7F4B, 71.25)]),
    p(9, "CHROMA_GLOW", false, 128.92, &[(0x0EB5BF, 3.94), (0x4C0CE0, 26.1), (0xA308A7, 39.82), (0x9A53FF, 56.89), (0x218BE0, 76.45)]),
    p(10, "FOREST", false, 162.27, &[(0x142215, 11.2), (0x2D4D39, 29.93), (0x454C32, 48.64), (0x5A7C58, 67.85), (0xA98E4B, 83.54)]),
    p(11, "CRIMSON_MOON", false, 64.92, &[(0x950909, 16.17), (0x000000, 72.)]),
    p(12, "MIDNIGHT_BLURPLE", false, 48.17, &[(0x5348CA, 11.21), (0x140730, 61.92)]),
    p(13, "MARS", false, 170.82, &[(0x895240, 14.61), (0x8F4343, 74.62)]),
    p(14, "DUSK", false, 180., &[(0x665069, 12.84), (0x91A3D1, 85.99)]),
    p(15, "UNDER_THE_SEA", false, 179.14, &[(0x647962, 1.91), (0x588575, 48.99), (0x6A8482, 96.35)]),
    p(17, "RETRO_STORM", false, 148.71, &[(0x3A7CA1, 5.64), (0x58579A, 26.38), (0x58579A, 49.92), (0x3A7CA1, 73.12)]),
    p(18, "NEON_NIGHTS", false, 180., &[(0x01A89E, 0.), (0x7D60BA, 50.), (0xB43898, 100.)]),
    p(20, "STRAWBERRY_LEMONADE", false, 161.03, &[(0xAF1A6C, 18.79), (0xC26B20, 49.76), (0xE7A525, 80.72)]),
    p(21, "AURORA", false, 239.16, &[(0x062053, 10.39), (0x191FBB, 26.87), (0x13929A, 48.31), (0x218573, 64.98), (0x051A81, 92.5)]),
    p(19, "SEPIA", false, 69.98, &[(0x857664, 14.14), (0x5B4421, 60.35)]),
    p(22, "BLURPLE_TWILIGHT", false, 47.61, &[(0x2C3FE7, 11.18), (0x261D83, 64.54)]),
];

pub static VICE_CITY: Preset = Preset {
    id: 23,
    key: "HEXAGON",
    light: false,
    angle: 135.,
    stops: &[(0x4645FF, 0.), (0xAF01F3, 33.33), (0xFF6698, 66.67), (0xF89A03, 100.)],
    overlay: 0.8,
};

static EASTER_EGG: Preset = p(16, "EASTER_EGG", true, 180., &[(0x7289DA, 4.), (0x4E5D94, 96.)]);

pub fn by_id(id: u32) -> Option<&'static Preset> {
    match id {
        23 => Some(&VICE_CITY),
        16 => Some(&EASTER_EGG),
        _ => PRESETS.iter().find(|p| p.id == id),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomTheme {
    pub colors: Vec<u32>,
    pub angle: f32,
    pub base_mix: f32,
}

impl CustomTheme {
    pub fn parse(colors: &[String], angle: i32, base_mix: i32) -> Option<Self> {
        let colors: Vec<u32> = colors.iter().filter_map(|c| u32::from_str_radix(c.trim().trim_start_matches('#'), 16).ok()).collect();
        if colors.is_empty() {
            return None;
        }
        Some(Self { colors, angle: angle as f32, base_mix: base_mix as f32 })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Preset(&'static Preset),
    Custom(CustomTheme),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub id: u64,
    pub light: bool,
    pub angle: f32,
    pub stops: Vec<(u32, f32)>,
    pub overlay: f32,
    pub custom_mix: Option<f32>,
}

impl Spec {
    pub fn new(source: &Source, light_base: bool) -> Self {
        match source {
            Source::Preset(p) => Self { id: u64::from(p.id), light: p.light, angle: p.angle, stops: p.stops.to_vec(), overlay: p.overlay, custom_mix: None },
            Source::Custom(t) => {
                let mut colors = t.colors.clone();
                if colors.len() == 1 {
                    colors.push(colors[0]);
                }
                let n = colors.len().max(2) - 1;
                let stops = colors.iter().enumerate().map(|(i, c)| (*c, i as f32 * 100. / n as f32)).collect();
                let mut id = 1u64 << 40;
                for c in &t.colors {
                    id = id.wrapping_mul(0x100_0000_01B3) ^ u64::from(*c);
                }
                id = id.wrapping_mul(31).wrapping_add(u64::from(t.angle.to_bits())).wrapping_mul(31).wrapping_add(u64::from(t.base_mix.to_bits()));
                Self { id: id | (u64::from(light_base) << 63), light: light_base, angle: t.angle, stops, overlay: 1., custom_mix: Some(t.base_mix) }
            }
        }
    }
}

struct Active {
    source: Source,
    hc: bool,
    theme: usize,
    spec: Arc<Spec>,
    table: Box<[u32]>,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

pub fn active() -> Option<Arc<Spec>> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    ACTIVE.read().as_ref().map(|a| a.spec.clone())
}

pub fn set_active(source: Option<Source>, hc: bool, base_theme: usize) -> bool {
    let Some(source) = source else {
        return ENABLED.swap(false, Ordering::Relaxed);
    };
    let theme = match &source {
        Source::Preset(p) => usize::from(p.light),
        Source::Custom(_) => base_theme.min(3),
    };
    let fresh = ACTIVE.read().as_ref().is_some_and(|a| a.source == source && a.hc == hc && a.theme == theme);
    if !fresh {
        let spec = Spec::new(&source, theme == 1);
        let table = build(&spec, theme, hc);
        *ACTIVE.write() = Some(Active { source, hc, theme, spec: Arc::new(spec), table });
    }
    !ENABLED.swap(true, Ordering::Relaxed) || !fresh
}

pub fn lookup(i: usize) -> Option<u32> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    ACTIVE.read().as_ref().and_then(|a| a.table.get(i).copied())
}

fn names() -> &'static HashMap<&'static str, usize> {
    static N: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    N.get_or_init(|| generated::NAMES.iter().enumerate().map(|(i, n)| (*n, i)).collect())
}

pub const BASE_MIX: f32 = 50.;
pub const TEXT_MIX: f32 = 38.;
pub const CUSTOM_BORDER_MIX: f32 = 5.;

pub fn amounts(spec: &Spec) -> (f32, f32, f32) {
    match spec.custom_mix {
        None => (BASE_MIX, TEXT_MIX, BASE_MIX),
        Some(u) => {
            let h = 100. - u;
            if spec.light { (30. - 0.2 * u, 40., CUSTOM_BORDER_MIX) } else { (60. + 0.4 * h, 30., CUSTOM_BORDER_MIX) }
        }
    }
}

fn build(spec: &Spec, theme: usize, hc: bool) -> Box<[u32]> {
    let tables = if hc { &generated::THEMES_HC } else { &generated::THEMES };
    let mut out: Box<[u32]> = tables[theme].to_vec().into_boxed_slice();
    let (base, text) = spec_colors(spec);
    let (base_amount, text_amount, border_amount) = amounts(spec);
    let entries = if spec.light { client_mix::LIGHT } else { client_mix::DARK };
    for (name, kind) in entries {
        let Some(&i) = names().get(name) else { continue };
        let color = if kind & 1 != 0 { text } else { base };
        let amount = if kind & 2 != 0 {
            text_amount
        } else if kind & 4 != 0 {
            border_amount
        } else {
            base_amount
        };
        out[i] = mix(out[i], color, amount);
    }
    out
}

pub fn spec_colors(spec: &Spec) -> (u32, u32) {
    let dark = !spec.light;
    let Some(u) = spec.custom_mix else {
        let primary = spec.stops.first().map_or(0, |s| s.0);
        let secondary = spec.stops.last().map_or(0, |s| s.0);
        return custom_colors(primary, secondary, dark);
    };
    let h = 100. - u;
    let lum = |c: &&(u32, f32)| luminance(c.0);
    let darkest = spec.stops.iter().min_by(|a, b| lum(a).total_cmp(&lum(b))).map_or(0, |s| s.0);
    let lightest = spec.stops.iter().max_by(|a, b| lum(a).total_cmp(&lum(b))).map_or(0, |s| s.0);
    if dark {
        let (dh, ds, _) = to_hsl(darkest);
        (from_hsl(dh, 1., 0.05), from_hsl(dh, ds, (100. - 0.1 * h).round() / 100.))
    } else {
        let (lh, ls, _) = to_hsl(lightest);
        (from_hsl(lh, 1., 0.9), from_hsl(lh, ls, (0.05 * h).round() / 100.))
    }
}

fn channels(c: u32) -> [f32; 3] {
    [((c >> 16) & 0xFF) as f32 / 255., ((c >> 8) & 0xFF) as f32 / 255., (c & 0xFF) as f32 / 255.]
}

fn channels64(c: u32) -> [f64; 3] {
    channels(c).map(f64::from)
}

fn to_lin(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn from_lin(v: f64) -> f64 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1. / 2.4) - 0.055 }
}

pub fn luminance(rgb: u32) -> f64 {
    let [r, g, b] = channels64(rgb).map(to_lin);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn to_oklab(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(to_lin);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab([l, a, b]: [f64; 3]) -> [f64; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
    .map(|v| from_lin(v.clamp(0., 1.)))
}

pub fn mix(rgba: u32, rgb: u32, amount: f32) -> u32 {
    let alpha = rgba & 0xFF;
    let amount = f64::from(amount);
    let w = amount / (100. + amount);
    let a = to_oklab(channels64(rgba >> 8));
    let b = to_oklab(channels64(rgb));
    let m = [0, 1, 2].map(|i| a[i] * (1. - w) + b[i] * w);
    let [r, g, bl] = from_oklab(m).map(|v| (v.clamp(0., 1.) * 255.).round() as u32);
    (r << 24) | (g << 16) | (bl << 8) | alpha
}

fn to_hsl(rgb: u32) -> (f32, f32, f32) {
    let [r, g, b] = channels(rgb);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    let d = max - min;
    if d == 0. {
        return (f32::NAN, 0., l);
    }
    let s = d / (1. - (2. * l - 1.).abs());
    let sector = if max == r {
        ((g - b) / d).rem_euclid(6.)
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    };
    let h = sector * 60.;
    (h, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> u32 {
    let h = if h.is_nan() { 0. } else { h };
    let c = (1. - (2. * l - 1.).abs()) * s;
    let x = c * (1. - ((h / 60.).rem_euclid(2.) - 1.).abs());
    let m = l - c / 2.;
    let (r, g, b) = match (h / 60.) as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let ch = |v: f32| (((v + m).clamp(0., 1.)) * 255.).round() as u32;
    (ch(r) << 16) | (ch(g) << 8) | ch(b)
}

pub fn custom_colors(primary: u32, secondary: u32, dark: bool) -> (u32, u32) {
    let (lighter, mut darker) = if luminance(primary) > luminance(secondary) { (primary, secondary) } else { (secondary, primary) };
    let dh = to_hsl(darker).0;
    if dh == 0. || dh.is_nan() {
        darker = lighter;
    }
    let pick = if dark { lighter } else { darker };
    let (h, _, _) = to_hsl(pick);
    let blueish = (200. ..=300.).contains(&h);
    let l = match (blueish, dark) {
        (true, true) => 0.8,
        (true, false) => 0.25,
        (false, true) => 0.95,
        (false, false) => 0.15,
    };
    let text = from_hsl(h, 1., l);
    let base = if dark { from_hsl(to_hsl(darker).0, 1., 0.05) } else { from_hsl(to_hsl(lighter).0, 1., 0.9) };
    (base, text)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    AppFrame,
    Chat,
    Highest,
}

fn opacity(spec: &Spec, level: Level) -> f32 {
    if let Some(u) = spec.custom_mix {
        let m = (30. + 0.7 * (100. - u)) / 100.;
        return match (level, spec.light) {
            (Level::AppFrame, false) => 0.42 + m * 0.4 * 0.7,
            (Level::Chat, false) => 0.48 + m * 0.4 * 0.8,
            (Level::Highest, false) => 0.39 + m * 0.4 * 0.65,
            (Level::AppFrame, true) => 0.325 + m * 0.5 * 0.65,
            (Level::Chat, true) => 0.95 * (0.6 + m * 0.4),
            (Level::Highest, true) => 0.8 + m * 0.2,
        };
    }
    let o = match (level, spec.light) {
        (Level::AppFrame, false) => 0.7,
        (Level::Chat, false) => 0.8,
        (Level::Highest, false) => 0.65,
        (Level::AppFrame, true) => 0.65,
        (Level::Chat, true) => 0.85,
        (Level::Highest, true) => 0.95,
    };
    (o * spec.overlay).clamp(0., 1.)
}

pub fn overlay_alpha(spec: &Spec, level: Level, under: Option<Level>) -> f32 {
    let o = opacity(spec, level);
    match under {
        None => o,
        Some(u) => {
            let ou = opacity(spec, u);
            if ou >= 1. { 0. } else { (1. - (1. - o) / (1. - ou)).max(0.) }
        }
    }
}

pub fn overlay_rgba(spec: &Spec, level: Level, under: Option<Level>) -> u32 {
    let a = (overlay_alpha(spec, level, under) * 255.).round() as u32;
    if spec.light { 0xFFFF_FF00 | a } else { a }
}

pub fn surface(level: Level, under: Option<Level>, fallback: u32) -> gpui::Hsla {
    match active() {
        Some(p) => gpui::rgba(overlay_rgba(&p, level, under)).into(),
        None => super::c(fallback),
    }
}

pub fn fixed_bg(fallback: u32) -> gpui::Hsla {
    if active().is_some() { gpui::transparent_black() } else { super::c(fallback) }
}

pub fn sample(spec: &Spec, w: f32, h: f32, x: f32, y: f32) -> u32 {
    let a = spec.angle.to_radians();
    let (sin, cos) = a.sin_cos();
    let len = (w * sin).abs() + (h * cos).abs();
    let t = (((x - w / 2.) * sin - (y - h / 2.) * cos) / len + 0.5) * 100.;
    let stops = &spec.stops;
    let Some(first) = stops.first() else { return 0 };
    if t <= first.1 {
        return first.0;
    }
    for pair in stops.windows(2) {
        let (c0, s0) = pair[0];
        let (c1, s1) = pair[1];
        if t <= s1 {
            let f = if s1 > s0 { (t - s0) / (s1 - s0) } else { 1. };
            let a = channels(c0);
            let b = channels(c1);
            let [r, g, bl] = [0, 1, 2].map(|i| ((a[i] + (b[i] - a[i]) * f).clamp(0., 1.) * 255.).round() as u32);
            return (r << 16) | (g << 8) | bl;
        }
    }
    stops.last().map_or(0, |s| s.0)
}

pub fn image(spec: &Spec, w: f32, h: f32) -> Option<Arc<gpui::RenderImage>> {
    type Cache = Option<((u64, u32, u32), Arc<gpui::RenderImage>)>;
    static CACHE: RwLock<Cache> = RwLock::new(None);
    if w < 1. || h < 1. {
        return None;
    }
    let scale = (320. / w.max(h)).min(1.);
    let (iw, ih) = (((w * scale).round() as u32).max(2), ((h * scale).round() as u32).max(2));
    let key = (spec.id, iw, ih);
    if let Some((k, img)) = CACHE.read().as_ref()
        && *k == key
    {
        return Some(img.clone());
    }
    let mut buf = image::RgbaImage::new(iw, ih);
    for (x, y, px) in buf.enumerate_pixels_mut() {
        let c = sample(spec, iw as f32, ih as f32, x as f32 + 0.5, y as f32 + 0.5);
        *px = image::Rgba([c as u8, (c >> 8) as u8, (c >> 16) as u8, 255]);
    }
    let img = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buf)]));
    *CACHE.write() = Some((key, img.clone()));
    Some(img)
}

static ROOT: RwLock<Option<gpui::Bounds<gpui::Pixels>>> = RwLock::new(None);

fn paint_layer(spec: &Spec, level: Level, bounds: gpui::Bounds<gpui::Pixels>, root: gpui::Bounds<gpui::Pixels>, radius: gpui::Pixels, window: &mut gpui::Window) {
    let corners = gpui::Corners::all(radius);
    if let Some(img) = image(spec, f32::from(root.size.width), f32::from(root.size.height)) {
        let _ = window.paint_image(bounds, root, corners, img, 0, false);
    }
    window.paint_quad(gpui::fill(bounds, gpui::rgba(overlay_rgba(spec, level, None))).corner_radii(corners));
}

pub fn backdrop(_: &gpui::Window) -> Option<gpui::AnyElement> {
    use gpui::{IntoElement, Styled};
    active()?;
    Some(
        gpui::canvas(
            |_, _, _| {},
            |bounds, _, window, _| {
                *ROOT.write() = Some(bounds);
                if let Some(spec) = active() {
                    paint_layer(&spec, Level::AppFrame, bounds, bounds, gpui::Pixels::ZERO, window);
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

pub fn fixed_layer(level: Level, radius: gpui::Pixels) -> Option<gpui::AnyElement> {
    use gpui::{IntoElement, Styled};
    active()?;
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let Some(spec) = active() else { return };
                let root = (*ROOT.read()).unwrap_or(bounds);
                paint_layer(&spec, level, bounds, root, radius, window);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_mix_matches_css() {
        let white = mix(0xFFFF_FFFF, 0xFFFFFF, 50.);
        assert!(channels(white >> 8).iter().all(|v| *v > 0.99) && white & 0xFF == 0xFF);
        assert_eq!(mix(0x0000_0080, 0x000000, 38.), 0x0000_0080);
        let m = mix(0xFF00_00FF, 0x0000FF, 100.);
        assert_eq!(m & 0xFF, 0xFF);
        let [r, _, b] = channels(m >> 8);
        assert!(r > 0.4 && b > 0.4);
    }

    fn spec(id: u32) -> Spec {
        Spec::new(&Source::Preset(by_id(id).unwrap()), false)
    }

    #[test]
    fn gradient_follows_css_angle() {
        let p = spec(18);
        assert_eq!(sample(&p, 100., 100., 50., 0.), 0x01A89E);
        assert_eq!(sample(&p, 100., 100., 50., 100.), 0xB43898);
        assert_eq!(sample(&spec(11), 100., 10., 100., 5.), 0x000000);
    }

    #[test]
    fn custom_colors_follow_the_client_rules() {
        let (base, text) = spec_colors(&spec(8));
        let (_, s, l) = to_hsl(base);
        assert!((s - 1.).abs() < 0.02 && (l - 0.05).abs() < 0.01);
        let (_, _, tl) = to_hsl(text);
        assert!((tl - 0.95).abs() < 0.01);
        assert_eq!(PRESETS.len(), 22);
    }

    #[test]
    fn overlays_compose_to_the_client_opacity() {
        let p = spec(8);
        let root = overlay_alpha(&p, Level::AppFrame, None);
        let chat = overlay_alpha(&p, Level::Chat, Some(Level::AppFrame));
        assert!((1. - (1. - root) * (1. - chat) - 0.8).abs() < 1e-4);
        assert_eq!(overlay_alpha(&p, Level::Highest, Some(Level::Chat)), 0.);
        assert!(overlay_alpha(&p, Level::Highest, None) < overlay_alpha(&p, Level::Chat, None));
    }

    #[test]
    fn vice_city_uses_the_hexagon_preset() {
        let v = spec(23);
        assert_eq!(v.stops.len(), 4);
        assert!((overlay_alpha(&v, Level::Chat, None) - 0.64).abs() < 1e-4);
        assert!((overlay_alpha(&v, Level::Highest, None) - 0.52).abs() < 1e-4);
        assert_eq!(by_id(16).map(|p| p.light), Some(true));
    }

    #[test]
    fn custom_themes_follow_the_editor_css() {
        let t = CustomTheme::parse(&["#5865F2".to_owned()], 90, 74).unwrap();
        let dark = Spec::new(&Source::Custom(t.clone()), false);
        assert_eq!(dark.stops, vec![(0x5865F2, 0.), (0x5865F2, 100.)]);
        let (base, text, border) = amounts(&dark);
        assert!((base - 70.4).abs() < 1e-4 && (text - 30.).abs() < 1e-4 && (border - 5.).abs() < 1e-4);
        let m = (30. + 0.7 * 26.) / 100.;
        assert!((overlay_alpha(&dark, Level::Chat, None) - (0.48 + m * 0.32)).abs() < 1e-4);
        let light = Spec::new(&Source::Custom(t), true);
        assert!((amounts(&light).0 - (30. - 0.2 * 74.)).abs() < 1e-4);
        let (b, tx) = spec_colors(&dark);
        assert!((to_hsl(b).2 - 0.05).abs() < 0.01);
        assert!((to_hsl(tx).2 - 0.97).abs() < 0.01);
        assert!(CustomTheme::parse(&[], 0, 0).is_none());
    }
}
