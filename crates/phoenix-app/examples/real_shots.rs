//! Offscreen render of Phoenix on the user's REAL account, strictly read-only.
//!
//! `cargo run -p phoenix-app --example real_shots -- out.png`
//!
//! The token comes from the keyring (never printed, never rewritten). The
//! backend is wrapped so nothing can be sent, acked, typed, reacted, joined or
//! changed: only READY, channel selection, message history, member list
//! subscription and media downloads go out.
//!
//! Location: env `PHX_GUILD` + `PHX_CHANNEL` (`PHX_GUILD` alone = its first text channel), or `PHX_DM=<channel id>`, or by
//! default the location shown by the user's Discord (`cdp.mjs eval location.pathname`).
//! Size / theme / density: env `PHX_W`, `PHX_H`, `PHX_THEME` (dark|light|ash|onyx), `PHX_HC`,
//! `PHX_DENSITY` (compact|default|cozy), `PHX_FONT` (px), otherwise read from the live Discord
//! through CDP. `PHX_COMPACT=1` = compact message display.
//! `PHX_AUDIO` = two digits mute/deaf (e.g. `11`), otherwise read from the live user panel.
//! `PHX_PROFILE=<user id>|peer` opens that profile (`PHX_PROFILE_KIND` popout|modal, `PHX_PROFILE_BOX=x,y`).
//! `PHX_VIEW=friends` opens the Friends page (`PHX_TAB` = online|all|pending|blocked|add);
//! `PHX_VIEW=inbox` opens the inbox popout over the location (`PHX_TAB` =
//! mentions|unreads|saved|reminders|scheduled).
//! `PHX_OPEN` = pins|threads|search[:q]|filters[:date|calendar|list<n>]|settings[:page]|server_settings[:n]|emoji|gif|sticker|msgmenu|chanmenu[:n]|catmenu[:n]|guildmenu|guilddrop|usermenu[:n]|switcher|leave|invite|post[:n]|events|community[:browse]|guide.
//! `PHX_PROFILE=member:<n>` = nth member of the list; `PHX_DEBUG_GIF=1` prints the GIF home previews.
//! `PHX_FX_STILL=1` = still profile effects (reduce motion).
//! `PHX_SEARCH_SORT=old|rel` and `PHX_SEARCH_PAGE=<n>` with `PHX_OPEN=search:<q>`.
//! `PHX_WAIT` = seconds to wait for messages and images (default 15).
//! `PHX_TABS=n` presses Tab n times before the capture (`-n` = Shift+Tab), for the focus rings.
//! `PHX_SEND=<what>` (send-lifecycle test, ONLY in the hard-coded test channel
//! [`SEND_TEST_CHANNEL`], refused anywhere else): types
//! "[Test Phoenix] envoyé depuis le client Rust (Phoenix) — <what>" and presses Enter.
//! The POST is held `PHX_SEND_HOLD` ms (default 4000) so the pending copy can be shot to
//! `<out>-pending.png`; the settled state goes to `<out>`. `PHX_SEND_FAIL=400` answers
//! with a local 400 instead of sending (nothing leaves the machine).

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

/// Keyring store that can load but never save or delete.
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

/// The only channel `PHX_SEND` may post to (user-granted test channel).
const SEND_TEST_CHANNEL: u64 = 1403447573389381652;
/// Every test message starts with this, so readers know where it came from.
const SEND_PREFIX: &str = "[Test Phoenix] envoyé depuis le client Rust (Phoenix) — ";

/// Delegates reads to the real backend; every write is refused (except the guarded test send).
struct ReadOnly(DiscordBackend);

impl Backend for ReadOnly {
    fn set_event_sink(&self, sink: EventSink) {
        // Drop events that would make the app answer with a write (none today, but keep the hook).
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
        self.0.prefetch_messages(channel) // GET only, never acks
    }
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.0.select_guild(guild)
    }
    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.0.select_channel(channel) // acks disabled via NO_ACKS
    }
    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.0.load_messages(channel, before, limit)
    }
    /// Server Guide resource previews: GET messages `after` the channel id (read-only).
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
        // Hard guard: PHX_SEND set, the test channel, our prefix, no reply. Anything else is refused.
        if std::env::var_os("PHX_SEND").is_none() || ch.0 != SEND_TEST_CHANNEL || !content.starts_with(SEND_PREFIX) || reply.is_some() {
            return denied();
        }
        let hold = std::env::var("PHX_SEND_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(4000u64);
        let fail = std::env::var("PHX_SEND_FAIL").ok();
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(hold));
            let _ = tx.send(());
        });
        // The real backend starts its request as soon as it is called: call it only after the hold.
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
    fn edit_message(&self, _: Id, _: Id, _: String) -> BoxFuture<Message> {
        denied()
    }
    fn delete_message(&self, _: Id, _: Id) -> BoxFuture<()> {
        denied()
    }
    fn trigger_typing(&self, _: Id) -> BoxFuture<()> {
        denied()
    }
    fn join_voice(&self, _: Option<Id>) -> BoxFuture<()> {
        denied()
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
    fn logout(&self) -> BoxFuture<()> {
        denied()
    }
    // Reads.
    fn notify_prefs(&self) -> Option<phoenix_app::notifications::NotifyPrefs> {
        self.0.notify_prefs()
    }
    fn member_can_view(&self, c: Id, u: Id) -> bool {
        self.0.member_can_view(c, u)
    }
    fn activity_feed_enabled(&self, g: Id) -> bool {
        self.0.activity_feed_enabled(g)
    }
    /// Gateway op 8 (read-only member lookup) for Activity feed authors.
    fn request_feed_members(&self, g: Id, u: Vec<Id>) {
        self.0.request_feed_members(g, u)
    }
    fn is_media_channel(&self, c: Id) -> bool {
        self.0.is_media_channel(c)
    }
    fn load_settings(&self) -> BoxFuture<phoenix_app::ui::settings::SyncedSettings> {
        self.0.load_settings()
    }
    fn guild_emojis(&self, g: Id) -> Vec<phoenix_app::ui::pickers::types::GuildEmoji> {
        self.0.guild_emojis(g)
    }
    fn has_premium(&self) -> bool {
        self.0.has_premium()
    }
    /// Read-only `GET /users/{id}/profile` (DM profile panel, popouts).
    fn user_profile(&self, u: Id, g: Option<Id>) -> BoxFuture<phoenix_app::presence::Profile> {
        self.0.user_profile(u, g)
    }
    /// Settings > Connections authorized apps (read-only GET).
    fn authorized_apps(&self) -> BoxFuture<Option<Vec<phoenix_app::ui::settings::authorized_apps::AuthorizedApp>>> {
        self.0.authorized_apps()
    }
    /// Settings > Billing (read-only GETs).
    fn billing(&self) -> BoxFuture<Option<phoenix_app::ui::settings::billing::Billing>> {
        self.0.billing()
    }
    /// Settings > Account devices count (read-only GET).
    fn session_count(&self) -> BoxFuture<Option<usize>> {
        self.0.session_count()
    }
    /// Profile Board: widgets, games and recent activity (read-only GETs).
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
    /// GET thread-members: read-only.
    fn thread_members(&self, t: Id) -> BoxFuture<Vec<phoenix_app::model::Member>> {
        self.0.thread_members(t)
    }
    fn guild_boosts(&self, g: Id) -> Option<(u32, bool)> {
        self.0.guild_boosts(g)
    }
    // Server Guide header art (READY cache only).
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
    // Browse Channels tab (PHX_OPEN=community:browse): READY settings and cache only.
    fn channel_opted_in(&self, c: Id) -> Option<bool> {
        self.0.channel_opted_in(c)
    }
    fn is_thread_joined(&self, c: Id) -> bool {
        self.0.is_thread_joined(c)
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
    fn last_read(&self, c: Id) -> Option<Id> {
        self.0.last_read(c)
    }
    fn forum_tags(&self, f: Id) -> Vec<phoenix_app::commands::ForumTag> {
        self.0.forum_tags(f)
    }
    /// Only the forum post listing and the content inventory (read-only GETs) go through.
    // GET collectibles product by SKU (profile effects in the DM panel / popout).
    fn collectible_product(&self, sku: u64) -> BoxFuture<Option<phoenix_app::ui::settings::collectibles::Collectible>> {
        self.0.collectible_product(sku)
    }
    // Inbox "Reminders" tab: read-only list of the saved-message reminders.
    fn action(&self, req: phoenix_app::actions::ActionRequest) -> BoxFuture<phoenix_app::actions::ActionReply> {
        match req {
            phoenix_app::actions::ActionRequest::Reminders => self.0.action(req),
            _ => denied(),
        }
    }
    fn parity(&self, req: phoenix_app::commands::ParityRequest) -> BoxFuture<phoenix_app::commands::ParityReply> {
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
            // GET /content-inventory/users/@me (member list Activity feed).
            phoenix_app::commands::ParityRequest::ContentInventory => self.0.parity(req),
            // Events modal (PHX_OPEN=events): GET scheduled-events.
            phoenix_app::commands::ParityRequest::ScheduledEvents { .. } => self.0.parity(req),
            // Channels & Roles page (PHX_OPEN=community): GET onboarding.
            phoenix_app::commands::ParityRequest::Onboarding { .. } => self.0.parity(req),
            // Server Guide (PHX_OPEN=guide): GET new-member-welcome + GET profile.
            // PHX_DEBUG_GUIDE=1 prints what came back (titles and lengths only).
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
            // Thread / forum post opened by id (PHX_CHANNEL of a post).
            phoenix_app::commands::ParityRequest::FetchThread { .. } => self.0.parity(req),
            // Inbox popout tabs (PHX_VIEW=inbox): GET mentions / saved / scheduled.
            phoenix_app::commands::ParityRequest::Mentions { .. }
            | phoenix_app::commands::ParityRequest::SavedMessages
            | phoenix_app::commands::ParityRequest::ScheduledMessages => self.0.parity(req),
            _ => denied(),
        }
    }
    // Pins, expression pickers and search (PHX_OPEN): read-only GETs.
    /// Inbox > Unreads: computed from the cached read states, no request.
    fn inbox_unreads(&self, order: &[Id]) -> Vec<phoenix_app::ui::inbox_unreads::Entry> {
        self.0.inbox_unreads(order)
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
    // nav / other parity / open_dm / notes / stickers / voice_control keep the
    // trait defaults (offline errors or no-ops): they may write.
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

/// Live scroll state of the user's Discord (read-only `scrollTop` reads).
#[derive(Debug, Default)]
struct Scroll {
    /// Channel list (`sidebarList_` scroller) scrollTop, CSS px.
    channels: f32,
    /// First channel-list row (category / channel / thread id) still in view,
    /// and how far the scroller's top edge sits inside its `li`.
    top_channel: Option<(Id, f32)>,
    /// Member list (`membersWrap_` scroller) scrollTop, CSS px.
    members: f32,
    /// First message whose bottom is below the top of the message scroller,
    /// and how far (CSS px) the scroller's top edge sits inside it.
    top_message: Option<(Id, f32)>,
    /// Message scroller distance from the bottom (fallback when the anchor
    /// message is not in our loaded history).
    from_bottom: f32,
    /// Server rail (`scroller_5dqHTG`) scrollTop, CSS px.
    rail: f32,
}

/// Activity cards shown by Discord: "author id ¦ game" records joined by ¶ (kept verbatim in the JSON output).
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
    // `PHX_SCROLL`: the SCROLL_JS result taken from a dump, to replay it without CDP.
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

/// Scrolls our three lists to the Discord offsets. Pixel offsets are replayed
/// in viewport-sized steps with a frame in between, because `ListState`
/// only knows the heights of items it has laid out.
fn apply_scroll(
    cx: &mut HeadlessAppContext,
    win: gpui::AnyWindowHandle,
    v: &Entity<PhoenixApp>,
    sc: &Scroll,
    pump: &dyn Fn(&mut HeadlessAppContext, u64),
) -> anyhow::Result<()> {
    use gpui::ListOffset;
    let top = ListOffset { item_ix: 0, offset_in_item: px(0.) };
    // Message list: anchor on the same message, else same distance from the bottom.
    // When Discord sits at the bottom, stick to the bottom too: a top anchor would
    // carry every height difference of the messages above into the visible rows.
    let at_bottom = sc.from_bottom < 2.;
    let anchor = cx.update(|cx| {
        if at_bottom {
            return None;
        }
        let app = v.read(cx);
        let (id, off) = sc.top_message?;
        let ix = app.messages.get(&app.selected_channel?)?.index_of(id)?;
        Some((ix + 1, off)) // item 0 is the channel header / loader
    });
    // Channel list: anchor on the same row (item 0 is the guild banner / top spacer).
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
                    // Messages scroll up from the bottom, the other lists down from the top.
                    l.scroll_by(px(if i == 2 { -step } else { step }));
                    left[i] -= step;
                }
            }
        });
    }
    Ok(())
}

/// Window height while READY and the history load (see `open_window`).
const LOAD_H: f32 = 800.;

fn parse_id(s: &str) -> Option<Id> {
    s.parse().ok().map(Id)
}

fn main() -> anyhow::Result<()> {
    phoenix_app::perf::start();
    let out = std::env::args().nth(1).unwrap_or_else(|| "real.png".into());
    phoenix_app::discord_backend::NO_ACKS.store(true, std::sync::atomic::Ordering::Relaxed);
    // The desktop client draws its own window buttons; so does Phoenix with client decorations.
    phoenix_app::ui::title_bar::FORCE_CLIENT_DECORATIONS.store(true, std::sync::atomic::Ordering::Relaxed);

    // Where: explicit env, else what the user's Discord shows.
    let (guild, channel) = if let Some(dm) = std::env::var("PHX_DM").ok().and_then(|v| parse_id(&v)) {
        (None, Some(dm))
    } else if let Some(c) = std::env::var("PHX_CHANNEL").ok().and_then(|v| parse_id(&v)) {
        (std::env::var("PHX_GUILD").ok().and_then(|v| parse_id(&v)), Some(c))
    } else if let Some(g) = std::env::var("PHX_GUILD").ok().and_then(|v| parse_id(&v)) {
        // Guild only: its first text channel is picked once READY is in.
        (Some(g), None)
    } else {
        let path = cdp("location.pathname").unwrap_or_default();
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["channels", "@me", c, ..] => (None, parse_id(c)),
            ["channels", g, c, ..] => (parse_id(g), parse_id(c)),
            _ => (None, None),
        }
    };
    // Same device scale as the user's Discord (`devicePixelRatio`, 1.5 here) so
    // subpixel borders and rounding snap the same way. PHX_SCALE overrides.
    let scale: f32 = env_or_cdp("PHX_SCALE", "devicePixelRatio").and_then(|v| v.parse().ok()).filter(|s: &f32| *s > 0.).unwrap_or(2.);
    let w: f32 = env_or_cdp("PHX_W", "innerWidth").and_then(|v| v.parse().ok()).unwrap_or(1280.);
    let h: f32 = env_or_cdp("PHX_H", "innerHeight").and_then(|v| v.parse().ok()).unwrap_or(800.);
    let classes = cdp("document.documentElement.className").unwrap_or_default();
    let theme = match std::env::var("PHX_THEME").ok().as_deref() {
        Some("light") => AppTheme::Light,
        Some("ash") => AppTheme::Ash,
        Some("onyx") => AppTheme::Onyx,
        Some(_) => AppTheme::Dark,
        // The client's Dark is `theme-dark theme-darker`; a bare `theme-dark` is Ash.
        None if classes.contains("theme-midnight") => AppTheme::Onyx,
        None if classes.contains("theme-darker") => AppTheme::Dark,
        None if classes.contains("theme-light") => AppTheme::Light,
        None if classes.contains("theme-dark") => AppTheme::Ash,
        None => AppTheme::Dark,
    };
    // Compact *message display* is a React layout, not an <html> class: only PHX_COMPACT turns it on.
    // UI density and font size are <html> classes (`density-*`, `font-size-N`); PHX_DENSITY
    // (compact|default|cozy) and PHX_FONT (12..24) override them.
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
    // Self mute / deafen are client-side state in Discord (not on the account):
    // read the user panel switches (`audioButtonWithMenu_` role=switch, aria-checked).
    let audio = env_or_cdp(
        "PHX_AUDIO",
        "[...document.querySelectorAll('button[role=switch][class*=audioButtonWithMenu]')].map(b=>b.getAttribute('aria-checked')==='true'?1:0).join('')",
    )
    .unwrap_or_default();
    let (self_mute, self_deaf) = (audio.starts_with('1'), audio.get(1..2) == Some("1"));
    // Channel list width the user dragged to (sidebarList_ box), so compares line up.
    if std::env::var_os("PHX_SIDEBAR_W").is_none()
        && let Some(sw) = cdp("(document.querySelector('[class*=sidebarList_]')?.getBoundingClientRect().width||'')+''")
        && sw.parse::<f32>().is_ok()
    {
        // SAFETY: single-threaded, before any other thread starts.
        unsafe { std::env::set_var("PHX_SIDEBAR_W", sw) };
    }
    let scroll = read_scroll();
    println!("scroll {scroll:?}");
    // Open guild folders are client-side state in Discord: read the rail's
    // `aria-expanded` folder buttons (`data-list-item-id=guildsnav___<folder id>`).
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
        // `PHX_FX_STILL=1`: profile effects (and other animations) show their still
        // frame, so a compare is not decided by the effect's animation phase.
        if std::env::var_os("PHX_FX_STILL").is_some() {
            cx.set_reduce_motion(true);
        }
    });
    let backend: Arc<dyn Backend> = Arc::new(ReadOnly(backend));
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    // Load small, capture big: in headless (test) mode gpui draws the window on
    // every effect flush, i.e. once per gateway event. READY and the presence
    // storm that follows meant thousands of full-size draws, so tall captures
    // (PHX_H=1600+) took minutes. The real size is applied after READY.
    let win = cx.open_window(size(px(w), px(h.min(LOAD_H))), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(backend, window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    cx.update_window(win.into(), |_, window, _| window.set_scale_factor(scale))?;
    // Time-based, not iteration-based: at tall sizes one parked run can draw a
    // large frame (animated avatars/GIFs keep requesting frames), so a fixed
    // `ms / 50` iteration count stretched a 15 s wait to minutes.
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
    // Wait for READY (restore_session from the keyring).
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
    // `PHX_LIST=1`: print DM / group DM ids and the open threads of the
    // current guild (for picking capture targets), then exit.
    if std::env::var_os("PHX_LIST").is_some() {
        cx.update(|cx| {
            let app = v.read(cx);
            for d in &app.dms {
                // A 1:1 DM is named after its recipient; anything else is a group.
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
            // Every guild's active threads (READY / GUILD_CREATE), not just the
            // sidebar rows of the selected guild.
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
            // In-memory only: the read-only capture never writes voice.json.
            app.mic_muted = self_mute || self_deaf;
            app.deafened = self_deaf;
            app.nav.expanded_folders.clear();
            app.nav.expanded_folders.extend(open_folders.iter().copied());
            // Discord's own voice connection (client-side): mirror it in memory only, so the
            // connected row / voice panel match. Never joins anything.
            if in_voice {
                app.voice_channel = app
                    .channels
                    .values()
                    .find(|c| c.guild.is_some() && c.voice_users.contains(&app.me.id))
                    .map(|c| c.id);
            }
            // Discord on the Friends page (`/channels/@me`): render it too.
            let view = std::env::var("PHX_VIEW").unwrap_or_else(|_| {
                let home = guild.is_none() && channel.is_none() && cdp("location.pathname").as_deref() == Some("/channels/@me");
                if home { "friends".into() } else { String::new() }
            });
            let tab = std::env::var("PHX_TAB").unwrap_or_default();
            if view == "friends" {
                use phoenix_app::ui::home::data::FriendsTab as F;
                // Like open_home_view(Friends), without saving the nav prefs.
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
            } else if let (Some(g), None) = (guild, channel) {
                app.select_guild(Some(g), cx);
                let first = app.guild_channels.get(&g).and_then(|l| {
                    l.iter().copied().find(|id| app.channels.get(id).is_some_and(|c| c.kind == phoenix_app::model::ChannelKind::Text))
                });
                if let Some(c) = first {
                    app.open_channel_chat(c, cx);
                }
            } else if let Some(c) = channel {
                // Threads live in `nav.threads` until selected (select_thread registers them).
                let thread = app.nav.find_thread(c).cloned();
                let g = guild.or_else(|| app.channels.get(&c).or(thread.as_ref()).and_then(|ch| ch.guild));
                app.select_guild(g, cx);
                // Voice channels: open their text chat only (never joins voice).
                if thread.is_some() && !app.channels.contains_key(&c) {
                    app.select_thread(c, cx);
                } else if !app.channels.contains_key(&c) {
                    // Forum posts / archived threads: GET /channels/{id}.
                    phoenix_app::ui::forum::open_by_id(app, c, cx);
                } else {
                    app.open_channel_chat(c, cx);
                }
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
    // Full size for the rest: images in view start loading now.
    cx.update_window(win.into(), |_, window, cx| {
        window.resize(size(px(w), px(h)));
        window.bounds_changed(cx);
        window.set_scale_factor(scale);
    })?;
    pump(&mut cx, wait * 1000 / 2);
    // Server nicks of authors outside the synced member-list range come from op 8
    // (GUILD_MEMBERS_CHUNK): wait until every author of the page has a guild member
    // (members who left the guild never get one, hence the timeout).
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
    // Same Activity cards as the Discord capture: the client fetched its feed at another
    // time, so put the entries it shows (read-only DOM read: author id from the avatar
    // URL + game) first, in its order. PHX_NO_FEED_SYNC=1 turns this off.
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
    // `PHX_DEBUG_UNREADS=1`: what Inbox > Unreads holds (channel ids, counts), on stderr.
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
    // Same rows as the user's Discord: replay its scroll offsets on our lists.
    if let Some(sc) = scroll {
        apply_scroll(&mut cx, win.into(), &v, &sc, &pump)?;
        eprintln!("[{:.1}s] scrolled", t0.elapsed().as_secs_f32());
    }
    // PHX_PROFILE=<user id> opens that user's profile over the location, like the
    // overlay open in Discord: PHX_PROFILE_KIND=popout (default; PHX_PROFILE_BOX=x,y
    // is the popout's top-left in Discord, CSS px) or modal. Read-only GET of the profile.
    // PHX_NO_FX=1: no profile effects (compare.py --no-effects hides them in Discord too).
    if std::env::var_os("PHX_NO_FX").is_some() {
        phoenix_app::ui::profile_fx::DISABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // `PHX_PROFILE=peer`: the other user of the open DM.
    let peer = cx.update(|cx| phoenix_app::ui::dm_profile_panel::peer(v.read(cx)));
    // `PHX_PROFILE=member:<n>`: the nth member of the guild's member list.
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
                // open_profile places the popout at (at.x - 316, at.y - 60).
                let (x, y) = at.unwrap_or((900., 200.));
                app.open_profile(uid, g, gpui::point(px(x + 316.), px(y + 60.)), window, cx);
                if modal {
                    app.open_profile_modal(uid, cx);
                }
            })
        })?;
        pump(&mut cx, wait * 1000 / 3);
    }
    // `PHX_OPEN`: an overlay over the location (read-only: nothing is sent).
    if let Ok(what) = std::env::var("PHX_OPEN") {
        use phoenix_app::ui::settings::Page as P;
        let (kind, arg) = what.split_once(':').unwrap_or((what.as_str(), ""));
        let (kind, arg) = (kind.to_string(), arg.to_string());
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| match kind.as_str() {
                "pins" => app.toggle_pins(cx),
                "threads" => app.toggle_threads_popout(cx),
                "search" => {
                    phoenix_app::ui::search::open(app, window, cx);
                    // The query also goes in the header bar, like the client after Enter.
                    phoenix_app::ui::search::set_query(if arg.is_empty() { "a" } else { &arg }, window, cx);
                    if let Ok(mut req) = phoenix_app::ui::search::build_request(app, if arg.is_empty() { "a" } else { &arg }) {
                        // `PHX_SEARCH_SORT=old|rel`, `PHX_SEARCH_PAGE=<n>` (1-based).
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
                    // Search "More filters" modal; `:date` adds a date row, `:calendar` opens its picker.
                    phoenix_app::ui::search::open(app, window, cx);
                    phoenix_app::ui::search_filters::open(app, window, cx);
                    if arg == "date" || arg == "calendar" {
                        phoenix_app::ui::search_filters::add_date(cx, arg == "calendar");
                    }
                    // `:list<n>` opens the option list of field n (0 De .. 5 Épinglé).
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
                }
                "server_settings" => {
                    if let Some(g) = app.selected_guild {
                        app.open_server_settings(g, arg.parse().unwrap_or(0), cx);
                    }
                }
                "emoji" | "gif" | "sticker" => {
                    use phoenix_app::ui::pickers::{EmojiTarget, Tab};
                    let tab = match kind.as_str() {
                        "gif" => Tab::Gif,
                        "sticker" => Tab::Sticker,
                        _ => Tab::Emoji,
                    };
                    phoenix_app::ui::pickers::open(app, tab, EmojiTarget::Composer, window, cx);
                    // `emoji:<row>`: scrolled to that grid row (later categories).
                    if let Ok(row) = arg.parse::<usize>()
                        && let Some(op) = cx.try_global::<phoenix_app::ui::pickers::Pickers>().and_then(|p| p.open.as_ref())
                    {
                        op.list.scroll_to(gpui::ListOffset { item_ix: row, offset_in_item: px(0.) });
                    }
                }
                // Context menus at (700, 300): channel / category (nth in the sidebar), server.
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
                // nth member of the guild's member list: right-click menu.
                "usermenu" => {
                    let u = app.selected_guild.and_then(|g| app.members.get(&g)?.get(arg.parse().unwrap_or(0)).map(|m| m.user));
                    if let Some(u) = u {
                        phoenix_app::ui::user_menu::open(u, app.selected_guild, gpui::point(px(900.), px(250.)), window);
                    }
                }
                "switcher" => phoenix_app::ui::switcher::open(app, window, cx),
                // Guild / channel / message dialogs, open only (nothing is saved).
                "chansettings" | "createchannel" | "privacy" | "serverprofile" | "security" | "createevent" | "nickname" => {
                    if let Some(g) = app.selected_guild {
                        match kind.as_str() {
                            "chansettings" => {
                                if let Some(c) = app.selected_channel {
                                    app.open_channel_settings(c, cx)
                                }
                            }
                            "createchannel" => app.open_create_channel(g, None, phoenix_app::model::ChannelKind::Text, cx),
                            "privacy" => app.open_privacy_settings(g, cx),
                            "serverprofile" => app.open_server_profile(g, cx),
                            "security" => app.open_security_actions(g, cx),
                            "createevent" => app.open_create_event(g, cx),
                            _ => app.open_nickname(g, None, cx),
                        }
                    }
                }
                // Reactions / forward dialogs on the last message with reactions (or the last one).
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
                // Server notification settings modal (open only).
                "guildnotify" => {
                    if let Some(g) = app.selected_guild {
                        let st = phoenix_app::ui::modals::guild_notify::State::new(app, g);
                        phoenix_app::actions::open_modal(phoenix_app::ui::modals::Modal::GuildNotify(st), None);
                    }
                }
                // User panel avatar popout (`statuspicker:sub` with the status submenu).
                "statuspicker" => {
                    app.toggle_status_picker(window, cx);
                    phoenix_app::ui::status_picker::debug_open_status_submenu(arg == "sub");
                }
                "leave" => {
                    if let Some(g) = app.selected_guild {
                        app.show_overlay(phoenix_app::ui::home::data::Overlay::LeaveGuild(g), cx);
                    }
                }
                // Invite dialog without creating an invite (read-only).
                "invite" => {
                    if let Some(guild) = app.selected_guild {
                        app.show_overlay(phoenix_app::ui::home::data::Overlay::InviteLink { guild, link: None }, cx);
                    }
                }
                // `community:browse` opens the Browse Channels tab.
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
                "events" => {
                    if let Some(g) = app.selected_guild {
                        phoenix_app::ui::events::open(app, g, cx);
                    }
                }
                // Forum post n (0 = first, active then older) in the thread sidebar.
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
        // Scroll the emoji grid once it has laid out (opening resets the list).
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
    // The account settings (`settings::on_ready`) can land after the setup above and bring
    // the account's own theme back: re-apply the requested look right before capturing.
    // PHX_HC=1 turns on High contrast (in memory only, like every pref here).
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
            cx.notify();
        })
    })?;
    pump(&mut cx, 300);
    // PHX_TABS=n (or -n for Shift+Tab): press Tab n times before the capture (focus rings).
    if let Some(n) = std::env::var("PHX_TABS").ok().and_then(|n| n.parse::<i32>().ok()) {
        for _ in 0..n.unsigned_abs() {
            cx.update_window(win.into(), |_, window, cx| phoenix_app::ui::focus_ring::tab(n < 0, window, cx))?;
            pump(&mut cx, 100);
        }
    }
    let shoot = |cx: &mut HeadlessAppContext, path: &str| -> anyhow::Result<()> {
        let mut img = None;
        for _ in 0..2 {
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            pump(cx, 200);
            // Headless time is virtual: advance it so enter animations (modals, popouts) finish.
            cx.advance_clock(std::time::Duration::from_secs(2));
            img = Some(cx.capture_screenshot(win.into())?);
        }
        eprintln!("[{:.1}s] captured", t0.elapsed().as_secs_f32());
        // The test window's framebuffer is always sized at its 2x default; the scene
        // is drawn at `scale`, so keep only the top-left w*scale x h*scale.
        let mut img = img.expect("frame");
        let (cw, ch) = (((w * scale).round() as u32).min(img.width()), ((h * scale).round() as u32).min(img.height()));
        if (cw, ch) != (img.width(), img.height()) {
            img = image::imageops::crop_imm(&img, 0, 0, cw, ch).to_image();
        }
        img.save(path)?;
        println!("wrote {path} ({}x{})", img.width(), img.height());
        Ok(())
    };
    if let Some(what) = std::env::var("PHX_SEND").ok().filter(|w| !w.trim().is_empty()) {
        anyhow::ensure!(channel.map(|c| c.0) == Some(SEND_TEST_CHANNEL), "PHX_SEND only works in the test channel");
        let text = format!("{SEND_PREFIX}{}", what.trim());
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
        pump(&mut cx, hold + 3000);
    }
    shoot(&mut cx, &out)
}
