//! Voice calls in the app.
//!
//! - UI side (this file): [`VoiceUi`] state kept by `PhoenixApp`, updated
//!   from [`VoiceUiEvent`]s (delivered as `AppEvent::Voice`), and the
//!   [`VoiceControl`] commands the UI sends through `Backend::voice_control`.
//! - Network side ([`call`]): joins through gateway op 4, waits for our
//!   VOICE_STATE_UPDATE (session id) and VOICE_SERVER_UPDATE (endpoint,
//!   token), then runs a `phoenix_voice` connection with the cpal audio engine.

pub mod call;
pub mod prefs;
pub mod start_time;
pub mod ptt;

use std::collections::{HashSet, VecDeque};

pub use phoenix_voice::connection::ConnStats;

use crate::model::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceStatus {
    #[default]
    Idle,
    /// Waiting for the voice server / handshake.
    Connecting,
    Connected,
    Reconnecting,
    /// Connection lost; the message explains why.
    Failed,
}

/// Events from the voice runtime for the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceUiEvent {
    Status(VoiceStatus),
    Ping(u32),
    Speaking { user: Id, speaking: bool },
    /// DAVE end-to-end encryption on/off.
    Encryption(bool),
    /// DAVE voice privacy code (30 digits, grouped by 5).
    PrivacyCode(Option<String>),
    Stats(ConnStats),
    /// Voice server host and transport encryption mode.
    Server { endpoint: String, mode: String },
    Error(String),
}

/// Commands from the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceControl {
    SetMute(bool),
    SetDeaf(bool),
    /// `None` = system default device.
    SetInputDevice(Option<String>),
    SetOutputDevice(Option<String>),
    /// `None` = automatic input sensitivity, else threshold in dBFS.
    SetSensitivity(Option<f32>),
    SetInputVolume(f32),
    SetOutputVolume(f32),
    SetUserVolume(Id, f32),
    SetNoiseSuppression(bool),
    SetEchoCancellation(bool),
    SetAgc(bool),
    /// Push-to-talk instead of voice activity.
    SetPushToTalk(bool),
    /// Global push-to-talk key (GPUI keystroke); `None` unbinds.
    SetPttKey(Option<String>),
    SetPttPressed(bool),
    SetPttDelay(u32),
}

/// Ping samples kept for the connection graph.
pub const PING_HISTORY: usize = 40;

/// Voice state shown by the UI.
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
}

impl VoiceUi {
    pub fn apply(&mut self, ev: VoiceUiEvent) {
        match ev {
            VoiceUiEvent::Status(s) => {
                self.status = s;
                if matches!(s, VoiceStatus::Idle | VoiceStatus::Failed) {
                    self.speaking.clear();
                    self.ping_ms = None;
                    self.e2ee = false;
                    self.ping_history.clear();
                    self.privacy_code = None;
                    self.stats = ConnStats::default();
                    self.endpoint = None;
                    self.mode = None;
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
        }
    }

    pub fn average_ping(&self) -> Option<u32> {
        let n = self.ping_history.len() as u32;
        (n > 0).then(|| self.ping_history.iter().sum::<u32>() / n)
    }

    /// Inbound packet loss in percent.
    pub fn inbound_loss_pct(&self) -> f32 {
        let s = self.stats;
        let total = s.packets_received + s.packets_lost;
        if total == 0 { 0. } else { s.packets_lost as f32 * 100. / total as f32 }
    }

    pub fn is_speaking(&self, user: Id) -> bool {
        self.speaking.contains(&user)
    }

    /// Discord's connection quality colour buckets.
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

/// `PhoenixApp::join_voice` hook: show "RTC Connecting" right away.
pub fn on_join_clicked(app: &mut crate::app::PhoenixApp, channel: Option<Id>) {
    app.voice.apply(VoiceUiEvent::Status(if channel.is_some() { VoiceStatus::Connecting } else { VoiceStatus::Idle }));
    crate::sounds::watch::on_join_requested(channel);
}

/// Push the user panel's mute/deafen toggles to the voice runtime.
pub fn sync_self_state(app: &crate::app::PhoenixApp) {
    // The real backend applies controls synchronously; the future is a no-op.
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
