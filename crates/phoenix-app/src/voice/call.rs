use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use phoenix_api::{Cache, GatewayEvent, GatewayHandle, models::Snowflake};
use phoenix_voice::audio::AudioControl;
use phoenix_voice::audio::device::{AudioEngine, PlaybackSink};
use phoenix_voice::connection::{self, AudioOut, VoiceConfig, VoiceConnection, VoiceEvent};
use phoenix_voice::gateway::VoiceConnectInfo;
use tokio::sync::mpsc;

use super::{VoiceControl, VoiceStatus, VoiceUiEvent};
use crate::backend::AppEvent;
use crate::model::Id;

pub type Emit = Arc<dyn Fn(AppEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Target {
    guild: Option<u64>,
    channel: u64,
}

struct Running {
    conn: VoiceConnection,
    engine: Option<AudioEngine>,
    out_tx: mpsc::UnboundedSender<AudioOut>,
    sink: Arc<Mutex<Option<PlaybackSink>>>,
}

#[derive(Default)]
struct State {
    target: Option<Target>,
    session_id: Option<String>,
    server: Option<(String, String)>,
    running: Option<Running>,
    generation: u64,
    input: Option<String>,
    output: Option<String>,
    muted: bool,
    deafened: bool,
    ptt_key: Option<String>,
    #[cfg(target_os = "linux")]
    ptt_portal: Option<phoenix_voice::hotkey::GlobalPtt>,
}

pub struct VoiceManager {
    state: Mutex<State>,
    control: Arc<AudioControl>,
}

pub fn manager() -> &'static VoiceManager {
    static M: OnceLock<VoiceManager> = OnceLock::new();
    static INIT: std::sync::Once = std::sync::Once::new();
    let m = M.get_or_init(|| VoiceManager { state: Mutex::new(State::default()), control: Arc::new(AudioControl::default()) });
    INIT.call_once(|| {
        let controls = super::prefs::prefs().lock().controls();
        for c in controls {
            m.apply_control(None, c);
        }
    });
    m
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PortalState {
    #[default]
    Off,
    Pending,
    Bound(String),
    Unavailable(String),
}

pub fn portal_state() -> &'static Mutex<PortalState> {
    static P: OnceLock<Mutex<PortalState>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(PortalState::Off))
}

fn ui(emit: &Emit, e: VoiceUiEvent) {
    emit(AppEvent::Voice(e));
}

fn stop(st: &mut State) {
    if let Some(r) = st.running.take() {
        r.conn.close();
        if let Some(e) = r.engine {
            std::thread::spawn(move || e.stop());
        }
    }
}

fn start_engine(control: &Arc<AudioControl>, st: &State, out_tx: mpsc::UnboundedSender<AudioOut>) -> Option<AudioEngine> {
    match AudioEngine::start(control.clone(), st.input.clone(), st.output.clone(), out_tx) {
        Ok(e) => Some(e),
        Err(e) => {
            eprintln!("discord-phoenix: audio devices unavailable: {e}");
            None
        }
    }
}

impl VoiceManager {
    pub fn control(&self) -> &Arc<AudioControl> {
        &self.control
    }

    pub fn join(&self, emit: &Emit, gw: &GatewayHandle, cache: &Cache, channel: Option<u64>) -> Result<(), String> {
        let mut st = self.state.lock();
        stop(&mut st);
        st.generation += 1;
        st.session_id = None;
        st.server = None;
        let Some(ch) = channel else {
            if let Some(t) = st.target.take() {
                gw.update_voice_state(t.guild.map(Snowflake), None, st.muted, st.deafened).map_err(|e| e.to_string())?;
            }
            ui(emit, VoiceUiEvent::Status(VoiceStatus::Idle));
            return Ok(());
        };
        let guild = cache.channel(Snowflake(ch)).and_then(|c| c.guild_id).map(|g| g.0);
        st.target = Some(Target { guild, channel: ch });
        gw.update_voice_state(guild.map(Snowflake), Some(Snowflake(ch)), st.muted, st.deafened).map_err(|e| e.to_string())?;
        ui(emit, VoiceUiEvent::Status(VoiceStatus::Connecting));
        Ok(())
    }

    pub fn on_gateway(&self, emit: &Emit, me: u64, ev: &GatewayEvent) {
        let mut st = self.state.lock();
        let Some(target) = st.target else { return };
        match ev {
            GatewayEvent::VoiceStateUpdate(v) if v.user_id.0 == me => match v.channel_id {
                None => {
                    stop(&mut st);
                    st.target = None;
                    st.generation += 1;
                    ui(emit, VoiceUiEvent::Status(VoiceStatus::Idle));
                }
                Some(c) => {
                    if c.0 != target.channel {
                        st.target = Some(Target { guild: target.guild, channel: c.0 });
                        stop(&mut st);
                        st.generation += 1;
                    }
                    if st.session_id.as_deref() != Some(v.session_id.as_str()) {
                        st.session_id = Some(v.session_id.clone());
                    }
                    self.try_start(&mut st, emit, me);
                }
            },
            GatewayEvent::VoiceServerUpdate(s) => {
                let ours = match target.guild {
                    Some(g) => s.guild_id.is_some_and(|x| x.0 == g),
                    None => s.channel_id.is_some_and(|x| x.0 == target.channel) || s.guild_id.is_none(),
                };
                if !ours {
                    return;
                }
                match &s.endpoint {
                    Some(ep) => {
                        let new = Some((ep.clone(), s.token.clone()));
                        if st.server != new || st.running.is_none() {
                            stop(&mut st);
                            st.generation += 1;
                            st.server = new;
                            self.try_start(&mut st, emit, me);
                        }
                    }
                    None => {
                        stop(&mut st);
                        st.server = None;
                        ui(emit, VoiceUiEvent::Status(VoiceStatus::Reconnecting));
                    }
                }
            }
            _ => {}
        }
    }

    fn try_start(&self, st: &mut State, emit: &Emit, me: u64) {
        if st.running.is_some() {
            return;
        }
        let (Some(t), Some(session_id), Some((endpoint, token))) = (st.target, st.session_id.clone(), st.server.clone()) else {
            return;
        };
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let engine = start_engine(&self.control, st, out_tx.clone());
        let sink = Arc::new(Mutex::new(engine.as_ref().map(|e| e.sink())));
        let info = VoiceConnectInfo {
            endpoint,
            server_id: t.guild.unwrap_or(t.channel).to_string(),
            user_id: me.to_string(),
            session_id,
            token,
            max_dave_protocol_version: phoenix_voice::dave::MAX_PROTOCOL_VERSION,
        };
        let endpoint = info.endpoint.split(':').next().unwrap_or_default().to_string();
        let s2 = sink.clone();
        let (conn, mut events) = connection::connect(
            VoiceConfig { info, channel_id: t.channel, user_id: me },
            out_rx,
            Box::new(move |a| {
                if let Some(s) = s2.lock().as_ref() {
                    s.push(a);
                }
            }),
        );
        st.running = Some(Running { conn, engine, out_tx, sink });
        let generation = st.generation;
        let emit = emit.clone();
        tokio::spawn(async move {
            while let Some(ev) = events.recv().await {
                if manager().state.lock().generation != generation {
                    return;
                }
                let e = match ev {
                    VoiceEvent::Connected { mode, .. } => {
                        ui(&emit, VoiceUiEvent::Server { endpoint: endpoint.clone(), mode });
                        VoiceUiEvent::Status(VoiceStatus::Connected)
                    }
                    VoiceEvent::PrivacyCode(c) => VoiceUiEvent::PrivacyCode(c),
                    VoiceEvent::Stats(s) => VoiceUiEvent::Stats(s),
                    VoiceEvent::Ping(p) => VoiceUiEvent::Ping(p),
                    VoiceEvent::Speaking { user_id, speaking } => VoiceUiEvent::Speaking { user: Id(user_id), speaking },
                    VoiceEvent::Encryption { e2ee, .. } => VoiceUiEvent::Encryption(e2ee),
                    VoiceEvent::Reconnecting => VoiceUiEvent::Status(VoiceStatus::Reconnecting),
                    VoiceEvent::Disconnected { reason } => {
                        {
                            let mut st = manager().state.lock();
                            if st.generation == generation {
                                stop(&mut st);
                            }
                        }
                        ui(&emit, VoiceUiEvent::Error(reason));
                        VoiceUiEvent::Status(VoiceStatus::Failed)
                    }
                };
                ui(&emit, e);
            }
        });
    }

    pub fn apply_control(&self, gw: Option<&GatewayHandle>, c: VoiceControl) {
        let mut st = self.state.lock();
        let mut resend = false;
        let mut restart = false;
        match c {
            VoiceControl::SetMute(m) => {
                st.muted = m;
                self.control.set_muted(m);
                resend = true;
            }
            VoiceControl::SetDeaf(d) => {
                st.deafened = d;
                self.control.set_deafened(d);
                resend = true;
            }
            VoiceControl::SetInputDevice(d) => {
                restart = st.input != d;
                st.input = d;
            }
            VoiceControl::SetOutputDevice(d) => {
                restart = st.output != d;
                st.output = d;
            }
            VoiceControl::SetSensitivity(t) => self.control.set_gate(t),
            VoiceControl::SetInputVolume(v) => self.control.set_input_gain(v),
            VoiceControl::SetOutputVolume(v) => self.control.set_output_gain(v),
            VoiceControl::SetUserVolume(u, v) => self.control.set_user_volume(u.0, v),
            VoiceControl::SetNoiseSuppression(v) => self.control.set_noise_suppression(v),
            VoiceControl::SetEchoCancellation(v) => self.control.set_echo_cancellation(v),
            VoiceControl::SetAgc(v) => self.control.set_agc(v),
            VoiceControl::SetPushToTalk(v) => {
                self.control.set_push_to_talk(v);
                self.control.set_ptt_pressed(false);
            }
            VoiceControl::SetPttPressed(v) => self.control.set_ptt_pressed(v),
            VoiceControl::SetPttDelay(ms) => self.control.set_ptt_release_ms(ms),
            VoiceControl::SetPttKey(k) => {
                if st.ptt_key != k {
                    st.ptt_key = k.clone();
                    self.bind_portal(&mut st, k);
                }
            }
        }
        if resend
            && let (Some(gw), Some(t)) = (gw, st.target)
        {
            let _ = gw.update_voice_state(t.guild.map(Snowflake), Some(Snowflake(t.channel)), st.muted, st.deafened);
        }
        if restart && st.running.is_some() {
            let Some(r) = st.running.as_ref() else { return };
            let out_tx = r.out_tx.clone();
            let sink = r.sink.clone();
            let old = st.running.as_mut().and_then(|r| r.engine.take());
            if let Some(e) = old {
                e.stop();
            }
            let engine = start_engine(&self.control, &st, out_tx);
            *sink.lock() = engine.as_ref().map(|e| e.sink());
            if let Some(r) = st.running.as_mut() {
                r.engine = engine;
            }
        }
    }
}

impl VoiceManager {
    #[cfg(target_os = "linux")]
    fn bind_portal(&self, st: &mut State, key: Option<String>) {
        st.ptt_portal = None;
        let Some(key) = key else {
            *portal_state().lock() = PortalState::Off;
            return;
        };
        *portal_state().lock() = PortalState::Pending;
        st.ptt_portal = Some(phoenix_voice::hotkey::start(self.control.clone(), &key, |s| {
            *portal_state().lock() = match s {
                phoenix_voice::hotkey::PortalStatus::Bound(d) => PortalState::Bound(d),
                phoenix_voice::hotkey::PortalStatus::Unavailable(e) => PortalState::Unavailable(e),
            };
        }));
    }

    #[cfg(not(target_os = "linux"))]
    fn bind_portal(&self, _st: &mut State, _key: Option<String>) {}
}

pub fn devices() -> (Vec<String>, Vec<String>) {
    (phoenix_voice::audio::device::input_devices(), phoenix_voice::audio::device::output_devices())
}
