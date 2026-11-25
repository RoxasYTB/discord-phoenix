use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures::{StreamExt, channel::mpsc};
use gpui::{App, Global, WeakEntity};
use zbus::object_server::SignalEmitter;

pub mod activation;

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrayState {
    pub connected: bool,
    pub muted: bool,
    pub deafened: bool,
    pub speaking: bool,
    pub unread: bool,
    pub count: u32,
}

impl TrayState {
    pub fn icon(&self) -> &'static str {
        if self.connected {
            if self.deafened {
                "tray-deafened"
            } else if self.muted {
                "tray-muted"
            } else if self.speaking {
                "tray-speaking"
            } else {
                "tray-connected"
            }
        } else if self.unread {
            "tray-unread"
        } else {
            "tray"
        }
    }

    fn from_app(a: &crate::app::PhoenixApp) -> Self {
        let connected = a.voice_channel.is_some();
        let dms = || a.channels.values().filter(|c| c.guild.is_none());
        let mentions = a.guilds.iter().map(|g| g.mentions).sum::<u32>() + dms().map(|c| c.mentions).sum::<u32>();
        let pending = a.nav.count(crate::ui::home::data::RelationshipKind::Incoming) as u32;
        let any_unread = a.guilds.iter().any(|g| g.unread) || dms().any(|c| c.unread);
        Self {
            connected,
            muted: a.mic_muted || a.deafened,
            deafened: a.deafened,
            speaking: connected && crate::voice::call::manager().control().transmitting(),
            unread: unread_icon(a.prefs.local.flag("badge", true), any_unread, mentions, pending),
            count: mentions + pending,
        }
    }
}

pub fn unread_icon(badge: bool, any_unread: bool, mentions: u32, pending: u32) -> bool {
    badge && (any_unread || mentions > 0 || pending > 0)
}

pub const LAUNCHER_PATH: &str = "/com/canonical/unity/launcherentry/discord_phoenix";

pub fn launcher_uri() -> String {
    format!("application://{}.desktop", crate::APP_ID)
}

pub fn launcher_props(count: u32) -> HashMap<&'static str, zbus::zvariant::Value<'static>> {
    HashMap::from([("count", zbus::zvariant::Value::from(i64::from(count))), ("count-visible", zbus::zvariant::Value::from(count > 0))])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmd {
    Open,
    Quit,
    ToggleMute,
    ToggleDeafen,
    VoiceSettings,
    Acknowledgements,
}

pub struct Target(pub WeakEntity<crate::app::PhoenixApp>);

impl Global for Target {}

pub fn set_target(cx: &mut App, app: WeakEntity<crate::app::PhoenixApp>) {
    cx.set_global(Target(app));
}

pub const ICON_NAMES: [&str; 6] = ["tray", "tray-unread", "tray-connected", "tray-speaking", "tray-muted", "tray-deafened"];

type Pixmap = Vec<(i32, i32, Vec<u8>)>;

struct Labels {
    open: String,
    mute: String,
    deafen: String,
    voice_settings: String,
    acknowledgements: String,
    quit: String,
}

fn client_tray_dirs() -> Vec<PathBuf> {
    let Some(home) = crate::paths::home() else { return Vec::new() };
    let mut out = Vec::new();
    for root in [home.join(".config/discord"), home.join(".var/app/com.discordapp.Discord/config/discord")] {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        versions.sort();
        for v in versions.into_iter().rev() {
            let Ok(mods) = std::fs::read_dir(v.join("modules")) else { continue };
            for m in mods.flatten() {
                let dir = m.path().join("discord_desktop_core/app/images/systemtray/linux");
                if dir.join("tray.png").is_file() {
                    out.push(dir);
                }
            }
        }
    }
    out
}

fn argb(img: &image::RgbaImage) -> (i32, i32, Vec<u8>) {
    let data = img.pixels().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
    (img.width() as i32, img.height() as i32, data)
}

fn load_png(path: &Path) -> Option<image::RgbaImage> {
    let bytes = std::fs::read(path).ok()?;
    Some(image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8())
}

fn app_pixmap() -> Pixmap {
    let Ok(img) = image::load_from_memory_with_format(crate::assets::APP_ICON_PNG, image::ImageFormat::Png) else {
        return Vec::new();
    };
    let rgba = img.to_rgba8();
    [32u32, 64].iter().map(|&s| argb(&image::imageops::resize(&rgba, s, s, image::imageops::FilterType::Lanczos3))).collect()
}

fn icons() -> HashMap<&'static str, Pixmap> {
    let fallback = app_pixmap();
    let dir = client_tray_dirs().into_iter().next();
    ICON_NAMES
        .iter()
        .map(|&n| {
            let px = dir.as_ref().and_then(|d| load_png(&d.join(format!("{n}.png")))).map(|i| vec![argb(&i)]).unwrap_or_else(|| fallback.clone());
            (n, px)
        })
        .collect()
}

pub fn spawn(cx: &mut App) {
    if !cfg!(target_os = "linux") || std::env::var_os("PHOENIX_NO_TRAY").is_some() {
        return;
    }
    let (tx, mut rx) = mpsc::unbounded::<Cmd>();
    let (state_tx, state_rx) = mpsc::unbounded::<TrayState>();
    let labels = Labels {
        open: crate::t!("tray.open").to_string(),
        mute: crate::t!("tray.mute").to_string(),
        deafen: crate::t!("tray.deafen").to_string(),
        voice_settings: crate::t!("tray.voice_settings").to_string(),
        acknowledgements: crate::t!("tray.acknowledgements").to_string(),
        quit: crate::t!("tray.quit").to_string(),
    };
    let spawned = std::thread::Builder::new().name("phoenix-tray".into()).spawn(move || {
        let icons = icons();
        if let Err(e) = zbus::block_on(serve(tx, state_rx, labels, icons)) {
            tracing::warn!("tray icon unavailable: {e}");
        }
        RUNNING.store(false, Ordering::Relaxed);
    });
    if spawned.is_err() {
        return;
    }
    cx.spawn(async move |cx| {
        while let Some(cmd) = rx.next().await {
            cx.update(|cx| handle(cmd, cx));
            if cmd == Cmd::Quit {
                break;
            }
        }
    })
    .detach();
    cx.spawn(async move |cx| {
        let mut last: Option<TrayState> = None;
        loop {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let st = cx.update(|cx| {
                let t = cx.try_global::<Target>()?.0.upgrade()?;
                Some(TrayState::from_app(t.read(cx)))
            });
            if let Some(st) = st
                && last != Some(st)
            {
                last = Some(st);
                if state_tx.unbounded_send(st).is_err() {
                    break;
                }
            }
        }
    })
    .detach();
}

fn activate(cx: &mut App) {
    let mut token = activation::take();
    for w in cx.windows() {
        let _ = w.update(cx, |_, window, _| {
            let granted = token.as_deref().is_some_and(|t| activation::activate(window, t));
            if granted {
                token = None;
            } else {
                window.activate_window();
            }
        });
    }
}

fn handle(cmd: Cmd, cx: &mut App) {
    let target = cx.try_global::<Target>().and_then(|t| t.0.upgrade());
    match cmd {
        Cmd::Quit => cx.quit(),
        Cmd::Open => activate(cx),
        Cmd::Acknowledgements => cx.open_url("https://discord.com/acknowledgements"),
        Cmd::ToggleMute | Cmd::ToggleDeafen => {
            if let Some(t) = target {
                t.update(cx, |a, cx| {
                    if cmd == Cmd::ToggleMute {
                        a.mic_muted = !a.mic_muted;
                    } else {
                        a.deafened = !a.deafened;
                    }
                    crate::voice::sync_self_state(a);
                    cx.notify();
                });
            }
        }
        Cmd::VoiceSettings => {
            activate(cx);
            if let Some(t) = target {
                t.update(cx, |a, cx| {
                    crate::ui::settings::open(a, cx);
                    if let Some(s) = a.settings.as_mut() {
                        s.page = crate::ui::settings::Page::VoiceVideo;
                    }
                    cx.notify();
                });
            }
        }
    }
}

async fn serve(
    tx: mpsc::UnboundedSender<Cmd>,
    mut states: mpsc::UnboundedReceiver<TrayState>,
    labels: Labels,
    icons: HashMap<&'static str, Pixmap>,
) -> zbus::Result<()> {
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let conn = zbus::connection::Builder::session()?
        .name(name.as_str())?
        .serve_at("/StatusNotifierItem", Item { tx: tx.clone(), icons, state: TrayState::default() })?
        .serve_at("/MenuBar", Menu { tx, labels, state: TrayState::default(), revision: 1 })?
        .build()
        .await?;
    conn.call_method(
        Some("org.kde.StatusNotifierWatcher"),
        "/StatusNotifierWatcher",
        Some("org.kde.StatusNotifierWatcher"),
        "RegisterStatusNotifierItem",
        &(name.as_str(),),
    )
    .await?;
    RUNNING.store(true, Ordering::Relaxed);
    let mut count = 0;
    while let Some(st) = states.next().await {
        if st.count != count {
            count = st.count;
            let _ = conn.emit_signal(None::<&str>, LAUNCHER_PATH, "com.canonical.Unity.LauncherEntry", "Update", &(launcher_uri(), launcher_props(count))).await;
        }
        let item = conn.object_server().interface::<_, Item>("/StatusNotifierItem").await?;
        let icon_changed = {
            let mut i = item.get_mut().await;
            let changed = i.state.icon() != st.icon();
            i.state = st;
            changed
        };
        if icon_changed {
            Item::new_icon(item.signal_emitter()).await?;
        }
        let menu = conn.object_server().interface::<_, Menu>("/MenuBar").await?;
        let rev = {
            let mut m = menu.get_mut().await;
            if m.state == st {
                continue;
            }
            m.state = st;
            m.revision += 1;
            m.revision
        };
        Menu::layout_updated(menu.signal_emitter(), rev, 0).await?;
    }
    Ok(())
}

struct Item {
    tx: mpsc::UnboundedSender<Cmd>,
    icons: HashMap<&'static str, Pixmap>,
    state: TrayState,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.unbounded_send(Cmd::Open);
    }
    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.unbounded_send(Cmd::Open);
    }
    fn context_menu(&self, _x: i32, _y: i32) {}
    fn scroll(&self, _delta: i32, _orientation: String) {}
    fn provide_xdg_activation_token(&self, token: String) {
        activation::provide(token);
    }
    #[zbus(signal)]
    async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(property)]
    fn category(&self) -> String {
        "Communications".into()
    }
    #[zbus(property)]
    fn id(&self) -> String {
        crate::APP_ID.into()
    }
    #[zbus(property)]
    fn title(&self) -> String {
        "Discord Phoenix".into()
    }
    #[zbus(property)]
    fn status(&self) -> String {
        "Active".into()
    }
    #[zbus(property)]
    fn icon_name(&self) -> String {
        "".into()
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Pixmap {
        self.icons.get(self.state.icon()).or_else(|| self.icons.get("tray")).cloned().unwrap_or_default()
    }
    #[zbus(property)]
    fn tool_tip(&self) -> (String, Pixmap, String, String) {
        (String::new(), Vec::new(), "Discord Phoenix".into(), String::new())
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn menu(&self) -> zbus::zvariant::OwnedObjectPath {
        zbus::zvariant::OwnedObjectPath::try_from("/MenuBar").expect("static object path")
    }
}

struct Menu {
    tx: mpsc::UnboundedSender<Cmd>,
    labels: Labels,
    state: TrayState,
    revision: u32,
}

const ID_OPEN: i32 = 1;
const ID_SEP_VOICE: i32 = 2;
const ID_MUTE: i32 = 3;
const ID_DEAFEN: i32 = 4;
const ID_VOICE_SETTINGS: i32 = 5;
const ID_SEP_META: i32 = 6;
const ID_ACK: i32 = 7;
const ID_SEP_QUIT: i32 = 8;
const ID_QUIT: i32 = 9;

type Props = HashMap<String, zbus::zvariant::OwnedValue>;

fn props(pairs: &[(&str, zbus::zvariant::Value<'_>)]) -> Props {
    pairs
        .iter()
        .filter_map(|(k, v)| Some(((*k).to_string(), zbus::zvariant::OwnedValue::try_from(v.try_clone().ok()?).ok()?)))
        .collect()
}

pub fn menu_ids(state: &TrayState) -> Vec<i32> {
    let mut ids = vec![ID_OPEN];
    if state.connected {
        ids.extend([ID_SEP_VOICE, ID_MUTE, ID_DEAFEN, ID_VOICE_SETTINGS]);
    }
    ids.extend([ID_SEP_META, ID_ACK, ID_SEP_QUIT, ID_QUIT]);
    ids
}

impl Menu {
    fn item_props(&self, id: i32) -> Props {
        use zbus::zvariant::Value;
        let check = |label: &str, on: bool| {
            props(&[("label", Value::from(label)), ("toggle-type", Value::from("checkmark")), ("toggle-state", Value::from(i32::from(on)))])
        };
        match id {
            ID_OPEN => props(&[("label", Value::from(self.labels.open.as_str()))]),
            ID_SEP_VOICE | ID_SEP_META | ID_SEP_QUIT => props(&[("type", Value::from("separator"))]),
            ID_MUTE => check(&self.labels.mute, self.state.muted),
            ID_DEAFEN => check(&self.labels.deafen, self.state.deafened),
            ID_VOICE_SETTINGS => props(&[("label", Value::from(self.labels.voice_settings.as_str()))]),
            ID_ACK => props(&[("label", Value::from(self.labels.acknowledgements.as_str()))]),
            ID_QUIT => props(&[("label", Value::from(self.labels.quit.as_str()))]),
            _ => props(&[("children-display", Value::from("submenu"))]),
        }
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(
        &self,
        _parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> (u32, (i32, Props, Vec<zbus::zvariant::OwnedValue>)) {
        let children = menu_ids(&self.state)
            .into_iter()
            .filter_map(|id| {
                let node = (id, self.item_props(id), Vec::<zbus::zvariant::OwnedValue>::new());
                zbus::zvariant::OwnedValue::try_from(zbus::zvariant::Value::from(zbus::zvariant::Structure::from(node))).ok()
            })
            .collect();
        (self.revision, (0, self.item_props(0), children))
    }

    fn get_group_properties(&self, ids: Vec<i32>, _property_names: Vec<String>) -> Vec<(i32, Props)> {
        ids.into_iter().map(|id| (id, self.item_props(id))).collect()
    }

    fn event(&self, id: i32, event_id: String, _data: zbus::zvariant::OwnedValue, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }
        let cmd = match id {
            ID_OPEN => Cmd::Open,
            ID_MUTE => Cmd::ToggleMute,
            ID_DEAFEN => Cmd::ToggleDeafen,
            ID_VOICE_SETTINGS => Cmd::VoiceSettings,
            ID_ACK => Cmd::Acknowledgements,
            ID_QUIT => Cmd::Quit,
            _ => return,
        };
        let _ = self.tx.unbounded_send(cmd);
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    #[zbus(signal)]
    async fn layout_updated(emitter: &SignalEmitter<'_>, revision: u32, parent: i32) -> zbus::Result<()>;

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    fn status(&self) -> String {
        "normal".into()
    }
    #[zbus(property)]
    fn text_direction(&self) -> String {
        "ltr".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_follows_the_client_priority() {
        let mut s = TrayState::default();
        assert_eq!(s.icon(), "tray");
        s.unread = true;
        assert_eq!(s.icon(), "tray-unread");
        s.connected = true;
        assert_eq!(s.icon(), "tray-connected");
        s.speaking = true;
        assert_eq!(s.icon(), "tray-speaking");
        s.muted = true;
        assert_eq!(s.icon(), "tray-muted");
        s.deafened = true;
        assert_eq!(s.icon(), "tray-deafened");
    }

    #[test]
    fn menu_shows_voice_rows_only_when_connected() {
        let idle = menu_ids(&TrayState::default());
        assert_eq!(idle, vec![ID_OPEN, ID_SEP_META, ID_ACK, ID_SEP_QUIT, ID_QUIT]);
        let call = menu_ids(&TrayState { connected: true, ..Default::default() });
        assert_eq!(call, vec![ID_OPEN, ID_SEP_VOICE, ID_MUTE, ID_DEAFEN, ID_VOICE_SETTINGS, ID_SEP_META, ID_ACK, ID_SEP_QUIT, ID_QUIT]);
    }

    #[test]
    fn unread_icon_follows_the_badge_setting() {
        assert!(unread_icon(true, true, 0, 0));
        assert!(unread_icon(true, false, 2, 0));
        assert!(unread_icon(true, false, 0, 1));
        assert!(!unread_icon(true, false, 0, 0));
        assert!(!unread_icon(false, true, 3, 1));
    }

    #[test]
    fn launcher_entry_carries_the_count() {
        assert_eq!(launcher_uri(), "application://discord-phoenix.desktop");
        let p = launcher_props(4);
        assert_eq!(p["count"], zbus::zvariant::Value::from(4i64));
        assert_eq!(p["count-visible"], zbus::zvariant::Value::from(true));
        assert_eq!(launcher_props(0)["count-visible"], zbus::zvariant::Value::from(false));
    }
}
