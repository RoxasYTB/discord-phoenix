use std::cell::RefCell;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const APP_DIR: &str = "discord-phoenix";
pub const DATA_DIR_ENV: &str = "PHOENIX_DATA_DIR";

static SANDBOX: AtomicBool = AtomicBool::new(cfg!(test));
static NEXT: AtomicU64 = AtomicU64::new(0);

struct ThreadRoot(PathBuf);

impl Drop for ThreadRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

thread_local! {
    static THREAD_ROOT: RefCell<Option<ThreadRoot>> = const { RefCell::new(None) };
}

pub fn sandbox() {
    SANDBOX.store(true, Ordering::Relaxed);
}

fn data_dir_env() -> Option<PathBuf> {
    std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()).map(PathBuf::from)
}

pub fn is_sandboxed() -> bool {
    SANDBOX.load(Ordering::Relaxed) || data_dir_env().is_some()
}

fn thread_root() -> PathBuf {
    THREAD_ROOT.with(|r| {
        r.borrow_mut()
            .get_or_insert_with(|| {
                let base = data_dir_env().unwrap_or_else(std::env::temp_dir);
                let n = NEXT.fetch_add(1, Ordering::Relaxed);
                ThreadRoot(base.join(format!("phoenix-sandbox-{}-{n}", std::process::id())))
            })
            .0
            .clone()
    })
}

pub fn sandbox_root() -> Option<PathBuf> {
    if SANDBOX.load(Ordering::Relaxed) {
        return Some(thread_root());
    }
    data_dir_env()
}

pub fn home() -> Option<PathBuf> {
    match sandbox_root() {
        Some(r) => Some(r),
        None => std::env::var_os("HOME").map(PathBuf::from),
    }
}

pub fn env_dir(name: &str) -> Option<OsString> {
    if is_sandboxed() {
        return None;
    }
    std::env::var_os(name)
}

fn absolute_env(name: &str) -> Option<PathBuf> {
    env_dir(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

pub fn config_home() -> Option<PathBuf> {
    absolute_env("XDG_CONFIG_HOME")
        .or_else(|| Some(home()?.join(".config")))
        .or_else(|| if cfg!(windows) { absolute_env("APPDATA") } else { None })
}

pub fn cache_home() -> Option<PathBuf> {
    absolute_env("XDG_CACHE_HOME")
        .or_else(|| absolute_env("LOCALAPPDATA"))
        .or_else(|| Some(home()?.join(".cache")))
}

pub fn config_dir() -> Option<PathBuf> {
    Some(config_home()?.join(APP_DIR))
}

pub fn cache_dir() -> Option<PathBuf> {
    Some(cache_home()?.join(APP_DIR))
}

pub fn downloads_dir() -> PathBuf {
    let home = home().unwrap_or_default();
    env_dir("XDG_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .or_else(|| ["Téléchargements", "Downloads"].iter().map(|d| home.join(d)).find(|p| p.is_dir()))
        .unwrap_or(home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tests_never_resolve_to_the_real_home() {
        assert!(is_sandboxed());
        let root = sandbox_root().unwrap();
        assert!(root.starts_with(std::env::temp_dir()) || data_dir_env().is_some_and(|d| root.starts_with(d)));
        assert_eq!(home(), Some(root.clone()));
        assert_eq!(config_dir(), Some(root.join(".config").join(APP_DIR)));
        assert_eq!(cache_dir(), Some(root.join(".cache").join(APP_DIR)));
        assert!(env_dir("HOME").is_none());
        assert!(downloads_dir().starts_with(&root));
    }

    #[test]
    fn each_test_thread_gets_its_own_root() {
        let here = sandbox_root().unwrap();
        let there = std::thread::spawn(|| sandbox_root().unwrap()).join().unwrap();
        assert_ne!(here, there);
        assert!(!there.exists());
    }
}
