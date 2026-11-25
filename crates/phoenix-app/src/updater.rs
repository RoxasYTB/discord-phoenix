use std::{
    cmp::Ordering,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
    time::Duration,
};

use parking_lot::RwLock;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const RELEASES_API_URL: &str = "https://api.github.com/repos/RoxasYTB/project-phoenix/releases";
pub const RELEASES_PAGE_URL: &str = "https://github.com/RoxasYTB/project-phoenix/releases";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreId {
    Num(u64),
    Str(String),
}

impl Ord for PreId {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PreId::Num(a), PreId::Num(b)) => a.cmp(b),
            (PreId::Num(_), PreId::Str(_)) => Ordering::Less,
            (PreId::Str(_), PreId::Num(_)) => Ordering::Greater,
            (PreId::Str(a), PreId::Str(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for PreId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<Vec<PreId>>,
}

impl SemVer {
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim().strip_prefix('v').or_else(|| raw.trim().strip_prefix('V')).unwrap_or(raw.trim());
        let s = s.split('+').next()?;
        let (core, pre_str) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next()?.parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre_str {
            Some(p) => {
                if p.is_empty() {
                    return None;
                }
                let mut ids = Vec::new();
                for seg in p.split('.') {
                    if seg.is_empty() {
                        return None;
                    }
                    if seg.bytes().all(|b| b.is_ascii_digit()) {
                        ids.push(PreId::Num(seg.parse::<u64>().ok()?));
                    } else {
                        ids.push(PreId::Str(seg.to_owned()));
                    }
                }
                Some(ids)
            }
            None => None,
        };
        Some(Self { major, minor, patch, pre })
    }

    pub fn current() -> Self {
        Self::parse(crate::diagnostics::VERSION).unwrap_or(Self { major: 0, minor: 0, patch: 0, pre: None })
    }

    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => {
                    for (x, y) in a.iter().zip(b.iter()) {
                        let c = x.cmp(y);
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                    a.len().cmp(&b.len())
                }
            })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    WindowsX64,
    LinuxAppImage,
    LinuxDeb,
    LinuxTarGz,
    MacOsUniversal,
}

impl Platform {
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Platform::WindowsX64
        }
        #[cfg(target_os = "macos")]
        {
            Platform::MacOsUniversal
        }
        #[cfg(target_os = "linux")]
        {
            if std::env::var_os("APPIMAGE").is_some() {
                return Platform::LinuxAppImage;
            }
            let in_usr = std::env::current_exe().ok().is_some_and(|p| p.starts_with("/usr/"));
            if in_usr && Path::new("/usr/bin/dpkg").exists() {
                return Platform::LinuxDeb;
            }
            Platform::LinuxTarGz
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Platform::LinuxTarGz
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub name: String,
    pub body: String,
    pub html_url: String,
    pub prerelease: bool,
    pub published_at: String,
    pub asset: Option<ReleaseAsset>,
    pub sha256sums_url: Option<String>,
}

pub fn select_asset(assets: &[ReleaseAsset], platform: Platform) -> Option<ReleaseAsset> {
    let find_any = |needles: &[&str]| -> Option<ReleaseAsset> {
        for needle in needles {
            let n = needle.to_ascii_lowercase();
            if let Some(a) = assets.iter().find(|a| a.name.to_ascii_lowercase().ends_with(&n)) {
                return Some(a.clone());
            }
        }
        None
    };
    match platform {
        Platform::WindowsX64 => find_any(&["windows-x86_64.exe", ".exe", "windows-x86_64.zip", ".zip"]),
        Platform::LinuxAppImage => find_any(&["x86_64.appimage", ".appimage", "linux-x86_64.tar.gz"]),
        Platform::LinuxDeb => find_any(&["amd64.deb", ".deb", "linux-x86_64.tar.gz"]),
        Platform::LinuxTarGz => find_any(&["linux-x86_64.tar.gz", ".tar.gz", "x86_64.appimage", ".appimage"]),
        Platform::MacOsUniversal => find_any(&["macos-universal.dmg", ".dmg"]),
    }
}

pub fn find_sha256sums(assets: &[ReleaseAsset]) -> Option<String> {
    assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt") || a.name.eq_ignore_ascii_case("SHA256SUMS"))
        .map(|a| a.download_url.clone())
}

pub fn parse_sha256sums(text: &str, filename: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.splitn(2, |c: char| c.is_ascii_whitespace());
        let hash = parts.next()?.trim();
        let rest = parts.next()?.trim();
        let name = rest.strip_prefix('*').unwrap_or(rest).trim();
        let base = name.rsplit('/').next().unwrap_or(name);
        if (name == filename || base == filename) && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    let expected = expected_hex.trim();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    sha256_hex(bytes).eq_ignore_ascii_case(expected)
}

pub fn parse_releases_json(json: &str, current: &SemVer, include_prerelease: bool, platform: Platform) -> Option<ReleaseInfo> {
    let items = serde_json::from_str::<Value>(json).ok()?.as_array()?.clone();
    let allow_pre = include_prerelease || current.is_prerelease();
    let mut best: Option<(SemVer, ReleaseInfo)> = None;
    for item in items {
        if item.get("draft").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let tag = item.get("tag_name").and_then(Value::as_str).unwrap_or_default();
        let Some(ver) = SemVer::parse(tag) else {
            continue;
        };
        let is_pre = item.get("prerelease").and_then(Value::as_bool).unwrap_or(false) || ver.is_prerelease();
        if is_pre && !allow_pre {
            continue;
        }
        if &ver <= current {
            continue;
        }
        let assets: Vec<ReleaseAsset> = item
            .get("assets")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        Some(ReleaseAsset {
                            name: a.get("name")?.as_str()?.to_owned(),
                            download_url: a.get("browser_download_url")?.as_str()?.to_owned(),
                            size: a.get("size").and_then(Value::as_u64).unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let asset = select_asset(&assets, platform);
        let sha256sums_url = find_sha256sums(&assets);
        let name = item.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or(tag).to_owned();
        let body = item.get("body").and_then(Value::as_str).unwrap_or_default().to_owned();
        let html_url = item.get("html_url").and_then(Value::as_str).unwrap_or(RELEASES_PAGE_URL).to_owned();
        let published_at = item.get("published_at").and_then(Value::as_str).unwrap_or_default().to_owned();
        let info = ReleaseInfo {
            tag: tag.to_owned(),
            name,
            body,
            html_url,
            prerelease: is_pre,
            published_at,
            asset,
            sha256sums_url,
        };
        if best.as_ref().is_none_or(|(b, _)| ver > *b) {
            best = Some((ver, info));
        }
    }
    best.map(|(_, info)| info)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Downloading { release: ReleaseInfo, downloaded: u64, total: u64 },
    Verifying(ReleaseInfo),
    ReadyToRestart { release: ReleaseInfo, path: PathBuf },
    Installed { release: ReleaseInfo, path: PathBuf },
    Error(String),
}

impl UpdateStatus {
    pub fn release(&self) -> Option<&ReleaseInfo> {
        match self {
            UpdateStatus::Available(r)
            | UpdateStatus::Downloading { release: r, .. }
            | UpdateStatus::Verifying(r)
            | UpdateStatus::ReadyToRestart { release: r, .. }
            | UpdateStatus::Installed { release: r, .. } => Some(r),
            _ => None,
        }
    }
}

static STATE: OnceLock<RwLock<UpdateStatus>> = OnceLock::new();
static REV: AtomicU64 = AtomicU64::new(0);
static MOCKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn state() -> &'static RwLock<UpdateStatus> {
    STATE.get_or_init(|| RwLock::new(UpdateStatus::Idle))
}

fn set_status(next: UpdateStatus, on_change: &dyn Fn()) {
    if MOCKED.load(AtomicOrdering::Relaxed) {
        return;
    }
    *state().write() = next;
    REV.fetch_add(1, AtomicOrdering::Relaxed);
    on_change();
}

pub fn status() -> UpdateStatus {
    state().read().clone()
}

pub fn revision() -> u64 {
    REV.load(AtomicOrdering::Relaxed)
}

pub fn available_release() -> Option<ReleaseInfo> {
    state().read().release().cloned()
}

pub fn set_mock_available(release: ReleaseInfo) {
    MOCKED.store(true, AtomicOrdering::Relaxed);
    *state().write() = UpdateStatus::Available(release);
    REV.fetch_add(1, AtomicOrdering::Relaxed);
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("Discord-Phoenix/{}", crate::diagnostics::VERSION))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

pub fn check_now(include_beta: bool, on_change: Arc<dyn Fn() + Send + Sync>) {
    if MOCKED.load(AtomicOrdering::Relaxed) {
        return;
    }
    if matches!(status(), UpdateStatus::Checking | UpdateStatus::Downloading { .. } | UpdateStatus::Verifying(_)) {
        return;
    }
    set_status(UpdateStatus::Checking, &*on_change);
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                set_status(UpdateStatus::Error(e.to_string()), &*on_change);
                return;
            }
        };
        let res = rt.block_on(async move {
            let client = http_client()?;
            let resp = client
                .get(RELEASES_API_URL)
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status().as_u16()));
            }
            resp.text().await.map_err(|e| e.to_string())
        });
        match res {
            Ok(body) => {
                let current = SemVer::current();
                let platform = Platform::current();
                match parse_releases_json(&body, &current, include_beta, platform) {
                    Some(info) => set_status(UpdateStatus::Available(info), &*on_change),
                    None => set_status(UpdateStatus::UpToDate, &*on_change),
                }
            }
            Err(e) => set_status(UpdateStatus::Error(e), &*on_change),
        }
    });
}

fn updates_dir() -> Result<PathBuf, String> {
    let base = crate::paths::cache_dir().ok_or_else(|| "cache directory unavailable".to_owned())?;
    let dir = base.join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn open_path_in_os(path: &Path) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
}

fn try_atomic_replace_appimage(downloaded: &Path) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(downloaded, std::fs::Permissions::from_mode(0o755));
        let target = std::env::var_os("APPIMAGE").map(PathBuf::from)?;
        let parent = target.parent()?;
        let staged = parent.join(format!(".{}.new", target.file_name()?.to_string_lossy()));
        std::fs::copy(downloaded, &staged).ok()?;
        let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
        std::fs::rename(&staged, &target).ok()?;
        Some(target)
    }
    #[cfg(not(unix))]
    {
        let _ = downloaded;
        None
    }
}

pub fn download_and_install(release: ReleaseInfo, on_change: Arc<dyn Fn() + Send + Sync>) {
    let Some(asset) = release.asset.clone() else {
        set_status(UpdateStatus::Error("no compatible asset for this platform".to_owned()), &*on_change);
        return;
    };
    set_status(UpdateStatus::Downloading { release: release.clone(), downloaded: 0, total: asset.size }, &*on_change);
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                set_status(UpdateStatus::Error(e.to_string()), &*on_change);
                return;
            }
        };
        let on_c = on_change.clone();
        let rel = release.clone();
        let res: Result<PathBuf, String> = rt.block_on(async move {
            let dir = updates_dir()?;
            let part_path = dir.join(format!("{}.part", asset.name));
            let final_path = dir.join(&asset.name);
            let client = reqwest::Client::builder()
                .user_agent(format!("Discord-Phoenix/{}", crate::diagnostics::VERSION))
                .timeout(Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?;
            let mut resp = client.get(&asset.download_url).send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status().as_u16()));
            }
            let total = resp.content_length().unwrap_or(asset.size);
            let mut file = std::fs::File::create(&part_path).map_err(|e| e.to_string())?;
            let mut downloaded: u64 = 0;
            let mut last_reported: u64 = 0;
            let mut all_bytes = Vec::with_capacity(total.min(128 * 1024 * 1024) as usize);
            while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
                file.write_all(&chunk).map_err(|e| e.to_string())?;
                all_bytes.extend_from_slice(&chunk);
                downloaded += chunk.len() as u64;
                if downloaded - last_reported >= 128 * 1024 || downloaded == total {
                    last_reported = downloaded;
                    set_status(UpdateStatus::Downloading { release: rel.clone(), downloaded, total }, &*on_c);
                }
            }
            file.flush().map_err(|e| e.to_string())?;
            drop(file);

            if let Some(sums_url) = &rel.sha256sums_url {
                set_status(UpdateStatus::Verifying(rel.clone()), &*on_c);
                let sums_resp = client.get(sums_url).send().await.map_err(|e| e.to_string())?;
                if sums_resp.status().is_success() {
                    let sums_text = sums_resp.text().await.map_err(|e| e.to_string())?;
                    if let Some(expected) = parse_sha256sums(&sums_text, &asset.name)
                        && !verify_sha256(&all_bytes, &expected)
                    {
                        let _ = std::fs::remove_file(&part_path);
                        return Err("SHA256 checksum mismatch".to_owned());
                    }
                }
            }

            std::fs::rename(&part_path, &final_path).map_err(|e| e.to_string())?;
            Ok(final_path)
        });

        match res {
            Ok(path) => {
                if let Some(replaced) = try_atomic_replace_appimage(&path) {
                    set_status(UpdateStatus::ReadyToRestart { release, path: replaced }, &*on_change);
                } else {
                    open_path_in_os(&path);
                    set_status(UpdateStatus::Installed { release, path }, &*on_change);
                }
            }
            Err(e) => set_status(UpdateStatus::Error(e), &*on_change),
        }
    });
}

pub fn restart_app(path: &Path) {
    let _ = std::process::Command::new(path).spawn();
    std::process::exit(0);
}

pub fn spawn_check(include_beta: bool, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    use futures::StreamExt;
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
    let on_change: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = tx.unbounded_send(());
    });
    cx.spawn(async move |this, cx| {
        while rx.next().await.is_some() {
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
    .detach();
    check_now(include_beta, on_change);
}

pub fn spawn_install(release: ReleaseInfo, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    use futures::StreamExt;
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
    let on_change: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = tx.unbounded_send(());
    });
    cx.spawn(async move |this, cx| {
        while rx.next().await.is_some() {
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
    .detach();
    download_and_install(release, on_change);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_ordering_handles_betas_and_stable() {
        let v = |s: &str| SemVer::parse(s).unwrap();
        assert!(v("0.1.0-beta.1") < v("0.1.0-beta.2"));
        assert!(v("0.1.0-beta.2") < v("0.1.0-beta.10"));
        assert!(v("0.1.0-beta.10") < v("0.1.0-rc.1"));
        assert!(v("0.1.0-rc.1") < v("0.1.0"));
        assert!(v("0.1.0") < v("0.1.1"));
        assert!(v("0.1.1") < v("0.2.0"));
        assert!(v("0.2.0") < v("1.0.0"));
        assert_eq!(v("v0.1.0-beta.1+build.42"), v("0.1.0-beta.1"));
        assert!(SemVer::parse("0.1").is_none());
        assert!(SemVer::parse("0.1.0-").is_none());
        assert!(SemVer::parse("not-a-version").is_none());
    }

    #[test]
    fn artifact_selection_matches_each_os() {
        let assets = vec![
            ReleaseAsset { name: "discord-phoenix-0.2.0-windows-x86_64.exe".into(), download_url: "u/win.exe".into(), size: 10 },
            ReleaseAsset { name: "discord-phoenix-0.2.0-windows-x86_64.zip".into(), download_url: "u/win.zip".into(), size: 11 },
            ReleaseAsset { name: "discord-phoenix-0.2.0-x86_64.AppImage".into(), download_url: "u/appimage".into(), size: 12 },
            ReleaseAsset { name: "discord-phoenix-0.2.0-amd64.deb".into(), download_url: "u/deb".into(), size: 13 },
            ReleaseAsset { name: "discord-phoenix-0.2.0-linux-x86_64.tar.gz".into(), download_url: "u/tar.gz".into(), size: 14 },
            ReleaseAsset { name: "discord-phoenix-0.2.0-macos-universal.dmg".into(), download_url: "u/dmg".into(), size: 15 },
            ReleaseAsset { name: "SHA256SUMS.txt".into(), download_url: "u/sums".into(), size: 16 },
        ];
        assert_eq!(select_asset(&assets, Platform::WindowsX64).unwrap().download_url, "u/win.exe");
        assert_eq!(select_asset(&assets, Platform::LinuxAppImage).unwrap().download_url, "u/appimage");
        assert_eq!(select_asset(&assets, Platform::LinuxDeb).unwrap().download_url, "u/deb");
        assert_eq!(select_asset(&assets, Platform::LinuxTarGz).unwrap().download_url, "u/tar.gz");
        assert_eq!(select_asset(&assets, Platform::MacOsUniversal).unwrap().download_url, "u/dmg");
        assert_eq!(find_sha256sums(&assets).as_deref(), Some("u/sums"));
    }

    #[test]
    fn sha256_parsing_and_verification() {
        let data = b"discord phoenix updater test payload";
        let digest = sha256_hex(data);
        assert_eq!(digest.len(), 64);
        assert!(verify_sha256(data, &digest));
        assert!(verify_sha256(data, &digest.to_ascii_uppercase()));
        assert!(!verify_sha256(b"tampered", &digest));
        assert!(!verify_sha256(data, "not-a-hash"));

        let sums = format!(
            "# checksums\n{digest}  dist/discord-phoenix-0.2.0-x86_64.AppImage\n0000000000000000000000000000000000000000000000000000000000000000 *other.zip\n"
        );
        assert_eq!(parse_sha256sums(&sums, "discord-phoenix-0.2.0-x86_64.AppImage").as_deref(), Some(digest.as_str()));
        assert_eq!(
            parse_sha256sums(&sums, "other.zip").as_deref(),
            Some("0000000000000000000000000000000000000000000000000000000000000000")
        );
        assert!(parse_sha256sums(&sums, "missing.deb").is_none());
    }

    #[test]
    fn release_filtering_respects_beta_channel_and_drafts() {
        let json = r#"[
            {"tag_name":"v0.3.0","draft":true,"prerelease":false,"body":"draft","html_url":"https://example/030","assets":[]},
            {"tag_name":"v0.2.0-beta.2","draft":false,"prerelease":true,"body":"beta","html_url":"https://example/020b2","assets":[]},
            {"tag_name":"v0.1.5","draft":false,"prerelease":false,"body":"stable","html_url":"https://example/015","assets":[]}
        ]"#;
        let stable = SemVer::parse("0.1.0").unwrap();
        let beta = SemVer::parse("0.1.0-beta.1").unwrap();

        let r_stable_no_beta = parse_releases_json(json, &stable, false, Platform::LinuxAppImage).unwrap();
        assert_eq!(r_stable_no_beta.tag, "v0.1.5");

        let r_stable_with_beta = parse_releases_json(json, &stable, true, Platform::LinuxAppImage).unwrap();
        assert_eq!(r_stable_with_beta.tag, "v0.2.0-beta.2");

        let r_beta_user = parse_releases_json(json, &beta, false, Platform::LinuxAppImage).unwrap();
        assert_eq!(r_beta_user.tag, "v0.2.0-beta.2");

        let newer = SemVer::parse("0.2.0").unwrap();
        assert!(parse_releases_json(json, &newer, true, Platform::LinuxAppImage).is_none());
    }
}
