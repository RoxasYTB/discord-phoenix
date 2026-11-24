//! Full-screen User Settings overlay (Discord layout: left nav, content,
//! close button; Esc closes) and the settings it reads/writes.
//!
//! * [`SyncedSettings`]: account settings stored by Discord (settings-proto),
//!   exactly the ones the client keeps in `PreloadedUserSettings`: theme,
//!   developer mode and UI density (`appearance`), compact message display
//!   (`text_and_images`), locale (`localization`), server DMs (`privacy`).
//! * [`LocalPrefs`]: what the client keeps per machine (localStorage stores:
//!   AccessibilityStore `fontScale`/`saturation`/`reducedMotion`, the Electron
//!   zoom factor, desktop notification and sound toggles), saved as JSON in
//!   `$XDG_CONFIG_HOME/discord-phoenix/settings.json` and never sent to Discord.

pub mod account;
mod art;
pub mod authorized_apps;
pub mod billing;
mod boost;
mod gift;
mod pages;
pub mod subscriptions;
pub mod synced_map;
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
    /// Preview swatch colour.
    pub fn swatch(self) -> u32 {
        match self {
            Self::Dark => 0x313338,
            Self::Light => 0xFFFFFF,
            Self::Ash => 0x2B2B30,
            Self::Onyx => 0x0C0C0E,
        }
    }
}

/// Account settings synced through Discord.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncedSettings {
    pub theme: AppTheme,
    pub compact: bool,
    pub developer_mode: bool,
    pub locale: SharedString,
    /// Privacy: allow DMs from members of servers (inverse of
    /// `default_guilds_restricted`).
    pub allow_server_dms: bool,
    /// Every other synced control, by settings id ([`synced_map::ENTRIES`]),
    /// as its wire value; missing = client default.
    pub proto: std::collections::HashMap<&'static str, u64>,
    /// UI density (`appearance.ui_density`, synced like in the client).
    pub density: crate::theme::Density,
    /// Consent / e-mail switches ([`synced_map::REST`]); missing = default.
    pub rest: std::collections::HashMap<&'static str, bool>,
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
    /// A [`synced_map`] entry set to a wire value.
    Proto(&'static str, u64),
    Density(crate::theme::Density),
    /// A [`synced_map::REST`] switch.
    Rest(&'static str, bool),
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
                // Entries sharing a field (flag bits) share the value.
                let path = synced_map::entry(id).map(|e| e.path);
                for e in synced_map::ENTRIES.iter().filter(|e| Some(e.path) == path) {
                    self.proto.insert(e.id, *v);
                }
            }
            SettingsPatch::Density(d) => self.density = *d,
        }
    }
    /// Synced switch value (client default when unset).
    pub fn flag(&self, id: &str) -> bool {
        if let Some((_, d)) = synced_map::rest(id) {
            return self.rest.get(id).copied().unwrap_or(d);
        }
        synced_map::get_bool(&self.proto, id)
    }
    /// Synced radio / select index (client default when unset).
    pub fn pick(&self, id: &str) -> usize {
        synced_map::get_pick(&self.proto, id)
    }
}

/// Client-only preferences.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalPrefs {
    /// Chat font size in px (Discord: 12..=24, default 16).
    pub font_size: f32,
    /// Zoom percent (50..=200, default 100).
    pub zoom: u32,
    pub desktop_notifications: bool,
    pub notification_sounds: bool,
    pub reduced_motion: bool,
    /// Accessibility "Play animated emoji" (client default on).
    pub animate_emoji: bool,
    pub saturation: u32,
    /// Windows/Linux settings "Minimize to tray" (client default on): the close
    /// button hides the window to the tray icon instead of quitting.
    pub minimize_to_tray: bool,
    /// Client-only controls without a dedicated field, by settings id
    /// (switch 0/1 or option index). Missing = client default.
    pub ui: std::collections::BTreeMap<String, u64>,
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
            ui: Default::default(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(crate::APP_ID).join("settings.json"))
}

impl LocalPrefs {
    /// Streamer Mode is on and hides personal info (email, phone).
    pub fn streamer_hides_info(&self) -> bool {
        self.flag("streamer", false) && self.flag("st-info", true)
    }
    /// Desktop notifications, muted by Streamer Mode "Disable notifications".
    pub fn notifications_on(&self) -> bool {
        self.desktop_notifications && !(self.flag("streamer", false) && self.flag("st-notifs", true))
    }
    /// Client-only switch `id` (see [`Self::ui`]).
    pub fn flag(&self, id: &str, default: bool) -> bool {
        self.ui.get(id).map(|v| *v != 0).unwrap_or(default)
    }
    /// Client-only option index `id`.
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
            ui: v
                .get("ui")
                .and_then(Value::as_object)
                .map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_u64()?))).collect())
                .unwrap_or_default(),
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
            "ui": self.ui,
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
        std::thread::spawn(move || {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, data).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        });
    }
}

/// Everything other views may read (compact mode, font size, dev mode...).
#[derive(Clone, Debug, Default)]
pub struct Prefs {
    pub synced: SyncedSettings,
    pub local: LocalPrefs,
}

impl Prefs {
    pub fn load() -> Self {
        let local = LocalPrefs::load();
        crate::notifications::set_enabled(local.notifications_on());
        Self { synced: SyncedSettings::default(), local }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    MyAccount,
    Profiles,
    /// "Données et confidentialité".
    DataPrivacy,
    /// "Permissions de messagerie" (content & social).
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
    /// "Jeux enregistrés" (Games & Apps).
    RegisteredGames,
    /// Billing section (vanilla sidebar): read-only pointers to discord.com.
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

/// Nav sections in Discord order.
/// Sidebar sections in the 2026 order (live settings-home dump): the
/// profile tab sits above them; the first section's label is visually hidden.
pub const NAV: &[(&str, &[Page])] = &[
    ("settings.nav.user", &[Page::MyAccount, Page::DataPrivacy, Page::PrivacySafety, Page::Notifications]),
    ("settings.nav.billing", &[Page::Nitro, Page::ServerBoost, Page::Subscriptions, Page::GiftInventory, Page::Billing]),
    ("settings.nav.app", &[Page::VoiceVideo, Page::Appearance, Page::Accessibility, Page::System, Page::Language]),
    ("settings.nav.apps", &[Page::RegisteredGames, Page::ActivityPrivacy, Page::Connections]),
    // last section: no label, "Développeur" then "Déconnexion"
    ("", &[Page::Advanced]),
];

#[derive(Default)]
pub struct SettingsState {
    pub page: Page,
    pub confirm_logout: bool,
    pub error: Option<SharedString>,
    pub reveal_email: bool,
    pub reveal_phone: bool,
    /// Session-only values of settings Phoenix doesn't back yet.
    pub toggles: std::collections::HashMap<&'static str, bool>,
    pub picks: std::collections::HashMap<&'static str, usize>,
    /// Open "Show advanced ..." expanders.
    pub expanded: std::collections::HashSet<&'static str>,
    /// Dropdown whose list is open.
    pub open_select: Option<&'static str>,
    /// Sidebar search box (created on first render).
    pub search: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub search_sub: Option<gpui::Subscription>,
    /// Full-screen profile editor (Page::Profiles).
    pub profile: Option<profile_editor::EditorState>,
    /// Content panel scroll (lets offscreen shots match scrolled dumps).
    pub scroll: gpui::ScrollHandle,
}

pub fn open(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if app.screen == crate::app::Screen::Main {
        app.settings = Some(SettingsState::default());
        account::fetch_sessions(app, cx);
        billing::fetch(app, cx);
        subscriptions::fetch(app, cx);
        authorized_apps::fetch(app, cx);
        cx.notify();
    }
}

pub fn open_action(app: &mut PhoenixApp, _: &OpenSettings, _: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    open(app, cx);
}

/// Esc: close the logout dialog, then the overlay. True when handled.
pub fn on_escape(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(s) = app.settings.as_mut() else { return false };
    if s.page == Page::Profiles {
        profile_editor::on_escape(s);
        cx.notify();
        return true;
    }
    if s.confirm_logout {
        s.confirm_logout = false;
    } else if s.open_select.is_some() {
        s.open_select = None;
    } else {
        app.settings = None;
    }
    cx.notify();
    true
}

/// After login: fetch the synced settings.
pub fn on_ready(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let fut = app.backend.load_settings();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            if let Ok(s) = res {
                crate::i18n::set_locale(&s.locale);
                this.prefs.synced = s;
                // Rows already rendered hold times / day dividers in the previous locale:
                // force the next chat_sync_style to rebuild them.
                this.chat.style_gen = u32::MAX;
                cx.notify();
            }
        });
    })
    .detach();
}

/// Optimistically apply then save a synced setting; reverts on failure.
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
                    // Backends that don't echo every entry (mock) keep ours.
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
}
