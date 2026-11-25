use std::ffi::OsString;
use std::path::PathBuf;

use phoenix_api::paths as api;

pub use api::{APP_DIR, DATA_DIR_ENV, sandbox};

#[cfg(all(test, target_os = "linux"))]
#[used]
#[unsafe(link_section = ".init_array")]
static ARM_BEFORE_MAIN: extern "C" fn() = {
    extern "C" fn arm_before_main() {
        api::sandbox();
    }
    arm_before_main
};

fn arm() {
    if cfg!(test) {
        api::sandbox();
    }
}

pub fn is_sandboxed() -> bool {
    arm();
    api::is_sandboxed()
}

pub fn home() -> Option<PathBuf> {
    arm();
    api::home()
}

pub fn env_dir(name: &str) -> Option<OsString> {
    arm();
    api::env_dir(name)
}

pub fn config_home() -> Option<PathBuf> {
    arm();
    api::config_home()
}

pub fn config_dir() -> Option<PathBuf> {
    arm();
    api::config_dir()
}

pub fn config_file(name: &str) -> Option<PathBuf> {
    Some(config_dir()?.join(name))
}

pub fn cache_dir() -> Option<PathBuf> {
    arm();
    api::cache_dir()
}

pub fn downloads_dir() -> PathBuf {
    arm();
    api::downloads_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_tests_run_in_a_sandbox_without_any_setup() {
        assert!(phoenix_api::paths::is_sandboxed());
        let root = phoenix_api::paths::sandbox_root().unwrap();
        let real = std::env::var_os("HOME").map(PathBuf::from);
        assert!(real.is_none_or(|h| root != h && !root.starts_with(h.join(".config")) && !root.starts_with(h.join(".cache"))));
        assert_eq!(config_file("voice.json"), Some(root.join(".config").join(APP_DIR).join("voice.json")));
        assert!(env_dir("XDG_CONFIG_HOME").is_none());
        assert!(crate::icons::discord_caches().is_empty());
        assert!(phoenix_api::build_info::discord_config_dirs().is_empty());
    }
}
