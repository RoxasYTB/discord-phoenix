use base64::Engine;
use serde::Serialize;

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
    pub os_arch: String,
    pub app_arch: String,
    pub system_locale: String,
    pub has_client_mods: bool,
    pub browser_user_agent: String,
    pub browser_version: String,
    pub window_manager: String,
    pub distro: String,
    pub runtime_environment: String,
    pub client_build_number: u64,
    pub native_build_number: Option<u64>,
    pub client_event_source: Option<String>,
    pub client_launch_id: String,
    pub client_heartbeat_session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_id: Option<u32>,
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
            os_version: "6.12.0".into(),
            os_arch: "x64".into(),
            app_arch: "x64".into(),
            system_locale: "en-US".into(),
            has_client_mods: false,
            browser_user_agent: ua,
            browser_version: electron.into(),
            window_manager: "KDE,unknown".into(),
            distro: "Debian GNU/Linux".into(),
            runtime_environment: "native".into(),
            client_build_number: b.build_number,
            native_build_number: None,
            client_event_source: None,
            client_launch_id: uuid::Uuid::new_v4().to_string(),
            client_heartbeat_session_id: uuid::Uuid::new_v4().to_string(),
            design_id: None,
        }
    }

    pub fn user_agent(&self) -> &str {
        &self.browser_user_agent
    }

    pub fn to_header(&self) -> String {
        let json = serde_json::to_vec(self).expect("properties serialize");
        base64::engine::general_purpose::STANDARD.encode(json)
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
        let raw = base64::engine::general_purpose::STANDARD.decode(p.to_header()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(v["browser"], "Discord Client");
        assert_eq!(v["os"], "Linux");
        assert!(v["client_build_number"].is_u64());
    }
}
