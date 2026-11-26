use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AnyElement, App, Corners, DevicePixels, Hsla, ImageCacheError, IntoElement, ParentElement, PresentTransitionExt, RenderImage, Styled, SvgRenderer, SvgSize, Window, img, px, size};

use crate::icons::lottie_icons::{self, Anim, Lottie};

pub fn action(kind: Lottie, on: bool) -> Option<&'static str> {
    Some(match kind {
        Lottie::Mute => {
            if on {
                "unmute"
            } else {
                "mute"
            }
        }
        Lottie::Deafen => {
            if on {
                "undeafen"
            } else {
                "deafen"
            }
        }
        Lottie::Camera | Lottie::Stream | Lottie::Krisp => {
            if on {
                "disable"
            } else {
                "enable"
            }
        }
        _ => return None,
    })
}

pub fn hover_marker(kind: Lottie, on: bool) -> &'static str {
    match kind {
        Lottie::Mute => {
            if on {
                "hover_muted"
            } else {
                "hover_unmuted"
            }
        }
        Lottie::Deafen => {
            if on {
                "hover_deafened"
            } else {
                "hover_undeafened"
            }
        }
        Lottie::Camera | Lottie::Stream | Lottie::Krisp => {
            if on {
                "hover_enabled"
            } else {
                "hover_disabled"
            }
        }
        Lottie::Activities | Lottie::Gif | Lottie::Sticker => "hover",
        Lottie::Settings | Lottie::Disconnect | Lottie::Soundboard | Lottie::Gift => "all",
    }
}

pub fn click_marker(kind: Lottie) -> Option<&'static str> {
    match kind {
        Lottie::Activities | Lottie::Gif | Lottie::Sticker => Some("click"),
        Lottie::Soundboard => Some("all"),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub kind: Lottie,
    from: f64,
    to: f64,
    at: Instant,
    held: Option<f64>,
    last: Option<&'static str>,
    hovered: bool,
    on: bool,
}

impl Player {
    pub fn new(kind: Lottie, anim: &Anim, on: bool) -> Self {
        let rest = action(kind, on).and_then(|a| anim.marker(a)).map_or(0., |m| m.start);
        Player { kind, from: rest, to: rest, at: Instant::now(), held: Some(rest), last: None, hovered: false, on }
    }

    pub fn play(&mut self, anim: &Anim, name: &'static str, at: Instant, reduced: bool) {
        let Some(m) = anim.marker(name) else { return };
        self.last = Some(name);
        if reduced {
            self.held = Some(m.end());
            return;
        }
        self.from = m.start;
        self.to = (m.end() - 1.).max(m.start);
        self.at = at;
        self.held = None;
    }

    pub fn stop_if_playing(&mut self, anim: &Anim, name: &'static str, reduced: bool) {
        if reduced || self.last != Some(name) {
            return;
        }
        if let Some(m) = anim.marker(name) {
            self.held = Some(m.start);
        }
    }

    pub fn hover(&mut self, anim: &Anim, over: bool, at: Instant, reduced: bool) {
        if over == self.hovered {
            return;
        }
        self.hovered = over;
        let name = hover_marker(self.kind, self.on);
        if over {
            self.play(anim, name, at, reduced);
        } else {
            self.stop_if_playing(anim, name, reduced);
        }
    }

    pub fn toggle(&mut self, anim: &Anim, on: bool, at: Instant, reduced: bool) {
        if on == self.on {
            return;
        }
        let old = action(self.kind, self.on);
        self.on = on;
        if let Some(a) = old {
            self.play(anim, a, at, reduced);
        }
    }

    pub fn click(&mut self, anim: &Anim, at: Instant, reduced: bool) {
        if let Some(c) = click_marker(self.kind) {
            self.play(anim, c, at, reduced);
        }
    }

    pub fn frame(&self, anim: &Anim, now: Instant) -> f64 {
        match self.held {
            Some(f) => f,
            None => (self.from + now.saturating_duration_since(self.at).as_secs_f64() * anim.fps).min(self.to),
        }
    }

    pub fn playing(&self, anim: &Anim, now: Instant) -> bool {
        self.held.is_none() && self.frame(anim, now) < self.to
    }

    pub fn segment(&self) -> Option<Segment> {
        self.held.is_none().then_some(Segment { from: self.from, to: self.to, at: self.at })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub from: f64,
    pub to: f64,
    pub at: Instant,
}

impl Segment {
    pub fn frames(&self) -> (i32, i32) {
        let first = self.from.round() as i32;
        (first, (self.to.round() as i32).max(first))
    }

    pub fn phase(&self) -> f32 {
        (self.from - self.from.round()) as f32 + 0.5
    }
}

pub fn sheet_columns(count: u32) -> u32 {
    ((count as f64).sqrt().ceil() as u32).max(1)
}

type FrameCache = (HashMap<FrameKey, Arc<RenderImage>>, VecDeque<FrameKey>);

thread_local! {
    static PLAYERS: RefCell<HashMap<u64, Player>> = RefCell::new(HashMap::new());
    static FROZEN: Cell<Option<Duration>> = const { Cell::new(None) };
    static FRAMES: RefCell<FrameCache> = RefCell::new(Default::default());
}

pub fn freeze(elapsed: Option<Duration>) {
    FROZEN.with(|f| f.set(elapsed));
}

fn now_for(p: &Player) -> Instant {
    FROZEN.with(Cell::get).map_or_else(Instant::now, |d| p.at + d)
}

fn reduced() -> bool {
    crate::ui::settings::effects::reduced_motion()
}

pub fn clicked(key: u64) {
    let Some(kind) = PLAYERS.with(|p| p.borrow().get(&key).map(|p| p.kind)) else { return };
    let Some(anim) = lottie_icons::get(kind) else { return };
    PLAYERS.with(|p| {
        if let Some(pl) = p.borrow_mut().get_mut(&key) {
            pl.click(&anim, Instant::now(), reduced());
        }
    });
}

pub fn frame_of(key: u64) -> Option<f64> {
    PLAYERS.with(|p| {
        let p = p.borrow();
        let pl = p.get(&key)?;
        let anim = lottie_icons::get(pl.kind)?;
        Some(pl.frame(&anim, now_for(pl)))
    })
}

fn step(key: u64, kind: Lottie, anim: &Anim, on: bool) -> (f64, bool, Option<Segment>) {
    let (over, at) = crate::ui::states::state(key).unwrap_or((false, Instant::now()));
    let red = reduced();
    PLAYERS.with(|p| {
        let mut p = p.borrow_mut();
        let pl = p.entry(key).or_insert_with(|| Player::new(kind, anim, on));
        if pl.kind != kind {
            *pl = Player::new(kind, anim, on);
        }
        pl.toggle(anim, on, Instant::now(), red);
        pl.hover(anim, over, at, red);
        let now = now_for(pl);
        let playing = pl.playing(anim, now) && FROZEN.with(Cell::get).is_none();
        (pl.frame(anim, now), playing, pl.segment())
    })
}

const MAX_FRAMES: usize = 1200;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct FrameKey {
    kind: Lottie,
    frame: i32,
    px: u32,
    rgba: u32,
}

fn rgba(c: Hsla) -> u32 {
    let r: gpui::Rgba = c.into();
    let b = |x: f32| (x * 255.).round().clamp(0., 255.) as u32;
    (b(r.r) << 24) | (b(r.g) << 16) | (b(r.b) << 8) | b(r.a)
}

fn hex(c: Hsla) -> (String, f32) {
    let r: gpui::Rgba = c.into();
    let b = |x: f32| (x * 255.).round().clamp(0., 255.) as u32;
    (format!("#{:02x}{:02x}{:02x}", b(r.r), b(r.g), b(r.b)), r.a)
}

pub fn render_frame(anim: &Anim, frame: f64, px_size: u32, color: Hsla, cx: &App) -> Option<Arc<RenderImage>> {
    render_frame_with(&cx.svg_renderer(), anim, frame, px_size, color)
}

fn render_frame_with(r: &SvgRenderer, anim: &Anim, frame: f64, px_size: u32, color: Hsla) -> Option<Arc<RenderImage>> {
    let (rgb, alpha) = hex(color);
    let svg = crate::media::lottie::frame_svg_tinted(&anim.root, frame, px_size.max(1), Some((&rgb, alpha)));
    let parsed = r.parse_svg(svg.as_bytes()).ok()?;
    let d = DevicePixels(px_size.max(1) as i32);
    r.render_parsed(&parsed, SvgSize::ExactSize(size(d, d))).ok()
}

fn cached(kind: Lottie, anim: &Anim, frame: f64, logical: f32, color: Hsla, window: &mut Window, cx: &mut App) -> Option<Arc<RenderImage>> {
    let px_size = (logical * window.scale_factor()).round().max(1.) as u32;
    let key = FrameKey { kind, frame: frame.round() as i32, px: px_size, rgba: rgba(color) };
    if let Some(i) = FRAMES.with(|f| f.borrow().0.get(&key).cloned()) {
        return Some(i);
    }
    let made = render_frame(anim, key.frame as f64, px_size, color, cx)?;
    let evicted = FRAMES.with(|f| {
        let mut f = f.borrow_mut();
        f.0.insert(key, made.clone());
        f.1.push_back(key);
        let mut out = Vec::new();
        while f.1.len() > MAX_FRAMES {
            if let Some(old) = f.1.pop_front()
                && let Some(i) = f.0.remove(&old)
            {
                out.push(i);
            }
        }
        out
    });
    for i in evicted {
        cx.drop_image(i, Some(window));
    }
    Some(made)
}

pub fn build_sheet(r: &SvgRenderer, anim: &Anim, frames: (i32, i32), px_size: u32, color: Hsla) -> Option<Arc<RenderImage>> {
    let count = (frames.1 - frames.0 + 1).max(1) as u32;
    let columns = sheet_columns(count);
    let rows = count.div_ceil(columns);
    let cell = px_size.max(1);
    let mut sheet = image::RgbaImage::new(columns * cell, rows * cell);
    for i in 0..count {
        let frame = render_frame_with(r, anim, f64::from(frames.0 + i as i32), cell, color)?;
        let bytes = frame.as_bytes(0)?;
        let s = frame.size(0);
        let (w, h) = ((s.width.0.max(0) as u32).min(cell), (s.height.0.max(0) as u32).min(cell));
        let (x0, y0) = ((i % columns) * cell, (i / columns) * cell);
        for y in 0..h {
            let src = (y * s.width.0.max(0) as u32 * 4) as usize;
            let dst = (((y0 + y) * columns * cell + x0) * 4) as usize;
            let len = (w * 4) as usize;
            if let (Some(from), Some(to)) = (bytes.get(src..src + len), sheet.get_mut(dst..dst + len)) {
                to.copy_from_slice(from);
            }
        }
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(sheet)])))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct SheetKey {
    kind: Lottie,
    first: i32,
    last: i32,
    px: u32,
    rgba: u32,
}

const MAX_SHEETS: usize = 48;

type SheetCache = (HashMap<SheetKey, Option<Arc<RenderImage>>>, VecDeque<SheetKey>);

thread_local! {
    static SHEETS: RefCell<SheetCache> = RefCell::new(Default::default());
}

fn per_frame() -> bool {
    static PER_FRAME: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PER_FRAME.get_or_init(|| std::env::var_os("PHX_LOTTIE_PER_FRAME").is_some())
}

fn sheet(kind: Lottie, anim: &Arc<Anim>, frames: (i32, i32), px_size: u32, color: Hsla, cx: &mut App) -> Option<Arc<RenderImage>> {
    let key = SheetKey { kind, first: frames.0, last: frames.1, px: px_size, rgba: rgba(color) };
    if let Some(found) = SHEETS.with(|s| s.borrow().0.get(&key).cloned()) {
        return found;
    }
    SHEETS.with(|s| {
        let mut s = s.borrow_mut();
        s.0.insert(key, None);
        s.1.push_back(key);
    });
    let (r, anim) = (cx.svg_renderer(), anim.clone());
    let task = cx.background_executor().spawn(async move { build_sheet(&r, &anim, frames, px_size, color) });
    cx.spawn(async move |cx| {
        let made = task.await;
        cx.update(|cx| {
            let evicted = SHEETS.with(|s| {
                let mut s = s.borrow_mut();
                match made {
                    Some(img) => {
                        s.0.insert(key, Some(img));
                    }
                    None => {
                        s.0.remove(&key);
                        s.1.retain(|k| *k != key);
                    }
                }
                let mut out = Vec::new();
                while s.1.len() > MAX_SHEETS {
                    if let Some(old) = s.1.pop_front()
                        && let Some(Some(img)) = s.0.remove(&old)
                    {
                        out.push(img);
                    }
                }
                out
            });
            for img in evicted {
                cx.drop_image(img, None);
            }
        });
    })
    .detach();
    None
}

fn segment_frames(anim: &Anim, name: &str) -> Option<(i32, i32)> {
    let m = anim.marker(name)?;
    Some(Segment { from: m.start, to: (m.end() - 1.).max(m.start), at: Instant::now() }.frames())
}

fn warm(kind: Lottie, anim: &Arc<Anim>, on: bool, px_size: u32, hover: Hsla, cx: &mut App) {
    let names = [Some(hover_marker(kind, on)), action(kind, on), click_marker(kind)];
    for name in names.into_iter().flatten() {
        if let Some(frames) = segment_frames(anim, name) {
            sheet(kind, anim, frames, px_size, hover, cx);
        }
    }
}

pub fn icon(key: u64, kind: Lottie, on: bool, logical: f32, colors: (Hsla, Hsla), fallback: impl FnOnce() -> AnyElement) -> AnyElement {
    let Some(anim) = lottie_icons::get(kind) else { return fallback() };
    let (frame, playing, segment) = step(key, kind, &anim, on);
    let color = if crate::ui::states::hovered(key) { colors.1 } else { colors.0 };
    if per_frame() {
        return img(move |window: &mut Window, cx: &mut App| -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
            if playing {
                window.request_animation_frame();
            }
            cached(kind, &anim, frame, logical, color, window, cx).map(Ok)
        })
        .size(px(logical))
        .flex_none()
        .into_any_element();
    }
    let frozen = FROZEN.with(Cell::get).is_some();
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let px_size = (logical * window.scale_factor()).round().max(1.) as u32;
            if !frozen {
                warm(kind, &anim, on, px_size, colors.1, cx);
            }
            if let Some(seg) = segment.filter(|_| playing) {
                let frames = seg.frames();
                if let Some(sheet) = sheet(kind, &anim, frames, px_size, color, cx) {
                    let count = (frames.1 - frames.0 + 1) as u32;
                    window
                        .paint_image_sequence(bounds, Corners::default(), sheet, sheet_columns(count), count, seg.at, anim.fps as f32, seg.phase())
                        .ok();
                    return;
                }
                window.request_animation_frame();
            }
            if let Some(i) = cached(kind, &anim, frame, logical, color, window, cx) {
                window.paint_image(bounds, bounds, Corners::default(), i, 0, false).ok();
            }
        },
    )
    .size(px(logical))
    .flex_none()
    .into_any_element()
}

pub const SPRING_TENSION: f64 = 800.;
pub const SPRING_FRICTION: f64 = 24.;
pub const EMOJI_SCALE: f32 = 1.14;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    target: f64,
    p0: f64,
    v0: f64,
    at: Instant,
}

impl Spring {
    pub fn new(at: Instant) -> Self {
        Spring { target: 0., p0: 0., v0: 0., at }
    }

    fn eval(&self, now: Instant) -> (f64, f64) {
        let t = now.saturating_duration_since(self.at).as_secs_f64();
        let w0 = SPRING_TENSION.sqrt();
        let zeta = SPRING_FRICTION / (2. * w0);
        let a = zeta * w0;
        let wd = w0 * (1. - zeta * zeta).sqrt();
        let (x0, v0) = (self.p0 - self.target, self.v0);
        let b = (v0 + a * x0) / wd;
        let e = (-a * t).exp();
        let (c, s) = ((wd * t).cos(), (wd * t).sin());
        let x = e * (x0 * c + b * s);
        let v = e * (-a * (x0 * c + b * s) + (-x0 * wd * s + b * wd * c));
        (self.target + x, v)
    }

    pub fn value(&self, now: Instant) -> f64 {
        self.eval(now).0
    }

    pub fn retarget(&mut self, target: f64, now: Instant, reduced: bool) {
        if target == self.target {
            return;
        }
        let (p, v) = if reduced { (target, 0.) } else { self.eval(now) };
        *self = Spring { target, p0: p, v0: v, at: now };
    }

    pub fn resting(&self, now: Instant) -> bool {
        let (x, v) = self.eval(now);
        (x - self.target).abs() < 1e-3 && v.abs() < 1e-2
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmojiFace {
    pub face: u32,
    pub hovered: bool,
    pub spring: Spring,
}

impl EmojiFace {
    pub fn new(at: Instant) -> Self {
        EmojiFace { face: crate::icons::emoji_sprite::REST_FACE, hovered: false, spring: Spring::new(at) }
    }

    pub fn hover(&mut self, over: bool, active: bool, at: Instant, pick: impl FnOnce() -> u32, reduced: bool) {
        let was = self.hovered || active;
        self.hovered = over;
        let lit = over || active;
        if lit && !was {
            self.face = pick() % crate::icons::emoji_sprite::FACES;
        }
        self.spring.retarget(if lit { 1. } else { 0. }, at, reduced);
    }

    pub fn scale(&self, now: Instant) -> f32 {
        1. + (EMOJI_SCALE - 1.) * self.spring.value(now) as f32
    }
}

thread_local! {
    static FACES: RefCell<HashMap<u64, EmojiFace>> = RefCell::new(HashMap::new());
    static CELLS: RefCell<HashMap<(u32, bool, u32), Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

fn random_face() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    (h.finish() % u64::from(crate::icons::emoji_sprite::FACES)) as u32
}

fn cell_image(face: u32, colored: bool, tint: Hsla) -> Option<Arc<RenderImage>> {
    let key = (face, colored, if colored { 0 } else { rgba(tint) });
    if let Some(i) = CELLS.with(|c| c.borrow().get(&key).cloned()) {
        return Some(i);
    }
    let sprite = crate::icons::emoji_sprite::get()?;
    let mut cell = crate::icons::emoji_sprite::cell(if colored { &sprite.colored } else { &sprite.mask }, face);
    let t: gpui::Rgba = tint.into();
    let b = |x: f32| (x * 255.).round().clamp(0., 255.) as u8;
    for p in cell.pixels_mut() {
        if colored {
            p.0.swap(0, 2);
        } else {
            let a = (p.0[3] as f32 * t.a).round() as u8;
            p.0 = [b(t.b), b(t.g), b(t.r), a];
        }
    }
    let img = Arc::new(RenderImage::new(vec![image::Frame::new(cell)]));
    CELLS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 256 {
            c.clear();
        }
        c.insert(key, img.clone());
    });
    Some(img)
}

pub fn emoji_button(key: u64, active: bool, logical: f32, tint: Hsla, fallback: impl FnOnce() -> AnyElement) -> AnyElement {
    if crate::icons::emoji_sprite::get().is_none() {
        return fallback();
    }
    let (over, at) = crate::ui::states::state(key).unwrap_or((false, Instant::now()));
    let red = reduced();
    let st = FACES.with(|f| {
        let mut f = f.borrow_mut();
        let e = f.entry(key).or_insert_with(|| EmojiFace::new(at));
        if e.hovered != over || (e.spring.target > 0.5) != (over || active) {
            e.hover(over, active, at, random_face, red);
        }
        *e
    });
    let frozen = FROZEN.with(Cell::get);
    let now = frozen.map_or_else(Instant::now, |d| st.spring.at + d);
    let lit = over || active;
    let moving = !st.spring.resting(now) && frozen.is_none();
    let scale = if moving { 1. + (EMOJI_SCALE - 1.) * st.spring.target as f32 } else { st.scale(now) };
    let face = st.face;
    let size = logical * scale;
    let off = (logical - size) / 2.;
    let t = moving.then(|| emoji_transition(&st.spring, scale));
    gpui::div()
        .relative()
        .size(px(logical))
        .flex_none()
        .child(
            img(move |_: &mut Window, _: &mut App| -> Option<Result<Arc<RenderImage>, ImageCacheError>> { cell_image(face, lit, tint).map(Ok) })
                .absolute()
                .left(px(off))
                .top(px(off))
                .size(px(size))
                .present_transition(gpui::ElementId::NamedInteger("emoji-spring".into(), key), t),
        )
        .into_any_element()
}

pub fn emoji_transition(spring: &Spring, to_scale: f32) -> gpui::Transition {
    let from = 1. + (EMOJI_SCALE - 1.) * spring.p0 as f32;
    gpui::Transition::spring(gpui::SpringConfig::new(SPRING_TENSION as f32, SPRING_FRICTION as f32, 1.))
        .start(spring.at)
        .scale(from / to_scale, 1.)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(kind: Lottie) -> Anim {
        let markers: &[(&str, f64, f64)] = match kind {
            Lottie::Mute => &[("mute", 0., 70.), ("unmute", 100., 70.), ("hover_unmuted", 180., 40.), ("hover_muted", 240., 40.)],
            Lottie::Camera => &[("disable", 0., 70.), ("enable", 100., 70.), ("hover_enabled", 180., 70.), ("hover_disabled", 280., 70.)],
            Lottie::Gif => &[("click", 0., 66.), ("hover", 90., 40.)],
            _ => &[("all", 0., 66.)],
        };
        let m: Vec<_> = markers.iter().map(|(n, s, d)| serde_json::json!({"cm": n, "tm": s, "dr": d})).collect();
        Anim::new(serde_json::json!({"fr": 60, "ip": 0, "op": 400, "w": 24, "h": 24, "layers": [], "markers": m}))
    }

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn settings_gear_plays_once_at_sixty_fps_and_snaps_back() {
        let a = anim(Lottie::Settings);
        let t0 = Instant::now();
        let mut p = Player::new(Lottie::Settings, &a, false);
        assert_eq!(p.frame(&a, t0), 0.);
        p.hover(&a, true, t0, false);
        assert!((p.frame(&a, ms(t0, 500)) - 30.).abs() < 1e-6);
        assert!(p.playing(&a, ms(t0, 500)));
        assert!((p.frame(&a, ms(t0, 1050)) - 63.).abs() < 1e-6);
        assert_eq!(p.frame(&a, ms(t0, 1100)), 65.);
        assert_eq!(p.frame(&a, ms(t0, 5000)), 65.);
        assert!(!p.playing(&a, ms(t0, 1100)));
        p.hover(&a, false, ms(t0, 1200), false);
        assert_eq!(p.frame(&a, ms(t0, 1201)), 0.);
        p.hover(&a, true, ms(t0, 2000), false);
        p.hover(&a, false, ms(t0, 2250), false);
        assert_eq!(p.frame(&a, ms(t0, 2260)), 0.);
    }

    #[test]
    fn mute_hover_segment_depends_on_state() {
        let a = anim(Lottie::Mute);
        let t0 = Instant::now();
        let mut p = Player::new(Lottie::Mute, &a, false);
        assert_eq!(p.frame(&a, t0), 0.);
        p.hover(&a, true, t0, false);
        assert_eq!(p.frame(&a, t0), 180.);
        assert!((p.frame(&a, ms(t0, 250)) - 195.).abs() < 1e-6);
        assert_eq!(p.frame(&a, ms(t0, 2000)), 219.);
        p.hover(&a, false, ms(t0, 2000), false);
        assert_eq!(p.frame(&a, ms(t0, 2001)), 180.);
        let mut q = Player::new(Lottie::Mute, &a, true);
        assert_eq!(q.frame(&a, t0), 100.);
        q.hover(&a, true, t0, false);
        assert_eq!(q.frame(&a, t0), 240.);
    }

    #[test]
    fn toggling_plays_the_old_action_and_survives_leave() {
        let a = anim(Lottie::Mute);
        let t0 = Instant::now();
        let mut p = Player::new(Lottie::Mute, &a, false);
        p.hover(&a, true, t0, false);
        p.toggle(&a, true, ms(t0, 300), false);
        assert_eq!(p.frame(&a, ms(t0, 300)), 0.);
        assert!((p.frame(&a, ms(t0, 800)) - 30.).abs() < 1e-6);
        assert_eq!(p.frame(&a, ms(t0, 3000)), 69.);
        p.hover(&a, false, ms(t0, 3000), false);
        assert_eq!(p.frame(&a, ms(t0, 3001)), 69.);
        p.hover(&a, true, ms(t0, 4000), false);
        assert_eq!(p.frame(&a, ms(t0, 4000)), 240.);
        p.toggle(&a, false, ms(t0, 4100), false);
        assert_eq!(p.frame(&a, ms(t0, 4100)), 100.);
        assert_eq!(p.frame(&a, ms(t0, 9000)), 169.);
    }

    #[test]
    fn camera_maps_enable_to_hover_disabled() {
        let a = anim(Lottie::Camera);
        let t0 = Instant::now();
        let mut off = Player::new(Lottie::Camera, &a, false);
        assert_eq!(off.frame(&a, t0), 100.);
        off.hover(&a, true, t0, false);
        assert_eq!(off.frame(&a, t0), 280.);
        assert_eq!(off.frame(&a, ms(t0, 5000)), 349.);
        let mut on = Player::new(Lottie::Camera, &a, true);
        assert_eq!(on.frame(&a, t0), 0.);
        on.hover(&a, true, t0, false);
        assert_eq!(on.frame(&a, t0), 180.);
    }

    #[test]
    fn click_plays_its_own_segment_and_leave_keeps_it() {
        let a = anim(Lottie::Gif);
        let t0 = Instant::now();
        let mut p = Player::new(Lottie::Gif, &a, false);
        assert_eq!(p.frame(&a, t0), 0.);
        p.hover(&a, true, t0, false);
        assert_eq!(p.frame(&a, ms(t0, 100)), 96.);
        p.click(&a, ms(t0, 1000), false);
        assert_eq!(p.frame(&a, ms(t0, 1000)), 0.);
        assert!((p.frame(&a, ms(t0, 1500)) - 30.).abs() < 1e-6);
        p.hover(&a, false, ms(t0, 1600), false);
        assert!((p.frame(&a, ms(t0, 1600)) - 36.).abs() < 1e-6);
        assert_eq!(p.frame(&a, ms(t0, 4000)), 65.);
    }

    #[test]
    fn reduced_motion_jumps_to_the_segment_end() {
        let a = anim(Lottie::Mute);
        let t0 = Instant::now();
        let mut p = Player::new(Lottie::Mute, &a, false);
        p.hover(&a, true, t0, true);
        assert_eq!(p.frame(&a, t0), 220.);
        assert!(!p.playing(&a, t0));
        p.hover(&a, false, ms(t0, 10), true);
        assert_eq!(p.frame(&a, ms(t0, 10)), 220.);
        p.toggle(&a, true, ms(t0, 20), true);
        assert_eq!(p.frame(&a, ms(t0, 20)), 70.);
    }

    #[test]
    fn emoji_spring_overshoots_then_settles() {
        let t0 = Instant::now();
        let mut e = EmojiFace::new(t0);
        assert_eq!(e.face, 50);
        e.hover(true, false, t0, || 80, false);
        assert_eq!(e.face, 3);
        assert!((e.scale(t0) - 1.).abs() < 1e-6);
        let peak = (0..60).map(|n| e.scale(ms(t0, n * 5))).fold(0f32, f32::max);
        assert!(peak > 1.16 && peak < 1.18, "{peak}");
        assert!((e.scale(ms(t0, 600)) - 1.14).abs() < 1e-3);
        assert!(e.spring.resting(ms(t0, 900)));
        e.hover(false, false, ms(t0, 1000), || 7, false);
        assert_eq!(e.face, 3);
        assert!((e.scale(ms(t0, 1000)) - 1.14).abs() < 1e-3);
        assert!((e.scale(ms(t0, 1700)) - 1.).abs() < 1e-3);
        e.hover(true, false, ms(t0, 2000), || 7, true);
        assert_eq!(e.face, 7);
        assert!((e.scale(ms(t0, 2000)) - 1.14).abs() < 1e-6);
    }

    #[test]
    fn emoji_spring_reverses_mid_flight_from_its_velocity() {
        let t0 = Instant::now();
        let mut s = Spring::new(t0);
        s.retarget(1., t0, false);
        let mid = s.value(ms(t0, 40));
        s.retarget(0., ms(t0, 40), false);
        assert!((s.value(ms(t0, 40)) - mid).abs() < 1e-9);
        assert!(s.value(ms(t0, 60)) > mid);
        assert!(s.value(ms(t0, 800)).abs() < 1e-3);
    }

    #[test]
    fn sheet_cells_follow_the_player_frames() {
        let a = anim(Lottie::Mute);
        let t0 = Instant::now();
        let mut p = Player::new(Lottie::Mute, &a, false);
        assert_eq!(p.segment(), None);
        p.hover(&a, true, t0, false);
        let seg = p.segment().expect("playing");
        let (first, last) = seg.frames();
        assert_eq!((first, last), (180, 219));
        let count = (last - first + 1) as u32;
        assert_eq!(sheet_columns(count), 7);
        let seq = gpui::ImageSequence {
            start: gpui::gradient_clock(seg.at),
            fps: a.fps as f32,
            phase: seg.phase(),
            count,
            columns: sheet_columns(count),
            cell: size(DevicePixels(30), DevicePixels(30)),
        };
        for n in [0u64, 7, 16, 100, 333, 650, 2000] {
            let now = ms(t0, n);
            assert_eq!(seq.index(gpui::gradient_clock(now)) as i32 + first, p.frame(&a, now).round() as i32, "{n} ms");
        }
        p.hover(&a, false, ms(t0, 2000), false);
        assert_eq!(p.segment(), None);
    }

    #[test]
    fn marker_names_follow_the_client_hooks() {
        assert_eq!(action(Lottie::Deafen, true), Some("undeafen"));
        assert_eq!(hover_marker(Lottie::Deafen, false), "hover_undeafened");
        assert_eq!(hover_marker(Lottie::Stream, true), "hover_enabled");
        assert_eq!(hover_marker(Lottie::Krisp, false), "hover_disabled");
        assert_eq!(click_marker(Lottie::Soundboard), Some("all"));
        assert_eq!(click_marker(Lottie::Settings), None);
        assert_eq!(action(Lottie::Gift, true), None);
    }
}
