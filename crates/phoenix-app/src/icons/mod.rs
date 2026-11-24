//! Runtime icon provider.
//!
//! The UI asks for icons by stable name ([`path`]). At startup we look for a
//! local Discord install, extract the icon SVGs the official client ships in
//! its cached JS bundles into `~/.cache/discord-phoenix/icons/<name>.svg`
//! (re-extracted when the cached bundle set changes, i.e. on a Discord
//! update), and [`crate::assets::Assets`] serves those first, falling back
//! to our hand-drawn icons. Nothing extracted is ever stored in the repo.

pub mod art;
mod cache;
mod extract;
mod names;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use gpui::SharedString;
use parking_lot::RwLock;

pub use names::MAP as NAMES;

/// Asset path for icon `name` (e.g. `"hash"`, `"chat/edit"`).
pub fn path(name: &str) -> SharedString {
    format!("icons/{name}.svg").into()
}

fn store() -> &'static RwLock<HashMap<String, Arc<[u8]>>> {
    static S: OnceLock<RwLock<HashMap<String, Arc<[u8]>>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// Extracted SVG for asset path `icons/<name>.svg`, if any.
pub fn lookup(asset_path: &str) -> Option<Arc<[u8]>> {
    let name = asset_path.strip_prefix("icons/")?.strip_suffix(".svg")?;
    store().read().get(name).cloned()
}

pub fn extracted_names() -> Vec<String> {
    store().read().keys().cloned().collect()
}

/// Output directory: `$XDG_CACHE_HOME|~/.cache/discord-phoenix/icons`
/// (`%LOCALAPPDATA%` on Windows).
pub fn out_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("discord-phoenix").join("icons"))
}

/// Candidate `Cache/Cache_Data` dirs of installed Discord clients.
pub fn discord_caches() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(h.join(".config"));
        roots.push(h.join(".var/app/com.discordapp.Discord/config"));
        roots.push(h.join(".var/app/com.discordapp.DiscordCanary/config"));
        roots.push(h.join("Library/Application Support"));
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        roots.insert(0, PathBuf::from(x));
    }
    if let Some(a) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(a));
    }
    let mut out = Vec::new();
    for r in roots {
        for flavor in ["discord", "discordcanary", "discordptb"] {
            let p = r.join(flavor).join("Cache").join("Cache_Data");
            if p.is_dir() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

fn load_dir(dir: &Path) {
    let mut map = HashMap::new();
    for (name, _) in NAMES {
        if let Ok(b) = std::fs::read(dir.join(format!("{name}.svg"))) {
            map.insert((*name).to_owned(), Arc::from(b));
        }
    }
    *store().write() = map;
}

/// Loads previously extracted icons now, then refreshes them in a background
/// thread if the local Discord's cached build changed.
pub fn init() {
    let Some(dir) = out_dir() else { return };
    load_dir(&dir);
    std::thread::Builder::new()
        .name("icon-extract".into())
        .spawn(move || {
            if refresh(&dir) {
                load_dir(&dir);
            }
            art::init(&dir);
        })
        .ok();
}

/// Re-extracts into `dir` if the bundle fingerprint changed. Returns whether
/// anything was rewritten.
pub fn refresh(dir: &Path) -> bool {
    let caches = discord_caches();
    let entries: Vec<_> = caches.iter().flat_map(|c| cache::js_entries(c)).collect();
    if entries.is_empty() {
        return false;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    entries.iter().for_each(|(k, _)| k.hash(&mut h));
    NAMES.hash(&mut h);
    2u8.hash(&mut h); // extractor revision (secondary-colour backdrops dropped)
    let stamp = format!("{:016x}", h.finish());
    let stamp_file = dir.join(".build");
    if std::fs::read_to_string(&stamp_file).is_ok_and(|s| s.trim() == stamp) {
        return false;
    }
    let found = extract_all(&entries);
    let _ = std::fs::create_dir_all(dir);
    for (name, svg) in &found {
        let p = dir.join(format!("{name}.svg"));
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, svg);
    }
    let _ = std::fs::write(stamp_file, stamp);
    true
}

/// Decodes and scans bundles on a few worker threads. Entries come largest
/// first; for each name the earliest entry that defines it wins, so the
/// result does not depend on thread timing, and workers stop once every
/// name is found in an earlier entry than the one they would open next.
fn extract_all(entries: &[(String, PathBuf)]) -> HashMap<&'static str, String> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let found: Mutex<HashMap<&'static str, (usize, String)>> = Mutex::new(HashMap::new());
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4);
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((_, p)) = entries.get(i) else { break };
                let todo: Vec<_> = {
                    let f = found.lock().unwrap_or_else(|e| e.into_inner());
                    NAMES.iter().filter(|(n, _)| f.get(n).is_none_or(|(j, _)| *j > i)).collect()
                };
                if todo.is_empty() {
                    break;
                }
                let Some(js) = cache::read_body(p) else { continue };
                if !js.contains("Icon:function(") {
                    continue;
                }
                for (name, candidates) in todo {
                    if let Some(svg) = candidates.iter().find_map(|c| extract::icon_svg(&js, c)) {
                        let mut f = found.lock().unwrap_or_else(|e| e.into_inner());
                        if f.get(name).is_none_or(|(j, _)| *j > i) {
                            f.insert(*name, (i, svg));
                        }
                    }
                }
            });
        }
    });
    found.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().map(|(k, (_, v))| (k, v)).collect()
}

#[cfg(test)]
mod tests {
    /// Manual: `cargo test -p phoenix-app --lib icons::tests::extract_local -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn extract_local() {
        let dir = super::out_dir().unwrap();
        let _ = std::fs::remove_file(dir.join(".build"));
        let t = std::time::Instant::now();
        assert!(super::refresh(&dir));
        super::load_dir(&dir);
        let mut got = super::extracted_names();
        got.sort();
        let missing: Vec<_> = super::NAMES.iter().map(|(n, _)| *n).filter(|n| !got.iter().any(|g| g == n)).collect();
        eprintln!("{} icons in {:?}; missing: {missing:?}", got.len(), t.elapsed());
    }

    /// Every named icon has a hand-made fallback, so nothing renders empty
    /// without a local Discord install.
    #[test]
    fn every_name_has_fallback() {
        for (name, _) in super::NAMES {
            assert!(crate::assets::fallback_svg(&super::path(name)).is_some(), "no fallback for {name}");
        }
        assert!(crate::assets::fallback_svg("icons/send.svg").is_some(), "no fallback for send");
    }

    /// Extracted Discord SVGs must never end up in the repository.
    #[test]
    fn no_extracted_icons_in_repo() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        fn walk(d: &std::path::Path, bad: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                    continue;
                }
                if p.is_dir() {
                    walk(&p, bad);
                } else if p.extension().is_some_and(|x| x == "svg") {
                    bad.push(p);
                }
            }
        }
        let mut bad = Vec::new();
        walk(root, &mut bad);
        assert!(bad.is_empty(), "SVG files must not be committed (icons are extracted at runtime): {bad:?}");
    }
}
