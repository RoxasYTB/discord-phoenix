use std::io::Read;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 8] = [0x30, 0x5c, 0x72, 0xa7, 0x1b, 0x6d, 0xfb, 0xfc];
const EOF_MAGIC: [u8; 8] = [0xd8, 0x41, 0x0d, 0x97, 0x45, 0x6f, 0xfa, 0xf4];

fn js_key(head: &[u8]) -> Option<String> {
    asset_key(head, ".js")
}

fn asset_key(head: &[u8], ext: &str) -> Option<String> {
    if head.len() < 24 || head[..8] != MAGIC {
        return None;
    }
    let kl = u32::from_le_bytes(head[12..16].try_into().ok()?) as usize;
    let key = std::str::from_utf8(head.get(24..24 + kl)?).ok()?;
    let name = key.rsplit_once("/assets/")?.1;
    let name = name.split_once('?').map_or(name, |(n, _)| n);
    (name.ends_with(ext) && !name.contains('/')).then(|| name.to_owned())
}

pub fn asset_entries(cache: &Path, ext: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(cache) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let mut head = [0u8; 1024];
        let Ok(mut f) = std::fs::File::open(&p) else { continue };
        let n = f.read(&mut head).unwrap_or(0);
        if let Some(k) = asset_key(&head[..n], ext) {
            out.push((k, p));
        }
    }
    out
}

pub fn js_entries(cache: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(cache) else { return Vec::new() };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let mut head = [0u8; 1024];
        let Ok(mut f) = std::fs::File::open(&p) else { continue };
        let n = f.read(&mut head).unwrap_or(0);
        if let Some(k) = js_key(&head[..n]) {
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((len, k, p));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    out.into_iter().map(|(_, k, p)| (k, p)).collect()
}

fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    memchr::memmem::find(h.get(from..)?, n).map(|p| p + from)
}

pub fn read_body(entry: &Path) -> Option<String> {
    let b = std::fs::read(entry).ok()?;
    let kl = u32::from_le_bytes(b.get(12..16)?.try_into().ok()?) as usize;
    let start = 24 + kl;
    let end = find(&b, &EOF_MAGIC, start).unwrap_or(b.len());
    let body = b.get(start..end)?;
    if let Ok(s) = std::str::from_utf8(body) {
        return Some(s.to_owned());
    }
    let mut out = Vec::with_capacity(body.len() * 5);
    if brotli_decompressor::Decompressor::new(body, 4096).read_to_end(&mut out).is_ok()
        && let Ok(s) = String::from_utf8(out)
    {
        return Some(s);
    }
    let mut out = String::new();
    flate2::read::GzDecoder::new(body).read_to_string(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(key: &str) -> Vec<u8> {
        let mut h = MAGIC.to_vec();
        h.extend_from_slice(&5u32.to_le_bytes());
        h.extend_from_slice(&(key.len() as u32).to_le_bytes());
        h.extend_from_slice(&[0; 8]);
        h.extend_from_slice(key.as_bytes());
        h
    }

    #[test]
    fn js_key_accepts_query_and_rejects_other_assets() {
        assert_eq!(js_key(&head("1/0/https://discord.com/assets/web.abc.js?cdt=1")).as_deref(), Some("web.abc.js"));
        assert_eq!(js_key(&head("1/0/https://discord.com/assets/0a1b.js")).as_deref(), Some("0a1b.js"));
        assert_eq!(js_key(&head("1/0/https://discord.com/assets/x.css")), None);
        assert_eq!(js_key(&head("1/0/https://cdn.discordapp.com/avatars/1/a.png")), None);
    }
}
