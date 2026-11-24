use std::{path::PathBuf, sync::OnceLock};

const PREFIX: &str = "Phx ";
pub const EMOJI_FAMILY: &str = "Noto Color Emoji";
const STACK: [&str; 4] = ["Noto Sans", "Helvetica Neue", "Helvetica", "Arial"];
const RENAMED: usize = 12;

static CHAIN: OnceLock<gpui::FontFallbacks> = OnceLock::new();

pub fn ui_fallbacks() -> gpui::FontFallbacks {
    CHAIN.get_or_init(|| gpui::FontFallbacks::from_fonts(plain_chain())).clone()
}

fn fc(args: &[&str], format: &str) -> Vec<String> {
    let Ok(out) = std::process::Command::new("fc-match").args(args).arg(format!("--format={format}")).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

fn plain_chain() -> Vec<String> {
    let sorted = fc(&["-s", "sans-serif"], "%{family[0]}\n");
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

pub fn register(cx: &mut gpui::App) {
    if CHAIN.get().is_some() {
        return;
    }
    if std::env::var_os("PHX_NO_FONT_FALLBACK").is_some() {
        let _ = CHAIN.set(gpui::FontFallbacks::from_fonts(Vec::new()));
        return;
    }
    let mut chain = plain_chain();
    let mut blobs: Vec<std::borrow::Cow<'static, [u8]>> = Vec::new();
    let web: Vec<Vec<u8>> = web_dir()
        .and_then(|d| {
            let mut files: Vec<_> = std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).collect();
            files.sort();
            Some(files.into_iter().filter_map(|p| std::fs::read(p).ok()).collect())
        })
        .unwrap_or_default();
    let web_name = format!("{PREFIX}Noto Sans Web");
    let mut have_web = false;
    for ttf in web {
        if let Some(r) = rename(&ttf, &web_name) {
            blobs.push(r.into());
            have_web = true;
        }
    }
    if !have_web {
        download_web_noto();
    }
    for (i, fam) in chain.clone().into_iter().enumerate() {
        if i >= RENAMED + STACK.len() {
            break;
        }
        let new = format!("{PREFIX}{fam}");
        let mut ok = false;
        let mut seen = Vec::new();
        for style in ["Regular", "Bold"] {
            let Some(file) = fc(&[&format!("{fam}:style={style}")], "%{file}").into_iter().next() else { continue };
            if seen.contains(&file) || !(file.ends_with(".ttf") || file.ends_with(".otf")) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&file) else { continue };
            if super::ttf_family(&bytes).as_deref() != Some(fam.as_str()) {
                continue;
            }
            if let Some(r) = rename(&bytes, &new) {
                blobs.push(r.into());
                ok = true;
            }
            seen.push(file);
        }
        if ok {
            chain[i] = new;
        }
    }
    if let Some(file) = fc(&["emoji"], "%{file}").into_iter().next()
        && let Ok(bytes) = std::fs::read(&file)
    {
        blobs.push(bytes.into());
    }
    if have_web && let Some(i) = chain.iter().position(|f| f == "Phx Noto Sans" || f == "Noto Sans") {
        chain[i] = web_name;
    }
    if !blobs.is_empty() && cx.text_system().add_fonts(blobs).is_err() {
        chain = plain_chain();
    }
    let _ = CHAIN.set(gpui::FontFallbacks::from_fonts(chain));
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
    name.1 = rename_table(&name.1, family)?;

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

fn rename_table(t: &[u8], family: &str) -> Option<Vec<u8>> {
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
    let ps: Vec<u8> = family.replace(' ', "").encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
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
    #[test]
    fn rename_sets_family() {
        let Ok(b) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") else { return };
        let r = super::rename(&b, "Phx DejaVu Sans").expect("renamed");
        assert_eq!(crate::fonts::ttf_family(&r).as_deref(), Some("Phx DejaVu Sans"));
        let face = ttf_parser::Face::parse(&r, 0).expect("parses");
        assert!(face.glyph_index('★').is_some());
    }
}
