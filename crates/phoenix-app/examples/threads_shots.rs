use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let login_only = false;
    let _ = args.next();

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
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                use phoenix_app::model::{ChannelKind, Id};
                let text = app.channels.values().find(|c| c.kind == ChannelKind::Text && c.guild.is_some()).map(|c| (c.id, c.guild));
                if let Some((ch, g)) = text {
                    app.select_guild(g, cx);
                    app.select_channel(ch, cx);
                    let owner = app.users.keys().next().copied();
                    let post = |id: u64, t: &str| phoenix_app::commands::ForumPost {
                        thread: Id(id),
                        title: t.into(),
                        tags: vec![],
                        message_count: 3,
                        first_message: None,
                        owner,
                        last_message: Some(Id(id)),
                        member_ids_preview: if id == 1_250_000_000_000_000_000 { app.users.keys().take(3).copied().collect() } else { vec![] },
                        member_count: if id == 1_250_000_000_000_000_000 { 3 } else { 0 },
                        pinned: false,
                        archived: true,
                        locked: false,
                    };
                    app.parity.archived_threads.insert(ch, vec![post(1_250_000_000_000_000_000, "Weekend plans"), post(1_200_000_000_000_000_000, "Old bug report")]);
                    app.parity.threads_open = true;
                }
                cx.notify();
            });
        })?;
        pump(&mut cx, 20);
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
