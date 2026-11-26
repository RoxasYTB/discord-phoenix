use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub const NAME_STYLE_FAMILIES: &[&str] = &[
    "Sakura", "Pricedown", "Journal", "Jellybean", "Headbang", "Mainframe", "Medieval",
    "Modern", "Monkey Bars", "Munro", "Tempo", "Vampyre", "8Bit", "Corinthia",
    "Delicious Handrawn", "Fraunces",
];

const MAGIC: [u8; 8] = [0x30, 0x5c, 0x72, 0xa7, 0x1b, 0x6d, 0xfb, 0xfc];
const EOF_MAGIC: [u8; 8] = [0xd8, 0x41, 0x0d, 0x97, 0x45, 0x6f, 0xfa, 0xf4];

pub fn out_dir() -> Option<PathBuf> {
    crate::icons::out_dir().map(|d| d.with_file_name("fonts"))
}

fn asset_key(head: &[u8]) -> Option<String> {
    if head.len() < 24 || head[..8] != MAGIC {
        return None;
    }
    let kl = u32::from_le_bytes(head[12..16].try_into().ok()?) as usize;
    let key = std::str::from_utf8(head.get(24..24 + kl)?).ok()?;
    let name = key.rsplit_once("/assets/")?.1;
    let name = name.split_once('?').map_or(name, |(n, _)| n);
    (!name.contains('/')).then(|| name.to_owned())
}

fn body(entry: &Path) -> Option<Vec<u8>> {
    let b = std::fs::read(entry).ok()?;
    let kl = u32::from_le_bytes(b.get(12..16)?.try_into().ok()?) as usize;
    let start = 24 + kl;
    let end = memchr::memmem::find(b.get(start..)?, &EOF_MAGIC).map_or(b.len(), |p| p + start);
    let raw = b.get(start..end)?;
    if is_font(raw) || raw.starts_with(b"@") || std::str::from_utf8(raw).is_ok() {
        return Some(raw.to_vec());
    }
    let mut out = Vec::new();
    if brotli_decompressor::Decompressor::new(raw, 4096).read_to_end(&mut out).is_ok() {
        return Some(out);
    }
    out.clear();
    flate2::read::GzDecoder::new(raw).read_to_end(&mut out).ok()?;
    Some(out)
}

fn is_font(b: &[u8]) -> bool {
    b.starts_with(b"wOF2") || b.starts_with(b"wOFF") || b.starts_with(&[0, 1, 0, 0]) || b.starts_with(b"OTTO")
}

pub const UI_FAMILIES: &[&str] = &["gg sans", "gg mono"];

pub const DISPLAY_FAMILIES: &[&str] = &["ABC Ginto Discord Nord", "ABC Ginto Discord", "ABC Ginto Nord"];

pub const NITRO: &str = "ABC Ginto Discord Nord";
pub const MARKETING: &str = "ABC Ginto Discord";
pub const HEADLINE: &str = "ABC Ginto Nord";

pub fn display_family(css: &str) -> gpui::SharedString {
    FAMILIES.read().as_ref().and_then(|m| m.get(css).cloned()).unwrap_or_else(|| display_stand_in(css).into())
}

pub fn display_stand_in(css: &str) -> &'static str {
    if css == MARKETING { ui_family() } else { fallback::noto_family() }
}

pub fn parse_font_faces(css: &str) -> Vec<(String, u16, String)> {
    parse_font_faces_styled(css).into_iter().map(|(f, w, _, a)| (f, w, a)).collect()
}

pub fn parse_font_faces_styled(css: &str) -> Vec<(String, u16, bool, String)> {
    let mut out = Vec::new();
    for rule in css.split("@font-face{").skip(1) {
        let rule = rule.split('}').next().unwrap_or("");
        let prop = |k: &str| {
            rule.split(';').find_map(|d| d.trim().strip_prefix(k)).map(|v| v.trim().trim_matches('"').trim_matches('\''))
        };
        let Some(family) = prop("font-family:") else { continue };
        let weight = prop("font-weight:").and_then(|w| w.parse().ok()).unwrap_or(400);
        let Some(src) = prop("src:") else { continue };
        let Some(url) = src.split("/assets/").nth(1).and_then(|s| s.split(['"', ')', '\'']).next()) else {
            continue;
        };
        let italic = prop("font-style:") == Some("italic");
        out.push((family.to_owned(), weight, italic, url.to_owned()));
    }
    out
}

fn wanted(family: &str) -> bool {
    NAME_STYLE_FAMILIES.contains(&family) || UI_FAMILIES.contains(&family) || DISPLAY_FAMILIES.contains(&family)
}

pub fn face_file(family: &str, weight: u16, italic: bool, ext: &str) -> String {
    format!("{}-{weight}{}.{ext}", family.replace(' ', "_"), if italic { "i" } else { "" })
}

pub struct Extracted {
    pub written: Vec<PathBuf>,
    pub missing: Vec<(PathBuf, String)>,
}

pub fn extract(cache: &Path, dir: &Path) -> Extracted {
    let mut entries: HashMap<String, PathBuf> = HashMap::new();
    for e in std::fs::read_dir(cache).into_iter().flatten().flatten() {
        let p = e.path();
        let mut head = [0u8; 1024];
        let Ok(mut f) = std::fs::File::open(&p) else { continue };
        let n = f.read(&mut head).unwrap_or(0);
        if let Some(k) = asset_key(&head[..n]) {
            entries.insert(k, p);
        }
    }
    let mut faces = Vec::new();
    for (k, p) in &entries {
        if k.ends_with(".css")
            && let Some(b) = body(p)
            && let Ok(css) = String::from_utf8(b)
        {
            faces.extend(parse_font_faces_styled(&css));
        }
    }
    let mut out = Extracted { written: Vec::new(), missing: Vec::new() };
    let _ = std::fs::create_dir_all(dir);
    faces.sort();
    faces.dedup();
    for (family, weight, italic, asset) in faces {
        if !wanted(&family) {
            continue;
        }
        let ext = asset.rsplit('.').next().unwrap_or("woff2");
        let target = dir.join(face_file(&family, weight, italic, ext));
        let Some(b) = entries.get(&asset).and_then(|p| body(p)).filter(|b| is_font(b)) else {
            if !target.exists() && !target.with_extension("ttf").exists() {
                out.missing.push((target, asset));
            }
            continue;
        };
        if std::fs::metadata(&target).is_ok_and(|m| m.len() == b.len() as u64) || std::fs::write(&target, &b).is_ok() {
            out.written.push(target);
        }
    }
    out
}

fn download(missing: Vec<(PathBuf, String)>) -> Vec<PathBuf> {
    if missing.is_empty() || std::env::var_os("PHX_NO_FONT_DOWNLOAD").is_some() {
        return Vec::new();
    }
    let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return Vec::new() };
    rt.block_on(async {
        let client = reqwest::Client::new();
        let mut got = Vec::new();
        for (target, asset) in missing {
            let url = format!("https://discord.com/assets/{}", asset.trim_start_matches("/assets/"));
            let Ok(r) = client.get(url).send().await else { continue };
            if !r.status().is_success() {
                continue;
            }
            let Ok(b) = r.bytes().await else { continue };
            if is_font(&b) && std::fs::write(&target, &b).is_ok() {
                got.push(target);
            }
        }
        got
    })
}

pub fn init() -> Vec<PathBuf> {
    let Some(dir) = out_dir() else { return Vec::new() };
    let mut written = Vec::new();
    let mut missing: Vec<(PathBuf, String)> = Vec::new();
    for c in crate::icons::discord_caches() {
        let e = extract(&c, &dir);
        written.extend(e.written);
        for m in e.missing {
            if !missing.iter().any(|(t, _)| *t == m.0) {
                missing.push(m);
            }
        }
    }
    missing.retain(|(t, _)| !written.contains(t));
    written.extend(download(missing));
    written
}

pub fn font_for_id(font_id: u32) -> Option<(&'static str, f32)> {
    Some(match font_id {
        3 => ("Sakura", 0.04),
        4 => ("Jellybean", 0.),
        6 => ("Modern", 0.01),
        7 => ("Medieval", 0.02),
        8 => ("8Bit", 0.02),
        10 => ("Vampyre", 0.01),
        12 => ("Tempo", 0.03),
        13 => ("Monkey Bars", 0.),
        14 => ("Mainframe", 0.03),
        15 => ("Headbang", 0.),
        16 => ("Journal", 0.),
        17 => ("Pricedown", 0.),
        _ => return None,
    })
}

static FAMILIES: parking_lot::RwLock<Option<HashMap<String, gpui::SharedString>>> = parking_lot::RwLock::new(None);

pub fn family_for_id(font_id: u32) -> Option<gpui::SharedString> {
    let (css, _) = font_for_id(font_id)?;
    FAMILIES.read().as_ref()?.get(css).cloned()
}

pub fn to_ttf(b: &[u8]) -> Option<Vec<u8>> {
    if b.starts_with(b"wOF2") {
        wuff::decompress_woff2(b).ok()
    } else if b.starts_with(&[0, 1, 0, 0]) || b.starts_with(b"OTTO") {
        Some(b.to_vec())
    } else {
        None
    }
}

pub fn ttf_family(ttf: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(ttf, 0).ok()?;
    let names = face.names();
    [16u16, 1].iter().find_map(|&id| {
        names.into_iter().filter(|n| n.name_id == id && n.is_unicode()).find_map(|n| n.to_string())
    })
}

static GG_SANS: AtomicBool = AtomicBool::new(false);
static GG_MONO: AtomicBool = AtomicBool::new(false);
static GG_MONO_NAME: parking_lot::RwLock<&'static str> = parking_lot::RwLock::new("gg mono");

pub fn ui_family() -> &'static str {
    if GG_SANS.load(Ordering::Relaxed) { "gg sans" } else { fallback::noto_family() }
}

pub fn mono_family() -> &'static str {
    if GG_MONO.load(Ordering::Relaxed) { *GG_MONO_NAME.read() } else { fallback::code_family() }
}

pub fn ui_root<E: gpui::Styled>(mut el: E) -> E {
    el.text_style().font_fallbacks = Some(ui_fallbacks());
    el.font_family(ui_family())
}

pub fn nitro<E: gpui::Styled>(el: E, italic: bool) -> E {
    let el = el.font_family(display_family(NITRO)).font_weight(gpui::FontWeight::BOLD);
    if italic { el.italic() } else { el }
}

fn apply_theme_fonts(cx: &mut gpui::App) {
    if !cx.has_global::<gpui_component::theme::Theme>() {
        return;
    }
    let theme = gpui_component::theme::Theme::global_mut(cx);
    theme.font_family = ui_family().into();
    theme.mono_font_family = mono_family().into();
}

pub struct Decoded {
    pub key: String,
    pub css: String,
    pub family: String,
    pub ttf: Vec<u8>,
}

fn load_decoded(dir: &Path) -> Vec<Decoded> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "ttf") {
            continue;
        }
        let Some(key) = p.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else { continue };
        let Some(css) = key.rsplit_once('-').map(|(f, _)| f.replace('_', " ")) else { continue };
        let Ok(ttf) = std::fs::read(&p) else { continue };
        let Some(family) = ttf_family(&ttf) else { continue };
        out.push(Decoded { key, css, family, ttf });
    }
    out
}

fn extract_and_decode() -> Vec<Decoded> {
    let Some(dir) = out_dir() else { return Vec::new() };
    for p in init() {
        let ttf_path = p.with_extension("ttf");
        if p.extension().is_some_and(|x| x == "ttf") || ttf_path.exists() {
            continue;
        }
        if let Some(ttf) = std::fs::read(&p).ok().and_then(|b| to_ttf(&b)) {
            let _ = std::fs::write(&ttf_path, ttf);
        }
    }
    load_decoded(&dir)
}

static INSTALLED: parking_lot::RwLock<Vec<String>> = parking_lot::RwLock::new(Vec::new());

fn install(fonts: Vec<Decoded>, cx: &mut gpui::App) -> bool {
    let no_gg = std::env::var_os("PHX_NO_GG_FONTS").is_some();
    let fonts: Vec<_> = {
        let installed = INSTALLED.read();
        fonts
            .into_iter()
            .filter(|f| !(no_gg && UI_FAMILIES.contains(&f.css.as_str())))
            .filter(|f| !installed.contains(&f.key))
            .collect()
    };
    if fonts.is_empty() {
        return false;
    }
    let mut map = FAMILIES.read().clone().unwrap_or_default();
    let mut blobs = Vec::new();
    let mut keys = Vec::new();
    for f in fonts {
        map.entry(f.css).or_insert_with(|| gpui::SharedString::from(f.family));
        keys.push(f.key);
        blobs.push(std::borrow::Cow::Owned(f.ttf));
    }
    if cx.text_system().add_fonts(blobs).is_err() {
        return false;
    }
    INSTALLED.write().extend(keys);
    if map.get("gg sans").is_some_and(|f| f == "gg sans") {
        GG_SANS.store(true, Ordering::Relaxed);
    }
    if let Some(m) = map.get("gg mono") {
        *GG_MONO_NAME.write() = Box::leak(m.to_string().into_boxed_str());
        GG_MONO.store(true, Ordering::Relaxed);
    }
    *FAMILIES.write() = Some(map);
    apply_theme_fonts(cx);
    true
}

pub fn register(cx: &mut gpui::App) {
    fallback::register(cx);
    let cached = out_dir().map(|d| load_decoded(&d)).unwrap_or_default();
    install(cached, cx);
    apply_theme_fonts(cx);
    if std::env::var_os("PHX_NO_FONT_EXTRACT").is_some() {
        return;
    }
    let task = cx.background_executor().spawn(async move { extract_and_decode() });
    cx.spawn(async move |cx| {
        let fonts = task.await;
        cx.update(|cx| {
            if install(fonts, cx) {
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

static NAME_STYLES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_name_styles(on: bool) -> bool {
    NAME_STYLES.swap(on, std::sync::atomic::Ordering::Relaxed) != on
}

pub fn name_style(user: Option<&crate::model::User>) -> Option<&(u32, u32, Vec<u32>)> {
    if !NAME_STYLES.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    user.and_then(|u| u.name_style.as_ref())
}

pub fn styled_name<E: gpui::Styled>(el: E, user: Option<&crate::model::User>, in_profile: bool) -> E {
    let Some((font, effect, colors)) = name_style(user) else { return el };
    let el = match family_for_id(*font) {
        Some(f) => el.font_family(f),
        None => el,
    };
    match colors.iter().find(|&&c| c <= 0xffffff) {
        Some(&rgb) if in_profile => el.text_color(gpui::rgb(dns_color(rgb, *effect))),
        _ => el,
    }
}

pub fn name_text(text: gpui::SharedString, user: Option<&crate::model::User>, size: f32) -> gpui::AnyElement {
    let fill = name_style(user).and_then(|(_, effect, colors)| dns_fill(*effect, colors, &text));
    paint_name(text, user, size, fill)
}

pub fn role_name_text(
    text: gpui::SharedString,
    user: Option<&crate::model::User>,
    size: f32,
    in_guild: bool,
    gradient: Option<[u32; 3]>,
    show_effect: bool,
) -> gpui::AnyElement {
    if in_guild {
        name_text_with(text, user, size, gradient)
    } else if show_effect {
        name_text(text, user, size)
    } else {
        paint_name(text, user, size, None)
    }
}

pub fn name_text_with(
    text: gpui::SharedString,
    user: Option<&crate::model::User>,
    size: f32,
    stops: Option<[u32; 3]>,
) -> gpui::AnyElement {
    let dim = crate::theme::is_light();
    let fill = stops.map(|s| role_fill(s.map(|c| if dim { dim_gradient_stop(c) } else { c }), crate::theme::z(100.)));
    paint_name(text, user, size, fill)
}

pub fn role_fill(colors: [u32; 3], period: gpui::Pixels) -> name_paint::NameFill {
    let [a, b, c] = colors;
    let stops = if c == a { vec![(a, 0.), (b, 0.5), (a, 1.)] } else { vec![(a, 0.), (b, 1. / 3.), (c, 2. / 3.), (a, 1.)] };
    name_paint::NameFill::Role { stops, period }
}

fn dim_gradient_stop(c: u32) -> u32 {
    let ch = |shift: u32| ((((c >> shift) & 0xff) as f32 * 0.85).round() as u32).min(255) << shift;
    ch(16) | ch(8) | ch(0)
}

fn dns_min_contrast(effect: u32) -> f64 {
    match effect {
        2 | 6 | 7 => 2.5,
        _ => 3.,
    }
}

pub fn dns_color(fg: u32, effect: u32) -> u32 {
    let bg = if effect == 4 { 0x333333 } else { crate::theme::base_low_rgb() };
    contrast_adjust(fg, bg, dns_min_contrast(effect), crate::theme::user_color_saturation() as f64)
}

type Rgb = [f64; 3];

fn split_rgb(c: u32) -> Rgb {
    [((c >> 16) & 0xff) as f64, ((c >> 8) & 0xff) as f64, (c & 0xff) as f64]
}

fn rgb_to_hsl([r, g, b]: Rgb) -> [f64; 3] {
    let (r, g, b) = (r / 255., g / 255., b / 255.);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.;
    if max == min {
        return [f64::NAN, 0., l];
    }
    let d = max - min;
    let s = if l < 0.5 { d / (max + min) } else { d / (2. - max - min) };
    let sector = if r == max {
        (g - b) / d
    } else if g == max {
        2. + (b - r) / d
    } else {
        4. + (r - g) / d
    };
    let h = sector * 60.;
    [if h < 0. { h + 360. } else { h }, s, l]
}

fn hsl_to_rgb([h, s, l]: [f64; 3]) -> Rgb {
    if s == 0. {
        return [l * 255.; 3];
    }
    let t2 = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
    let t1 = 2. * l - t2;
    let h = h / 360.;
    [h + 1. / 3., h, h - 1. / 3.].map(|mut t| {
        if t < 0. {
            t += 1.;
        }
        if t > 1. {
            t -= 1.;
        }
        let c = if 6. * t < 1. {
            t1 + (t2 - t1) * 6. * t
        } else if 2. * t < 1. {
            t2
        } else if 3. * t < 2. {
            t1 + (t2 - t1) * (2. / 3. - t) * 6.
        } else {
            t1
        };
        (c * 255.).round()
    })
}

fn luminance(c: Rgb) -> f64 {
    let lin = |x: f64| {
        let x = x.clamp(0., 255.) / 255.;
        if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn with_hsl(c: Rgb, i: usize, v: f64) -> Rgb {
    let mut hsl = rgb_to_hsl(c);
    hsl[i] = v;
    hsl_to_rgb(hsl)
}

fn contrast_adjust(fg: u32, bg: u32, ratio: f64, saturation: f64) -> u32 {
    let (mut t, mut n) = (split_rgb(fg), split_rgb(bg));
    if saturation < 1. {
        t = with_hsl(t, 1, rgb_to_hsl(t)[1] * saturation);
        n = with_hsl(n, 1, rgb_to_hsl(n)[1] * saturation);
    }
    let dark = luminance(n) <= 0.5;
    let mut l = contrast(t, n);
    for _ in 0..10 {
        if l >= ratio {
            break;
        }
        let light = rgb_to_hsl(t)[2];
        if dark && light < 0.95 {
            t = with_hsl(t, 2, light + 0.05);
        } else if !dark && light > 0.05 {
            t = with_hsl(t, 2, light - 0.05);
        } else {
            break;
        }
        l = contrast(t, n);
    }
    let ch = |x: f64| x.clamp(0., 255.).round() as u32;
    (ch(t[0]) << 16) | (ch(t[1]) << 8) | ch(t[2])
}

pub fn dns_fill(effect: u32, colors: &[u32], name: &str) -> Option<name_paint::NameFill> {
    use name_paint::NameFill;
    let adjusted: Vec<u32> = colors.iter().filter(|&&c| c <= 0xffffff).map(|&c| dns_color(c, effect)).collect();
    let n = adjusted.len();
    match (effect, adjusted.as_slice()) {
        (_, []) => None,
        (2 | 6, [_, _, ..]) => {
            Some(NameFill::Diagonal(adjusted.iter().enumerate().map(|(i, &c)| (c, (10. + 80. * i as f32 / (n - 1) as f32) / 100.)).collect()))
        }
        (7, [_, _, ..]) => Some(NameFill::Prism {
            stops: (0..=n).map(|i| (adjusted[i % n], i as f32 / n as f32)).collect(),
            chars: name.encode_utf16().count(),
            min: crate::theme::z(38.),
        }),
        (_, [c, ..]) => Some(NameFill::Solid(*c)),
    }
}

fn paint_name(
    text: gpui::SharedString,
    user: Option<&crate::model::User>,
    size: f32,
    fill: Option<name_paint::NameFill>,
) -> gpui::AnyElement {
    use gpui::IntoElement;
    let ls = name_style(user)
        .filter(|s| family_for_id(s.0).is_some())
        .and_then(|s| font_for_id(s.0))
        .map_or(0., |(_, ls)| ls);
    if ls <= 0. && fill.is_none() {
        return crate::ui::emoji_text::label(text, size);
    }
    name_paint::NameText::new(text, crate::theme::z(ls * size), fill).into_any_element()
}

pub mod fallback;
pub mod name_paint;
pub use fallback::ui_fallbacks;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_theme_role_gradient_stops_are_dimmed_like_the_client_filter() {
        assert_eq!(dim_gradient_stop(0xffffff), 0xd9d9d9);
        assert_eq!(dim_gradient_stop(0x369876), 0x2e8164);
        assert_eq!(dim_gradient_stop(0), 0);
    }

    #[test]
    fn dark_theme_lifts_a_black_gradient_stop_like_the_client() {
        assert_eq!(contrast_adjust(0x000000, 0x202024, 2.5, 1.), 0x666666);
        assert_eq!(contrast_adjust(0xfffffe, 0x202024, 2.5, 1.), 0xfffffe);
        assert_eq!(contrast_adjust(0x000000, 0x202024, 3., 1.), 0x737373);
        assert_eq!(contrast_adjust(0x000000, 0x36373e, 2.5, 1.), 0x7f7f7f);
        assert_eq!(contrast_adjust(0x3498db, 0x202024, 3., 1.), 0x3498db);
    }

    #[test]
    fn light_theme_darkens_a_white_stop_with_the_client_hsl_steps() {
        assert_eq!(contrast_adjust(0xfffffe, 0xfbfbfb, 2.5, 1.), 0xfefe00);
        assert_eq!(contrast_adjust(0x000000, 0xfbfbfb, 2.5, 1.), 0x000000);
    }

    #[test]
    fn contrast_steps_stop_after_ten_tries() {
        assert_eq!(contrast_adjust(0x000000, 0x000000, 21., 1.), 0x7f7f7f);
    }

    #[test]
    fn black_to_white_display_name_gradient_in_the_dark_theme() {
        use name_paint::NameFill;
        if crate::theme::active() != crate::ui::settings::AppTheme::Dark || crate::theme::high_contrast() {
            return;
        }
        assert_eq!(dns_fill(2, &[0x000000, 0xfffffe], "Clem"), Some(NameFill::Diagonal(vec![(0x666666, 0.1), (0xfffffe, 0.9)])));
        assert_eq!(dns_fill(6, &[0x000000, 0xfffffe], "Clem"), Some(NameFill::Diagonal(vec![(0x666666, 0.1), (0xfffffe, 0.9)])));
        let three = dns_fill(2, &[0x3498db, 0x9b59b6, 0xe91e63], "x");
        assert_eq!(three, Some(NameFill::Diagonal(vec![(0x3498db, 0.1), (0x9b59b6, 0.5), (0xe91e63, 0.9)])));
        assert_eq!(dns_fill(1, &[0x000000], "Clem"), Some(NameFill::Solid(0x737373)));
        assert_eq!(dns_fill(2, &[0x000000], "Clem"), Some(NameFill::Solid(0x666666)));
        assert!(dns_fill(2, &[], "Clem").is_none());
        assert!(dns_fill(2, &[0x1000000], "Clem").is_none());
    }

    #[test]
    fn prism_stops_cycle_back_to_the_first_colour_over_the_name_length() {
        use name_paint::NameFill;
        if crate::theme::active() != crate::ui::settings::AppTheme::Dark || crate::theme::high_contrast() {
            return;
        }
        let Some(NameFill::Prism { stops, chars, min }) = dns_fill(7, &[0x3498db, 0xfffffe], "\u{1d538}\u{1d56b}_TV") else {
            panic!("prism fill");
        };
        assert_eq!(stops, vec![(0x3498db, 0.), (0xfffffe, 0.5), (0x3498db, 1.)]);
        assert_eq!(chars, 7);
        assert_eq!(min, crate::theme::z(38.));
    }

    #[test]
    fn role_gradients_go_back_to_the_first_colour_like_the_client_classes() {
        use name_paint::NameFill;
        let two = role_fill([0x9e6bff, 0x9fc1ff, 0x9e6bff], gpui::px(100.));
        assert_eq!(two, NameFill::Role { stops: vec![(0x9e6bff, 0.), (0x9fc1ff, 0.5), (0x9e6bff, 1.)], period: gpui::px(100.) });
        let holo = role_fill([0xa9c9ff, 0xffbbec, 0xffc3a0], gpui::px(100.));
        assert_eq!(
            holo,
            NameFill::Role {
                stops: vec![(0xa9c9ff, 0.), (0xffbbec, 1. / 3.), (0xffc3a0, 2. / 3.), (0xa9c9ff, 1.)],
                period: gpui::px(100.)
            }
        );
    }

    #[test]
    fn parses_font_face() {
        let css = r#"@font-face{font-display:swap;font-family:Sakura;font-weight:400;src:url("/assets/ab12.woff2") format("woff2")}"#;
        assert_eq!(parse_font_faces(css), vec![("Sakura".into(), 400, "ab12.woff2".into())]);
        let css = r#"@font-face{font-display:swap;font-family:gg sans;font-style:italic;font-weight:600;src:url(/assets/cd.woff2) format("woff2")}"#;
        assert_eq!(parse_font_faces_styled(css), vec![("gg sans".into(), 600, true, "cd.woff2".into())]);
    }

    #[test]
    #[ignore]
    fn extract_local() {
        for p in init() {
            let ttf = to_ttf(&std::fs::read(&p).unwrap());
            println!("{p:?} -> {:?}", ttf.as_deref().and_then(ttf_family));
        }
    }

    #[test]
    fn font_ids() {
        assert_eq!(font_for_id(17).map(|f| f.0), Some("Pricedown"));
        assert!(font_for_id(11).is_none());
        for id in 1..=17 {
            if let Some((f, _)) = font_for_id(id) {
                assert!(NAME_STYLE_FAMILIES.contains(&f), "{f}");
            }
        }
    }

    const FR_EN: &str = "Paramètres Données Accessibilité àâäéèêëîïôöùûüÿçœæ ÀÂÄÉÈÊËÎÏÔÖÙÛÜŸÇŒÆ «\u{a0}»’‘“”…–—•·€°% 0123456789 The quick brown fox jumps over the lazy dog ABCDEFGHIJKLMNOPQRSTUVWXYZ ?!:;,.()[]{}@#&*+-=/\\'\"_~|<>$";

    fn weight_style(key: &str) -> Option<(f32, bool)> {
        let (_, w) = key.rsplit_once('-')?;
        let italic = w.ends_with('i');
        Some((w.trim_end_matches('i').parse().ok()?, italic))
    }

    fn font(family: &str, weight: f32, italic: bool, fallbacks: &gpui::FontFallbacks) -> gpui::Font {
        gpui::Font {
            family: family.to_owned().into(),
            features: gpui::FontFeatures::default(),
            fallbacks: Some(fallbacks.clone()),
            weight: gpui::FontWeight(weight),
            style: if italic { gpui::FontStyle::Italic } else { gpui::FontStyle::Normal },
        }
    }

    fn run(text: &str, font: gpui::Font) -> gpui::TextRun {
        gpui::TextRun { len: text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None }
    }

    #[test]
    fn ui_text_resolves_to_the_client_faces() {
        let Some(dir) = out_dir() else { return };
        let faces = load_decoded(&dir);
        let client: Vec<&Decoded> = faces
            .iter()
            .filter(|f| UI_FAMILIES.contains(&f.css.as_str()) || DISPLAY_FAMILIES.contains(&f.css.as_str()))
            .collect();
        if !client.iter().any(|f| f.key == "gg_sans-400") {
            return;
        }
        for key in ["gg_sans-400", "gg_sans-500", "gg_sans-600", "gg_sans-700"] {
            assert!(client.iter().any(|f| f.key == key), "{key} missing from {dir:?}");
        }
        for f in &client {
            let face = ttf_parser::Face::parse(&f.ttf, 0).expect("face");
            let missing: Vec<char> = FR_EN.chars().filter(|c| *c != ' ' && face.glyph_index(*c).is_none()).collect();
            assert!(missing.is_empty(), "{} lacks {missing:?}", f.key);
        }
        let web = std::fs::read(dir.join("fallback/Noto_Sans-400.ttf")).ok().and_then(|b| fallback::rename(&b, "Phx Noto Sans Web"));
        let platform = gpui_platform::current_platform(true).text_system();
        let ts = std::sync::Arc::new(gpui::TextSystem::new(platform));
        let mut blobs: Vec<std::borrow::Cow<'static, [u8]>> = client.iter().map(|f| std::borrow::Cow::Owned(f.ttf.clone())).collect();
        blobs.extend(web.clone().map(std::borrow::Cow::Owned));
        ts.add_fonts(blobs).expect("fonts");
        let wts = gpui::WindowTextSystem::new(ts.clone());
        let fallbacks = gpui::FontFallbacks::from_fonts(vec!["Phx Noto Sans Web".into(), "DejaVu Sans".into()]);
        let mut seen: Vec<(String, gpui::FontId)> = Vec::new();
        for f in &client {
            let (weight, italic) = weight_style(&f.key).expect("key");
            let font = font(&f.family, weight, italic, &fallbacks);
            let id = ts.resolve_font(&font);
            if let Some((other, _)) = seen.iter().find(|(k, i)| *i == id && k.rsplit_once('-').map(|x| x.0) == f.key.rsplit_once('-').map(|x| x.0)) {
                panic!("{} and {other} resolve to the same face", f.key);
            }
            seen.push((f.key.clone(), id));
            let line = wts.layout_line(FR_EN, gpui::px(16.), &[run(FR_EN, font.clone())], None);
            assert!(!line.runs.is_empty());
            for r in line.runs.iter() {
                assert_eq!(r.font_id, id, "{}: part of the FR/EN text left the face", f.key);
            }
        }
        if web.is_some() {
            let text = "1\u{202f}234";
            let gg = font("gg sans", 400., false, &fallbacks);
            let gg_id = ts.resolve_font(&gg);
            let noto_id = ts.resolve_font(&gpui::font("Phx Noto Sans Web"));
            let line = wts.layout_line(text, gpui::px(16.), &[run(text, gg)], None);
            let ids: Vec<gpui::FontId> = line.runs.iter().flat_map(|r| r.glyphs.iter().map(move |_| r.font_id)).collect();
            assert_eq!(ids, vec![gg_id, noto_id, gg_id, gg_id, gg_id], "U+202F must take the web Noto Sans of the CSS stack");
        }
    }

    #[test]
    fn display_stand_ins_follow_the_css_stacks() {
        assert_eq!(display_stand_in(MARKETING), ui_family());
        assert_eq!(display_stand_in(NITRO), fallback::noto_family());
        assert_eq!(display_stand_in(HEADLINE), fallback::noto_family());
    }

    #[test]
    fn face_files_keep_weight_and_style() {
        assert_eq!(face_file("gg sans", 800, true, "woff2"), "gg_sans-800i.woff2");
        assert_eq!(weight_style("ABC_Ginto_Discord_Nord-700i"), Some((700., true)));
        assert_eq!(weight_style("gg_sans-500"), Some((500., false)));
    }

    #[test]
    fn ui_text_never_names_a_font_directly() {
        fn walk(d: &Path, bad: &mut Vec<String>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if !p.ends_with("fonts") {
                        walk(&p, bad);
                    }
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                let src = std::fs::read_to_string(&p).unwrap_or_default();
                let src = src.split("#[cfg(test)]").next().unwrap_or("");
                for (i, l) in src.lines().enumerate() {
                    if l.contains(".font_family(\"") || l.contains("gpui::font(\"") {
                        bad.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
        let mut bad = Vec::new();
        walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut bad);
        assert!(bad.is_empty(), "use fonts::ui_family / mono_family / display_family: {bad:?}");
        let tip = include_str!("../ui/tooltip.rs");
        assert!(tip.contains("crate::fonts::ui_root(div())"), "window-level tooltips are laid out outside the app root");
        let popout = include_str!("../ui/call_popout.rs");
        assert!(popout.contains("crate::fonts::ui_root(div())"), "the call popout is its own window");
    }

    #[test]
    fn no_font_files_in_repo() {
        fn walk(d: &Path, bad: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                    continue;
                }
                if p.is_dir() {
                    walk(&p, bad);
                } else if p.extension().is_some_and(|x| ["woff", "woff2", "ttf", "otf"].iter().any(|e| x == *e)) {
                    bad.push(p);
                }
            }
        }
        let mut bad = Vec::new();
        walk(Path::new(env!("CARGO_MANIFEST_DIR")), &mut bad);
        assert!(bad.is_empty(), "font files must not be committed (extracted at runtime): {bad:?}");
    }
}
