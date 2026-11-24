use std::{sync::Arc, time::Duration};

use gpui::{AppContext, InteractiveElement, Context, HeadlessAppContext, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb, size};
use phoenix_app::{assets::Assets, media::emoji};

static PLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

struct Shot {
    ids: Vec<String>,
    dir: String,
}

impl Render for Shot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        emoji::sync_motion(PLAY.load(std::sync::atomic::Ordering::Relaxed), true);
        let row = |size: f32, hover_only: bool, ids: &[String]| {
            let mut r = div().flex().items_center().gap(px(8.));
            for id in ids {
                let t = format!("<a:e:{id}>");
                r = r.child(if hover_only { emoji::emoji_element_hover(&t, size) } else { emoji::emoji_element(&t, size) });
            }
            r
        };
        div()
            .size_full()
            .bg(rgb(0x1a1a1e))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(row(22., false, &self.ids))
            .child(row(48., false, &self.ids))
            .child(row(40., true, &self.ids))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .children(self.ids.first().map(|id| gpui::img(format!("local/{}/{id}.gif", self.dir)).size(px(22.))))
                    .children(self.ids.first().map(|id| gpui::img(format!("local/{}/{id}.gif", self.dir)).id("ctl").size(px(22.)))),
            )
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "anim".into());
    let dir = args.next().unwrap_or_default();
    if !dir.is_empty() {
        emoji::set_local_dir(dir.clone());
    }
    let ids: Vec<String> = args.filter(|a| a.chars().all(|c| c.is_ascii_digit())).collect();
    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(Assets), gpui_platform::current_headless_renderer);
    let rt = tokio::runtime::Runtime::new()?;
    let http = phoenix_app::discord_backend::http::CdnHttpClient::new(rt.handle().clone(), "Mozilla/5.0");
    cx.update(|cx| cx.set_http_client(Arc::new(phoenix_app::media::cache::CachingHttpClient::new(Arc::new(http)))));
    let win = cx.open_window(size(px(420.), px(240.)), move |_, cx| cx.new(|_| Shot { ids, dir }))?;
    let pump = |cx: &mut HeadlessAppContext, n: usize| {
        for _ in 0..n {
            std::thread::sleep(Duration::from_millis(50));
            cx.advance_clock(Duration::from_millis(50));
            cx.run_until_parked();
        }
    };
    cx.update_window(win.into(), |_, window, _| window.activate_window())?;
    pump(&mut cx, 60);
    for n in 0..6 {
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        pump(&mut cx, 3);
        let img = cx.capture_screenshot(win.into())?;
        img.save(format!("{out}-{n}.png"))?;
    }
    println!("wrote {out}-0..5.png");

    let scale = cx.update_window(win.into(), |_, window, _| window.scale_factor())?;
    let at = ((16. + 11.) * scale) as u32;
    let mut last = None;
    let mut changes = 0;
    let (mut ctl, mut ctl_last) = ((0usize, 0usize), None);
    let t0 = std::time::Instant::now();
    while t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
        cx.advance_clock(Duration::from_millis(20));
        cx.update_window(win.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
        let shot = cx.capture_screenshot(win.into())?;
        let px = shot.get_pixel(at, at).0;
        if last.is_some_and(|l| l != px) {
            changes += 1;
        }
        last = Some(px);
        let cy = ((16. + 22. + 12. + 48. + 12. + 40. + 12. + 11.) * scale) as u32;
        let (a, b) = (shot.get_pixel(at, cy).0, shot.get_pixel(((16. + 22. + 8. + 11.) * scale) as u32, cy).0);
        ctl.0 += usize::from(ctl_last.is_some_and(|(la, _)| la != a));
        ctl.1 += usize::from(ctl_last.is_some_and(|(_, lb)| lb != b));
        ctl_last = Some((a, b));
    }
    println!("fps {:.1} ({changes} frame changes in {:.1} s)", changes as f32 / t0.elapsed().as_secs_f32(), t0.elapsed().as_secs_f32());
    println!("control img without id: {} frame changes, with id: {}", ctl.0, ctl.1);

    let cpu = || -> f64 {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
        let f: Vec<&str> = stat.rsplit_once(')').map_or("", |(_, r)| r).split_whitespace().collect();
        let ticks = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
        (ticks(11) + ticks(12)) / 100.
    };
    for (label, play) in [("playing", true), ("still", false)] {
        PLAY.store(play, std::sync::atomic::Ordering::Relaxed);
        let (c0, t0) = (cpu(), std::time::Instant::now());
        while t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(16));
            cx.advance_clock(Duration::from_millis(16));
            cx.update_window(win.into(), |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        println!("cpu {label}: {:.1} % of one core", (cpu() - c0) / t0.elapsed().as_secs_f64() * 100.);
    }
    Ok(())
}
