//! Gateway v10 session: JSON encoding, zlib-stream transport compression,
//! heartbeating with zombie detection, identify/resume, reconnect with backoff.
//!
//! ```no_run
//! # async fn f() {
//! use phoenix_api::{Gateway, GatewayConfig, GatewayEvent};
//! let (handle, mut events) = Gateway::spawn(GatewayConfig::new("token"));
//! while let Some(ev) = events.recv().await {
//!     if let GatewayEvent::Ready(r) = ev { println!("logged in as {}", r.user.username); }
//! }
//! # drop(handle); }
//! ```

pub mod events;
pub mod zlib;

use std::collections::VecDeque;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use serde_json::value::RawValue;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

pub use events::*;

use crate::models::{ChannelId, GuildId};
use crate::props::ClientProperties;
use zlib::ZlibStream;

pub const GATEWAY_URL: &str = "wss://gateway.discord.gg";

/// Capability bits sent in Identify (user accounts).
pub mod capabilities {
    pub const LAZY_USER_NOTES: u64 = 1 << 0;
    pub const NO_AFFINE_USER_IDS: u64 = 1 << 1;
    pub const VERSIONED_READ_STATES: u64 = 1 << 2;
    pub const VERSIONED_USER_GUILD_SETTINGS: u64 = 1 << 3;
    pub const DEDUPE_USER_OBJECTS: u64 = 1 << 4;
    pub const PRIORITIZED_READY_PAYLOAD: u64 = 1 << 5;
    pub const MULTIPLE_GUILD_EXPERIMENT_POPULATIONS: u64 = 1 << 6;
    pub const NON_CHANNEL_READ_STATES: u64 = 1 << 7;
    pub const AUTH_TOKEN_REFRESH: u64 = 1 << 8;
    pub const USER_SETTINGS_PROTO: u64 = 1 << 9;
    pub const CLIENT_STATE_V2: u64 = 1 << 10;
    pub const PASSIVE_GUILD_UPDATE: u64 = 1 << 11;
    pub const AUTO_CALL_CONNECT: u64 = 1 << 12;
    pub const DEBOUNCE_MESSAGE_REACTIONS: u64 = 1 << 13;
    pub const PASSIVE_GUILD_UPDATE_V2: u64 = 1 << 14;

    /// The set this crate's models are written against.
    /// PRIORITIZED_READY_PAYLOAD is what makes Discord send READY_SUPPLEMENTAL
    /// (friend / guild presences and voice states at login); without it a
    /// user session gets no presences until each one changes.
    pub const DEFAULT: u64 = LAZY_USER_NOTES
        | NO_AFFINE_USER_IDS
        | PRIORITIZED_READY_PAYLOAD
        | VERSIONED_READ_STATES
        | VERSIONED_USER_GUILD_SETTINGS
        | DEDUPE_USER_OBJECTS
        | NON_CHANNEL_READ_STATES
        | AUTH_TOKEN_REFRESH
        | DEBOUNCE_MESSAGE_REACTIONS;
}

mod op {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
    pub const PRESENCE_UPDATE: u8 = 3;
    pub const VOICE_STATE_UPDATE: u8 = 4;
    pub const RESUME: u8 = 6;
    pub const RECONNECT: u8 = 7;
    pub const REQUEST_GUILD_MEMBERS: u8 = 8;
    pub const INVALID_SESSION: u8 = 9;
    pub const HELLO: u8 = 10;
    pub const HEARTBEAT_ACK: u8 = 11;
    pub const LAZY_REQUEST: u8 = 14;
    pub const REQUEST_SOUNDBOARD_SOUNDS: u8 = 31;
    pub const GUILD_SUBSCRIPTIONS_BULK: u8 = 37;
}

#[derive(Clone)]
pub struct GatewayConfig {
    pub token: String,
    /// Base URL without query (e.g. `wss://gateway.discord.gg`).
    pub url: String,
    pub properties: ClientProperties,
    pub capabilities: u64,
    /// Use `compress=zlib-stream`.
    pub compress: bool,
    /// Initial presence (Identify).
    pub presence: Value,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    /// Base wait after a non-resumable INVALID_SESSION (randomised 1x..5x).
    pub invalid_session_delay: Duration,
    /// Max wait for HELLO after connecting.
    pub hello_timeout: Duration,
    /// Called with (event name, raw `d` JSON) for every dispatch before it is
    /// parsed, e.g. to persist READY for an instant next start.
    pub raw_dispatch: Option<RawDispatchHook>,
    /// Session saved by a previous run: the first connect sends RESUME
    /// instead of IDENTIFY (falls back to IDENTIFY on INVALID_SESSION).
    pub resume: Option<ResumeState>,
}

/// See [`GatewayConfig::raw_dispatch`]: (event name, raw `d`, sequence number).
pub type RawDispatchHook = std::sync::Arc<dyn Fn(&str, &str, Option<u64>) + Send + Sync>;

/// See [`GatewayConfig::resume`].
#[derive(Clone)]
pub struct ResumeState {
    pub session_id: String,
    pub resume_url: Option<String>,
    pub seq: u64,
}

impl std::fmt::Debug for GatewayConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayConfig")
            .field("token", &"<redacted>")
            .field("url", &self.url)
            .field("capabilities", &self.capabilities)
            .field("compress", &self.compress)
            .finish_non_exhaustive()
    }
}

impl GatewayConfig {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            url: GATEWAY_URL.into(),
            properties: ClientProperties::default(),
            capabilities: capabilities::DEFAULT,
            compress: true,
            presence: json!({ "status": "unknown", "since": 0, "activities": [], "afk": false }),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            invalid_session_delay: Duration::from_secs(1),
            hello_timeout: Duration::from_secs(20),
            raw_dispatch: None,
            resume: None,
        }
    }

    fn connect_url(&self, base: &str) -> String {
        let base = base.trim_end_matches('/');
        let mut u = format!("{base}/?encoding=json&v=10");
        if self.compress {
            u.push_str("&compress=zlib-stream");
        }
        u
    }
}

/// Commands accepted by the gateway task.
#[derive(Debug, Clone)]
pub enum GatewayCommand {
    UpdateVoiceState(UpdateVoiceState),
    UpdatePresence(UpdatePresence),
    RequestGuildMembers(RequestGuildMembers),
    RequestSoundboardSounds(RequestSoundboardSounds),
    LazyRequest(LazyGuildRequest),
    GuildSubscriptionsBulk(std::collections::BTreeMap<GuildId, GuildSubscription>),
    /// Send an arbitrary opcode.
    Raw { op: u8, d: Value },
    /// Close the session for good (the session is invalidated).
    Shutdown,
}

/// Handle to a running gateway task. Cheap to clone. Commands issued while
/// reconnecting are queued and sent once the session is back.
#[derive(Debug, Clone)]
pub struct GatewayHandle {
    tx: mpsc::UnboundedSender<GatewayCommand>,
}

impl GatewayHandle {
    pub fn send(&self, cmd: GatewayCommand) -> crate::Result<()> {
        self.tx.send(cmd).map_err(|_| crate::Error::Closed)
    }

    /// Join (`Some(channel)`) or leave (`None`) a voice channel. For DM calls pass `guild: None`.
    pub fn update_voice_state(
        &self,
        guild: Option<GuildId>,
        channel: Option<ChannelId>,
        self_mute: bool,
        self_deaf: bool,
    ) -> crate::Result<()> {
        self.send(GatewayCommand::UpdateVoiceState(UpdateVoiceState {
            guild_id: guild,
            channel_id: channel,
            self_mute,
            self_deaf,
            self_video: false,
        }))
    }

    pub fn update_presence(&self, status: &str) -> crate::Result<()> {
        self.send(GatewayCommand::UpdatePresence(UpdatePresence {
            since: 0,
            activities: vec![],
            status: status.into(),
            afk: false,
        }))
    }

    /// Op 37: subscribe to one guild (typing/activities/threads + member list ranges).
    pub fn subscribe_guild(&self, guild: GuildId, sub: GuildSubscription) -> crate::Result<()> {
        let mut m = std::collections::BTreeMap::new();
        m.insert(guild, sub);
        self.send(GatewayCommand::GuildSubscriptionsBulk(m))
    }

    /// Op 14 (legacy lazy request).
    pub fn lazy_request(&self, req: LazyGuildRequest) -> crate::Result<()> {
        self.send(GatewayCommand::LazyRequest(req))
    }

    pub fn request_guild_members(&self, req: RequestGuildMembers) -> crate::Result<()> {
        self.send(GatewayCommand::RequestGuildMembers(req))
    }

    /// Op 31: request custom Soundboard sounds for the given guilds.
    pub fn request_soundboard_sounds(&self, guild_ids: Vec<GuildId>) -> crate::Result<()> {
        self.send(GatewayCommand::RequestSoundboardSounds(RequestSoundboardSounds { guild_ids }))
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(GatewayCommand::Shutdown);
    }
}

pub struct Gateway;

impl Gateway {
    /// Spawn the gateway task on the current tokio runtime.
    pub fn spawn(config: GatewayConfig) -> (GatewayHandle, mpsc::UnboundedReceiver<GatewayEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        tokio::spawn(run(config, cmd_rx, ev_tx));
        (GatewayHandle { tx: cmd_tx }, ev_rx)
    }
}

#[derive(Debug, Default, Clone)]
struct Session {
    session_id: Option<String>,
    resume_url: Option<String>,
    seq: Option<u64>,
}

impl Session {
    fn resumable(&self) -> bool {
        self.session_id.is_some()
    }
    fn clear(&mut self) {
        *self = Session::default();
    }
}

enum Outcome {
    Shutdown,
    Fatal { code: u16, reason: String },
    Reconnect { delay: Option<Duration> },
}

fn fatal_close(code: u16) -> bool {
    matches!(code, 4004 | 4010 | 4011 | 4012 | 4013 | 4014)
}

fn jitter() -> f64 {
    rand::random::<f64>()
}

struct Runner {
    config: GatewayConfig,
    cmd_rx: mpsc::UnboundedReceiver<GatewayCommand>,
    ev_tx: mpsc::UnboundedSender<GatewayEvent>,
    pending: VecDeque<GatewayCommand>,
    session: Session,
    /// Set once READY/RESUMED arrives; resets backoff.
    healthy: bool,
}

async fn run(
    config: GatewayConfig,
    cmd_rx: mpsc::UnboundedReceiver<GatewayCommand>,
    ev_tx: mpsc::UnboundedSender<GatewayEvent>,
) {
    let session = config
        .resume
        .as_ref()
        .map(|r| Session { session_id: Some(r.session_id.clone()), resume_url: r.resume_url.clone(), seq: Some(r.seq) })
        .unwrap_or_default();
    let mut r = Runner { config, cmd_rx, ev_tx, pending: VecDeque::new(), session, healthy: false };
    let mut attempt: u32 = 0;
    loop {
        r.healthy = false;
        let base = r.session.resume_url.clone().filter(|_| r.session.resumable()).unwrap_or_else(|| r.config.url.clone());
        let url = r.config.connect_url(&base);
        tracing::info!(%url, resume = r.session.resumable(), "gateway connecting");
        let outcome = match tokio_tungstenite::connect_async(url.as_str()).await {
            Ok((ws, _)) => r.connection(ws).await,
            Err(e) => {
                let _ = r.ev_tx.send(GatewayEvent::Disconnected { code: None, reason: e.to_string() });
                Outcome::Reconnect { delay: None }
            }
        };
        let delay = match outcome {
            Outcome::Shutdown => return,
            Outcome::Fatal { code, reason } => {
                let _ = r.ev_tx.send(GatewayEvent::Fatal { code, reason });
                return;
            }
            Outcome::Reconnect { delay } => {
                if r.healthy {
                    attempt = 0;
                }
                delay.unwrap_or_else(|| {
                    if attempt == 0 {
                        Duration::ZERO
                    } else {
                        let exp = r.config.initial_backoff.saturating_mul(1 << (attempt - 1).min(16));
                        exp.min(r.config.max_backoff).mul_f64(0.75 + jitter() * 0.5)
                    }
                })
            }
        };
        attempt = attempt.saturating_add(1);
        let _ = r.ev_tx.send(GatewayEvent::Reconnecting { attempt, delay, resume: r.session.resumable() });
        // Wait, but stay responsive to shutdown.
        let deadline = Instant::now() + delay;
        loop {
            tokio::select! {
                _ = sleep_until(deadline) => break,
                cmd = r.cmd_rx.recv() => match cmd {
                    None | Some(GatewayCommand::Shutdown) => return,
                    Some(c) => r.pending.push_back(c),
                }
            }
        }
    }
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Deserialize)]
struct Incoming<'a> {
    op: u8,
    #[serde(borrow, default)]
    d: Option<&'a RawValue>,
    #[serde(default)]
    s: Option<u64>,
    #[serde(default)]
    t: Option<String>,
}

enum Frame {
    Text(String),
    Close(Option<u16>, String),
    Ignore,
}

impl Runner {
    fn emit(&self, ev: GatewayEvent) {
        let _ = self.ev_tx.send(ev);
    }

    fn decode(z: &mut ZlibStream, msg: WsMessage) -> Result<Frame, String> {
        Ok(match msg {
            WsMessage::Text(t) => Frame::Text(t.to_string()),
            WsMessage::Binary(b) => match z.push(&b)? {
                Some(s) => Frame::Text(s),
                None => Frame::Ignore,
            },
            WsMessage::Close(f) => match f {
                Some(f) => Frame::Close(Some(f.code.into()), f.reason.to_string()),
                None => Frame::Close(None, String::new()),
            },
            _ => Frame::Ignore,
        })
    }

    async fn send_json(ws: &mut Ws, v: &Value) -> Result<(), String> {
        ws.send(WsMessage::Text(v.to_string().into())).await.map_err(|e| e.to_string())
    }

    fn identify(&self) -> Value {
        json!({
            "op": op::IDENTIFY,
            "d": {
                "token": self.config.token,
                "capabilities": self.config.capabilities,
                "properties": self.config.properties,
                "presence": self.config.presence,
                "compress": false,
                "client_state": { "guild_versions": {} },
            }
        })
    }

    fn command_payload(cmd: &GatewayCommand) -> Option<Value> {
        let (o, d) = match cmd {
            GatewayCommand::UpdateVoiceState(v) => (op::VOICE_STATE_UPDATE, serde_json::to_value(v).ok()?),
            GatewayCommand::UpdatePresence(v) => (op::PRESENCE_UPDATE, serde_json::to_value(v).ok()?),
            GatewayCommand::RequestGuildMembers(v) => (op::REQUEST_GUILD_MEMBERS, serde_json::to_value(v).ok()?),
            GatewayCommand::RequestSoundboardSounds(v) => {
                (op::REQUEST_SOUNDBOARD_SOUNDS, serde_json::to_value(v).ok()?)
            }
            GatewayCommand::LazyRequest(v) => (op::LAZY_REQUEST, serde_json::to_value(v).ok()?),
            GatewayCommand::GuildSubscriptionsBulk(m) => {
                (op::GUILD_SUBSCRIPTIONS_BULK, json!({ "subscriptions": m }))
            }
            GatewayCommand::Raw { op, d } => (*op, d.clone()),
            GatewayCommand::Shutdown => return None,
        };
        Some(json!({ "op": o, "d": d }))
    }

    async fn close(ws: &mut Ws, code: u16, reason: &str) {
        let frame = CloseFrame { code: CloseCode::from(code), reason: reason.to_owned().into() };
        let _ = timeout(Duration::from_secs(2), ws.close(Some(frame))).await;
    }

    /// Run one websocket connection until it ends.
    async fn connection(&mut self, mut ws: Ws) -> Outcome {
        let mut z = ZlibStream::new();

        // ---- HELLO ----
        let hello_deadline = Instant::now() + self.config.hello_timeout;
        let interval = loop {
            let msg = match timeout_at(hello_deadline, ws.next()).await {
                Some(Some(Ok(m))) => m,
                Some(Some(Err(e))) => return self.disconnected(None, e.to_string()),
                Some(None) => return self.disconnected(None, "closed before HELLO".into()),
                None => {
                    Self::close(&mut ws, 4000, "hello timeout").await;
                    return self.disconnected(None, "HELLO timeout".into());
                }
            };
            match Self::decode(&mut z, msg) {
                Ok(Frame::Text(t)) => {
                    let Ok(p) = serde_json::from_str::<Incoming>(&t) else { continue };
                    if p.op == op::HELLO {
                        #[derive(Deserialize)]
                        struct Hello {
                            heartbeat_interval: u64,
                        }
                        match p.d.map(|d| serde_json::from_str::<Hello>(d.get())) {
                            Some(Ok(h)) => break Duration::from_millis(h.heartbeat_interval.max(1)),
                            _ => return self.disconnected(None, "bad HELLO".into()),
                        }
                    }
                }
                Ok(Frame::Close(code, reason)) => return self.on_close(code, reason),
                Ok(Frame::Ignore) => {}
                Err(e) => return self.disconnected(None, format!("zlib: {e}")),
            }
        };
        self.emit(GatewayEvent::Connected { heartbeat_interval: interval });

        // ---- IDENTIFY / RESUME ----
        let first = if let (Some(sid), true) = (&self.session.session_id, self.session.resumable()) {
            json!({ "op": op::RESUME, "d": { "token": self.config.token, "session_id": sid, "seq": self.session.seq } })
        } else {
            self.identify()
        };
        if let Err(e) = Self::send_json(&mut ws, &first).await {
            return self.disconnected(None, e);
        }

        let mut next_beat = Instant::now() + interval.mul_f64(jitter());
        let mut acked = true;
        let mut beat_sent_at: Option<Instant> = None;

        loop {
            // Flush queued commands once the session is established.
            if self.healthy {
                while let Some(cmd) = self.pending.pop_front() {
                    if let Some(p) = Self::command_payload(&cmd)
                        && let Err(e) = Self::send_json(&mut ws, &p).await
                    {
                        self.pending.push_front(cmd);
                        return self.disconnected(None, e);
                    }
                }
            }
            tokio::select! {
                _ = sleep_until(next_beat) => {
                    if !acked {
                        tracing::warn!("heartbeat ACK missed; zombie connection, reconnecting");
                        Self::close(&mut ws, 4000, "heartbeat ack missed").await;
                        return self.disconnected(None, "heartbeat ACK missed".into());
                    }
                    if let Err(e) = Self::send_json(&mut ws, &json!({ "op": op::HEARTBEAT, "d": self.session.seq })).await {
                        return self.disconnected(None, e);
                    }
                    acked = false;
                    beat_sent_at = Some(Instant::now());
                    next_beat = Instant::now() + interval;
                }
                cmd = self.cmd_rx.recv() => match cmd {
                    None | Some(GatewayCommand::Shutdown) => {
                        Self::close(&mut ws, 1000, "shutdown").await;
                        self.session.clear();
                        return Outcome::Shutdown;
                    }
                    Some(c) => self.pending.push_back(c),
                },
                msg = ws.next() => {
                    let msg = match msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => return self.disconnected(None, e.to_string()),
                        None => return self.disconnected(None, "stream ended".into()),
                    };
                    let text = match Self::decode(&mut z, msg) {
                        Ok(Frame::Text(t)) => t,
                        Ok(Frame::Ignore) => continue,
                        Ok(Frame::Close(code, reason)) => return self.on_close(code, reason),
                        Err(e) => {
                            Self::close(&mut ws, 4000, "decode error").await;
                            return self.disconnected(None, format!("zlib: {e}"));
                        }
                    };
                    let p = match serde_json::from_str::<Incoming>(&text) {
                        Ok(p) => p,
                        Err(e) => { tracing::warn!(error = %e, "undecodable gateway payload"); continue; }
                    };
                    match p.op {
                        op::DISPATCH => {
                            if let Some(s) = p.s { self.session.seq = Some(s); }
                            let name = p.t.as_deref().unwrap_or("");
                            let raw = p.d.map(|d| d.get()).unwrap_or("null");
                            if let Some(hook) = &self.config.raw_dispatch {
                                hook(name, raw, p.s);
                            }
                            let ev = parse_dispatch(name, raw);
                            match &ev {
                                GatewayEvent::Ready(r) => {
                                    self.session.session_id = Some(r.session_id.clone());
                                    self.session.resume_url = r.resume_gateway_url.clone();
                                    self.healthy = true;
                                }
                                GatewayEvent::Resumed => self.healthy = true,
                                _ => {}
                            }
                            self.emit(ev);
                        }
                        op::HEARTBEAT => {
                            if let Err(e) = Self::send_json(&mut ws, &json!({ "op": op::HEARTBEAT, "d": self.session.seq })).await {
                                return self.disconnected(None, e);
                            }
                        }
                        op::HEARTBEAT_ACK => {
                            acked = true;
                            if let Some(t) = beat_sent_at.take() {
                                self.emit(GatewayEvent::HeartbeatAck { latency: t.elapsed() });
                            }
                        }
                        op::RECONNECT => {
                            Self::close(&mut ws, 4000, "reconnect requested").await;
                            self.emit(GatewayEvent::Disconnected { code: None, reason: "server requested reconnect".into() });
                            return Outcome::Reconnect { delay: Some(Duration::ZERO) };
                        }
                        op::INVALID_SESSION => {
                            let resumable = p.d.and_then(|d| serde_json::from_str::<bool>(d.get()).ok()).unwrap_or(false);
                            if !resumable { self.session.clear(); }
                            Self::close(&mut ws, 4000, "invalid session").await;
                            self.emit(GatewayEvent::Disconnected { code: None, reason: format!("invalid session (resumable: {resumable})") });
                            let base = self.config.invalid_session_delay;
                            return Outcome::Reconnect { delay: Some(base.mul_f64(1.0 + jitter() * 4.0)) };
                        }
                        other => tracing::debug!(op = other, "unhandled gateway opcode"),
                    }
                }
            }
        }
    }

    fn disconnected(&self, code: Option<u16>, reason: String) -> Outcome {
        tracing::warn!(?code, %reason, "gateway disconnected");
        self.emit(GatewayEvent::Disconnected { code, reason });
        Outcome::Reconnect { delay: None }
    }

    fn on_close(&mut self, code: Option<u16>, reason: String) -> Outcome {
        if let Some(c) = code {
            if fatal_close(c) {
                return Outcome::Fatal { code: c, reason };
            }
            // Invalid seq / session timed out: must re-identify.
            if matches!(c, 4007 | 4009) {
                self.session.clear();
            }
        }
        self.disconnected(code, reason)
    }
}

async fn timeout_at<F: std::future::Future>(deadline: Instant, f: F) -> Option<F::Output> {
    tokio::time::timeout_at(deadline, f).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_capabilities_request_ready_supplemental() {
        assert_ne!(capabilities::DEFAULT & capabilities::PRIORITIZED_READY_PAYLOAD, 0);
    }

    #[test]
    fn url_building() {
        let c = GatewayConfig::new("t");
        assert_eq!(c.connect_url("wss://gateway.discord.gg/"), "wss://gateway.discord.gg/?encoding=json&v=10&compress=zlib-stream");
    }

    #[test]
    fn identify_shape() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let (etx, _erx) = mpsc::unbounded_channel();
        let r = Runner { config: GatewayConfig::new("tok"), cmd_rx: rx, ev_tx: etx, pending: VecDeque::new(), session: Session::default(), healthy: false };
        let v = r.identify();
        assert_eq!(v["op"], 2);
        assert_eq!(v["d"]["token"], "tok");
        assert_eq!(v["d"]["capabilities"], capabilities::DEFAULT);
        assert_eq!(v["d"]["properties"]["browser"], "Discord Client");
        assert!(v["d"].get("intents").is_none());
    }

    #[test]
    fn op37_payload() {
        let mut m = std::collections::BTreeMap::new();
        m.insert(crate::models::Snowflake(1), GuildSubscription::focused(Some(crate::models::Snowflake(2))));
        let p = Runner::command_payload(&GatewayCommand::GuildSubscriptionsBulk(m)).unwrap();
        assert_eq!(p["op"], 37);
        assert_eq!(p["d"]["subscriptions"]["1"]["channels"]["2"], json!([[0, 99]]));
        assert_eq!(p["d"]["subscriptions"]["1"]["typing"], true);
    }
}
