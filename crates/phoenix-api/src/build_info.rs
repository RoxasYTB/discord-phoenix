use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::props::{
    DEFAULT_BUILD_NUMBER, DEFAULT_CHROME_VERSION, DEFAULT_CLIENT_VERSION, DEFAULT_ELECTRON_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    pub build_number: u64,
    pub client_version: String,
    pub electron_version: String,
    pub chrome_version: String,
    pub release_channel: String,
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self {
            build_number: DEFAULT_BUILD_NUMBER,
            client_version: DEFAULT_CLIENT_VERSION.into(),
            electron_version: DEFAULT_ELECTRON_VERSION.into(),
            chrome_version: DEFAULT_CHROME_VERSION.into(),
            release_channel: "stable".into(),
        }
    }
}

pub fn discord_config_dirs() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    [
        xdg.join("discord"),
        home.join(".var/app/com.discordapp.Discord/config/discord"),
        home.join("snap/discord/current/.config/discord"),
    ]
    .into_iter()
    .filter(|p| p.is_dir())
    .collect()
}

fn cache_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".cache"));
    Some(base.join("discord-phoenix/build_info.json"))
}

fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn valid_version(v: &str) -> bool {
    !v.is_empty() && v.len() < 32 && v.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn install_dir(config: &Path) -> Option<PathBuf> {
    let best = std::fs::read_dir(config)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let v = name.strip_prefix("app-")?.to_string();
            valid_version(&v).then(|| (version_key(&v), e.path()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, p)| p);
    best.or_else(|| config.join("resources/build_info.json").is_file().then(|| config.to_path_buf()))
}

pub fn parse_build_info_json(s: &str) -> Option<(String, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let ver = v.get("version")?.as_str()?.to_string();
    valid_version(&ver).then_some(())?;
    let ch = v.get("releaseChannel").and_then(|c| c.as_str()).map(str::to_string);
    Some((ver, ch))
}

pub fn find_chrome_version(s: &[u8]) -> Option<String> {
    find_version_after(s, b"Chrome/")
}

fn find_version_after(s: &[u8], needle: &[u8]) -> Option<String> {
    let mut from = 0;
    while let Some(i) = find(s, needle, from) {
        let start = i + needle.len();
        let end = s[start..].iter().position(|b| !(b.is_ascii_digit() || *b == b'.')).map_or(s.len(), |n| start + n);
        let v = std::str::from_utf8(&s[start..end]).ok()?.trim_end_matches('.');
        if valid_version(v) && v.contains('.') {
            return Some(v.to_string());
        }
        from = start;
    }
    None
}

pub fn find_build_number(js: &[u8]) -> Option<u64> {
    for needle in [&b"build_number:\""[..], b"buildNumber:\""] {
        let mut from = 0;
        while let Some(i) = find(js, needle, from) {
            let start = i + needle.len();
            let digits: Vec<u8> = js[start..].iter().take_while(|b| b.is_ascii_digit()).copied().collect();
            if (5..=8).contains(&digits.len())
                && js.get(start + digits.len()) == Some(&b'"')
                && let Ok(n) = std::str::from_utf8(&digits).unwrap_or("").parse()
            {
                return Some(n);
            }
            from = start;
        }
    }
    None
}

fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if from >= h.len() || n.is_empty() {
        return None;
    }
    h[from..].windows(n.len()).position(|w| w == n).map(|p| p + from)
}

impl BuildInfo {
    pub fn detect_fast() -> Self {
        let mut b = Self::default();
        if let Some(cached) = cache_file().and_then(|p| std::fs::read(p).ok()).and_then(|s| serde_json::from_slice::<BuildInfo>(&s).ok()) {
            b = cached;
        }
        for dir in discord_config_dirs() {
            if let Some((ver, ch)) = install_dir(&dir)
                .and_then(|d| std::fs::read_to_string(d.join("resources/build_info.json")).ok())
                .and_then(|s| parse_build_info_json(&s))
            {
                b.client_version = ver;
                if let Some(ch) = ch {
                    b.release_channel = ch;
                }
                if let Some(c) = std::fs::read(dir.join("Preferences")).ok().and_then(|p| find_chrome_version(&p)) {
                    b.chrome_version = c;
                }
                break;
            }
        }
        b
    }

    pub fn refresh_cache() -> Self {
        let mut b = Self::detect_fast();
        for dir in discord_config_dirs() {
            let Some(inst) = install_dir(&dir) else { continue };
            if let Some(e) = std::fs::read(inst.join("Discord")).ok().and_then(|bin| find_version_after(&bin, b"Electron v")) {
                b.electron_version = e;
            }
            if let Some(n) = newest_build_number(&dir.join("Cache/Cache_Data")) {
                b.build_number = n;
            }
            break;
        }
        if let Some(p) = cache_file() {
            let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
            if let Ok(s) = serde_json::to_vec_pretty(&b) {
                let _ = std::fs::write(p, s);
            }
        }
        b
    }
}

fn newest_build_number(cache: &Path) -> Option<u64> {
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(cache)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if m.is_file() && m.len() > 4096 { Some((m.modified().ok()?, e.path())) } else { None }
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.0));
    entries.into_iter().find_map(|(_, p)| {
        let mut head = [0u8; 512];
        let n = std::io::Read::read(&mut std::fs::File::open(&p).ok()?, &mut head).ok()?;
        find(&head[..n], b"/assets/", 0)?;
        find(&head[..n], b".js", 0)?;
        find_build_number(&std::fs::read(&p).ok()?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_build_info_json() {
        let s = r#"{"releaseChannel":"stable","sentryDist":"stable-linux-x64","version":"1.0.171"}"#;
        assert_eq!(parse_build_info_json(s), Some(("1.0.171".into(), Some("stable".into()))));
        assert_eq!(parse_build_info_json(r#"{"version":"x"}"#), None);
    }

    #[test]
    fn finds_build_number() {
        let js = br#"foo,BUILD_NUMBER="DISCORD_APP_GET_BUILD_NUMBER",x={build_number:"679055",version:"1"}"#;
        assert_eq!(find_build_number(js), Some(679_055));
        assert_eq!(find_build_number(br#"buildNumber:"680001""#), Some(680_001));
        assert_eq!(find_build_number(br#"build_number:"abc""#), None);
    }

    #[test]
    fn finds_versions() {
        assert_eq!(find_chrome_version(b"UA Chrome/152.0.7977.169 Electron"), Some("152.0.7977.169".into()));
        assert_eq!(find_version_after(b"xx Electron v44.10.2\0", b"Electron v"), Some("44.10.2".into()));
        assert_eq!(find_chrome_version(b"Chrome/ nothing"), None);
    }

    #[test]
    fn picks_newest_install_dir() {
        let d = std::env::temp_dir().join(format!("phx-bi-{}", std::process::id()));
        for v in ["app-1.0.99", "app-1.0.170", "app-junk"] {
            std::fs::create_dir_all(d.join(v)).unwrap();
        }
        assert_eq!(install_dir(&d), Some(d.join("app-1.0.170")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn props_follow_build_info() {
        let b = BuildInfo { build_number: 700_000, client_version: "1.0.200".into(), electron_version: "45.0.0".into(), chrome_version: "153.0.1.2".into(), release_channel: "stable".into() };
        let p = crate::ClientProperties::from_build_info(&b);
        assert_eq!(p.client_build_number, 700_000);
        assert_eq!(p.client_version, "1.0.200");
        assert_eq!(p.browser_version, "45.0.0");
        assert!(p.browser_user_agent.contains("discord/1.0.200 Chrome/153.0.1.2 Electron/45.0.0"));
    }
}

#[cfg(test)]
mod measure {
    #[test]
    #[ignore]
    fn on_this_machine() {
        let t = std::time::Instant::now();
        let fast = super::BuildInfo::detect_fast();
        let tf = t.elapsed();
        let t = std::time::Instant::now();
        let slow = super::BuildInfo::refresh_cache();
        eprintln!("fast {tf:?} {fast:?}\nslow {:?} {slow:?}", t.elapsed());
    }
}
