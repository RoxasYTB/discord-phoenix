use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{AppContext, Entity, HeadlessAppContext, ListOffset, px, size};
use gpui_component::Root;
use phoenix_app::{app::PhoenixApp, assets::Assets, mock::MockBackend};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(n as u64, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[path = "common/frames_impl.rs"]
mod frames_impl;

fn allocs() -> (u64, u64) {
    (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed))
}

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

const CORPUS: &[&str] = &[
    "hey everyone 👋",
    "has anyone tried the new **GPUI** release? it's *really* fast",
    "`cargo build --release` takes forever on my laptop though",
    "check this out https://github.com/zed-industries/zed and <#1003>",
    "> quoting for posterity\nthat's a hot take",
    "```rust\nfn main() {\n    println!(\"hello from phoenix\");\n}\n```",
    "||spoiler: it was the borrow checker all along|| <@1>",
    "__underlined__ and ~~struck~~ text works too, also <t:1700000000:R>",
    "# Heading\n- item one\n- item **two**\n- item `three`",
    "lol",
];

fn bench_markdown() {
    let n = 20_000;
    let r = phoenix_app::markdown::NoResolve;
    let color = gpui::white();
    let (a0, b0) = allocs();
    let t = Instant::now();
    let mut blocks = 0usize;
    for i in 0..n {
        let src = CORPUS[i % CORPUS.len()];
        let parsed = phoenix_app::markdown::parse(src, &r);
        blocks += phoenix_app::richtext::render_blocks(&parsed, &|_| true, color).len();
    }
    let el = t.elapsed();
    let (a1, b1) = allocs();
    println!("markdown_ns_per_msg={}", el.as_nanos() / n as u128);
    println!("markdown_allocs_per_msg={:.1}", (a1 - a0) as f64 / n as f64);
    println!("markdown_bytes_per_msg={}", (b1 - b0) / n as u64);
    std::hint::black_box(blocks);
}

fn main() -> anyhow::Result<()> {
    if std::env::var_os("PHX_FRAMES").is_some() {
        let _ = frames_impl::COUNTER.set(|| (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed)));
        return frames_impl::run();
    }
    let target: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(5000);
    let t_start = Instant::now();
    bench_markdown();
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
    });
    let view: Rc<RefCell<Option<Entity<PhoenixApp>>>> = Rc::default();
    let v2 = view.clone();
    let win = cx.open_window(size(px(1280.), px(800.)), move |window, cx| {
        let v = cx.new(|cx| PhoenixApp::new(Arc::new(MockBackend::new()), window, cx));
        *v2.borrow_mut() = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    })?;
    let v = view.borrow().clone().expect("view");
    let draw = |cx: &mut HeadlessAppContext| {
        cx.update_window(win.into(), |_, window, _| window.refresh()).ok();
        cx.run_until_parked();
        cx.advance_clock(std::time::Duration::from_secs(2));
        let _ = cx.capture_screenshot(win.into());
    };
    draw(&mut cx);
    println!("headless_first_frame_ms={}", t_start.elapsed().as_millis());

    cx.update_window(win.into(), |_, window, cx| {
        v.update(cx, |app, cx| {
            app.login_with_token = true;
            app.login_token.update(cx, |s, cx| s.set_value("mock-token", window, cx));
            app.submit_login(window, cx);
        });
    })?;
    let count = |cx: &mut HeadlessAppContext| {
        cx.update(|cx| {
            let a = v.read(cx);
            a.selected_channel.and_then(|c| a.messages.get(&c)).map_or(0, |e| e.msgs.len())
        })
    };
    let t_login = Instant::now();
    while count(&mut cx) == 0 && t_login.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(10));
        draw(&mut cx);
    }
    println!("headless_login_to_messages_ms={}", t_login.elapsed().as_millis());
    println!("rss_main_view_kib={}", rss_kib());

    let t_load = Instant::now();
    while count(&mut cx) < target && t_load.elapsed() < Duration::from_secs(120) {
        cx.update(|cx| {
            v.read(cx).message_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
            v.update(cx, |app, cx| app.load_older(cx));
        });
        std::thread::sleep(Duration::from_millis(5));
        draw(&mut cx);
    }
    println!("channel_messages={}", count(&mut cx));
    println!("channel_load_ms={}", t_load.elapsed().as_millis());

    let frames = 600;
    let (a0, b0) = allocs();
    let mut times = Vec::with_capacity(frames);
    for _ in 0..frames {
        cx.update(|cx| v.read(cx).message_list.scroll_by(px(400.)));
        let t = Instant::now();
        draw(&mut cx);
        times.push(t.elapsed().as_micros() as u64);
    }
    let (a1, b1) = allocs();
    times.sort_unstable();
    println!("scroll_frame_us_p50={}", times[frames / 2]);
    println!("scroll_frame_us_p95={}", times[frames * 95 / 100]);
    println!("scroll_allocs_per_frame={}", (a1 - a0) / frames as u64);
    println!("scroll_bytes_per_frame={}", (b1 - b0) / frames as u64);

    let (a0, _) = allocs();
    let t = Instant::now();
    for _ in 0..100 {
        draw(&mut cx);
    }
    println!("idle_frame_us={}", t.elapsed().as_micros() / 100);
    println!("idle_allocs_per_frame={}", (allocs().0 - a0) / 100);
    println!("rss_after_scroll_kib={}", rss_kib());
    let (a0, b0) = allocs();
    let t = Instant::now();
    for _ in 0..100 {
        cx.update_window(win.into(), |_, window, _| window.refresh()).ok();
        cx.run_until_parked();
    }
    println!("idle_nocapture_us={}", t.elapsed().as_micros() / 100);
    println!("idle_nocapture_allocs={} bytes={}", (allocs().0 - a0) / 100, (allocs().1 - b0) / 100);
    Ok(())
}
