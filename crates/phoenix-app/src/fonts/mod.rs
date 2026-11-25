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

pub fn extract(cache: &Path, dir: &Path) -> Vec<PathBuf> {
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
    let mut written = Vec::new();
    let _ = std::fs::create_dir_all(dir);
    faces.sort();
    faces.dedup();
    for (family, weight, italic, asset) in faces {
        if !NAME_STYLE_FAMILIES.contains(&family.as_str()) && !UI_FAMILIES.contains(&family.as_str()) {
            continue;
        }
        let Some(b) = entries.get(&asset).and_then(|p| body(p)).filter(|b| is_font(b)) else { continue };
        let ext = asset.rsplit('.').next().unwrap_or("woff2");
        let out = dir.join(format!("{}-{weight}{}.{ext}", family.replace(' ', "_"), if italic { "i" } else { "" }));
        if std::fs::write(&out, &b).is_ok() {
            written.push(out);
        }
    }
    written
}

pub fn init() -> Vec<PathBuf> {
    let Some(dir) = out_dir() else { return Vec::new() };
    crate::icons::discord_caches().iter().flat_map(|c| extract(c, &dir)).collect()
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
    if GG_SANS.load(Ordering::Relaxed) { "gg sans" } else { "Noto Sans" }
}

pub fn mono_family() -> &'static str {
    if GG_MONO.load(Ordering::Relaxed) { *GG_MONO_NAME.read() } else { "DejaVu Sans Mono" }
}

fn load_decoded(dir: &Path) -> Vec<(String, String, Vec<u8>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "ttf") {
            continue;
        }
        let css = p.file_stem().and_then(|s| s.to_str()).and_then(|s| s.rsplit_once('-')).map(|(f, _)| f.replace('_', " "));
        let (Some(css), Ok(ttf)) = (css, std::fs::read(&p)) else { continue };
        let Some(fam) = ttf_family(&ttf) else { continue };
        out.push((css, fam, ttf));
    }
    out
}

fn extract_and_decode() -> Vec<(String, String, Vec<u8>)> {
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

fn install(fonts: Vec<(String, String, Vec<u8>)>, cx: &mut gpui::App) -> bool {
    let fonts: Vec<_> = if std::env::var_os("PHX_NO_GG_FONTS").is_some() {
        fonts.into_iter().filter(|(css, ..)| !UI_FAMILIES.contains(&css.as_str())).collect()
    } else {
        fonts
    };
    if fonts.is_empty() {
        return false;
    }
    let mut map = HashMap::new();
    let mut blobs = Vec::new();
    for (css, fam, ttf) in fonts {
        map.insert(css, gpui::SharedString::from(fam));
        blobs.push(std::borrow::Cow::Owned(ttf));
    }
    if cx.text_system().add_fonts(blobs).is_err() {
        return false;
    }
    GG_SANS.store(map.get("gg sans").is_some_and(|f| f == "gg sans"), Ordering::Relaxed);
    if let Some(m) = map.get("gg mono") {
        *GG_MONO_NAME.write() = Box::leak(m.to_string().into_boxed_str());
        GG_MONO.store(true, Ordering::Relaxed);
    }
    *FAMILIES.write() = Some(map);
    true
}

pub fn register(cx: &mut gpui::App) {
    fallback::register(cx);
    let cached = out_dir().map(|d| load_decoded(&d)).unwrap_or_default();
    if install(cached, cx) && GG_SANS.load(Ordering::Relaxed) {
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
    let Some((font, _effect, colors)) = name_style(user) else { return el };
    let el = match family_for_id(*font) {
        Some(f) => el.font_family(f),
        None => el,
    };
    match colors.first() {
        Some(&rgb) if in_profile => el.text_color(gpui::rgb(rgb)),
        _ => el,
    }
}

pub fn name_text(text: gpui::SharedString, user: Option<&crate::model::User>, size: f32) -> gpui::AnyElement {
    let stops = name_style(user).and_then(|(_, _, c)| match c.as_slice() {
        [a, b, ..] => Some([*a, *b, *a]),
        _ => None,
    });
    name_text_with(text, user, size, stops)
}

pub fn name_text_with(
    text: gpui::SharedString,
    user: Option<&crate::model::User>,
    size: f32,
    stops: Option<[u32; 3]>,
) -> gpui::AnyElement {
    use gpui::IntoElement;
    let ls = name_style(user)
        .filter(|s| family_for_id(s.0).is_some())
        .and_then(|s| font_for_id(s.0))
        .map_or(0., |(_, ls)| ls);
    let stops: Vec<gpui::Hsla> = stops.map(|s| s.iter().map(|&c| gpui::rgb(c).into()).collect()).unwrap_or_default();
    if ls <= 0. && stops.is_empty() {
        return crate::ui::emoji_text::label(text, size);
    }
    name_paint::NameText::new(text, crate::theme::z(ls * size), stops, Some(crate::theme::z(100.))).into_any_element()
}

pub mod fallback;
pub mod name_paint;
pub use fallback::ui_fallbacks;

#[cfg(test)]
mod tests {
    use super::*;

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
