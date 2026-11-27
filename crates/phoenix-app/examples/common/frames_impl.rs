use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use gpui::{
    AppContext, Entity, HeadlessAppContext, ListOffset, Modifiers, MouseMoveEvent, PlatformInput, ScrollDelta,
    ScrollEvent, ScrollSource, TouchPhase, point, px, size,
};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

pub static COUNTER: OnceLock<fn() -> (u64, u64)> = OnceLock::new();

pub type Sampler = (fn(u64), fn() -> Vec<std::backtrace::Backtrace>);

pub static SAMPLER: OnceLock<Sampler> = OnceLock::new();

fn frame_names(bt: &std::backtrace::Backtrace) -> Vec<String> {
    bt.to_string()
        .lines()
        .filter_map(|l| {
            let (n, name) = l.trim_start().split_once(": ")?;
            n.parse::<usize>().ok()?;
            let name = match name.rsplit_once("::h") {
                Some((head, hash)) if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) => head,
                _ => name,
            };
            Some(name.to_string())
        })
        .collect()
}

fn plumbing(f: &str) -> bool {
    let f = f.trim_start_matches('<');
    ["std::", "core::", "alloc::", "__rust", "frames::", "bench::", "hashbrown::", "smallvec::", "collections::", "_Unwind", "rust_", "unknown"]
        .iter()
        .any(|p| f.starts_with(p))
        || f.contains("Counting")
}

fn report_sites(sites: &[std::backtrace::Backtrace]) {
    let mut leaf: std::collections::HashMap<String, usize> = Default::default();
    let mut app: std::collections::HashMap<String, usize> = Default::default();
    let mut incl: std::collections::HashMap<String, usize> = Default::default();
    for bt in sites {
        let names = frame_names(bt);
        if let Some(f) = names.iter().find(|f| !plumbing(f)) {
            *leaf.entry(f.clone()).or_default() += 1;
        }
        if let Some(f) = names.iter().find(|f| f.contains("phoenix_app::")) {
            *app.entry(f.clone()).or_default() += 1;
        }
        let mut seen = std::collections::HashSet::new();
        for f in names.iter().filter(|f| f.contains("phoenix_app::")) {
            if seen.insert(f.clone()) {
                *incl.entry(f.clone()).or_default() += 1;
            }
        }
    }
    let n = sites.len().max(1) as f64;
    for (label, map) in [("leaf", leaf), ("app", app), ("incl", incl)] {
        let mut rows: Vec<_> = map.into_iter().collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.1));
        for (f, c) in rows.into_iter().take(40) {
            println!("alloc_{label} {:5.1}% {c:5} {f}", c as f64 * 100. / n);
        }
    }
    println!("alloc_samples={}", sites.len());
}

fn counts() -> (u64, u64) {
    COUNTER.get().map_or((0, 0), |f| f())
}

struct Probe<'a> {
    cx: &'a mut HeadlessAppContext,
    win: gpui::AnyWindowHandle,
}

impl Probe<'_> {
    fn frame(&mut self) {
        self.cx.update_window(self.win, |_, window, _| window.refresh()).ok();
        self.cx.run_until_parked();
        self.cx.advance_clock(Duration::from_millis(16));
    }

    fn input(&mut self, ev: PlatformInput) {
        self.cx
            .update_window(self.win, |_, window, cx| {
                window.dispatch_event(ev, cx);
            })
            .ok();
    }

    fn measure(&mut self, name: &str, frames: usize, mut step: impl FnMut(&mut Self, usize)) {
        let (a0, b0) = counts();
        let mut times = Vec::with_capacity(frames);
        for i in 0..frames {
            let t = Instant::now();
            step(self, i);
            self.frame();
            times.push(t.elapsed().as_micros() as u64);
        }
        let (a1, b1) = counts();
        let over = times.iter().filter(|t| **t > 16_000).count();
        times.sort_unstable();
        println!(
            "{name} p50_us={} p95_us={} max_us={} over16={over}/{frames} allocs={} bytes={}",
            times[frames / 2],
            times[frames * 95 / 100],
            times[frames - 1],
            (a1 - a0) / frames as u64,
            (b1 - b0) / frames as u64
        );
    }
}

fn cpu_secs() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let f: Vec<&str> = stat.rsplit_once(')').map_or("", |(_, r)| r).split_whitespace().collect();
    let ticks = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
    (ticks(11) + ticks(12)) / 100.
}

impl Probe<'_> {
    fn live(&mut self, name: &str, frames: usize, mut step: impl FnMut(&mut Self, usize)) {
        let (c0, w0) = (cpu_secs(), Instant::now());
        let mut times = Vec::with_capacity(frames);
        for i in 0..frames {
            let t = Instant::now();
            step(self, i);
            self.cx.simulate_frame_request(self.win);
            self.cx.run_until_parked();
            let spent = t.elapsed();
            times.push(spent.as_micros() as u64);
            if let Some(rest) = Duration::from_millis(16).checked_sub(spent) {
                std::thread::sleep(rest);
            }
        }
        let cpu = (cpu_secs() - c0) / w0.elapsed().as_secs_f64() * 100.;
        let total: u64 = times.iter().sum();
        let over = times.iter().filter(|t| **t > 16_000).count();
        times.sort_unstable();
        println!(
            "{name} p50_us={} p95_us={} max_us={} mean_us={} over16={over}/{frames} cpu_pct={cpu:.1}",
            times[frames / 2],
            times[frames * 95 / 100],
            times[frames - 1],
            total / frames as u64
        );
    }
}

fn notch(x: f32, y: f32, lines: f32) -> PlatformInput {
    PlatformInput::Scroll(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Lines(point(0., lines)),
        modifiers: Modifiers::default(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    })
}

fn wheel(x: f32, y: f32, dy: f32) -> PlatformInput {
    PlatformInput::Scroll(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        modifiers: Modifiers::default(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    })
}

fn mouse(x: f32, y: f32) -> PlatformInput {
    PlatformInput::MouseMove(MouseMoveEvent { position: point(px(x), px(y)), pressed_button: None, modifiers: Modifiers::default() })
}

pub fn run() -> anyhow::Result<()> {
    let target: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(2000);
    // SAFETY: single-threaded at this point; the mock reads it when generating history.
    unsafe { std::env::set_var("PHOENIX_MOCK_HISTORY", target.to_string()) };

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), gpui_platform::current_headless_renderer);
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Medium.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ]
    .iter()
    .filter_map(|p| std::fs::read(p).ok().map(Into::into))
    .collect();
    cx.update(|cx| {
        let _ = cx.text_system().add_fonts(fonts);
        phoenix_app::init(cx);
        let b = phoenix_app::discord_backend::DiscordBackend::new();
        cx.set_http_client(Arc::new(phoenix_app::media::cache::CachingHttpClient::new(b.http_client())));
        std::mem::forget(b);
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(1280.), px(800.)), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(Arc::new(MockBackend::new()), window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    let v = view.borrow().clone().expect("view");
    let mut p = Probe { cx: &mut cx, win: win.into() };
    p.frame();
    p.cx.update_window(p.win, |_, window, cx| {
        v.update(cx, |app, cx| {
            app.login_with_token = true;
            app.login_token.update(cx, |s, cx| s.set_value("mock-token", window, cx));
            app.submit_login(window, cx);
        });
    })?;
    let count = |p: &mut Probe| {
        p.cx.update(|cx| {
            let a = v.read(cx);
            a.selected_channel.and_then(|c| a.messages.get(&c)).map_or(0, |e| e.msgs.len())
        })
    };
    let t = Instant::now();
    while count(&mut p) == 0 && t.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(10));
        p.frame();
    }
    let t = Instant::now();
    while count(&mut p) < target && t.elapsed() < Duration::from_secs(120) {
        p.cx.update(|cx| {
            v.read(cx).message_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
            v.update(cx, |app, cx| app.load_older(cx));
        });
        std::thread::sleep(Duration::from_millis(5));
        p.frame();
    }
    println!("messages={} rss_kib={}", count(&mut p), rss_kib());

    p.measure("idle", 60, |_, _| {});
    p.measure("scroll_chat_wheel", 300, |p, _| p.input(wheel(700., 400., -120.)));
    if let (Some(every), Some((start, stop))) = (std::env::var("PHX_ALLOC_SITES").ok().and_then(|v| v.parse::<u64>().ok()), SAMPLER.get()) {
        start(every.max(1));
        for _ in 0..120 {
            p.input(wheel(700., 400., -120.));
            p.frame();
        }
        let sites = stop();
        report_sites(&sites);
        return Ok(());
    }
    let trace = std::env::var_os("PHX_TRACE_SCROLL").is_some();
    let t_trace = Instant::now();
    p.live("scroll_chat_notches", 240, |p, i| {
        if i % 8 == 0 {
            p.input(notch(700., 400., if i < 120 { 3. } else { -3. }));
        }
        if trace && i < 48 {
            let y = p.cx.update(|cx| v.read(cx).message_list.scroll_px_offset_for_scrollbar().y);
            println!("trace t_ms={} y={:.2}", t_trace.elapsed().as_millis(), f32::from(y));
        }
    });
    p.cx.update(|cx| v.read(cx).message_list.scroll_to_end());
    p.frame();
    let t_burst = Instant::now();
    p.live("scroll_chat_burst_from_end", 60, |p, i| {
        if i < 20 && i % 2 == 0 {
            p.input(notch(700., 400., 3.));
        }
        if trace {
            let y = p.cx.update(|cx| v.read(cx).message_list.scroll_px_offset_for_scrollbar().y);
            println!("burst t_ms={} y={:.2}", t_burst.elapsed().as_millis(), f32::from(y));
        }
    });
    p.cx.update(|cx| v.read(cx).message_list.scroll_to_end());
    p.cx.update_window(p.win, |_, window, cx| {
        window.activate_window();
        v.update(cx, |app, cx| app.composer.update(cx, |s, cx| s.focus(window, cx)))
    })
    .ok();
    p.frame();
    let t_page = Instant::now();
    p.live("scroll_chat_page", 120, |p, i| {
        if i % 40 == 0 {
            let key = if i < 80 { "pageup" } else { "pagedown" };
            if let Ok(ks) = gpui::Keystroke::parse(key) {
                p.cx.update_window(p.win, |_, window, cx| {
                    window.dispatch_keystroke(ks, cx);
                })
                .ok();
            }
        }
        if trace {
            let y = p.cx.update(|cx| v.read(cx).message_list.scroll_px_offset_for_scrollbar().y);
            println!("page t_ms={} y={:.2}", t_page.elapsed().as_millis(), f32::from(y));
        }
    });
    p.cx.update_window(p.win, |_, window, cx| window.blur(cx)).ok();
    let mic = phoenix_app::ui::states::key(("audio-btn", "Mic"));
    let hover = |p: &mut Probe, over: bool| {
        phoenix_app::ui::states::set_hover(mic, over);
        p.cx.update_window(p.win, |_, window, _| window.refresh()).ok();
    };
    p.live("lottie_idle", 120, |_, _| {});
    p.live("lottie_hover_mic", 480, |p, i| match i % 60 {
        0 => hover(p, true),
        50 => hover(p, false),
        _ => {}
    });
    p.live("hover_rail_guild", 240, |p, i| match i % 40 {
        0 => p.input(mouse(36., 140.)),
        20 => p.input(mouse(640., 300.)),
        _ => {}
    });
    p.measure("scroll_chat_fast", 200, |p, _| p.cx.update(|cx| v.read(cx).message_list.scroll_by(px(600.))));
    p.measure("hover_messages", 200, |p, i| p.input(mouse(700., 120. + (i % 40) as f32 * 15.)));
    let guilds: Vec<_> = p.cx.update(|cx| v.read(cx).guilds.iter().map(|g| g.id).collect());
    if !guilds.is_empty() {
        p.measure("switch_guild", 40, |p, i| {
            let g = guilds[i % guilds.len()];
            p.cx.update(|cx| v.update(cx, |app, cx| app.select_guild(Some(g), cx)));
        });
        p.measure("switch_home_guild", 20, |p, i| {
            let g = (i % 2 == 0).then_some(guilds[0]);
            p.cx.update(|cx| v.update(cx, |app, cx| app.select_guild(g, cx)));
        });
    }
    p.cx.update(|cx| {
        v.update(cx, |app, cx| {
            app.show_members = true;
            cx.notify();
        })
    });
    p.frame();
    p.measure("scroll_members", 150, |p, _| p.input(wheel(1180., 400., -120.)));
    p.live("scroll_members_notches", 160, |p, i| {
        if i % 8 == 0 {
            p.input(notch(1180., 400., if i < 80 { -3. } else { 3. }));
        }
    });
    p.live("open_expression_picker", 240, |p, i| match i % 60 {
        0 => {
            p.cx.update_window(p.win, |_, window, cx| {
                v.update(cx, |app, cx| {
                    phoenix_app::ui::pickers::open(app, phoenix_app::ui::pickers::Tab::Emoji, phoenix_app::ui::pickers::EmojiTarget::Composer, window, cx)
                })
            })
            .ok();
        }
        40 => {
            p.cx.update_window(p.win, |_, window, cx| v.update(cx, |app, cx| phoenix_app::ui::pickers::close(app, window, cx))).ok();
        }
        _ => {}
    });
    p.cx.update_window(p.win, |_, window, cx| {
        v.update(cx, |app, cx| {
            phoenix_app::ui::pickers::open(app, phoenix_app::ui::pickers::Tab::Emoji, phoenix_app::ui::pickers::EmojiTarget::Composer, window, cx)
        })
    })?;
    for _ in 0..10 {
        p.frame();
    }
    p.measure("scroll_emoji_picker_cold", 150, |p, _| p.input(wheel(1000., 450., -120.)));
    p.measure("scroll_emoji_picker_back", 150, |p, _| p.input(wheel(1000., 450., 120.)));
    p.measure("hover_emoji_picker", 150, |p, i| p.input(mouse(820. + (i % 9) as f32 * 40., 400. + (i / 9 % 5) as f32 * 40.)));
    println!("rss_end_kib={}", rss_kib());

    let soak: u64 = std::env::var("PHX_SOAK_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    if soak > 0 && !guilds.is_empty() {
        p.cx.update_window(p.win, |_, window, cx| v.update(cx, |app, cx| phoenix_app::ui::pickers::close(app, window, cx)))?;
        let t = Instant::now();
        let mut next = 60;
        let mut i = 0usize;
        while t.elapsed().as_secs() < soak {
            let g = guilds[i % guilds.len()];
            p.cx.update(|cx| {
                v.update(cx, |app, cx| {
                    app.select_guild(Some(g), cx);
                    let chans: Vec<_> = app.guild_channels.get(&g).cloned().unwrap_or_default().into_iter().filter(|c| !app.is_trap_channel(*c)).collect();
                    if let Some(c) = chans.get(i / guilds.len() % chans.len().max(1)) {
                        app.select_channel(*c, cx);
                    }
                })
            });
            for k in 0..20 {
                p.input(wheel(700., 400., if k < 10 { 240. } else { -240. }));
                p.input(mouse(700., 100. + k as f32 * 30.));
                p.frame();
            }
            i += 1;
            if t.elapsed().as_secs() >= next {
                println!("soak t={}s switches={i} rss_kib={}", t.elapsed().as_secs(), rss_kib());
                next += 60;
            }
        }
    }
    Ok(())
}
