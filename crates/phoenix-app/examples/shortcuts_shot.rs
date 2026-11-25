use std::{sync::Arc, time::Duration};

use std::{cell::RefCell, rc::Rc};

use gpui::{AppContext, Entity, HeadlessAppContext, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "phoenix.png".into());
    let scene = args.next().unwrap_or_else(|| "modal".into());

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), || gpui_platform::current_headless_renderer());
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
    let env = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let win = cx.open_window(size(px(env("PHOENIX_SHOT_W", 1536.)), px(env("PHOENIX_SHOT_H", 840.))), move |window, cx| {
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
        v.update(cx, |app, cx| match scene.as_str() {
            "focus" => {
                app.focus_last_message(window, cx);
            }
            "mention" | "emoji" | "channel" | "long" | "multiline" => {
                let text = match scene.as_str() {
                    "mention" => "@".to_string(),
                    "emoji" => ":sm".to_string(),
                    "channel" => "#".to_string(),
                    "multiline" => "first line\nsecond line\nthird line".to_string(),
                    _ => "lorem ipsum dolor sit amet ".repeat(70),
                };
                app.composer.update(cx, |s, cx| {
                    s.set_value(text, window, cx);
                    s.focus(window, cx);
                });
                phoenix_app::ui::autocomplete::refresh(app, window, cx);
            }
            "slowmode" | "blocked" => {
                if let Some(ch) = app.selected_channel {
                    let x = app.nav.extras.entry(ch).or_default();
                    if scene == "blocked" {
                        x.send_blocked = true;
                    } else {
                        x.slowmode = 30;
                        phoenix_app::ui::chat::composer::note_sent(ch);
                    }
                }
                cx.notify();
            }
            "upload" | "spoiler" => {
                let icon = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"));
                app.add_paths(vec![icon], cx);
                if scene == "spoiler" {
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_millis(200)).await;
                        let _ = this.update(cx, |a, cx| a.toggle_upload_spoiler(0, cx));
                    })
                    .detach();
                }
            }
            "thread" => {
                if let Some(ch) = app.selected_channel {
                    app.open_create_thread(ch, window, cx);
                }
            }
            _ => {
                phoenix_app::shortcuts::open_help();
                cx.notify();
            }
        })
    })?;
    pump(&mut cx, 8);
    if scene == "bench" {
        return bench(&mut cx, win.into(), &v);
    }
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    pump(&mut cx, 2);
    cx.advance_clock(Duration::from_secs(2));
    let _ = cx.capture_screenshot(win.into())?;
    cx.update_window(win.into(), |_, window, _| window.refresh())?;
    cx.advance_clock(Duration::from_secs(2));
    let img = cx.capture_screenshot(win.into())?;
    img.save(&out)?;
    println!("wrote {out} ({}x{})", img.width(), img.height());
    Ok(())
}

fn bench_draft() -> String {
    let block = "Some **bold** text, *italic*, __underline__, ~~strike~~ and `inline code` here.\n\
-# a subtext line with a [link](https://example.com)\n\
> a quoted line with ||a spoiler|| inside\n\
```rust\nfn main() { let x = 42; println!(\"{x}\"); }\n```\n\
# A heading and plain words that wrap around the composer width once or twice at least\n";
    let mut out = String::new();
    while out.len() < 2000 {
        out.push_str(block);
    }
    out
}

fn bench(cx: &mut HeadlessAppContext, win: gpui::AnyWindowHandle, v: &Entity<PhoenixApp>) -> anyhow::Result<()> {
    let draft = bench_draft();
    for preview in [false, true] {
        cx.update_window(win, |_, window, cx| {
            v.update(cx, |app, cx| {
                app.prefs.synced.proto.insert("md-preview", preview as u64);
                app.prefs.synced.proto.insert("legacy-input", 0);
                app.composer.update(cx, |s, cx| {
                    s.set_value(draft.clone(), window, cx);
                    s.focus(window, cx);
                });
                phoenix_app::ui::chat::rich_input::sync(app, cx);
            });
            for _ in 0..3 {
                window.refresh();
                window.draw(cx).clear(cx);
            }
        })?;
        let mut sync = Vec::new();
        let mut draw = Vec::new();
        for i in 0..60 {
            cx.update_window(win, |_, window, cx| {
                let t = std::time::Instant::now();
                v.update(cx, |app, cx| {
                    app.composer.update(cx, |s, cx| s.insert(if i % 7 == 6 { " " } else { "x" }, window, cx));
                    phoenix_app::ui::chat::rich_input::sync(app, cx);
                });
                sync.push(t.elapsed().as_secs_f64() * 1e3);
                let t = std::time::Instant::now();
                window.refresh();
                window.draw(cx).clear(cx);
                draw.push(t.elapsed().as_secs_f64() * 1e3);
            })?;
        }
        let stat = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            (v[v.len() / 2], v[v.len() * 9 / 10])
        };
        let (sm, sp) = stat(&mut sync);
        let (dm, dp) = stat(&mut draw);
        println!("bench chars={} preview={preview} edit+sync median={sm:.3}ms p90={sp:.3}ms frame median={dm:.3}ms p90={dp:.3}ms", draft.len());
    }
    Ok(())
}
