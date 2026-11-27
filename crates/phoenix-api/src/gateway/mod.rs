pub mod events;
pub mod zlib;

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
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
    pub const QOS_HEARTBEAT: u8 = 40;
}

pub const QOS_VERSION: u32 = 31;
pub const CONNECT_REASONS: &str = "AppSkeleton";

pub fn qos_token(active: bool, derived: Option<&str>) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut out = vec![0x0a];
    if active {
        out.extend_from_slice(&[0x02, 0x08, 0x01]);
    } else {
        out.push(0x00);
    }
    if let Some(d) = derived.and_then(|d| b64.decode(d.trim()).ok()) {
        out.push(0x12);
        let mut n = d.len();
        while n >= 0x80 {
            out.push((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
        out.push(n as u8);
        out.extend_from_slice(&d);
    }
    b64.encode(out)
}
pub const RESUME_WINDOW: Duration = Duration::from_secs(180);
pub const EXPEDITED_HEARTBEAT: Duration = Duration::from_secs(5);
pub const HEARTBEAT_GRACE: Duration = Duration::from_secs(5);
pub const RESET_AFTER_FAILS: u32 = 4;
const WAKE_CHECK: Duration = Duration::from_secs(5);
const WAKE_DRIFT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QosPayload {
    pub active: bool,
    pub ver: u32,
    pub reasons: Vec<String>,
}

impl QosPayload {
    pub fn new(foregrounded: bool, rtc_connected: bool) -> Self {
        let mut reasons = Vec::new();
        if foregrounded {
            reasons.push("foregrounded".to_owned());
        }
        if rtc_connected {
            reasons.push("rtc_connected".to_owned());
        }
        Self { active: !reasons.is_empty(), ver: QOS_VERSION, reasons }
    }
}

#[derive(Debug, Default, Clone)]
struct QosState {
    current: Option<QosPayload>,
    upcoming: Option<QosPayload>,
}

impl QosState {
    fn update(&mut self, e: QosPayload) -> bool {
        let was_inactive = !self.current.as_ref().is_some_and(|c| c.active);
        let cur = self.current.get_or_insert_with(|| e.clone());
        let mut send = false;
        if e.active {
            cur.active = true;
            cur.reasons.extend(e.reasons.iter().cloned());
            cur.reasons.sort();
            cur.reasons.dedup();
            send = was_inactive;
        }
        self.upcoming = Some(e);
        send
    }

    fn is_active(&self) -> bool {
        self.upcoming.as_ref().or(self.current.as_ref()).is_some_and(|q| q.active)
    }

    fn consume(&mut self) -> Option<QosPayload> {
        let out = self.current.clone();
        if let Some(u) = self.upcoming.take() {
            self.current = Some(u);
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Backoff {
    min: Duration,
    max: Duration,
    current: Duration,
    fails: u32,
}

impl Backoff {
    pub fn new(min: Duration, max: Duration) -> Self {
        Self { min, max, current: min, fails: 0 }
    }

    pub fn fails(&self) -> u32 {
        self.fails
    }

    pub fn fail_with(&mut self, r: f64) -> Duration {
        self.fails += 1;
        let grow = self.current.mul_f64(2.0 * r.clamp(0.0, 1.0));
        self.current = (self.current + grow).min(self.max);
        self.current
    }

    pub fn fail(&mut self) -> Duration {
        self.fail_with(jitter())
    }

    pub fn succeed(&mut self) {
        self.fails = 0;
        self.current = self.min;
    }
}

#[derive(Clone)]
pub struct GatewayConfig {
    pub token: String,
    pub url: String,
    pub properties: ClientProperties,
    pub capabilities: u64,
    pub compress: bool,
    pub presence: Value,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub hello_timeout: Duration,
    pub qos: QosPayload,
    pub raw_dispatch: Option<RawDispatchHook>,
    pub resume: Option<ResumeState>,
}

pub type RawDispatchHook = std::sync::Arc<dyn Fn(&str, &str, Option<u64>) + Send + Sync>;

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
            hello_timeout: Duration::from_secs(30),
            qos: QosPayload::new(true, false),
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

#[derive(Debug, Clone)]
pub enum GatewayCommand {
    UpdateVoiceState(UpdateVoiceState),
    UpdatePresence(UpdatePresence),
    RequestGuildMembers(RequestGuildMembers),
    RequestSoundboardSounds(RequestSoundboardSounds),
    LazyRequest(LazyGuildRequest),
    GuildSubscriptionsBulk(std::collections::BTreeMap<GuildId, GuildSubscription>),
    Raw { op: u8, d: Value },
    Activity { foregrounded: bool, rtc_connected: bool },
    ExpeditedHeartbeat { timeout: Duration },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct GatewayHandle {
    tx: mpsc::UnboundedSender<GatewayCommand>,
}

impl GatewayHandle {
    pub fn send(&self, cmd: GatewayCommand) -> crate::Result<()> {
        self.tx.send(cmd).map_err(|_| crate::Error::Closed)
    }

    pub fn update_voice_state(
        &self,
        guild: Option<GuildId>,
        channel: Option<ChannelId>,
        self_mute: bool,
        self_deaf: bool,
    ) -> crate::Result<()> {
        self.update_voice_state_video(guild, channel, self_mute, self_deaf, false)
    }

    pub fn update_voice_state_video(
        &self,
        guild: Option<GuildId>,
        channel: Option<ChannelId>,
        self_mute: bool,
        self_deaf: bool,
        self_video: bool,
    ) -> crate::Result<()> {
        self.send(GatewayCommand::UpdateVoiceState(UpdateVoiceState {
            guild_id: guild,
            channel_id: channel,
            self_mute,
            self_deaf,
            self_video,
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

    pub fn subscribe_guild(&self, guild: GuildId, sub: GuildSubscription) -> crate::Result<()> {
        let mut m = std::collections::BTreeMap::new();
        m.insert(guild, sub);
        self.send(GatewayCommand::GuildSubscriptionsBulk(m))
    }

    pub fn lazy_request(&self, req: LazyGuildRequest) -> crate::Result<()> {
        self.send(GatewayCommand::LazyRequest(req))
    }

    pub fn request_guild_members(&self, req: RequestGuildMembers) -> crate::Result<()> {
        self.send(GatewayCommand::RequestGuildMembers(req))
    }

    pub fn request_soundboard_sounds(&self, guild_ids: Vec<GuildId>) -> crate::Result<()> {
        self.send(GatewayCommand::RequestSoundboardSounds(RequestSoundboardSounds { guild_ids }))
    }

    pub fn stream_create(&self, guild: GuildId, channel: ChannelId, region: Option<&str>) -> crate::Result<()> {
        self.send(GatewayCommand::Raw {
            op: 18,
            d: json!({ "type": "guild", "guild_id": guild.0.to_string(), "channel_id": channel.0.to_string(), "preferred_region": region }),
        })
    }

    pub fn stream_watch(&self, stream_key: &str) -> crate::Result<()> {
        self.send(GatewayCommand::Raw { op: 20, d: json!({ "stream_key": stream_key }) })
    }

    pub fn voice_server_ping(&self) -> crate::Result<()> {
        self.send(GatewayCommand::Raw { op: 5, d: serde_json::Value::Null })
    }

    pub fn stream_ping(&self, stream_key: &str) -> crate::Result<()> {
        self.send(GatewayCommand::Raw { op: 21, d: json!({ "stream_key": stream_key }) })
    }

    pub fn stream_delete(&self, stream_key: &str) -> crate::Result<()> {
        self.send(GatewayCommand::Raw { op: 19, d: json!({ "stream_key": stream_key }) })
    }

    pub fn stream_set_paused(&self, stream_key: &str, paused: bool) -> crate::Result<()> {
        self.send(GatewayCommand::Raw { op: 22, d: json!({ "stream_key": stream_key, "paused": paused }) })
    }

    pub fn set_activity(&self, foregrounded: bool, rtc_connected: bool) -> crate::Result<()> {
        self.send(GatewayCommand::Activity { foregrounded, rtc_connected })
    }

    pub fn expedited_heartbeat(&self, timeout: Duration) -> crate::Result<()> {
        self.send(GatewayCommand::ExpeditedHeartbeat { timeout })
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(GatewayCommand::Shutdown);
    }
}

pub struct Gateway;

impl Gateway {
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
    Reconnect { immediate: bool, closed: bool },
}

fn fatal_close(code: u16) -> bool {
    code == 4004
}

fn jitter() -> f64 {
    rand::random::<f64>()
}

pub fn should_resume(has_session: bool, last_ack: Option<SystemTime>, now: SystemTime) -> bool {
    has_session && last_ack.is_none_or(|t| now.duration_since(t).unwrap_or_default() <= RESUME_WINDOW)
}

fn wake_ticker() -> tokio::time::Interval {
    let mut t = tokio::time::interval_at(Instant::now() + WAKE_CHECK, WAKE_CHECK);
    t.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    t
}

struct WakeWatch {
    wall: SystemTime,
    mono: Instant,
}

impl WakeWatch {
    fn new() -> Self {
        Self { wall: SystemTime::now(), mono: Instant::now() }
    }

    fn woke(&mut self) -> bool {
        let wall = SystemTime::now().duration_since(self.wall).unwrap_or_default();
        let mono = self.mono.elapsed();
        *self = Self::new();
        wall > mono + WAKE_DRIFT
    }
}

struct Runner {
    config: GatewayConfig,
    cmd_rx: mpsc::UnboundedReceiver<GatewayCommand>,
    ev_tx: mpsc::UnboundedSender<GatewayEvent>,
    pending: VecDeque<GatewayCommand>,
    session: Session,
    healthy: bool,
    backoff: Backoff,
    last_ack: Option<SystemTime>,
    qos: QosState,
    derived_qos: Option<String>,
}

async fn run(
    config: GatewayConfig,
    cmd_rx: mpsc::UnboundedReceiver<GatewayCommand>,
    ev_tx: mpsc::UnboundedSender<GatewayEvent>,
) {
    let mut r = Runner::new(config, cmd_rx, ev_tx);
    loop {
        r.healthy = false;
        let base = r.session.resume_url.clone().filter(|_| r.session.resumable()).unwrap_or_else(|| r.config.url.clone());
        let url = r.config.connect_url(&base);
        tracing::info!(%url, resume = r.session.resumable(), "gateway connecting");
        let delay = match r.connection(&url).await {
            Outcome::Shutdown => return,
            Outcome::Fatal { code, reason } => {
                let _ = r.ev_tx.send(GatewayEvent::Fatal { code, reason });
                return;
            }
            Outcome::Reconnect { immediate: true, .. } => Duration::ZERO,
            Outcome::Reconnect { immediate: false, closed } => {
                let d = r.backoff.fail();
                if closed && r.backoff.fails() > RESET_AFTER_FAILS {
                    tracing::warn!(fails = r.backoff.fails(), "gateway session reset after repeated failures");
                    r.session.clear();
                    let _ = r.ev_tx.send(GatewayEvent::SessionReset);
                }
                d
            }
        };
        let _ = r.ev_tx.send(GatewayEvent::Reconnecting { attempt: r.backoff.fails(), delay, resume: r.session.resumable() });
        if !r.wait(delay).await {
            return;
        }
    }
}

impl Runner {
    fn new(
        config: GatewayConfig,
        cmd_rx: mpsc::UnboundedReceiver<GatewayCommand>,
        ev_tx: mpsc::UnboundedSender<GatewayEvent>,
    ) -> Self {
        let session = config
            .resume
            .as_ref()
            .map(|r| Session { session_id: Some(r.session_id.clone()), resume_url: r.resume_url.clone(), seq: Some(r.seq) })
            .unwrap_or_default();
        let backoff = Backoff::new(config.initial_backoff, config.max_backoff);
        let mut qos = QosState::default();
        qos.update(config.qos.clone());
        Runner {
            config,
            cmd_rx,
            ev_tx,
            pending: VecDeque::new(),
            session,
            healthy: false,
            backoff,
            last_ack: None,
            qos,
            derived_qos: None,
        }
    }

    async fn wait(&mut self, delay: Duration) -> bool {
        let deadline = Instant::now() + delay;
        let mut wake = WakeWatch::new();
        let mut tick = wake_ticker();
        loop {
            tokio::select! {
                _ = sleep_until(deadline) => return true,
                _ = tick.tick() => {
                    if wake.woke() {
                        tracing::info!("system resumed from sleep, reconnecting now");
                        self.backoff.succeed();
                        return true;
                    }
                }
                cmd = self.cmd_rx.recv() => match cmd {
                    None | Some(GatewayCommand::Shutdown) => return false,
                    Some(GatewayCommand::Activity { foregrounded, rtc_connected }) => {
                        self.qos.update(QosPayload::new(foregrounded, rtc_connected));
                    }
                    Some(GatewayCommand::ExpeditedHeartbeat { .. }) => {
                        self.backoff.succeed();
                        return true;
                    }
                    Some(c) => self.queue(c),
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
        let mut properties = self.config.properties.to_value(crate::props::client_app_state());
        if let Some(m) = properties.as_object_mut() {
            m.insert("is_fast_connect".into(), false.into());
            m.insert("gateway_connect_reasons".into(), CONNECT_REASONS.into());
        }
        json!({
            "op": op::IDENTIFY,
            "d": {
                "token": self.config.token,
                "capabilities": self.config.capabilities,
                "properties": properties,
                "presence": self.config.presence,
                "compress": false,
                "client_state": { "guild_versions": {} },
                "qos_token": qos_token(self.qos.is_active(), self.derived_qos.as_deref()),
            }
        })
    }

    fn queue(&mut self, c: GatewayCommand) {
        if let GatewayCommand::UpdatePresence(p) = &c
            && let Ok(v) = serde_json::to_value(p)
        {
            self.config.presence = v;
        }
        self.pending.push_back(c);
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
            GatewayCommand::Activity { .. } | GatewayCommand::ExpeditedHeartbeat { .. } | GatewayCommand::Shutdown => {
                return None;
            }
        };
        Some(json!({ "op": o, "d": d }))
    }

    async fn close(ws: &mut Ws, code: u16, reason: &str) {
        let frame = CloseFrame { code: CloseCode::from(code), reason: reason.to_owned().into() };
        let _ = timeout(Duration::from_secs(2), ws.close(Some(frame))).await;
    }

    fn resume_or_identify(&mut self) -> Value {
        let now = SystemTime::now();
        let resume = should_resume(self.session.resumable(), self.last_ack, now);
        self.last_ack = Some(now);
        if resume && let Some(sid) = &self.session.session_id {
            return json!({ "op": op::RESUME, "d": { "token": self.config.token, "session_id": sid, "seq": self.session.seq } });
        }
        self.session.session_id = None;
        self.session.seq = None;
        self.identify()
    }

    fn heartbeat_payload(&mut self) -> Value {
        json!({ "op": op::QOS_HEARTBEAT, "d": { "seq": self.session.seq.unwrap_or(0), "qos": self.qos.consume() } })
    }

    async fn heartbeat_timeout(&self, ws: &mut Ws) -> Outcome {
        tracing::warn!("heartbeat ACK missed; zombie connection, reconnecting");
        Self::close(ws, 4000, "heartbeat ack missed").await;
        self.emit(GatewayEvent::Disconnected { code: None, reason: "heartbeat ACK missed".into() });
        Outcome::Reconnect { immediate: false, closed: false }
    }

    async fn connection(&mut self, url: &str) -> Outcome {
        let deadline = Instant::now() + self.config.hello_timeout;
        let mut ws = match timeout_at(deadline, tokio_tungstenite::connect_async(url)).await {
            Some(Ok((ws, _))) => ws,
            Some(Err(e)) => return self.errored(e.to_string()),
            None => return self.errored("connect timed out before HELLO".into()),
        };
        let mut z = ZlibStream::new();

        let interval = loop {
            let msg = match timeout_at(deadline, ws.next()).await {
                Some(Some(Ok(m))) => m,
                Some(Some(Err(e))) => return self.errored(e.to_string()),
                Some(None) => return self.disconnected(None, "closed before HELLO".into()),
                None => {
                    Self::close(&mut ws, 4000, "hello timeout").await;
                    return self.errored("HELLO timeout".into());
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
                Err(e) => return self.errored(format!("zlib: {e}")),
            }
        };
        self.emit(GatewayEvent::Connected { heartbeat_interval: interval });

        let first = self.resume_or_identify();
        if let Err(e) = Self::send_json(&mut ws, &first).await {
            return self.errored(e);
        }

        let far = Instant::now() + Duration::from_secs(86400 * 365);
        let mut next_beat = Instant::now() + interval.mul_f64(jitter());
        let mut interval_started = false;
        let mut acked = true;
        let mut expedited: Option<Instant> = None;
        let mut beat_sent_at: Option<Instant> = None;
        let mut wake = WakeWatch::new();
        let mut tick = wake_ticker();

        loop {
            if self.healthy {
                while let Some(cmd) = self.pending.pop_front() {
                    if let Some(p) = Self::command_payload(&cmd)
                        && let Err(e) = Self::send_json(&mut ws, &p).await
                    {
                        self.pending.push_front(cmd);
                        return self.errored(e);
                    }
                }
            }
            let mut beat_now = false;
            tokio::select! {
                _ = sleep_until(next_beat) => {
                    next_beat = Instant::now() + interval;
                    if !interval_started {
                        interval_started = true;
                        acked = true;
                    }
                    if acked {
                        acked = false;
                        beat_now = true;
                    } else if expedited.is_none() {
                        return self.heartbeat_timeout(&mut ws).await;
                    }
                }
                _ = sleep_until(expedited.unwrap_or(far)), if expedited.is_some() => {
                    expedited = None;
                    if !acked {
                        return self.heartbeat_timeout(&mut ws).await;
                    }
                }
                _ = tick.tick() => {
                    if wake.woke() {
                        tracing::info!("system resumed from sleep, sending an expedited heartbeat");
                        acked = false;
                        beat_now = true;
                        expedited = Some(Instant::now() + EXPEDITED_HEARTBEAT);
                    }
                }
                cmd = self.cmd_rx.recv() => match cmd {
                    None | Some(GatewayCommand::Shutdown) => {
                        Self::close(&mut ws, 1000, "shutdown").await;
                        self.session.clear();
                        return Outcome::Shutdown;
                    }
                    Some(GatewayCommand::Activity { foregrounded, rtc_connected }) => {
                        beat_now = self.qos.update(QosPayload::new(foregrounded, rtc_connected)) && self.healthy;
                    }
                    Some(GatewayCommand::ExpeditedHeartbeat { timeout }) => {
                        acked = false;
                        beat_now = true;
                        expedited = Some(Instant::now() + timeout);
                    }
                    Some(c) => self.queue(c),
                },
                msg = ws.next() => {
                    let msg = match msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => return self.errored(e.to_string()),
                        None => return self.errored("stream ended".into()),
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
                    if let Some(s) = p.s { self.session.seq = Some(s); }
                    match p.op {
                        op::DISPATCH => {
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
                                    self.derived_qos = r.qos_token.clone();
                                    self.healthy = true;
                                    self.backoff.succeed();
                                }
                                GatewayEvent::ReadySupplemental(_) => self.backoff.succeed(),
                                GatewayEvent::Resumed => {
                                    self.healthy = true;
                                    self.backoff.succeed();
                                }
                                _ => {}
                            }
                            self.emit(ev);
                        }
                        op::HEARTBEAT => {
                            beat_now = true;
                            if interval_started {
                                next_beat = Instant::now() + interval;
                            }
                        }
                        op::HEARTBEAT_ACK => {
                            acked = true;
                            expedited = None;
                            self.last_ack = Some(SystemTime::now());
                            if let Some(t) = beat_sent_at {
                                self.emit(GatewayEvent::HeartbeatAck { latency: t.elapsed() });
                            }
                        }
                        op::RECONNECT => {
                            Self::close(&mut ws, 4000, "reconnect requested").await;
                            self.emit(GatewayEvent::Disconnected { code: None, reason: "server requested reconnect".into() });
                            return Outcome::Reconnect { immediate: true, closed: false };
                        }
                        op::INVALID_SESSION => {
                            let resumable = p.d.and_then(|d| serde_json::from_str::<bool>(d.get()).ok()).unwrap_or(false);
                            tracing::info!(resumable, "gateway invalid session");
                            self.healthy = false;
                            let next = if resumable {
                                self.resume_or_identify()
                            } else {
                                self.session.session_id = None;
                                self.session.seq = None;
                                self.identify()
                            };
                            if let Err(e) = Self::send_json(&mut ws, &next).await {
                                return self.errored(e);
                            }
                        }
                        other => tracing::debug!(op = other, "unhandled gateway opcode"),
                    }
                    if beat_sent_at.is_some_and(|t| t.elapsed() > interval + HEARTBEAT_GRACE) {
                        beat_now = true;
                    }
                }
            }
            if beat_now {
                let hb = self.heartbeat_payload();
                if let Err(e) = Self::send_json(&mut ws, &hb).await {
                    return self.errored(e);
                }
                beat_sent_at = Some(Instant::now());
            }
        }
    }

    fn disconnected(&self, code: Option<u16>, reason: String) -> Outcome {
        tracing::warn!(?code, %reason, "gateway disconnected");
        self.emit(GatewayEvent::Disconnected { code, reason });
        Outcome::Reconnect { immediate: false, closed: true }
    }

    fn errored(&mut self, reason: String) -> Outcome {
        self.session.resume_url = None;
        self.disconnected(None, reason)
    }

    fn on_close(&mut self, code: Option<u16>, reason: String) -> Outcome {
        if let Some(c) = code {
            if fatal_close(c) {
                return Outcome::Fatal { code: c, reason };
            }
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
        let r = Runner::new(GatewayConfig::new("tok"), rx, etx);
        let v = r.identify();
        assert_eq!(v["op"], 2);
        assert_eq!(v["d"]["token"], "tok");
        assert_eq!(v["d"]["capabilities"], capabilities::DEFAULT);
        assert_eq!(v["d"]["properties"]["browser"], "Discord Client");
        assert_eq!(v["d"]["properties"]["is_fast_connect"], false);
        assert_eq!(v["d"]["properties"]["gateway_connect_reasons"], "AppSkeleton");
        assert!(v["d"]["properties"]["client_app_state"].is_string());
        assert_eq!(v["d"]["qos_token"], "CgIIAQ==");
        assert!(v["d"].get("intents").is_none());
    }

    #[test]
    fn qos_token_wraps_the_derived_data_from_ready() {
        assert_eq!(qos_token(true, None), "CgIIAQ==");
        assert_eq!(qos_token(false, None), "CgA=");
        assert_eq!(qos_token(true, Some("CgEB")), "CgIIARIDCgEB");
        assert_eq!(qos_token(false, Some("not base64!")), "CgA=");
    }

    #[test]
    fn identify_carries_the_last_presence_sent() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let (etx, _erx) = mpsc::unbounded_channel();
        let mut r = Runner::new(GatewayConfig::new("tok"), rx, etx);
        assert_eq!(r.identify()["d"]["presence"]["status"], "unknown");
        r.queue(GatewayCommand::UpdatePresence(UpdatePresence { since: 0, activities: vec![], status: "dnd".into(), afk: false }));
        assert_eq!(r.identify()["d"]["presence"], json!({ "since": 0, "activities": [], "status": "dnd", "afk": false }));
        assert_eq!(r.pending.len(), 1);
    }

    #[test]
    fn backoff_grows_like_the_client_and_resets() {
        let mut b = Backoff::new(Duration::from_secs(1), Duration::from_secs(60));
        assert_eq!(b.fail_with(0.0), Duration::from_secs(1));
        assert_eq!(b.fail_with(1.0), Duration::from_secs(3));
        assert_eq!(b.fail_with(1.0), Duration::from_secs(9));
        assert_eq!(b.fail_with(1.0), Duration::from_secs(27));
        assert_eq!(b.fail_with(1.0), Duration::from_secs(60));
        assert_eq!(b.fails(), 5);
        b.succeed();
        assert_eq!(b.fails(), 0);
        assert_eq!(b.fail_with(0.5), Duration::from_secs(2));
    }

    #[test]
    fn qos_reports_activity_seen_since_the_last_beat() {
        let mut q = QosState::default();
        assert!(!q.update(QosPayload::new(false, false)));
        assert!(q.update(QosPayload::new(true, false)));
        assert!(!q.update(QosPayload::new(false, true)));
        let sent = q.consume().unwrap();
        assert_eq!(sent.reasons, vec!["foregrounded", "rtc_connected"]);
        assert!(sent.active);
        assert_eq!(q.consume().unwrap(), QosPayload::new(false, true));
        assert!(!q.update(QosPayload::new(false, false)));
        assert_eq!(q.consume().unwrap(), QosPayload::new(false, true));
        assert!(!q.consume().unwrap().active);
        let p = serde_json::to_value(QosPayload::new(true, false)).unwrap();
        assert_eq!(p, json!({ "active": true, "ver": 31, "reasons": ["foregrounded"] }));
    }

    #[test]
    fn resume_only_within_three_minutes_of_the_last_ack() {
        let now = SystemTime::now();
        assert!(should_resume(true, None, now));
        assert!(should_resume(true, Some(now - Duration::from_secs(179)), now));
        assert!(!should_resume(true, Some(now - Duration::from_secs(181)), now));
        assert!(!should_resume(false, None, now));
    }

    #[test]
    fn only_auth_failure_is_fatal() {
        assert!(fatal_close(4004));
        for c in [4000, 4001, 4002, 4003, 4005, 4007, 4008, 4009, 4010, 4011, 4012, 4013, 4014] {
            assert!(!fatal_close(c), "{c}");
        }
    }

    #[test]
    fn wake_watch_sees_a_wall_clock_jump() {
        let mut w = WakeWatch { wall: SystemTime::now() - Duration::from_secs(600), mono: Instant::now() };
        assert!(w.woke());
        assert!(!w.woke());
    }

    #[test]
    fn heartbeat_is_the_qos_opcode() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let (etx, _erx) = mpsc::unbounded_channel();
        let mut r = Runner::new(GatewayConfig::new("tok"), rx, etx);
        assert_eq!(r.heartbeat_payload(), json!({ "op": 40, "d": { "seq": 0, "qos": { "active": true, "ver": 31, "reasons": ["foregrounded"] } } }));
        r.session.seq = Some(7);
        assert_eq!(r.heartbeat_payload()["d"]["seq"], 7);
    }

    #[test]
    fn stale_session_identifies_instead_of_resuming() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let (etx, _erx) = mpsc::unbounded_channel();
        let mut cfg = GatewayConfig::new("tok");
        cfg.resume = Some(ResumeState { session_id: "s".into(), resume_url: None, seq: 4 });
        let mut r = Runner::new(cfg, rx, etx);
        assert_eq!(r.resume_or_identify()["op"], 6);
        r.last_ack = Some(SystemTime::now() - Duration::from_secs(3600));
        assert_eq!(r.resume_or_identify()["op"], 2);
        assert!(r.session.session_id.is_none());
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
