use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::codec::Codec;
use crate::crypto::{EncryptionMode, TransportCipher};
use crate::dave::frame::OPUS_SILENCE;
use crate::dave::openmls_session::OpenMlsSession;
use crate::dave::session::{DaveAction, DaveSession};
use crate::gateway::protocol::speaking;
use crate::gateway::{self, GatewayEvent, GatewayHandle, ReconnectPolicy, ServerMessage, VoiceConnectInfo};
use crate::rtp::{self, RtpSender};
use crate::udp::{Incoming, MediaSocket};
use crate::{Error, Result};

const SPEAKING_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOut {
    Frame(Vec<u8>),
    End,
}

#[derive(Debug, Clone)]
pub struct AudioIn {
    pub user_id: u64,
    pub ssrc: u32,
    pub sequence: u16,
    pub timestamp: u32,
    pub opus: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    Connected { ssrc: u32, mode: String },
    Ping(u32),
    Speaking { user_id: u64, speaking: bool },
    Encryption { e2ee: bool, protocol_version: u16 },
    PrivacyCode(Option<String>),
    Stats(ConnStats),
    Reconnecting,
    Disconnected { reason: String },
    VideoState { user_id: u64, active: bool },
    Video { user_id: u64, frame: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ConnStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_lost: u64,
    pub outbound_loss_pct: Option<f32>,
    pub video_packets_sent: u64,
    pub video_rtcp_received: u64,
    pub video_frames_received: u64,
}

pub struct VoiceConfig {
    pub info: VoiceConnectInfo,
    pub channel_id: u64,
    pub user_id: u64,
}

#[derive(Debug)]
pub enum VideoOut {
    Start { width: u32, height: u32, fps: u32, bitrate: u32 },
    Update { width: u32, height: u32, fps: u32, bitrate: u32, source_resolution: bool },
    Frame(crate::media::EncodedVideo),
    Stop,
}

enum Cmd {
    Close,
    Video(VideoOut),
}

#[derive(Clone)]
pub struct VoiceConnection {
    tx: mpsc::UnboundedSender<Cmd>,
}

impl VoiceConnection {
    pub fn close(&self) {
        let _ = self.tx.send(Cmd::Close);
    }

    pub fn video(&self, v: VideoOut) {
        let _ = self.tx.send(Cmd::Video(v));
    }
}

pub fn connect(
    cfg: VoiceConfig,
    audio_out: mpsc::UnboundedReceiver<AudioOut>,
    audio_in: Box<dyn FnMut(AudioIn) + Send>,
) -> (VoiceConnection, mpsc::UnboundedReceiver<VoiceEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (etx, erx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let reason = match run(cfg, rx, audio_out, audio_in, &etx).await {
            Ok(()) => "disconnected".to_string(),
            Err(e) => e.to_string(),
        };
        let _ = etx.send(VoiceEvent::Disconnected { reason });
    });
    (VoiceConnection { tx }, erx)
}

struct Media {
    socket: MediaSocket,
    ssrc: u32,
    sender: RtpSender,
    timestamp: u32,
    transmitting: bool,
    silence_left: u8,
    video_ssrc: u32,
    video: Option<VideoTx>,
    video_state: VideoSendState,
}

struct VideoTx {
    sender: RtpSender,
    packetizer: crate::rtp::h264::H264Packetizer,
    base: u32,
}

pub const CAMERA_MAX_BITRATE: u32 = 2_500_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSendState {
    kind: &'static str,
    audio_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
    active: bool,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    source_resolution: bool,
}

impl VideoSendState {
    pub fn new(stream: bool, audio_ssrc: u32, video_ssrc: u32, rtx_ssrc: u32) -> Self {
        Self {
            kind: if stream { "screen" } else { "video" },
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            active: false,
            width: 1280,
            height: 720,
            fps: 20,
            bitrate: CAMERA_MAX_BITRATE,
            source_resolution: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn start(&mut self, width: u32, height: u32, fps: u32, bitrate: u32) -> Option<gateway::protocol::VideoPayload> {
        if self.video_ssrc == 0 {
            return None;
        }
        self.active = true;
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.bitrate = bitrate;
        self.source_resolution = false;
        Some(self.payload())
    }

    pub fn update(
        &mut self,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        source_resolution: bool,
    ) -> Option<gateway::protocol::VideoPayload> {
        if !self.active || self.video_ssrc == 0 {
            return None;
        }
        if (self.width, self.height, self.fps, self.bitrate, self.source_resolution)
            == (width, height, fps, bitrate, source_resolution)
        {
            return None;
        }
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.bitrate = bitrate;
        self.source_resolution = source_resolution;
        Some(self.payload())
    }

    pub fn stop(&mut self) -> Option<gateway::protocol::VideoPayload> {
        if !self.active {
            return None;
        }
        self.active = false;
        Some(self.payload())
    }

    pub fn payload(&self) -> gateway::protocol::VideoPayload {
        use gateway::protocol::{Resolution, StreamDescriptor, VideoPayload};
        let max_resolution = if self.active && self.source_resolution {
            Resolution { kind: "source".into(), width: 0, height: 0 }
        } else if self.active {
            Resolution { kind: "fixed".into(), width: self.width, height: self.height }
        } else {
            Resolution { kind: "fixed".into(), width: 1280, height: 720 }
        };
        let stream = StreamDescriptor {
            kind: self.kind.into(),
            rid: "100".into(),
            ssrc: self.video_ssrc,
            rtx_ssrc: self.rtx_ssrc,
            active: self.active,
            quality: 100,
            max_bitrate: Some(self.bitrate),
            max_framerate: Some(if self.active { self.fps } else { 20 }),
            max_resolution: Some(max_resolution),
        };
        VideoPayload {
            user_id: None,
            audio_ssrc: self.audio_ssrc,
            video_ssrc: if self.active { self.video_ssrc } else { 0 },
            rtx_ssrc: if self.active { self.rtx_ssrc } else { 0 },
            streams: vec![stream],
        }
    }
}

pub fn video_payload(audio_ssrc: u32, video_ssrc: u32, rtx_ssrc: u32, active: Option<(u32, u32, u32)>) -> gateway::protocol::VideoPayload {
    stream_payload("video", audio_ssrc, video_ssrc, rtx_ssrc, active, CAMERA_MAX_BITRATE)
}

pub fn stream_payload(
    kind: &str,
    audio_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
    active: Option<(u32, u32, u32)>,
    max_bitrate: u32,
) -> gateway::protocol::VideoPayload {
    use gateway::protocol::{Resolution, StreamDescriptor, VideoPayload};
    let stream = StreamDescriptor {
        kind: kind.into(),
        rid: "100".into(),
        ssrc: video_ssrc,
        rtx_ssrc,
        active: active.is_some(),
        quality: 100,
        max_bitrate: Some(max_bitrate),
        max_framerate: Some(active.map_or(20, |(_, _, fps)| fps)),
        max_resolution: Some(active.map_or(Resolution { kind: "fixed".into(), width: 1280, height: 720 }, |(width, height, _)| Resolution {
            kind: "fixed".into(),
            width,
            height,
        })),
    };
    VideoPayload {
        user_id: None,
        audio_ssrc,
        video_ssrc: if active.is_some() { video_ssrc } else { 0 },
        rtx_ssrc: if active.is_some() { rtx_ssrc } else { 0 },
        streams: vec![stream],
    }
}

pub const UNMAPPED_HOLD: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RxPacket {
    pub ssrc: u32,
    pub sequence: u16,
    pub timestamp: u32,
    pub payload: Vec<u8>,
}

#[derive(Default)]
pub struct Receiver {
    ssrc_users: HashMap<u32, u64>,
    unmapped: HashMap<u32, std::collections::VecDeque<RxPacket>>,
    last_heard: HashMap<u64, Instant>,
    decrypt_errors: u32,
}

impl Receiver {
    pub fn user_for(&self, ssrc: u32) -> Option<u64> {
        self.ssrc_users.get(&ssrc).copied()
    }

    pub fn map_ssrc(&mut self, ssrc: u32, uid: u64) -> Vec<RxPacket> {
        if ssrc == 0 {
            return Vec::new();
        }
        self.ssrc_users.retain(|s, u| *u != uid || *s == ssrc);
        self.ssrc_users.insert(ssrc, uid);
        self.unmapped.remove(&ssrc).map(Vec::from).unwrap_or_default()
    }

    pub fn hold(&mut self, p: RxPacket) {
        if !self.unmapped.contains_key(&p.ssrc) && self.unmapped.len() >= 32 {
            return;
        }
        let q = self.unmapped.entry(p.ssrc).or_default();
        q.push_back(p);
        while q.len() > UNMAPPED_HOLD {
            q.pop_front();
        }
    }

    pub fn remove_user(&mut self, uid: u64, events: &mpsc::UnboundedSender<VoiceEvent>) {
        self.ssrc_users.retain(|_, u| *u != uid);
        if self.last_heard.remove(&uid).is_some() {
            let _ = events.send(VoiceEvent::Speaking { user_id: uid, speaking: false });
        }
    }

    pub fn deliver(
        &mut self,
        uid: u64,
        p: RxPacket,
        dave: &mut DaveSession,
        audio_in: &mut (dyn FnMut(AudioIn) + Send),
        events: &mpsc::UnboundedSender<VoiceEvent>,
    ) {
        let opus = match dave.decrypt(uid, &p.payload) {
            Ok(f) => f,
            Err(e) => {
                self.decrypt_errors += 1;
                if self.decrypt_errors.is_power_of_two() {
                    tracing::debug!(error = %e, user = uid, "voice: frame decrypt failed");
                }
                return;
            }
        };
        if opus != OPUS_SILENCE && self.last_heard.insert(uid, Instant::now()).is_none() {
            let _ = events.send(VoiceEvent::Speaking { user_id: uid, speaking: true });
        }
        audio_in(AudioIn { user_id: uid, ssrc: p.ssrc, sequence: p.sequence, timestamp: p.timestamp, opus });
    }

    pub fn expire(&mut self, now: Instant, events: &mpsc::UnboundedSender<VoiceEvent>) {
        self.last_heard.retain(|uid, t| {
            let alive = now.duration_since(*t) < SPEAKING_TIMEOUT;
            if !alive {
                let _ = events.send(VoiceEvent::Speaking { user_id: *uid, speaking: false });
            }
            alive
        });
    }
}

fn send_dave_actions(gw: &GatewayHandle, actions: Vec<DaveAction>) {
    for a in actions {
        let r = match a {
            DaveAction::KeyPackage(kp) => gw.dave_key_package(kp),
            DaveAction::CommitWelcome(cw) => gw.dave_commit_welcome(&cw),
            DaveAction::TransitionReady(t) => gw.dave_transition_ready(t),
            DaveAction::InvalidCommitWelcome(t) => gw.dave_invalid_commit_welcome(t),
        };
        if let Err(e) = r {
            tracing::warn!(error = %e, "dave: send failed");
        }
    }
}

async fn run(
    cfg: VoiceConfig,
    mut cmds: mpsc::UnboundedReceiver<Cmd>,
    mut audio_out: mpsc::UnboundedReceiver<AudioOut>,
    mut audio_in: Box<dyn FnMut(AudioIn) + Send>,
    events: &mpsc::UnboundedSender<VoiceEvent>,
) -> Result<()> {
    let (gw, mut gev, _task) = gateway::spawn(cfg.info.clone(), None, ReconnectPolicy::default());
    let mls = OpenMlsSession::new(cfg.user_id, cfg.channel_id)?;
    let mut dave = DaveSession::new(Box::new(mls), cfg.user_id);
    let mut media: Option<Media> = None;
    let mut rx = Receiver::default();
    let mut was_e2ee = false;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut recv_buf_errors = 0u32;
    let mut audio_open = true;
    let mut stats = ConnStats::default();
    let mut last_seq: HashMap<u32, u16> = HashMap::new();
    let mut last_code: Option<String> = None;
    let mut ticks = 0u32;
    let mut video_users: HashMap<u32, u64> = HashMap::new();
    let mut depack: HashMap<u32, crate::rtp::h264::H264Depacketizer> = HashMap::new();

    loop {
        tokio::select! {
            ev = gev.recv() => {
                let Some(ev) = ev else { return Err(Error::Closed { code: None, reason: "gateway task ended".into() }) };
                match ev {
                    GatewayEvent::Message(m) => match m {
                        ServerMessage::Ready(r) => {
                            let addr = (r.ip.as_str(), r.port)
                                .to_socket_addrs()?
                                .next()
                                .ok_or(Error::Protocol("voice server address".into()))?;
                            let socket = MediaSocket::connect(addr).await?;
                            let (ip, port) = socket.discover(r.ssrc).await?;
                            let mode = EncryptionMode::choose(&r.modes).ok_or(Error::Unsupported("no common encryption mode".into()))?;
                            let caps = crate::media::video::probe_capabilities();
                            gw.select_protocol(&ip, port, mode.name(), &caps.offer())?;
                            let want = if cfg.info.stream { "screen" } else { "video" };
                            if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
                                let list: Vec<_> = r.streams.iter().map(|s| (s.kind.as_str(), s.rid.as_str(), s.ssrc, s.rtx_ssrc)).collect();
                                eprintln!("voice-trace: ready stream={} streams={list:?}", cfg.info.stream);
                            }
                            let vs = r
                                .streams
                                .iter()
                                .find(|s| s.kind == want)
                                .or_else(|| r.streams.iter().find(|s| s.ssrc != 0))
                                .map_or((0, 0), |s| (s.ssrc, s.rtx_ssrc));
                            media = Some(Media {
                                socket,
                                ssrc: r.ssrc,
                                sender: RtpSender::new(r.ssrc, Codec::Opus.payload_type()),
                                timestamp: rand::random(),
                                transmitting: false,
                                silence_left: 0,
                                video_ssrc: vs.0,
                                video: None,
                                video_state: VideoSendState::new(cfg.info.stream, r.ssrc, vs.0, vs.1),
                            });
                        }
                        ServerMessage::SessionDescription(sd) => {
                            let m = media.as_mut().ok_or(Error::Protocol("session description before ready".into()))?;
                            let mode = EncryptionMode::from_name(&sd.mode).ok_or(Error::Unsupported(sd.mode.clone()))?;
                            m.socket.set_cipher(TransportCipher::new(mode, &sd.secret_key)?);
                            send_dave_actions(&gw, dave.on_session_description(sd.dave_protocol_version));
                            let _ = events.send(VoiceEvent::Connected { ssrc: m.ssrc, mode: sd.mode.clone() });
                        }
                        ServerMessage::HeartbeatAck { nonce } => {
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                            if nonce > 0 && nonce <= now && now - nonce < 60_000 {
                                let _ = events.send(VoiceEvent::Ping((now - nonce) as u32));
                            }
                        }
                        ServerMessage::Speaking { user_id, ssrc, .. } => {
                            if let Some(uid) = user_id.and_then(|u| u.parse::<u64>().ok()) {
                                dave.add_users([uid]);
                                for p in rx.map_ssrc(ssrc, uid) {
                                    rx.deliver(uid, p, &mut dave, &mut *audio_in, events);
                                }
                            }
                        }
                        ServerMessage::Video(v) => {
                            if let Some(uid) = v.user_id.as_deref().and_then(|u| u.parse::<u64>().ok()) {
                                if v.audio_ssrc != 0 {
                                    dave.add_users([uid]);
                                    for p in rx.map_ssrc(v.audio_ssrc, uid) {
                                        rx.deliver(uid, p, &mut dave, &mut *audio_in, events);
                                    }
                                }
                                video_users.retain(|_, u| *u != uid);
                                if v.video_ssrc != 0 {
                                    video_users.insert(v.video_ssrc, uid);
                                }
                                let _ = events.send(VoiceEvent::VideoState { user_id: uid, active: v.video_ssrc != 0 });
                            }
                        }
                        ServerMessage::ClientsConnect { user_ids } => {
                            dave.add_users(user_ids.iter().filter_map(|u| u.parse().ok()));
                        }
                        ServerMessage::ClientDisconnect { user_id } => {
                            if let Ok(uid) = user_id.parse::<u64>() {
                                dave.remove_user(uid);
                                rx.remove_user(uid, events);
                            }
                        }
                        ServerMessage::Dave(d) => {
                            let actions = dave.handle(&d);
                            send_dave_actions(&gw, actions);
                        }
                        _ => {}
                    },
                    GatewayEvent::Reconnecting { .. } => {
                        let _ = events.send(VoiceEvent::Reconnecting);
                    }
                    GatewayEvent::Closed { code, reason } => {
                        return if code == Some(1000) { Ok(()) } else { Err(Error::Closed { code, reason }) };
                    }
                }
                let e2ee = dave.is_active();
                if e2ee != was_e2ee {
                    was_e2ee = e2ee;
                    let _ = events.send(VoiceEvent::Encryption { e2ee, protocol_version: dave.protocol_version() });
                }
                let code = if e2ee { dave.epoch_authenticator().and_then(|a| crate::dave::privacy_code(&a)) } else { None };
                if code != last_code {
                    last_code = code.clone();
                    let _ = events.send(VoiceEvent::PrivacyCode(code));
                }
            }
            pkt = async {
                match media.as_ref() {
                    Some(m) => m.socket.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                let pkt = match pkt {
                    Ok(Incoming::Rtp(p)) => p,
                    Ok(Incoming::Rtcp(r)) => {
                        if let Some(m) = media.as_ref()
                            && m.video.is_some()
                            && r.windows(4).any(|w| w == m.video_ssrc.to_be_bytes())
                        {
                            stats.video_rtcp_received += 1;
                        }
                        if let Some(m) = media.as_ref()
                            && let Some((_, frac)) = rtp::rtcp::report_blocks(&r).into_iter().find(|(ssrc, _)| *ssrc == m.ssrc)
                        {
                            stats.outbound_loss_pct = Some(frac as f32 * 100.0 / 256.0);
                        }
                        continue;
                    }
                    Err(Error::Io(e)) => return Err(Error::Io(e)),
                    Err(e) => {
                        recv_buf_errors += 1;
                        if recv_buf_errors.is_power_of_two() {
                            tracing::debug!(error = %e, "voice: dropped packet");
                        }
                        continue;
                    }
                };
                let Ok((h, payload)) = rtp::parse_packet(&pkt) else { continue };
                if h.payload_type == Codec::H264.payload_type() {
                    let Some(&uid) = video_users.get(&h.ssrc) else { continue };
                    let d = depack.entry(h.ssrc).or_default();
                    let au = match rtp::Depacketizer::push(d, payload, h.marker) {
                        Ok(Some(au)) => au,
                        Ok(None) => continue,
                        Err(_) => {
                            *d = Default::default();
                            continue;
                        }
                    };
                    stats.video_frames_received += 1;
                    if let Ok(frame) = dave.decrypt(uid, &au) {
                        let _ = events.send(VoiceEvent::Video { user_id: uid, frame });
                    }
                    continue;
                }
                if h.payload_type != Codec::Opus.payload_type() {
                    continue;
                }
                stats.packets_received += 1;
                if let Some(prev) = last_seq.insert(h.ssrc, h.sequence) {
                    let d = h.sequence.wrapping_sub(prev);
                    if (2..1000).contains(&d) {
                        stats.packets_lost += u64::from(d - 1);
                    } else if d == 0 || d >= 32768 {
                        last_seq.insert(h.ssrc, prev);
                    }
                }
                let p = RxPacket { ssrc: h.ssrc, sequence: h.sequence, timestamp: h.timestamp, payload: payload.to_vec() };
                match rx.user_for(h.ssrc) {
                    Some(uid) => rx.deliver(uid, p, &mut dave, &mut *audio_in, events),
                    None => rx.hold(p),
                }
            }
            frame = audio_out.recv(), if audio_open => {
                let Some(frame) = frame else { audio_open = false; continue };
                let Some(m) = media.as_mut() else { continue };
                let opus = match frame {
                    AudioOut::Frame(f) => {
                        if !m.transmitting {
                            m.transmitting = true;
                            gw.speaking(if cfg.info.stream { speaking::SOUNDSHARE } else { speaking::MICROPHONE }, m.ssrc)?;
                            if !cfg.info.stream {
                                let _ = events.send(VoiceEvent::Speaking { user_id: cfg.user_id, speaking: true });
                            }
                        }
                        m.silence_left = 5;
                        f
                    }
                    AudioOut::End => {
                        if m.transmitting {
                            m.transmitting = false;
                            let _ = events.send(VoiceEvent::Speaking { user_id: cfg.user_id, speaking: false });
                        }
                        if m.silence_left == 0 {
                            continue;
                        }
                        m.silence_left -= 1;
                        if m.silence_left == 0 {
                            gw.speaking(0, m.ssrc)?;
                        }
                        OPUS_SILENCE.to_vec()
                    }
                };
                let Ok(payload) = dave.encrypt(Codec::Opus, &opus) else { continue };
                m.timestamp = m.timestamp.wrapping_add(crate::FRAME_SAMPLES as u32);
                for p in m.sender.wrap(vec![payload], m.timestamp) {
                    match m.socket.send_rtp(&p).await {
                        Ok(()) => stats.packets_sent += 1,
                        Err(e) => tracing::debug!(error = %e, "voice: send failed"),
                    }
                }
            }
            _ = tick.tick() => {
                rx.expire(Instant::now(), events);
                ticks += 1;
                if ticks.is_multiple_of(10) && media.is_some() {
                    let _ = events.send(VoiceEvent::Stats(stats));
                }
            }
            cmd = cmds.recv() => match cmd {
                Some(Cmd::Close) | None => {
                    let _ = gw.close();
                    return Ok(());
                }
                Some(Cmd::Video(v)) => {
                    let Some(m) = media.as_mut() else { continue };
                    match v {
                        VideoOut::Start { width, height, fps, bitrate } => {
                            let Some(p) = m.video_state.start(width, height, fps, bitrate) else {
                                tracing::warn!("voice: no video ssrc from ready");
                                if std::env::var_os("PHOENIX_VOICE_TRACE").is_some() {
                                    eprintln!("voice-trace: no video ssrc from ready, video not started");
                                }
                                continue;
                            };
                            gw.video(&p)?;
                            m.video = Some(VideoTx {
                                sender: RtpSender::new(m.video_ssrc, Codec::H264.payload_type()),
                                packetizer: crate::rtp::h264::H264Packetizer::new(),
                                base: rand::random(),
                            });
                        }
                        VideoOut::Update { width, height, fps, bitrate, source_resolution } => {
                            if let Some(p) = m.video_state.update(width, height, fps, bitrate, source_resolution) {
                                gw.video(&p)?;
                            }
                        }
                        VideoOut::Stop => {
                            if m.video.take().is_some()
                                && let Some(p) = m.video_state.stop()
                            {
                                gw.video(&p)?;
                            }
                        }
                        VideoOut::Frame(e) => {
                            let Some(vt) = m.video.as_mut() else { continue };
                            let au = vt.packetizer.prepare_access_unit(&e.data);
                            let Ok(au) = dave.encrypt(Codec::H264, &au) else { continue };
                            let Ok(payloads) = vt.packetizer.packetize_prepared(&au, rtp::DEFAULT_MAX_PAYLOAD) else { continue };
                            let ts = vt.base.wrapping_add((e.pts_us.wrapping_mul(90) / 1000) as u32);
                            for p in vt.sender.wrap(payloads, ts) {
                                match m.socket.send_rtp(&p).await {
                                    Ok(()) => stats.video_packets_sent += 1,
                                    Err(e) => tracing::debug!(error = %e, "voice: video send failed"),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod video_tests {
    use super::*;

    #[test]
    fn camera_op12_active_and_stopped() {
        let on = video_payload(1, 2, 3, Some((1280, 720, 30)));
        assert_eq!((on.video_ssrc, on.rtx_ssrc), (2, 3));
        let s = &on.streams[0];
        assert!(s.active);
        assert_eq!(s.max_framerate, Some(30));
        assert_eq!(s.max_resolution.as_ref().map(|r| (r.width, r.height)), Some((1280, 720)));
        let off = video_payload(1, 2, 3, None);
        assert_eq!(off.video_ssrc, 0);
        assert!(!off.streams[0].active);
        assert_eq!(off.streams[0].ssrc, 2);
    }

    #[test]
    fn stream_renegotiation_updates_op12_without_stopping() {
        let mut st = VideoSendState::new(true, 10, 20, 21);
        assert!(!st.is_active());
        assert!(st.update(1920, 1080, 60, 6_000_000, false).is_none());

        let started = st.start(1280, 720, 30, 2_500_000).unwrap();
        assert!(st.is_active());
        assert_eq!((started.audio_ssrc, started.video_ssrc, started.rtx_ssrc), (10, 20, 21));
        assert_eq!(started.streams[0].kind, "screen");
        assert_eq!(started.streams[0].max_framerate, Some(30));
        assert_eq!(started.streams[0].max_bitrate, Some(2_500_000));

        assert!(st.update(1280, 720, 30, 2_500_000, false).is_none());

        let updated = st.update(1920, 1080, 60, 6_000_000, false).unwrap();
        assert!(st.is_active());
        let json: serde_json::Value = serde_json::from_str(&gateway::protocol::video(&updated).unwrap()).unwrap();
        assert_eq!(json["op"], 12);
        assert_eq!(json["d"]["audio_ssrc"], 10);
        assert_eq!(json["d"]["video_ssrc"], 20);
        assert_eq!(json["d"]["rtx_ssrc"], 21);
        assert_eq!(json["d"]["streams"][0]["active"], true);
        assert_eq!(json["d"]["streams"][0]["max_framerate"], 60);
        assert_eq!(json["d"]["streams"][0]["max_bitrate"], 6_000_000);
        assert_eq!(json["d"]["streams"][0]["max_resolution"]["type"], "fixed");
        assert_eq!(json["d"]["streams"][0]["max_resolution"]["width"], 1920);
        assert_eq!(json["d"]["streams"][0]["max_resolution"]["height"], 1080);

        let src = st.update(2560, 1440, 30, 8_000_000, true).unwrap();
        let src_json: serde_json::Value = serde_json::from_str(&gateway::protocol::video(&src).unwrap()).unwrap();
        assert_eq!(src_json["d"]["streams"][0]["max_resolution"]["type"], "source");
        assert_eq!(src_json["d"]["streams"][0]["max_resolution"]["width"], 0);
        assert_eq!(src_json["d"]["streams"][0]["max_resolution"]["height"], 0);

        let stopped = st.stop().unwrap();
        assert!(!st.is_active());
        assert_eq!(stopped.video_ssrc, 0);
        assert!(!stopped.streams[0].active);
        assert!(st.update(1280, 720, 30, 2_500_000, false).is_none());
    }
}

#[cfg(test)]
mod receive_tests {
    use super::*;
    use crate::rtp::RtpHeader;
    use crate::udp::classify_and_decrypt;

    fn wire(tx: &mut TransportCipher, ssrc: u32, seq: u16, payload: &[u8]) -> Vec<u8> {
        let mut h = RtpHeader::new(Codec::Opus.payload_type(), seq, seq as u32 * 960, ssrc, false);
        h.extension = Some((0xbede, vec![0x10, 0x7f, 0x00, 0x00]));
        tx.encrypt_rtp(&rtp::build_packet(&h, payload)).unwrap()
    }

    fn unwrap_wire(rx: &TransportCipher, w: &[u8]) -> RxPacket {
        let Incoming::Rtp(p) = classify_and_decrypt(Some(rx), w).unwrap() else { panic!("rtcp") };
        let (h, payload) = rtp::parse_packet(&p).unwrap();
        assert_eq!(h.extension.as_ref().map(|e| e.0), Some(0xbede));
        RxPacket { ssrc: h.ssrc, sequence: h.sequence, timestamp: h.timestamp, payload: payload.to_vec() }
    }

    type Sink = Box<dyn FnMut(AudioIn) + Send>;
    type Got = std::sync::Arc<std::sync::Mutex<Vec<AudioIn>>>;

    fn collect() -> (Sink, Got) {
        let got = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = got.clone();
        (Box::new(move |a| g.lock().unwrap().push(a)), got)
    }

    #[test]
    fn transport_and_dave_layers_peel_off_per_sender() {
        let (mut alice, mut bob) = crate::dave::session::tests::paired_sessions();
        let (etx, mut erx) = mpsc::unbounded_channel();
        let (mut sink, got) = collect();
        for mode in [EncryptionMode::Aes256GcmRtpSize, EncryptionMode::XChaCha20Poly1305RtpSize] {
            let mut tx = TransportCipher::new(mode, &[3; 32]).unwrap();
            let rxc = TransportCipher::new(mode, &[3; 32]).unwrap();
            let mut r = Receiver::default();
            let opus = [0x78u8, 9, 8, 7, 6, 5, 4, 3, 2, 1];
            let e2ee = alice.encrypt(Codec::Opus, &opus).unwrap();
            let p = unwrap_wire(&rxc, &wire(&mut tx, 77, 10, &e2ee));
            assert_eq!(p.payload, e2ee);
            assert!(r.user_for(77).is_none());
            r.hold(p);
            let held = r.map_ssrc(77, 1);
            assert_eq!(held.len(), 1);
            for p in held {
                r.deliver(1, p, &mut bob, &mut *sink, &etx);
            }
            let p = unwrap_wire(&rxc, &wire(&mut tx, 77, 11, &e2ee));
            r.deliver(r.user_for(77).unwrap(), p, &mut bob, &mut *sink, &etx);
            let mut bad = e2ee.clone();
            bad[2] ^= 0x40;
            let p = unwrap_wire(&rxc, &wire(&mut tx, 77, 12, &bad));
            r.deliver(1, p, &mut bob, &mut *sink, &etx);
            let p = unwrap_wire(&rxc, &wire(&mut tx, 77, 13, &opus));
            r.deliver(1, p, &mut bob, &mut *sink, &etx);
        }
        let got = got.lock().unwrap();
        assert!(got.iter().all(|a| a.opus == [0x78u8, 9, 8, 7, 6, 5, 4, 3, 2, 1] && a.user_id == 1 && a.ssrc == 77));
        assert_eq!(got.iter().map(|a| a.sequence).collect::<Vec<_>>(), [10, 11, 13, 10, 11, 13]);
        for _ in 0..2 {
            assert_eq!(erx.try_recv().ok(), Some(VoiceEvent::Speaking { user_id: 1, speaking: true }));
        }
        assert!(erx.try_recv().is_err());
    }

    #[test]
    fn ssrc_remap_hold_limit_and_speaking_timeout() {
        let (etx, mut erx) = mpsc::unbounded_channel();
        let mut dave = DaveSession::new(Box::new(OpenMlsSession::new(1, 9).unwrap()), 1);
        let (mut sink, got) = collect();
        let mut r = Receiver::default();
        for s in 0..40u16 {
            r.hold(RxPacket { ssrc: 5, sequence: s, timestamp: 0, payload: vec![1, 2, 3] });
        }
        let held = r.map_ssrc(5, 42);
        assert_eq!(held.len(), UNMAPPED_HOLD);
        assert_eq!(held[0].sequence, 15);
        assert!(r.map_ssrc(0, 42).is_empty());
        r.map_ssrc(6, 42);
        assert_eq!((r.user_for(5), r.user_for(6)), (None, Some(42)));
        r.deliver(42, RxPacket { ssrc: 6, sequence: 1, timestamp: 0, payload: OPUS_SILENCE.to_vec() }, &mut dave, &mut *sink, &etx);
        assert!(erx.try_recv().is_err());
        r.deliver(42, RxPacket { ssrc: 6, sequence: 2, timestamp: 960, payload: vec![0x78, 1] }, &mut dave, &mut *sink, &etx);
        assert_eq!(erx.try_recv().ok(), Some(VoiceEvent::Speaking { user_id: 42, speaking: true }));
        r.expire(Instant::now(), &etx);
        assert!(erx.try_recv().is_err());
        r.expire(Instant::now() + SPEAKING_TIMEOUT, &etx);
        assert_eq!(erx.try_recv().ok(), Some(VoiceEvent::Speaking { user_id: 42, speaking: false }));
        r.deliver(42, RxPacket { ssrc: 6, sequence: 3, timestamp: 1920, payload: vec![0x78, 1] }, &mut dave, &mut *sink, &etx);
        r.remove_user(42, &etx);
        assert_eq!(erx.try_recv().ok(), Some(VoiceEvent::Speaking { user_id: 42, speaking: true }));
        assert_eq!(erx.try_recv().ok(), Some(VoiceEvent::Speaking { user_id: 42, speaking: false }));
        assert!(r.user_for(6).is_none());
        assert_eq!(got.lock().unwrap().len(), 3);
    }

    #[cfg(feature = "opus")]
    #[test]
    fn received_packets_reach_the_mixer() {
        use crate::audio::AudioControl;
        use crate::audio::mixer::Mixer;
        use crate::media::AudioEncoder;
        use crate::media::opus::OpusEncoder;
        let (mut alice, mut bob) = crate::dave::session::tests::paired_sessions();
        let (etx, _erx) = mpsc::unbounded_channel();
        let mixer = std::sync::Arc::new(std::sync::Mutex::new(Mixer::default()));
        let m2 = mixer.clone();
        let mut sink: Box<dyn FnMut(AudioIn) + Send> = Box::new(move |a| m2.lock().unwrap().push(a));
        let mut tx = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &[9; 32]).unwrap();
        let rxc = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &[9; 32]).unwrap();
        let mut enc = OpusEncoder::new(true, 64_000).unwrap();
        let pcm: Vec<i16> = (0..crate::FRAME_SAMPLES * 2).map(|i| (((i / 2) as f32 * 0.06).sin() * 10000.0) as i16).collect();
        let mut r = Receiver::default();
        r.map_ssrc(300, 1);
        for s in 0..8u16 {
            let e2ee = alice.encrypt(Codec::Opus, &enc.encode(&pcm).unwrap()).unwrap();
            let p = unwrap_wire(&rxc, &wire(&mut tx, 300, s, &e2ee));
            r.deliver(1, p, &mut bob, &mut *sink, &etx);
        }
        let c = AudioControl::default();
        let mut m = mixer.lock().unwrap();
        let e: f32 = (0..6).map(|_| m.mix_frame(&c).iter().map(|s| s * s).sum::<f32>()).sum();
        assert!(e > 1.0);
        let st = m.stats();
        assert_eq!(st[0].0, 1);
        assert_eq!(st[0].1.decoded, 6);
        assert!(st[0].1.level_db > -40.0);
    }
}
