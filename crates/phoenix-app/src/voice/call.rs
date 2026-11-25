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

pub const STREAM_AUDIO_BIT: u64 = 1 << 63;

struct OwnStream {
    generation: u64,
    key: Option<String>,
    resolution: u32,
    fps: u32,
    rtc_server_id: Option<String>,
    conn: Option<VoiceConnection>,
    capture: Option<phoenix_voice::media::camera::Camera>,
    input: Option<phoenix_voice::media::camera::Input>,
    size: Option<(u32, u32)>,
    #[cfg(target_os = "linux")]
    portal: Option<phoenix_voice::screencast::PortalSession>,
    preview_failed: Option<std::time::Instant>,
    stats: connection::ConnStats,
    sound: Option<phoenix_voice::media::soundshare::Source>,
    audio_tx: Option<mpsc::UnboundedSender<AudioOut>>,
    soundshare: Option<phoenix_voice::media::soundshare::Soundshare>,
}

struct Watch {
    generation: u64,
    owner: u64,
    rtc_server_id: Option<String>,
    conn: Option<VoiceConnection>,
    stats: connection::ConnStats,
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
    camera_on: bool,
    camera_device: Option<String>,
    camera: Option<phoenix_voice::media::camera::Camera>,
    region: Option<String>,
    gw: Option<GatewayHandle>,
    emit: Option<Emit>,
    own: Option<OwnStream>,
    watches: std::collections::HashMap<String, Watch>,
    pending_watch: Option<String>,
    stream_generation: u64,
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

pub const CAMERA_SIZE: (u32, u32, u32) = (1280, 720, 20);

type Decoders = std::collections::HashMap<u64, phoenix_voice::media::ffmpeg::FfmpegDecoder>;

fn remote() -> &'static Mutex<Decoders> {
    static R: OnceLock<Mutex<Decoders>> = OnceLock::new();
    R.get_or_init(Default::default)
}

pub fn remote_frame(user: u64) -> Option<phoenix_voice::media::VideoFrame> {
    let r = remote().lock();
    r.get(&user)?.latest.lock().ok()?.clone()
}

pub fn remote_video_active() -> bool {
    !remote().lock().is_empty() || !streams().lock().is_empty() || own_stream_active()
}

fn streams() -> &'static Mutex<Decoders> {
    static R: OnceLock<Mutex<Decoders>> = OnceLock::new();
    R.get_or_init(Default::default)
}

pub fn stream_frame(owner: u64) -> Option<phoenix_voice::media::VideoFrame> {
    let r = streams().lock();
    r.get(&owner)?.latest.lock().ok()?.clone()
}

pub fn own_stream_frame() -> Option<phoenix_voice::media::VideoFrame> {
    let st = manager().state.lock();
    let cap = st.own.as_ref()?.capture.as_ref()?;
    cap.preview.lock().ok()?.clone()
}

pub fn own_stream_active() -> bool {
    manager().state.lock().own.as_ref().is_some_and(|o| o.capture.is_some())
}

pub fn own_stream_stats() -> Option<connection::ConnStats> {
    manager().state.lock().own.as_ref().map(|o| o.stats)
}

pub fn own_stream_pending() -> bool {
    manager().state.lock().own.is_some()
}

pub fn watch_stats(key: &str) -> Option<connection::ConnStats> {
    manager().state.lock().watches.get(key).map(|w| w.stats)
}

pub fn watching(key: &str) -> bool {
    manager().state.lock().watches.contains_key(key)
}

fn on_remote_video(user: u64, frame: &[u8]) {
    push_video(remote(), user, frame, (phoenix_voice::media::ffmpeg::DECODE_W, phoenix_voice::media::ffmpeg::DECODE_H));
}

pub const STREAM_DECODE: (u32, u32) = (1280, 720);

fn push_video(map: &'static Mutex<Decoders>, user: u64, frame: &[u8], size: (u32, u32)) {
    let mut r = map.lock();
    if let std::collections::hash_map::Entry::Vacant(v) = r.entry(user) {
        match phoenix_voice::media::ffmpeg::FfmpegDecoder::spawn_sized(size.0, size.1) {
            Ok(d) => {
                v.insert(d);
            }
            Err(e) => {
                tracing::warn!("video decoder unavailable: {e}");
                return;
            }
        }
    }
    if let Some(d) = r.get_mut(&user)
        && d.push(frame).is_err()
    {
        r.remove(&user);
    }
}

pub fn camera_preview() -> Option<phoenix_voice::media::VideoFrame> {
    let st = manager().state.lock();
    let cam = st.camera.as_ref()?;
    cam.preview.lock().ok()?.clone()
}

pub fn camera_active() -> bool {
    manager().state.lock().camera.is_some()
}

fn stop_camera(st: &mut State) {
    if let Some(c) = st.camera.take() {
        std::thread::spawn(move || c.stop());
    }
    if let Some(r) = st.running.as_ref() {
        r.conn.video(phoenix_voice::connection::VideoOut::Stop);
    }
}

fn start_camera(st: &mut State) {
    if st.camera.is_some() {
        return;
    }
    let Some(r) = st.running.as_ref() else { return };
    let conn = r.conn.clone();
    let (w, h, fps) = CAMERA_SIZE;
    conn.video(phoenix_voice::connection::VideoOut::Start { width: w, height: h, fps, bitrate: phoenix_voice::connection::CAMERA_MAX_BITRATE });
    let input = phoenix_voice::media::camera::Input::from_name(st.camera_device.as_deref());
    let sink = conn.clone();
    match phoenix_voice::media::camera::Camera::start(input, w, h, fps, move |e| sink.video(phoenix_voice::connection::VideoOut::Frame(e))) {
        Ok(c) => st.camera = Some(c),
        Err(e) => {
            tracing::warn!("camera unavailable: {e}");
            conn.video(phoenix_voice::connection::VideoOut::Stop);
        }
    }
}

fn stop(st: &mut State) {
    st.camera_on = false;
    remote().lock().clear();
    stop_own_stream(st, None);
    for key in st.watches.keys().cloned().collect::<Vec<_>>() {
        stop_watch(st, &key);
    }
    if let Some(c) = st.camera.take() {
        std::thread::spawn(move || c.stop());
    }
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
            tracing::warn!("audio devices unavailable: {e}");
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
        st.emit = Some(emit.clone());
        st.gw = Some(gw.clone());
        if channel.is_some() && st.target.map(|t| t.channel) == channel && st.running.is_some() {
            ui(emit, VoiceUiEvent::Status(VoiceStatus::Connected));
            return Ok(());
        }
        st.region = channel.and_then(|c| cache.channel(Snowflake(c))).and_then(|c| c.rtc_region.clone());
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
        if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
            match ev {
                GatewayEvent::VoiceStateUpdate(v) if v.user_id.0 == me => {
                    eprintln!("voice-trace: own voice state channel={:?} self_video={}", v.channel_id.map(|c| c.0), v.self_video)
                }
                GatewayEvent::VoiceServerUpdate(s) => eprintln!("voice-trace: server update endpoint_set={}", s.endpoint.is_some()),
                _ => {}
            }
        }
        let mut st = self.state.lock();
        st.emit = Some(emit.clone());
        match ev {
            GatewayEvent::StreamCreate(sc) => return on_stream_create(&mut st, me, sc),
            GatewayEvent::StreamServerUpdate(su) => return on_stream_server(&mut st, me, su),
            GatewayEvent::StreamDelete(sd) => {
                if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
                    eprintln!("voice-trace: STREAM_DELETE key={} reason={:?}", sd.stream_key, sd.reason);
                }
                if st.own.as_ref().and_then(|o| o.key.as_deref()) == Some(sd.stream_key.as_str()) {
                    stop_own_stream(&mut st, sd.reason.clone());
                } else if st.watches.contains_key(&sd.stream_key) {
                    end_watch(&mut st, &sd.stream_key, false);
                }
                return;
            }
            _ => {}
        }
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
            stream: false,
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
                if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() && !matches!(ev, VoiceEvent::Video { .. } | VoiceEvent::Stats(_) | VoiceEvent::Ping(_)) {
                    eprintln!("voice-trace: {ev:?}");
                }
                let e = match ev {
                    VoiceEvent::Connected { mode, .. } => {
                        {
                            let mut st = manager().state.lock();
                            if let Some(k) = st.pending_watch.take() {
                                start_watch(&mut st, k);
                            }
                        }
                        ui(&emit, VoiceUiEvent::Server { endpoint: endpoint.clone(), mode });
                        VoiceUiEvent::Status(VoiceStatus::Connected)
                    }
                    VoiceEvent::PrivacyCode(c) => VoiceUiEvent::PrivacyCode(c),
                    VoiceEvent::Stats(s) => VoiceUiEvent::Stats(s),
                    VoiceEvent::Ping(p) => VoiceUiEvent::Ping(p),
                    VoiceEvent::Speaking { user_id, speaking } => VoiceUiEvent::Speaking { user: Id(user_id), speaking },
                    VoiceEvent::Encryption { e2ee, .. } => VoiceUiEvent::Encryption(e2ee),
                    VoiceEvent::Reconnecting => VoiceUiEvent::Status(VoiceStatus::Reconnecting),
                    VoiceEvent::Video { user_id, frame } => {
                        on_remote_video(user_id, &frame);
                        continue;
                    }
                    VoiceEvent::VideoState { user_id, active } => {
                        if !active {
                            remote().lock().remove(&user_id);
                        }
                        continue;
                    }
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
        if let Some(g) = gw {
            st.gw = Some(g.clone());
        }
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
            VoiceControl::SetCameraDevice(d) => st.camera_device = d,
            VoiceControl::SetCamera(on) => {
                st.camera_on = on && st.target.is_some();
                if st.camera_on {
                    start_camera(&mut st);
                    st.camera_on = st.camera.is_some();
                } else {
                    stop_camera(&mut st);
                }
                resend = true;
            }
            VoiceControl::StartStream { source, resolution, fps, sound } => start_own_stream(&mut st, source, resolution, fps, sound),
            VoiceControl::StopStream => stop_own_stream(&mut st, None),
            VoiceControl::WatchStream(key) => start_watch(&mut st, key),
            VoiceControl::WatchAfterJoin(key) => st.pending_watch = Some(key),
            VoiceControl::StopWatching(key) => stop_watch(&mut st, &key),
            VoiceControl::SetStreamVolume(u, v) => self.control.set_user_volume(u.0 | STREAM_AUDIO_BIT, v),
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
            let _ = gw.update_voice_state_video(t.guild.map(Snowflake), Some(Snowflake(t.channel)), st.muted, st.deafened, st.camera_on);
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

fn stream_ui(st: &State, e: super::StreamUiEvent) {
    if let Some(emit) = st.emit.as_ref() {
        ui(emit, VoiceUiEvent::Stream(e));
    }
}

fn start_own_stream(st: &mut State, source: super::StreamSource, resolution: u32, fps: u32, sound: bool) {
    stop_own_stream(st, None);
    let (Some(t), true) = (st.target, st.running.is_some()) else {
        stream_ui(st, super::StreamUiEvent::Failed("not connected to voice".into()));
        return;
    };
    if t.guild.is_none() {
        stream_ui(st, super::StreamUiEvent::Failed("Go Live needs a server voice channel".into()));
        return;
    }
    st.stream_generation += 1;
    let generation = st.stream_generation;
    st.own = Some(OwnStream {
        generation,
        key: None,
        resolution,
        fps,
        rtc_server_id: None,
        conn: None,
        capture: None,
        input: None,
        size: None,
        #[cfg(target_os = "linux")]
        portal: None,
        preview_failed: None,
        stats: Default::default(),
        sound: sound.then_some(if source == super::StreamSource::TestPattern {
            phoenix_voice::media::soundshare::Source::Tone
        } else {
            phoenix_voice::media::soundshare::Source::Monitor
        }),
        audio_tx: None,
        soundshare: None,
    });
    match source {
        super::StreamSource::TestPattern => {
            if let Some(o) = st.own.as_mut() {
                o.input = Some(phoenix_voice::media::camera::Input::TestPattern);
            }
            send_stream_create(st);
        }
        #[cfg(target_os = "linux")]
        super::StreamSource::Portal(kind) => {
            let kind = match kind {
                super::stream::SourceKind::Screen => phoenix_voice::screencast::Kind::Screen,
                super::stream::SourceKind::Window => phoenix_voice::screencast::Kind::Window,
                super::stream::SourceKind::Any => phoenix_voice::screencast::Kind::Any,
            };
            phoenix_voice::screencast::pick(kind, move |r| {
                let mut st = manager().state.lock();
                if st.own.as_ref().map(|o| o.generation) != Some(generation) {
                    return;
                }
                match r {
                    Ok(p) => {
                        #[cfg(unix)]
                        if let Some(o) = st.own.as_mut() {
                            o.input = Some(phoenix_voice::media::camera::Input::PipeWire { fd: p.fd, node: p.node });
                            o.size = p.size;
                            o.portal = Some(p.session);
                        }
                        #[cfg(not(unix))]
                        let _ = p;
                        send_stream_create(&st);
                    }
                    Err(e) => {
                        st.own = None;
                        stream_ui(&st, super::StreamUiEvent::Failed(e));
                    }
                }
            });
        }
        #[cfg(not(target_os = "linux"))]
        super::StreamSource::Portal(_) => {
            st.own = None;
            stream_ui(st, super::StreamUiEvent::Failed("screen capture is only available on Linux".into()));
        }
    }
}

fn send_stream_create(st: &State) {
    let (Some(t), Some(gw)) = (st.target, st.gw.as_ref()) else { return };
    let Some(g) = t.guild else { return };
    if let Err(e) = gw.stream_create(Snowflake(g), Snowflake(t.channel), st.region.as_deref()) {
        stream_ui(st, super::StreamUiEvent::Failed(e.to_string()));
    }
}

fn stop_own_stream(st: &mut State, reason: Option<String>) {
    let Some(o) = st.own.take() else { return };
    if let (Some(k), Some(gw)) = (o.key.as_deref(), st.gw.as_ref()) {
        let _ = gw.stream_delete(k);
    }
    if let Some(c) = o.conn.as_ref() {
        c.video(phoenix_voice::connection::VideoOut::Stop);
        c.close();
    }
    if let Some(c) = o.capture {
        std::thread::spawn(move || c.stop());
    }
    if let Some(s) = o.soundshare {
        std::thread::spawn(move || drop(s));
    }
    if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
        eprintln!("voice-trace: own stream stopped reason={reason:?}");
    }
    stream_ui(st, super::StreamUiEvent::Live(None));
}

fn on_stream_create(st: &mut State, me: u64, sc: &phoenix_api::gateway::events::StreamCreate) {
    let parsed = super::stream::parse_key(&sc.stream_key);
    if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
        eprintln!("voice-trace: STREAM_CREATE key={} rtc_server_set={} viewers={} paused={}", sc.stream_key, sc.rtc_server_id.is_some(), sc.viewer_ids.len(), sc.paused);
    }
    if let Some(w) = st.watches.get_mut(&sc.stream_key) {
        w.rtc_server_id = sc.rtc_server_id.clone();
        return;
    }
    let target = st.target;
    if let Some(o) = st.own.as_mut()
        && parsed.is_some_and(|k| k.owner == me && Some(k.channel) == target.map(|t| t.channel))
        && o.key.as_ref().is_none_or(|k| *k == sc.stream_key)
    {
        o.key = Some(sc.stream_key.clone());
        o.rtc_server_id = sc.rtc_server_id.clone();
    }
}

fn on_stream_server(st: &mut State, me: u64, su: &phoenix_api::gateway::events::StreamServerUpdate) {
    if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
        eprintln!("voice-trace: STREAM_SERVER_UPDATE key={} endpoint_set={}", su.stream_key, su.endpoint.is_some());
    }
    let Some(endpoint) = su.endpoint.clone() else { return };
    let Some(session_id) = st.session_id.clone().or_else(|| Some(crate::commands::session_id()).filter(|s| !s.is_empty())) else { return };
    let own = st.own.as_ref().is_some_and(|o| o.key.as_deref() == Some(su.stream_key.as_str()));
    if own {
        let Some(o) = st.own.as_mut() else { return };
        let Some(server_id) = o.rtc_server_id.clone() else { return };
        if let Some(c) = o.conn.take() {
            c.close();
        }
        let generation = o.generation;
        let info = stream_info(endpoint, server_id, me, session_id, su.token.clone());
        let (audio_tx, audio_rx) = mpsc::unbounded_channel();
        let (conn, events) = connection::connect(VoiceConfig { info, channel_id: st.target.map_or(0, |t| t.channel), user_id: me }, audio_rx, Box::new(|_: connection::AudioIn| {}));
        o.conn = Some(conn);
        o.soundshare = None;
        o.audio_tx = Some(audio_tx);
        tokio::spawn(own_stream_events(generation, su.stream_key.clone(), events));
        return;
    }
    let Some(w) = st.watches.get_mut(&su.stream_key) else { return };
    let Some(server_id) = w.rtc_server_id.clone() else { return };
    if let Some(c) = w.conn.take() {
        c.close();
    }
    let owner = w.owner;
    let generation = w.generation;
    let sink = st.running.as_ref().map(|r| r.sink.clone());
    let info = stream_info(endpoint, server_id, me, session_id, su.token.clone());
    let (conn, events) = connection::connect(
        VoiceConfig { info, channel_id: super::stream::parse_key(&su.stream_key).map_or(0, |k| k.channel), user_id: me },
        mpsc::unbounded_channel().1,
        Box::new(move |mut a: connection::AudioIn| {
            a.user_id = owner | STREAM_AUDIO_BIT;
            if let Some(s) = sink.as_ref().and_then(|s| s.lock().clone()) {
                s.push(a);
            }
        }),
    );
    if let Some(w) = st.watches.get_mut(&su.stream_key) {
        w.conn = Some(conn);
    }
    tokio::spawn(watch_events(generation, su.stream_key.clone(), owner, events));
}

fn stream_info(endpoint: String, server_id: String, me: u64, session_id: String, token: String) -> VoiceConnectInfo {
    VoiceConnectInfo {
        endpoint,
        server_id,
        user_id: me.to_string(),
        session_id,
        token,
        max_dave_protocol_version: phoenix_voice::dave::MAX_PROTOCOL_VERSION,
        stream: true,
    }
}

async fn own_stream_events(generation: u64, key: String, mut events: mpsc::UnboundedReceiver<VoiceEvent>) {
    while let Some(ev) = events.recv().await {
        let mut st = manager().state.lock();
        if st.own.as_ref().map(|o| o.generation) != Some(generation) {
            return;
        }
        if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() && !matches!(ev, VoiceEvent::Video { .. } | VoiceEvent::Ping(_)) {
            eprintln!("voice-trace: stream(own) {ev:?}");
        }
        match ev {
            VoiceEvent::Connected { .. } => {
                start_capture(&mut st);
                if st.own.as_ref().is_some_and(|o| o.capture.is_some()) {
                    stream_ui(&st, super::StreamUiEvent::Live(Some(key.clone())));
                    let k = key.clone();
                    std::thread::Builder::new()
                        .name("phoenix-stream-preview".into())
                        .spawn(move || preview_loop(generation, k))
                        .ok();
                }
            }
            VoiceEvent::Stats(s) => {
                if let Some(o) = st.own.as_mut() {
                    o.stats = s;
                }
            }
            VoiceEvent::Disconnected { reason } => {
                stop_own_stream(&mut st, Some(reason.clone()));
                stream_ui(&st, super::StreamUiEvent::Failed(reason));
                return;
            }
            _ => {}
        }
    }
}

fn start_capture(st: &mut State) {
    let Some(o) = st.own.as_mut() else { return };
    if o.capture.is_some() {
        return;
    }
    let (Some(conn), Some(input)) = (o.conn.clone(), o.input.take()) else { return };
    let (w, h) = super::stream::frame_size(o.resolution, o.size);
    let fps = o.fps.max(5);
    let bitrate = super::stream::bitrate(o.resolution, fps);
    conn.video(phoenix_voice::connection::VideoOut::Start { width: w, height: h, fps, bitrate });
    let sink = conn.clone();
    if let (Some(src), Some(tx)) = (o.sound.clone(), o.audio_tx.clone()) {
        match phoenix_voice::media::soundshare::Soundshare::start(src, tx) {
            Ok(s) => o.soundshare = Some(s),
            Err(e) => tracing::warn!("stream audio unavailable: {e}"),
        }
    }
    match phoenix_voice::media::camera::Camera::start_with(input, w, h, fps, bitrate, false, move |e| sink.video(phoenix_voice::connection::VideoOut::Frame(e))) {
        Ok(c) => o.capture = Some(c),
        Err(e) => {
            tracing::warn!("screen capture unavailable: {e}");
            conn.video(phoenix_voice::connection::VideoOut::Stop);
        }
    }
}

fn preview_loop(generation: u64, key: String) {
    use std::time::{Duration, Instant};
    std::thread::sleep(Duration::from_millis(super::stream::PREVIEW_FIRST_MS));
    let mut next = Instant::now();
    loop {
        let alive = |st: &State| st.own.as_ref().is_some_and(|o| o.generation == generation);
        if Instant::now() >= next {
            let mut frame = None;
            for _ in 0..60 {
                let f = {
                    let st = manager().state.lock();
                    if !alive(&st) {
                        return;
                    }
                    st.own.as_ref().and_then(|o| o.capture.as_ref()).and_then(|c| c.preview.lock().ok()?.clone())
                };
                if let Some(f) = f.filter(|f| !super::stream::is_black(&f.data)) {
                    frame = Some(f);
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let thumb = frame.and_then(|f| super::stream::preview_jpeg(f.width, f.height, &f.data));
            let st = manager().state.lock();
            if !alive(&st) {
                return;
            }
            match thumb {
                Some(t) => {
                    stream_ui(&st, super::StreamUiEvent::Preview { key: key.clone(), thumbnail: t });
                    next = Instant::now() + Duration::from_millis(super::stream::PREVIEW_EVERY_MS);
                }
                None => next = Instant::now() + Duration::from_millis(super::stream::PREVIEW_RETRY_MS),
            }
        }
        std::thread::sleep(Duration::from_secs(1));
        let mut st = manager().state.lock();
        if !alive(&st) {
            return;
        }
        if let Some(failed) = st.own.as_mut().and_then(|o| o.preview_failed.take()) {
            next = failed + Duration::from_millis(super::stream::PREVIEW_RETRY_MS);
        }
    }
}

pub fn preview_uploaded(key: &str, ok: bool) {
    let mut st = manager().state.lock();
    if let Some(o) = st.own.as_mut().filter(|o| o.key.as_deref() == Some(key))
        && !ok
    {
        o.preview_failed = Some(std::time::Instant::now());
    }
}

fn start_watch(st: &mut State, key: String) {
    let Some(k) = super::stream::parse_key(&key) else { return };
    if st.watches.contains_key(&key) {
        return;
    }
    if st.target.is_some_and(|t| t.channel != k.channel) {
        return;
    }
    let Some(gw) = st.gw.clone() else {
        if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
            eprintln!("voice-trace: watch {key}: no gateway");
        }
        return;
    };
    if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
        eprintln!("voice-trace: op 20 STREAM_WATCH {key}");
    }
    st.stream_generation += 1;
    st.watches.insert(key.clone(), Watch { generation: st.stream_generation, owner: k.owner, rtc_server_id: None, conn: None, stats: Default::default() });
    if let Err(e) = gw.stream_watch(&key) {
        st.watches.remove(&key);
        stream_ui(st, super::StreamUiEvent::Failed(e.to_string()));
        return;
    }
    stream_ui(st, super::StreamUiEvent::Watching { key, on: true });
}

fn stop_watch(st: &mut State, key: &str) {
    end_watch(st, key, true);
}

fn end_watch(st: &mut State, key: &str, tell_server: bool) {
    let Some(w) = st.watches.remove(key) else { return };
    if tell_server && let Some(gw) = st.gw.as_ref() {
        let _ = gw.stream_delete(key);
    }
    if let Some(c) = w.conn {
        c.close();
    }
    streams().lock().remove(&w.owner);
    if let Some(s) = st.running.as_ref().and_then(|r| r.sink.lock().clone()) {
        s.remove_user(w.owner | STREAM_AUDIO_BIT);
    }
    stream_ui(st, super::StreamUiEvent::Watching { key: key.to_string(), on: false });
}

async fn watch_events(generation: u64, key: String, owner: u64, mut events: mpsc::UnboundedReceiver<VoiceEvent>) {
    while let Some(ev) = events.recv().await {
        if manager().state.lock().watches.get(&key).map(|w| w.generation) != Some(generation) {
            return;
        }
        if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() && !matches!(ev, VoiceEvent::Video { .. } | VoiceEvent::Ping(_) | VoiceEvent::Stats(_)) {
            eprintln!("voice-trace: stream(watch) {ev:?}");
        }
        match ev {
            VoiceEvent::Video { frame, .. } => push_video(streams(), owner, &frame, STREAM_DECODE),
            VoiceEvent::Stats(s) => {
                if let Some(w) = manager().state.lock().watches.get_mut(&key) {
                    w.stats = s;
                }
            }
            VoiceEvent::VideoState { active: false, .. } => {
                streams().lock().remove(&owner);
            }
            VoiceEvent::Disconnected { .. } => {
                let mut st = manager().state.lock();
                stop_watch(&mut st, &key);
                return;
            }
            _ => {}
        }
    }
}

pub fn devices() -> (Vec<String>, Vec<String>) {
    (phoenix_voice::audio::device::input_devices(), phoenix_voice::audio::device::output_devices())
}
