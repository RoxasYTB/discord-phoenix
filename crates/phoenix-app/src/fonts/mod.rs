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
    if GG_MONO.load(Ordering::Relaxed) { *GG_MONO_NAME.read() } else { "DejaVu Sans Mono" }
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

pub fn role_name_text(
    text: gpui::SharedString,
    user: Option<&crate::model::User>,
    size: f32,
    in_guild: bool,
    gradient: Option<[u32; 3]>,
) -> gpui::AnyElement {
    if in_guild {
        name_text_with(text, user, size, gradient)
    } else {
        name_text(text, user, size)
    }
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
