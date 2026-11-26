pub mod call;
pub mod prefs;
pub mod soundboard;
pub mod start_time;
pub mod stream;
pub mod ptt;

use std::collections::{HashSet, VecDeque};

pub use phoenix_voice::connection::ConnStats;

use crate::model::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceStatus {
    #[default]
    Idle,
    AwaitingEndpoint,
    Connecting,
    Connected,
    Reconnecting,
    NoRoute,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Connecting,
    Connected,
    Error,
}

impl VoiceStatus {
    pub fn label(self, video: bool) -> (&'static str, StatusKind) {
        match self {
            VoiceStatus::Connected if video => ("voice.video_connected", StatusKind::Connected),
            VoiceStatus::Connected => ("voice.connected", StatusKind::Connected),
            VoiceStatus::AwaitingEndpoint => ("voice.waiting_server", StatusKind::Connecting),
            VoiceStatus::Reconnecting => ("voice.connecting", StatusKind::Connecting),
            VoiceStatus::Connecting => ("voice.rtc_connecting", StatusKind::Connecting),
            VoiceStatus::NoRoute => ("voice.no_route", StatusKind::Error),
            VoiceStatus::Failed => ("voice.rtc_disconnected", StatusKind::Error),
            VoiceStatus::Idle => ("voice.disconnected", StatusKind::Error),
        }
    }

    pub fn from_disconnect(reason: &str) -> Self {
        if reason.contains("ip discovery") { VoiceStatus::NoRoute } else { VoiceStatus::Failed }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceUiEvent {
    Status(VoiceStatus),
    Ping(u32),
    Speaking { user: Id, speaking: bool },
    Encryption(bool),
    PrivacyCode(Option<String>),
    Stats(ConnStats),
    Server { endpoint: String, mode: String },
    Error(String),
    Stream(StreamUiEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamUiEvent {
    Live(Option<String>),
    Watching { key: String, on: bool },
    Preview { key: String, thumbnail: String },
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSource {
    TestPattern,
    Portal(stream::SourceKind),
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceControl {
    SetMute(bool),
    SetDeaf(bool),
    SetInputDevice(Option<String>),
    SetOutputDevice(Option<String>),
    SetSensitivity(Option<f32>),
    SetInputVolume(f32),
    SetOutputVolume(f32),
    SetUserVolume(Id, f32),
    SetNoiseSuppression(bool),
    SetEchoCancellation(bool),
    SetAgc(bool),
    SetPushToTalk(bool),
    SetPttKey(Option<String>),
    SetGlobalKeybinds(Vec<(String, String, String)>),
    SetPttPressed(bool),
    SetPttDelay(u32),
    SetCamera(bool),
    SetCameraDevice(Option<String>),
    StartStream { source: StreamSource, resolution: u32, fps: u32, sound: bool },
    UpdateStreamQuality { resolution: u32, fps: u32, sound: bool },
    StopStream,
    WatchStream(String),
    WatchAfterJoin(String),
    StopWatching(String),
    SetStreamVolume(Id, f32),
}

pub const PING_HISTORY: usize = 40;

#[derive(Debug, Clone, Default)]
pub struct VoiceUi {
    pub status: VoiceStatus,
    pub ping_ms: Option<u32>,
    pub speaking: HashSet<Id>,
    pub e2ee: bool,
    pub last_error: Option<String>,
    pub ping_history: VecDeque<u32>,
    pub privacy_code: Option<String>,
    pub stats: ConnStats,
    pub endpoint: Option<String>,
    pub mode: Option<String>,
    pub own_stream: Option<String>,
    pub watching: std::collections::BTreeSet<String>,
    pub stream_error: Option<String>,
}

impl VoiceUi {
    pub fn apply(&mut self, ev: VoiceUiEvent) {
        match ev {
            VoiceUiEvent::Status(s) => {
                self.status = s;
                if matches!(s, VoiceStatus::Idle | VoiceStatus::Failed | VoiceStatus::NoRoute) {
                    self.speaking.clear();
                    self.ping_ms = None;
                    self.e2ee = false;
                    self.ping_history.clear();
                    self.privacy_code = None;
                    self.stats = ConnStats::default();
                    self.endpoint = None;
                    self.mode = None;
                    self.own_stream = None;
                    self.watching.clear();
                }
                if s == VoiceStatus::Connected {
                    self.last_error = None;
                }
            }
            VoiceUiEvent::Ping(p) => {
                self.ping_ms = Some(p);
                if self.ping_history.len() >= PING_HISTORY {
                    self.ping_history.pop_front();
                }
                self.ping_history.push_back(p);
            }
            VoiceUiEvent::PrivacyCode(c) => self.privacy_code = c,
            VoiceUiEvent::Stats(s) => self.stats = s,
            VoiceUiEvent::Server { endpoint, mode } => {
                self.endpoint = Some(endpoint);
                self.mode = Some(mode);
            }
            VoiceUiEvent::Speaking { user, speaking } => {
                if speaking {
                    self.speaking.insert(user);
                } else {
                    self.speaking.remove(&user);
                }
            }
            VoiceUiEvent::Encryption(on) => {
                self.e2ee = on;
                if !on {
                    self.privacy_code = None;
                }
            }
            VoiceUiEvent::Error(e) => self.last_error = Some(e),
            VoiceUiEvent::Stream(e) => match e {
                StreamUiEvent::Live(k) => {
                    self.own_stream = k;
                    if self.own_stream.is_some() {
                        self.stream_error = None;
                    }
                }
                StreamUiEvent::Watching { key, on } => {
                    if on {
                        self.watching.insert(key);
                    } else {
                        self.watching.remove(&key);
                    }
                }
                StreamUiEvent::Preview { .. } => {}
                StreamUiEvent::Failed(e) => {
                    self.own_stream = None;
                    self.stream_error = Some(e);
                }
            },
        }
    }

    pub fn average_ping(&self) -> Option<u32> {
        let n = self.ping_history.len() as u32;
        (n > 0).then(|| self.ping_history.iter().sum::<u32>() / n)
    }

    pub fn inbound_loss_pct(&self) -> f32 {
        let s = self.stats;
        let total = s.packets_received + s.packets_lost;
        if total == 0 { 0. } else { s.packets_lost as f32 * 100. / total as f32 }
    }

    pub fn is_speaking(&self, user: Id) -> bool {
        self.speaking.contains(&user)
    }

    pub fn quality(&self) -> Quality {
        match self.ping_ms {
            None => Quality::Unknown,
            Some(p) if p < 150 => Quality::Good,
            Some(p) if p < 400 => Quality::Fair,
            Some(_) => Quality::Bad,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Unknown,
    Good,
    Fair,
    Bad,
}

pub fn on_join_clicked(app: &mut crate::app::PhoenixApp, channel: Option<Id>) {
    app.voice.apply(VoiceUiEvent::Status(if channel.is_some() { VoiceStatus::AwaitingEndpoint } else { VoiceStatus::Idle }));
    crate::sounds::watch::on_join_requested(channel);
}

pub fn sync_self_state(app: &crate::app::PhoenixApp) {
    drop(app.backend.voice_control(VoiceControl::SetMute(app.mic_muted || app.deafened)));
    drop(app.backend.voice_control(VoiceControl::SetDeaf(app.deafened)));
    let (m, d) = (app.mic_muted, app.deafened);
    prefs::update(|p| (p.self_mute, p.self_deaf) = (m, d), true);
    crate::sounds::watch::on_self_toggle(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_labels_follow_the_client() {
        assert_eq!(VoiceStatus::Connected.label(false), ("voice.connected", StatusKind::Connected));
        assert_eq!(VoiceStatus::Connected.label(true), ("voice.video_connected", StatusKind::Connected));
        assert_eq!(VoiceStatus::AwaitingEndpoint.label(false).0, "voice.waiting_server");
        assert_eq!(VoiceStatus::Connecting.label(false), ("voice.rtc_connecting", StatusKind::Connecting));
        assert_eq!(VoiceStatus::Idle.label(false), ("voice.disconnected", StatusKind::Error));
        assert_eq!(VoiceStatus::from_disconnect("timed out: ip discovery"), VoiceStatus::NoRoute);
        assert_eq!(VoiceStatus::from_disconnect("gateway closed (code Some(4006)): session"), VoiceStatus::Failed);
        assert_eq!(VoiceStatus::Failed.label(false), ("voice.rtc_disconnected", StatusKind::Error));
    }

    #[test]
    fn ui_state_follows_events() {
        let mut v = VoiceUi::default();
        v.apply(VoiceUiEvent::Status(VoiceStatus::Connected));
        v.apply(VoiceUiEvent::Ping(42));
        v.apply(VoiceUiEvent::Speaking { user: Id(5), speaking: true });
        assert!(v.is_speaking(Id(5)));
        assert_eq!(v.quality(), Quality::Good);
        v.apply(VoiceUiEvent::Speaking { user: Id(5), speaking: false });
        assert!(!v.is_speaking(Id(5)));
        v.apply(VoiceUiEvent::Speaking { user: Id(6), speaking: true });
        v.apply(VoiceUiEvent::Status(VoiceStatus::Idle));
        assert!(v.speaking.is_empty() && v.ping_ms.is_none());
        for p in 0..50 {
            v.apply(VoiceUiEvent::Ping(p));
        }
        assert_eq!(v.ping_history.len(), PING_HISTORY);
        assert_eq!(v.average_ping(), Some((10..50).sum::<u32>() / 40));
        v.apply(VoiceUiEvent::Stats(ConnStats { packets_received: 90, packets_lost: 10, ..Default::default() }));
        assert!((v.inbound_loss_pct() - 10.).abs() < 0.01);
    }
}
