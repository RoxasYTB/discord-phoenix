use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AppContext, Entity, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, px, size,
};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "states".into());
    let points: Vec<String> = args.collect();
    let login_only = false;

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
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
        phoenix_app::demo_media::install(cx);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(1280.), px(800.)), move |window, cx| {
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
        if let Ok(what) = std::env::var("PHX_OPEN") {
            cx.update_window(win.into(), |_, window, cx| {
                v.update(cx, |app, cx| match what.split_once(':').map_or(what.as_str(), |w| w.0) {
                    "pins" => app.toggle_pins(cx),
                    "inbox" => app.toggle_inbox(cx),
                    "threads" => app.toggle_threads_popout(cx),
                    "groupdm" => app.open_group_dm_picker(cx),
                    "privacy" => {
                        if let Some(g) = app.guilds.first().map(|g| g.id) {
                            app.open_privacy_settings(g, cx)
                        }
                    }
                    "server_settings" => {
                        if let Some(g) = app.guilds.first().map(|g| g.id) {
                            let arg = what.split_once(':').map_or("", |w| w.1);
                            let (section, sub) = arg.split_once('/').unwrap_or((arg, ""));
                            app.open_server_settings(g, phoenix_app::ui::modals::server_settings::section_index(&format!("menu.gs.{section}")), cx);
                            if section == "access" && !sub.is_empty() {
                                phoenix_app::ui::modals::server_settings::preselect_access(sub);
                                phoenix_app::ui::modals::server_settings::load(app, cx);
                            }
                        }
                    }
                    "server_profile" => {
                        if let Some(g) = app.guilds.first().map(|g| g.id) {
                            app.open_server_profile(g, cx)
                        }
                    }
                    "search" => {
                        phoenix_app::ui::search::open(app, window, cx);
                        let q = what.split_once(':').map_or("rust", |w| w.1);
                        if let Ok(req) = phoenix_app::ui::search::build_request(app, q) {
                            phoenix_app::ui::search::run(app, req, cx);
                        }
                    }
                    _ => {}
                })
            })?;
            pump(&mut cx, 20);
        }
        if let Some(n) = std::env::var("PHX_TYPING").ok().and_then(|n| n.parse::<usize>().ok()) {
            cx.update_window(win.into(), |_, _, cx| {
                v.update(cx, |app, cx| {
                    let Some(ch) = app.selected_channel else { return };
                    let me = app.me.id;
                    let mut ids: Vec<_> = app.users.keys().copied().filter(|u| *u != me).collect();
                    ids.sort_by_key(|u| u.0);
                    ids.truncate(n);
                    if let Some(first) = ids.first() {
                        app.typing_nicks.insert((ch, *first), "Pseudo du serveur".into());
                    }
                    app.typing.insert(ch, ids.into_iter().map(|u| (u, std::time::Instant::now())).collect());
                    cx.notify();
                })
            })?;
            pump(&mut cx, 4);
        }
        if std::env::var_os("PHX_SETTINGS").is_some() {
            cx.update_window(win.into(), |_, _, cx| v.update(cx, phoenix_app::ui::settings::open))?;
            pump(&mut cx, 10);
        }
    }
    std::fs::create_dir_all(&out)?;
    for (n, p) in std::iter::once("0,0".to_string()).chain(points).enumerate() {
        let parts: Vec<&str> = p.split(',').collect();
        let (x, y): (f32, f32) = (parts[0].parse()?, parts[1].parse()?);
        let down = parts.get(2) == Some(&"down");
        let right = parts.get(2) == Some(&"right");
        let position = gpui::point(px(x), px(y));
        cx.update_window(win.into(), |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers: Modifiers::default() }),
                cx,
            );
            if down || right {
                let button = if right { MouseButton::Right } else { MouseButton::Left };
                window.dispatch_event(
                    PlatformInput::MouseDown(MouseDownEvent {
                        button,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    cx,
                );
            }
            window.refresh();
        })?;
        pump(&mut cx, 6);
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(&mut cx, 2);
        cx.advance_clock(std::time::Duration::from_secs(2));
        let img = cx.capture_screenshot(win.into())?;
        let path = format!("{out}/state-{n:02}.png");
        img.save(&path)?;
        println!("wrote {path} ({p})");
        if down {
            cx.update_window(win.into(), |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position, modifiers: Modifiers::default(), click_count: 1 }),
                    cx,
                );
            })?;
            pump(&mut cx, 2);
        }
    }
    Ok(())
}
