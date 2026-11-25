pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("PHOENIX_COMMIT");

pub fn version_line() -> String {
    format!("Discord Phoenix {VERSION} ({COMMIT})")
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gpu {
    pub device: String,
    pub driver: String,
    pub driver_info: String,
    pub software: bool,
}

impl From<gpui::GpuSpecs> for Gpu {
    fn from(g: gpui::GpuSpecs) -> Self {
        Self { device: g.device_name, driver: g.driver_name, driver_info: g.driver_info, software: g.is_software_emulated }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub os: String,
    pub arch: &'static str,
    pub session: String,
    pub gpu: Option<Gpu>,
    pub renderer: &'static str,
    pub hwaccel: bool,
    pub locale: String,
    pub zoom: u32,
    pub font_px: u32,
    pub density: &'static str,
    pub theme: String,
    pub log_filter: String,
}

impl Report {
    pub fn collect(gpu: Option<Gpu>, prefs: &crate::ui::settings::LocalPrefs) -> Self {
        Self {
            os: os_description(),
            arch: std::env::consts::ARCH,
            session: session_description(),
            gpu,
            renderer: renderer(),
            hwaccel: crate::hwaccel::enabled(prefs),
            locale: crate::i18n::locale().to_owned(),
            zoom: prefs.zoom,
            font_px: prefs.font_size.round() as u32,
            density: crate::theme::density().key(),
            theme: format!("{:?}", crate::theme::active()).to_lowercase(),
            log_filter: std::env::var("RUST_LOG").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| crate::logging::DEFAULT_FILTER.to_owned()),
        }
    }

    pub fn render(&self) -> String {
        let gpu = match &self.gpu {
            Some(g) => {
                let driver = [g.driver.trim(), g.driver_info.trim()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
                let driver = if driver.is_empty() { "?".to_owned() } else { driver };
                format!("{} | driver: {driver}{}", g.device.trim(), if g.software { " | software" } else { "" })
            }
            None => "unknown".to_owned(),
        };
        let accel = if self.hwaccel { "on" } else { "off" };
        [
            version_line(),
            format!("OS: {} ({})", self.os, self.arch),
            format!("Session: {}", self.session),
            format!("GPU: {gpu}"),
            format!("Renderer: {} | hardware acceleration: {accel}", self.renderer),
            format!("Language: {}", self.locale),
            format!("Zoom: {}% | font: {}px | density: {}", self.zoom, self.font_px, self.density),
            format!("Theme: {}", self.theme),
            format!("Log level: {}", self.log_filter),
        ]
        .join("\n")
    }
}

pub fn renderer() -> &'static str {
    if cfg!(target_os = "macos") { "Metal" } else { "wgpu" }
}

fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub fn os_release_name(text: &str) -> Option<String> {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_owned())
            .filter(|v| !v.is_empty())
    };
    field("PRETTY_NAME").or_else(|| field("NAME"))
}

pub fn os_description() -> String {
    if cfg!(target_os = "linux") {
        let distro = read_trim("/etc/os-release").and_then(|t| os_release_name(&t)).unwrap_or_else(|| "Linux".to_owned());
        match read_trim("/proc/sys/kernel/osrelease") {
            Some(k) => format!("{distro}, kernel {k}"),
            None => distro,
        }
    } else {
        std::env::consts::OS.to_owned()
    }
}

pub fn session_from(wayland_display: Option<&str>, display: Option<&str>, session_type: Option<&str>, desktop: Option<&str>) -> String {
    let set = |v: Option<&str>| v.is_some_and(|v| !v.trim().is_empty());
    let kind = if set(wayland_display) {
        "Wayland".to_owned()
    } else if set(display) {
        "X11".to_owned()
    } else {
        session_type.filter(|v| !v.trim().is_empty()).unwrap_or("unknown").to_owned()
    };
    match desktop.filter(|d| !d.trim().is_empty()) {
        Some(d) => format!("{kind} ({d})"),
        None => kind,
    }
}

pub fn session_description() -> String {
    if !cfg!(target_os = "linux") {
        return "native".to_owned();
    }
    let var = |k: &str| std::env::var(k).ok();
    session_from(
        var("WAYLAND_DISPLAY").as_deref(),
        var("DISPLAY").as_deref(),
        var("XDG_SESSION_TYPE").as_deref(),
        var("XDG_CURRENT_DESKTOP").as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Report {
        Report {
            os: "Debian GNU/Linux forky/sid, kernel 7.3.4".into(),
            arch: "x86_64",
            session: "Wayland (KDE)".into(),
            gpu: Some(Gpu { device: "NVIDIA GeForce GTX 1080".into(), driver: "NVIDIA".into(), driver_info: "580.196.07".into(), software: false }),
            renderer: "wgpu",
            hwaccel: true,
            locale: "fr".into(),
            zoom: 125,
            font_px: 16,
            density: "default",
            theme: "dark".into(),
            log_filter: "info".into(),
        }
    }

    #[test]
    fn report_lists_every_field() {
        let text = sample().render();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], format!("Discord Phoenix {VERSION} ({COMMIT})"));
        assert_eq!(lines[1], "OS: Debian GNU/Linux forky/sid, kernel 7.3.4 (x86_64)");
        assert_eq!(lines[2], "Session: Wayland (KDE)");
        assert_eq!(lines[3], "GPU: NVIDIA GeForce GTX 1080 | driver: NVIDIA 580.196.07");
        assert_eq!(lines[4], "Renderer: wgpu | hardware acceleration: on");
        assert_eq!(lines[5], "Language: fr");
        assert_eq!(lines[6], "Zoom: 125% | font: 16px | density: default");
        assert_eq!(lines[7], "Theme: dark");
        assert_eq!(lines[8], "Log level: info");
        assert_eq!(crate::logging::redact(&text), text);
    }

    #[test]
    fn report_handles_missing_and_software_gpu() {
        let mut r = sample();
        r.gpu = None;
        assert!(r.render().contains("GPU: unknown"));
        r.gpu = Some(Gpu { device: "llvmpipe (LLVM 19.1.7, 256 bits)".into(), software: true, ..Default::default() });
        r.hwaccel = false;
        let text = r.render();
        assert!(text.contains("GPU: llvmpipe (LLVM 19.1.7, 256 bits) | driver: ? | software"), "{text}");
        assert!(text.contains("hardware acceleration: off"));
    }

    #[test]
    fn collected_report_has_no_account_data() {
        let prefs = crate::ui::settings::LocalPrefs::default();
        let text = Report::collect(None, &prefs).render();
        assert!(text.starts_with("Discord Phoenix "));
        for word in ["token", "email", "@", "user", "guild", "channel"] {
            assert!(!text.to_lowercase().contains(word), "{word} in {text}");
        }
    }

    #[test]
    fn session_detection() {
        assert_eq!(session_from(Some("wayland-0"), Some(":0"), Some("wayland"), Some("KDE")), "Wayland (KDE)");
        assert_eq!(session_from(None, Some(":0"), Some("x11"), None), "X11");
        assert_eq!(session_from(Some(""), None, Some("tty"), Some("")), "tty");
        assert_eq!(session_from(None, None, None, None), "unknown");
    }

    #[test]
    fn os_release_parsing() {
        assert_eq!(os_release_name("NAME=\"Debian GNU/Linux\"\nPRETTY_NAME=\"Debian GNU/Linux forky/sid\"\n").as_deref(), Some("Debian GNU/Linux forky/sid"));
        assert_eq!(os_release_name("NAME=Arch Linux\n").as_deref(), Some("Arch Linux"));
        assert_eq!(os_release_name("ID=x\n"), None);
    }

    #[test]
    fn commit_is_baked_in() {
        assert!(!COMMIT.is_empty());
        assert!(version_line().contains(VERSION));
    }
}
