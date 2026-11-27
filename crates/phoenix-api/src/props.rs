use base64::Engine;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

pub const DEFAULT_ELECTRON_VERSION: &str = "44.10.2";
pub const DEFAULT_CLIENT_VERSION: &str = "1.0.170";
pub const DEFAULT_BUILD_NUMBER: u64 = 679_055;
pub const DEFAULT_CHROME_VERSION: &str = "152.0.7977.169";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClientProperties {
    pub os: String,
    pub browser: String,
    pub release_channel: String,
    pub client_version: String,
    pub os_version: String,
    pub os_version_is_release: bool,
    pub os_arch: String,
    pub app_arch: String,
    pub system_locale: String,
    pub has_client_mods: bool,
    pub client_launch_id: String,
    pub browser_user_agent: String,
    pub browser_version: String,
    pub window_manager: String,
    pub distro: String,
    pub runtime_environment: String,
    pub display_server: String,
    pub client_build_number: u64,
    pub native_build_number: Option<u64>,
    pub client_event_source: Option<String>,
    pub client_heartbeat_session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_id: Option<u32>,
}

static APP_FOCUSED: AtomicBool = AtomicBool::new(true);

pub fn set_app_focused(focused: bool) {
    APP_FOCUSED.store(focused, Ordering::Relaxed);
}

pub fn client_app_state() -> &'static str {
    if APP_FOCUSED.load(Ordering::Relaxed) { "focused" } else { "unfocused" }
}

static COLD_START: AtomicBool = AtomicBool::new(true);
static APP_SKELETON: AtomicBool = AtomicBool::new(false);

pub fn set_app_skeleton(mounted: bool) {
    if mounted {
        COLD_START.store(false, Ordering::Relaxed);
    }
    APP_SKELETON.store(mounted, Ordering::Relaxed);
}

pub fn connect_reasons(cold_start: bool, app_skeleton: bool) -> &'static str {
    match (cold_start, app_skeleton) {
        (true, true) => "AppSkeleton,COLD_START",
        (true, false) => "COLD_START",
        (false, true) => "AppSkeleton",
        (false, false) => "NO_REASONS",
    }
}

pub fn gateway_connect_reasons() -> &'static str {
    connect_reasons(COLD_START.load(Ordering::Relaxed), APP_SKELETON.load(Ordering::Relaxed))
}

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_owned())
}

pub fn os_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|s| s.trim().to_owned()).unwrap_or_default()
}

pub fn pretty_name(os_release: &str) -> Option<String> {
    let mut name = None;
    for line in os_release.trim_start_matches('\u{feff}').lines() {
        let Some(v) = line.trim().strip_prefix("PRETTY_NAME=") else { continue };
        let v = v.trim();
        let unquoted = match v.as_bytes().first() {
            Some(b'"') | Some(b'\'') if v.len() >= 2 && v.ends_with(&v[..1]) => &v[1..v.len() - 1],
            _ => v,
        };
        name = (!unquoted.is_empty()).then(|| unquoted.replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    name
}

fn linux_distro() -> Option<String> {
    let flatpak = std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists();
    let snap = std::env::var_os("SNAP").is_some() || std::env::var_os("SNAP_NAME").is_some();
    let mut files: Vec<&str> = Vec::new();
    if flatpak {
        files.push("/run/host/os-release");
    }
    if snap {
        files.extend(["/var/lib/snapd/hostfs/etc/os-release", "/var/lib/snapd/hostfs/usr/lib/os-release"]);
    }
    files.extend(["/etc/os-release", "/usr/lib/os-release"]);
    files.into_iter().find_map(|f| std::fs::read_to_string(f).ok()).and_then(|t| pretty_name(&t))
}

fn runtime_environment() -> &'static str {
    if std::env::var_os("FLATPAK_ID").is_some() {
        "flatpak"
    } else if std::env::var_os("SNAP").is_some() || std::env::var_os("SNAP_NAME").is_some() {
        "snap"
    } else if std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some() {
        "appimage"
    } else {
        "native"
    }
}

impl Default for ClientProperties {
    fn default() -> Self {
        Self::linux_desktop()
    }
}

impl ClientProperties {
    pub fn linux_desktop() -> Self {
        Self::from_build_info(&crate::build_info::BuildInfo::default())
    }

    pub fn current() -> Self {
        Self::from_build_info(&crate::build_info::BuildInfo::detect_fast())
    }

    pub fn from_build_info(b: &crate::build_info::BuildInfo) -> Self {
        let electron = b.electron_version.as_str();
        let client = b.client_version.as_str();
        let chrome = b.chrome_version.as_str();
        let ua = format!(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
             discord/{client} Chrome/{chrome} Electron/{electron} Safari/537.36"
        );
        Self {
            os: "Linux".into(),
            browser: "Discord Client".into(),
            release_channel: b.release_channel.clone(),
            client_version: client.into(),
            os_version: os_release(),
            os_version_is_release: true,
            os_arch: "x64".into(),
            app_arch: "x64".into(),
            system_locale: os_discord_locale(),
            has_client_mods: false,
            client_launch_id: uuid::Uuid::new_v4().to_string(),
            browser_user_agent: ua,
            browser_version: electron.into(),
            window_manager: format!("{},{}", env_or("XDG_CURRENT_DESKTOP", "unknown"), env_or("GDMSESSION", "unknown")),
            distro: linux_distro().unwrap_or_default(),
            runtime_environment: runtime_environment().into(),
            display_server: env_or("XDG_SESSION_TYPE", "unknown"),
            client_build_number: b.build_number,
            native_build_number: b.native_build_number,
            client_event_source: None,
            client_heartbeat_session_id: uuid::Uuid::new_v4().to_string(),
            design_id: None,
        }
    }

    pub fn user_agent(&self) -> &str {
        &self.browser_user_agent
    }

    pub fn to_value(&self, app_state: &str) -> serde_json::Value {
        let mut v = serde_json::to_value(self).expect("properties serialize");
        if let Some(m) = v.as_object_mut() {
            m.insert("client_app_state".into(), app_state.into());
        }
        v
    }

    pub fn to_header_with(&self, app_state: &str) -> String {
        let mut json = serde_json::to_vec(self).expect("properties serialize");
        json.pop();
        json.extend_from_slice(format!(",\"client_app_state\":\"{app_state}\"}}").as_bytes());
        base64::engine::general_purpose::STANDARD.encode(json)
    }

    pub fn to_header(&self) -> String {
        self.to_header_with(client_app_state())
    }
}

pub fn os_locale_tag() -> String {
    let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .unwrap_or_default();
    let base = raw.split(['.', '@']).next().unwrap_or("");
    if base.len() < 2 {
        return "en-US".into();
    }
    base.replace('_', "-")
}

pub fn os_discord_locale() -> String {
    discord_locale_for(&os_locale_tag())
}

pub fn discord_locale_for(tag: &str) -> String {
    const REGIONAL: [&str; 8] = ["en-US", "en-GB", "es-ES", "es-419", "pt-BR", "sv-SE", "zh-CN", "zh-TW"];
    if REGIONAL.contains(&tag) {
        return tag.into();
    }
    let lang = tag.split('-').next().unwrap_or("en");
    match lang {
        "en" => "en-US".into(),
        "es" => "es-ES".into(),
        "pt" => "pt-BR".into(),
        "sv" => "sv-SE".into(),
        "zh" => "zh-CN".into(),
        l => l.into(),
    }
}

pub fn accept_language(locale: &str, os_tag: &str) -> String {
    if locale.eq_ignore_ascii_case(os_tag) {
        locale.into()
    } else {
        format!("{locale},{os_tag};q=0.9")
    }
}

pub fn os_timezone() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim_start_matches(':');
        if tz.contains('/') && !tz.starts_with('/') {
            return Some(tz.into());
        }
    }
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let s = target.to_string_lossy();
    let i = s.find("zoneinfo/")?;
    Some(s[i + "zoneinfo/".len()..].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_reasons_follow_the_app_skeleton() {
        assert_eq!(connect_reasons(true, false), "COLD_START");
        assert_eq!(connect_reasons(false, true), "AppSkeleton");
        assert_eq!(connect_reasons(false, false), "NO_REASONS");
        assert_eq!(connect_reasons(true, true), "AppSkeleton,COLD_START");
    }

    #[test]
    fn locale_headers() {
        assert_eq!(discord_locale_for("fr-FR"), "fr");
        assert_eq!(discord_locale_for("en-GB"), "en-GB");
        assert_eq!(discord_locale_for("pt-PT"), "pt-BR");
        assert_eq!(accept_language("fr", "fr-FR"), "fr,fr-FR;q=0.9");
        assert_eq!(accept_language("en-US", "en-US"), "en-US");
    }

    #[test]
    fn header_roundtrip() {
        let p = ClientProperties::default();
        let raw = base64::engine::general_purpose::STANDARD.decode(p.to_header_with("focused")).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(v["browser"], "Discord Client");
        assert_eq!(v["os"], "Linux");
        assert!(v["client_build_number"].is_u64());
        assert_eq!(v["os_version_is_release"], true);
        assert_eq!(v["client_app_state"], "focused");
        let raw = base64::engine::general_purpose::STANDARD.decode(p.to_header_with("unfocused")).unwrap();
        let text = String::from_utf8(raw).unwrap();
        let keys: Vec<&str> = text.split(",\"").map(|k| k.trim_start_matches("{\"").split('"').next().unwrap()).collect();
        assert_eq!(
            keys,
            [
                "os", "browser", "release_channel", "client_version", "os_version", "os_version_is_release", "os_arch",
                "app_arch", "system_locale", "has_client_mods", "client_launch_id", "browser_user_agent", "browser_version",
                "window_manager", "distro", "runtime_environment", "display_server", "client_build_number",
                "native_build_number", "client_event_source", "client_heartbeat_session_id", "client_app_state",
            ]
        );
    }

    #[test]
    fn distro_is_the_os_release_pretty_name() {
        assert_eq!(pretty_name("NAME=\"Debian GNU/Linux\"\nPRETTY_NAME=\"Debian GNU/Linux forky/sid\"\n").as_deref(), Some("Debian GNU/Linux forky/sid"));
        assert_eq!(pretty_name("PRETTY_NAME=Arch\n").as_deref(), Some("Arch"));
        assert_eq!(pretty_name("PRETTY_NAME=\"\"\n"), None);
        assert_eq!(pretty_name("ID=x\n"), None);
    }
}
