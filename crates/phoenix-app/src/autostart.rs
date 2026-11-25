use std::io;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "discord-phoenix.desktop";
pub const DIR_ENV: &str = "PHOENIX_AUTOSTART_DIR";

pub fn dir() -> Option<PathBuf> {
    if let Some(d) = crate::paths::env_dir(DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    Some(crate::paths::config_home()?.join("autostart"))
}

fn exec_arg(exe: &Path) -> String {
    let s = exe.to_string_lossy();
    if !s.chars().any(|c| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c)) {
        return s.into_owned();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub fn entry(exe: &Path) -> String {
    let name = exe.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| crate::APP_ID.to_owned());
    format!(
        "[Desktop Entry]\nType=Application\nExec={}\nHidden=false\nNoDisplay=false\nName={name}\nIcon={}\nComment=Text and voice chat for gamers.\nX-GNOME-Autostart-enabled=true\n",
        exec_arg(exe),
        crate::APP_ID,
    )
}

pub fn is_installed_in(dir: &Path) -> bool {
    dir.join(FILE_NAME).is_file()
}

pub fn install_in(dir: &Path, exe: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    let tmp = dir.join(format!(".{FILE_NAME}.tmp"));
    std::fs::write(&tmp, entry(exe))?;
    std::fs::rename(tmp, path)
}

pub fn uninstall_in(dir: &Path) -> io::Result<()> {
    match std::fs::remove_file(dir.join(FILE_NAME)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

pub fn is_installed() -> bool {
    dir().is_some_and(|d| is_installed_in(&d))
}

pub fn set(on: bool) {
    let Some(d) = dir() else { return };
    let res = if on { std::env::current_exe().and_then(|exe| install_in(&d, &exe)) } else { uninstall_in(&d) };
    if let Err(e) = res {
        tracing::warn!("autostart: {e}");
    }
}

pub fn refresh() {
    let Some(d) = dir() else { return };
    if is_installed_in(&d)
        && let Ok(exe) = std::env::current_exe()
        && std::fs::read_to_string(d.join(FILE_NAME)).ok().as_deref() != Some(entry(&exe).as_str())
    {
        let _ = install_in(&d, &exe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phoenix-autostart-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn entry_matches_the_client_layout() {
        let e = entry(Path::new("/opt/phoenix/discord-phoenix"));
        assert_eq!(
            e,
            "[Desktop Entry]\nType=Application\nExec=/opt/phoenix/discord-phoenix\nHidden=false\nNoDisplay=false\nName=discord-phoenix\nIcon=discord-phoenix\nComment=Text and voice chat for gamers.\nX-GNOME-Autostart-enabled=true\n"
        );
        assert!(entry(Path::new("/home/a b/my \"app\"")).contains("Exec=\"/home/a b/my \\\"app\\\"\"\n"));
    }

    #[test]
    fn install_and_uninstall_in_a_temp_dir() {
        let d = temp_dir("roundtrip");
        assert!(!is_installed_in(&d));
        install_in(&d, Path::new("/usr/bin/discord-phoenix")).unwrap();
        assert!(is_installed_in(&d));
        assert!(std::fs::read_to_string(d.join(FILE_NAME)).unwrap().contains("Exec=/usr/bin/discord-phoenix\n"));
        uninstall_in(&d).unwrap();
        assert!(!is_installed_in(&d));
        uninstall_in(&d).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}
