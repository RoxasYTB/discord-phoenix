use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::codec::{Codec, LocalCapabilities};
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
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ConnStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_lost: u64,
    pub outbound_loss_pct: Option<f32>,
}

pub struct VoiceConfig {
    pub info: VoiceConnectInfo,
    pub channel_id: u64,
    pub user_id: u64,
}

enum Cmd {
    Close,
}

#[derive(Clone)]
pub struct VoiceConnection {
    tx: mpsc::UnboundedSender<Cmd>,
}

impl VoiceConnection {
    pub fn close(&self) {
        let _ = self.tx.send(Cmd::Close);
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
    let mut ssrc_users: HashMap<u32, u64> = HashMap::new();
    let mut last_heard: HashMap<u64, Instant> = HashMap::new();
    let mut was_e2ee = false;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut recv_buf_errors = 0u32;
    let mut audio_open = true;
    let mut stats = ConnStats::default();
    let mut last_seq: HashMap<u32, u16> = HashMap::new();
    let mut last_code: Option<String> = None;
    let mut ticks = 0u32;

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
                            let caps = LocalCapabilities::default();
                            gw.select_protocol(&ip, port, mode.name(), &caps.offer())?;
                            media = Some(Media {
                                socket,
                                ssrc: r.ssrc,
                                sender: RtpSender::new(r.ssrc, Codec::Opus.payload_type()),
                                timestamp: rand::random(),
                                transmitting: false,
                                silence_left: 0,
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
                                ssrc_users.insert(ssrc, uid);
                                dave.add_users([uid]);
                            }
                        }
                        ServerMessage::Video(v) => {
                            if let Some(uid) = v.user_id.as_deref().and_then(|u| u.parse::<u64>().ok())
                                && v.audio_ssrc != 0
                            {
                                ssrc_users.insert(v.audio_ssrc, uid);
                                dave.add_users([uid]);
                            }
                        }
                        ServerMessage::ClientsConnect { user_ids } => {
                            dave.add_users(user_ids.iter().filter_map(|u| u.parse().ok()));
                        }
                        ServerMessage::ClientDisconnect { user_id } => {
                            if let Ok(uid) = user_id.parse::<u64>() {
                                dave.remove_user(uid);
                                ssrc_users.retain(|_, u| *u != uid);
                                if last_heard.remove(&uid).is_some() {
                                    let _ = events.send(VoiceEvent::Speaking { user_id: uid, speaking: false });
                                }
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
                let Some(&uid) = ssrc_users.get(&h.ssrc) else { continue };
                let opus = match dave.decrypt(uid, payload) {
                    Ok(f) => f,
                    Err(e) => {
                        recv_buf_errors += 1;
                        if recv_buf_errors.is_power_of_two() {
                            tracing::debug!(error = %e, user = uid, "voice: frame decrypt failed");
                        }
                        continue;
                    }
                };
                if opus != OPUS_SILENCE && last_heard.insert(uid, Instant::now()).is_none() {
                    let _ = events.send(VoiceEvent::Speaking { user_id: uid, speaking: true });
                }
                audio_in(AudioIn { user_id: uid, ssrc: h.ssrc, sequence: h.sequence, timestamp: h.timestamp, opus });
            }
            frame = audio_out.recv(), if audio_open => {
                let Some(frame) = frame else { audio_open = false; continue };
                let Some(m) = media.as_mut() else { continue };
                let opus = match frame {
                    AudioOut::Frame(f) => {
                        if !m.transmitting {
                            m.transmitting = true;
                            gw.speaking(speaking::MICROPHONE, m.ssrc)?;
                            let _ = events.send(VoiceEvent::Speaking { user_id: cfg.user_id, speaking: true });
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
                let now = Instant::now();
                last_heard.retain(|uid, t| {
                    let alive = now.duration_since(*t) < SPEAKING_TIMEOUT;
                    if !alive {
                        let _ = events.send(VoiceEvent::Speaking { user_id: *uid, speaking: false });
                    }
                    alive
                });
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
            }
        }
    }
}
