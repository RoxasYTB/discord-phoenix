use std::cell::{Cell, RefCell};
use std::sync::{Mutex, OnceLock};

use gpui::{Context, KeyDownEvent, KeyUpEvent, SharedString};
use serde_json::{Value, json};

use crate::{
    app::PhoenixApp,
    model::Id,
    ui::settings::Page,
    voice::{VoiceControl, prefs::InputMode},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Unassigned,
    PushToTalk,
    PushToTalkPriority,
    PushToMute,
    VadPriority,
    ToggleMute,
    ToggleDeafen,
    ToggleVoiceMode,
    ToggleStreamerMode,
    ToggleVoiceChannelChat,
    ToggleCamera,
    NavigateBack,
    NavigateForward,
    SwitchToVoiceChannel,
    DisconnectFromVoiceChannel,
}

pub const ACTIONS: &[Action] = &[
    Action::Unassigned,
    Action::PushToTalk,
    Action::PushToTalkPriority,
    Action::PushToMute,
    Action::VadPriority,
    Action::ToggleMute,
    Action::ToggleDeafen,
    Action::ToggleVoiceMode,
    Action::ToggleStreamerMode,
    Action::ToggleVoiceChannelChat,
    Action::ToggleCamera,
    Action::NavigateBack,
    Action::NavigateForward,
    Action::SwitchToVoiceChannel,
    Action::DisconnectFromVoiceChannel,
];

impl Action {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unassigned => "UNASSIGNED",
            Self::PushToTalk => "PUSH_TO_TALK",
            Self::PushToTalkPriority => "PUSH_TO_TALK_PRIORITY",
            Self::PushToMute => "PUSH_TO_MUTE",
            Self::VadPriority => "VAD_PRIORITY",
            Self::ToggleMute => "TOGGLE_MUTE",
            Self::ToggleDeafen => "TOGGLE_DEAFEN",
            Self::ToggleVoiceMode => "TOGGLE_VOICE_MODE",
            Self::ToggleStreamerMode => "TOGGLE_STREAMER_MODE",
            Self::ToggleVoiceChannelChat => "TOGGLE_VOICE_CHANNEL_CHAT",
            Self::ToggleCamera => "TOGGLE_CAMERA",
            Self::NavigateBack => "NAVIGATE_BACK",
            Self::NavigateForward => "NAVIGATE_FORWARD",
            Self::SwitchToVoiceChannel => "SWITCH_TO_VOICE_CHANNEL",
            Self::DisconnectFromVoiceChannel => "DISCONNECT_FROM_VOICE_CHANNEL",
        }
    }

    pub fn from_code(s: &str) -> Self {
        ACTIONS.iter().copied().find(|a| a.code() == s).unwrap_or(Self::Unassigned)
    }

    fn key(self) -> &'static str {
        match self {
            Self::Unassigned => "unassigned",
            Self::PushToTalk => "ptt",
            Self::PushToTalkPriority => "ptt_priority",
            Self::PushToMute => "ptm",
            Self::VadPriority => "vad_priority",
            Self::ToggleMute => "mute",
            Self::ToggleDeafen => "deafen",
            Self::ToggleVoiceMode => "voice_mode",
            Self::ToggleStreamerMode => "streamer",
            Self::ToggleVoiceChannelChat => "voice_chat",
            Self::ToggleCamera => "camera",
            Self::NavigateBack => "back",
            Self::NavigateForward => "forward",
            Self::SwitchToVoiceChannel => "switch_voice",
            Self::DisconnectFromVoiceChannel => "disconnect",
        }
    }

    pub fn label(self) -> SharedString {
        crate::t!(&format!("kb.act.{}", self.key()))
    }

    pub fn description(self) -> Option<SharedString> {
        match self {
            Self::SwitchToVoiceChannel | Self::DisconnectFromVoiceChannel => None,
            a => Some(crate::t!(&format!("kb.desc.{}", a.key()))),
        }
    }

    pub fn hold(self) -> bool {
        matches!(self, Self::PushToTalk | Self::PushToTalkPriority | Self::PushToMute | Self::VadPriority)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keybind {
    pub id: u64,
    pub action: Action,
    pub shortcut: String,
    pub enabled: bool,
    pub channel: Option<u64>,
}

impl Keybind {
    pub fn to_json(&self) -> Value {
        json!({ "id": self.id, "action": self.action.code(), "shortcut": self.shortcut, "enabled": self.enabled, "channel_id": self.channel })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: v.get("id")?.as_u64()?,
            action: Action::from_code(v.get("action").and_then(Value::as_str).unwrap_or_default()),
            shortcut: v.get("shortcut").and_then(Value::as_str).unwrap_or_default().to_owned(),
            enabled: v.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            channel: v.get("channel_id").and_then(Value::as_u64),
        })
    }

    fn fires_on(&self, stroke: &str) -> bool {
        self.enabled && self.action != Action::Unassigned && !self.shortcut.is_empty() && self.shortcut == stroke
    }
}

pub fn add(list: &mut Vec<Keybind>) -> u64 {
    let id = list.iter().map(|k| k.id + 1).max().unwrap_or(0);
    list.push(Keybind { id, action: Action::Unassigned, shortcut: String::new(), enabled: true, channel: None });
    id
}

pub fn edit(app: &mut PhoenixApp, id: u64, f: impl FnOnce(&mut Keybind), cx: &mut Context<PhoenixApp>) {
    crate::ui::settings::update_local(app, |p| {
        if let Some(k) = p.keybinds.iter_mut().find(|k| k.id == id) {
            f(k);
        }
    }, cx);
}

thread_local! {
    static RECORDING: Cell<Option<u64>> = const { Cell::new(None) };
    static HELD: RefCell<Vec<Keybind>> = const { RefCell::new(Vec::new()) };
}

pub fn recording() -> Option<u64> {
    RECORDING.with(Cell::get)
}

pub fn set_recording(id: Option<u64>) {
    RECORDING.with(|r| r.set(id));
}

pub fn portal_id(id: u64) -> String {
    format!("kb-{id}")
}

type Bound = Option<Vec<(String, String)>>;

fn portal() -> &'static Mutex<Bound> {
    static P: OnceLock<Mutex<Bound>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

pub fn set_portal(bound: Option<Vec<(String, String)>>) {
    if let Ok(mut p) = portal().lock() {
        *p = bound;
    }
}

pub fn portal_active() -> bool {
    portal().lock().is_ok_and(|p| p.is_some())
}

pub fn portal_trigger(id: u64) -> Option<Option<String>> {
    let p = portal().lock().ok()?;
    let list = p.as_ref()?;
    let key = portal_id(id);
    Some(list.iter().find(|(i, _)| *i == key).map(|(_, d)| d.clone()))
}

fn portal_bound(id: u64) -> bool {
    matches!(portal_trigger(id), Some(Some(_)))
}

type GlobalTx = futures::channel::mpsc::UnboundedSender<(u64, bool)>;

fn global_tx() -> &'static Mutex<Option<GlobalTx>> {
    static T: OnceLock<Mutex<Option<GlobalTx>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(None))
}

pub fn global_event(id: &str, down: bool) {
    let Some(n) = id.strip_prefix("kb-").and_then(|n| n.parse::<u64>().ok()) else { return };
    if let Ok(tx) = global_tx().lock()
        && let Some(tx) = tx.as_ref()
    {
        let _ = tx.unbounded_send((n, down));
    }
}

pub fn listen(cx: &mut Context<PhoenixApp>) {
    let Ok(mut slot) = global_tx().lock() else { return };
    if slot.is_some() {
        return;
    }
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<(u64, bool)>();
    *slot = Some(tx);
    drop(slot);
    cx.spawn(async move |this, cx| {
        use futures::StreamExt;
        while let Some((id, down)) = rx.next().await {
            if this.update(cx, |app, cx| on_global(app, id, down, cx)).is_err() {
                break;
            }
        }
    })
    .detach();
}

pub fn global_list(list: &[Keybind]) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = list
        .iter()
        .filter(|b| b.enabled && b.action != Action::Unassigned && !b.shortcut.is_empty())
        .map(|b| (portal_id(b.id), b.action.label().to_string(), b.shortcut.clone()))
        .collect();
    out.sort();
    out
}

pub fn sync_global(app: &PhoenixApp) {
    thread_local! {
        static LAST: RefCell<Bound> = const { RefCell::new(None) };
    }
    if phoenix_api::rest::guard::agent_session() {
        return;
    }
    let list = global_list(&app.prefs.local.keybinds);
    let keys: Vec<(String, String)> = list.iter().map(|(i, _, k)| (i.clone(), k.clone())).collect();
    let changed = LAST.with(|l| {
        let mut l = l.borrow_mut();
        if l.as_ref() == Some(&keys) {
            return false;
        }
        *l = Some(keys);
        true
    });
    if changed {
        crate::voice::call::manager().apply_control(None, VoiceControl::SetGlobalKeybinds(list));
    }
}

fn on_global(app: &mut PhoenixApp, id: u64, down: bool, cx: &mut Context<PhoenixApp>) {
    if suspended(app) {
        return;
    }
    let Some(b) = app.prefs.local.keybinds.iter().find(|b| b.id == id && b.enabled).cloned() else { return };
    if b.action.hold() {
        hold(app, b.action, down);
    } else if !down {
        trigger(app, &b, cx);
    }
}

pub fn suspended(app: &PhoenixApp) -> bool {
    app.settings.as_ref().is_some_and(|s| matches!(s.page, Page::System | Page::Keybinds))
}

pub fn on_key_down(app: &mut PhoenixApp, ev: &KeyDownEvent, cx: &mut Context<PhoenixApp>) -> bool {
    if let Some(id) = recording() {
        set_recording(None);
        if ev.keystroke.key != "escape" {
            let k = crate::voice::ptt::stroke_string(&ev.keystroke);
            edit(app, id, |b| b.shortcut = k, cx);
        }
        cx.stop_propagation();
        cx.notify();
        return true;
    }
    if ev.is_held || suspended(app) {
        return false;
    }
    let stroke = crate::voice::ptt::stroke_string(&ev.keystroke);
    let hits: Vec<Keybind> = app.prefs.local.keybinds.iter().filter(|b| b.fires_on(&stroke) && !portal_bound(b.id)).cloned().collect();
    for b in hits {
        if b.action.hold() {
            hold(app, b.action, true);
        }
        HELD.with(|h| h.borrow_mut().push(b));
    }
    false
}

pub fn on_key_up(app: &mut PhoenixApp, ev: &KeyUpEvent, cx: &mut Context<PhoenixApp>) {
    let released: Vec<Keybind> = HELD.with(|h| {
        let mut h = h.borrow_mut();
        let (out, keep): (Vec<Keybind>, Vec<Keybind>) = h.drain(..).partition(|b| crate::voice::ptt::base_key(&b.shortcut) == ev.keystroke.key);
        *h = keep;
        out
    });
    for b in released {
        if b.action.hold() {
            hold(app, b.action, false);
        } else {
            trigger(app, &b, cx);
        }
    }
}

fn hold(app: &PhoenixApp, a: Action, down: bool) {
    let mode = crate::voice::prefs::prefs().lock().input_mode;
    match a {
        Action::PushToTalk | Action::PushToTalkPriority if mode == InputMode::PushToTalk => {
            crate::voice::call::manager().apply_control(None, VoiceControl::SetPttPressed(down));
            crate::sounds::watch::on_ptt(app, down);
        }
        Action::PushToMute if mode == InputMode::VoiceActivity && app.voice_channel.is_some() => {
            drop(app.backend.voice_control(VoiceControl::SetMute(down || app.mic_muted || app.deafened)));
        }
        _ => {}
    }
}

fn trigger(app: &mut PhoenixApp, b: &Keybind, cx: &mut Context<PhoenixApp>) {
    match b.action {
        Action::ToggleMute => {
            app.mic_muted = !app.mic_muted;
            crate::voice::sync_self_state(app);
        }
        Action::ToggleDeafen => {
            app.deafened = !app.deafened;
            crate::voice::sync_self_state(app);
        }
        Action::ToggleVoiceMode => crate::voice::prefs::update(
            |p| p.input_mode = if p.input_mode == InputMode::PushToTalk { InputMode::VoiceActivity } else { InputMode::PushToTalk },
            true,
        ),
        Action::ToggleStreamerMode => {
            let on = crate::streamer::enabled(&app.prefs.local);
            crate::ui::settings::update_local(app, |p| {
                p.ui.insert("streamer".into(), u64::from(!on));
            }, cx);
        }
        Action::ToggleVoiceChannelChat if app.voice_channel.is_some() => {
            crate::ui::call_view::set_chat_open(!crate::ui::call_view::chat_open());
        }
        Action::ToggleCamera if app.voice_channel.is_some() => {
            let on = !crate::ui::voice_panel::flags(app.me.id).video;
            drop(app.backend.voice_control(VoiceControl::SetCamera(on)));
        }
        Action::NavigateBack => app.history_step(true, cx),
        Action::NavigateForward => app.history_step(false, cx),
        Action::SwitchToVoiceChannel => {
            if let Some(ch) = b.channel.map(Id).filter(|c| app.channels.contains_key(c) && app.voice_channel != Some(*c)) {
                app.select_channel(ch, cx);
            }
        }
        Action::DisconnectFromVoiceChannel if app.voice_channel.is_some() => app.join_voice(None, cx),
        _ => return,
    }
    cx.notify();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybinds_roundtrip_and_ids_follow_the_client() {
        let mut list = Vec::new();
        assert_eq!(add(&mut list), 0);
        assert_eq!(add(&mut list), 1);
        assert_eq!(list[1].action, Action::Unassigned);
        assert!(list[1].enabled && list[1].shortcut.is_empty());
        list[0] = Keybind { id: 0, action: Action::SwitchToVoiceChannel, shortcut: "ctrl-shift-v".into(), enabled: false, channel: Some(42) };
        for k in &list {
            assert_eq!(Keybind::from_json(&k.to_json()).as_ref(), Some(k));
        }
        for a in ACTIONS {
            assert_eq!(Action::from_code(a.code()), *a);
        }
        assert_eq!(Action::from_code("TOGGLE_OVERLAY"), Action::Unassigned);
    }

    #[test]
    fn global_list_keeps_live_bindings_and_routes_portal_ids() {
        let k = |id, action, shortcut: &str, enabled| Keybind { id, action, shortcut: shortcut.into(), enabled, channel: None };
        let list = vec![k(3, Action::ToggleMute, "ctrl-m", true), k(1, Action::Unassigned, "f9", true), k(2, Action::ToggleDeafen, "", true), k(4, Action::PushToMute, "f10", false), k(0, Action::PushToTalk, "f8", true)];
        let ids: Vec<String> = global_list(&list).into_iter().map(|(i, _, _)| i).collect();
        assert_eq!(ids, ["kb-0", "kb-3"]);
        set_portal(Some(vec![("kb-3".into(), "Ctrl+M".into())]));
        assert_eq!(portal_trigger(3), Some(Some("Ctrl+M".into())));
        assert_eq!(portal_trigger(0), Some(None));
        assert!(portal_bound(3) && !portal_bound(0));
        set_portal(None);
        assert_eq!(portal_trigger(3), None);
    }

    #[test]
    fn only_enabled_assigned_bindings_fire() {
        let mut k = Keybind { id: 0, action: Action::ToggleMute, shortcut: "ctrl-m".into(), enabled: true, channel: None };
        assert!(k.fires_on("ctrl-m"));
        assert!(!k.fires_on("m"));
        k.enabled = false;
        assert!(!k.fires_on("ctrl-m"));
        k.enabled = true;
        k.action = Action::Unassigned;
        assert!(!k.fires_on("ctrl-m"));
    }
}
