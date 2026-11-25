use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use parking_lot::{Mutex, RwLock};
use regex::Regex;
use serde_json::Value;

pub type MessageTable = Arc<HashMap<String, Value>>;

static TABLES: OnceLock<RwLock<HashMap<String, MessageTable>>> = OnceLock::new();
static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn tables() -> &'static RwLock<HashMap<String, MessageTable>> {
    TABLES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn in_flight() -> &'static Mutex<HashSet<String>> {
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn cache_dir() -> Option<PathBuf> {
    if crate::paths::is_sandboxed() {
        return None;
    }
    Some(crate::paths::cache_dir()?.join("i18n"))
}

fn cache_path(locale: &str) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("{locale}.json")))
}

pub fn get(locale: &str) -> Option<MessageTable> {
    tables().read().get(locale).cloned()
}

pub fn load_cached_only(locale: &str) -> Option<MessageTable> {
    if let Some(t) = get(locale) {
        return Some(t);
    }
    let path = cache_path(locale)?;
    let bytes = std::fs::read(&path).ok()?;
    let map: HashMap<String, Value> = serde_json::from_slice(&bytes).ok()?;
    if map.is_empty() {
        return None;
    }
    let arc = Arc::new(map);
    tables().write().insert(locale.to_owned(), arc.clone());
    Some(arc)
}

pub fn ensure_loaded(locale: &str) {
    if locale == "en-US" || crate::paths::is_sandboxed() {
        return;
    }
    if load_cached_only(locale).is_some() {
        return;
    }
    let mut guard = in_flight().lock();
    if !guard.insert(locale.to_owned()) {
        return;
    }
    drop(guard);
    let loc = locale.to_owned();
    std::thread::Builder::new()
        .name(format!("i18n-{loc}"))
        .spawn(move || {
            let _ = load_blocking(&loc);
            in_flight().lock().remove(&loc);
        })
        .ok();
}

pub fn load_blocking(locale: &str) -> Option<MessageTable> {
    if locale == "en-US" {
        return None;
    }
    if let Some(t) = load_cached_only(locale) {
        return Some(t);
    }
    let chunks = chunk_files_for(locale);
    if chunks.is_empty() {
        return None;
    }
    let cached_js = cached_js_map();
    let mut merged: HashMap<String, Value> = HashMap::new();
    for file in &chunks {
        let js = cached_js
            .get(file)
            .and_then(|p| crate::icons::cache::read_body(p))
            .or_else(|| download_js(file));
        if let Some(js) = js {
            extract_messages(&js, &mut merged);
        }
    }
    if merged.is_empty() {
        return None;
    }
    if let Some(path) = cache_path(locale) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(&merged) {
            let _ = std::fs::write(&path, bytes);
        }
    }
    let arc = Arc::new(merged);
    tables().write().insert(locale.to_owned(), arc.clone());
    Some(arc)
}

fn cached_js_map() -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    for cache in crate::icons::discord_caches() {
        for (name, path) in crate::icons::cache::js_entries(&cache) {
            out.entry(name).or_insert(path);
        }
    }
    out
}

fn chunk_files_for(locale: &str) -> Vec<String> {
    for cache in crate::icons::discord_caches() {
        for (_, entry) in crate::icons::cache::js_entries(&cache)
            .into_iter()
            .filter(|(n, _)| n.starts_with("web."))
            .take(4)
        {
            if let Some(js) = crate::icons::cache::read_body(&entry) {
                let files = parse_loader_chunks(&js, locale);
                if !files.is_empty() {
                    return files;
                }
            }
        }
    }
    Vec::new()
}

pub fn parse_loader_chunks(bundle: &str, locale: &str) -> Vec<String> {
    static LOADER_RE: OnceLock<Regex> = OnceLock::new();
    static ENTRY_RE: OnceLock<Regex> = OnceLock::new();
    static CHUNK_RE: OnceLock<Regex> = OnceLock::new();
    let loader_re = LOADER_RE.get_or_init(|| {
        Regex::new(r#"createLoader:\w+\}=n\(\d+\),\w+=\w+\(\{(.*?)\},"en-US"\)"#).expect("valid regex")
    });
    let entry_re = ENTRY_RE.get_or_init(|| {
        Regex::new(r#"(?:"([\w-]+)"|(\w+)):\(\)=>n\.e\((\d+)\)"#).expect("valid regex")
    });
    let chunk_re = CHUNK_RE.get_or_init(|| {
        Regex::new(r#"(\d+):"([0-9a-f]{16})""#).expect("valid regex")
    });

    let mut cmap: HashMap<&str, &str> = HashMap::new();
    for cap in chunk_re.captures_iter(bundle) {
        if let (Some(id), Some(hash)) = (cap.get(1), cap.get(2)) {
            cmap.insert(id.as_str(), hash.as_str());
        }
    }
    let mut files = Vec::new();
    for cap in loader_re.captures_iter(bundle) {
        let Some(block) = cap.get(1) else { continue };
        for ecap in entry_re.captures_iter(block.as_str()) {
            let loc = ecap.get(1).or_else(|| ecap.get(2)).map_or("", |m| m.as_str());
            if loc == locale
                && let Some(cid) = ecap.get(3).map(|m| m.as_str())
                && let Some(hash) = cmap.get(cid)
            {
                let name = format!("{hash}.js");
                if !files.contains(&name) {
                    files.push(name);
                }
            }
        }
    }
    files
}

fn download_js(file: &str) -> Option<String> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    rt.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .ok()?;
        let r = client
            .get(format!("https://discord.com/assets/{file}"))
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0")
            .send()
            .await
            .ok()?;
        if !r.status().is_success() {
            return None;
        }
        r.text().await.ok()
    })
}

pub fn extract_messages(js: &str, out: &mut HashMap<String, Value>) {
    let needle = "JSON.parse('";
    let mut pos = 0;
    while let Some(rel) = js[pos..].find(needle) {
        let start = pos + rel + needle.len();
        let bytes = js.as_bytes();
        let mut i = start;
        let mut escaped = false;
        while i < bytes.len() {
            if escaped {
                escaped = false;
            } else if bytes[i] == b'\\' {
                escaped = true;
            } else if bytes[i] == b'\'' {
                break;
            }
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let raw = &js[start..i];
        pos = i + 1;
        let unescaped = unescape_js_single_quoted(raw);
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&unescaped) {
            for (k, v) in map {
                out.insert(k, v);
            }
        }
    }
}

fn unescape_js_single_quoted(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut units: Vec<u16> = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'x' if i + 3 < bytes.len() => {
                    if let Ok(v) = u16::from_str_radix(&s[i + 2..i + 4], 16) {
                        units.push(v);
                        i += 4;
                        continue;
                    }
                }
                b'u' if i + 5 < bytes.len() => {
                    if let Ok(v) = u16::from_str_radix(&s[i + 2..i + 6], 16) {
                        units.push(v);
                        i += 6;
                        continue;
                    }
                }
                b'n' => {
                    units.push(b'\n' as u16);
                    i += 2;
                    continue;
                }
                b'r' => {
                    units.push(b'\r' as u16);
                    i += 2;
                    continue;
                }
                b't' => {
                    units.push(b'\t' as u16);
                    i += 2;
                    continue;
                }
                b'b' => {
                    units.push(0x08);
                    i += 2;
                    continue;
                }
                b'f' => {
                    units.push(0x0c);
                    i += 2;
                    continue;
                }
                b'0' => {
                    units.push(0);
                    i += 2;
                    continue;
                }
                other => {
                    units.push(other as u16);
                    i += 2;
                    continue;
                }
            }
        }
        let ch = s[i..].chars().next().unwrap();
        let mut buf = [0u16; 2];
        for u in ch.encode_utf16(&mut buf) {
            units.push(*u);
        }
        i += ch.len_utf8();
    }
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_parse_blocks() {
        let js = r#"e.exports=JSON.parse('{"60kAzo":["Online"],"foo":["Caf\xe9 \u2014 \'ok\'"]}')}"#;
        let mut m = HashMap::new();
        extract_messages(js, &mut m);
        assert_eq!(m.get("60kAzo").and_then(|v| v[0].as_str()), Some("Online"));
        assert_eq!(m.get("foo").and_then(|v| v[0].as_str()), Some("Café — 'ok'"));
    }

    #[test]
    fn parses_loader_chunks() {
        let bundle = r#"123:"0123456789abcdef",456:"fedcba9876543210";const {createLoader:a}=n(1),b=a({"de":()=>n.e(123),"pt-BR":()=>n.e(456)},"en-US")"#;
        assert_eq!(parse_loader_chunks(bundle, "de"), vec!["0123456789abcdef.js"]);
        assert_eq!(parse_loader_chunks(bundle, "pt-BR"), vec!["fedcba9876543210.js"]);
    }
}
