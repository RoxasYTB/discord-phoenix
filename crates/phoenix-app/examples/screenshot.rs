use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

#[path = "common/frames_impl.rs"]
mod frames_impl;
#[path = "common/input_script.rs"]
mod input_script;
#[path = "common/name_frames.rs"]
mod name_frames;

fn main() -> anyhow::Result<()> {
    if std::env::var_os("PHX_FRAMES").is_some() {
        return frames_impl::run();
    }
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let login_only = args.next().as_deref() == Some("login");
    if std::env::var_os("PHOENIX_CSD").is_some() {
        phoenix_app::ui::title_bar::FORCE_CLIENT_DECORATIONS.store(true, std::sync::atomic::Ordering::Relaxed);
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
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
        phoenix_app::demo_media::install(cx);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let dim = |key: &str, default: f32| std::env::var(key).ok().and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.).unwrap_or(default);
    let win = cx.open_window(size(px(dim("PHX_W", 1280.)), px(dim("PHX_H", 800.))), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(Arc::new(MockBackend::new()), window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    let scale = std::env::var("PHX_SCALE").ok().and_then(|v| v.parse::<f32>().ok()).filter(|s| *s > 0.);
    if let Some(scale) = scale {
        cx.update_window(win.into(), |_, window, _| window.set_scale_factor(scale))?;
    }
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
    }
    let compact = std::env::var_os("PHX_COMPACT").is_some();
    let compact_avatars = std::env::var("PHX_COMPACT_AVATARS").ok().and_then(|v| match v.as_str() {
        "0" | "false" => Some(false),
        "1" | "true" => Some(true),
        _ => None,
    });
    if compact || compact_avatars.is_some() {
        let v = view.borrow().clone().expect("view");
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| {
                if compact {
                    app.prefs.synced.compact = true;
                }
                if let Some(show) = compact_avatars {
                    app.prefs.synced.proto.insert("avatars", if show { 1 } else { 0 });
                }
                cx.notify();
            });
            window.refresh();
        })?;
        pump(&mut cx, 2);
    }
    if std::env::var_os("PHX_LAUNCHER").is_some() {
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(&mut cx, 2);
        let v = view.borrow().clone().expect("view");
        cx.update_window(win.into(), |_, window, cx| {
            v.update(cx, |app, cx| phoenix_app::ui::chat::app_launcher::toggle(app, window, cx));
        })?;
        pump(&mut cx, 4);
    }
    if let Some(dm) = std::env::var("PHX_DM").ok().and_then(|v| v.parse::<u64>().ok()) {
        let v = view.borrow().clone().expect("view");
        cx.update_window(win.into(), |_, _, cx| {
            v.update(cx, |app, cx| {
                app.select_guild(None, cx);
                app.open_channel_chat(phoenix_app::model::Id(dm), cx);
            })
        })?;
        pump(&mut cx, 20);
    }
    if let Ok(spec) = std::env::var("PHX_SCRIPT") {
        let script = std::fs::read_to_string(&spec).unwrap_or(spec);
        let shoot = |cx: &mut HeadlessAppContext, path: &str| -> anyhow::Result<()> {
            let mut img = None;
            for _ in 0..2 {
                cx.update_window(win.into(), |_, window, _| window.refresh())?;
                pump(cx, 2);
                cx.advance_clock(Duration::from_secs(2));
                img = Some(cx.capture_screenshot(win.into())?);
            }
            let img = img.expect("frame");
            img.save(path)?;
            println!("wrote {path} ({}x{})", img.width(), img.height());
            Ok(())
        };
        let v = view.borrow().clone().expect("view");
        let env = input_script::Env { win: win.into(), app: v, stem: out.trim_end_matches(".png").to_string(), shoot: &shoot };
        let report = input_script::run(&mut cx, &env, &script)?;
        if std::env::var_os("PHX_SCRIPT_STRICT").is_some() {
            anyhow::ensure!(report.ko.is_empty(), "{} QA checks failed", report.ko.len());
        }
        return Ok(());
    }
    if let Ok(spec) = std::env::var("PHX_GRADIENT_AT") {
        return name_frames::gradient_frames(&mut cx, win.into(), &spec, &out, scale.unwrap_or(1.));
    }
    if let Some(secs) = std::env::var("PHX_PRESENT_CPU").ok().and_then(|v| v.parse::<u64>().ok()) {
        return name_frames::present_cpu(&mut cx, win.into(), secs, scale.unwrap_or(1.));
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
