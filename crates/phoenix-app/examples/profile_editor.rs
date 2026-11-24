use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{
    app::PhoenixApp,
    assets::Assets,
    backend::{
        AppEvent, Backend, BackendError, BoxFuture, Credentials, EventSink, ReadyData, Upload,
    },
    discord_backend::{
        DiscordBackend,
        store::{KeyringStore, Secret, TokenStore},
    },
    mock::MockBackend,
    model::{Id, Message, Presence, Reaction},
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
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.0.select_guild(guild)
    }
    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.0.select_channel(channel)
    }
    fn load_messages(
        &self,
        channel: Id,
        before: Option<Id>,
        limit: u32,
    ) -> BoxFuture<Vec<Message>> {
        self.0.load_messages(channel, before, limit)
    }
    fn send_message(&self, _: Id, _: String, _: Option<Id>) -> BoxFuture<Message> {
        denied()
    }
    fn send_message_ex(
        &self,
        _: Id,
        _: String,
        _: Option<(Id, bool)>,
        _: Vec<Upload>,
        _: u64,
        _: Option<phoenix_app::backend::UploadProgress>,
    ) -> BoxFuture<Message> {
        denied()
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
    fn save_settings(
        &self,
        _: phoenix_app::ui::settings::SettingsPatch,
    ) -> BoxFuture<phoenix_app::ui::settings::SyncedSettings> {
        denied()
    }
    fn set_status(&self, _: Presence, _: Option<String>) -> BoxFuture<()> {
        denied()
    }
    fn logout(&self) -> BoxFuture<()> {
        denied()
    }
    fn notify_prefs(&self) -> Option<phoenix_app::notifications::NotifyPrefs> {
        self.0.notify_prefs()
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
    fn user_profile(&self, u: Id, g: Option<Id>) -> BoxFuture<phoenix_app::presence::Profile> {
        self.0.user_profile(u, g)
    }
    fn recent_activity(&self, u: Id) -> BoxFuture<Vec<phoenix_app::ui::settings::profile_activity::RecentGame>> {
        self.0.recent_activity(u)
    }
    fn owned_collectibles(&self) -> BoxFuture<Vec<phoenix_app::ui::settings::collectibles::Collectible>> {
        self.0.owned_collectibles()
    }
    fn collectibles_shop(&self) -> BoxFuture<phoenix_app::ui::settings::collectibles::ShopCatalog> {
        self.0.collectibles_shop()
    }
    fn collectible_product(&self, sku: u64) -> BoxFuture<Option<phoenix_app::ui::settings::collectibles::Collectible>> {
        self.0.collectible_product(sku)
    }
    fn profile_widgets(&self, u: Id) -> BoxFuture<Vec<phoenix_app::ui::settings::profile_widgets::AppWidget>> {
        self.0.profile_widgets(u)
    }
    fn subscribe_member_ranges(&self, g: Id, c: Id, r: Vec<[u32; 2]>) -> BoxFuture<()> {
        self.0.subscribe_member_ranges(g, c, r)
    }
    fn guild_boosts(&self, g: Id) -> Option<(u32, bool)> {
        self.0.guild_boosts(g)
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
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let login_only = false;
    let dirty = args.next().as_deref() == Some("dirty");
    let real = std::env::var_os("PHX_REAL").is_some();
    if let Ok(path) = std::env::var("PHX_GET") {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        let token = KeyringStore.load().map_err(anyhow::Error::msg)?.ok_or_else(|| anyhow::anyhow!("no token"))?;
        let rest = phoenix_api::RestClient::new(token.expose(), phoenix_api::ClientProperties::linux_desktop());
        let v = rt.block_on(rest.request_json(reqwest::Method::GET, &path, None))?;
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let tab: usize = std::env::var("PHX_TAB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let wait: u64 = std::env::var("PHX_WAIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    if real {
        phoenix_app::discord_backend::NO_ACKS.store(true, std::sync::atomic::Ordering::Relaxed);
    }

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
    let backend: Arc<dyn Backend> = if real {
        let b = DiscordBackend::with_store(Arc::new(LoadOnlyStore));
        let http = b.http_client();
        cx.update(|cx| {
            cx.set_http_client(Arc::new(phoenix_app::media::cache::CachingHttpClient::new(
                http,
            )))
        });
        Arc::new(ReadOnly(b))
    } else {
        Arc::new(MockBackend::new())
    };
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(1280.), px(700.)), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(backend, window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    let pump = |cx: &mut HeadlessAppContext, n: usize| {
        for _ in 0..n {
            std::thread::sleep(Duration::from_millis(50));
            cx.run_until_parked();
        }
    };
    pump(&mut cx, 2);
    if !login_only {
        let v = view.borrow().clone().expect("view");
        if real {
            let mut ready = false;
            for _ in 0..60 {
                pump(&mut cx, 10);
                ready = cx.update(|cx| !v.read(cx).channels.is_empty());
                if ready {
                    break;
                }
            }
            anyhow::ensure!(ready, "no READY after 30s (token missing from keyring?)");
        } else {
            cx.update_window(win.into(), |_, window, cx| {
                v.update(cx, |app, cx| {
                    app.login_with_token = true;
                    app.login_token
                        .update(cx, |s, cx| s.set_value("mock-token", window, cx));
                    app.submit_login(window, cx);
                });
            })?;
            pump(&mut cx, 30);
        }
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                phoenix_app::ui::settings::open(app, cx);
                if let Some(s) = app.settings.as_mut() {
                    s.page = phoenix_app::ui::settings::Page::Profiles;
                }
            });
        })?;
        pump(&mut cx, 10);
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                if let Some(e) = app.settings.as_mut().and_then(|s| s.profile.as_mut()) {
                    e.tab = tab;
                    e.switcher_open = std::env::var_os("PHX_SWITCHER").is_some();
                }
                use phoenix_app::ui::settings::collectibles::Kind;
                let kind = match std::env::var("PHX_PICKER").as_deref() {
                    Ok("decoration") => Some(Kind::Decoration),
                    Ok("effect") => Some(Kind::Effect),
                    Ok("frame") => Some(Kind::Frame),
                    Ok("nameplate") => Some(Kind::Nameplate),
                    _ => None,
                };
                if let Some(k) = kind {
                    phoenix_app::ui::settings::collectible_picker::open(app, k, cx);
                }
                cx.notify();
            });
        })?;
        pump(&mut cx, if real { wait as usize * 20 } else { 2 });
        if dirty {
            cx.update_window(win.into(), |_, window, cx| {
                v.update(cx, |app, cx| {
                    let i = app
                        .settings
                        .as_ref()
                        .and_then(|s| s.profile.as_ref())
                        .and_then(|e| e.pronouns.clone());
                    if let Some(i) = i {
                        i.update(cx, |s, cx| s.set_value("il/lui", window, cx));
                    }
                });
            })?;
            pump(&mut cx, 5);
        }
    }
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    cx.advance_clock(std::time::Duration::from_secs(2));
    let _ = cx.capture_screenshot(win.into())?;
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    cx.advance_clock(std::time::Duration::from_secs(2));
    let img = cx.capture_screenshot(win.into())?;
    img.save(&out)?;
    println!("wrote {out} ({}x{})", img.width(), img.height());
    Ok(())
}
