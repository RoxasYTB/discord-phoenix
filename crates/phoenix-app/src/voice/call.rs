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
    sound_source: phoenix_voice::media::soundshare::Source,
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
    view: Option<(u32, u32)>,
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
    retries: u32,
    joining: bool,
    input: Option<String>,
    output: Option<String>,
    muted: bool,
    deafened: bool,
    ptt_key: Option<String>,
    global_binds: Vec<(String, String, String)>,
    camera_on: bool,
    priority: bool,
    camera_device: Option<String>,
    camera: Option<phoenix_voice::media::camera::Camera>,
    region: Option<String>,
    gw: Option<GatewayHandle>,
    emit: Option<Emit>,
    own: Option<OwnStream>,
    watches: std::collections::HashMap<String, Watch>,
    pending_watch: Option<String>,
    stream_generation: u64,
    call_view: Option<phoenix_voice::sink_wants::CallView>,
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

#[cfg(target_os = "linux")]
pub fn configure_portal() -> bool {
    let st = manager().state.lock();
    let Some(p) = st.ptt_portal.as_ref() else { return false };
    p.configure();
    true
}

#[cfg(not(target_os = "linux"))]
pub fn configure_portal() -> bool {
    false
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

fn push_video(map: &'static Mutex<Decoders>, user: u64, codec: phoenix_voice::codec::Codec, keyframe: bool, frame: &[u8], max: (u32, u32)) -> bool {
    use phoenix_voice::media::ffmpeg::FfmpegDecoder;
    let mut r = map.lock();
    static SOFTWARE_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if let Some(d) = r.get_mut(&user) {
        let dead = !d.alive();
        if dead && d.hwaccel() {
            tracing::warn!("hardware video decoder stopped, using the software decoder");
            SOFTWARE_ONLY.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if d.codec() != codec || dead {
            r.remove(&user);
        }
    }
    if let std::collections::hash_map::Entry::Vacant(v) = r.entry(user) {
        if !keyframe {
            return true;
        }
        let hwaccel = if SOFTWARE_ONLY.load(std::sync::atomic::Ordering::Relaxed) { None } else { phoenix_voice::media::ffmpeg::hwaccel() };
        match FfmpegDecoder::spawn_with(codec, max, hwaccel.as_deref()) {
            Ok(d) => {
                v.insert(d);
            }
            Err(e) => {
                tracing::warn!("video decoder unavailable: {e}");
                return false;
            }
        }
    }
    if let Some(d) = r.get_mut(&user)
        && d.push(frame).is_err()
    {
        r.remove(&user);
        return true;
    }
    false
}

pub fn set_call_view(view: phoenix_voice::sink_wants::CallView) {
    let mut st = manager().state.lock();
    if st.call_view.as_ref() == Some(&view) {
        return;
    }
    if let Some(r) = st.running.as_ref() {
        r.conn.set_view(connection::VideoView::Call(view.clone()));
    }
    st.call_view = Some(view);
}

pub fn set_pip_view() {
    let view = manager().state.lock().call_view.clone().unwrap_or(phoenix_voice::sink_wants::CallView { incoming_video: true, ..Default::default() });
    set_call_view(phoenix_voice::sink_wants::CallView { pip: true, ..view });
}

pub fn set_stream_view(key: &str, width: u32, height: u32) {
    let mut st = manager().state.lock();
    let Some(w) = st.watches.get_mut(key) else { return };
    if w.view == Some((width, height)) {
        return;
    }
    w.view = Some((width, height));
    if let Some(c) = w.conn.as_ref() {
        c.set_view(connection::VideoView::Stream { width, height, zoom: 1.0 });
    }
}

pub fn in_call() -> bool {
    manager().state.lock().running.is_some()
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

fn camera_failed(st: &State, failure: super::MediaFailure) {
    tracing::warn!(?failure, "camera unavailable");
    if let Some(emit) = st.emit.as_ref() {
        ui(emit, VoiceUiEvent::CameraFailed(failure));
    }
}

fn screen_failed(st: &State, failure: super::MediaFailure) {
    tracing::warn!(?failure, "screen capture unavailable");
    if let Some(emit) = st.emit.as_ref() {
        ui(emit, VoiceUiEvent::ScreenFailed(failure));
    }
}

const CAMERA_FIRST_FRAME: std::time::Duration = std::time::Duration::from_secs(8);

fn start_camera(st: &mut State) {
    if st.camera.is_some() {
        return;
    }
    let Some(r) = st.running.as_ref() else { return };
    let conn = r.conn.clone();
    let input = phoenix_voice::media::camera::Input::from_name(st.camera_device.as_deref());
    if !phoenix_voice::media::ffmpeg::available() {
        camera_failed(st, super::MediaFailure::NoFfmpeg);
        return;
    }
    #[cfg(not(windows))]
    if let phoenix_voice::media::camera::Input::Device(dev) = &input
        && !std::path::Path::new(dev).exists()
    {
        camera_failed(st, super::MediaFailure::NoDevice);
        return;
    }
    let (w, h, fps) = CAMERA_SIZE;
    conn.video(phoenix_voice::connection::VideoOut::Start { width: w, height: h, fps, bitrate: phoenix_voice::connection::CAMERA_MAX_BITRATE });
    let sink = conn.clone();
    match phoenix_voice::media::camera::Camera::start(input, w, h, fps, move |e| sink.video(phoenix_voice::connection::VideoOut::Frame(e))) {
        Ok(c) => {
            conn.forward_keyframe_requests(c.keyframe_flag());
            watch_first_frame(c.preview.clone());
            st.camera = Some(c);
        }
        Err(e) => {
            conn.video(phoenix_voice::connection::VideoOut::Stop);
            let failure = if e.to_string().contains("no video capture device") { super::MediaFailure::NoDevice } else { super::MediaFailure::Other(e.to_string()) };
            camera_failed(st, failure);
        }
    }
}

fn watch_first_frame(preview: phoenix_voice::media::camera::Preview) {
    let spawned = std::thread::Builder::new().name("phoenix-camera-watch".into()).spawn(move || {
        let deadline = std::time::Instant::now() + CAMERA_FIRST_FRAME;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if preview.lock().is_ok_and(|p| p.is_some()) {
                return;
            }
        }
        let mut st = manager().state.lock();
        if !st.camera.as_ref().is_some_and(|c| Arc::ptr_eq(&c.preview, &preview)) {
            return;
        }
        st.camera_on = false;
        stop_camera(&mut st);
        if let (Some(gw), Some(t)) = (st.gw.as_ref(), st.target) {
            let _ = gw.update_voice_state_video(t.guild.map(Snowflake), Some(Snowflake(t.channel)), st.muted, st.deafened, false);
        }
        camera_failed(&st, super::MediaFailure::NoDevice);
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "camera watch thread");
    }
}

fn stop(st: &mut State) {
    st.camera_on = false;
    manager().control.clear_priority_speakers();
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
            if std::env::var_os("PHOENIX_SOUND_LOG").is_some() {
                eprintln!("voice-audio: audio engine failed: {e}");
            }
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
        st.retries = 0;
        st.joining = false;
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
        st.joining = true;
        gw.update_voice_state(guild.map(Snowflake), Some(Snowflake(ch)), st.muted, st.deafened).map_err(|e| e.to_string())?;
        ui(emit, VoiceUiEvent::Status(VoiceStatus::AwaitingEndpoint));
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
            GatewayEvent::VoiceStateUpdate(v) if v.user_id.0 == me => {
                let action = own_voice_state(target, st.joining, v.guild_id.map(|g| g.0), v.channel_id.map(|c| c.0));
                if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
                    eprintln!("voice-trace: own voice state target={} action={action:?}", target.channel);
                }
                match action {
                    OwnVoice::Ignore => {}
                    OwnVoice::Left => {
                        stop(&mut st);
                        st.target = None;
                        st.generation += 1;
                        ui(emit, VoiceUiEvent::Status(VoiceStatus::Idle));
                    }
                    OwnVoice::Moved(t) => {
                        st.target = Some(t);
                        st.joining = false;
                        stop(&mut st);
                        st.generation += 1;
                        st.session_id = Some(v.session_id.clone());
                        self.try_start(&mut st, emit, me);
                    }
                    OwnVoice::Joined(t) => {
                        st.target = Some(t);
                        st.joining = false;
                        if st.session_id.as_deref() != Some(v.session_id.as_str()) {
                            st.session_id = Some(v.session_id.clone());
                        }
                        self.try_start(&mut st, emit, me);
                    }
                }
            }
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
                            st.retries = 0;
                            st.server = new;
                            self.try_start(&mut st, emit, me);
                        }
                    }
                    None => {
                        stop(&mut st);
                        st.server = None;
                        ui(emit, VoiceUiEvent::Status(VoiceStatus::AwaitingEndpoint));
                    }
                }
            }
            _ => {}
        }
    }

    fn try_start(&self, st: &mut State, emit: &Emit, me: u64) {
        if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
            eprintln!(
                "voice-trace: try_start target={:?} session={} server={} running={}",
                st.target.map(|t| t.channel),
                st.session_id.is_some(),
                st.server.is_some(),
                st.running.is_some()
            );
        }
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
        if let Some(v) = st.call_view.clone() {
            conn.set_view(connection::VideoView::Call(v));
        }
        conn.set_priority(st.priority);
        st.running = Some(Running { conn, engine, out_tx, sink });
        ui(emit, VoiceUiEvent::Status(VoiceStatus::Connecting));
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
                            st.retries = 0;
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
                    VoiceEvent::SpeakingFlags { user_id, flags } => VoiceUiEvent::SpeakingFlags { user: Id(user_id), flags },
                    VoiceEvent::Encryption { e2ee, .. } => VoiceUiEvent::Encryption(e2ee),
                    VoiceEvent::Reconnecting => {
                        ping_on_reconnect(None);
                        VoiceUiEvent::Status(VoiceStatus::Reconnecting)
                    }
                    VoiceEvent::Video { user_id, codec, keyframe, frame } => {
                        if push_video(remote(), user_id, codec, keyframe, &frame, phoenix_voice::media::ffmpeg::CAMERA_MAX)
                            && let Some(r) = manager().state.lock().running.as_ref()
                        {
                            r.conn.request_keyframe(user_id);
                        }
                        continue;
                    }
                    VoiceEvent::VideoState { user_id, active } => {
                        if !active {
                            remote().lock().remove(&user_id);
                        }
                        continue;
                    }
                    VoiceEvent::Disconnected { reason, retry } => {
                        let again = {
                            let mut st = manager().state.lock();
                            if st.generation != generation {
                                return;
                            }
                            stop(&mut st);
                            let delay = retry_delay(retry, st.retries);
                            if delay.is_some() {
                                st.retries += 1;
                                st.generation += 1;
                            }
                            delay.map(|d| (d, st.generation))
                        };
                        if let Some((delay, next)) = again {
                            if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
                                eprintln!("voice-trace: reconnecting in {}ms after: {reason}", delay.as_millis());
                            }
                            ui(&emit, VoiceUiEvent::Status(VoiceStatus::Reconnecting));
                            let emit = emit.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(delay).await;
                                let m = manager();
                                let mut st = m.state.lock();
                                if st.generation == next {
                                    m.try_start(&mut st, &emit, me);
                                }
                            });
                            return;
                        }
                        let status = VoiceStatus::from_disconnect(&reason);
                        ui(&emit, VoiceUiEvent::Error(reason));
                        VoiceUiEvent::Status(status)
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
            VoiceControl::SetPriority(on) => {
                st.priority = on;
                if let Some(r) = st.running.as_ref() {
                    r.conn.set_priority(on);
                }
            }
            VoiceControl::SetPrioritySpeaker(u, on) => self.control.set_priority_speaker(u.0, on),
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
            VoiceControl::UpdateStreamQuality { resolution, fps, sound } => update_own_stream(&mut st, resolution, fps, sound),
            VoiceControl::StopStream => stop_own_stream(&mut st, None),
            VoiceControl::WatchStream(key) => start_watch(&mut st, key),
            VoiceControl::WatchAfterJoin(key) => st.pending_watch = Some(key),
            VoiceControl::StopWatching(key) => stop_watch(&mut st, &key),
            VoiceControl::SetStreamVolume(u, v) => self.control.set_user_volume(u.0 | STREAM_AUDIO_BIT, v),
            VoiceControl::SetPttKey(k) => {
                if st.ptt_key != k {
                    st.ptt_key = k;
                    self.bind_portal(&mut st);
                }
            }
            VoiceControl::SetGlobalKeybinds(b) => {
                if st.global_binds != b {
                    st.global_binds = b;
                    self.bind_portal(&mut st);
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
    fn bind_portal(&self, st: &mut State) {
        use phoenix_voice::hotkey::{Binding, PortalStatus, SHORTCUT_ID};
        st.ptt_portal = None;
        let mut binds: Vec<Binding> = st.global_binds.iter().map(|(id, description, keystroke)| Binding { id: id.clone(), description: description.clone(), keystroke: keystroke.clone() }).collect();
        if let Some(k) = st.ptt_key.clone() {
            binds.insert(0, Binding { id: SHORTCUT_ID.into(), description: "Push to Talk".into(), keystroke: k });
        }
        let has_ptt = st.ptt_key.is_some();
        *portal_state().lock() = if has_ptt { PortalState::Pending } else { PortalState::Off };
        crate::keybinds::set_portal(None);
        if binds.is_empty() {
            return;
        }
        st.ptt_portal = Some(phoenix_voice::hotkey::start(self.control.clone(), binds, crate::keybinds::global_event, move |s| {
            let ptt = match &s {
                PortalStatus::Bound(list) => PortalState::Bound(list.iter().find(|(id, _)| id == SHORTCUT_ID).map(|(_, d)| d.clone()).unwrap_or_default()),
                PortalStatus::Unavailable(e) => PortalState::Unavailable(e.clone()),
            };
            if has_ptt {
                *portal_state().lock() = ptt;
            }
            crate::keybinds::set_portal(match s {
                PortalStatus::Bound(list) => Some(list),
                PortalStatus::Unavailable(_) => None,
            });
        }));
    }

    #[cfg(not(target_os = "linux"))]
    fn bind_portal(&self, _st: &mut State) {}
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
    if !phoenix_voice::media::ffmpeg::available() {
        screen_failed(st, super::MediaFailure::NoFfmpeg);
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
        sound_source: if source == super::StreamSource::TestPattern {
            phoenix_voice::media::soundshare::Source::Tone
        } else {
            phoenix_voice::media::soundshare::Source::Monitor
        },
        sound: sound.then_some(if source == super::StreamSource::TestPattern {
            phoenix_voice::media::soundshare::Source::Tone
        } else {
            phoenix_voice::media::soundshare::Source::Monitor
        }),
        audio_tx: None,
        soundshare: None,
    });
    match source {
        #[cfg(windows)]
        super::StreamSource::Camera(name) => {
            if let Some(o) = st.own.as_mut() {
                o.input = Some(phoenix_voice::media::camera::Input::Device(name));
            }
            send_stream_create(st);
        }
        #[cfg(windows)]
        super::StreamSource::Selected { target, size } => {
            if let Some(o) = st.own.as_mut() {
                o.input = Some(phoenix_voice::media::camera::Input::Selected(target));
                o.size = size;
            }
            send_stream_create(st);
        }
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

fn update_own_stream(st: &mut State, resolution: u32, fps: u32, sound: bool) {
    let Some(o) = st.own.as_mut() else { return };
    let video_changed = o.resolution != resolution || o.fps != fps;
    o.resolution = resolution;
    o.fps = fps;
    if video_changed
        && let (Some(capture), Some(conn)) = (o.capture.as_ref(), o.conn.as_ref())
    {
        let (w, h) = super::stream::frame_size(resolution, o.size);
        let fps = fps.max(5);
        let bitrate = super::stream::bitrate(resolution, fps);
        capture.reconfigure(w, h, fps, bitrate);
        conn.video(phoenix_voice::connection::VideoOut::Update {
            width: w,
            height: h,
            fps,
            bitrate,
            source_resolution: resolution == 0,
        });
    }
    if sound && o.sound.is_none() {
        o.sound = Some(o.sound_source.clone());
        if o.capture.is_some()
            && let (Some(src), Some(tx)) = (o.sound.clone(), o.audio_tx.clone())
        {
            match phoenix_voice::media::soundshare::Soundshare::start(src, tx) {
                Ok(s) => o.soundshare = Some(s),
                Err(e) => tracing::warn!("stream audio unavailable: {e}"),
            }
        }
    } else if !sound && o.sound.is_some() {
        o.sound = None;
        if let Some(s) = o.soundshare.take() {
            std::thread::spawn(move || drop(s));
        }
        if let Some(tx) = o.audio_tx.as_ref() {
            let _ = tx.send(AudioOut::End);
        }
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
        if let Some((width, height)) = w.view {
            conn.set_view(connection::VideoView::Stream { width, height, zoom: 1.0 });
        }
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

fn ping_on_reconnect(stream_key: Option<&str>) {
    let Some(gw) = manager().state.lock().gw.clone() else { return };
    drop(match stream_key {
        Some(k) => gw.stream_ping(k),
        None => gw.voice_server_ping(),
    });
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
            VoiceEvent::Reconnecting => {
                if let Some(gw) = st.gw.as_ref() {
                    drop(gw.stream_ping(&key));
                }
            }
            VoiceEvent::Disconnected { reason, .. } => {
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
        Ok(c) => {
            conn.forward_keyframe_requests(c.keyframe_flag());
            o.capture = Some(c);
        }
        Err(e) => {
            conn.video(phoenix_voice::connection::VideoOut::Stop);
            screen_failed(st, super::MediaFailure::Other(e.to_string()));
        }
    }
}

fn preview_loop(generation: u64, key: String) {
    use std::time::{Duration, Instant};
    std::thread::sleep(Duration::from_millis(super::stream::PREVIEW_FIRST_MS));
    let mut next = Instant::now();
    loop {
        let alive = |st: &State| st.own.as_ref().is_some_and(|o| o.generation == generation);
        if Instant::now() >= next && !super::stream::hide_preview() {
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
    st.watches.insert(key.clone(), Watch { generation: st.stream_generation, owner: k.owner, rtc_server_id: None, conn: None, stats: Default::default(), view: None });
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
            VoiceEvent::Video { codec, keyframe, frame, .. } => {
                if push_video(streams(), owner, codec, keyframe, &frame, phoenix_voice::media::ffmpeg::STREAM_MAX)
                    && let Some(c) = manager().state.lock().watches.get(&key).and_then(|w| w.conn.clone())
                {
                    c.request_keyframe(owner);
                }
            }
            VoiceEvent::Stats(s) => {
                if let Some(w) = manager().state.lock().watches.get_mut(&key) {
                    w.stats = s;
                }
            }
            VoiceEvent::VideoState { active: false, .. } => {
                streams().lock().remove(&owner);
            }
            VoiceEvent::Reconnecting => ping_on_reconnect(Some(&key)),
            VoiceEvent::Disconnected { .. } => {
                let mut st = manager().state.lock();
                stop_watch(&mut st, &key);
                return;
            }
            _ => {}
        }
    }
}

const MAX_RETRIES: u32 = 3;

fn retry_delay(retryable: bool, retries: u32) -> Option<std::time::Duration> {
    (retryable && retries < MAX_RETRIES).then(|| std::time::Duration::from_millis(1000 << retries))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnVoice {
    Ignore,
    Left,
    Joined(Target),
    Moved(Target),
}

fn own_voice_state(target: Target, joining: bool, guild: Option<u64>, channel: Option<u64>) -> OwnVoice {
    match channel {
        Some(c) if c == target.channel => OwnVoice::Joined(Target { guild: guild.or(target.guild), channel: c }),
        Some(c) if guild == target.guild && !joining => OwnVoice::Moved(Target { guild, channel: c }),
        None if guild == target.guild && !joining => OwnVoice::Left,
        _ => OwnVoice::Ignore,
    }
}

pub fn devices() -> (Vec<String>, Vec<String>) {
    (phoenix_voice::audio::device::input_devices(), phoenix_voice::audio::device::output_devices())
}

#[cfg(test)]
mod tests {
    use super::{push_video, remote, remote_frame, stream_frame, streams};
    use crate::model::Id;
    use phoenix_voice::codec::Codec;
    use phoenix_voice::media::ffmpeg;
    use phoenix_voice::rtp::RtpHeader;
    use phoenix_voice::video_rx::{self, Layer, VideoReceiver};
    use std::time::{Duration, Instant};

    fn inject(map: &'static parking_lot::Mutex<super::Decoders>, user: u64, codec: Codec, frames: &[Vec<u8>], max: (u32, u32)) -> usize {
        let mut rx = VideoReceiver::default();
        rx.set_layers(user, vec![Layer { ssrc: 77, rtx_ssrc: 0, quality: 100 }]);
        let now = Instant::now();
        rx.push(&RtpHeader::new(codec.payload_type(), 0, 0, 77, true), &[0x10, 0, 0, 0, 0x65], now);
        let packets = video_rx::rtp_packets(codec, frames, 77, 1, 1100).expect("packetize");
        let mut pushed = 0;
        for (h, p) in &packets {
            for f in rx.push(h, p, now) {
                let key = video_rx::is_keyframe(codec, &f.data);
                assert!(rx.accept(&f, key), "{} frame refused", codec.name());
                assert!(!push_video(map, user, codec, key, &f.data, max), "{} decoder asked for a keyframe", codec.name());
                pushed += 1;
            }
        }
        let t = Instant::now();
        while map.lock().get(&user).map_or(0, |d| d.decoded()) < pushed as u64 && t.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(20));
        }
        pushed
    }

    fn ready(codec: Codec) -> bool {
        ffmpeg::available() && ffmpeg::decoder_support().codecs.contains(&codec)
    }

    #[test]
    fn synthetic_cameras_reach_the_call_tile_image() {
        for (i, codec) in [Codec::H264, Codec::Vp8, Codec::Av1].into_iter().enumerate() {
            if !ready(codec) {
                continue;
            }
            let Some(frames) = ffmpeg::synthetic_frames(codec, "color=c=0x00ff00:s=320x180:r=15", 4) else { continue };
            let user = 9_100 + i as u64;
            assert_eq!(inject(remote(), user, codec, &frames, ffmpeg::CAMERA_MAX), frames.len(), "{}", codec.name());
            let f = remote_frame(user).unwrap_or_else(|| panic!("{} produced no frame", codec.name()));
            assert_eq!((f.width, f.height), (320, 180), "{}", codec.name());
            let px = &f.data[(90 * 320 + 160) * 4..][..4];
            assert!(px[1] > 200 && px[0] < 60 && px[2] < 60, "{} centre pixel {px:?} is not green", codec.name());
            let img = crate::ui::call_view::remote_image(Id(user)).expect("tile image");
            let size = img.size(0);
            assert_eq!((size.width.0, size.height.0), (320, 180));
            assert!(std::sync::Arc::ptr_eq(&img, &crate::ui::call_view::remote_image(Id(user)).expect("cached image")));
            assert_eq!(crate::ui::call_view::remote_aspect(Id(user)), Some(16. / 9.));
            remote().lock().remove(&user);
            assert!(remote_frame(user).is_none());
            assert!(crate::ui::call_view::remote_image(Id(user)).is_none());
        }
    }

    #[test]
    fn a_portrait_camera_keeps_its_ratio_for_letterboxing() {
        if !ready(Codec::Vp8) {
            return;
        }
        let Some(frames) = ffmpeg::synthetic_frames(Codec::Vp8, "testsrc2=s=360x640:r=15", 3) else { return };
        let user = 9_200;
        inject(remote(), user, Codec::Vp8, &frames, ffmpeg::CAMERA_MAX);
        let a = crate::ui::call_view::remote_aspect(Id(user)).expect("aspect");
        assert!((a - 360. / 640.).abs() < 0.01, "{a}");
        assert!(matches!(crate::ui::call_view::video_fit(true, Some(a)), gpui::ObjectFit::Contain));
        assert!(matches!(crate::ui::call_view::video_fit(false, Some(16. / 9.)), gpui::ObjectFit::Contain));
        assert!(matches!(crate::ui::call_view::video_fit(true, Some(16. / 9.)), gpui::ObjectFit::Cover));
        remote().lock().remove(&user);
    }

    #[test]
    fn a_watched_go_live_reaches_the_stream_tile_and_is_capped() {
        if !ready(Codec::H264) {
            return;
        }
        let Some(frames) = ffmpeg::synthetic_frames(Codec::H264, "testsrc2=s=2560x1440:r=15", 3) else { return };
        let owner = 9_300;
        assert_eq!(inject(streams(), owner, Codec::H264, &frames, ffmpeg::STREAM_MAX), frames.len());
        let f = stream_frame(owner).expect("stream frame");
        assert_eq!((f.width, f.height), ffmpeg::STREAM_MAX);
        let img = crate::ui::call_view::stream_image(Id(owner), false).expect("stream image");
        assert_eq!((img.size(0).width.0 as u32, img.size(0).height.0 as u32), ffmpeg::STREAM_MAX);
        streams().lock().remove(&owner);
        assert!(crate::ui::call_view::stream_image(Id(owner), false).is_none());
    }

    #[test]
    fn delta_frames_without_a_decoder_ask_for_a_keyframe() {
        if !ready(Codec::Vp8) {
            return;
        }
        let Some(frames) = ffmpeg::synthetic_frames(Codec::Vp8, "testsrc2=s=160x90:r=15", 3) else { return };
        let user = 9_400;
        assert!(video_rx::is_keyframe(Codec::Vp8, &frames[0]));
        assert!(!video_rx::is_keyframe(Codec::Vp8, &frames[1]));
        assert!(push_video(remote(), user, Codec::Vp8, false, &frames[1], ffmpeg::CAMERA_MAX));
        assert!(!remote().lock().contains_key(&user));
        assert!(!push_video(remote(), user, Codec::Vp8, true, &frames[0], ffmpeg::CAMERA_MAX));
        assert!(remote().lock().contains_key(&user));
        if ready(Codec::H264)
            && let Some(h264) = ffmpeg::synthetic_frames(Codec::H264, "testsrc2=s=160x90:r=15", 1)
        {
            assert!(!push_video(remote(), user, Codec::H264, true, &h264[0], ffmpeg::CAMERA_MAX));
            assert_eq!(remote().lock().get(&user).map(|d| d.codec()), Some(Codec::H264));
        }
        remote().lock().remove(&user);
    }
}

#[cfg(test)]
mod voice_state_tests {
    use super::*;

    const A: Target = Target { guild: Some(10), channel: 100 };
    const B: Target = Target { guild: Some(20), channel: 200 };

    #[test]
    fn leaving_the_old_guild_keeps_the_new_target() {
        assert_eq!(own_voice_state(B, false, Some(10), None), OwnVoice::Ignore);
        assert_eq!(own_voice_state(B, false, Some(20), Some(200)), OwnVoice::Joined(B));
        assert_eq!(own_voice_state(A, false, Some(20), None), OwnVoice::Ignore);
        assert_eq!(own_voice_state(A, false, Some(10), Some(100)), OwnVoice::Joined(A));
    }

    #[test]
    fn a_late_state_of_another_guild_is_ignored() {
        assert_eq!(own_voice_state(B, false, Some(10), Some(100)), OwnVoice::Ignore);
    }

    #[test]
    fn leaving_and_moving_in_the_same_guild() {
        assert_eq!(own_voice_state(A, false, Some(10), None), OwnVoice::Left);
        assert_eq!(own_voice_state(A, false, Some(10), Some(101)), OwnVoice::Moved(Target { guild: Some(10), channel: 101 }));
    }

    #[test]
    fn the_old_state_of_a_quick_rejoin_is_ignored() {
        let a2 = Target { guild: Some(10), channel: 101 };
        assert_eq!(own_voice_state(a2, true, Some(10), None), OwnVoice::Ignore);
        assert_eq!(own_voice_state(a2, true, Some(10), Some(100)), OwnVoice::Ignore);
        assert_eq!(own_voice_state(a2, true, Some(10), Some(101)), OwnVoice::Joined(a2));
    }

    #[test]
    fn private_calls_have_no_guild() {
        let dm = Target { guild: None, channel: 300 };
        assert_eq!(own_voice_state(dm, false, None, Some(300)), OwnVoice::Joined(dm));
        assert_eq!(own_voice_state(dm, false, None, None), OwnVoice::Left);
        assert_eq!(own_voice_state(dm, false, Some(10), None), OwnVoice::Ignore);
        assert_eq!(own_voice_state(Target { guild: None, channel: 100 }, false, Some(10), Some(100)), OwnVoice::Joined(A));
    }

    #[test]
    fn a_dropped_connection_is_retried_three_times() {
        let d: Vec<_> = (0..4).map(|n| retry_delay(true, n).map(|d| d.as_millis())).collect();
        assert_eq!(d, [Some(1000), Some(2000), Some(4000), None]);
        assert_eq!(retry_delay(false, 0), None);
    }

    #[test]
    fn switching_guilds_reaches_a_startable_state() {
        let mut st = State { target: Some(B), ..Default::default() };
        for (g, c) in [(Some(10), None), (Some(20), Some(200))] {
            match own_voice_state(st.target.unwrap(), true, g, c) {
                OwnVoice::Joined(t) | OwnVoice::Moved(t) => {
                    st.target = Some(t);
                    st.session_id = Some("s2".into());
                }
                OwnVoice::Left => st.target = None,
                OwnVoice::Ignore => {}
            }
        }
        st.server = Some(("new".into(), "t2".into()));
        assert_eq!(st.target, Some(B));
        assert!(st.session_id.is_some() && st.server.is_some() && st.running.is_none());
    }
}
