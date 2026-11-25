pub mod art;
pub mod clan_badges;
pub(crate) mod cache;
mod extract;
mod names;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use gpui::SharedString;
use parking_lot::RwLock;

pub use names::MAP as NAMES;

pub fn path(name: &str) -> SharedString {
    format!("icons/{name}.svg").into()
}

fn store() -> &'static RwLock<HashMap<String, Arc<[u8]>>> {
    static S: OnceLock<RwLock<HashMap<String, Arc<[u8]>>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

pub fn lookup(asset_path: &str) -> Option<Arc<[u8]>> {
    let name = asset_path.strip_prefix("icons/")?.strip_suffix(".svg")?;
    store().read().get(name).cloned()
}

pub fn extracted_names() -> Vec<String> {
    store().read().keys().cloned().collect()
}

pub fn out_dir() -> Option<PathBuf> {
    Some(crate::paths::cache_dir()?.join("icons"))
}

pub fn discord_caches() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(h) = crate::paths::home() {
        roots.push(h.join(".config"));
        roots.push(h.join(".var/app/com.discordapp.Discord/config"));
        roots.push(h.join(".var/app/com.discordapp.DiscordCanary/config"));
        roots.push(h.join("Library/Application Support"));
    }
    if let Some(x) = crate::paths::env_dir("XDG_CONFIG_HOME") {
        roots.insert(0, PathBuf::from(x));
    }
    if let Some(a) = crate::paths::env_dir("APPDATA") {
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
            clan_badges::init(&dir);
        })
        .ok();
}

pub fn refresh(dir: &Path) -> bool {
    let caches = discord_caches();
    let entries: Vec<_> = caches.iter().flat_map(|c| cache::js_entries(c)).collect();
    if entries.is_empty() {
        return false;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    entries.iter().for_each(|(k, _)| k.hash(&mut h));
    NAMES.hash(&mut h);
    4u8.hash(&mut h);
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

type Found = ((usize, usize), String);

fn extract_all(entries: &[(String, PathBuf)]) -> HashMap<&'static str, String> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let found: Mutex<HashMap<&'static str, Found>> = Mutex::new(HashMap::new());
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4);
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((_, p)) = entries.get(i) else { break };
                let todo: Vec<_> = {
                    let f = found.lock().unwrap_or_else(|e| e.into_inner());
                    NAMES.iter().filter(|(n, _)| f.get(n).is_none_or(|(r, _)| *r > (0, i))).collect()
                };
                if todo.is_empty() {
                    break;
                }
                let Some(js) = cache::read_body(p) else { continue };
                let exports = js.contains("Icon:function(");
                if !exports && !js.contains("(\"svg\",{") {
                    continue;
                }
                for (name, candidates) in todo {
                    let usable = |c: &&str| exports || c.starts_with("path:");
                    if let Some((rank, svg)) = candidates.iter().enumerate().filter(|(_, c)| usable(c)).find_map(|(ci, c)| extract::icon_svg(&js, c).map(|s| ((ci, i), s))) {
                        let mut f = found.lock().unwrap_or_else(|e| e.into_inner());
                        if f.get(name).is_none_or(|(r, _)| *r > rank) {
                            f.insert(*name, (rank, svg));
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

    #[test]
    fn every_name_has_fallback() {
        for (name, _) in super::NAMES {
            assert!(crate::assets::fallback_svg(&super::path(name)).is_some(), "no fallback for {name}");
        }
        assert!(crate::assets::fallback_svg("icons/send.svg").is_some(), "no fallback for send");
    }

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
