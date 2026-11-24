//! Adapter seam between the UI and the network layer.
//!
//! The UI only talks to a `dyn Backend` and consumes `AppEvent`s. Two
//! implementations exist: [`crate::discord_backend::DiscordBackend`] (the real
//! `phoenix-api` adapter: REST calls -> futures, gateway dispatches ->
//! `AppEvent`s pushed through the [`EventSink`]) and
//! [`crate::mock::MockBackend`] (offline data for tests and screenshots).

use std::{future::Future, pin::Pin, sync::Arc};

use crate::model::{Channel, Guild, Id, Member, Message, Presence, Reaction, Role, User};

/// Anchor for [`Backend::load_messages_at`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryAt {
    Around(Id),
    After(Id),
}

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = Result<T, BackendError>> + Send + 'static>>;

#[derive(Clone, Debug)]
pub struct BackendError(pub String);

/// Marks a send the API answered with a 4xx: the client then posts its local
/// "your message could not be delivered" system message under the failed one.
const REJECTED: &str = "\u{1}rejected\u{1}";

impl BackendError {
    pub fn rejected(msg: impl std::fmt::Display) -> Self {
        Self(format!("{REJECTED}{msg}"))
    }
    /// The API refused the message (4xx) rather than the network failing.
    pub fn is_rejected(&self) -> bool {
        self.0.starts_with(REJECTED)
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub enum Credentials {
    Token(String),
    Password { login: String, password: String },
    /// Second step of a password login when [`Backend::needs_mfa`] is true.
    Mfa { code: String },
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.write_str("Token(<redacted>)"),
            Self::Password { login, .. } => {
                f.debug_struct("Password").field("login", login).field("password", &"<redacted>").finish()
            }
            Self::Mfa { .. } => f.write_str("Mfa(<redacted>)"),
        }
    }
}

/// Everything the UI needs once logged in ("READY").
pub struct ReadyData {
    pub me: User,
    pub users: Vec<User>,
    pub guilds: Vec<Guild>,
    pub channels: Vec<Channel>,
    pub roles: Vec<(Id, Vec<Role>)>,
    pub dm_channels: Vec<Channel>,
    /// Known presences at login (friends, DM recipients).
    pub presences: Vec<(Id, Presence)>,
}

/// Pushed from the backend (gateway) to the UI, delivered on the UI thread.
#[derive(Debug)]
pub enum AppEvent {
    Ready,
    MessageCreate(Message),
    MessageUpdate(Message),
    MessageDelete { channel: Id, id: Id },
    /// `nick`: `member.nick` of the TYPING_START payload (guild channels).
    TypingStart { channel: Id, user: Id, nick: Option<gpui::SharedString> },
    PresenceUpdate { user: Id, presence: Presence },
    GuildMembers { guild: Id, members: Vec<Member> },
    ChannelUnread { channel: Id, unread: bool, mentions: u32 },
    VoiceStateUpdate { channel: Option<Id>, user: Id },
    /// Mute / deafen / stream / video flags of a user's voice state.
    VoiceFlags { user: Id, flags: crate::model::VoiceFlags },
    /// Voice call runtime state (connection, ping, speaking, encryption).
    Voice(crate::voice::VoiceUiEvent),
    /// New or updated user objects (message authors, member list entries...).
    Users(Vec<User>),
    /// Background refresh of a channel's latest page that differs from the cached one shown.
    HistoryRefresh { channel: Id, messages: Vec<Message> },
    /// Replace the whole (ordered) channel list of a guild.
    GuildChannels { guild: Id, channels: Vec<Channel> },
    GuildRoles { guild: Id, roles: Vec<Role> },
    GuildUnread { guild: Id, unread: bool, mentions: u32 },
    /// Joined a guild (or it became available again).
    GuildCreate { guild: Guild, channels: Vec<Channel>, roles: Vec<Role> },
    GuildDelete(Id),
    /// A DM / group DM was opened or changed.
    DmUpsert(Channel),
    DmDelete(Id),
    Disconnected(String),
    /// Friends, guild folders, threads, channel decorations (`ui::home`).
    Nav(crate::ui::home::data::NavEvent),
    /// Synced settings were changed by another client (USER_SETTINGS_PROTO_UPDATE): reload them.
    SettingsChanged,
    /// Server-ordered lazy member list of a guild (groups + members).
    MemberListRows { guild: Id, rows: Vec<crate::presence::ListEntry> },
    /// Rich presence of a user (empty = no activity).
    Activities { user: Id, activities: Vec<crate::presence::Activity> },
    /// QR code login progress (see [`Backend::remote_auth`]).
    RemoteAuth(crate::auth::RemoteAuthUpdate),
    /// Fresh READY after a reconnect that could not resume.
    Resync(crate::commands::Resync),
    /// Bot modals, interaction failures, live poll votes.
    Parity(crate::commands::ParityEvent),
}

/// A file to attach to an outgoing message.
#[derive(Clone)]
pub struct Upload {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub spoiler: bool,
    /// Alt text (`attachments[].description`).
    pub description: Option<String>,
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload").field("filename", &self.filename).field("len", &self.bytes.len()).finish()
    }
}

/// Upload progress: `(bytes sent, total bytes)` over every file of a message;
/// returning `false` cancels the upload (nothing more is sent).
pub type UploadProgress = Arc<dyn Fn(u64, u64) -> bool + Send + Sync>;

pub type EventSink = Arc<dyn Fn(AppEvent) + Send + Sync>;

pub trait Backend: Send + Sync + 'static {
    /// Register where gateway events must be delivered.
    fn set_event_sink(&self, sink: EventSink);
    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData>;
    /// Called when the user opens a guild (lazy member list subscription).
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()>;
    fn select_channel(&self, channel: Id) -> BoxFuture<()>;
    /// Newest-last page of history, `before` a message id when paging up.
    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>>;
    /// Detached history (jump to a message): the page `around` a message, or the
    /// page `after` one when scrolling back down. Newest-last. Read-only GETs.
    fn load_messages_at(&self, _channel: Id, _at: HistoryAt, _limit: u32) -> BoxFuture<Vec<Message>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message>;
    fn edit_message(&self, channel: Id, id: Id, content: String) -> BoxFuture<Message>;
    fn trigger_typing(&self, channel: Id) -> BoxFuture<()>;
    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()>;
    /// Mute/deafen, devices and volumes of the voice call.
    fn voice_control(&self, control: crate::voice::VoiceControl) -> BoxFuture<()> {
        let _ = control;
        Box::pin(async { Ok(()) })
    }
    fn delete_message(&self, channel: Id, id: Id) -> BoxFuture<()>;
    /// Send with reply options (`mention` pings the replied-to author) and
    /// file attachments. `nonce` is the client-made snowflake that also serves as
    /// the optimistic local message id; the echoed MESSAGE_CREATE carries it back.
    /// Defaults to [`Backend::send_message`] without files.
    fn send_message_ex(
        &self,
        channel: Id,
        content: String,
        reply: Option<(Id, bool)>,
        files: Vec<Upload>,
        nonce: u64,
        progress: Option<UploadProgress>,
    ) -> BoxFuture<Message> {
        let _ = (files, nonce, progress);
        self.send_message(channel, content, reply.map(|r| r.0))
    }
    /// Per-file upload limit in bytes for `channel` (Nitro of the account,
    /// boosts of the guild), as checked by the client before an upload.
    fn max_upload_size(&self, channel: Id) -> u64 {
        let _ = channel;
        phoenix_api::rest::uploads::message_upload_limit(10 << 20)
    }
    /// Add (`add`) or remove our own reaction.
    fn set_reaction(&self, channel: Id, msg: Id, emoji: Reaction, add: bool) -> BoxFuture<()> {
        let _ = (channel, msg, emoji, add);
        Box::pin(async { Ok(()) })
    }
    fn set_pinned(&self, channel: Id, msg: Id, pinned: bool) -> BoxFuture<()> {
        let _ = (channel, msg, pinned);
        Box::pin(async { Ok(()) })
    }
    /// The channel's pins changed since they were last seen (header pin dot).
    fn has_unread_pins(&self, _channel: Id) -> bool {
        false
    }
    /// Mark the channel's pins seen (the pins popout opened).
    fn ack_pins(&self, _channel: Id) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    /// Pinned messages of a channel, newest first.
    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        let _ = channel;
        Box::pin(async { Ok(Vec::new()) })
    }
    /// "Mark Unread": `last_read` is the message just before the one clicked.
    fn mark_unread(&self, channel: Id, last_read: Id) -> BoxFuture<()> {
        let _ = (channel, last_read);
        Box::pin(async { Ok(()) })
    }
    /// Log in from a stored session at startup, if the backend has one.
    /// The future fails with an empty message when nothing is stored.
    /// Last session from the disk cache, to draw the UI before the gateway answers.
    fn restore_cached(&self) -> Option<BoxFuture<ReadyData>> {
        None
    }
    /// Warm the message cache for a channel the user is likely to open (read-only, never acks).
    fn prefetch_messages(&self, _channel: Id) {}
    fn restore_session(&self) -> Option<BoxFuture<ReadyData>> {
        None
    }
    /// True after a password login asked for a TOTP code.
    fn needs_mfa(&self) -> bool {
        false
    }
    /// Custom emojis of a guild (from the cache).
    fn guild_emojis(&self, _guild: Id) -> Vec<crate::ui::pickers::types::GuildEmoji> {
        Vec::new()
    }
    fn guild_stickers(&self, _guild: Id) -> Vec<crate::ui::pickers::types::StickerData> {
        Vec::new()
    }
    /// Whether the user has Nitro (unlocks emoji/stickers from other servers).
    fn has_premium(&self) -> bool {
        true
    }
    fn sticker_packs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::StickerPackData>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Stop the incoming call ringing for us (`POST /channels/{id}/call/stop-ringing`).
    fn stop_ringing(&self, _channel: Id) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn send_sticker(&self, _channel: Id, _sticker: Id) -> BoxFuture<()> {
        Box::pin(async { Err(BackendError(crate::t!("backend.stickers_unavailable").to_string())) })
    }
    fn gif_trending(&self) -> BoxFuture<crate::ui::pickers::types::GifHome> {
        Box::pin(async { Err(BackendError(crate::t!("backend.gifs_unavailable").to_string())) })
    }
    fn gif_trending_gifs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Favorite GIFs and emoji/sticker frecency synced with the account.
    fn expression_frecency(&self) -> BoxFuture<crate::ui::pickers::types::Frecency> {
        Box::pin(async { Ok(Default::default()) })
    }
    fn gif_search(&self, _query: String) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn search_messages(&self, _req: crate::ui::search::SearchRequest) -> BoxFuture<crate::ui::search::SearchPage> {
        Box::pin(async { Err(BackendError(crate::t!("backend.search_unavailable").to_string())) })
    }
    /// Second-factor methods offered by the pending password login.
    fn mfa_options(&self) -> crate::auth::MfaOptions {
        crate::auth::MfaOptions { totp: true, ..Default::default() }
    }
    /// Finish a password login with a TOTP, SMS or backup code.
    fn login_mfa(&self, _method: crate::auth::MfaMethod, code: String) -> BoxFuture<ReadyData> {
        self.login(Credentials::Mfa { code })
    }
    /// Text an MFA code; resolves to the redacted phone number.
    fn send_mfa_sms(&self) -> BoxFuture<String> {
        Box::pin(async { Err(BackendError(crate::t!("backend.sms_unavailable").to_string())) })
    }
    /// Start a QR code login. Progress arrives as [`AppEvent::RemoteAuth`];
    /// resolves once the phone confirmed (empty error when cancelled/expired).
    fn remote_auth(&self) -> Option<BoxFuture<ReadyData>> {
        None
    }
    fn cancel_remote_auth(&self) {}
    /// The account's per-guild notification levels and mutes (READY
    /// `user_guild_settings`, kept current by updates).
    fn notify_prefs(&self) -> Option<crate::notifications::NotifyPrefs> {
        None
    }
    /// Whether a forum-like channel is a media channel (type 16, grid layout).
    fn is_media_channel(&self, _channel: Id) -> bool {
        false
    }
    /// Whether the member list shows the "Activity" (content inventory) section
    /// for this guild: guild feature ACTIVITY_FEED_ENABLED_BY_USER /
    /// _DISABLED_BY_USER wins, else total member count < 10 000.
    fn activity_feed_enabled(&self, _guild: Id) -> bool {
        true
    }
    /// Request guild members for content feed authors (gateway op 8, like the
    /// client's `requestMember`); results arrive as member events.
    fn request_feed_members(&self, _guild: Id, _users: Vec<Id>) {}
    /// Whether guild member `user` can VIEW_CHANNEL `channel` (true when unknown).
    fn member_can_view(&self, _channel: Id, _user: Id) -> bool {
        true
    }
    /// Synced user settings (settings-proto, JSON fallback).
    fn load_settings(&self) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        Box::pin(async { Ok(Default::default()) })
    }
    fn save_settings(&self, patch: crate::ui::settings::SettingsPatch) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        Box::pin(async move {
            let mut s = crate::ui::settings::SyncedSettings::default();
            s.apply(&patch);
            Ok(s)
        })
    }
    /// Close the session and forget stored credentials.
    fn logout(&self) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    /// Navigation REST calls: invites, relationships, leaving a guild...
    fn nav(&self, _req: crate::ui::home::data::NavRequest) -> BoxFuture<crate::ui::home::data::NavReply> {
        Box::pin(async { Err(BackendError(crate::t!("backend.offline").to_string())) })
    }
    /// Op 37 member list ranges for the open channel (sidebar scrolling).
    fn subscribe_member_ranges(&self, _guild: Id, _channel: Id, _ranges: Vec<[u32; 2]>) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    /// `GET /channels/{thread}/thread-members?with_member=true`: the thread member list (read-only).
    fn thread_members(&self, _thread: Id) -> BoxFuture<Vec<crate::model::Member>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// `GET /users/{id}/profile` (+ note).
    fn user_profile(&self, _user: Id, _guild: Option<Id>) -> BoxFuture<crate::presence::Profile> {
        Box::pin(async { Err(BackendError(crate::t!("backend.profiles_unavailable").to_string())) })
    }
    fn set_note(&self, _user: Id, _note: String) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    /// Profile editor save: `PATCH /users/@me` + `/users/@me/profile`, or
    /// `PATCH /guilds/{id}/members/@me` for a per-server profile.
    fn update_profile(&self, _guild: Option<Id>, _patch: crate::ui::settings::profile_editor::ProfilePatch) -> BoxFuture<()> {
        Box::pin(async { Err(BackendError(crate::t!("backend.profile_edit_unavailable").to_string())) })
    }
    /// Profile Board application widgets (profile + application identities +
    /// widget configs, all GETs).
    fn profile_widgets(&self, _user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_widgets::AppWidget>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Profile wishlist (`wishlist_settings` + `GET /wishlists/{id}`).
    fn wishlist(&self, _user: Id) -> BoxFuture<Vec<crate::ui::profile_wishlist::WishItem>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Profile Board game widgets (`widgets[]` + `GET /games/{id}`).
    fn game_widgets(&self, _user: Id) -> BoxFuture<Vec<crate::ui::profile_games::GameWidget>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Recent played games (`GET /content-inventory/users/{id}/outbox`).
    fn recent_activity(&self, _user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_activity::RecentGame>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Settings > Connections: authorized apps (`GET /oauth2/tokens`).
    fn authorized_apps(&self) -> BoxFuture<Option<Vec<crate::ui::settings::authorized_apps::AuthorizedApp>>> {
        Box::pin(async { Ok(None) })
    }
    /// Settings > Billing: payment sources, country, transactions (GETs).
    fn billing(&self) -> BoxFuture<Option<crate::ui::settings::billing::Billing>> {
        Box::pin(async { Ok(None) })
    }
    /// Settings > Subscriptions: Nitro Squad membership and plan credits (GET only).
    fn subscriptions(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        Box::pin(async { Ok(None) })
    }
    /// Logged-in devices count (`GET /auth/sessions`) for Settings > Account.
    fn session_count(&self) -> BoxFuture<Option<usize>> {
        Box::pin(async { Ok(None) })
    }
    /// Owned collectibles (`GET /users/@me/collectibles-purchases`) for the
    /// profile editor pickers.
    fn owned_collectibles(&self) -> BoxFuture<Vec<crate::ui::settings::collectibles::Collectible>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    /// Shop catalogue (`GET /collectibles-categories/v2`) for the pickers'
    /// "Nitro exclusive" and "Shop" sections. Read-only.
    fn collectibles_shop(&self) -> BoxFuture<crate::ui::settings::collectibles::ShopCatalog> {
        Box::pin(async { Ok(Default::default()) })
    }
    /// Raw shop catalogue (`GET /collectibles-categories/v2`) for the Shop
    /// page cards (names, prices, previews). Read-only.
    fn shop_storefront(&self) -> BoxFuture<serde_json::Value> {
        Box::pin(async { Ok(serde_json::Value::Null) })
    }
    /// App launcher collections (`GET /application-directory-static/collections`,
    /// public and read-only): Promoted, Puzzle Games, ...
    fn app_directory_collections(&self, _locale: String) -> BoxFuture<serde_json::Value> {
        Box::pin(async { Ok(serde_json::Value::Null) })
    }
    /// Orbs balance for the Shop header (`GET /users/@me/virtual-currency/balance`).
    fn orbs_balance(&self) -> BoxFuture<Option<u64>> {
        Box::pin(async { Ok(None) })
    }
    /// One collectible by SKU (`GET /collectibles-products/{sku}`): the
    /// applied profile effect / frame of a shown profile.
    fn collectible_product(&self, _sku: u64) -> BoxFuture<Option<crate::ui::settings::collectibles::Collectible>> {
        Box::pin(async { Ok(None) })
    }
    /// Own status (op 3 + settings). `custom` = custom status text.
    fn set_status(&self, _status: Presence, _custom: Option<String>) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    /// Slash commands, interactions, inbox, moderation, forums, polls (`crate::commands`).
    fn parity(&self, _req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        Box::pin(async { Err(BackendError(crate::t!("backend.offline").to_string())) })
    }
    /// Our guild-level permission bits (moderation menu entries).
    fn guild_permissions(&self, _guild: Id) -> u64 {
        0
    }
    /// Last message we read in `channel` (for the "NEW" divider).
    fn last_read(&self, _channel: Id) -> Option<Id> {
        None
    }
    /// Tags of a forum / media channel.
    fn forum_tags(&self, _forum: Id) -> Vec<crate::commands::ForumTag> {
        Vec::new()
    }
    /// Context menu actions (`crate::actions`).
    fn action(&self, _req: crate::actions::ActionRequest) -> BoxFuture<crate::actions::ActionReply> {
        Box::pin(async { Err(BackendError(crate::t!("backend.offline").to_string())) })
    }
    /// Guild feature flags (`COMMUNITY`, ...).
    fn guild_features(&self, _guild: Id) -> Vec<String> {
        Vec::new()
    }
    /// Browse Channels: the channel's (or category's) override follows it
    /// (`OPT_IN_ENABLED`); `None` when the settings are unknown.
    fn channel_opted_in(&self, _channel: Id) -> Option<bool> {
        None
    }
    /// Newest message id of a channel (Browse Channels "last activity").
    /// Inbox > Unreads channels in the client's order (`ui::inbox_unreads`),
    /// from the read states and settings the backend holds. Read-only.
    fn inbox_unreads(&self, _guild_order: &[Id]) -> Vec<crate::ui::inbox_unreads::Entry> {
        Vec::new()
    }

    fn channel_last_message(&self, _channel: Id) -> Option<Id> {
        None
    }
    /// Boost count (`premium_subscription_count`) and `premium_progress_bar_enabled`.
    fn guild_boosts(&self, _guild: Id) -> Option<(u32, bool)> {
        None
    }
    /// Server Guide header art hash (`home_header`), from READY.
    fn guild_home_header(&self, _guild: Id) -> Option<String> {
        None
    }
    /// Voice channel `user_limit` (0 / None = unlimited).
    fn channel_user_limit(&self, _channel: Id) -> Option<u32> {
        None
    }
    /// Whether the current user has joined (follows) the given thread / forum post.
    fn is_thread_joined(&self, _channel: Id) -> bool {
        false
    }
    /// Open (or create) the DM with `user`; emits `DmUpsert` first.
    fn open_dm(&self, _user: Id) -> BoxFuture<Id> {
        Box::pin(async { Err(BackendError(crate::t!("backend.dm_unavailable").to_string())) })
    }
}
