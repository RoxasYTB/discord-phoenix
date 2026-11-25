use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{
    app::PhoenixApp,
    assets::Assets,
    backend::{AppEvent, Backend, BackendError, BoxFuture, Credentials, EventSink, ReadyData, Upload},
    discord_backend::{
        DiscordBackend,
        store::{KeyringStore, Secret, TokenStore},
    },
    model::{Id, Message, Presence, Reaction},
    ui::settings::AppTheme,
};

struct LoadOnlyStore;

impl TokenStore for LoadOnlyStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        KeyringStore.load()
    }
    fn save(&self, _: &Secret) -> Result<(), String> {
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
}

fn denied<T: Send + 'static>() -> BoxFuture<T> {
    Box::pin(async { Err(BackendError("read-only capture".into())) })
}

const SEND_TEST_CHANNEL: u64 = 1403447573389381652;
const SANDBOX_CHANNEL: u64 = 1574797385463496724;

fn send_allowed(ch: u64) -> bool {
    ch == SEND_TEST_CHANNEL || ch == SANDBOX_CHANNEL
}
const VOICE_TEST_CHANNEL: u64 = 1574900893613039696;

fn voice_test_allowed(channel: Option<Id>) -> bool {
    std::env::var_os("PHX_VOICE").is_some() && channel.is_none_or(|c| c.0 == VOICE_TEST_CHANNEL)
}

const SEND_PREFIX: &str = "[Test Phoenix] envoyé depuis le client Rust (Phoenix) — ";

fn report_path(path: &str) {
    use phoenix_app::ui::modals::Modal;
    phoenix_app::actions::with_modal(|m| {
        let Some(Modal::Report(r)) = m else { return };
        for tok in path.split('|').filter(|t| !t.is_empty()) {
            let Some(node) = r.current().cloned() else { break };
            if let Some(key) = tok.strip_prefix('+') {
                let label = node.element("checkbox").and_then(|e| e.checkbox_options().into_iter().find(|o| o.key == key)).map(|o| o.label);
                if let Some(label) = label {
                    r.toggle(key, &label);
                }
            } else if tok == "=ok" {
                r.submitted(Ok(()));
            } else if tok == "<" {
                r.back();
            } else if tok == ">" {
                if let Some(t) = node.button.as_ref().and_then(|b| b.target) {
                    let _ = r.go(String::new(), t);
                }
            } else if let Some((label, target)) = node.children.iter().find(|(l, _)| l == tok).cloned() {
                if r.menu.as_ref().and_then(|m| m.node(target)).is_some_and(|n| n.is_auto_submit) {
                    eprintln!("real_shots: refusing {tok:?}, that step submits a report");
                    break;
                }
                let _ = r.go(label, target);
            } else {
                eprintln!("real_shots: report step {tok:?} not found");
                break;
            }
        }
    });
}

struct ReadOnly(DiscordBackend);

impl Backend for ReadOnly {
    fn set_event_sink(&self, sink: EventSink) {
        self.0.set_event_sink(Arc::new(move |e: AppEvent| sink(e)));
    }
    fn login(&self, _: Credentials) -> BoxFuture<ReadyData> {
        denied()
    }
    fn restore_session(&self) -> Option<BoxFuture<ReadyData>> {
        self.0.restore_session()
    }
    fn restore_cached(&self) -> Option<BoxFuture<ReadyData>> {
        self.0.restore_cached()
    }
    fn prefetch_messages(&self, channel: Id) {
        self.0.prefetch_messages(channel)
    }
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.0.select_guild(guild)
    }
    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.0.select_channel(channel)
    }
    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.0.load_messages(channel, before, limit)
    }
    fn load_messages_at(&self, channel: Id, at: phoenix_app::backend::HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        self.0.load_messages_at(channel, at, limit)
    }
    fn send_message(&self, _: Id, _: String, _: Option<Id>) -> BoxFuture<Message> {
        denied()
    }
    fn send_message_ex(
        &self,
        ch: Id,
        content: String,
        reply: Option<(Id, bool)>,
        files: Vec<Upload>,
        nonce: u64,
        progress: Option<phoenix_app::backend::UploadProgress>,
    ) -> BoxFuture<Message> {
        if std::env::var_os("PHX_SEND").is_none() || !send_allowed(ch.0) || !content.starts_with(SEND_PREFIX) || reply.is_some() {
            return denied();
        }
        let hold = std::env::var("PHX_SEND_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(4000u64);
        let fail = std::env::var("PHX_SEND_FAIL").ok();
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(hold));
            let _ = tx.send(());
        });
        let real = self.0.clone();
        Box::pin(async move {
            let _ = rx.await;
            eprintln!("send released after {hold} ms (fail={fail:?})");
            match fail {
                None => real.send_message_ex(ch, content, None, files, nonce, progress).await,
                Some(_) => Err(BackendError::rejected("HTTP 400 (simulated locally, not sent)")),
            }
        })
    }
    fn preupload(&self, ch: Id, key: u64, filename: String, bytes: Arc<Vec<u8>>) {
        if std::env::var_os("PHX_SEND").is_some() && send_allowed(ch.0) {
            eprintln!("preupload started: {filename} ({} bytes)", bytes.len());
            self.0.preupload(ch, key, filename, bytes);
        }
    }
    fn drop_preupload(&self, key: u64) {
        self.0.drop_preupload(key);
    }
    fn edit_message(&self, _: Id, _: Id, _: String) -> BoxFuture<Message> {
        denied()
    }
    fn delete_message(&self, _: Id, _: Id) -> BoxFuture<()> {
        denied()
    }
    fn trigger_typing(&self, _: Id) -> BoxFuture<()> {
        denied()
    }
    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()> {
        if !voice_test_allowed(channel) {
            return denied();
        }
        self.0.join_voice(channel)
    }
    fn voice_control(&self, control: phoenix_app::voice::VoiceControl) -> BoxFuture<()> {
        use phoenix_app::voice::VoiceControl as V;
        let test_key = |k: &str| k.starts_with(&format!("guild:1369382919948210248:{VOICE_TEST_CHANNEL}:"));
        if std::env::var_os("PHX_STREAM_WATCH").is_some()
            && let V::WatchStream(k) | V::StopWatching(k) = &control
            && test_key(k)
        {
            return self.0.voice_control(control);
        }
        if std::env::var_os("PHX_VOICE").is_none() {
            return Box::pin(async { Ok(()) });
        }
        let control = match control {
            V::SetInputDevice(_) => V::SetInputDevice(Some(phoenix_voice::audio::device::SYNTHETIC_INPUT.into())),
            V::SetMute(_) => V::SetMute(true),
            V::SetPttKey(_) => return Box::pin(async { Ok(()) }),
            V::SetCameraDevice(_) => V::SetCameraDevice(Some(phoenix_voice::media::camera::TEST_PATTERN.into())),
            V::StartStream { resolution, fps, sound, .. } => V::StartStream { source: phoenix_app::voice::StreamSource::TestPattern, resolution, fps, sound },
            V::WatchStream(k) | V::WatchAfterJoin(k) if !k.contains(&format!(":{VOICE_TEST_CHANNEL}:")) => return Box::pin(async { Ok(()) }),
            c => c,
        };
        self.0.voice_control(control)
    }
    fn send_soundboard_sound(&self, channel: Id, sound: phoenix_app::model::SoundboardSound) -> BoxFuture<()> {
        if std::env::var_os("PHX_VOICE").is_none() || channel.0 != VOICE_TEST_CHANNEL {
            return denied();
        }
        self.0.send_soundboard_sound(channel, sound)
    }
    fn premium_tier2(&self) -> bool {
        self.0.premium_tier2()
    }
    fn upload_stream_preview(&self, key: String, thumbnail: String) -> BoxFuture<()> {
        if std::env::var_os("PHX_VOICE").is_none() || !key.starts_with(&format!("guild:1369382919948210248:{VOICE_TEST_CHANNEL}:")) {
            return denied();
        }
        eprintln!("voice-trace: uploading stream preview for {key} ({} bytes)", thumbnail.len());
        self.0.upload_stream_preview(key, thumbnail)
    }
    fn stream_preview(&self, key: String) -> BoxFuture<Option<String>> {
        self.0.stream_preview(key)
    }
    fn soundboard_default_sounds(&self) -> BoxFuture<Vec<phoenix_app::model::SoundboardSound>> {
        self.0.soundboard_default_sounds()
    }
    fn request_soundboard_sounds(&self, guilds: Vec<Id>) {
        self.0.request_soundboard_sounds(guilds)
    }
    fn top_soundboard_sounds(&self, guild: Id) -> BoxFuture<Vec<Id>> {
        self.0.top_soundboard_sounds(guild)
    }
    fn voice_regions(&self) -> BoxFuture<Vec<(gpui::SharedString, gpui::SharedString)>> {
        self.0.voice_regions()
    }
    fn nav(&self, req: phoenix_app::ui::home::data::NavRequest) -> BoxFuture<phoenix_app::ui::home::data::NavReply> {
        use phoenix_app::ui::home::data::NavRequest as R;
        match req {
            R::FetchInvite(_) | R::FetchInviteEvent(..) | R::TopEmojis(_) => self.0.nav(req),
            _ => denied(),
        }
    }
    fn set_reaction(&self, _: Id, _: Id, _: Reaction, _: bool) -> BoxFuture<()> {
        denied()
    }
    fn set_pinned(&self, _: Id, _: Id, _: bool) -> BoxFuture<()> {
        denied()
    }
    fn mark_unread(&self, _: Id, _: Id) -> BoxFuture<()> {
        denied()
    }
    fn save_settings(&self, _: phoenix_app::ui::settings::SettingsPatch) -> BoxFuture<phoenix_app::ui::settings::SyncedSettings> {
        denied()
    }
    fn set_status(&self, _: Presence, _: Option<String>) -> BoxFuture<()> {
        denied()
    }
    fn save_custom_status(&self, _: Presence, _: Option<phoenix_app::ui::modals::custom_status::StatusDraft>) -> BoxFuture<()> {
        denied()
    }
    fn logout(&self) -> BoxFuture<()> {
        denied()
    }
    fn notify_prefs(&self) -> Option<phoenix_app::notifications::NotifyPrefs> {
        self.0.notify_prefs()
    }
    fn member_can_view(&self, c: Id, u: Id) -> bool {
        self.0.member_can_view(c, u)
    }
    fn activity_feed_enabled(&self, g: Id) -> bool {
        self.0.activity_feed_enabled(g)
    }
    fn request_feed_members(&self, g: Id, u: Vec<Id>) {
        self.0.request_feed_members(g, u)
    }
    fn is_media_channel(&self, c: Id) -> bool {
        self.0.is_media_channel(c)
    }
    fn load_settings(&self) -> BoxFuture<phoenix_app::ui::settings::SyncedSettings> {
        let fut = self.0.load_settings();
        Box::pin(async move {
            let mut s = fut.await?;
            if let Some(f) = env_favorites() {
                s.favorites = f;
            }
            Ok(s)
        })
    }
    fn guild_emojis(&self, g: Id) -> Vec<phoenix_app::ui::pickers::types::GuildEmoji> {
        self.0.guild_emojis(g)
    }
    fn has_premium(&self) -> bool {
        self.0.has_premium()
    }
    fn premium_group_member(&self) -> bool {
        self.0.premium_group_member()
    }
    fn user_profile(&self, u: Id, g: Option<Id>) -> BoxFuture<phoenix_app::presence::Profile> {
        self.0.user_profile(u, g)
    }
    fn authorized_apps(&self) -> BoxFuture<Option<Vec<phoenix_app::ui::settings::authorized_apps::AuthorizedApp>>> {
        self.0.authorized_apps()
    }
    fn user_connections(&self) -> BoxFuture<Option<Vec<phoenix_app::ui::settings::connections::LinkedAccount>>> {
        self.0.user_connections()
    }
    fn billing(&self) -> BoxFuture<Option<phoenix_app::ui::settings::billing::Billing>> {
        self.0.billing()
    }
    fn account_standing(&self) -> BoxFuture<Option<u64>> {
        self.0.account_standing()
    }
    fn auth_sessions(&self) -> BoxFuture<Option<Vec<phoenix_app::ui::settings::sessions::Session>>> {
        self.0.auth_sessions()
    }
    fn profile_widgets(&self, u: Id) -> BoxFuture<Vec<phoenix_app::ui::settings::profile_widgets::AppWidget>> {
        self.0.profile_widgets(u)
    }
    fn wishlist(&self, u: Id) -> BoxFuture<Vec<phoenix_app::ui::profile_wishlist::WishItem>> {
        self.0.wishlist(u)
    }
    fn game_widgets(&self, u: Id) -> BoxFuture<Vec<phoenix_app::ui::profile_games::GameWidget>> {
        self.0.game_widgets(u)
    }
    fn recent_activity(&self, u: Id) -> BoxFuture<Vec<phoenix_app::ui::settings::profile_activity::RecentGame>> {
        self.0.recent_activity(u)
    }
    fn subscribe_member_ranges(&self, g: Id, c: Id, r: Vec<[u32; 2]>) -> BoxFuture<()> {
        self.0.subscribe_member_ranges(g, c, r)
    }
    fn thread_members(&self, t: Id) -> BoxFuture<Vec<phoenix_app::model::Member>> {
        self.0.thread_members(t)
    }
    fn guild_boosts(&self, g: Id) -> Option<(u32, bool)> {
        self.0.guild_boosts(g)
    }
    fn guild_home_header(&self, g: Id) -> Option<String> {
        let h = self.0.guild_home_header(g);
        if std::env::var_os("PHX_DEBUG_GUIDE").is_some() {
            eprintln!("real_shots: home_header {}", if h.is_some() { "present" } else { "absent" });
        }
        h
    }
    fn channel_user_limit(&self, c: Id) -> Option<u32> {
        self.0.channel_user_limit(c)
    }
    fn channel_opted_in(&self, c: Id) -> Option<bool> {
        self.0.channel_opted_in(c)
    }
    fn is_thread_joined(&self, c: Id) -> bool {
        self.0.is_thread_joined(c)
    }
    fn thread_info(&self, c: Id) -> Option<phoenix_app::model::ThreadInfo> {
        self.0.thread_info(c)
    }
    fn forum_settings(&self, c: Id) -> Option<phoenix_app::commands::ForumSettings> {
        self.0.forum_settings(c)
    }
    fn channel_last_message(&self, c: Id) -> Option<Id> {
        self.0.channel_last_message(c)
    }
    fn guild_features(&self, g: Id) -> Vec<String> {
        self.0.guild_features(g)
    }
    fn guild_permissions(&self, g: Id) -> u64 {
        self.0.guild_permissions(g)
    }
    fn channel_permissions(&self, c: Id) -> Option<u64> {
        self.0.channel_permissions(c)
    }
    fn last_read(&self, c: Id) -> Option<Id> {
        self.0.last_read(c)
    }
    fn forum_tags(&self, f: Id) -> Vec<phoenix_app::commands::ForumTag> {
        self.0.forum_tags(f)
    }
    fn collectible_product(&self, sku: u64) -> BoxFuture<Option<phoenix_app::ui::settings::collectibles::Collectible>> {
        self.0.collectible_product(sku)
    }
    fn guarded_write(&self, w: phoenix_app::actions::guard::GuardedWrite) -> BoxFuture<()> {
        eprintln!("real_shots: {}", w.refusal());
        denied()
    }
    fn action(&self, req: phoenix_app::actions::ActionRequest) -> BoxFuture<phoenix_app::actions::ActionReply> {
        if phoenix_app::actions::guard::read_only_action(&req) {
            self.0.action(req)
        } else {
            eprintln!("real_shots: write refused: {req:?}");
            denied()
        }
    }
    fn parity(&self, req: phoenix_app::commands::ParityRequest) -> BoxFuture<phoenix_app::commands::ParityReply> {
        if !phoenix_app::actions::guard::read_only_parity(&req) {
            return denied();
        }
        match req {
            phoenix_app::commands::ParityRequest::ForumPosts { .. } => {
                let what = format!("{req:?}");
                let fut = self.0.parity(req);
                Box::pin(async move {
                    let r = fut.await;
                    if let Err(e) = &r {
                        eprintln!("real_shots: {what} failed: {}", e.0);
                    }
                    r
                })
            }
            phoenix_app::commands::ParityRequest::ContentInventory => self.0.parity(req),
            phoenix_app::commands::ParityRequest::ScheduledEvents { .. } => self.0.parity(req),
            phoenix_app::commands::ParityRequest::Onboarding { .. } => self.0.parity(req),
            phoenix_app::commands::ParityRequest::ServerGuide { .. } => {
                let fut = self.0.parity(req);
                Box::pin(async move {
                    let r = fut.await;
                    if std::env::var_os("PHX_DEBUG_GUIDE").is_some() {
                        match &r {
                            Ok(phoenix_app::commands::ParityReply::Guide(g)) => eprintln!(
                                "real_shots: guide resources {:?}, actions {}, welcome {} chars, profile {}",
                                g.resources.iter().map(|r| (r.title.clone(), r.description.chars().count(), r.icon.is_some())).collect::<Vec<_>>(),
                                g.actions.len(),
                                g.welcome_message.chars().count(),
                                g.profile.is_some()
                            ),
                            Ok(_) => eprintln!("real_shots: guide: unexpected reply"),
                            Err(e) => eprintln!("real_shots: guide failed: {}", e.0),
                        }
                    }
                    r
                })
            }
            phoenix_app::commands::ParityRequest::FetchThread { .. } => self.0.parity(req),
            phoenix_app::commands::ParityRequest::ArchivedThreads { .. } => self.0.parity(req),
            phoenix_app::commands::ParityRequest::Mentions { .. }
            | phoenix_app::commands::ParityRequest::SavedMessages
            | phoenix_app::commands::ParityRequest::ScheduledMessages => {
                let what = format!("{req:?}");
                let fut = self.0.parity(req);
                Box::pin(async move {
                    let r = fut.await;
                    match &r {
                        Ok(phoenix_app::commands::ParityReply::Messages(m)) => eprintln!("real_shots: {what}: {} messages", m.len()),
                        Ok(_) => {}
                        Err(e) => eprintln!("real_shots: {what} failed: {}", e.0),
                    }
                    r
                })
            }
            _ => denied(),
        }
    }
    fn inbox_unreads(&self, order: &[Id]) -> Vec<phoenix_app::ui::inbox_unreads::Entry> {
        self.0.inbox_unreads(order)
    }
    fn quests(&self) -> BoxFuture<serde_json::Value> {
        self.0.quests()
    }
    fn discoverable_guilds(&self, category: Option<u32>, offset: u32) -> BoxFuture<serde_json::Value> {
        self.0.discoverable_guilds(category, offset)
    }
    fn discovery_categories(&self, locale: String) -> BoxFuture<serde_json::Value> {
        self.0.discovery_categories(locale)
    }
    fn app_directory_collections(&self, locale: String) -> BoxFuture<serde_json::Value> {
        self.0.app_directory_collections(locale)
    }
    fn orbs_balance(&self) -> BoxFuture<Option<u64>> {
        self.0.orbs_balance()
    }
    fn directory_get(&self, path: String) -> BoxFuture<serde_json::Value> {
        self.0.directory_get(path)
    }
    fn shop_storefront(&self) -> BoxFuture<serde_json::Value> {
        self.0.shop_storefront()
    }
    fn load_pins(&self, c: Id) -> BoxFuture<Vec<Message>> {
        self.0.load_pins(c)
    }
    fn guild_stickers(&self, g: Id) -> Vec<phoenix_app::ui::pickers::types::StickerData> {
        self.0.guild_stickers(g)
    }
    fn sticker_packs(&self) -> BoxFuture<Vec<phoenix_app::ui::pickers::types::StickerPackData>> {
        self.0.sticker_packs()
    }
    fn gif_trending(&self) -> BoxFuture<phoenix_app::ui::pickers::types::GifHome> {
        self.0.gif_trending()
    }
    fn gif_trending_gifs(&self) -> BoxFuture<Vec<phoenix_app::ui::pickers::types::GifItem>> {
        self.0.gif_trending_gifs()
    }
    fn expression_frecency(&self) -> BoxFuture<phoenix_app::ui::pickers::types::Frecency> {
        self.0.expression_frecency()
    }
    fn gif_search(&self, q: String) -> BoxFuture<Vec<phoenix_app::ui::pickers::types::GifItem>> {
        self.0.gif_search(q)
    }
    fn search_messages(&self, r: phoenix_app::ui::search::SearchRequest) -> BoxFuture<phoenix_app::ui::search::SearchPage> {
        self.0.search_messages(r)
    }
}

fn env_favorites() -> Option<phoenix_api::rest::favorites::Favorites> {
    let spec = std::env::var("PHX_FAVORITES").ok().filter(|v| !v.trim().is_empty())?;
    let mut f = phoenix_api::rest::favorites::Favorites::default();
    if spec.trim() == "empty" {
        f.guild_visible = Some(phoenix_api::rest::settings::BoolValue { value: true });
        return Some(f);
    }
    for (n, group) in spec.split('|').enumerate() {
        let (name, ids) = match group.split_once(':') {
            Some((name, ids)) => (Some(name.trim()), ids),
            None => (None, group),
        };
        let ids: Vec<(u64, Option<u32>)> = ids.split(',').filter_map(|x| x.trim().parse().ok()).map(|x| (x, None)).collect();
        let _ = match name {
            Some(name) => f.add_to_category(&ids, name, 1000 + n as u64, true),
            None => f.add(&ids, None, true),
        };
    }
    Some(f)
}

fn cdp(expr: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let out = std::process::Command::new("node")
        .arg(format!("{home}/discord-phoenix-ref/cdp.mjs"))
        .args(["eval", expr])
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().trim_matches('"').to_string())
}

fn env_or_cdp(var: &str, expr: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.is_empty()).or_else(|| cdp(expr))
}

#[derive(Debug, Default)]
struct Scroll {
    channels: f32,
    top_channel: Option<(Id, f32)>,
    members: f32,
    top_message: Option<(Id, f32)>,
    from_bottom: f32,
    rail: f32,
}

const FEED_JS: &str = r"[...document.querySelectorAll('[class*=container_g7esM5]')].map(c=>{const m=(c.querySelector('img')?.src||'').match(/avatars\/(\d+)/);
return (m?m[1]:'')+'¦'+(c.querySelector('[class*=lineClamp1]')?.textContent||'')}).join('¶')";

const SCROLL_JS: &str = r"(()=>{const q=s=>document.querySelector(s);
const ch=q('[class*=sidebarList_] [class*=scroller_]'),mb=q('[class*=membersWrap_] [class*=scroller_]')||q('[class*=membersWrap_] [class*=members_]'),
ms=q('ol[data-list-id=chat-messages]')?.closest('[class*=scroller_]')||q('[class*=messagesWrapper_] > [class*=scroller_]');let top='',off=0,fb=0,ct='',co=0;
if(ch){const r=ch.getBoundingClientRect();for(const e of ch.querySelectorAll('[data-list-item-id^=channels___]')){
const b=(e.closest('li')||e).getBoundingClientRect();if(b.bottom>r.top+1){ct=e.getAttribute('data-list-item-id').split('___').pop();co=r.top-b.top;break}}}
if(ms){const r=ms.getBoundingClientRect();fb=ms.scrollHeight-ms.scrollTop-ms.clientHeight;
for(const li of ms.querySelectorAll('li[id^=chat-messages-]')){const b=li.getBoundingClientRect();
if(b.bottom>r.top+1){top=li.id.split('-').pop();off=r.top-b.top;break}}}
const rl=q('nav[class*=guilds_] [class*=scroller_]');
return [ch?ch.scrollTop:0,mb?mb.scrollTop:0,top,off,fb,ct,co,rl?rl.scrollTop:0].join(' ')})()";

fn read_scroll() -> Option<Scroll> {
    if std::env::var_os("PHX_NO_SCROLL").is_some() {
        return None;
    }
    let raw = std::env::var("PHX_SCROLL").ok().or_else(|| cdp(SCROLL_JS))?;
    let f: Vec<&str> = raw.split(' ').collect();
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.);
    Some(Scroll {
        channels: num(0),
        top_channel: f.get(5).and_then(|v| parse_id(v)).map(|id| (id, num(6))),
        members: num(1),
        top_message: f.get(2).and_then(|v| parse_id(v)).map(|id| (id, num(3))),
        from_bottom: num(4),
        rail: num(7),
    })
}

fn apply_scroll(
    cx: &mut HeadlessAppContext,
    win: gpui::AnyWindowHandle,
    v: &Entity<PhoenixApp>,
    sc: &Scroll,
    pump: &dyn Fn(&mut HeadlessAppContext, u64),
) -> anyhow::Result<()> {
    use gpui::ListOffset;
    let top = ListOffset { item_ix: 0, offset_in_item: px(0.) };
    let at_bottom = sc.from_bottom < 2.;
    let anchor = cx.update(|cx| {
        if at_bottom {
            return None;
        }
        let app = v.read(cx);
        let (id, off) = sc.top_message?;
        let ix = app.messages.get(&app.selected_channel?)?.index_of(id)?;
        Some((ix + 1, off))
    });
    let ch_anchor = cx.update(|cx| {
        use phoenix_app::app::SidebarRow::*;
        let (id, off) = sc.top_channel?;
        let ix = v.read(cx).sidebar_rows.iter().position(|r| matches!(r, Category(c) | Channel(c) | Thread(c) if *c == id))?;
        Some((ix + 1, off))
    });
    cx.update(|cx| {
        let app = v.read(cx);
        match ch_anchor {
            Some((ix, off)) => app.sidebar_list.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(off.max(0.)) }),
            None => app.sidebar_list.scroll_to(top),
        }
        app.member_list.scroll_to(top);
        phoenix_app::ui::server_bar::scroll_rail_to(sc.rail);
        match anchor {
            Some((ix, off)) => app.message_list.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(off.max(0.)) }),
            None => app.message_list.scroll_to_end(),
        }
    });
    eprintln!("channel anchor {:?} -> {ch_anchor:?}", sc.top_channel);
    if anchor.is_none() && !at_bottom {
        eprintln!("top message not in loaded history, using distance from bottom");
    }
    let mut left = [if ch_anchor.is_none() { sc.channels } else { 0. }, sc.members, if anchor.is_none() && !at_bottom { sc.from_bottom } else { 0. }];
    while left.iter().any(|l| *l > 0.5) {
        cx.update_window(win, |_, window, _| window.refresh())?;
        pump(cx, 100);
        cx.update(|cx| {
            let app = v.read(cx);
            let lists = [&app.sidebar_list, &app.member_list, &app.message_list];
            for (i, l) in lists.into_iter().enumerate() {
                let step = left[i].min(300.);
                if step > 0. {
                    l.scroll_by(px(if i == 2 { -step } else { step }));
                    left[i] -= step;
                }
            }
        });
    }
    Ok(())
}

const LOAD_H: f32 = 800.;

fn parse_id(s: &str) -> Option<Id> {
    s.parse().ok().map(Id)
}

fn main() -> anyhow::Result<()> {
    phoenix_app::perf::start();
    let out = std::env::args().nth(1).unwrap_or_else(|| "real.png".into());
    phoenix_app::discord_backend::NO_ACKS.store(true, std::sync::atomic::Ordering::Relaxed);
    phoenix_app::ui::title_bar::FORCE_CLIENT_DECORATIONS.store(true, std::sync::atomic::Ordering::Relaxed);

    let (guild, channel) = if let Some(dm) = std::env::var("PHX_DM").ok().and_then(|v| parse_id(&v)) {
        (None, Some(dm))
    } else if let Some(c) = std::env::var("PHX_CHANNEL").ok().and_then(|v| parse_id(&v)) {
        (std::env::var("PHX_GUILD").ok().and_then(|v| parse_id(&v)), Some(c))
    } else if let Some(g) = std::env::var("PHX_GUILD").ok().and_then(|v| parse_id(&v)) {
        (Some(g), None)
    } else {
        let path = std::env::var("PHX_ROUTE").ok().or_else(|| cdp("location.pathname")).unwrap_or_default();
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["channels", "@me", c, ..] => (None, parse_id(c)),
            ["channels", g, c, ..] => (parse_id(g), parse_id(c)),
            _ => (None, None),
        }
    };
    let scale: f32 = env_or_cdp("PHX_SCALE", "devicePixelRatio").and_then(|v| v.parse().ok()).filter(|s: &f32| *s > 0.).unwrap_or(2.);
    let w: f32 = env_or_cdp("PHX_W", "innerWidth").and_then(|v| v.parse().ok()).unwrap_or(1280.);
    let h: f32 = env_or_cdp("PHX_H", "innerHeight").and_then(|v| v.parse().ok()).unwrap_or(800.);
    let classes = cdp("document.documentElement.className").unwrap_or_default();
    let theme = match std::env::var("PHX_THEME").ok().as_deref() {
        Some("light") => AppTheme::Light,
        Some("ash") => AppTheme::Ash,
        Some("onyx") => AppTheme::Onyx,
        Some(_) => AppTheme::Dark,
        None if classes.contains("theme-midnight") => AppTheme::Onyx,
        None if classes.contains("theme-darker") => AppTheme::Dark,
        None if classes.contains("theme-light") => AppTheme::Light,
        None if classes.contains("theme-dark") => AppTheme::Ash,
        None => AppTheme::Dark,
    };
    let compact = std::env::var_os("PHX_COMPACT").is_some();
    let density = std::env::var("PHX_DENSITY")
        .ok()
        .and_then(|d| phoenix_app::theme::Density::from_key(&d))
        .or_else(|| classes.split(' ').find_map(|c| c.strip_prefix("density-").and_then(phoenix_app::theme::Density::from_key)))
        .unwrap_or_default();
    let font_px: f32 = std::env::var("PHX_FONT")
        .ok()
        .or_else(|| classes.split(' ').find_map(|c| c.strip_prefix("font-size-").map(str::to_owned)))
        .and_then(|v| v.parse().ok())
        .unwrap_or(16.);
    let audio = env_or_cdp(
        "PHX_AUDIO",
        "[...document.querySelectorAll('button[role=switch][class*=audioButtonWithMenu]')].map(b=>b.getAttribute('aria-checked')==='true'?1:0).join('')",
    )
    .unwrap_or_default();
    let (self_mute, self_deaf) = (audio.starts_with('1'), audio.get(1..2) == Some("1"));
    if std::env::var_os("PHX_SIDEBAR_W").is_none()
        && let Some(sw) = cdp("(document.querySelector('[class*=sidebarList_]')?.getBoundingClientRect().width||'')+''")
        && sw.parse::<f32>().is_ok()
    {
        // SAFETY: single-threaded, before any other thread starts.
        unsafe { std::env::set_var("PHX_SIDEBAR_W", sw) };
    }
    let scroll = read_scroll();
    println!("scroll {scroll:?}");
    let open_folders: Vec<u64> = env_or_cdp(
        "PHX_FOLDERS",
        "[...document.querySelectorAll('nav[class*=guilds_] [aria-expanded=true][data-list-item-id^=guildsnav___]')].map(e=>e.getAttribute('data-list-item-id').slice(12)).join(',')",
    )
    .unwrap_or_default()
    .split(',')
    .filter_map(|s| s.trim().parse().ok())
    .collect();
    let in_voice = env_or_cdp("PHX_IN_VOICE", "(!!document.querySelector('[class*=rtcConnectionStatus_]'))+''").as_deref()
        == Some("true");
    let show_members = env_or_cdp("PHX_MEMBERS", "(!!document.querySelector('[class*=membersWrap_]'))+''").as_deref() != Some("false");
    let wait: u64 = std::env::var("PHX_WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(15);
    println!("target guild={guild:?} channel={channel:?} size={w}x{h} theme={theme:?} compact={compact} density={density:?} font={font_px} mute={self_mute} deaf={self_deaf}");

    let t0 = std::time::Instant::now();
    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), || {
        gpui_platform::current_headless_renderer()
    });
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Medium.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Italic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ]
    .iter()
    .filter_map(|p| std::fs::read(p).ok().map(Into::into))
    .collect();
    let backend = DiscordBackend::with_store(Arc::new(LoadOnlyStore));
    let http = backend.http_client();
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        cx.set_http_client(Arc::new(phoenix_app::media::cache::CachingHttpClient::new(http)));
        phoenix_app::init(cx);
        if std::env::var_os("PHX_FX_STILL").is_some() {
            cx.set_reduce_motion(true);
        }
    });
    let backend: Arc<dyn Backend> = Arc::new(ReadOnly(backend));
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(w), px(h.min(LOAD_H))), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(backend, window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    cx.update_window(win.into(), |_, window, _| window.set_scale_factor(scale))?;
    let pump = |cx: &mut HeadlessAppContext, ms: u64| {
        let end = std::time::Instant::now() + Duration::from_millis(ms);
        while std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
            let t = std::time::Instant::now();
            cx.run_until_parked();
            if std::env::var_os("PHX_TRACE").is_some() && t.elapsed() > Duration::from_millis(300) {
                eprintln!("slow parked run: {:?}", t.elapsed());
            }
        }
    };
    let v = view.borrow().clone().expect("view");
    let mut ready = false;
    for _ in 0..60 {
        pump(&mut cx, 500);
        ready = cx.update(|cx| !v.read(cx).channels.is_empty());
        if ready {
            break;
        }
    }
    eprintln!("[{:.1}s] ready", t0.elapsed().as_secs_f32());
    anyhow::ensure!(ready, "no READY after 30s (token missing from keyring?)");
    if std::env::var_os("PHX_LIST").is_some() {
        cx.update(|cx| {
            let app = v.read(cx);
            for d in &app.dms {
                let group = app.channels.get(d).is_some_and(|c| {
                    c.voice_users.first().and_then(|u| app.users.get(u)).is_none_or(|u| u.display_name != c.name)
                });
                println!("dm {} group={group}", d.0);
            }
            for g in &app.guilds {
                println!("guild {} name={:?}", g.id.0, g.name.as_ref());
            }
            for c in app.channels.values() {
                if let Some(g) = c.guild {
                    println!("channel {} guild={} kind={:?} name={:?}", c.id.0, g.0, c.kind, c.name.as_ref());
                }
            }
            for (parent, list) in &app.nav.threads {
                for t in list {
                    println!("thread {} guild={:?} parent={} name={:?}", t.id.0, t.guild.map(|g| g.0), parent.0, t.name.as_ref());
                }
            }
        });
        return Ok(());
    }
    cx.update_window(win.into(), |_, _, cx| {
        v.update(cx, |app, cx| {
            app.prefs.synced.theme = theme;
            app.prefs.local.zoom = 100;
            app.prefs.synced.compact = compact;
            app.prefs.synced.density = density;
            app.prefs.local.font_size = font_px;
            app.mic_muted = self_mute || self_deaf;
            app.deafened = self_deaf;
            app.nav.expanded_folders.clear();
            app.nav.expanded_folders.extend(open_folders.iter().copied());
            app.show_members = show_members;
            if in_voice {
                app.voice_channel = app
                    .channels
                    .values()
                    .find(|c| c.guild.is_some() && c.voice_users.contains(&app.me.id))
                    .map(|c| c.id);
            }
            let view = std::env::var("PHX_VIEW").unwrap_or_else(|_| {
                let home = guild.is_none() && channel.is_none() && std::env::var("PHX_ROUTE").ok().or_else(|| cdp("location.pathname")).as_deref() == Some("/channels/@me");
                if home { "friends".into() } else { String::new() }
            });
            let tab = std::env::var("PHX_TAB").unwrap_or_default();
            if view == "inbox" && guild.is_none() && channel.is_none() {
                app.select_guild(None, cx);
                app.nav.home = phoenix_app::ui::home::data::HomeView::Friends;
                app.selected_channel = None;
            }
            if view == "friends" {
                use phoenix_app::ui::home::data::FriendsTab as F;
                app.select_guild(None, cx);
                app.nav.home = phoenix_app::ui::home::data::HomeView::Friends;
                app.selected_channel = None;
                app.nav.friends_tab = match tab.as_str() {
                    "all" => F::All,
                    "pending" => F::Pending,
                    "blocked" => F::Blocked,
                    "add" => F::AddFriend,
                    _ => F::Online,
                };
            } else if view == "requests" {
                app.select_guild(None, cx);
                app.nav.home = phoenix_app::ui::home::data::HomeView::MessageRequests;
                app.nav.requests_spam_tab = tab == "spam";
                app.selected_channel = None;
            } else if view == "discovery" {
                app.select_guild(None, cx);
                app.selected_channel = None;
                phoenix_app::ui::discovery::open(if tab == "apps" { phoenix_app::ui::discovery::Tab::Apps } else { phoenix_app::ui::discovery::Tab::Servers });
            } else if view == "quests" {
                app.select_guild(None, cx);
                app.nav.home = phoenix_app::ui::home::data::HomeView::Quests;
                app.selected_channel = None;
            } else if let (Some(g), None) = (guild, channel) {
                app.select_guild(Some(g), cx);
                let first = app.guild_channels.get(&g).and_then(|l| {
                    l.iter().copied().find(|id| app.channels.get(id).is_some_and(|c| c.kind == phoenix_app::model::ChannelKind::Text))
                });
                if let Some(c) = first {
                    app.open_channel_chat(c, cx);
                }
            } else if let Some(c) = channel {
                let thread = app.nav.find_thread(c).cloned();
                let g = guild.or_else(|| app.channels.get(&c).or(thread.as_ref()).and_then(|ch| ch.guild));
                app.select_guild(g, cx);
                if thread.is_some() && !app.channels.contains_key(&c) {
                    app.select_thread(c, cx);
                } else if !app.channels.contains_key(&c) {
                    phoenix_app::ui::forum::open_by_id(app, c, cx);
                } else {
                    app.open_channel_chat(c, cx);
                }
            }
            if view == "favorites"
                && let Some(f) = env_favorites()
            {
                app.prefs.synced.favorites = f;
                app.favorites_last = channel;
                phoenix_app::ui::favorites::open(app, cx);
            }
            if view == "inbox" {
                use phoenix_app::commands::state::InboxTab as T;
                app.parity.inbox.tab = match tab.as_str() {
                    "unreads" => T::Unreads,
                    "saved" => T::Saved,
                    "reminders" => T::Reminders,
                    "scheduled" => T::Scheduled,
                    _ => T::Mentions,
                };
                app.parity.inbox.open = false;
                app.toggle_inbox(cx);
            }
            cx.notify();
        });
    })?;
    pump(&mut cx, wait * 1000 / 2);
    if std::env::var("PHX_VIEW").as_deref() == Ok("favorites")
        && let Some(f) = env_favorites()
    {
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                app.prefs.synced.favorites = f;
                app.favorites_last = channel;
                phoenix_app::ui::favorites::open(app, cx);
                cx.notify();
            })
        })?;
    }
    cx.update_window(win.into(), |_, window, cx| {
        window.resize(size(px(w), px(h)));
        window.bounds_changed(cx);
        window.set_scale_factor(scale);
    })?;
    pump(&mut cx, wait * 1000 / 2);
    if let Some(n) = std::env::var("PHX_INBOX_COLLAPSE").ok().and_then(|v| v.parse::<usize>().ok()) {
        cx.update(|cx| {
            v.update(cx, |app, cx| {
                if let Some(card) = app.parity.inbox.unreads.get_mut(n) {
                    card.collapsed = true;
                }
                cx.notify();
            })
        });
        pump(&mut cx, 500);
    }
    let missing = |cx: &mut HeadlessAppContext| {
        cx.update(|cx| {
            let app = v.read(cx);
            let (Some(g), Some(c)) = (app.selected_guild, app.selected_channel) else { return 0 };
            let members = app.members.get(&g);
            let mut authors: Vec<Id> = app.messages.get(&c).map(|m| m.msgs.iter().map(|m| m.author).collect()).unwrap_or_default();
            authors.sort_unstable_by_key(|a| a.0);
            authors.dedup();
            authors.iter().filter(|a| !members.is_some_and(|ms| ms.iter().any(|m| m.user == **a))).count()
        })
    };
    for _ in 0..20 {
        if missing(&mut cx) == 0 {
            break;
        }
        pump(&mut cx, 500);
    }
    println!("authors without guild member: {}", missing(&mut cx));
    cx.update(|cx| {
        let app = v.read(cx);
        let (Some(g), Some(c)) = (app.selected_guild, app.selected_channel) else { return };
        let Some(entry) = app.messages.get(&c) else { return };
        let mut authors: Vec<Id> = entry.msgs.iter().filter(|m| m.webhook_name.is_none()).map(|m| m.author).collect();
        authors.sort_unstable_by_key(|a| a.0);
        authors.dedup();
        let colored: Vec<&Id> = authors.iter().filter(|a| app.author_role_style_in(Some(g), **a).0.is_some()).collect();
        println!("coloured authors: {}/{}", colored.len(), authors.len());
        for a in authors.iter().filter(|a| !colored.contains(a)) {
            println!("  uncoloured {} {:?}", a.0, app.users.get(a).map(|u| u.display_name.clone()));
        }
    });
    if std::env::var("PHX_DEBUG_NICKS").is_ok() {
        cx.update(|cx| {
            let app = v.read(cx);
            for ch in app.channels.values().filter(|c| !c.voice_users.is_empty() && c.guild == app.selected_guild) {
                let names: Vec<_> = ch.voice_users.iter().map(|u| app.users.get(u).map(|u| u.display_name.clone())).collect();
                println!("voice {} {:?}", ch.name, names);
                for u in &ch.voice_users {
                    let m = app.selected_guild.and_then(|g| app.members.get(&g)).and_then(|ms| ms.iter().find(|m| m.user == *u));
                    println!("  vu {} member={:?}", u.0, m.map(|m| (m.nick.clone(), m.presence, m.activity.clone())));
                }
            }
            let feed = phoenix_app::ui::activity_traits::entries();
            let g = app.selected_guild;
            let members = feed.iter().filter(|e| g.and_then(|g| app.members.get(&g)).is_some_and(|ms| ms.iter().any(|m| m.user == e.author))).count();
            println!("feed {} entries, {} by guild members, {} with icons", feed.len(), members, feed.iter().filter(|e| e.icon.is_some()).count());
            if let Some(c) = app.selected_channel.and_then(|c| app.messages.get(&c)) {
                for m in c.msgs.iter().rev().take(12) {
                    let u = app.users.get(&m.author);
                    println!("msg {} author {} dn={:?} author_nick={:?} member_nick={:?} reply_to={:?}", m.id.0, m.author.0, u.map(|u| u.display_name.clone()), m.author_nick, app.member_nick(m.author), m.reply_to.map(|r| r.0));
                }
            }
        });
    }
    if std::env::var_os("PHX_DEBUG_FX").is_some() {
        cx.update(|cx| {
            for (u, p) in &v.read(cx).people.profiles {
                println!("profile {} effect={:?} frame={:?}", u.0, p.effect_sku, p.frame_sku);
            }
        });
    }
    if std::env::var_os("PHX_NO_FEED_SYNC").is_none()
        && let Some(cards) = cdp(FEED_JS)
    {
        let want: Vec<(Option<u64>, String)> = cards
            .split('¶')
            .filter(|c| !c.is_empty())
            .map(|c| {
                let (u, g) = c.split_once('¦').unwrap_or(("", c));
                (u.parse().ok(), g.to_string())
            })
            .collect();
        let mut feed = phoenix_app::ui::activity_traits::entries();
        let mut front = Vec::new();
        for (u, g) in &want {
            let hit = feed.iter().position(|e| {
                e.game.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(g)) && u.is_none_or(|u| e.author.0 == u)
            });
            if let Some(i) = hit {
                front.push(feed.remove(i));
            }
        }
        println!("feed sync: {}/{} Discord cards matched", front.len(), want.len());
        front.extend(feed);
        phoenix_app::ui::activity_traits::seed(front);
        cx.update(|cx| v.update(cx, |a, cx| {
            a.member_list.remeasure();
            cx.notify();
        }));
        pump(&mut cx, 500);
    }
    eprintln!("[{:.1}s] waited", t0.elapsed().as_secs_f32());
    if let Some(c) = channel.filter(|_| std::env::var("PHX_VIEW").as_deref() != Ok("favorites")) {
        let missed = cx.update(|cx| v.read(cx).selected_channel != Some(c));
        if missed {
            eprintln!("location: {} not selected after the wait, opening it again", c.0);
            cx.update(|cx| v.update(cx, |app, cx| phoenix_app::ui::forum::open_by_id(app, c, cx)));
            pump(&mut cx, 4000);
            let ok = cx.update(|cx| v.read(cx).selected_channel == Some(c));
            eprintln!("location: second try {}", if ok { "ok" } else { "failed" });
        }
    }
    if std::env::var("PHX_VIEW").as_deref() == Ok("inbox") {
        cx.update(|cx| {
            v.update(cx, |a, cx| {
                a.parity.inbox.open = false;
                a.toggle_inbox(cx);
            })
        });
        pump(&mut cx, 3000);
    }
    if std::env::var_os("PHX_DEBUG_UNREADS").is_some() {
        cx.update(|cx| {
            let a = v.read(cx);
            let order = phoenix_app::ui::server_bar::flat_guild_order(a);
            let entries = a.backend.inbox_unreads(&order);
            eprintln!("unreads: {} guilds in order, {} entries from the backend", order.len(), entries.len());
            for e in entries.iter().take(20) {
                eprintln!("  entry {} guild {:?} sort {} mentions {} ack {} last {}", e.channel.0, e.guild.map(|g| g.0), e.sort, e.mentions, e.oldest_read.0, e.newest.0);
            }
            let i = &a.parity.inbox;
            eprintln!("cards: {} (loading {})", i.unreads.len(), i.unreads_loading);
            for c in &i.unreads {
                eprintln!("  card {} loaded {} msgs {} more {} error {:?}", c.entry.channel.0, c.loaded, c.msgs.len(), c.more, c.error);
            }
        });
    }
    if let Some(sc) = scroll {
        apply_scroll(&mut cx, win.into(), &v, &sc, &pump)?;
        eprintln!("[{:.1}s] scrolled", t0.elapsed().as_secs_f32());
    }
    if let Some(id) = std::env::var("PHX_JUMP").ok().and_then(|v| parse_id(&v)) {
        cx.update(|cx| {
            v.update(cx, |app, cx| {
                if let Some(ch) = app.selected_channel {
                    app.jump_to_message(ch, id, cx);
                }
            })
        });
        for _ in 0..60 {
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            pump(&mut cx, 100);
        }
        eprintln!("[{:.1}s] jumped", t0.elapsed().as_secs_f32());
    }
    if std::env::var_os("PHX_NO_FX").is_some() {
        phoenix_app::ui::profile_fx::DISABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let peer = cx.update(|cx| phoenix_app::ui::dm_profile_panel::peer(v.read(cx)));
    let want = std::env::var("PHX_PROFILE").ok();
    let nth = want.as_deref().and_then(|v| v.strip_prefix("member:")).and_then(|n| n.parse::<usize>().ok());
    let member = nth.and_then(|n| {
        cx.update(|cx| {
            let app = v.read(cx);
            app.selected_guild.and_then(|g| app.members.get(&g)?.get(n).map(|m| m.user))
        })
    });
    let profile = match want.as_deref() {
        Some("peer") => peer,
        Some(_) if nth.is_some() => member,
        Some(v) => parse_id(v),
        None => None,
    };
    if let Some(uid) = profile {
        let modal = std::env::var("PHX_PROFILE_KIND").is_ok_and(|k| k == "modal");
        let at = std::env::var("PHX_PROFILE_BOX").ok().and_then(|b| {
            let (x, y) = b.split_once(',')?;
            Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))
        });
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                let g = app.selected_guild;
                let (x, y) = at.unwrap_or((900., 200.));
                app.open_profile(uid, g, gpui::point(px(x + 316.), px(y + 60.)), window, cx);
                if modal {
                    app.open_profile_modal(uid, cx);
                }
            })
        })?;
        pump(&mut cx, wait * 1000 / 3);
    }
    if let Ok(what) = std::env::var("PHX_OPEN") {
        use phoenix_app::ui::settings::Page as P;
        let (kind, arg) = what.split_once(':').unwrap_or((what.as_str(), ""));
        let (kind, arg) = (kind.to_string(), arg.to_string());
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| match kind.as_str() {
                "bell" => {
                    if let Some(ch) = app.selected_channel {
                        phoenix_app::ui::nav_menus::toggle_bell(ch, gpui::point(gpui::px(820.), gpui::px(48.)), window);
                    }
                }
                "threadmenu" => {
                    let mut it = arg.split(',');
                    let id = it.next().and_then(|x| x.parse::<u64>().ok()).map(phoenix_app::model::Id).or(app.selected_channel);
                    let place = match it.next() {
                        Some("ctx") => phoenix_app::actions::thread::Place::ContextMenu,
                        Some("panel") => phoenix_app::actions::thread::Place::Sidebar,
                        _ => phoenix_app::actions::thread::Place::Toolbar,
                    };
                    let x = it.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(700.);
                    let y = it.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(60.);
                    if let Some(id) = id {
                        if place == phoenix_app::actions::thread::Place::ContextMenu {
                            phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Thread(id), gpui::point(px(x), px(y)), window);
                        } else {
                            phoenix_app::ui::thread_menu::open_at(id, place, gpui::point(px(x), px(y)), window);
                        }
                    }
                }
                "threadsettings" | "threaddelete" | "threadpanel" => {
                    let id = arg.parse::<u64>().ok().map(phoenix_app::model::Id).or(app.selected_channel);
                    if let Some(id) = id {
                        match kind.as_str() {
                            "threadsettings" => app.open_thread_settings(id, cx),
                            "threaddelete" => app.confirm_delete_thread(id, false, cx),
                            _ => app.open_thread_split(id, window, cx),
                        }
                    }
                }
                "golive" => {
                    phoenix_app::ui::modals::go_live::open(Some(window));
                    phoenix_app::actions::with_modal(|m| {
                        if let Some(phoenix_app::ui::modals::Modal::GoLive(s)) = m {
                            s.kind = match arg.as_str() {
                                "screen" | "screen-menu" => Some(phoenix_app::voice::stream::SourceKind::Screen),
                                "window" => Some(phoenix_app::voice::stream::SourceKind::Window),
                                _ => None,
                            };
                            s.menu = arg.ends_with("menu");
                            s.devices_tab = arg == "devices";
                        }
                    });
                }
                "pins" => app.toggle_pins(cx),
                "acct" => {
                    use phoenix_app::ui::modals::account_edit::{Confirm, EmailStep, Flow};
                    let flow = match arg.as_str() {
                        "username" => Flow::Username,
                        "email" => Flow::Email(EmailStep::Start),
                        "email-reasons" => Flow::Email(EmailStep::Reasons),
                        "email-warning" => Flow::Email(EmailStep::Warning),
                        "email-form" => Flow::Email(EmailStep::Form),
                        "phone" => Flow::Phone,
                        "phone-entry" => Flow::PhoneEntry,
                        "phone-remove" => Flow::Confirm(Confirm::RemovePhone),
                        "sms" => Flow::Confirm(Confirm::DisableSms),
                        "backup" => Flow::Confirm(Confirm::BackupCodes),
                        "totp" => Flow::RemoveTotp,
                        _ => Flow::Password,
                    };
                    app.open_account_edit(flow, window, cx);
                }
                "imagemenu" | "wishmenu" | "announcemenu" => {
                    use phoenix_app::ui::extra_menus::{Target, open};
                    let target = match (kind.as_str(), arg.as_str()) {
                        ("imagemenu", _) => match &app.me.avatar {
                            phoenix_app::model::ImageRef::Url(u) => Some(Target::Image(u.clone())),
                            phoenix_app::model::ImageRef::Default(_) => None,
                        },
                        ("wishmenu", "more") => Some(Target::WishlistMore(None)),
                        ("wishmenu", _) => Some(Target::WishlistAdd),
                        _ => app.selected_channel.map(|channel| Target::AnnouncementSend { channel, can_thread: true }),
                    };
                    let x = std::env::var("PHX_MENU_X").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(600.);
                    let y = std::env::var("PHX_MENU_Y").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(300.);
                    if let Some(t) = target {
                        open(t, gpui::point(px(x), px(y)), window);
                    }
                }
                "threads" => app.toggle_threads_popout(cx),
                "search" => {
                    phoenix_app::ui::search::open(app, window, cx);
                    phoenix_app::ui::search::set_query(if arg.is_empty() { "a" } else { &arg }, window, cx);
                    if let Ok(mut req) = phoenix_app::ui::search::build_request(app, if arg.is_empty() { "a" } else { &arg }) {
                        req.sort = match std::env::var("PHX_SEARCH_SORT").as_deref() {
                            Ok("old") => phoenix_app::ui::search::SortOrder::Oldest,
                            Ok("rel") => phoenix_app::ui::search::SortOrder::Relevance,
                            _ => phoenix_app::ui::search::SortOrder::Newest,
                        };
                        req.offset = std::env::var("PHX_SEARCH_PAGE").ok().and_then(|p| p.parse::<u32>().ok()).unwrap_or(1).saturating_sub(1)
                            * phoenix_app::ui::search::PAGE;
                        phoenix_app::ui::search::run(app, req, cx);
                    }
                }
                "filters" => {
                    phoenix_app::ui::search::open(app, window, cx);
                    phoenix_app::ui::search_filters::open(app, window, cx);
                    if arg == "date" || arg == "calendar" {
                        phoenix_app::ui::search_filters::add_date(cx, arg == "calendar");
                    }
                    if let Some(n) = arg.strip_prefix("list").and_then(|n| n.parse().ok()) {
                        phoenix_app::ui::search_filters::show_list(cx, n);
                    }
                }
                "settings" => {
                    phoenix_app::ui::settings::open(app, cx);
                    let pages = [
                        P::MyAccount, P::Profiles, P::DataPrivacy, P::PrivacySafety, P::Devices, P::Connections,
                        P::Notifications, P::Keybinds, P::Language, P::Appearance, P::Accessibility, P::VoiceVideo,
                        P::Advanced, P::System, P::ActivityPrivacy, P::RegisteredGames, P::Nitro, P::ServerBoost,
                        P::Subscriptions, P::GiftInventory, P::Billing,
                    ];
                    if let (Some(s), Some(p)) =
                        (app.settings.as_mut(), pages.into_iter().find(|p| format!("{p:?}").eq_ignore_ascii_case(&arg)))
                    {
                        s.page = p;
                    }
                    use phoenix_app::ui::settings::nested::Nested as N;
                    let nested = match arg.as_str() {
                        "devices" => Some(N::Devices),
                        "standing" => Some(N::Standing),
                        "mfa" => Some(N::Mfa),
                        "family" | "family_members" | "family_settings" => Some(N::FamilyCenter),
                        _ => None,
                    };
                    if let (Some(s), Some(n)) = (app.settings.as_mut(), nested) {
                        s.page = P::MyAccount;
                        s.nested = Some(n);
                    }
                    phoenix_app::ui::settings::nested::set_family_tab(match arg.as_str() {
                        "family_members" => 1,
                        "family_settings" => 2,
                        _ => 0,
                    });
                    if arg == "changelog"
                        && let Some(s) = app.settings.as_mut()
                    {
                        s.show_changelog = true;
                        phoenix_app::updater::spawn_changelog(cx);
                    }
                }
                "server_settings" => {
                    if let Some(g) = app.selected_guild {
                        let (section, sub) = arg.split_once('/').unwrap_or((arg.as_str(), ""));
                        let index = section.parse().unwrap_or_else(|_| phoenix_app::ui::modals::server_settings::section_index(&if section.starts_with("menu.") { section.to_owned() } else { format!("menu.gs.{section}") }));
                        app.open_server_settings(g, index, cx);
                        phoenix_app::ui::server_pages::open_sub(g, sub);
                    }
                }
                "emoji" | "gif" | "sticker" => {
                    use phoenix_app::ui::pickers::{EmojiTarget, Tab};
                    let tab = match kind.as_str() {
                        "gif" => Tab::Gif,
                        "sticker" => Tab::Sticker,
                        _ => Tab::Emoji,
                    };
                    phoenix_app::ui::pickers::pin_favorite_tile(std::env::var("PHX_GIF_FAV").ok().filter(|v| !v.is_empty()));
                    phoenix_app::ui::pickers::open(app, tab, EmojiTarget::Composer, window, cx);
                    if let Ok(row) = arg.parse::<usize>()
                        && let Some(op) = cx.try_global::<phoenix_app::ui::pickers::Pickers>().and_then(|p| p.open.as_ref())
                    {
                        op.list.scroll_to(gpui::ListOffset { item_ix: row, offset_in_item: px(0.) });
                    }
                }
                "chanmenu" | "catmenu" => {
                    use phoenix_app::app::SidebarRow;
                    let n: usize = arg.parse().unwrap_or(0);
                    let cat = kind == "catmenu";
                    let id = app
                        .sidebar_rows
                        .iter()
                        .filter_map(|r| match r {
                            SidebarRow::Category(c) if cat => Some(*c),
                            SidebarRow::Channel(c) if !cat => Some(*c),
                            _ => None,
                        })
                        .nth(n);
                    if let Some(id) = id {
                        phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Channel(id), gpui::point(px(300.), px(300.)), window);
                    }
                }
                "guildmenu" => {
                    if let Some(g) = app.selected_guild {
                        phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Guild(g), gpui::point(px(80.), px(200.)), window);
                    }
                }
                "guilddrop" => app.nav.guild_menu = true,
                "usermenu" => {
                    let u = app.selected_guild.and_then(|g| app.members.get(&g)?.get(arg.parse().unwrap_or(0)).map(|m| m.user));
                    if let Some(u) = u {
                        phoenix_app::ui::user_menu::open(u, app.selected_guild, gpui::point(px(900.), px(250.)), window);
                    }
                }
                "switcher" => phoenix_app::ui::switcher::open(app, window, cx),
                "thread" => {
                    if let Ok(id) = arg.parse::<u64>() {
                        let id = phoenix_app::model::Id(id);
                        let name = app.channels.get(&id).or_else(|| app.nav.find_thread(id)).map(|c| c.name.clone()).unwrap_or_default();
                        app.open_thread(phoenix_app::model::ThreadRef { id, name, message_count: 0 }, window, cx);
                    }
                }
                "attach" => app.chat.attach_menu = true,
                "poll_create" => {
                    if let Some(ch) = app.selected_channel {
                        phoenix_app::actions::open_modal(
                            phoenix_app::ui::modals::Modal::CreatePoll(phoenix_app::ui::modals::poll_create::State::new(ch)),
                            Some(window),
                        );
                        if arg == "scheduled" || arg == "picker" {
                            use phoenix_app::ui::modals::poll_create as pc;
                            pc::set_scheduled(Some(phoenix_app::ui::chat::scheduled::default_at(chrono::Local::now())));
                            if arg == "picker" {
                                pc::open_schedule(cx);
                            }
                        }
                        if arg == "emoji" {
                            use phoenix_app::ui::modals::poll_create as pc;
                            pc::pick_emoji(1, pc::AnswerEmoji { id: None, name: "\u{1f525}".into(), animated: false });
                            phoenix_app::ui::pickers::open_emoji_picker(app, phoenix_app::ui::pickers::EmojiTarget::PollAnswer(0), window, cx);
                        }
                    }
                }
                "thread_create" => {
                    if let (Some(ch), Ok(m)) = (app.selected_channel, arg.parse::<u64>()) {
                        app.open_create_thread_from(ch, phoenix_app::model::Id(m), window, cx);
                    }
                }
                "chansettings" | "createchannel" | "privacy" | "serverprofile" | "security" | "createevent" | "nickname" => {
                    if let Some(g) = app.selected_guild {
                        match kind.as_str() {
                            "chansettings" => {
                                let target = std::env::var("PHX_CS_CHANNEL").ok().and_then(|v| v.parse::<u64>().ok()).map(phoenix_app::model::Id).or(app.selected_channel);
                                if let Some(c) = target {
                                    app.open_channel_settings(c, cx);
                                    use phoenix_app::ui::modals::channel_settings::Section as S;
                                    let sec = match arg.as_str() {
                                        "perms" => Some(S::Permissions),
                                        "invites" => Some(S::Invites),
                                        "integrations" => Some(S::Integrations),
                                        "webhooks" => Some(S::Webhooks),
                                        _ => None,
                                    };
                                    if let Some(sec) = sec {
                                        phoenix_app::ui::modals::channel_settings::pick_section(app, sec, cx);
                                    }
                                    if std::env::var("PHX_CS_ADD").is_ok() {
                                        phoenix_app::actions::with_modal(|m| {
                                            if let Some(phoenix_app::ui::modals::Modal::ChannelSettings(s)) = m {
                                                s.add_members = Some(phoenix_app::ui::modals::channel_perms::AddMembers { search: None, picked: Vec::new() });
                                            }
                                        });
                                    }
                                    if std::env::var("PHX_CS_DELETE").is_ok() {
                                        phoenix_app::actions::with_modal(|m| {
                                            if let Some(phoenix_app::ui::modals::Modal::ChannelSettings(s)) = m {
                                                s.confirm_delete = true;
                                            }
                                        });
                                    }
                                    if let Ok(w) = std::env::var("PHX_CS_HOOK") {
                                        phoenix_app::actions::with_modal(|m| {
                                            if let Some(phoenix_app::ui::modals::Modal::ChannelSettings(s)) = m {
                                                s.open_webhook = Some(w.clone());
                                            }
                                        });
                                    }
                                }
                            }
                            "createchannel" => app.open_create_channel(g, None, phoenix_app::model::ChannelKind::Text, cx),
                            "privacy" => app.open_privacy_settings(g, cx),
                            "serverprofile" => app.open_server_profile(g, cx),
                            "security" => app.open_security_actions(g, cx),
                            "createevent" => {
                                app.open_create_event(g, cx);
                                let kind = match arg.split(':').next().unwrap_or("") {
                                    "voice" => Some(phoenix_app::ui::modals::create_event::Kind::Voice),
                                    "external" => Some(phoenix_app::ui::modals::create_event::Kind::External),
                                    "stage" => Some(phoenix_app::ui::modals::create_event::Kind::Stage),
                                    _ => None,
                                };
                                if let Some(k) = kind {
                                    app.set_event_kind(k, cx);
                                }
                                let step = arg.split(':').nth(1).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                                let cover = std::env::var("PHX_EVENT_COVER").ok().map(std::path::PathBuf::from);
                                phoenix_app::actions::with_modal(|m| {
                                    if let Some(phoenix_app::ui::modals::Modal::CreateEvent(s)) = m {
                                        s.step = step.min(2);
                                        s.cover = cover;
                                        if std::env::var("PHX_EVENT_OPEN").as_deref() == Ok("channel") {
                                            s.open = Some(phoenix_app::ui::modals::create_event::Open::Channel);
                                        }
                                        if step >= 3 {
                                            s.done = true;
                                            s.link = Some("https://discord.gg/hTKzmak?event=1".into());
                                        }
                                    }
                                });
                            }
                            _ => {
                                let me = app.me.id;
                                let list = app.members.get(&g);
                                let pick = list
                                    .and_then(|l| l.iter().find(|m| m.user != me && m.nick.is_some()).or_else(|| l.iter().find(|m| m.user != me)))
                                    .map(|m| m.user);
                                app.open_nickname(g, pick, cx);
                            }
                        }
                    }
                }
                "reactions" | "forward" => {
                    let pick = app.selected_channel.and_then(|c| {
                        let msgs = &app.messages.get(&c)?.msgs;
                        let m = msgs.iter().rev().find(|m| !m.reactions.is_empty()).or(msgs.last())?;
                        Some((c, m.id))
                    });
                    if let Some((c, m)) = pick {
                        if kind == "reactions" {
                            app.open_reactions(c, m, 0, cx)
                        } else {
                            app.open_forward(c, m, cx)
                        }
                    }
                }
                "guildnotify" => {
                    if let Some(g) = app.selected_guild {
                        let st = phoenix_app::ui::modals::guild_notify::State::new(app, g);
                        phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::GuildNotify(st), None);
                    }
                }
                "statuspicker" => {
                    app.toggle_status_picker(window, cx);
                    phoenix_app::ui::status_picker::debug_open_status_submenu(arg == "sub");
                    phoenix_app::ui::status_picker::debug_open_accounts_submenu(arg == "accounts");
                }
                "accounts" => app.open_switch_accounts(cx),
                "gdmclean" => app.open_gdm_cleanup(cx),
                "spamreq" => app.open_spam_requests(cx),
                "clearpending" => app.confirm_clear_pending(arg.parse().unwrap_or(5), cx),
                "stranger" => phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::AcceptStranger(Id(arg.parse().unwrap_or(0))), None),
                "addfriend" => {
                    use phoenix_app::ui::home::add_friend as af;
                    match arg.split_once(':') {
                        Some(("ok", tag)) => {
                            app.nav.add_friend.on_success(tag);
                            app.nav.add_friend_success_tag = Some(tag.to_owned().into());
                        }
                        Some(("err", code)) => {
                            let f = af::Failure { status: 400, code: code.parse().ok(), note_rejected: false };
                            app.nav.add_friend.on_error(af::inline_error(&f, "wumpus"));
                        }
                        _ => app.nav.add_friend.on_change(&arg),
                    }
                    cx.notify();
                }
                "discovery" => {
                    use phoenix_app::ui::discovery as d;
                    let apps = arg == "apps" || arg.starts_with("app:");
                    d::open(if apps { d::Tab::Apps } else { d::Tab::Servers });
                    if let Some(id) = arg.strip_prefix("app:").and_then(|x| x.parse().ok()) {
                        phoenix_app::ui::app_profile::open(id);
                    } else if let Some(c) = arg.strip_prefix("cat:").and_then(|x| x.parse().ok()) {
                        d::set_category(d::Category::Id(c));
                    } else if let Some(q) = arg.strip_prefix("search:") {
                        d::start_search(app, q.to_owned(), None, cx);
                    }
                    cx.notify();
                }
                "questsview" => {
                    phoenix_app::ui::quests_page::debug_open(arg == "claimed", arg.as_str());
                    cx.notify();
                }
                "boost" => {
                    if let Some(g) = app.selected_guild {
                        phoenix_app::ui::boost_layer::open(g);
                        cx.notify();
                    }
                }
                "customstatus" => {
                    app.open_custom_status(cx);
                    if arg == "emoji" {
                        phoenix_app::ui::pickers::open_emoji_picker(app, phoenix_app::ui::pickers::EmojiTarget::Status, window, cx);
                    } else if !arg.is_empty() {
                        phoenix_app::ui::modals::custom_status::pick_emoji(phoenix_app::ui::modals::custom_status::StatusEmoji { id: None, name: arg.clone().into(), animated: false });
                    }
                }
                "plaintextlang" | "plaintextmd" => {
                    let url = app.selected_channel.and_then(|c| app.messages.get(&c)).and_then(|m| {
                        m.msgs.iter().rev().flat_map(|m| m.attachments.iter()).find(|a| phoenix_app::ui::chat::plaintext::previewable(&a.filename)).map(|a| a.url.clone())
                    });
                    if let Some(u) = url {
                        if kind == "plaintextmd" {
                            phoenix_app::ui::chat::plaintext::set_language(&u, "markdown");
                        } else {
                            let (x, y) = arg.split_once(',').and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))).unwrap_or((900., 500.));
                            phoenix_app::ui::chat::plaintext::open_language_picker(u, gpui::point(px(x), px(y)), false, window, cx);
                        }
                    }
                }
                "activethreads" => {
                    if let Some(g) = app.selected_guild {
                        app.open_active_threads(g, cx);
                    }
                }
                "ignore" => {
                    let target = arg.parse::<u64>().ok().map(phoenix_app::model::Id).or_else(|| {
                        let ch = app.selected_channel?;
                        let me = app.me.id;
                        app.messages.get(&ch)?.msgs.iter().rev().map(|m| m.author).find(|a| *a != me)
                    });
                    if let Some(u) = target {
                        phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::Ignore(u), None);
                    }
                }
                "leave" => {
                    if let Some(g) = app.selected_guild {
                        app.show_overlay(phoenix_app::ui::home::data::Overlay::LeaveGuild(g), cx);
                    }
                }
                "invite" => {
                    if let Some(guild) = app.selected_guild {
                        app.show_overlay(phoenix_app::ui::home::data::Overlay::InviteLink { guild, link: None }, cx);
                    }
                }
                "community" => {
                    if let Some(g) = app.selected_guild {
                        phoenix_app::ui::community::open_tab(app, g, arg == "browse", cx);
                    }
                }
                "guide" => {
                    if let Some(g) = app.selected_guild {
                        phoenix_app::ui::guide::open(app, g, cx);
                    }
                }
                "guidelines" => {
                    if let Some(f) = app.selected_channel {
                        let v = app.parity.forums.entry(f).or_default();
                        v.guidelines_open = true;
                        v.guidelines_modal = arg == "all";
                    }
                }
                "favadd" => phoenix_app::ui::favorites::open_add_modal(),
                "events" => {
                    if let Some(g) = app.selected_guild {
                        phoenix_app::ui::events::open(app, g, cx);
                    }
                }
                "post" => {
                    let n: usize = arg.parse().unwrap_or(0);
                    let post = app
                        .selected_channel
                        .and_then(|f| app.parity.forums.get(&f))
                        .and_then(|v| v.posts.iter().chain(v.archived.iter()).nth(n))
                        .map(|p| phoenix_app::model::ThreadRef {
                            id: p.thread,
                            name: p.title.clone().into(),
                            message_count: p.message_count,
                        });
                    if let Some(r) = post {
                        app.open_thread(r, window, cx);
                    }
                }
                "report" => {
                    let pick = app.selected_channel.and_then(|c| {
                        let msgs = &app.messages.get(&c)?.msgs;
                        let m = arg.parse::<u64>().ok().and_then(|id| msgs.iter().find(|m| m.id.0 == id)).or(msgs.last())?;
                        Some((c, m.id))
                    });
                    if let Some((c, m)) = pick {
                        let sf = phoenix_api::models::Snowflake;
                        app.open_report(phoenix_api::rest::reporting::ReportTarget::Message { channel: sf(c.0), message: sf(m.0) }, cx);
                    }
                }
                "textmenu" => phoenix_app::ui::extra_menus::open(phoenix_app::ui::extra_menus::Target::Textarea, gpui::point(px(500.), px(560.)), window),
                "profilemenu" => {
                    let u = app.selected_guild.and_then(|g| app.members.get(&g)?.get(arg.parse().unwrap_or(0)).map(|m| m.user));
                    if let Some(user) = u {
                        let target = phoenix_app::ui::extra_menus::Target::ProfileOverflow { user, guild: app.selected_guild };
                        phoenix_app::ui::extra_menus::open(target, gpui::point(px(900.), px(250.)), window);
                    }
                }
                "friendmenu" => {
                    let friend = app.nav.relationships.iter().find(|r| r.kind == phoenix_app::ui::home::data::RelationshipKind::Friend).map(|r| r.user);
                    if let Some(u) = friend {
                        phoenix_app::ui::extra_menus::open(phoenix_app::ui::extra_menus::Target::FriendRow(u), gpui::point(px(900.), px(250.)), window);
                    }
                }
                "addserver" => app.open_add_server(window, cx),
                "favmenu" => phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Favorites, gpui::point(px(40.), px(130.)), window),
                "favaddmenu" => phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::FavoritesAdd, gpui::point(px(300.), px(70.)), window),
                "favheader" => {
                    app.nav.guild_menu = true;
                    cx.notify();
                }
                "foldermenu" | "foldermodal" => {
                    let id = app.nav.folders.iter().filter_map(|f| f.id).nth(arg.parse().unwrap_or(0));
                    if let Some(f) = id {
                        if kind == "foldermenu" {
                            phoenix_app::ui::nav_menus::open(phoenix_app::ui::nav_menus::Target::Folder(f), gpui::point(px(80.), px(200.)), window);
                        } else {
                            app.open_folder_settings(f, cx);
                        }
                    }
                }
                "editgroup" => {
                    let gdm = app.nav.extras.iter().find(|(_, x)| x.recipients > 1).map(|(id, _)| *id);
                    if let Some(ch) = gdm {
                        app.open_edit_group(ch, cx);
                    }
                }
                "schedmodal" | "schedbar" | "schedmenu" | "schedcal" | "schedtime" => {
                    if let Some(ch) = app.selected_channel {
                        let at = phoenix_app::ui::chat::scheduled::default_at(chrono::Local::now());
                        match kind.as_str() {
                            "schedmodal" => app.open_schedule(ch, cx),
                            "schedcal" | "schedtime" => {
                                app.open_schedule(ch, cx);
                                let which = if kind == "schedcal" { 0 } else { 1 };
                                phoenix_app::actions::with_modal(|m| {
                                    if let Some(phoenix_app::ui::modals::Modal::Schedule(p)) = m {
                                        p.open = Some(which);
                                    }
                                });
                            }
                            "schedbar" => phoenix_app::ui::chat::scheduled::set_draft(ch, Some(at)),
                            _ => {
                                app.chat.attach_menu = true;
                                phoenix_app::ui::context_menu::debug_open_submenu(arg.parse().unwrap_or(4));
                            }
                        }
                    }
                }
                "friendnick" => {
                    let friend = app.nav.relationships.iter().find(|r| r.kind == phoenix_app::ui::home::data::RelationshipKind::Friend).map(|r| r.user);
                    if let Some(u) = friend {
                        app.open_friend_nickname(u, cx);
                    }
                }
                "remindcustom" | "remindcal" | "remindtime" => {
                    let last = app.selected_channel.and_then(|c| Some((c, app.messages.get(&c)?.msgs.last()?.id)));
                    if let Some((c, m)) = last {
                        app.open_custom_reminder(c, m, cx);
                        let which = match kind.as_str() {
                            "remindcal" => Some(0),
                            "remindtime" => Some(1),
                            _ => None,
                        };
                        phoenix_app::actions::with_modal(|m| {
                            if let Some(phoenix_app::ui::modals::Modal::Schedule(p)) = m {
                                p.open = which;
                            }
                        });
                    }
                }
                "follow" => {
                    let ch = arg.parse::<u64>().ok().map(phoenix_app::model::Id).or(app.selected_channel);
                    if let Some(ch) = ch {
                        phoenix_app::ui::modals::follow::open(ch, window);
                    }
                }
                "followok" => phoenix_app::ui::modals::follow::open_success(),
                "threadmore" => {
                    let id = arg.parse::<u64>().ok().map(phoenix_app::model::Id).or(app.selected_channel);
                    let w = f32::from(window.viewport_size().width);
                    let forum = id.and_then(|t| app.nav.thread_parent(t)).and_then(|p| app.channels.get(&p)).is_some_and(|p| p.kind == phoenix_app::model::ChannelKind::Forum);
                    let place = if forum { phoenix_app::actions::thread::Place::ForumToolbar } else { phoenix_app::actions::thread::Place::Toolbar };
                    if let Some(id) = id {
                        phoenix_app::ui::thread_menu::open_at(id, place, gpui::point(px(w - 24.), px(84.)), window);
                    }
                }
                "modview" => {
                    let me = app.me.id;
                    let g = app.selected_guild;
                    let user = arg.parse::<u64>().ok().map(phoenix_app::model::Id).or_else(|| {
                        g.and_then(|g| app.members.get(&g)).and_then(|ms| ms.iter().find(|m| m.user != me && !app.users.get(&m.user).is_some_and(|u| u.bot))).map(|m| m.user)
                    });
                    if let (Some(g), Some(u)) = (g, user) {
                        app.open_mod_view(g, u, cx);
                        let panel = match std::env::var("PHX_MODVIEW_PANEL").as_deref() {
                            Ok("perms") => Some(phoenix_app::ui::mod_view::Panel::Permissions),
                            Ok("audit") => Some(phoenix_app::ui::mod_view::Panel::AuditLog),
                            Ok("history") => Some(phoenix_app::ui::mod_view::Panel::MessageHistory),
                            _ => None,
                        };
                        if let Some(p) = panel {
                            phoenix_app::ui::mod_view::navigate(p);
                            if std::env::var_os("PHX_MODVIEW_SLIDE").is_none() {
                                phoenix_app::ui::mod_view::settle();
                            }
                        }
                        if let Some(page) = std::env::var("PHX_MODVIEW_PAGE").ok().and_then(|v| v.parse::<u32>().ok()) {
                            phoenix_app::ui::mod_view::go_history_page(app, page.saturating_sub(1), cx);
                        }
                    } else {
                        eprintln!("real_shots: modview needs a guild and a member");
                    }
                }
                "msgmenu" => {
                    let last = app.selected_channel.and_then(|c| Some((c, app.messages.get(&c)?.msgs.last()?.id)));
                    if let Some((c, m)) = last {
                        app.open_message_menu(c, m, gpui::point(px(700.), px(400.)), cx);
                    }
                }
                _ => {}
            })
        })?;
        pump(&mut cx, wait * 1000 / 3);
        if kind == "report"
            && let Ok(path) = std::env::var("PHX_REPORT_PATH")
        {
            pump(&mut cx, 1500);
            report_path(&path);
            cx.update_window(win.into(), |_, _, cx| v.update(cx, |_, cx| cx.notify()))?;
            pump(&mut cx, 800);
        }
        if kind == "pins"
            && let Ok(y) = arg.parse::<f32>()
        {
            cx.update(|_| phoenix_app::ui::chat::scrollbar::handle("pins-list").set_offset(gpui::point(px(0.), px(-y))));
            pump(&mut cx, 1500);
        }
        if kind == "emoji"
            && let Ok(row) = arg.parse::<usize>()
        {
            cx.update(|cx| {
                if let Some(op) = cx.try_global::<phoenix_app::ui::pickers::Pickers>().and_then(|p| p.open.as_ref()) {
                    op.list.scroll_to(gpui::ListOffset { item_ix: row, offset_in_item: px(0.) });
                }
            });
            pump(&mut cx, 1000);
        }
    }
    if std::env::var_os("PHX_DEBUG_GIF").is_some() {
        cx.update(|cx| {
            if let Some(h) = cx.try_global::<phoenix_app::ui::pickers::Pickers>().and_then(|p| p.gifs.home.as_ref()) {
                println!("gif trending_preview={:?}", h.trending_preview);
                println!("gif first category={:?}", h.categories.first().map(|c| c.preview.clone()));
            } else {
                println!("gif home not loaded");
            }
        });
    }
    let hc = std::env::var_os("PHX_HC").is_some();
    cx.update_window(win.into(), |_, _, cx| {
        v.update(cx, |app, cx| {
            app.prefs.synced.theme = theme;
            app.prefs.synced.compact = compact;
            app.prefs.synced.density = density;
            app.prefs.local.font_size = font_px;
            if hc {
                app.prefs.local.ui.insert("contrast".into(), 1);
            }
            for (id, v) in std::env::var("PHX_LOCAL").unwrap_or_default().split(',').filter_map(|kv| kv.split_once('=')) {
                if let Ok(v) = v.trim().parse::<u64>() {
                    app.prefs.local.ui.insert(id.trim().to_owned(), v);
                }
            }
            for (id, v) in std::env::var("PHX_PROTO").unwrap_or_default().split(',').filter_map(|kv| kv.split_once('=')) {
                if let Ok(v) = v.trim().parse::<u64>() {
                    app.prefs.synced.proto.insert(Box::leak(id.trim().to_owned().into_boxed_str()), v);
                }
            }
            cx.notify();
        })
    })?;
    pump(&mut cx, 300);
    if let Some(n) = std::env::var("PHX_TABS").ok().and_then(|n| n.parse::<i32>().ok()) {
        for _ in 0..n.unsigned_abs() {
            cx.update_window(win.into(), |_, window, cx| phoenix_app::ui::focus_ring::tab(n < 0, window, cx))?;
            pump(&mut cx, 100);
        }
    }
    if let Ok(spec) = std::env::var("PHX_VMSG") {
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                phoenix_app::ui::chat::voice_message::shot_state(app, &spec, cx);
                cx.notify();
            })
        })?;
        pump(&mut cx, 400);
    }
    if let Ok(mode) = std::env::var("PHX_UPDATE_MOCK") {
        phoenix_app::updater::set_mock_available(phoenix_app::updater::ReleaseInfo {
            tag: "v0.2.0-beta.2".into(),
            name: "Discord Phoenix v0.2.0-beta.2".into(),
            body: "## Nouveautés\n- Vérification automatique des mises à jour GitHub\n- Bouton vert dans la barre de titre et modale de notes de version\n- Vérification `SHA256SUMS.txt` avant installation\n\n## Corrections\n- Meilleure stabilité du rendu GPUI".into(),
            html_url: "https://github.com/RoxasYTB/project-phoenix/releases/tag/v0.2.0-beta.2".into(),
            prerelease: true,
            published_at: "2026-11-25T14:00:00Z".into(),
            asset: Some(phoenix_app::updater::ReleaseAsset {
                name: "discord-phoenix-0.2.0-beta.2-x86_64.AppImage".into(),
                download_url: "https://github.com/RoxasYTB/project-phoenix/releases/download/v0.2.0-beta.2/discord-phoenix-0.2.0-beta.2-x86_64.AppImage".into(),
                size: 28_450_816,
            }),
            sha256sums_url: Some("https://github.com/RoxasYTB/project-phoenix/releases/download/v0.2.0-beta.2/SHA256SUMS.txt".into()),
        });
        cx.update_window(win.into(), |_, window, cx| {
            if mode == "modal" {
                phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::Update, Some(window));
            }
            v.update(cx, |_, cx| cx.notify());
        })?;
        pump(&mut cx, 300);
    }
    if let Ok(q) = std::env::var("PHX_SETTINGS_SEARCH") {
        pump(&mut cx, 300);
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                if let Some(i) = app.settings.as_ref().and_then(|s| s.search.clone()) {
                    i.update(cx, |i, cx| i.set_value(q.clone(), window, cx));
                }
                cx.notify();
            })
        })?;
        pump(&mut cx, 300);
    }
    if let Ok(draft) = std::env::var("PHX_DRAFT") {
        let draft = draft.replace("\\n", "\n");
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                app.composer.update(cx, |s, cx| s.set_value(draft.clone(), window, cx));
                phoenix_app::ui::chat::rich_input::sync(app, cx);
                cx.notify();
            })
        })?;
        pump(&mut cx, 300);
    }
    if let Ok(edit) = std::env::var("PHX_EDIT") {
        let draft = edit.replace("\\n", "\n");
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                let me = app.me.id;
                let last = app.selected_channel.and_then(|ch| app.messages.get(&ch)).and_then(|e| e.msgs.iter().rev().find(|m| m.author == me).map(|m| m.id));
                if let Some(id) = last {
                    app.start_edit(id, window, cx);
                    if let Some(input) = app.chat.edit.as_ref().map(|e| e.input.clone()).filter(|_| !draft.is_empty()) {
                        input.update(cx, |s, cx| s.set_value(draft.clone(), window, cx));
                    }
                    phoenix_app::ui::chat::rich_input::sync(app, cx);
                    cx.notify();
                }
            })
        })?;
        pump(&mut cx, 300);
    }
    if let Some((x, y)) = std::env::var("PHX_MOUSE").ok().and_then(|v| {
        let (x, y) = v.split_once(',')?;
        Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))
    }) {
        let ev = gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: gpui::point(px(x), px(y)),
            pressed_button: None,
            modifiers: gpui::Modifiers::default(),
        });
        cx.update_window(win.into(), |_, window, cx| {
            window.dispatch_event(ev, cx);
        })?;
        pump(&mut cx, 200);
    }
    let shoot = |cx: &mut HeadlessAppContext, path: &str| -> anyhow::Result<()> {
        let mut img = None;
        for _ in 0..2 {
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            pump(cx, 200);
            cx.advance_clock(std::time::Duration::from_secs(2));
            img = Some(cx.capture_screenshot(win.into())?);
        }
        eprintln!("[{:.1}s] captured", t0.elapsed().as_secs_f32());
        let mut img = img.expect("frame");
        let (cw, ch) = (((w * scale).round() as u32).min(img.width()), ((h * scale).round() as u32).min(img.height()));
        if (cw, ch) != (img.width(), img.height()) {
            img = image::imageops::crop_imm(&img, 0, 0, cw, ch).to_image();
        }
        img.save(path)?;
        println!("wrote {path} ({}x{})", img.width(), img.height());
        Ok(())
    };
    let voice_test = std::env::var_os("PHX_VOICE").is_some();
    if voice_test {
        use phoenix_app::voice::VoiceControl as V;
        let deaf = std::env::var("PHX_VOICE_DEAF").as_deref() == Ok("1");
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                drop(app.backend.voice_control(V::SetInputDevice(None)));
                drop(app.backend.voice_control(V::SetMute(true)));
                drop(app.backend.voice_control(V::SetDeaf(deaf)));
                app.mic_muted = true;
                app.deafened = deaf;
                app.select_channel(Id(VOICE_TEST_CHANNEL), cx);
            })
        })?;
        let mut connected = false;
        for _ in 0..40 {
            pump(&mut cx, 500);
            connected = cx.update(|cx| v.read(cx).voice.status == phoenix_app::voice::VoiceStatus::Connected);
            eprintln!("voice-trace: ui status {:?}", cx.update(|cx| v.read(cx).voice.status));
            if connected {
                break;
            }
        }
        pump(&mut cx, 1500);
        cx.update(|cx| {
            let a = v.read(cx);
            let vs = &a.voice;
            println!(
                "voice status={:?} connected={connected} e2ee={} mode={:?} endpoint_set={} ping={:?} privacy_code={} sent={} received={} lost={} error={:?} channel={:?}",
                vs.status,
                vs.e2ee,
                vs.mode,
                vs.endpoint.is_some(),
                vs.ping_ms,
                vs.privacy_code.is_some(),
                vs.stats.packets_sent,
                vs.stats.packets_received,
                vs.stats.packets_lost,
                vs.last_error,
                a.voice_channel.map(|c| c.0)
            );
        });
        if std::env::var_os("PHX_VOICE_CAMERA").is_some() {
            cx.update_window(win.into(), |_, _, cx| {
                v.update(cx, |app, _| {
                    drop(app.backend.voice_control(V::SetCameraDevice(None)));
                    drop(app.backend.voice_control(V::SetCamera(true)));
                })
            })?;
            pump(&mut cx, 6000);
            cx.update(|cx| {
                let a = v.read(cx);
                let f = phoenix_app::ui::voice_panel::flags(a.me.id);
                println!(
                    "camera active={} self_video={} video_sent={} video_rtcp={}",
                    phoenix_app::voice::call::camera_active(),
                    f.video,
                    a.voice.stats.video_packets_sent,
                    a.voice.stats.video_rtcp_received
                );
            });
        }
        if let Ok(q) = std::env::var("PHX_VOICE_STREAM") {
            let mut it = q.split('p');
            let resolution = it.next().and_then(|r| r.parse().ok()).unwrap_or(720u32);
            let fps = it.next().and_then(|r| r.parse().ok()).unwrap_or(30u32);
            cx.update_window(win.into(), |_, _, cx| {
                v.update(cx, |app, _| {
                    drop(app.backend.voice_control(V::StartStream { source: phoenix_app::voice::StreamSource::TestPattern, resolution, fps, sound: std::env::var_os("PHX_VOICE_STREAM_SOUND").is_some() }))
                })
            })?;
            let hold: u64 = std::env::var("PHX_VOICE_STREAM_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(8000);
            let mut waited = 0;
            while waited < hold {
                pump(&mut cx, 1000);
                waited += 1000;
                let s = phoenix_app::voice::call::own_stream_stats();
                cx.update(|cx| {
                    let a = v.read(cx);
                    println!(
                        "stream t={waited} own={:?} capturing={} self_stream={} video_sent={:?} rtcp={:?} error={:?}",
                        a.voice.own_stream,
                        phoenix_app::voice::call::own_stream_active(),
                        phoenix_app::ui::voice_panel::flags(a.me.id).stream,
                        s.map(|s| s.video_packets_sent),
                        s.map(|s| s.video_rtcp_received),
                        a.voice.stream_error
                    );
                });
                if let Ok(flag) = std::env::var("PHX_VOICE_STREAM_UNTIL")
                    && std::path::Path::new(&flag).exists()
                {
                    break;
                }
            }
        }
        if let Some(key) = std::env::var("PHX_VOICE_WATCH").ok().filter(|k| !k.is_empty()) {
            cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, _| drop(app.backend.voice_control(V::WatchStream(key.clone())))))?;
            for t in 0..10 {
                pump(&mut cx, 1000);
                let owner = phoenix_app::voice::stream::parse_key(&key).map_or(0, |k| k.owner);
                println!("watch t={t} watching={} frame={:?}", phoenix_app::voice::call::watching(&key), phoenix_app::voice::call::stream_frame(owner).map(|f| (f.width, f.height, f.pts_us)));
            }
        }
        if std::env::var("PHX_VOICE_OPEN").as_deref() == Ok("soundboard") {
            cx.update_window(win.into(), |_, window, cx| v.update(cx, |app, cx| phoenix_app::ui::pickers::soundboard::toggle(app, window, cx)))?;
            pump(&mut cx, 4000);
            cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, cx| phoenix_app::ui::pickers::soundboard::rebuild(app, cx)))?;
            pump(&mut cx, 500);
        }
        if let Some(name) = std::env::var("PHX_VOICE_SOUND").ok().filter(|n| !n.is_empty()) {
            let found = cx.update_window(win.into(), |_, _, cx| {
                v.update(cx, |app, cx| {
                    let st = &cx.global::<phoenix_app::ui::pickers::Pickers>().soundboard;
                    let sound = st.sections.iter().flat_map(|s| s.sounds.iter()).find(|x| x.name.as_deref() == Some(name.as_str())).cloned();
                    match sound {
                        Some(sound) => {
                            phoenix_app::voice::soundboard::play_own(app, sound, cx);
                            true
                        }
                        None => false,
                    }
                })
            })?;
            println!("soundboard play {name:?} found={found}");
            if std::env::var_os("PHX_VOICE_CLOSE").is_some() {
                cx.update_window(win.into(), |_, window, cx| v.update(cx, |app, cx| phoenix_app::ui::pickers::soundboard::toggle(app, window, cx)))?;
            }
            let shots: Vec<u64> = std::env::var("PHX_VOICE_SOUND_SHOTS")
                .ok()
                .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                .filter(|v: &Vec<u64>| !v.is_empty())
                .unwrap_or_else(|| vec![700, 900, 1200]);
            for (i, ms) in shots.iter().enumerate() {
                pump(&mut cx, *ms);
                cx.update_window(win.into(), |_, window, _| window.refresh())?;
                pump(&mut cx, 100);
                let img = cx.capture_screenshot(win.into())?;
                let (cw, ch) = (((w * scale).round() as u32).min(img.width()), ((h * scale).round() as u32).min(img.height()));
                let img = image::imageops::crop_imm(&img, 0, 0, cw, ch).to_image();
                let path = format!("{}-play{i}.png", out.trim_end_matches(".png"));
                img.save(&path)?;
                println!("wrote {path} playing={}", phoenix_app::voice::soundboard::is_user_playing(cx.update(|cx| v.read(cx).me.id)));
            }
        }
    }
    if let Some(key) = std::env::var("PHX_STREAM_WATCH").ok().filter(|k| !k.is_empty()) {
        use phoenix_app::voice::VoiceControl as V;
        let owner = phoenix_app::voice::stream::parse_key(&key).map_or(0, |k| k.owner);
        cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, _| drop(app.backend.voice_control(V::WatchStream(key.clone())))))?;
        let secs: u64 = std::env::var("PHX_STREAM_WATCH_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(15);
        let mut saved = false;
        for t in 0..secs {
            pump(&mut cx, 1000);
            let f = phoenix_app::voice::call::stream_frame(owner);
            println!(
                "watch-only t={t} watching={} stats={:?} frame={:?}",
                phoenix_app::voice::call::watching(&key),
                phoenix_app::voice::call::watch_stats(&key),
                f.as_ref().map(|f| (f.width, f.height, f.pts_us))
            );
            if let Some(f) = f.filter(|f| f.pts_us > 30)
                && !saved
            {
                let rgba: Vec<u8> = f.data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
                if let Some(img) = image::RgbaImage::from_raw(f.width, f.height, rgba) {
                    let path = format!("{}-stream-frame.png", out.trim_end_matches(".png"));
                    img.save(&path)?;
                    println!("wrote {path}");
                    saved = true;
                }
            }
        }
        cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, _| drop(app.backend.voice_control(V::StopWatching(key.clone())))))?;
        pump(&mut cx, 800);
    }
    if let Some(what) = std::env::var("PHX_SEND").ok().filter(|w| !w.trim().is_empty()) {
        anyhow::ensure!(channel.is_some_and(|c| send_allowed(c.0)), "PHX_SEND only works in the test channels");
        let text = format!("{SEND_PREFIX}{}", what.trim());
        if let Ok(path) = std::env::var("PHX_SEND_FILE") {
            let bytes = std::fs::read(&path)?;
            let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, _| app.push_upload(name.clone(), bytes.clone())))?;
            println!("added to the tray: {name}");
            pump(&mut cx, 300);
            shoot(&mut cx, &format!("{}-tray.png", out.trim_end_matches(".png")))?;
            let wait: u64 = std::env::var("PHX_SEND_PRE_WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            pump(&mut cx, wait);
        }
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                app.composer.update(cx, |s, cx| s.set_value(text.clone(), window, cx));
                app.submit_composer(window, cx);
            })
        })?;
        println!("submitted: {text}");
        pump(&mut cx, 600);
        shoot(&mut cx, &format!("{}-pending.png", out.trim_end_matches(".png")))?;
        let hold: u64 = std::env::var("PHX_SEND_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
        let steps: u64 = std::env::var("PHX_SEND_SHOTS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        for i in 0..steps {
            pump(&mut cx, 250);
            shoot(&mut cx, &format!("{}-progress{i}.png", out.trim_end_matches(".png")))?;
        }
        pump(&mut cx, (hold + 3000).saturating_sub(steps * 250));
    }
    let shot = shoot(&mut cx, &out);
    if voice_test {
        cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, _| drop(app.backend.voice_control(phoenix_app::voice::VoiceControl::SetCamera(false)))))?;
        cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, _| drop(app.backend.voice_control(phoenix_app::voice::VoiceControl::StopStream))))?;
        pump(&mut cx, 500);
        cx.update_window(win.into(), |_, _, cx| v.update(cx, |app, cx| app.join_voice(None, cx)))?;
        pump(&mut cx, 2500);
        let status = cx.update(|cx| v.read(cx).voice.status);
        println!("voice left status={status:?}");
    }
    shot
}
