use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend, ui::settings::AppTheme};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let theme = match args.next().as_deref() {
        Some("light") => AppTheme::Light,
        Some("ash") => AppTheme::Ash,
        Some("onyx") => AppTheme::Onyx,
        _ => AppTheme::Dark,
    };
    let scene = args.next().unwrap_or_else(|| "appearance".into());
    let login_only = false;

    let dir = std::env::temp_dir().join("phoenix-mock-media");
    std::fs::create_dir_all(&dir)?;
    for (i, (name, w, h)) in [("photo", 1280, 720), ("one", 800, 600), ("two", 800, 600), ("three", 800, 600), ("clip", 1280, 720), ("thumb", 400, 400), ("video", 1280, 720), ("SPOILER_photo", 1280, 720)].into_iter().enumerate() {
        let hue = i as f32 / 8.;
        image::RgbaImage::from_fn(w, h, |x, y| {
            let t = (x as f32 / w as f32 + y as f32 / h as f32) / 2.;
            let r = (60. + 150. * ((hue + t) * std::f32::consts::TAU).sin().abs()) as u8;
            let g = (50. + 120. * ((hue * 2. + t) * std::f32::consts::PI).cos().abs()) as u8;
            image::Rgba([r, g, (200. - 120. * t) as u8, 255])
        })
        .save(dir.join(format!("{name}.png")))?;
    }
    // SAFETY: set before any thread is spawned.
    unsafe { std::env::set_var("PHX_MOCK_MEDIA_DIR", &dir) };
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
    let h: f32 = std::env::var("PHOENIX_SHOT_H").ok().and_then(|v| v.parse().ok()).unwrap_or(800.);
    let w: f32 = std::env::var("PHOENIX_SHOT_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280.);
    let win = cx.open_window(size(px(w), px(h)), move |window, cx| {
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
                app.prefs.synced.theme = theme;
                let g = app.guilds.first().map(|g| g.id);
                app.select_guild(g, cx);
                let id = app.channels.values().find(|c| c.guild == g && c.name.as_ref() == "showcase").map(|c| c.id);
                if let Some(id) = id {
                    app.select_channel(id, cx);
                }
                let _ = (&scene, window);
                cx.notify();
            });
        })?;
        pump(&mut cx, 4);
    }
    let frames = 10;
    for _ in 0..frames {
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(&mut cx, 2);
    }
    let _ = cx.capture_screenshot(win.into())?;
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    let img = cx.capture_screenshot(win.into())?;
    img.save(&out)?;
    println!("wrote {out} ({}x{})", img.width(), img.height());
    Ok(())
}
