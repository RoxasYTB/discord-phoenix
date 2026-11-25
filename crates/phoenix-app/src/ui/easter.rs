use std::cell::RefCell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, img, prelude::*, px};
use phoenix_voice::audio::effects::Handle;

use crate::app::PhoenixApp;
use crate::sounds::assets::asset_file;

const D: f32 = 0.15;

const RAYS: [(f32, bool); 14] = [
    (90., true),
    (82., false),
    (67., true),
    (52., false),
    (37., true),
    (22., false),
    (7., true),
    (-8., false),
    (-23., true),
    (-38., false),
    (-53., true),
    (-68., false),
    (-83., true),
    (-90., false),
];

fn rays(alpha: f32) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| {},
        move |b, (), window, _| {
            if alpha <= 0. {
                return;
            }
            let c = b.center();
            let r = f32::from(b.size.width).hypot(f32::from(b.size.height));
            let at = |deg: f32| {
                let a = deg.to_radians();
                gpui::point(c.x + px(r * a.sin()), c.y - px(r * a.cos()))
            };
            for w in RAYS.windows(2) {
                let (hi, lo) = (w[0].0 - 90., w[1].0 - 90.);
                let col = if w[1].1 { gpui::hsla(0., 0., 1., alpha) } else { gpui::hsla(0., 0., 221. / 255., alpha) };
                for turn in [0., 180.] {
                    let mut pb = gpui::PathBuilder::fill();
                    pb.move_to(c);
                    pb.line_to(at(lo + turn));
                    pb.line_to(at(hi + turn));
                    pb.close();
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, col);
                    }
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

const SYMBOL: &str = "a53fe63d4b568d70.svg";
const PRIMARY: [&str; 4] = ["3e4d5e236f62c4ba.png", "c385ef9dd294be87.png", "ec22204197211dc7.png", "47b5cca448b07ece.png"];
const SECONDARY: [&str; 4] = ["f3cbcdad40b7140d.png", "65e38691f7c05321.png", "3f93966b676ef0fb.png", "2063fc2ef0c778bf.png"];
const ARROWS: [(&str, &str, &str); 4] = [
    ("left", "574907598f87a615.svg", "ddr-left"),
    ("down", "0a9fe5975bf11d56.svg", "ddr-down"),
    ("up", "1930678cb18a10c6.svg", "ddr-up"),
    ("right", "600de8edafaf94b2.svg", "ddr-right"),
];

#[derive(Default)]
struct State {
    keys: Vec<String>,
    demon: Option<(Instant, u64)>,
    arrows: [Option<Instant>; 4],
    lofi: Option<Handle>,
}

thread_local! {
    static S: RefCell<State> = RefCell::default();
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    S.with(|s| f(&mut s.borrow_mut()))
}

pub fn preload() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(preload_now);
}

fn preload_now() {
    for f in PRIMARY.iter().chain(&SECONDARY).chain([&SYMBOL]) {
        let _ = asset_file(f);
    }
    for (_, f, _) in ARROWS {
        let _ = asset_file(f);
    }
    crate::sounds::warm(&["ddr-up", "ddr-down", "ddr-left", "ddr-right"]);
}

pub fn demon_combo(keys: &[String]) -> bool {
    keys.len() >= 5 && keys[keys.len() - 5..].iter().map(String::as_str).eq(["h", "h", "right", "n", "k"])
}

pub fn demon_active() -> bool {
    with(|s| s.demon.is_some())
}

pub fn on_key(_app: &mut PhoenixApp, ks: &gpui::Keystroke, cx: &mut Context<PhoenixApp>) -> bool {
    let m = ks.modifiers;
    if m.control && m.alt && m.shift && ks.key == "w" {
        toggle_lofi();
        return true;
    }
    if !crate::shortcuts::help_open() || demon_active() {
        return demon_active();
    }
    let fire = with(|s| {
        s.keys.push(ks.key.clone());
        let n = s.keys.len();
        if n > 5 {
            s.keys.drain(..n - 5);
        }
        demon_combo(&s.keys)
    });
    if fire && !crate::ui::settings::effects::reduced_motion() {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64) | 1;
        with(|s| {
            s.demon = Some((Instant::now(), seed));
            s.keys.clear();
        });
        cx.notify();
        return true;
    }
    if m.control || m.alt || m.shift {
        return false;
    }
    if let Some(i) = ARROWS.iter().position(|(k, _, _)| *k == ks.key) {
        let now = Instant::now();
        let play = with(|s| {
            let ok = s.arrows[i].is_none_or(|t| now.duration_since(t) >= Duration::from_millis(100));
            s.arrows[i] = Some(now);
            ok
        });
        if play {
            crate::sounds::play(ARROWS[i].2, 1.);
        }
        cx.notify();
        return true;
    }
    false
}

pub fn toggle_lofi() {
    match with(|s| s.lofi.take()) {
        Some(h) => fade(h, 0.5, 0., true),
        None => {
            if let Some(h) = crate::sounds::play_loop("vibing_wumpus", 0.) {
                with(|s| s.lofi = Some(h.clone()));
                fade(h, 0., 0.5, false);
            }
        }
    }
}

fn fade(h: Handle, from: f32, to: f32, stop: bool) {
    let _ = std::thread::Builder::new().name("lofi-fade".into()).spawn(move || {
        let step = 0.2 * (to - from);
        let mut v = from;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            v = (v + step).clamp(0., 0.5);
            h.set_volume(v);
            if (step >= 0. && v >= to) || (step < 0. && v <= to) {
                break;
            }
        }
        if stop {
            h.stop();
        }
    });
}

fn rng(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 40) as f32 / (1u64 << 24) as f32
}

fn op(p: f32, a: f32, b: f32) -> f32 {
    if p <= 0. || p >= 1. {
        0.
    } else if p < a {
        p / a
    } else if p <= b {
        1.
    } else {
        1. - (p - b) / (1. - b)
    }
}

fn ease_out(p: f32) -> f32 {
    1. - (1. - p.clamp(0., 1.)).powi(2)
}

struct Part {
    file: &'static str,
    rect: (f32, f32, f32, f32),
    origin: Option<(f32, f32)>,
    anim: (f32, f32),
    scale: (f32, f32),
    plateau: (f32, f32),
}

fn part(p: &Part, t: f32, k: f32, base: (f32, f32)) -> Option<AnyElement> {
    let (dur, delay) = (p.anim.0 * D, p.anim.1 * D);
    let q = (t - delay) / dur;
    if !(0. ..1.).contains(&q) {
        return None;
    }
    let path = asset_file(p.file)?;
    let s = p.scale.0 + (p.scale.1 - p.scale.0) * ease_out(q);
    let (x, y, w, h) = p.rect;
    let (ox, oy) = p.origin.unwrap_or((w / 2., h / 2.));
    let (nx, ny) = (x + ox * (1. - s), y + oy * (1. - s));
    Some(
        img(path)
            .absolute()
            .left(px(base.0 + nx * k))
            .top(px(base.1 + ny * k))
            .w(px(w * s * k))
            .h(px(h * s * k))
            .opacity(op(q, p.plateau.0, p.plateau.1))
            .into_any_element(),
    )
}

fn primary() -> [Part; 4] {
    [
        Part { file: PRIMARY[0], rect: (43., 69., 70., 69.), origin: None, anim: (2., 1.), scale: (0.2, 0.8), plateau: (0.08, 0.7) },
        Part { file: PRIMARY[1], rect: (30., 56., 96., 95.), origin: None, anim: (3., 0.), scale: (0.2, 0.9), plateau: (0.08, 0.7) },
        Part { file: PRIMARY[2], rect: (0., 59., 183., 104.), origin: Some((77., 43.)), anim: (2.5, 1.5), scale: (0., 1.), plateau: (0.2, 0.5) },
        Part { file: PRIMARY[3], rect: (71., 0., 69., 180.), origin: Some((3., 110.)), anim: (2., 1.), scale: (0., 1.), plateau: (0.3, 0.5) },
    ]
}

fn secondary() -> [Part; 4] {
    [
        Part { file: SECONDARY[0], rect: (58., 130., 61., 58.), origin: Some((30.5, 27.5)), anim: (2., 1.), scale: (0.2, 0.8), plateau: (0.08, 0.7) },
        Part { file: SECONDARY[1], rect: (46., 115., 85., 85.), origin: None, anim: (3., 0.), scale: (0.2, 0.9), plateau: (0.08, 0.7) },
        Part { file: SECONDARY[2], rect: (19., 68., 162., 173.), origin: Some((70., 91.)), anim: (2.5, 1.5), scale: (0., 1.), plateau: (0.2, 0.5) },
        Part { file: SECONDARY[3], rect: (5., 6., 156., 306.), origin: Some((87., 150.)), anim: (2., 1.), scale: (0., 1.), plateau: (0.3, 0.5) },
    ]
}

const SHOW_MS: f32 = 1800.;
const SYMBOL_MS: f32 = 1800. + 240. * 8. + 750.;
const CLOSE_MS: f32 = SYMBOL_MS + 3000.;

pub fn shake_offset(window: &mut gpui::Window) -> (f32, f32) {
    let Some((start, seed)) = with(|s| s.demon) else { return (0., 0.) };
    let ms = start.elapsed().as_secs_f32() * 1000.;
    if !(SHOW_MS..SHOW_MS + 2400.).contains(&ms) {
        return (0., 0.);
    }
    window.request_animation_frame();
    let frame = ((ms - SHOW_MS) / 16.7) as u64;
    let mut r = seed ^ frame.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let dir = if (frame / 2).is_multiple_of(2) { -1. } else { 1. };
    (dir * 5., (rng(&mut r) * 2. - 1.) * 5.)
}

pub fn render_demon(window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (start, seed) = with(|s| s.demon)?;
    let ms = start.elapsed().as_secs_f32() * 1000.;
    if ms >= CLOSE_MS + 500. {
        with(|s| s.demon = None);
        crate::shortcuts::on_escape();
        cx.notify();
        return None;
    }
    window.request_animation_frame();
    let vp = window.viewport_size();
    let (cw, ch) = (f32::from(vp.width), f32::from(vp.height));
    let backdrop = if ms < CLOSE_MS { (ms / 1000.).min(1.) } else { 1. - ((ms - CLOSE_MS) / 500.).min(1.) };
    let mut layer = div().absolute().top_0().left_0().size_full();
    if (SHOW_MS..SYMBOL_MS).contains(&ms) {
        for i in 0..8u64 {
            let born = SHOW_MS + 240. * i as f32;
            let t = (ms - born) / 1000.;
            if !(0. ..1.).contains(&t) {
                continue;
            }
            let mut r = seed.wrapping_add(i.wrapping_mul(0x2545_f491_4f6c_dd1d)) | 1;
            let top = ch / 2. - 100. + rng(&mut r) * 200.;
            let left = cw / 2. - 200. + rng(&mut r) * 400.;
            let k = 0.6 + rng(&mut r) * 0.4;
            let (dx, dy) = (rng(&mut r) * 140. - 70., rng(&mut r) * 140. - 70.);
            for p in primary() {
                layer = layer.children(part(&p, t, k, (left - 77. * k, top - 102. * k)));
            }
            if t >= 0.12 {
                for p in secondary() {
                    layer = layer.children(part(&p, t - 0.12, k, (left + (dx - 88.) * k, top + (dy - 156.) * k)));
                }
            }
        }
    }
    if (SYMBOL_MS..CLOSE_MS).contains(&ms)
        && let Some(path) = asset_file(SYMBOL)
    {
        let q = (ms - SYMBOL_MS) / 3000.;
        let (o, s) = if q < 0.05 { (q / 0.15, 0.8 + 0.25 * (q / 0.05)) } else if q < 0.15 { (q / 0.15, 1.05 - 0.05 * ((q - 0.05) / 0.1)) } else { (1., 1.) };
        let bg = if q < 0.05 { q / 0.05 * 0.2 } else if q < 0.15 { 0.2 } else if q < 0.6 { 0.2 * (1. - (q - 0.15) / 0.45) } else { 0. };
        let w = (cw * 0.8).min(ch * 0.6 * 360. / 280.) * s;
        let h = w * 280. / 360.;
        layer = layer
            .child(rays(bg))
            .child(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(img(path).w(px(w)).h(px(h)).opacity(o)));
    }
    Some(
        gpui::deferred(
            div()
                .id("raging-demon")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .child(div().absolute().top_0().left_0().size_full().bg(gpui::hsla(0., 0., 0., 0.5 * backdrop)))
                .child(layer),
        )
        .with_priority(21)
        .into_any_element(),
    )
}

pub fn arrows() -> AnyElement {
    let now = Instant::now();
    let mut row = div().absolute().top(px(22.)).right(px(40.)).flex();
    for (i, (_, file, _)) in ARROWS.iter().enumerate() {
        let since = with(|s| s.arrows[i]).map(|t| now.duration_since(t).as_millis() as u64);
        let y = match since {
            Some(ms) if ms < 100 => -50.,
            Some(ms) if ms < 500 => -100.,
            _ => 0.,
        };
        let cell = div().size(px(50.)).m(px(4.)).overflow_hidden().relative();
        row = row.child(match asset_file(file) {
            Some(p) => cell.child(img(p).absolute().left_0().top(px(y)).w(px(50.)).h(px(150.))),
            None => cell,
        });
    }
    row.into_any_element()
}

pub fn arrows_animating() -> bool {
    let now = Instant::now();
    with(|s| s.arrows.iter().flatten().any(|t| now.duration_since(*t) < Duration::from_millis(520)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_and_keyframes() {
        let k = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(demon_combo(&k(&["x", "h", "h", "right", "n", "k"])));
        assert!(!demon_combo(&k(&["h", "right", "n", "k"])));
        assert!(!demon_combo(&k(&["h", "h", "left", "n", "k"])));
        assert_eq!(op(0.5, 0.08, 0.7), 1.);
        assert!((op(0.04, 0.08, 0.7) - 0.5).abs() < 1e-6);
        assert_eq!(op(1., 0.08, 0.7), 0.);
        assert_eq!(SYMBOL_MS, 4470.);
        assert_eq!(CLOSE_MS, 7470.);
    }
}
