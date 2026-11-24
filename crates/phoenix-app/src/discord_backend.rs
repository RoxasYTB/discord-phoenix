//! Real [`Backend`] on top of `phoenix-api`.
//!
//! Networking runs on a private multi-threaded tokio runtime (its own worker
//! threads), never on GPUI's executor: every trait method spawns onto that
//! runtime and hands the UI a `JoinHandle` to await. Gateway dispatches are
//! applied to a [`Cache`] and translated into [`AppEvent`]s by
//! [`convert::Adapter`].

pub mod convert;
mod account;
mod expressions;
pub mod http;
mod nav;
mod people;
mod profile_edit;
mod profile_widgets;
pub(crate) mod profile_games;
mod collectibles_shop;
mod thread_members;
mod pins;
mod member_list_id;
mod history;
mod inbox;
mod msg_cache;
mod snapshot;
#[cfg(test)]
pub(crate) use profile_edit::bodies as profile_edit_bodies;
pub mod store;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use phoenix_api::{
    Cache, ClientProperties, Error as ApiError, Gateway, GatewayConfig, GatewayEvent, GatewayHandle, RestClient,
    gateway::GuildSubscription,
    models as api,
    rest::{CreateMessage, LoginResponse},
};
use phoenix_api::gateway::events as api_ready;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    backend::{AppEvent, Backend, BackendError, BoxFuture, Credentials, EventSink, ReadyData},
    model::{Id, Message},
};
use convert::{Adapter, Prefs, sf};
use store::{KeyringStore, Secret, TokenStore};

const READY_TIMEOUT: Duration = Duration::from_secs(45);
/// Delay before acking a channel that keeps receiving messages while open.
const TYPING_INTERVAL: Duration = Duration::from_secs(8);
const ACK_DEBOUNCE: Duration = Duration::from_millis(1500);

fn err(s: impl Into<String>) -> BackendError {
    BackendError(s.into())
}

/// Waits between network retries of a message send (first one measured on the client).
const SEND_RETRY_DELAYS: [Duration; 2] = [Duration::from_millis(1500), Duration::from_millis(3000)];

/// Human-readable login/API error. Never includes the token (the API error
/// types don't carry it).
fn api_err(e: ApiError) -> BackendError {
    match &e {
        ApiError::CaptchaRequired { .. } => {
            err("Discord wants a captcha for this login, which Phoenix can't show. Scan the QR code with the mobile app or log in with a token instead.")
        }
        ApiError::Api { body, .. } if e.is_unauthorized() || body.code == 50035 => {
            err(if body.message.is_empty() { "Login or password is invalid.".to_owned() } else { body.message.clone() })
        }
        _ if e.is_unauthorized() => err("Invalid token."),
        ApiError::RateLimited { retry_after, .. } => {
            err(format!("Rate limited by Discord, retry in {}s.", retry_after.as_secs().max(1)))
        }
        _ => err(e.to_string()),
    }
}

struct Session {
    rest: RestClient,
    gateway: GatewayHandle,
    cache: Cache,
    adapter: Arc<Mutex<Adapter>>,
    selected_channel: Mutex<Option<api::Snowflake>>,
    /// Channels with a debounced ack in flight.
    pending_acks: Mutex<HashSet<u64>>,
    /// Per guild, the channel whose member list op 37 last asked for.
    subscribed: Mutex<HashMap<u64, u64>>,
    /// Last typing notification (channel, when): Discord shows it for ~10s.
    last_typing: Mutex<Option<(u64, Instant)>>,
    /// Notification levels / mutes for desktop notifications.
    notify: Mutex<crate::notifications::NotifyPrefs>,
    /// READY `notification_settings` USE_NEW_NOTIFICATIONS (inbox unreads setting).
    new_notifications: bool,
}

struct Inner {
    rt: tokio::runtime::Runtime,
    sink: Mutex<Option<EventSink>>,
    session: Mutex<Option<Arc<Session>>>,
    mfa_ticket: Mutex<Option<Secret>>,
    mfa_opts: Mutex<crate::auth::MfaOptions>,
    remote_auth: Mutex<Option<tokio::task::AbortHandle>>,
    /// REST client from the stored token, usable before READY (history while
    /// the disk snapshot is shown).
    pre_rest: Mutex<Option<RestClient>>,
    store: Arc<dyn TokenStore>,
    props: ClientProperties,
}

impl Inner {
    fn emit(&self, ev: AppEvent) {
        if let Some(s) = self.sink.lock().as_ref() {
            s(ev);
        }
    }
    fn session(&self) -> Result<Arc<Session>, BackendError> {
        self.session.lock().clone().ok_or_else(|| err("Not logged in."))
    }
}

#[derive(Clone)]
pub struct DiscordBackend {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for DiscordBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordBackend").field("logged_in", &self.inner.session.lock().is_some()).finish_non_exhaustive()
    }
}

impl DiscordBackend {
    /// Backend storing the token in the OS keyring.
    pub fn new() -> Self {
        Self::with_store(Arc::new(KeyringStore))
    }

    pub fn with_store(store: Arc<dyn TokenStore>) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("phoenix-net")
            .enable_all()
            .build()
            .expect("tokio runtime");
        // Slow scan of the local Discord install for the next start's super
        // properties (build number, Electron); low priority, off the UI thread.
        let _ = std::thread::Builder::new().name("phoenix-buildinfo".into()).spawn(|| {
            phoenix_api::BuildInfo::refresh_cache();
        });
        // Parse last session's READY while the window is being built.
        snapshot::preload();
        Self {
            inner: Arc::new(Inner {
                rt,
                sink: Mutex::new(None),
                session: Mutex::new(None),
                mfa_ticket: Mutex::new(None),
                mfa_opts: Mutex::new(Default::default()),
                remote_auth: Mutex::new(None),
                pre_rest: Mutex::new(None),
                store,
                props: ClientProperties::current(),
            }),
        }
    }

    /// HTTP client for GPUI image loading (`cx.set_http_client`).
    pub fn http_client(&self) -> Arc<dyn gpui::http_client::HttpClient> {
        Arc::new(http::CdnHttpClient::new(self.inner.rt.handle().clone(), self.inner.props.user_agent()))
    }

    fn spawn<T, F>(&self, f: impl FnOnce(Arc<Inner>) -> F) -> BoxFuture<T>
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, BackendError>> + Send + 'static,
    {
        let handle = self.inner.rt.spawn(f(self.inner.clone()));
        Box::pin(async move { handle.await.map_err(|e| err(format!("internal error: {e}")))? })
    }

    fn with_session<T, F>(&self, f: impl FnOnce(Arc<Inner>, Arc<Session>) -> F + Send + 'static) -> BoxFuture<T>
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, BackendError>> + Send + 'static,
    {
        self.spawn(move |inner| async move {
            let s = inner.session()?;
            f(inner, s).await
        })
    }
}

impl Default for DiscordBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Tolerate tokens pasted with quotes or whitespace.
fn clean_token(t: &str) -> String {
    t.trim().trim_matches('"').trim().to_owned()
}

async fn store_op<T: Send + 'static>(
    store: &Arc<dyn TokenStore>,
    f: impl FnOnce(&dyn TokenStore) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let s = store.clone();
    tokio::task::spawn_blocking(move || f(&*s)).await.map_err(|e| e.to_string())?
}

/// Connect the gateway with `token`, wait for READY, start the event pump.
async fn connect(inner: Arc<Inner>, token: Secret, remember: bool) -> Result<ReadyData, BackendError> {
    if let Some(old) = inner.session.lock().take() {
        old.gateway.shutdown();
    }
    let t0 = Instant::now();
    crate::perf::mark("connect_start");
    let rest = RestClient::new(token.expose(), inner.props.clone());
    let mut cfg = GatewayConfig::new(token.expose());
    cfg.properties = inner.props.clone();
    cfg.raw_dispatch = Some(snapshot::hook());
    // Stored-session start: RESUME the previous run's gateway session when it
    // is recent and nothing else owns it (see snapshot.rs).
    let core = if remember { None } else { tokio::task::spawn_blocking(snapshot::take_core).await.ok().flatten() };
    if let Some(c) = &core {
        cfg.resume = c.resume.clone();
        crate::perf::mark("gateway_resume_attempt");
    }
    let (gateway, mut rx) = Gateway::spawn(cfg);
    // The token check (clear 401 for a bad token) runs during the gateway
    // handshake instead of before it; dispatches queue in `rx` meanwhile.
    if let Err(e) = rest.get_me().await {
        gateway.shutdown();
        return Err(api_err(e));
    }
    crate::perf::span("rest_get_me", t0);
    if let Some(snapshot::Core { cache, ready, adapter, .. }) = core {
        // Replayed events and RESUMED go through the pump like live ones; an
        // INVALID_SESSION makes the gateway IDENTIFY and the pump resyncs on READY.
        let data = convert::build_ready(&cache, &ready, &adapter.prefs);
        crate::perf::span("resume_session_built", t0);
        return Ok(start_session(&inner, rest, gateway, cache, &ready, adapter, rx, data));
    }
    let cache = Cache::default();

    let ready = tokio::time::timeout(READY_TIMEOUT, async {
        while let Some(ev) = rx.recv().await {
            cache.apply(&ev);
            match ev {
                GatewayEvent::Ready(r) => {
                    crate::perf::span("gateway_ready", t0);
                    return Ok(r);
                }
                GatewayEvent::Fatal { code, reason } => return Err(err(convert::fatal_message(code, &reason))),
                _ => {}
            }
        }
        Err(err("Gateway closed before READY."))
    })
    .await
    .map_err(|_| err("Timed out waiting for Discord."))
    .flatten();
    let ready = match ready {
        Ok(r) => r,
        Err(e) => {
            gateway.shutdown();
            return Err(e);
        }
    };

    if remember && let Err(e) = store_op(&inner.store, move |s| s.save(&token)).await {
        tracing_warn(&format!("could not save token to the keyring: {e}"));
    }

    let prefs = Prefs::from_ready(&ready);
    let data = convert::build_ready(&cache, &ready, &prefs);
    crate::perf::span("ready_built", t0);
    let adapter = Adapter::new(ready.user.id, prefs);
    Ok(start_session(&inner, rest, gateway, cache, &ready, adapter, rx, data))
}

/// Install the session and start the event pump.
#[allow(clippy::too_many_arguments)]
fn start_session(
    inner: &Arc<Inner>,
    rest: RestClient,
    gateway: GatewayHandle,
    cache: Cache,
    ready: &api_ready::Ready,
    adapter: Adapter,
    rx: UnboundedReceiver<GatewayEvent>,
    data: ReadyData,
) -> ReadyData {
    crate::commands::set_session_id(&ready.session_id);
    let adapter = Arc::new(Mutex::new(adapter));
    let session = Arc::new(Session {
        rest,
        gateway,
        cache,
        adapter,
        selected_channel: Mutex::new(None),
        pending_acks: Mutex::new(HashSet::new()),
        subscribed: Mutex::new(HashMap::new()),
        last_typing: Mutex::new(None),
        notify: Mutex::new(crate::notifications::NotifyPrefs::from_ready_with_defaults(
            ready.user_guild_settings.as_ref(),
            ready.guilds.iter().map(|g| (g.id.0, crate::notifications::Level::from_default(g.properties.default_message_notifications))).collect(),
        )),
        new_notifications: inbox::new_notifications(ready.notification_settings.as_ref()),
    });
    pins::init(&session.cache);
    *inner.session.lock() = Some(session.clone());
    if let Some(e) = convert::self_activities(ready) {
        inner.emit(e);
    }
    if let Some(e) = convert::self_status(ready) {
        inner.emit(e);
    }
    nav::snapshot(&session.cache, ready).into_iter().for_each(|e| inner.emit(e));
    // Voice users already in the cache are put on the channels of `data`;
    // their mute / deafen flags go through the UI-thread store.
    for g in &ready.guilds {
        convert::voice_flag_events(&session.cache, Some(g.id)).into_iter().for_each(|e| inner.emit(e));
    }
    history::warm_recent(inner, &session);
    tokio::spawn(pump(inner.clone(), session, rx));
    data
}

fn tracing_warn(msg: &str) {
    eprintln!("discord-phoenix: {msg}");
}

/// Gateway events -> cache -> AppEvents, until the session ends.
async fn pump(inner: Arc<Inner>, s: Arc<Session>, mut rx: UnboundedReceiver<GatewayEvent>) {
    while let Some(ev) = rx.recv().await {
        // Logged out / replaced: stop delivering.
        if !inner.session.lock().as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &s)) {
            return;
        }
        s.cache.apply(&ev);
        msg_cache::on_gateway(&ev);
        if matches!(ev, GatewayEvent::VoiceStateUpdate(_) | GatewayEvent::VoiceServerUpdate(_)) {
            let i = inner.clone();
            let emit: crate::voice::call::Emit = Arc::new(move |e| i.emit(e));
            crate::voice::call::manager().on_gateway(&emit, s.adapter.lock().me.0, &ev);
        }
        account::on_gateway(&s, &ev);
        crate::voice::start_time::on_gateway(&ev);
        thread_members::on_gateway(&ev);
        pins::on_gateway(&ev);
        if let GatewayEvent::Ready(r) = &ev {
            pins::init(&s.cache);
            // Reconnected without RESUME: the cache was rebuilt, rebuild the UI too.
            crate::commands::set_session_id(&r.session_id);
            let prefs = Prefs::from_ready(r);
            let data = convert::build_ready(&s.cache, r, &prefs);
            *s.adapter.lock() = Adapter::new(r.user.id, prefs);
            // New session: nothing is subscribed any more.
            s.subscribed.lock().clear();
            inner.emit(AppEvent::Resync(crate::commands::Resync(data)));
            nav::snapshot(&s.cache, r).into_iter().for_each(|e| inner.emit(e));
            continue;
        }
        if let Some(e) = crate::commands::from_gateway(&ev) {
            inner.emit(AppEvent::Parity(e));
        }
        nav::on_gateway(&s.cache, &ev).into_iter().for_each(|e| inner.emit(e));
        // Mark the open channel read BEFORE mapping, so the guild pill / mention
        // badge computed for this message doesn't flash on while it's being read.
        if let GatewayEvent::MessageCreate(m) = &ev
            && *s.selected_channel.lock() == Some(m.channel_id)
            && m.author.id != s.adapter.lock().me
        {
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            schedule_ack(&s, m.channel_id, m.id);
        }
        let events = s.adapter.lock().map(&s.cache, &ev);
        let fatal = matches!(ev, GatewayEvent::Fatal { .. });
        for e in events {
            inner.emit(e);
        }
        if let GatewayEvent::Fatal { code: 4004, .. } = ev {
            let _ = store_op(&inner.store, |st| st.clear()).await;
        }
        if fatal {
            inner.session.lock().take();
            return;
        }
    }
}

/// Read-only mode (examples/real_shots): never ack / mark anything read on the server.
pub static NO_ACKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn acks_disabled() -> bool {
    NO_ACKS.load(std::sync::atomic::Ordering::Relaxed)
}

fn schedule_ack(s: &Arc<Session>, channel: api::Snowflake, msg: api::Snowflake) {
    if acks_disabled() {
        return;
    }
    s.adapter.lock().mark_read(channel.0, msg.0);
    if !s.pending_acks.lock().insert(channel.0) {
        return;
    }
    let s = s.clone();
    tokio::spawn(async move {
        tokio::time::sleep(ACK_DEBOUNCE).await;
        s.pending_acks.lock().remove(&channel.0);
        let last = s.cache.channel(channel).and_then(|c| c.last_message_id).unwrap_or(msg);
        let _ = s.rest.ack_message(channel, last).await;
    });
}

impl Backend for DiscordBackend {
    fn notify_prefs(&self) -> Option<crate::notifications::NotifyPrefs> {
        Some(self.inner.session.lock().as_ref()?.notify.lock().clone())
    }

    fn set_event_sink(&self, sink: EventSink) {
        *self.inner.sink.lock() = Some(sink);
    }

    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData> {
        self.spawn(move |inner| async move {
            match creds {
                Credentials::Token(t) => {
                    let t = clean_token(&t);
                    if t.is_empty() {
                        return Err(err("Enter a token."));
                    }
                    connect(inner, Secret::new(t), true).await
                }
                Credentials::Password { login, password } => {
                    if login.trim().is_empty() || password.is_empty() {
                        return Err(err("Enter your email and password."));
                    }
                    let rest = RestClient::unauthenticated(inner.props.clone());
                    match rest.login(login.trim(), &password).await.map_err(api_err)? {
                        LoginResponse::Token { token, .. } => connect(inner, Secret::new(token), true).await,
                        LoginResponse::MfaRequired { ticket, totp, sms, backup, .. } => {
                            account::mfa_required(&inner, ticket, totp, sms, backup)
                        }
                    }
                }
                Credentials::Mfa { code } => {
                    let ticket = inner.mfa_ticket.lock().clone().ok_or_else(|| err("Log in with your password first."))?;
                    let code: String = code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
                    let rest = RestClient::unauthenticated(inner.props.clone());
                    let token = rest.login_mfa_totp(ticket.expose(), &code).await.map_err(api_err)?;
                    inner.mfa_ticket.lock().take();
                    connect(inner, Secret::new(token), true).await
                }
            }
        })
    }

    fn restore_session(&self) -> Option<BoxFuture<ReadyData>> {
        Some(self.spawn(move |inner| async move {
            let tok = store_op(&inner.store, |s| s.load()).await.map_err(|e| {
                tracing_warn(&format!("keyring unavailable: {e}"));
                err("")
            })?;
            let Some(tok) = tok else { return Err(err("")) };
            crate::perf::mark("keyring_loaded");
            *inner.pre_rest.lock() = Some(RestClient::new(tok.expose(), inner.props.clone()));
            let res = connect(inner.clone(), tok, false).await;
            if let Err(e) = &res
                && e.0 == "Invalid token."
            {
                let _ = store_op(&inner.store, |s| s.clear()).await;
            }
            res
        }))
    }

    fn mfa_options(&self) -> crate::auth::MfaOptions {
        self.mfa_options_impl()
    }

    fn login_mfa(&self, method: crate::auth::MfaMethod, code: String) -> BoxFuture<ReadyData> {
        self.login_mfa_impl(method, code)
    }

    fn send_mfa_sms(&self) -> BoxFuture<String> {
        self.send_mfa_sms_impl()
    }

    fn remote_auth(&self) -> Option<BoxFuture<ReadyData>> {
        Some(self.remote_auth_impl())
    }

    fn cancel_remote_auth(&self) {
        self.cancel_remote_auth_impl()
    }

    fn load_settings(&self) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        self.load_settings_impl()
    }

    fn save_settings(&self, patch: crate::ui::settings::SettingsPatch) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        self.save_settings_impl(patch)
    }

    fn needs_mfa(&self) -> bool {
        self.inner.mfa_ticket.lock().is_some()
    }

    fn parity(&self, req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        self.with_session(move |inner, s| {
            let emit: crate::backend::EventSink = Arc::new(move |e| inner.emit(e));
            crate::commands::exec::request(s.rest.clone(), req, emit)
        })
    }

    fn action(&self, req: crate::actions::ActionRequest) -> BoxFuture<crate::actions::ActionReply> {
        self.with_session(move |inner, s| {
            let emit: crate::backend::EventSink = Arc::new(move |e| inner.emit(e));
            crate::actions::exec::request(s.rest.clone(), req, emit)
        })
    }

    fn guild_features(&self, guild: Id) -> Vec<String> {
        let Ok(s) = self.inner.session() else { return Vec::new() };
        s.cache.guild(sf(guild)).map(|g| g.properties.features.clone()).unwrap_or_default()
    }

    fn channel_opted_in(&self, channel: Id) -> Option<bool> {
        let s = self.inner.session().ok()?;
        Some(s.adapter.lock().prefs.mutes.opted_in.contains(&channel.0))
    }

    fn inbox_unreads(&self, guild_order: &[Id]) -> Vec<crate::ui::inbox_unreads::Entry> {
        let Ok(s) = self.inner.session() else { return Vec::new() };
        // Clone first: never hold the notify and adapter locks together.
        let notify = s.notify.lock().clone();
        let adapter = s.adapter.lock();
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        inbox::Ctx { cache: &s.cache, prefs: &adapter.prefs, notify: &notify, new_notifications: s.new_notifications }.entries(guild_order, now)
    }

    fn channel_last_message(&self, channel: Id) -> Option<Id> {
        let s = self.inner.session().ok()?;
        s.cache.channel(sf(channel))?.last_message_id.map(|m| Id(m.0))
    }

    fn guild_boosts(&self, guild: Id) -> Option<(u32, bool)> {
        let s = self.inner.session().ok()?;
        let g = s.cache.guild(sf(guild))?;
        Some((g.properties.premium_subscription_count.unwrap_or(0), g.properties.premium_progress_bar_enabled.unwrap_or(false)))
    }

    fn guild_home_header(&self, guild: Id) -> Option<String> {
        let s = self.inner.session().ok()?;
        s.cache.guild(sf(guild))?.properties.home_header.clone()
    }

    fn channel_user_limit(&self, channel: Id) -> Option<u32> {
        let s = self.inner.session().ok()?;
        s.cache.channel(sf(channel))?.user_limit.filter(|n| *n > 0)
    }

    fn is_thread_joined(&self, channel: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        s.cache.channel(sf(channel)).is_some_and(|c| c.member.is_some())
    }

    fn guild_permissions(&self, guild: Id) -> u64 {
        let Ok(s) = self.inner.session() else { return 0 };
        // Base permissions are the same in every channel for the bits we use.
        s.cache.guild_channels(sf(guild)).first().map(|c| s.cache.permissions_in(c.id).bits()).unwrap_or(0)
    }

    fn last_read(&self, channel: Id) -> Option<Id> {
        let s = self.inner.session().ok()?;
        let r = s.adapter.lock().prefs.reads.get(&channel.0).map(|r| r.last_read)?;
        Some(Id(r))
    }

    fn is_media_channel(&self, channel: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        s.cache.channel(sf(channel)).is_some_and(|c| c.kind == api::ChannelType::GuildMedia)
    }

    fn member_can_view(&self, channel: Id, user: Id) -> bool {
        let Ok(s) = self.inner.session() else { return true };
        s.cache
            .member_permissions_in(sf(channel), sf(user))
            .is_none_or(|p| p.contains(phoenix_api::models::Permissions::VIEW_CHANNEL))
    }

    fn request_feed_members(&self, guild: Id, users: Vec<Id>) {
        let Ok(s) = self.inner.session() else { return };
        let g = sf(guild);
        let mut ids: Vec<phoenix_api::models::Snowflake> = users.into_iter().map(sf).filter(|u| s.cache.member(g, *u).is_none()).collect();
        ids.truncate(100);
        if !ids.is_empty() {
            let _ = s.gateway.request_guild_members(phoenix_api::gateway::RequestGuildMembers {
                guild_id: vec![g],
                user_ids: Some(ids),
                presences: true,
                ..Default::default()
            });
        }
    }

    fn activity_feed_enabled(&self, guild: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        let Some(g) = s.cache.guild(sf(guild)) else { return false };
        let has = |f: &str| g.properties.features.iter().any(|x| x == f);
        let (on, off) = (has("ACTIVITY_FEED_ENABLED_BY_USER"), has("ACTIVITY_FEED_DISABLED_BY_USER"));
        if on || off {
            return on;
        }
        g.member_count.is_some_and(|n| n < 10_000)
    }

    fn forum_tags(&self, forum: Id) -> Vec<crate::commands::ForumTag> {
        let Ok(s) = self.inner.session() else { return Vec::new() };
        s.cache.channel(sf(forum)).map(|c| convert::forum_tags(&c)).unwrap_or_default()
    }

    fn nav(&self, req: crate::ui::home::data::NavRequest) -> BoxFuture<crate::ui::home::data::NavReply> {
        self.with_session(move |_, s| nav::request(s.rest.clone(), req))
    }

    fn logout(&self) -> BoxFuture<()> {
        self.spawn(move |inner| async move {
            inner.mfa_ticket.lock().take();
            inner.pre_rest.lock().take();
            if let Some(s) = inner.session.lock().take() {
                s.gateway.shutdown();
            }
            let _ = tokio::task::spawn_blocking(|| {
                snapshot::clear();
                msg_cache::clear();
            })
            .await;
            store_op(&inner.store, |s| s.clear()).await.map_err(err)
        })
    }

    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            if let Some(g) = guild {
                let _ = s.gateway.subscribe_guild(sf(g), GuildSubscription::focused(None));
                if let Some((op, d)) = crate::voice::start_time::request(g.0) {
                    let _ = s.gateway.send(phoenix_api::gateway::GatewayCommand::Raw { op, d });
                }
            }
            Ok(())
        })
    }

    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |inner, s| async move {
            let ch = sf(channel);
            *s.selected_channel.lock() = Some(ch);
            let c = s.cache.channel(ch);
            // Op 37 with a channel range populates the member list + presences.
            // Sent again on every switch: the client asks for the open channel only.
            if let Some(g) = c.as_ref().and_then(|c| c.guild_id)
                && s.subscribed.lock().insert(g.0, ch.0) != Some(ch.0)
            {
                let _ = s.gateway.subscribe_guild(g, GuildSubscription::focused(Some(ch)));
            }
            // The list shown follows the channel's list id, whatever SYNC lands last.
            let shown = s.adapter.lock().show_list(&s.cache, ch);
            shown.into_iter().for_each(|e| inner.emit(e));
            if let Some(last) = c.and_then(|c| c.last_message_id) {
                let was_unread = {
                    let mut a = s.adapter.lock();
                    let unread = a.prefs.reads.get(&ch.0).is_none_or(|r| r.last_read < last.0 || r.mentions > 0);
                    a.mark_read(ch.0, last.0);
                    unread
                };
                if was_unread && !acks_disabled() {
                    s.rest.ack_message(ch, last).await.map_err(api_err)?;
                }
            }
            Ok(())
        })
    }

    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.load_messages_impl(channel, before, limit)
    }

    fn load_messages_at(&self, channel: Id, at: crate::backend::HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        self.load_messages_at_impl(channel, at, limit)
    }

    fn prefetch_messages(&self, channel: Id) {
        self.prefetch_impl(channel)
    }

    fn restore_cached(&self) -> Option<BoxFuture<ReadyData>> {
        if !snapshot::exists() {
            return None;
        }
        Some(self.spawn(move |inner| async move {
            let t0 = Instant::now();
            let loaded = tokio::task::spawn_blocking(snapshot::take).await.ok().flatten();
            let (data, events) = loaded.ok_or_else(|| err(""))?;
            crate::perf::span("snapshot_loaded", t0);
            if inner.session.lock().is_none() {
                events.into_iter().for_each(|e| inner.emit(e));
            }
            Ok(data)
        }))
    }

    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let mut msg = CreateMessage::text(content);
            if let Some(r) = reply_to {
                msg = msg.reply_to(sf(channel), sf(r), true);
            }
            let m = s.rest.send_message(sf(channel), msg).await.map_err(api_err)?;
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            Ok(convert::message(&m))
        })
    }

    fn edit_message(&self, channel: Id, msg: Id, content: String) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let m = s.rest.edit_message(sf(channel), sf(msg), &content).await.map_err(api_err)?;
            Ok(convert::message(&m))
        })
    }

    fn delete_message(&self, channel: Id, msg: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move { s.rest.delete_message(sf(channel), sf(msg)).await.map_err(api_err) })
    }

    fn trigger_typing(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            {
                let mut last = s.last_typing.lock();
                if last.is_some_and(|(c, t)| c == channel.0 && t.elapsed() < TYPING_INTERVAL) {
                    return Ok(());
                }
                *last = Some((channel.0, Instant::now()));
            }
            s.rest.trigger_typing(sf(channel)).await.map_err(api_err)
        })
    }

    fn send_message_ex(
        &self,
        channel: Id,
        content: String,
        reply: Option<(Id, bool)>,
        files: Vec<crate::backend::Upload>,
        nonce: u64,
        progress: Option<crate::backend::UploadProgress>,
    ) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let mut msg = CreateMessage::text(content);
            // enforce_nonce makes the network retries below idempotent.
            msg.nonce = Some(nonce.to_string());
            msg.enforce_nonce = Some(true);
            if let Some((r, mention)) = reply {
                msg = msg.reply_to(sf(channel), sf(r), mention);
            }
            let files: Vec<phoenix_api::rest::FileUpload> = files
                .into_iter()
                .map(|f| phoenix_api::rest::FileUpload {
                    filename: f.filename,
                    bytes: f.bytes,
                    content_type: None,
                    description: f.description,
                    spoiler: f.spoiler,
                })
                .collect();
            // Files first (same flow as the client: upload URLs, chunked PUT to the
            // cloud), once; only the message POST below is retried.
            let attachments = if files.is_empty() {
                Vec::new()
            } else {
                let slots = s.rest.upload_files(sf(channel), &files, progress, phoenix_api::rest::uploads::UPLOAD_CHUNK).await.map_err(|e| match e {
                    e @ ApiError::Api { status: 400..=499, .. } => BackendError::rejected(api_err(e)),
                    e => api_err(e),
                })?;
                phoenix_api::rest::uploads::message_attachments(&files, &slots).as_array().cloned().unwrap_or_default()
            };
            msg.attachments = attachments;
            // The client retries a send the network dropped (measured: second POST
            // ~1.5 s after the first) and only marks it failed after that.
            let mut retries = SEND_RETRY_DELAYS.iter();
            let m = loop {
                let res = s.rest.send_message(sf(channel), msg.clone()).await;
                match res {
                    Ok(m) => break m,
                    Err(ApiError::Http(e)) => match retries.next() {
                        Some(d) => tokio::time::sleep(*d).await,
                        None => return Err(api_err(ApiError::Http(e))),
                    },
                    Err(e @ ApiError::Api { status: 400..=499, .. }) => return Err(BackendError::rejected(api_err(e))),
                    Err(e) => return Err(api_err(e)),
                }
            };
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            Ok(convert::message(&m))
        })
    }

    fn guild_emojis(&self, guild: Id) -> Vec<crate::ui::pickers::types::GuildEmoji> {
        self.inner.session().map(|s| expressions::guild_emojis(&s, guild)).unwrap_or_default()
    }
    fn max_upload_size(&self, channel: Id) -> u64 {
        use phoenix_api::rest::uploads::{max_file_size, message_upload_limit};
        let Ok(s) = self.inner.session() else { return message_upload_limit(10 << 20) };
        let premium = s.cache.current_user().and_then(|u| u.premium_type);
        let guild = s.cache.channel(sf(channel)).and_then(|c| c.guild_id).and_then(|g| s.cache.guild(g));
        let limit = max_file_size(premium, guild.as_ref().map(|g| (g.properties.premium_tier, g.properties.features.iter().map(String::as_str))));
        message_upload_limit(limit)
    }
    fn has_premium(&self) -> bool {
        self.inner.session().ok().and_then(|s| s.cache.current_user()).is_none_or(|u| u.premium_type.unwrap_or(0) > 0)
    }
    fn guild_stickers(&self, guild: Id) -> Vec<crate::ui::pickers::types::StickerData> {
        self.inner.session().map(|s| expressions::guild_stickers(&s, guild)).unwrap_or_default()
    }
    fn sticker_packs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::StickerPackData>> {
        self.with_session(|_, s| expressions::sticker_packs(s))
    }
    fn stop_ringing(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let path = format!("/channels/{}/call/stop-ringing", channel.0);
            s.rest.request_json(reqwest::Method::POST, &path, Some(serde_json::json!({}))).await.map_err(api_err)?;
            Ok(())
        })
    }
    fn send_sticker(&self, channel: Id, sticker: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| expressions::send_sticker(s, channel, sticker))
    }
    fn gif_trending(&self) -> BoxFuture<crate::ui::pickers::types::GifHome> {
        self.with_session(|_, s| expressions::gif_trending(s))
    }
    fn gif_trending_gifs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        self.with_session(|_, s| expressions::gif_trending_gifs(s))
    }
    fn expression_frecency(&self) -> BoxFuture<crate::ui::pickers::types::Frecency> {
        self.with_session(|_, s| expressions::frecency(s))
    }
    fn gif_search(&self, query: String) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        self.with_session(move |_, s| expressions::gif_search(s, query))
    }
    fn search_messages(&self, req: crate::ui::search::SearchRequest) -> BoxFuture<crate::ui::search::SearchPage> {
        self.with_session(move |inner, s| expressions::search(inner, s, req))
    }

    fn set_reaction(&self, channel: Id, msg: Id, emoji: crate::model::Reaction, add: bool) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let e = api::ReactionEmoji {
                id: emoji.emoji_id.map(sf),
                name: Some(crate::ui::chat::state::reaction_name(&emoji.emoji)),
                animated: false,
            };
            if add {
                s.rest.add_reaction(sf(channel), sf(msg), &e).await
            } else {
                s.rest.remove_own_reaction(sf(channel), sf(msg), &e).await
            }
            .map_err(api_err)
        })
    }

    fn set_pinned(&self, channel: Id, msg: Id, pinned: bool) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            if pinned {
                s.rest.pin_message(sf(channel), sf(msg)).await
            } else {
                s.rest.unpin_message(sf(channel), sf(msg)).await
            }
            .map_err(api_err)
        })
    }

    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        self.with_session(move |inner, s| async move {
            let msgs = s.rest.get_pinned_messages(sf(channel)).await.map_err(api_err)?;
            let mut users = Vec::new();
            for m in &msgs {
                convert::message_users(m, &mut users);
            }
            inner.emit(AppEvent::Users(users));
            Ok(msgs.iter().map(convert::message).collect())
        })
    }

    fn has_unread_pins(&self, channel: Id) -> bool {
        pins::has_unread(channel.0)
    }

    /// The client acks when its pins popout mounts with unread pins (`ackPins`):
    /// `POST /channels/{id}/pins/ack`. Never in read-only mode.
    fn ack_pins(&self, channel: Id) -> BoxFuture<()> {
        let send = pins::mark_seen(channel.0) && !acks_disabled();
        self.with_session(move |_, s| async move {
            if send {
                s.rest.request_json(reqwest::Method::POST, &format!("/channels/{}/pins/ack", channel.0), None).await.map_err(api_err)?;
            }
            Ok(())
        })
    }

    fn mark_unread(&self, channel: Id, last_read: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.adapter.lock().prefs.reads.entry(channel.0).or_default().last_read = last_read.0;
            s.rest.ack_message_manual(sf(channel), sf(last_read), 0).await.map_err(api_err)
        })
    }

    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()> {
        self.with_session(move |inner, s| async move {
            let emit: crate::voice::call::Emit = Arc::new(move |e| inner.emit(e));
            crate::voice::call::manager().join(&emit, &s.gateway, &s.cache, channel.map(|c| c.0)).map_err(err)
        })
    }

    fn voice_control(&self, control: crate::voice::VoiceControl) -> BoxFuture<()> {
        let s = self.inner.session.lock().clone();
        crate::voice::call::manager().apply_control(s.as_ref().map(|s| &s.gateway), control);
        Box::pin(async { Ok(()) })
    }

    fn subscribe_member_ranges(&self, guild: Id, channel: Id, ranges: Vec<[u32; 2]>) -> BoxFuture<()> {
        self.people_ranges(guild, channel, ranges)
    }
    fn thread_members(&self, thread: Id) -> BoxFuture<Vec<crate::model::Member>> {
        self.thread_members_impl(thread)
    }
    fn user_profile(&self, user: Id, guild: Option<Id>) -> BoxFuture<crate::presence::Profile> {
        self.people_profile(user, guild)
    }
    fn set_note(&self, user: Id, note: String) -> BoxFuture<()> {
        self.people_note(user, note)
    }
    fn wishlist(&self, user: Id) -> BoxFuture<Vec<crate::ui::profile_wishlist::WishItem>> {
        self.wishlist_impl(user)
    }
    fn game_widgets(&self, user: Id) -> BoxFuture<Vec<crate::ui::profile_games::GameWidget>> {
        self.game_widgets_impl(user)
    }
    fn recent_activity(&self, user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_activity::RecentGame>> {
        self.recent_activity_impl(user)
    }
    fn owned_collectibles(&self) -> BoxFuture<Vec<crate::ui::settings::collectibles::Collectible>> {
        self.owned_collectibles_impl()
    }
    fn collectibles_shop(&self) -> BoxFuture<crate::ui::settings::collectibles::ShopCatalog> {
        self.collectibles_shop_impl()
    }
    fn shop_storefront(&self) -> BoxFuture<serde_json::Value> {
        self.shop_storefront_impl()
    }
    fn app_directory_collections(&self, locale: String) -> BoxFuture<serde_json::Value> {
        // Query of the client's launcher (surface 2, active state 1, platform 1);
        // platform must be numeric, other surfaces/platforms come back empty.
        self.with_session(move |_, s| async move {
            let path = format!("/application-directory-static/collections?surface=2&active_state=1&platform=1&locale={locale}");
            s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err)
        })
    }
    fn orbs_balance(&self) -> BoxFuture<Option<u64>> {
        self.orbs_balance_impl()
    }
    fn collectible_product(&self, sku: u64) -> BoxFuture<Option<crate::ui::settings::collectibles::Collectible>> {
        self.collectible_product_impl(sku)
    }
    fn authorized_apps(&self) -> BoxFuture<Option<Vec<crate::ui::settings::authorized_apps::AuthorizedApp>>> {
        self.authorized_apps_impl()
    }
    fn billing(&self) -> BoxFuture<Option<crate::ui::settings::billing::Billing>> {
        self.billing_impl()
    }
    fn subscriptions(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        self.subscriptions_impl()
    }
    fn session_count(&self) -> BoxFuture<Option<usize>> {
        self.session_count_impl()
    }
    fn profile_widgets(&self, user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_widgets::AppWidget>> {
        self.profile_widgets_impl(user)
    }
    fn update_profile(&self, guild: Option<Id>, patch: crate::ui::settings::profile_editor::ProfilePatch) -> BoxFuture<()> {
        self.profile_update_impl(guild, patch)
    }
    fn set_status(&self, status: crate::model::Presence, custom: Option<String>) -> BoxFuture<()> {
        self.people_status(status, custom)
    }
    fn open_dm(&self, user: Id) -> BoxFuture<Id> {
        self.people_open_dm(user)
    }
}
