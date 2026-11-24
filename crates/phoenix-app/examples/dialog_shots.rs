//! Offscreen renders of dialogs and settings pages at the live client's
//! 1280x700 viewport. `cargo run -p phoenix-app --example dialog_shots -- out.png <scene>`
//! scene: settings:<page> | leave | delete | status

use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let scene = args.next().unwrap_or_else(|| "settings:account".into());
    let login_only = false;

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), || {
        gpui_platform::current_headless_renderer()
    });
    // The headless text system doesn't scan system fonts: load them explicitly.
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
    // PHOENIX_SHOT_NET=1 loads remote art (Discord media hosts only) like the app.
    let _rt = std::env::var_os("PHOENIX_SHOT_NET").map(|_| tokio::runtime::Runtime::new().expect("runtime"));
    if let Some(rt) = &_rt {
        let http = phoenix_app::discord_backend::http::CdnHttpClient::new(rt.handle().clone(), "Mozilla/5.0");
        cx.update(|cx| cx.set_http_client(std::sync::Arc::new(phoenix_app::media::cache::CachingHttpClient::new(std::sync::Arc::new(http)))));
    }
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    // PHOENIX_SHOT_W/H match other dump viewports (vanilla dumps are 1920x1050).
    let env = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let win = cx.open_window(size(px(env("PHOENIX_SHOT_W", 1280.)), px(env("PHOENIX_SHOT_H", 700.))), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(Arc::new(MockBackend::new()), window, cx));
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
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                app.login_with_token = true;
                app.login_token.update(cx, |s, cx| s.set_value("mock-token", window, cx));
                app.submit_login(window, cx);
            });
        })?;
        pump(&mut cx, 30);
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                use phoenix_app::ui::settings::Page;
                // PHOENIX_SHOT_THEME=onyx matches the user's midnight dumps.
                if let Ok(t) = std::env::var("PHOENIX_SHOT_THEME") {
                    use phoenix_app::ui::settings::AppTheme;
                    app.prefs.synced.theme = match t.as_str() {
                        "light" => AppTheme::Light,
                        "ash" => AppTheme::Ash,
                        "onyx" => AppTheme::Onyx,
                        _ => AppTheme::Dark,
                    };
                }
                if let Some(p) = scene.strip_prefix("settings:") {
                    phoenix_app::ui::settings::open(app, cx);
                    let page = match p.split(':').next().unwrap_or("") {
                        "profiles" => Page::Profiles,
                        "privacy" | "content" => Page::PrivacySafety,
                        "data" => Page::DataPrivacy,
                        "system" => Page::System,
                        "activity" => Page::ActivityPrivacy,
                        "devices" => Page::Devices,
                        "connections" => Page::Connections,
                        "notifications" => Page::Notifications,
                        "keybinds" => Page::Keybinds,
                        "language" => Page::Language,
                        "appearance" => Page::Appearance,
                        "accessibility" => Page::Accessibility,
                        "voice" => Page::VoiceVideo,
                        "advanced" => Page::Advanced,
                        "games" => Page::RegisteredGames,
                        "nitro" => Page::Nitro,
                        "billing" => Page::Billing,
                        "subscriptions" => Page::Subscriptions,
                        _ => Page::MyAccount,
                    };
                    if let Some(s) = app.settings.as_mut() {
                        s.page = page;
                        s.confirm_logout = p == "logout";
                    }
                } else if scene == "leave" {
                    if let Some(g) = app.guilds.first().map(|g| g.id) {
                        app.show_overlay(phoenix_app::ui::home::data::Overlay::LeaveGuild(g), cx);
                    }
                } else if scene == "delete" {
                    let pick = app.messages.iter().find_map(|(ch, m)| m.msgs.last().map(|x| (*ch, x.id)));
                    if let Some((ch, id)) = pick {
                        app.select_channel(ch, cx);
                        app.chat.confirm_delete = Some((ch, id));
                    }
                } else if scene == "status" {
                    app.toggle_status_picker(window, cx);
                }
            });
        })?;
        pump(&mut cx, 10);
        // settings:<page>:<query> types <query> into the sidebar search
        if let Some(q) = scene.strip_prefix("settings:").and_then(|p| p.split_once(':')).map(|(_, q)| q.to_owned()) {
            cx.update_window(win.into(), |_, window, cx| {
                v.update(cx, |app, cx| {
                    if let Some(i) = app.settings.as_ref().and_then(|s| s.search.clone()) {
                        i.update(cx, |i, cx| i.set_value(q.clone(), window, cx));
                    }
                });
            })?;
            pump(&mut cx, 6);
        }
        // PHOENIX_SHOT_SCROLL=<px> scrolls the settings content like a scrolled dump.
        if let Some(y) = std::env::var("PHOENIX_SHOT_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
            cx.update_window(win.into(), |_, window, cx| {
                window.refresh();
                v.update(cx, |app, _| {
                    if let Some(s) = app.settings.as_ref() {
                        s.scroll.set_offset(gpui::point(px(0.), px(-y)));
                    }
                });
            })?;
            pump(&mut cx, 4);
        }
    }
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, if _rt.is_some() { 80 } else { 8 });
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 4);
    // Headless time is virtual: advance it so enter animations (modals, popouts) finish.
    cx.advance_clock(std::time::Duration::from_secs(2));
    let _ = cx.capture_screenshot(win.into())?;
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    // Headless time is virtual: advance it so enter animations (modals, popouts) finish.
    cx.advance_clock(std::time::Duration::from_secs(2));
    let img = cx.capture_screenshot(win.into())?;
    img.save(&out)?;
    println!("wrote {out} ({}x{})", img.width(), img.height());
    Ok(())
}
