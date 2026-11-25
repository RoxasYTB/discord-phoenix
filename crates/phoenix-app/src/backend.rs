use std::{future::Future, pin::Pin, sync::Arc};

use crate::model::{Channel, Guild, Id, Member, Message, Presence, Reaction, Role, User};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryAt {
    Around(Id),
    After(Id),
}

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = Result<T, BackendError>> + Send + 'static>>;

#[derive(Clone, Debug)]
pub struct BackendError(pub String);

const REJECTED: &str = "\u{1}rejected\u{1}";

impl BackendError {
    pub fn rejected(msg: impl std::fmt::Display) -> Self {
        Self(format!("{REJECTED}{msg}"))
    }
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

pub struct ReadyData {
    pub me: User,
    pub users: Vec<User>,
    pub guilds: Vec<Guild>,
    pub channels: Vec<Channel>,
    pub roles: Vec<(Id, Vec<Role>)>,
    pub dm_channels: Vec<Channel>,
    pub presences: Vec<(Id, Presence)>,
}

#[derive(Debug)]
pub enum AppEvent {
    Ready,
    MessageCreate(Message),
    MessageUpdate(Message),
    MessageDelete { channel: Id, id: Id },
    TypingStart { channel: Id, user: Id, nick: Option<gpui::SharedString> },
    PresenceUpdate { user: Id, presence: Presence },
    GuildMembers { guild: Id, members: Vec<Member> },
    ChannelUnread { channel: Id, unread: bool, mentions: u32 },
    VoiceStateUpdate { channel: Option<Id>, user: Id },
    VoiceFlags { user: Id, flags: crate::model::VoiceFlags },
    Voice(crate::voice::VoiceUiEvent),
    Users(Vec<User>),
    HistoryRefresh { channel: Id, messages: Vec<Message> },
    GuildChannels { guild: Id, channels: Vec<Channel> },
    GuildRoles { guild: Id, roles: Vec<Role> },
    GuildUnread { guild: Id, unread: bool, mentions: u32 },
    GuildCreate { guild: Guild, channels: Vec<Channel>, roles: Vec<Role> },
    GuildDelete(Id),
    DmUpsert(Channel),
    DmDelete(Id),
    Disconnected(String),
    Nav(crate::ui::home::data::NavEvent),
    SettingsChanged,
    SoundboardSounds { guild: Id, sounds: Vec<crate::model::SoundboardSound> },
    VoiceEffect(crate::voice::soundboard::Effect),
    MemberListRows { guild: Id, rows: Vec<crate::presence::ListEntry> },
    Activities { user: Id, activities: Vec<crate::presence::Activity> },
    RemoteAuth(crate::auth::RemoteAuthUpdate),
    Resync(crate::commands::Resync),
    Parity(crate::commands::ParityEvent),
}

#[derive(Clone)]
pub struct Upload {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub spoiler: bool,
    pub description: Option<String>,
}

#[derive(Clone)]
pub struct VoiceUpload {
    pub ogg: Vec<u8>,
    pub duration_secs: f64,
    pub waveform: String,
}

impl std::fmt::Debug for VoiceUpload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceUpload").field("len", &self.ogg.len()).field("duration_secs", &self.duration_secs).finish()
    }
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload").field("filename", &self.filename).field("len", &self.bytes.len()).finish()
    }
}

pub type UploadProgress = Arc<dyn Fn(u64, u64) -> bool + Send + Sync>;

pub type EventSink = Arc<dyn Fn(AppEvent) + Send + Sync>;

pub trait Backend: Send + Sync + 'static {
    fn set_event_sink(&self, sink: EventSink);
    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData>;
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()>;
    fn select_channel(&self, channel: Id) -> BoxFuture<()>;
    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>>;
    fn load_messages_at(&self, _channel: Id, _at: HistoryAt, _limit: u32) -> BoxFuture<Vec<Message>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message>;
    fn edit_message(&self, channel: Id, id: Id, content: String) -> BoxFuture<Message>;
    fn send_tts_message(&self, channel: Id, content: String) -> BoxFuture<Message> {
        let fut = self.send_message(channel, content, None);
        Box::pin(async move {
            let mut m = fut.await?;
            m.tts = true;
            Ok(m)
        })
    }
    fn trigger_typing(&self, channel: Id) -> BoxFuture<()>;
    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()>;
    fn voice_control(&self, control: crate::voice::VoiceControl) -> BoxFuture<()> {
        let _ = control;
        Box::pin(async { Ok(()) })
    }
    fn delete_message(&self, channel: Id, id: Id) -> BoxFuture<()>;
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
    fn send_voice_message(&self, channel: Id, voice: VoiceUpload, reply: Option<(Id, bool)>, nonce: u64) -> BoxFuture<Message> {
        let _ = (channel, voice, reply, nonce);
        Box::pin(async { Err(BackendError("voice messages are not supported by this backend".into())) })
    }
    fn max_upload_size(&self, channel: Id) -> u64 {
        let _ = channel;
        phoenix_api::rest::uploads::message_upload_limit(10 << 20)
    }
    fn set_reaction(&self, channel: Id, msg: Id, emoji: Reaction, add: bool) -> BoxFuture<()> {
        let _ = (channel, msg, emoji, add);
        Box::pin(async { Ok(()) })
    }
    fn set_pinned(&self, channel: Id, msg: Id, pinned: bool) -> BoxFuture<()> {
        let _ = (channel, msg, pinned);
        Box::pin(async { Ok(()) })
    }
    fn has_unread_pins(&self, _channel: Id) -> bool {
        false
    }
    fn ack_pins(&self, _channel: Id) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        let _ = channel;
        Box::pin(async { Ok(Vec::new()) })
    }
    fn mark_unread(&self, channel: Id, last_read: Id) -> BoxFuture<()> {
        let _ = (channel, last_read);
        Box::pin(async { Ok(()) })
    }
    fn restore_cached(&self) -> Option<BoxFuture<ReadyData>> {
        None
    }
    fn prefetch_messages(&self, _channel: Id) {}
    fn restore_session(&self) -> Option<BoxFuture<ReadyData>> {
        None
    }
    fn needs_mfa(&self) -> bool {
        false
    }
    fn guild_emojis(&self, _guild: Id) -> Vec<crate::ui::pickers::types::GuildEmoji> {
        Vec::new()
    }
    fn guild_stickers(&self, _guild: Id) -> Vec<crate::ui::pickers::types::StickerData> {
        Vec::new()
    }
    fn has_premium(&self) -> bool {
        true
    }
    fn sticker_packs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::StickerPackData>> {
        Box::pin(async { Ok(Vec::new()) })
    }
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
    fn expression_frecency(&self) -> BoxFuture<crate::ui::pickers::types::Frecency> {
        Box::pin(async { Ok(Default::default()) })
    }
    fn gif_search(&self, _query: String) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn search_messages(&self, _req: crate::ui::search::SearchRequest) -> BoxFuture<crate::ui::search::SearchPage> {
        Box::pin(async { Err(BackendError(crate::t!("backend.search_unavailable").to_string())) })
    }
    fn mfa_options(&self) -> crate::auth::MfaOptions {
        crate::auth::MfaOptions { totp: true, ..Default::default() }
    }
    fn login_mfa(&self, _method: crate::auth::MfaMethod, code: String) -> BoxFuture<ReadyData> {
        self.login(Credentials::Mfa { code })
    }
    fn send_mfa_sms(&self) -> BoxFuture<String> {
        Box::pin(async { Err(BackendError(crate::t!("backend.sms_unavailable").to_string())) })
    }
    fn remote_auth(&self) -> Option<BoxFuture<ReadyData>> {
        None
    }
    fn cancel_remote_auth(&self) {}
    fn notify_prefs(&self) -> Option<crate::notifications::NotifyPrefs> {
        None
    }
    fn is_media_channel(&self, _channel: Id) -> bool {
        false
    }
    fn activity_feed_enabled(&self, _guild: Id) -> bool {
        true
    }
    fn request_feed_members(&self, _guild: Id, _users: Vec<Id>) {}
    fn member_can_view(&self, _channel: Id, _user: Id) -> bool {
        true
    }
    fn premium_tier2(&self) -> bool {
        false
    }
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
    fn logout(&self) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn nav(&self, _req: crate::ui::home::data::NavRequest) -> BoxFuture<crate::ui::home::data::NavReply> {
        Box::pin(async { Err(BackendError(crate::t!("backend.offline").to_string())) })
    }
    fn subscribe_member_ranges(&self, _guild: Id, _channel: Id, _ranges: Vec<[u32; 2]>) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn thread_members(&self, _thread: Id) -> BoxFuture<Vec<crate::model::Member>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn user_profile(&self, _user: Id, _guild: Option<Id>) -> BoxFuture<crate::presence::Profile> {
        Box::pin(async { Err(BackendError(crate::t!("backend.profiles_unavailable").to_string())) })
    }
    fn set_note(&self, _user: Id, _note: String) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn update_profile(&self, _guild: Option<Id>, _patch: crate::ui::settings::profile_editor::ProfilePatch) -> BoxFuture<()> {
        Box::pin(async { Err(BackendError(crate::t!("backend.profile_edit_unavailable").to_string())) })
    }
    fn profile_widgets(&self, _user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_widgets::AppWidget>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn wishlist(&self, _user: Id) -> BoxFuture<Vec<crate::ui::profile_wishlist::WishItem>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn game_widgets(&self, _user: Id) -> BoxFuture<Vec<crate::ui::profile_games::GameWidget>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn recent_activity(&self, _user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_activity::RecentGame>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn authorized_apps(&self) -> BoxFuture<Option<Vec<crate::ui::settings::authorized_apps::AuthorizedApp>>> {
        Box::pin(async { Ok(None) })
    }
    fn billing(&self) -> BoxFuture<Option<crate::ui::settings::billing::Billing>> {
        Box::pin(async { Ok(None) })
    }
    fn subscriptions(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        Box::pin(async { Ok(None) })
    }
    fn session_count(&self) -> BoxFuture<Option<usize>> {
        Box::pin(async { Ok(None) })
    }
    fn owned_collectibles(&self) -> BoxFuture<Vec<crate::ui::settings::collectibles::Collectible>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn collectibles_shop(&self) -> BoxFuture<crate::ui::settings::collectibles::ShopCatalog> {
        Box::pin(async { Ok(Default::default()) })
    }
    fn shop_storefront(&self) -> BoxFuture<serde_json::Value> {
        Box::pin(async { Ok(serde_json::Value::Null) })
    }
    fn app_directory_collections(&self, _locale: String) -> BoxFuture<serde_json::Value> {
        Box::pin(async { Ok(serde_json::Value::Null) })
    }
    fn orbs_balance(&self) -> BoxFuture<Option<u64>> {
        Box::pin(async { Ok(None) })
    }
    fn collectible_product(&self, _sku: u64) -> BoxFuture<Option<crate::ui::settings::collectibles::Collectible>> {
        Box::pin(async { Ok(None) })
    }
    fn set_status(&self, _status: Presence, _custom: Option<String>) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn parity(&self, _req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        Box::pin(async { Err(BackendError(crate::t!("backend.offline").to_string())) })
    }
    fn guild_permissions(&self, _guild: Id) -> u64 {
        0
    }
    fn last_read(&self, _channel: Id) -> Option<Id> {
        None
    }
    fn forum_tags(&self, _forum: Id) -> Vec<crate::commands::ForumTag> {
        Vec::new()
    }
    fn action(&self, _req: crate::actions::ActionRequest) -> BoxFuture<crate::actions::ActionReply> {
        Box::pin(async { Err(BackendError(crate::t!("backend.offline").to_string())) })
    }
    fn guild_features(&self, _guild: Id) -> Vec<String> {
        Vec::new()
    }
    fn channel_opted_in(&self, _channel: Id) -> Option<bool> {
        None
    }
    fn inbox_unreads(&self, _guild_order: &[Id]) -> Vec<crate::ui::inbox_unreads::Entry> {
        Vec::new()
    }

    fn channel_last_message(&self, _channel: Id) -> Option<Id> {
        None
    }
    fn guild_boosts(&self, _guild: Id) -> Option<(u32, bool)> {
        None
    }
    fn guild_home_header(&self, _guild: Id) -> Option<String> {
        None
    }
    fn channel_user_limit(&self, _channel: Id) -> Option<u32> {
        None
    }
    fn is_thread_joined(&self, _channel: Id) -> bool {
        false
    }
    fn thread_info(&self, _thread: Id) -> Option<crate::model::ThreadInfo> {
        None
    }
    fn forum_settings(&self, _forum: Id) -> Option<crate::commands::ForumSettings> {
        None
    }
    fn open_dm(&self, _user: Id) -> BoxFuture<Id> {
        Box::pin(async { Err(BackendError(crate::t!("backend.dm_unavailable").to_string())) })
    }
    fn soundboard_default_sounds(&self) -> BoxFuture<Vec<crate::model::SoundboardSound>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn request_soundboard_sounds(&self, _guilds: Vec<Id>) {}
    fn top_soundboard_sounds(&self, _guild: Id) -> BoxFuture<Vec<Id>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn send_soundboard_sound(&self, _channel: Id, _sound: crate::model::SoundboardSound) -> BoxFuture<()> {
        Box::pin(async { Err(BackendError("soundboard unavailable".into())) })
    }
    fn upload_stream_preview(&self, _stream_key: String, _thumbnail: String) -> BoxFuture<()> {
        Box::pin(async { Err(BackendError("stream previews unavailable".into())) })
    }
    fn stream_preview(&self, _stream_key: String) -> BoxFuture<Option<String>> {
        Box::pin(async { Ok(None) })
    }
}
