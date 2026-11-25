use std::{path::PathBuf, sync::OnceLock};

const PREFIX: &str = "Phx ";
pub const EMOJI_FAMILY: &str = "Noto Color Emoji";
const STACK: [&str; 4] = ["Noto Sans", "Helvetica Neue", "Helvetica", "Arial"];
const RENAMED: usize = 24;
const SELECTOR_SUFFIX: &str = "Selector";

const EMOJI_PRESENTATION: [(u32, u32); 81] = [
    (0x231A, 0x231B), (0x23E9, 0x23EC), (0x23F0, 0x23F0), (0x23F3, 0x23F3), (0x25FD, 0x25FE), (0x2614, 0x2615),
    (0x2648, 0x2653), (0x267F, 0x267F), (0x2693, 0x2693), (0x26A1, 0x26A1), (0x26AA, 0x26AB), (0x26BD, 0x26BE),
    (0x26C4, 0x26C5), (0x26CE, 0x26CE), (0x26D4, 0x26D4), (0x26EA, 0x26EA), (0x26F2, 0x26F3), (0x26F5, 0x26F5),
    (0x26FA, 0x26FA), (0x26FD, 0x26FD), (0x2705, 0x2705), (0x270A, 0x270B), (0x2728, 0x2728), (0x274C, 0x274C),
    (0x274E, 0x274E), (0x2753, 0x2755), (0x2757, 0x2757), (0x2795, 0x2797), (0x27B0, 0x27B0), (0x27BF, 0x27BF),
    (0x2B1B, 0x2B1C), (0x2B50, 0x2B50), (0x2B55, 0x2B55), (0x1F004, 0x1F004), (0x1F0CF, 0x1F0CF), (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A), (0x1F1E6, 0x1F1FF), (0x1F201, 0x1F201), (0x1F21A, 0x1F21A), (0x1F22F, 0x1F22F), (0x1F232, 0x1F236),
    (0x1F238, 0x1F23A), (0x1F250, 0x1F251), (0x1F300, 0x1F320), (0x1F32D, 0x1F335), (0x1F337, 0x1F37C), (0x1F37E, 0x1F393),
    (0x1F3A0, 0x1F3CA), (0x1F3CF, 0x1F3D3), (0x1F3E0, 0x1F3F0), (0x1F3F4, 0x1F3F4), (0x1F3F8, 0x1F43E), (0x1F440, 0x1F440),
    (0x1F442, 0x1F4FC), (0x1F4FF, 0x1F53D), (0x1F54B, 0x1F54E), (0x1F550, 0x1F567), (0x1F57A, 0x1F57A), (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A4), (0x1F5FB, 0x1F64F), (0x1F680, 0x1F6C5), (0x1F6CC, 0x1F6CC), (0x1F6D0, 0x1F6D2), (0x1F6D5, 0x1F6D8),
    (0x1F6DC, 0x1F6DF), (0x1F6EB, 0x1F6EC), (0x1F6F4, 0x1F6FC), (0x1F7E0, 0x1F7EB), (0x1F7F0, 0x1F7F0), (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945), (0x1F947, 0x1F9FF), (0x1FA70, 0x1FA7C), (0x1FA80, 0x1FA8A), (0x1FA8E, 0x1FAC6), (0x1FAC8, 0x1FAC8),
    (0x1FACD, 0x1FADC), (0x1FADF, 0x1FAEA), (0x1FAEF, 0x1FAF8),
];
const EMOJI_SEQUENCE_PARTS: [(u32, u32); 7] = [
    (0x23, 0x23), (0x2A, 0x2A), (0x30, 0x39), (0x200D, 0x200D), (0x20E3, 0x20E3), (0xFE0F, 0xFE0F), (0xE0020, 0xE007F),
];

static REGISTERED: OnceLock<Vec<String>> = OnceLock::new();
static WEB_NOTO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const WEB_NOTO_NAME: &str = "Phx Noto Sans Web";

pub fn noto_family() -> &'static str {
    if WEB_NOTO.load(std::sync::atomic::Ordering::Relaxed) { WEB_NOTO_NAME } else { "Noto Sans" }
}
static CHAINS: OnceLock<parking_lot::RwLock<std::collections::HashMap<&'static str, gpui::FontFallbacks>>> = OnceLock::new();

pub fn ui_fallbacks() -> gpui::FontFallbacks {
    let loc = crate::i18n::locale();
    let cache = CHAINS.get_or_init(|| parking_lot::RwLock::new(std::collections::HashMap::new()));
    if let Some(fb) = cache.read().get(loc) {
        return fb.clone();
    }
    let mut chain = if let Some(base) = REGISTERED.get() {
        let mut out = Vec::new();
        for name in preferred_script_fonts(loc) {
            if let Some(f) = fc(&[name], "%{family[0]}\n").into_iter().next()
                && f == *name
                && !out.contains(&f)
            {
                out.push(f);
            }
        }
        for f in base {
            if !out.contains(f) {
                out.push(f.clone());
            }
        }
        out
    } else {
        plain_chain_for(loc)
    };
    if chain.is_empty() {
        chain = plain_chain_for(loc);
    }
    let fb = gpui::FontFallbacks::from_fonts(chain);
    cache.write().insert(loc, fb.clone());
    fb
}

const NATIVE_FALLBACK: bool = cfg!(any(windows, target_os = "macos"));

fn fc(args: &[&str], format: &str) -> Vec<String> {
    if NATIVE_FALLBACK || crate::paths::is_sandboxed() {
        return Vec::new();
    }
    let Ok(out) = std::process::Command::new("fc-match").args(args).arg(format!("--format={format}")).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

fn preferred_script_fonts(locale: &str) -> &'static [&'static str] {
    match locale {
        "ja" => &["Noto Sans CJK JP", "Hiragino Sans", "Yu Gothic"],
        "ko" => &["Noto Sans CJK KR", "Apple SD Gothic Neo", "Malgun Gothic"],
        "zh-CN" => &["Noto Sans CJK SC", "PingFang SC", "Microsoft YaHei"],
        "zh-TW" => &["Noto Sans CJK TC", "PingFang TC", "Microsoft JhengHei"],
        "th" => &["Noto Sans Thai", "Thonburi", "Leelawadee UI"],
        "hi" => &["Noto Sans Devanagari", "Kohinoor Devanagari", "Nirmala UI"],
        _ => &[],
    }
}

fn plain_chain() -> Vec<String> {
    plain_chain_for(crate::i18n::locale())
}

fn plain_chain_for(locale: &str) -> Vec<String> {
    let query = format!("sans-serif:lang={locale}");
    let sorted = fc(&["-s", &query], "%{family[0]}\n");
    let mut chain: Vec<String> = Vec::new();
    for name in STACK {
        let metric_ok = |f: &str| f == name || (name == "Arial" && matches!(f, "Arimo" | "Liberation Sans"));
        if let Some(f) = fc(&[name], "%{family[0]}\n").into_iter().next()
            && metric_ok(&f)
            && !chain.contains(&f)
        {
            chain.push(f);
        }
    }
    for name in preferred_script_fonts(locale) {
        if NATIVE_FALLBACK {
            chain.push((*name).to_owned());
        } else if let Some(f) = fc(&[name], "%{family[0]}\n").into_iter().next()
            && f == *name
            && !chain.contains(&f)
        {
            chain.push(f);
        }
    }
    for f in sorted.into_iter().take(40) {
        if !chain.contains(&f) {
            chain.push(f);
        }
    }
    if chain.is_empty() {
        chain = ["Noto Sans", "DejaVu Sans", "Noto Sans CJK JP", "FreeSerif"].map(String::from).to_vec();
    }
    chain
}

fn web_dir() -> Option<PathBuf> {
    Some(super::out_dir()?.join("fallback"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortedFace {
    pub family: String,
    pub style: String,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub family: String,
    pub style: Option<String>,
    pub files: Vec<String>,
}

impl Entry {
    fn name(&self) -> String {
        match &self.style {
            Some(st) => format!("{PREFIX}{} {st}", self.family),
            None => format!("{PREFIX}{}", self.family),
        }
    }
}

fn sorted_faces() -> Vec<SortedFace> {
    fc(&["-s", "sans-serif"], "%{family[0]}\t%{style[0]}\t%{file}\n")
        .into_iter()
        .filter_map(|l| {
            let mut it = l.split('\t');
            Some(SortedFace { family: it.next()?.to_string(), style: it.next()?.to_string(), file: it.next()?.to_string() })
        })
        .take(SORTED_FACES)
        .collect()
}

const SORTED_FACES: usize = 48;

pub fn plan(
    stack: &[(String, String)],
    sorted: &[SortedFace],
    bold_of: impl Fn(&str, &str) -> Option<String>,
    bold_covers_regular: impl Fn(&str, &str) -> bool,
) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let family_entry = |family: &str, file: &str| {
        let mut files = vec![file.to_string()];
        if let Some(bold) = bold_of(family, file)
            && bold != file
            && bold_covers_regular(file, &bold)
        {
            files.push(bold);
        }
        Entry { family: family.to_string(), style: None, files }
    };
    for (family, file) in stack {
        if !out.iter().any(|e| &e.family == family) {
            out.push(family_entry(family, file));
        }
    }
    for f in sorted {
        if out.iter().any(|e| e.files.contains(&f.file)) {
            continue;
        }
        if out.iter().any(|e| e.family == f.family) {
            out.push(Entry { family: f.family.clone(), style: Some(f.style.clone()), files: vec![f.file.clone()] });
        } else {
            out.push(family_entry(&f.family, &f.file));
        }
    }
    out
}

pub fn codepoints(font: &[u8]) -> Option<std::collections::HashSet<u32>> {
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let cmap = face.tables().cmap?;
    let mut set = std::collections::HashSet::new();
    for sub in cmap.subtables.into_iter().filter(|s| s.is_unicode()) {
        sub.codepoints(|c| {
            if sub.glyph_index(c).is_some_and(|g| g.0 != 0) {
                set.insert(c);
            }
        });
    }
    Some(set)
}

fn bold_file(family: &str, _regular: &str) -> Option<String> {
    let line = fc(&[&format!("{family}:weight=bold:slant=roman")], "%{family[0]}\t%{weight}\t%{file}").into_iter().next()?;
    let mut it = line.split('\t');
    let (fam, weight, file) = (it.next()?, it.next()?.parse::<f32>().ok()?, it.next()?);
    (fam == family && weight >= 180.).then(|| file.to_string())
}

fn bold_covers(regular: &str, bold: &str) -> bool {
    if ![regular, bold].iter().all(|f| f.ends_with(".ttf") || f.ends_with(".otf")) {
        return false;
    }
    let (Ok(r), Ok(b)) = (std::fs::read(regular), std::fs::read(bold)) else { return false };
    match (codepoints(&r), codepoints(&b)) {
        (Some(r), Some(b)) => r.is_subset(&b),
        _ => false,
    }
}

fn stack_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for name in STACK {
        let metric_ok = |f: &str| f == name || (name == "Arial" && matches!(f, "Arimo" | "Liberation Sans"));
        let Some(line) = fc(&[&format!("{name}:weight=regular:slant=roman")], "%{family[0]}\t%{file}").into_iter().next() else { continue };
        let Some((fam, file)) = line.split_once('\t') else { continue };
        if metric_ok(fam) {
            out.push((fam.to_string(), file.to_string()));
        }
    }
    out
}

pub fn emoji_presentation(cp: u32) -> bool {
    EMOJI_PRESENTATION.iter().any(|&(a, b)| (a..=b).contains(&cp))
}

fn emoji_keeps(cp: u32) -> bool {
    emoji_presentation(cp) || EMOJI_SEQUENCE_PARTS.iter().any(|&(a, b)| (a..=b).contains(&cp))
}

pub fn cmap_format12(map: &[(u32, u16)]) -> Vec<u8> {
    let mut groups: Vec<(u32, u32, u32)> = Vec::new();
    for &(cp, gid) in map {
        let gid = gid as u32;
        let extends = groups.last().is_some_and(|&(a, b, g)| b + 1 == cp && g + (cp - a) == gid);
        if !extends {
            groups.push((cp, cp, gid));
        } else if let Some(last) = groups.last_mut() {
            last.1 = cp;
        }
    }
    let mut out = Vec::new();
    for v in [0u16, 1, 3, 10] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&12u32.to_be_bytes());
    out.extend_from_slice(&12u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(16 + 12 * groups.len() as u32).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&(groups.len() as u32).to_be_bytes());
    for (a, b, g) in groups {
        for v in [a, b, g] {
            out.extend_from_slice(&v.to_be_bytes());
        }
    }
    out
}

fn unicode_map(face: &ttf_parser::Face) -> Option<std::collections::BTreeMap<u32, u16>> {
    let cmap = face.tables().cmap?;
    let mut map = std::collections::BTreeMap::new();
    for sub in cmap.subtables.into_iter().filter(|s| s.is_unicode()) {
        sub.codepoints(|c| {
            if let Some(g) = sub.glyph_index(c).filter(|g| g.0 != 0) {
                map.entry(c).or_insert(g.0);
            }
        });
    }
    Some(map)
}

fn keep_loadable(map: &mut std::collections::BTreeMap<u32, u16>, space: Option<u16>) {
    if let Some(g) = space {
        map.entry('m' as u32).or_insert(g);
    }
}

pub fn text_face(font: &[u8], family: &str) -> Option<Vec<u8>> {
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let mut map = unicode_map(&face)?;
    if map.contains_key(&('m' as u32)) {
        return rename(font, family);
    }
    let space = map.get(&0x20).copied();
    keep_loadable(&mut map, space);
    let kept: Vec<(u32, u16)> = map.into_iter().collect();
    rebuild(font, family, None, Some(cmap_format12(&kept)))
}

pub fn emoji_presentation_font(font: &[u8], family: &str) -> Option<Vec<u8>> {
    emoji_copy(font, family, emoji_keeps)
}

pub fn emoji_selector_font(font: &[u8], family: &str) -> Option<Vec<u8>> {
    emoji_copy(font, family, |c| c > 0x7F || EMOJI_SEQUENCE_PARTS.iter().any(|&(a, b)| (a..=b).contains(&c)))
}

fn emoji_copy(font: &[u8], family: &str, keep: impl Fn(u32) -> bool) -> Option<Vec<u8>> {
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let mut map = unicode_map(&face)?;
    let space = map.get(&0x20).copied();
    map.retain(|&c, _| keep(c));
    if map.is_empty() {
        return None;
    }
    let ps = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == 6 && n.is_unicode())
        .find_map(|n| n.to_string())
        .unwrap_or_else(|| family.replace(' ', ""));
    keep_loadable(&mut map, space);
    let map: Vec<(u32, u16)> = map.into_iter().collect();
    rebuild(font, family, Some(&ps), Some(cmap_format12(&map)))
}

pub fn register(cx: &mut gpui::App) {
    if REGISTERED.get().is_some() {
        return;
    }
    if std::env::var_os("PHX_NO_FONT_FALLBACK").is_some() {
        let _ = REGISTERED.set(Vec::new());
        return;
    }
    let mut blobs: Vec<std::borrow::Cow<'static, [u8]>> = Vec::new();
    let web: Vec<Vec<u8>> = web_dir()
        .and_then(|d| {
            let mut files: Vec<_> = std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).collect();
            files.sort();
            Some(files.into_iter().filter_map(|p| std::fs::read(p).ok()).collect())
        })
        .unwrap_or_default();
    let web_name = WEB_NOTO_NAME.to_owned();
    let mut have_web = false;
    for ttf in web {
        if let Some(r) = rename(&ttf, &web_name) {
            blobs.push(r.into());
            have_web = true;
        }
    }
    if have_web {
        WEB_NOTO.store(true, std::sync::atomic::Ordering::Relaxed);
    } else {
        download_web_noto();
    }
    let stack = stack_files();
    let entries = plan(&stack, &sorted_faces(), bold_file, bold_covers);
    let mut chain: Vec<String> = Vec::new();
    let mut after_stack = 0;
    for (i, e) in entries.iter().enumerate() {
        if i == stack.len() {
            after_stack = chain.len();
        }
        if have_web && e.family == "Noto Sans" && e.style.is_none() {
            chain.push(web_name.clone());
        }
        let mut renamed = Vec::new();
        if i < RENAMED {
            let name = e.name();
            for file in &e.files {
                if !(file.ends_with(".ttf") || file.ends_with(".otf")) {
                    break;
                }
                let Ok(bytes) = std::fs::read(file) else { break };
                if super::ttf_family(&bytes).as_deref() != Some(e.family.as_str()) {
                    break;
                }
                let Some(r) = text_face(&bytes, &name) else { break };
                renamed.push(r);
            }
        }
        if !renamed.is_empty() {
            blobs.extend(renamed.into_iter().map(Into::into));
            chain.push(e.name());
        } else if e.style.is_none() && !chain.contains(&e.family) {
            chain.push(e.family.clone());
        }
    }
    if entries.len() <= stack.len() {
        after_stack = chain.len();
    }
    if let Some(file) = fc(&["emoji"], "%{file}").into_iter().next()
        && let Ok(bytes) = std::fs::read(&file)
    {
        let name = format!("{PREFIX}{EMOJI_FAMILY}");
        if let Some(only) = emoji_presentation_font(&bytes, &name) {
            blobs.push(only.into());
            chain.insert(after_stack, name);
        }
        let selector = format!("{PREFIX}{EMOJI_FAMILY} {SELECTOR_SUFFIX}");
        if let Some(all) = emoji_selector_font(&bytes, &selector) {
            blobs.push(all.into());
            chain.push(selector);
        }
        blobs.push(bytes.into());
    }
    if chain.is_empty() || (!blobs.is_empty() && cx.text_system().add_fonts(blobs).is_err()) {
        chain = plain_chain();
    }
    let _ = REGISTERED.set(chain);
    if let Some(cache) = CHAINS.get() {
        cache.write().clear();
    }
}

fn download_web_noto() {
    let Some(dir) = web_dir() else { return };
    let mut assets = Vec::new();
    for cache in crate::icons::discord_caches() {
        for e in std::fs::read_dir(cache).into_iter().flatten().flatten() {
            let p = e.path();
            let mut head = [0u8; 1024];
            let n = std::fs::File::open(&p).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
            if !super::asset_key(&head[..n]).is_some_and(|k| k.ends_with(".css")) {
                continue;
            }
            let Some(css) = super::body(&p).and_then(|b| String::from_utf8(b).ok()) else { continue };
            for (family, weight, italic, asset) in super::parse_font_faces_styled(&css) {
                if family == "Noto Sans" && !italic && matches!(weight, 400 | 500 | 600 | 700) {
                    assets.push((weight, asset));
                }
            }
        }
    }
    assets.sort();
    assets.dedup_by_key(|a| a.0);
    if assets.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
        rt.block_on(async {
            let client = reqwest::Client::new();
            let _ = std::fs::create_dir_all(&dir);
            for (weight, asset) in assets {
                let url = format!("https://discord.com/assets/{}", asset.trim_start_matches("/assets/"));
                let Ok(r) = client.get(url).send().await else { continue };
                let Ok(b) = r.bytes().await else { continue };
                if let Some(ttf) = super::to_ttf(&b) {
                    let _ = std::fs::write(dir.join(format!("Noto_Sans-{weight}.ttf")), ttf);
                }
            }
        });
    });
}

pub fn rename(font: &[u8], family: &str) -> Option<Vec<u8>> {
    rebuild(font, family, None, None)
}

fn rebuild(font: &[u8], family: &str, ps: Option<&str>, cmap: Option<Vec<u8>>) -> Option<Vec<u8>> {
    let be16 = |o: usize| font.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let be32 = |o: usize| font.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let sig = be32(0)?;
    if sig != 0x0001_0000 && sig != u32::from_be_bytes(*b"OTTO") {
        return None;
    }
    let n = be16(4)? as usize;
    let mut tables = Vec::with_capacity(n);
    for i in 0..n {
        let r = 12 + i * 16;
        let tag = font.get(r..r + 4)?.to_vec();
        let (off, len) = (be32(r + 8)? as usize, be32(r + 12)? as usize);
        tables.push((tag, font.get(off..off + len)?.to_vec()));
    }
    let name = tables.iter_mut().find(|(t, _)| t == b"name")?;
    name.1 = rename_table(&name.1, family, ps)?;
    if let Some(cmap) = cmap {
        tables.iter_mut().find(|(t, _)| t == b"cmap")?.1 = cmap;
    }

    let mut out = Vec::new();
    out.extend_from_slice(&font[..12]);
    let mut offset = 12 + n * 16;
    let mut dir = Vec::new();
    let mut data = Vec::new();
    for (tag, body) in &tables {
        let sum = body.chunks(4).fold(0u32, |s, c| {
            let mut w = [0u8; 4];
            w[..c.len()].copy_from_slice(c);
            s.wrapping_add(u32::from_be_bytes(w))
        });
        dir.extend_from_slice(tag);
        dir.extend_from_slice(&sum.to_be_bytes());
        dir.extend_from_slice(&(offset as u32).to_be_bytes());
        dir.extend_from_slice(&(body.len() as u32).to_be_bytes());
        data.extend_from_slice(body);
        let pad = (4 - body.len() % 4) % 4;
        data.extend(std::iter::repeat_n(0, pad));
        offset += body.len() + pad;
    }
    out.extend(dir);
    out.extend(data);
    Some(out)
}

fn rename_table(t: &[u8], family: &str, ps: Option<&str>) -> Option<Vec<u8>> {
    let be16 = |o: usize| t.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let count = be16(2)? as usize;
    let storage = be16(4)? as usize;
    let mut recs: Vec<(u16, u16, u16, u16, Vec<u8>)> = Vec::new();
    for i in 0..count {
        let r = 6 + i * 12;
        let (pid, eid, lid, nid) = (be16(r)?, be16(r + 2)?, be16(r + 4)?, be16(r + 6)?);
        let (len, off) = (be16(r + 8)? as usize, be16(r + 10)? as usize);
        if matches!(nid, 1 | 3 | 4 | 6 | 16 | 17 | 18 | 21 | 22 | 25) {
            continue;
        }
        recs.push((pid, eid, lid, nid, t.get(storage + off..storage + off + len)?.to_vec()));
    }
    let utf16: Vec<u8> = family.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    let ps: Vec<u8> = ps.map(str::to_string).unwrap_or_else(|| family.replace(' ', "")).encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    for nid in [1u16, 4] {
        recs.push((3, 1, 0x409, nid, utf16.clone()));
    }
    recs.push((3, 1, 0x409, 6, ps));
    recs.sort_by_key(|r| (r.0, r.1, r.2, r.3));
    let mut out = Vec::new();
    let header = 6 + recs.len() * 12;
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(recs.len() as u16).to_be_bytes());
    out.extend_from_slice(&(header as u16).to_be_bytes());
    let mut strings = Vec::new();
    for (pid, eid, lid, nid, s) in &recs {
        for v in [*pid, *eid, *lid, *nid, s.len() as u16, strings.len() as u16] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        strings.extend_from_slice(s);
    }
    out.extend(strings);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{Entry, SortedFace, plan};

    fn face(family: &str, style: &str, file: &str) -> SortedFace {
        SortedFace { family: family.into(), style: style.into(), file: file.into() }
    }

    #[test]
    fn plan_keeps_every_sorted_face_and_pairs_only_superset_bold() {
        let stack = vec![("Noto Sans".to_string(), "noto-r".to_string())];
        let sorted = vec![
            face("Noto Sans", "Regular", "noto-r"),
            face("DejaVu Sans", "Book", "dv-r"),
            face("DejaVu Sans", "Bold", "dv-b"),
            face("DejaVu Sans", "Oblique", "dv-i"),
            face("FreeSans", "Regular", "fs-r"),
            face("FreeSerif", "Regular", "fsf-r"),
        ];
        let bold = |fam: &str, _: &str| match fam {
            "Noto Sans" => Some("noto-b".to_string()),
            "DejaVu Sans" => Some("dv-b".to_string()),
            "FreeSans" => Some("fs-b".to_string()),
            _ => None,
        };
        let superset = |_: &str, b: &str| b == "noto-b";
        let e = |family: &str, style: Option<&str>, files: &[&str]| Entry {
            family: family.into(),
            style: style.map(Into::into),
            files: files.iter().map(|f| f.to_string()).collect(),
        };
        assert_eq!(
            plan(&stack, &sorted, bold, superset),
            vec![
                e("Noto Sans", None, &["noto-r", "noto-b"]),
                e("DejaVu Sans", None, &["dv-r"]),
                e("DejaVu Sans", Some("Bold"), &["dv-b"]),
                e("DejaVu Sans", Some("Oblique"), &["dv-i"]),
                e("FreeSans", None, &["fs-r"]),
                e("FreeSerif", None, &["fsf-r"]),
            ]
        );
        assert_eq!(plan(&stack, &sorted, bold, superset)[2].name(), "Phx DejaVu Sans Bold");
    }

    #[test]
    fn emoji_presentation_matches_unicode_data() {
        for cp in [0x1F50A, 0x1F338, 0x1F525, 0x26A1, 0x2728, 0x2B50, 0x231A, 0x1F1EB] {
            assert!(super::emoji_presentation(cp), "{cp:x}");
        }
        for cp in [0x2744, 0x2603, 0x2764, 0x2714, 0x25B6, 0x263A, 0x2665, 0x41] {
            assert!(!super::emoji_presentation(cp), "{cp:x}");
        }
    }

    #[test]
    fn format12_cmap_round_trips() {
        let Ok(b) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") else { return };
        let map = [(0x41u32, 36u16), (0x42, 37), (0x43, 38), (0x2744, 900)];
        let r = super::rebuild(&b, "Phx Probe", None, Some(super::cmap_format12(&map))).expect("rebuilt");
        let face = ttf_parser::Face::parse(&r, 0).expect("parses");
        assert_eq!(face.glyph_index('A').map(|g| g.0), Some(36));
        assert_eq!(face.glyph_index('C').map(|g| g.0), Some(38));
        assert_eq!(face.glyph_index('❄').map(|g| g.0), Some(900));
        assert!(face.glyph_index('D').is_none());
    }

    #[test]
    fn emoji_copy_keeps_only_emoji_presentation() {
        let Ok(b) = std::fs::read("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf") else { return };
        let r = super::emoji_presentation_font(&b, "Phx Noto Color Emoji").expect("built");
        let face = ttf_parser::Face::parse(&r, 0).expect("parses");
        assert!(face.glyph_index('🔊').is_some());
        assert!(face.glyph_index('\u{200D}').is_some());
        assert!(face.glyph_index('❄').is_none());
        assert!(face.glyph_index('☺').is_none());
        assert!(face.glyph_index('m').is_some());
        assert_eq!(crate::fonts::ttf_family(&r).as_deref(), Some("Phx Noto Color Emoji"));
        let ps = face.names().into_iter().find(|n| n.name_id == 6 && n.is_unicode()).and_then(|n| n.to_string());
        assert_eq!(ps.as_deref(), Some("NotoColorEmoji"));
    }

    #[test]
    fn emoji_selector_copy_keeps_text_default_emoji() {
        let Ok(b) = std::fs::read("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf") else { return };
        let r = super::emoji_selector_font(&b, "Phx Noto Color Emoji Selector").expect("built");
        let face = ttf_parser::Face::parse(&r, 0).expect("parses");
        for c in ['❄', '❤', '☃', '↔', '©', '🔊', '#', '1', '\u{20E3}'] {
            assert!(face.glyph_index(c).is_some(), "{c}");
        }
        assert!(face.glyph_index('m').is_some());
        assert!(face.glyph_index('a').is_none());
        let ps = face.names().into_iter().find(|n| n.name_id == 6 && n.is_unicode()).and_then(|n| n.to_string());
        assert_eq!(ps.as_deref(), Some("NotoColorEmoji"));
    }

    #[test]
    fn text_faces_keep_emoji_code_points_for_vs15() {
        let Ok(b) = std::fs::read("/usr/share/fonts/opentype/freefont/FreeSerif.otf") else { return };
        let r = super::text_face(&b, "Phx FreeSerif").expect("built");
        let face = ttf_parser::Face::parse(&r, 0).expect("parses");
        assert!(face.glyph_index('🔊').is_some());
        assert!(face.glyph_index('❄').is_some());
        assert!(face.glyph_index('𝓢').is_some());
        assert_eq!(crate::fonts::ttf_family(&r).as_deref(), Some("Phx FreeSerif"));
    }

    #[test]
    fn freesans_bold_lacks_math_bold_letters() {
        let (Ok(r), Ok(b)) = (
            std::fs::read("/usr/share/fonts/opentype/freefont/FreeSans.otf"),
            std::fs::read("/usr/share/fonts/opentype/freefont/FreeSansBold.otf"),
        ) else {
            return;
        };
        let (r, b) = (super::codepoints(&r).expect("cmap"), super::codepoints(&b).expect("cmap"));
        assert!(r.contains(&0x1D40D));
        assert!(!b.contains(&0x1D40D));
        assert!(!r.is_subset(&b));
    }
    #[test]
    fn rename_sets_family() {
        let Ok(b) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") else { return };
        let r = super::rename(&b, "Phx DejaVu Sans").expect("renamed");
        assert_eq!(crate::fonts::ttf_family(&r).as_deref(), Some("Phx DejaVu Sans"));
        let face = ttf_parser::Face::parse(&r, 0).expect("parses");
        assert!(face.glyph_index('★').is_some());
    }
}
