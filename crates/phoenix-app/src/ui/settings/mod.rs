pub mod account;
pub mod nested;
pub mod sessions;
pub mod art;
pub mod authorized_apps;
pub mod connections;
pub mod billing;
mod boost;
mod gift;
mod pages;
pub mod subscriptions;
pub mod synced_map;
pub mod widget_add;
pub mod effects;
pub mod emoticons;
pub(crate) mod profile_editor;
pub mod collectible_picker;
pub mod collectibles;
pub mod profile_activity;
pub mod profile_widgets;
mod view;
pub(crate) mod voice;

use std::path::PathBuf;

use gpui::{Context, SharedString, actions};
use serde_json::{Value, json};

use crate::app::PhoenixApp;

pub use view::render;

actions!(phoenix_settings, [OpenSettings]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppTheme {
    #[default]
    Dark,
    Light,
    Ash,
    Onyx,
}

impl AppTheme {
    pub const ALL: [AppTheme; 4] = [AppTheme::Light, AppTheme::Ash, AppTheme::Dark, AppTheme::Onyx];
    pub fn label(self) -> gpui::SharedString {
        match self {
            Self::Dark => crate::t!("settings.theme.dark"),
            Self::Light => crate::t!("settings.theme.light"),
            Self::Ash => crate::t!("settings.theme.ash"),
            Self::Onyx => crate::t!("settings.theme.onyx"),
        }
    }
    pub fn swatch(self) -> u32 {
        use crate::theme::{generated::idx, tok};
        match self {
            Self::Dark => tok(idx::PLUM_20),
            Self::Light => tok(idx::WHITE),
            Self::Ash => tok(idx::PRIMARY_600),
            Self::Onyx => tok(idx::BLACK),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncedSettings {
    pub theme: AppTheme,
    pub compact: bool,
    pub developer_mode: bool,
    pub locale: SharedString,
    pub allow_server_dms: bool,
    pub proto: std::collections::HashMap<&'static str, u64>,
    pub density: crate::theme::Density,
    pub rest: std::collections::HashMap<&'static str, bool>,
    pub favorites: phoenix_api::rest::favorites::Favorites,
}

impl Default for SyncedSettings {
    fn default() -> Self {
        Self {
            theme: AppTheme::Dark,
            compact: false,
            developer_mode: false,
            locale: "en-US".into(),
            allow_server_dms: true,
            proto: Default::default(),
            density: crate::theme::Density::default(),
            rest: Default::default(),
            favorites: Default::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsPatch {
    Theme(AppTheme),
    Compact(bool),
    DeveloperMode(bool),
    Locale(SharedString),
    AllowServerDms(bool),
    Proto(&'static str, u64),
    Density(crate::theme::Density),
    Rest(&'static str, bool),
    Favorites(Box<phoenix_api::rest::favorites::Favorites>),
}

impl SyncedSettings {
    pub fn apply(&mut self, p: &SettingsPatch) {
        match p {
            SettingsPatch::Theme(t) => self.theme = *t,
            SettingsPatch::Compact(v) => self.compact = *v,
            SettingsPatch::DeveloperMode(v) => self.developer_mode = *v,
            SettingsPatch::Locale(l) => {
                crate::i18n::set_locale(l);
                self.locale = l.clone()
            }
            SettingsPatch::AllowServerDms(v) => self.allow_server_dms = *v,
            SettingsPatch::Rest(id, v) => {
                self.rest.insert(id, *v);
            }
            SettingsPatch::Proto(id, v) => {
                let path = synced_map::entry(id).map(|e| e.path);
                for e in synced_map::ENTRIES.iter().filter(|e| Some(e.path) == path) {
                    self.proto.insert(e.id, *v);
                }
                if *id == "favs" {
                    self.favorites.guild_visible = Some(phoenix_api::rest::settings::BoolValue { value: *v != 0 });
                }
            }
            SettingsPatch::Density(d) => self.density = *d,
            SettingsPatch::Favorites(f) => self.favorites = (**f).clone(),
        }
    }
    pub fn flag(&self, id: &str) -> bool {
        if let Some((_, d)) = synced_map::rest(id) {
            return self.rest.get(id).copied().unwrap_or(d);
        }
        synced_map::get_bool(&self.proto, id)
    }
    pub fn pick(&self, id: &str) -> usize {
        synced_map::get_pick(&self.proto, id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalPrefs {
    pub font_size: f32,
    pub zoom: u32,
    pub desktop_notifications: bool,
    pub notification_sounds: bool,
    pub reduced_motion: bool,
    pub animate_emoji: bool,
    pub saturation: u32,
    pub minimize_to_tray: bool,
    pub beta_channel: bool,
    pub ui: std::collections::BTreeMap<String, u64>,
    pub trusted_domains: std::collections::BTreeSet<String>,
    pub trusted_protocols: std::collections::BTreeSet<String>,
    pub collapsed_in_inbox: std::collections::BTreeSet<u64>,
    pub keybinds: Vec<crate::keybinds::Keybind>,
}

impl Default for LocalPrefs {
    fn default() -> Self {
        Self {
            font_size: 16.,
            zoom: 100,
            desktop_notifications: true,
            notification_sounds: true,
            reduced_motion: false,
            animate_emoji: true,
            saturation: 100,
            minimize_to_tray: true,
            beta_channel: false,
            ui: Default::default(),
            trusted_domains: Default::default(),
            trusted_protocols: Default::default(),
            collapsed_in_inbox: Default::default(),
            keybinds: Vec::new(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    crate::paths::config_file("settings.json")
}

pub const PROTECTED_WRITES: &str = "protected-writes";

impl LocalPrefs {
    pub fn protected_writes(&self) -> bool {
        self.flag(PROTECTED_WRITES, false)
    }
    pub fn streamer_hides_info(&self) -> bool {
        self.flag("streamer", false) && self.flag("st-info", true)
    }
    pub fn notifications_on(&self) -> bool {
        self.desktop_notifications && !(self.flag("streamer", false) && self.flag("st-notifs", true))
    }
    pub fn flag(&self, id: &str, default: bool) -> bool {
        self.ui.get(id).map(|v| *v != 0).unwrap_or(default)
    }
    pub fn pick(&self, id: &str, default: usize) -> usize {
        self.ui.get(id).map(|v| *v as usize).unwrap_or(default)
    }
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let f = |k: &str| v.get(k).and_then(Value::as_f64);
        let b = |k: &str, def: bool| v.get(k).and_then(Value::as_bool).unwrap_or(def);
        Self {
            font_size: f("font_size").map(|x| (x as f32).clamp(12., 24.)).unwrap_or(d.font_size),
            zoom: f("zoom").map(|x| (x as u32).clamp(50, 200)).unwrap_or(d.zoom),
            desktop_notifications: b("desktop_notifications", d.desktop_notifications),
            notification_sounds: b("notification_sounds", d.notification_sounds),
            reduced_motion: b("reduced_motion", d.reduced_motion),
            animate_emoji: b("animate_emoji", d.animate_emoji),
            saturation: f("saturation").map(|x| (x as u32).min(100)).unwrap_or(d.saturation),
            minimize_to_tray: b("minimize_to_tray", d.minimize_to_tray),
            beta_channel: b("beta_channel", d.beta_channel),
            ui: v
                .get("ui")
                .and_then(Value::as_object)
                .map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_u64()?))).collect())
                .unwrap_or_default(),
            trusted_domains: strings(v, "trusted_domains"),
            trusted_protocols: strings(v, "trusted_protocols"),
            collapsed_in_inbox: v
                .get("collapsed_in_inbox")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).collect())
                .unwrap_or_default(),
            keybinds: v.get("keybinds").and_then(Value::as_array).map(|a| a.iter().filter_map(crate::keybinds::Keybind::from_json).collect()).unwrap_or_default(),
        }
    }
    pub fn to_json(&self) -> Value {
        json!({
            "font_size": self.font_size,
            "zoom": self.zoom,
            "desktop_notifications": self.desktop_notifications,
            "notification_sounds": self.notification_sounds,
            "reduced_motion": self.reduced_motion,
            "animate_emoji": self.animate_emoji,
            "saturation": self.saturation,
            "minimize_to_tray": self.minimize_to_tray,
            "beta_channel": self.beta_channel,
            "ui": self.ui,
            "trusted_domains": self.trusted_domains,
            "trusted_protocols": self.trusted_protocols,
            "collapsed_in_inbox": self.collapsed_in_inbox,
            "keybinds": self.keybinds.iter().map(crate::keybinds::Keybind::to_json).collect::<Vec<_>>(),
        })
    }
    pub fn load() -> Self {
        config_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|v| Self::from_json(&v))
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let Some(p) = config_path() else { return };
        let data = serde_json::to_vec_pretty(&self.to_json()).unwrap_or_default();
        std::thread::spawn(move || write_prefs(&p, &data));
    }
    pub fn save_blocking(&self) {
        let Some(p) = config_path() else { return };
        write_prefs(&p, &serde_json::to_vec_pretty(&self.to_json()).unwrap_or_default());
    }
}

fn strings(v: &Value, key: &str) -> std::collections::BTreeSet<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

fn write_prefs(p: &std::path::Path, data: &[u8]) {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(tmp, p);
    }
}

pub const LOCAL_THEME: &str = "theme-local";

pub const LOCAL_CLIENT_THEME: &str = "client-theme-local";

pub fn client_preset(prefs: &Prefs) -> Option<&'static crate::theme::client_theme::Preset> {
    let id = match prefs.local.ui.get(LOCAL_CLIENT_THEME) {
        Some(0) => return None,
        Some(v) => v - 1,
        None => *prefs.synced.proto.get("client-theme")?,
    };
    crate::theme::client_theme::by_id(u32::try_from(id).ok()?)
}

pub fn effective_theme(prefs: &Prefs, os_dark: bool) -> AppTheme {
    if let Some(p) = client_preset(prefs) {
        return if p.light { AppTheme::Light } else { AppTheme::Dark };
    }
    let local = prefs.local.ui.get(LOCAL_THEME).and_then(|i| AppTheme::ALL.get(*i as usize)).copied();
    let base = match local {
        Some(t) if !prefs.local.flag("theme-devices", true) => t,
        _ => prefs.synced.theme,
    };
    if !prefs.local.flag("theme-sync", false) {
        return base;
    }
    match (os_dark, base) {
        (false, _) => AppTheme::Light,
        (true, AppTheme::Light) => AppTheme::Dark,
        (true, t) => t,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Prefs {
    pub synced: SyncedSettings,
    pub local: LocalPrefs,
}

impl Prefs {
    pub fn load() -> Self {
        let local = LocalPrefs::load();
        crate::notifications::set_enabled(local.notifications_on());
        crate::sounds::sync_prefs(&local);
        Self { synced: SyncedSettings::default(), local }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    MyAccount,
    Profiles,
    DataPrivacy,
    PrivacySafety,
    Devices,
    Connections,
    Notifications,
    Keybinds,
    Language,
    Appearance,
    Accessibility,
    VoiceVideo,
    Advanced,
    System,
    ActivityPrivacy,
    RegisteredGames,
    Nitro,
    ServerBoost,
    Subscriptions,
    GiftInventory,
    Billing,
}

impl Page {
    pub fn label(self) -> gpui::SharedString {
        match self {
            Self::MyAccount => crate::t!("settings.page.my_account"),
            Self::Profiles => crate::t!("settings.page.profiles"),
            Self::DataPrivacy => crate::t!("settings.page.privacy"),
            Self::PrivacySafety => crate::t!("settings.page.content_social"),
            Self::System => crate::t!("settings.page.system"),
            Self::ActivityPrivacy => crate::t!("settings.page.activity"),
            Self::Devices => crate::t!("settings.page.devices"),
            Self::Connections => crate::t!("settings.page.connections"),
            Self::Notifications => crate::t!("settings.page.notifications"),
            Self::Keybinds => crate::t!("settings.page.keybinds"),
            Self::Language => crate::t!("settings.page.language"),
            Self::Appearance => crate::t!("settings.page.appearance"),
            Self::Accessibility => crate::t!("settings.page.accessibility"),
            Self::VoiceVideo => crate::t!("settings.page.voice_video"),
            Self::Advanced => crate::t!("settings.page.advanced"),
            Self::RegisteredGames => crate::t!("sp.games"),
            Self::Nitro => crate::t!("settings.page.nitro"),
            Self::ServerBoost => crate::t!("settings.page.server_boost"),
            Self::Subscriptions => crate::t!("settings.page.subscriptions"),
            Self::GiftInventory => crate::t!("settings.page.gift_inventory"),
            Self::Billing => crate::t!("settings.page.billing"),
        }
    }
}

pub const NAV: &[(&str, &[Page])] = &[
    ("settings.nav.user", &[Page::MyAccount, Page::DataPrivacy, Page::PrivacySafety, Page::Notifications]),
    ("settings.nav.billing", &[Page::Nitro, Page::ServerBoost, Page::Subscriptions, Page::GiftInventory, Page::Billing]),
    ("settings.nav.app", &[Page::VoiceVideo, Page::Appearance, Page::Accessibility, Page::System, Page::Language]),
    ("settings.nav.apps", &[Page::RegisteredGames, Page::ActivityPrivacy, Page::Connections]),
    ("", &[Page::Advanced]),
];

#[derive(Default)]
pub struct SettingsState {
    pub page: Page,
    pub confirm_logout: bool,
    pub show_changelog: bool,
    pub confirm_hwaccel: Option<bool>,
    pub error: Option<SharedString>,
    pub reveal_email: bool,
    pub reveal_phone: bool,
    pub toggles: std::collections::HashMap<&'static str, bool>,
    pub picks: std::collections::HashMap<&'static str, usize>,
    pub expanded: std::collections::HashSet<&'static str>,
    pub open_select: Option<&'static str>,
    pub search: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub search_sub: Option<gpui::Subscription>,
    pub profile: Option<profile_editor::EditorState>,
    pub scroll: gpui::ScrollHandle,
    pub nested: Option<nested::Nested>,
    pub active_section: usize,
}

pub fn open(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if app.screen == crate::app::Screen::Main {
        app.settings = Some(SettingsState::default());
        nested::reset();
        view::reset_nav();
        account::fetch_sessions(app, cx);
        billing::fetch(app, cx);
        subscriptions::fetch(app, cx);
        authorized_apps::fetch(app, cx);
        connections::fetch(app, cx);
        boost::fetch(app, cx);
        cx.notify();
    }
}

pub fn open_action(app: &mut PhoenixApp, _: &OpenSettings, _: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    open(app, cx);
}

pub fn on_escape(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(s) = app.settings.as_mut() else { return false };
    if widget_add::close() {
        cx.notify();
        return true;
    }
    if s.page == Page::Profiles {
        if profile_editor::on_escape(s) {
            app.settings = None;
        }
        cx.notify();
        return true;
    }
    if authorized_apps::close_confirm() {
        cx.notify();
        return true;
    }
    if s.confirm_hwaccel.is_some() {
        s.confirm_hwaccel = None;
    } else if s.show_changelog {
        s.show_changelog = false;
    } else if s.confirm_logout {
        s.confirm_logout = false;
    } else if s.open_select.is_some() {
        s.open_select = None;
    } else {
        app.settings = None;
    }
    cx.notify();
    true
}

pub fn on_ready(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let fut = app.backend.load_settings();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            if let Ok(s) = res {
                crate::i18n::set_locale(&s.locale);
                this.prefs.synced = s;
                this.chat.style_gen = u32::MAX;
                cx.notify();
            }
        });
    })
    .detach();
}

pub fn save(app: &mut PhoenixApp, patch: SettingsPatch, cx: &mut Context<PhoenixApp>) {
    let before = app.prefs.synced.clone();
    app.prefs.synced.apply(&patch);
    let fut = app.backend.save_settings(patch);
    cx.notify();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            match res {
                Ok(mut s) => {
                    for (k, v) in &this.prefs.synced.proto {
                        s.proto.entry(*k).or_insert(*v);
                    }
                    for (k, v) in &this.prefs.synced.rest {
                        s.rest.entry(*k).or_insert(*v);
                    }
                    crate::i18n::set_locale(&s.locale);
                    this.prefs.synced = s
                }
                Err(e) => {
                    crate::i18n::set_locale(&before.locale);
                    this.prefs.synced = before;
                    if let Some(st) = this.settings.as_mut() {
                        st.error = Some(e.0.into());
                    }
                }
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn update_local(app: &mut PhoenixApp, f: impl FnOnce(&mut LocalPrefs), cx: &mut Context<PhoenixApp>) {
    f(&mut app.prefs.local);
    crate::notifications::set_enabled(app.prefs.local.notifications_on());
    crate::sounds::sync_prefs(&app.prefs.local);
    app.prefs.local.save();
    cx.notify();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_prefs_roundtrip_and_clamp() {
        let p = LocalPrefs { font_size: 20., zoom: 125, desktop_notifications: false, ..Default::default() };
        assert_eq!(LocalPrefs::from_json(&p.to_json()), p);
        let c = LocalPrefs::from_json(&json!({"font_size": 99, "zoom": 10}));
        assert_eq!((c.font_size, c.zoom), (24., 50));
        assert_eq!(LocalPrefs::from_json(&json!(null)), LocalPrefs::default());
    }

    #[test]
    fn patches_apply() {
        let mut s = SyncedSettings::default();
        s.apply(&SettingsPatch::Theme(AppTheme::Onyx));
        s.apply(&SettingsPatch::DeveloperMode(true));
        assert_eq!(s.theme, AppTheme::Onyx);
        assert!(s.developer_mode);
    }

    #[test]
    fn device_theme_and_local_theme() {
        let mut p = Prefs::default();
        p.synced.theme = AppTheme::Onyx;
        assert_eq!(effective_theme(&p, false), AppTheme::Onyx);
        p.local.ui.insert("theme-sync".into(), 1);
        assert_eq!(effective_theme(&p, false), AppTheme::Light);
        assert_eq!(effective_theme(&p, true), AppTheme::Onyx);
        p.synced.theme = AppTheme::Light;
        assert_eq!(effective_theme(&p, true), AppTheme::Dark);
        p.local.ui.insert("theme-sync".into(), 0);
        p.local.ui.insert(LOCAL_THEME.into(), 1);
        assert_eq!(effective_theme(&p, true), AppTheme::Light);
        p.local.ui.insert("theme-devices".into(), 0);
        assert_eq!(effective_theme(&p, true), AppTheme::Ash);
    }
}
