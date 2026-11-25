use std::path::{Path, PathBuf};
use std::process::Command;

pub const PREF: &str = "hwaccel";
const ICD_DIRS: &[&str] = &["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d", "/usr/local/share/vulkan/icd.d"];

pub fn enabled(prefs: &crate::ui::settings::LocalPrefs) -> bool {
    prefs.flag(PREF, true)
}

pub fn lavapipe_icd(dirs: &[&Path]) -> Option<PathBuf> {
    dirs.iter().find_map(|d| {
        let mut found: Vec<PathBuf> = std::fs::read_dir(d)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("lvp_icd") && n.ends_with(".json")))
            .collect();
        found.sort();
        found.into_iter().next()
    })
}

pub fn software_env(icd: Option<&Path>) -> Vec<(&'static str, String)> {
    let mut v = vec![("LIBGL_ALWAYS_SOFTWARE", "1".to_owned())];
    if let Some(p) = icd {
        let s = p.to_string_lossy().into_owned();
        v.push(("VK_DRIVER_FILES", s.clone()));
        v.push(("VK_ICD_FILENAMES", s));
    }
    v
}

pub fn apply_at_startup() {
    if enabled(&crate::ui::settings::LocalPrefs::load()) {
        return;
    }
    let dirs: Vec<&Path> = ICD_DIRS.iter().map(Path::new).collect();
    for (k, v) in software_env(lavapipe_icd(&dirs).as_deref()) {
        if std::env::var_os(k).is_none() {
            // SAFETY: called first thing in `main`, before any other thread is spawned.
            unsafe { std::env::set_var(k, v) };
        }
    }
}

pub fn relaunch_command() -> std::io::Result<Command> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(std::env::args_os().skip(1));
    Ok(cmd)
}

pub fn relaunch(cx: &mut gpui::App) {
    match relaunch_command().and_then(|mut c| c.spawn()) {
        Ok(_) => cx.quit(),
        Err(e) => tracing::warn!("relaunch: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_env_forces_cpu_adapters() {
        let env = software_env(Some(Path::new("/usr/share/vulkan/icd.d/lvp_icd.x86_64.json")));
        assert_eq!(env[0], ("LIBGL_ALWAYS_SOFTWARE", "1".to_owned()));
        assert!(env.contains(&("VK_DRIVER_FILES", "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json".to_owned())));
        assert_eq!(software_env(None).len(), 1);
    }

    #[test]
    fn finds_lavapipe_icd() {
        let d = std::env::temp_dir().join(format!("phoenix-icd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("nvidia_icd.json"), "{}").unwrap();
        assert_eq!(lavapipe_icd(&[d.as_path()]), None);
        std::fs::write(d.join("lvp_icd.x86_64.json"), "{}").unwrap();
        assert_eq!(lavapipe_icd(&[d.as_path()]), Some(d.join("lvp_icd.x86_64.json")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hwaccel_defaults_on() {
        let mut p = crate::ui::settings::LocalPrefs::default();
        assert!(enabled(&p));
        p.ui.insert(PREF.into(), 0);
        assert!(!enabled(&p));
    }
}
