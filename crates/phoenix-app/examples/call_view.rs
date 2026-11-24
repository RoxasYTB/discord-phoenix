use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let login_only = false;
    let rest: Vec<String> = args.collect();
    let with_chat = rest.iter().any(|a| a == "chat");
    let focus_grid = rest.iter().any(|a| a == "grid");
    let no_strip = rest.iter().any(|a| a == "nostrip");

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
                use phoenix_app::model::{Id, VoiceFlags};
                let Some(lounge) = app.channels.values().find(|c| c.name.as_ref() == "Lounge").map(|c| c.id) else { return };
                let me = app.me.id;
                if let Some(ch) = app.channels.get_mut(&lounge)
                    && !ch.voice_users.contains(&me)
                {
                    ch.voice_users.insert(0, me);
                }
                app.voice_channel = Some(lounge);
                app.open_channel_chat(lounge, cx);
                phoenix_app::ui::call_view::show(lounge);
                phoenix_app::ui::call_view::set_chat_open(with_chat);
                if focus_grid {
                    phoenix_app::ui::call_view::set_focus(phoenix_app::ui::call_view::Focus::Grid);
                }
                phoenix_app::ui::call_view::touch();
                phoenix_app::ui::call_view::set_strip_open(!no_strip);
                phoenix_app::ui::voice_panel::set_flags(me, VoiceFlags { stream: true, ..Default::default() });
                phoenix_app::ui::voice_panel::set_flags(Id(4), VoiceFlags { self_mute: true, ..Default::default() });
                phoenix_app::ui::voice_panel::set_flags(Id(7), VoiceFlags { self_deaf: true, ..Default::default() });
                app.voice.speaking.insert(Id(3));
            });
        })?;
        pump(&mut cx, 10);
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
