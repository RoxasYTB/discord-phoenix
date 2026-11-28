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

const DRIVER_VARS: &[&str] = &["VK_DRIVER_FILES", "VK_ICD_FILENAMES", "VK_LOADER_DRIVERS_SELECT", "VK_LOADER_DRIVERS_DISABLE", "__EGL_VENDOR_LIBRARY_FILENAMES", "__EGL_VENDOR_LIBRARY_DIRS"];

const EGL_VENDOR_DIRS: &[&str] = &["/etc/glvnd/egl_vendor.d", "/usr/share/glvnd/egl_vendor.d"];

fn first_named(dirs: &[&Path], pick: impl Fn(&str) -> bool) -> Option<PathBuf> {
    dirs.iter().find_map(|d| {
        let mut found: Vec<PathBuf> = std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(&pick)).collect();
        found.sort();
        found.into_iter().next()
    })
}

pub fn nvidia_only_env(icd_dirs: &[&Path], egl_dirs: &[&Path], nvidia_only: bool) -> Vec<(&'static str, String)> {
    if !nvidia_only || first_named(icd_dirs, |n| n.starts_with("nvidia_icd") && n.ends_with(".json")).is_none() {
        return Vec::new();
    }
    let mut v = vec![("VK_LOADER_DRIVERS_SELECT", "nvidia_icd*".to_owned())];
    if let Some(egl) = first_named(egl_dirs, |n| n.contains("nvidia") && n.ends_with(".json")) {
        v.push(("__EGL_VENDOR_LIBRARY_FILENAMES", egl.to_string_lossy().into_owned()));
    }
    v
}

pub fn only_nvidia_gpus(drm: &Path) -> bool {
    let Ok(it) = std::fs::read_dir(drm) else { return false };
    let vendors: Vec<String> = it
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with("card") && !n.contains('-')))
        .filter_map(|e| std::fs::read_to_string(e.path().join("device/vendor")).ok())
        .map(|v| v.trim().to_ascii_lowercase())
        .collect();
    !vendors.is_empty() && vendors.iter().all(|v| v == "0x10de")
}

pub fn apply_at_startup() {
    let dirs: Vec<&Path> = ICD_DIRS.iter().map(Path::new).collect();
    if enabled(&crate::ui::settings::LocalPrefs::load()) {
        if DRIVER_VARS.iter().any(|k| std::env::var_os(k).is_some()) {
            return;
        }
        let egl: Vec<&Path> = EGL_VENDOR_DIRS.iter().map(Path::new).collect();
        let nvidia_only = Path::new("/proc/driver/nvidia/version").exists() && only_nvidia_gpus(Path::new("/sys/class/drm"));
        set_startup_env(nvidia_only_env(&dirs, &egl, nvidia_only));
        return;
    }
    set_startup_env(software_env(lavapipe_icd(&dirs).as_deref()).into_iter().filter(|(k, _)| std::env::var_os(k).is_none()).collect());
}

static APPLIED: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();

fn set_startup_env(vars: Vec<(&'static str, String)>) {
    for (k, v) in &vars {
        // SAFETY: called first thing in `main`, before any other thread is spawned.
        unsafe { std::env::set_var(k, v) };
    }
    let _ = APPLIED.set(vars.into_iter().map(|(k, _)| k).collect());
}

pub fn relaunch_command() -> std::io::Result<Command> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(std::env::args_os().skip(1));
    for k in APPLIED.get().into_iter().flatten() {
        cmd.env_remove(k);
    }
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
    fn selects_only_nvidia_drivers_when_every_gpu_is_nvidia() {
        let d = std::env::temp_dir().join(format!("phoenix-icd-nv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("icd")).unwrap();
        std::fs::create_dir_all(d.join("egl")).unwrap();
        std::fs::write(d.join("icd/lvp_icd.x86_64.json"), "{}").unwrap();
        std::fs::write(d.join("egl/50_mesa.json"), "{}").unwrap();
        let (icd, egl) = (d.join("icd"), d.join("egl"));
        assert!(nvidia_only_env(&[icd.as_path()], &[egl.as_path()], true).is_empty());
        std::fs::write(d.join("icd/nvidia_icd.json"), "{}").unwrap();
        assert!(nvidia_only_env(&[icd.as_path()], &[egl.as_path()], false).is_empty());
        assert_eq!(nvidia_only_env(&[icd.as_path()], &[egl.as_path()], true), vec![("VK_LOADER_DRIVERS_SELECT", "nvidia_icd*".to_owned())]);
        std::fs::write(d.join("egl/10_nvidia.json"), "{}").unwrap();
        let env = nvidia_only_env(&[icd.as_path()], &[egl.as_path()], true);
        assert_eq!(env[1], ("__EGL_VENDOR_LIBRARY_FILENAMES", d.join("egl/10_nvidia.json").to_string_lossy().into_owned()));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nvidia_only_needs_every_card_to_be_nvidia() {
        let d = std::env::temp_dir().join(format!("phoenix-drm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(!only_nvidia_gpus(&d));
        std::fs::create_dir_all(d.join("card1/device")).unwrap();
        std::fs::create_dir_all(d.join("card1-DP-1")).unwrap();
        std::fs::write(d.join("card1/device/vendor"), "0x10de\n").unwrap();
        assert!(only_nvidia_gpus(&d));
        std::fs::create_dir_all(d.join("card0/device")).unwrap();
        std::fs::write(d.join("card0/device/vendor"), "0x8086\n").unwrap();
        assert!(!only_nvidia_gpus(&d));
        let _ = std::fs::remove_dir_all(&d);
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
