use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, ElementId, FontWeight, Hitbox, HitboxBehavior, Hsla, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderImage, SharedString, StatefulInteractiveElement,
    Styled, Window, div, img, prelude::*, px, svg,
};

use crate::media::video_file::{self, Probe, Session, SessionSpec};
use crate::theme::{c, generated::global, generated::idx, tok, z};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Playing,
    Paused,
    Ended,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    Single,
    Tile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub bar_h: f32,
    pub pad_x: f32,
    pub bottom: f32,
    pub scrim_h: f32,
    pub gap: f32,
    pub mid_gap: f32,
    pub duration_margin: f32,
    pub trailing_w: Option<f32>,
    pub timeline_pad: f32,
    pub timeline_h: f32,
    pub timeline_hover_h: f32,
    pub playhead: f32,
    pub hitbox_offset: f32,
    pub icon: f32,
    pub button_h: f32,
    pub time_px: f32,
    pub volume_w: f32,
    pub volume_ms: f32,
    pub volume_px: f32,
    pub volume_me: f32,
}

pub const ATTACHMENT: Config = Config {
    bar_h: 32.,
    pad_x: 4.,
    bottom: 2.,
    scrim_h: 48.,
    gap: 4.,
    mid_gap: 4.,
    duration_margin: 0.,
    trailing_w: None,
    timeline_pad: 4.,
    timeline_h: 2.,
    timeline_hover_h: 4.,
    playhead: 8.,
    hitbox_offset: -12.,
    icon: 18.,
    button_h: 24.,
    time_px: 12.,
    volume_w: 48.,
    volume_ms: 4.,
    volume_px: 4.,
    volume_me: 6.,
};

pub const FULLSCREEN: Config = Config {
    bar_h: 72.,
    pad_x: 20.,
    bottom: 4.,
    scrim_h: 121.,
    gap: 16.,
    mid_gap: 8.,
    duration_margin: 3.,
    trailing_w: Some(200.),
    timeline_pad: 20.,
    timeline_h: 4.,
    timeline_hover_h: 4.,
    playhead: 12.,
    hitbox_offset: 0.,
    icon: 32.,
    button_h: 32.,
    time_px: 14.,
    volume_w: 100.,
    volume_ms: 6.,
    volume_px: 0.,
    volume_me: 0.,
};

pub const COMPACT_MAX_W: f32 = 400.;
pub const TINY_MAX_W: f32 = 240.;
pub const RATES: [f64; 7] = [0.5, 0.75, 1., 1.25, 1.5, 1.75, 2.];
const IDLE_HIDE: Duration = Duration::from_secs(3);
const LEGACY_IDLE_FULLSCREEN: Duration = Duration::from_secs(1);
const OFFSCREEN: Duration = Duration::from_millis(300);
const VOLUME_HIDE: Duration = Duration::from_millis(150);
const POP_MS: f32 = 1000.;

pub fn format_time(secs: f64) -> String {
    let neg = secs < 0.;
    let n = secs.abs() as u64;
    let (h, m, s) = (n / 3600, n % 3600 / 60, n % 60);
    let sign = if neg { "-" } else { "" };
    if h > 0 { format!("{sign}{h}:{m:02}:{s:02}") } else { format!("{sign}{m}:{s:02}") }
}

pub fn time_width(duration: f64, font_px: f32, compact: bool) -> f32 {
    let i = duration.max(0.).floor() as u64;
    let (h, m) = (i / 3600, i % 3600 / 60);
    let digits = if h > 0 { h.to_string().len() + 4 } else { m.to_string().len() + 2 };
    let s = 0.562 * digits as f32 + (if h > 0 { 2. } else { 1. }) * 0.278;
    if compact { (0.389 + s) * font_px } else { (0.389 + 2. * s + 0.333) * font_px + 8. }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub skip: bool,
    pub inline_volume: bool,
    pub compact_time: bool,
    pub trailing: usize,
    pub speed: bool,
    pub fullscreen: bool,
}

pub fn min_width(cfg: &Config, s: &Shown, duration: f64) -> f32 {
    let i = cfg.icon.max(24.);
    let a = cfg.gap;
    let start = i + if s.skip { 2. * (a + i) } else { 0. };
    let vol = if s.inline_volume { cfg.volume_ms + 2. * cfg.volume_px + cfg.volume_w + cfg.volume_me } else { 0. };
    let mid = a + i + vol + cfg.mid_gap + cfg.duration_margin + time_width(duration, cfg.time_px, s.compact_time) + 8.;
    let u = (s.trailing + usize::from(s.speed) + usize::from(s.fullscreen)) as f32;
    let end = if u > 0. { u * i + (u - 1.) * a } else { 0. };
    (2. * cfg.pad_x + start + mid + end).ceil()
}

pub fn collapses(buttons: usize, can_collapse: bool) -> bool {
    can_collapse && buttons > 1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub skip: bool,
    pub floating_volume: bool,
    pub autohide_volume: bool,
    pub compact_time: bool,
    pub speed: bool,
}

pub fn plan(layout: Layout, portrait: bool, width: f32, duration: f64, fullscreen: bool) -> Plan {
    let single = layout == Layout::Single;
    let e3 = single && portrait;
    let hide_skip = !single || portrait;
    if fullscreen {
        let base = Shown { skip: !hide_skip, inline_volume: true, compact_time: false, trailing: 0, speed: true, fullscreen: true };
        let stages = [base, Shown { inline_volume: false, ..base }, Shown { inline_volume: false, compact_time: true, ..base }];
        let n1 = stages.iter().position(|s| width >= min_width(&FULLSCREEN, s, duration)).unwrap_or(stages.len());
        return Plan {
            skip: !hide_skip && n1 < 3,
            floating_volume: n1 >= 1,
            autohide_volume: !single || portrait,
            compact_time: n1 >= 2,
            speed: true,
        };
    }
    let t7 = width < COMPACT_MAX_W;
    let t9 = width < TINY_MAX_W;
    let floating = e3 || t7;
    let skip = !(hide_skip || t9);
    let speed = !(e3 || t9);
    let shown = Shown { skip, inline_volume: !floating, compact_time: false, trailing: 0, speed, fullscreen: true };
    Plan { skip, floating_volume: floating, autohide_volume: !single || portrait || floating, compact_time: width < min_width(&ATTACHMENT, &shown, duration), speed }
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    from: f32,
    to: f32,
    at: Option<Instant>,
    tension: f32,
    friction: f32,
    clamp: bool,
}

impl Spring {
    pub const fn new(v: f32, tension: f32, friction: f32) -> Self {
        Self { from: v, to: v, at: None, tension, friction, clamp: true }
    }

    pub const fn unclamped(self) -> Self {
        Self { clamp: false, ..self }
    }

    pub fn free(tension: f32, friction: f32, t: f32) -> f32 {
        let w = tension.sqrt();
        let zeta = friction / (2. * w);
        if t * zeta * w > 12. {
            return 1.;
        }
        if zeta >= 1. {
            let root = w * (zeta * zeta - 1.).sqrt();
            let (r1, r2) = (-zeta * w + root, -zeta * w - root);
            if (r2 - r1).abs() < 1e-6 {
                return 1. - (1. + w * t) * (-w * t).exp();
            }
            return 1. - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1);
        }
        let wd = w * (1. - zeta * zeta).sqrt();
        1. - (-zeta * w * t).exp() * ((wd * t).cos() + zeta * w / wd * (wd * t).sin())
    }

    pub fn progress(tension: f32, friction: f32, t: f32) -> f32 {
        let w = tension.sqrt();
        let zeta = friction / (2. * w);
        if zeta >= 1. {
            return (1. - (1. + w * t) * (-w * t).exp()).clamp(0., 1.);
        }
        let wd = w * (1. - zeta * zeta).sqrt();
        let k = zeta * w / wd;
        let cross = (std::f32::consts::PI - (1. / k).atan()) / wd;
        if t >= cross {
            return 1.;
        }
        1. - (-zeta * w * t).exp() * ((wd * t).cos() + k * (wd * t).sin())
    }

    pub fn value(&self, now: Instant) -> f32 {
        let Some(at) = self.at else { return self.to };
        let t = now.saturating_duration_since(at).as_secs_f32();
        let p = if self.clamp { Self::progress(self.tension, self.friction, t) } else { Self::free(self.tension, self.friction, t) };
        self.from + (self.to - self.from) * p
    }

    pub fn moving(&self, now: Instant) -> bool {
        let Some(at) = self.at else { return false };
        if self.clamp { (self.value(now) - self.to).abs() > 1e-4 } else { now.saturating_duration_since(at) < Duration::from_millis(1200) }
    }

    pub fn set(&mut self, to: f32, now: Instant, immediate: bool) {
        if (to - self.to).abs() < 1e-6 {
            return;
        }
        self.from = if immediate { to } else { self.value(now) };
        self.to = to;
        self.at = (!immediate).then_some(now);
    }
}

enum Load {
    Idle,
    Fetching,
    Ready(PathBuf, Probe),
    Failed,
}

struct Seek {
    bounds: Bounds<Pixels>,
    resume: bool,
}

struct Player {
    src: SharedString,
    load: Load,
    active: bool,
    state: State,
    looped: bool,
    session: Option<Session>,
    spawning: bool,
    started: bool,
    position: f64,
    image: Option<Arc<RenderImage>>,
    painted: Option<Instant>,
    shown_at: Option<Bounds<Pixels>>,
    hovered: bool,
    moved: Instant,
    controls: Spring,
    volume_open: Spring,
    volume_hover: bool,
    volume_hide_at: Option<Instant>,
    pop: Option<(Instant, State)>,
    remaining: bool,
    rate: f64,
    want_max: (u32, u32),
    seek: Option<Seek>,
    track_hover: Option<f32>,
    speed_open: bool,
    speed_closed: Option<Instant>,
    volume_drag: Option<Bounds<Pixels>>,
    measured: Option<f32>,
    clicked: bool,
    focus: Option<gpui::FocusHandle>,
    legacy_bar: Spring,
}

impl Player {
    fn new(src: SharedString, looped: bool) -> Self {
        Self {
            src,
            load: Load::Idle,
            active: false,
            state: State::Paused,
            looped,
            session: None,
            spawning: false,
            started: false,
            position: 0.,
            image: None,
            painted: None,
            shown_at: None,
            hovered: false,
            moved: Instant::now(),
            controls: Spring::new(0., 250., 5.),
            volume_open: Spring::new(0., 100., 3.),
            volume_hover: false,
            volume_hide_at: None,
            pop: None,
            remaining: false,
            rate: 1.,
            want_max: (640, 360),
            seek: None,
            track_hover: None,
            speed_open: false,
            speed_closed: None,
            volume_drag: None,
            measured: None,
            clicked: false,
            focus: None,
            legacy_bar: Spring::new(0., 200., 14.).unclamped(),
        }
    }

    fn probe(&self) -> Option<&Probe> {
        match &self.load {
            Load::Ready(_, p) => Some(p),
            _ => None,
        }
    }

    fn duration(&self) -> Option<f64> {
        self.probe().and_then(|p| p.duration)
    }

    fn current(&self) -> f64 {
        let pos = self.session.as_ref().filter(|_| self.started).map_or(self.position, Session::position);
        match self.duration() {
            Some(d) if self.looped && d > 0. => pos % d,
            Some(d) => pos.min(d),
            None => pos,
        }
    }

    fn controls_target(&self, now: Instant) -> bool {
        let idle = self.state == State::Playing && now.duration_since(self.moved) >= IDLE_HIDE;
        self.seek.is_some() || self.speed_open || self.volume_drag.is_some() || ((self.hovered || self.state == State::Ended) && !idle)
    }

    fn loading(&self) -> bool {
        self.state == State::Playing && !(self.started && self.session.is_some())
    }
}

#[derive(Default)]
struct Store {
    players: HashMap<SharedString, Player>,
    volume: Option<f32>,
    muted: bool,
    ticking: bool,
    fullscreen: Option<SharedString>,
    entered_fullscreen: bool,
    fs_width: f32,
    focus: Option<gpui::FocusHandle>,
    silent: bool,
    retired: (Images, Images),
}

type Images = Vec<Arc<RenderImage>>;

impl gpui::Global for Store {}

fn store(cx: &mut App) -> &mut Store {
    cx.default_global::<Store>()
}

fn volume(cx: &App) -> f32 {
    cx.try_global::<Store>().and_then(|s| s.volume).unwrap_or_else(|| super::voice_message::media_volume(cx))
}

fn effective_volume(cx: &App, key: &SharedString) -> f32 {
    let st = cx.try_global::<Store>();
    let looped = st.and_then(|s| s.players.get(key)).is_some_and(|p| p.looped);
    if looped || st.is_some_and(|s| s.silent) {
        0.
    } else if !dvp_enabled() {
        super::voice_message::media_key_volume(cx, &legacy_volume_key(key))
    } else if st.is_some_and(|s| s.muted) {
        0.
    } else {
        volume(cx)
    }
}

pub fn dvp_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PHOENIX_DVP").is_some_and(|v| v != "0"))
}

pub fn legacy_volume_key(key: &SharedString) -> SharedString {
    format!("video-attachment:{key}").into()
}

pub fn supported() -> bool {
    video_file::available()
}

fn retire(cx: &mut App, img: Option<Arc<RenderImage>>) {
    if let Some(i) = img {
        store(cx).retired.0.push(i);
    }
}

fn release(window: &mut Window, cx: &mut App) {
    let aged = {
        let st = store(cx);
        let fresh = std::mem::take(&mut st.retired.0);
        std::mem::replace(&mut st.retired.1, fresh)
    };
    for img in aged {
        cx.drop_image(img, Some(window));
    }
}

fn ensure(cx: &mut App, key: &SharedString, src: &SharedString, looped: bool) {
    store(cx).players.entry(key.clone()).or_insert_with(|| Player::new(src.clone(), looped));
}

fn fetch(cx: &mut App, key: &SharedString) {
    let Some(p) = store(cx).players.get_mut(key) else { return };
    if !matches!(p.load, Load::Idle) {
        return;
    }
    p.load = Load::Fetching;
    let src = p.src.to_string();
    evict(cx, Instant::now());
    video_file::sweep_stale();
    let k = key.clone();
    let http = cx.http_client();
    cx.spawn(async move |cx| {
        let local = src.strip_prefix("local/").map(PathBuf::from);
        let path = match local {
            Some(p) => Some(p),
            None => {
                use futures::AsyncReadExt;
                let mut bytes = Vec::new();
                let ok = match http.get(&src, Default::default(), true).await {
                    Ok(mut r) if r.status().is_success() => r.body_mut().take(video_file::MAX_DOWNLOAD as u64 + 1).read_to_end(&mut bytes).await.is_ok(),
                    _ => false,
                };
                if ok && bytes.len() <= video_file::MAX_DOWNLOAD {
                    let target = video_file::temp_path(&src);
                    cx.background_spawn(async move {
                        std::fs::create_dir_all(video_file::temp_dir()).ok()?;
                        std::fs::write(&target, &bytes).ok()?;
                        Some(target)
                    })
                    .await
                } else {
                    None
                }
            }
        };
        let probed = match path {
            Some(p) => cx.background_spawn(async move { video_file::probe(&p).map(|pr| (p, pr)) }).await,
            None => None,
        };
        cx.update(|cx| {
            if let Some(p) = store(cx).players.get_mut(&k) {
                p.load = probed.map_or(Load::Failed, |(path, pr)| Load::Ready(path, pr));
                if matches!(p.load, Load::Failed) {
                    p.state = State::Paused;
                }
            }
            ensure_ticker(cx);
            cx.refresh_windows();
        });
    })
    .detach();
}

const EVICT_AFTER: Duration = Duration::from_secs(60);

fn evict(cx: &mut App, now: Instant) {
    let stale: Vec<SharedString> = store(cx)
        .players
        .iter()
        .filter(|(_, p)| p.state != State::Playing && !matches!(p.load, Load::Fetching) && p.painted.is_none_or(|t| now.duration_since(t) > EVICT_AFTER))
        .map(|(k, _)| k.clone())
        .collect();
    for k in stale {
        let Some(p) = store(cx).players.remove(&k) else { continue };
        if let Load::Ready(path, _) = &p.load
            && path.starts_with(video_file::temp_dir())
        {
            let _ = std::fs::remove_file(path);
        }
        retire(cx, p.image);
    }
}

fn set_state(cx: &mut App, key: &SharedString, state: State, user: bool) {
    let now = Instant::now();
    let Some(p) = store(cx).players.get_mut(key) else { return };
    if p.state == state {
        return;
    }
    if state == State::Paused || state == State::Ended {
        p.position = p.current();
        if let Some(s) = p.session.as_mut() {
            s.set_paused(true);
        }
    }
    if state == State::Ended {
        p.session = None;
        p.started = false;
        p.position = p.duration().unwrap_or(p.position);
    }
    if state == State::Playing
        && p.started
        && let Some(s) = p.session.as_mut()
    {
        s.set_paused(false);
    }
    if user && state != State::Ended {
        p.pop = Some((now, state));
    }
    p.state = state;
    ensure_ticker(cx);
}

fn restart_session(cx: &mut App, key: &SharedString) {
    if let Some(p) = store(cx).players.get_mut(key) {
        p.position = p.current();
        p.session = None;
        p.started = false;
    }
}

pub fn toggle(cx: &mut App, key: &SharedString) {
    let Some(p) = store(cx).players.get_mut(key) else { return };
    p.active = true;
    let pop = dvp_enabled() || p.clicked;
    p.clicked = true;
    match p.state {
        State::Ended => {
            p.position = 0.;
            p.session = None;
            p.started = false;
            set_state(cx, key, State::Playing, pop);
        }
        State::Playing => set_state(cx, key, State::Paused, true),
        State::Paused => set_state(cx, key, State::Playing, pop),
    }
    fetch(cx, key);
    ensure_ticker(cx);
}

fn seek_to(cx: &mut App, key: &SharedString, secs: f64) {
    let Some(p) = store(cx).players.get_mut(key) else { return };
    let d = p.duration().unwrap_or(0.);
    p.position = secs.clamp(0., d.max(0.));
    p.session = None;
    p.started = false;
    if p.state == State::Ended && p.position < d {
        p.state = State::Paused;
    }
    ensure_ticker(cx);
}

fn skip(cx: &mut App, key: &SharedString, delta: f64) {
    let Some(p) = cx.try_global::<Store>().and_then(|s| s.players.get(key)) else { return };
    let (cur, d, state) = (p.current(), p.duration().unwrap_or(0.), p.state);
    let next = (cur + delta).clamp(0., d);
    seek_to(cx, key, next);
    if delta > 0. && next >= d && state != State::Ended {
        set_state(cx, key, State::Ended, false);
    } else if delta < 0. && state == State::Ended {
        set_state(cx, key, State::Paused, false);
    }
}

fn set_rate(cx: &mut App, key: &SharedString, rate: f64) {
    restart_session(cx, key);
    if let Some(p) = store(cx).players.get_mut(key) {
        p.rate = rate;
    }
    ensure_ticker(cx);
}

fn apply_volume(cx: &mut App) {
    let keys: Vec<SharedString> = store(cx).players.keys().cloned().collect();
    for k in keys {
        let v = effective_volume(cx, &k);
        if let Some(s) = store(cx).players.get(&k).and_then(|p| p.session.as_ref()) {
            s.set_volume(v);
        }
    }
}

fn set_volume(cx: &mut App, v: f32) {
    let v = v.clamp(0., 1.);
    let st = store(cx);
    st.volume = Some(v);
    st.muted = v == 0.;
    super::voice_message::set_media_volume(v, cx);
    apply_volume(cx);
}

fn toggle_mute(cx: &mut App) {
    let v = volume(cx);
    let st = store(cx);
    if st.muted || v == 0. {
        st.muted = false;
        if v == 0. {
            set_volume(cx, 0.3);
            return;
        }
    } else {
        st.muted = true;
    }
    apply_volume(cx);
}

fn shown_volume(cx: &App) -> f32 {
    if cx.try_global::<Store>().is_some_and(|s| s.muted) { 0. } else { volume(cx) }
}

fn set_fullscreen(cx: &mut App, window: &mut Window, key: Option<SharedString>) {
    let st = store(cx);
    let was = st.fullscreen.is_some();
    st.fullscreen = key.clone();
    if let Some(k) = &key {
        if let Some(p) = st.players.get_mut(k) {
            p.hovered = true;
            p.moved = Instant::now();
        }
        let h = cx.focus_handle();
        window.focus(&h, cx);
        store(cx).focus = Some(h);
        if !window.is_fullscreen() {
            window.toggle_fullscreen();
            store(cx).entered_fullscreen = true;
        }
    } else if was {
        st.focus = None;
    }
    let st = store(cx);
    if key.is_none() && was && st.entered_fullscreen {
        st.entered_fullscreen = false;
        if window.is_fullscreen() {
            window.toggle_fullscreen();
        }
    }
    window.refresh();
}

fn ensure_ticker(cx: &mut App) {
    let st = store(cx);
    if st.ticking {
        return;
    }
    st.ticking = true;
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(16)).await;
            if !cx.update(tick) {
                break;
            }
        }
    })
    .detach();
}

fn spawn_session(cx: &mut App, key: &SharedString) {
    let vol = effective_volume(cx, key);
    let silent = store(cx).silent;
    let Some(p) = store(cx).players.get_mut(key) else { return };
    let Load::Ready(path, probe) = &p.load else { return };
    let (path, probe) = (path.clone(), *probe);
    p.spawning = true;
    let (start, rate, max, looped) = (p.position, p.rate, p.want_max, p.looped);
    let k = key.clone();
    cx.spawn(async move |cx| {
        let session = cx
            .background_spawn(async move {
                Session::spawn(&SessionSpec {
                    path: &path,
                    start,
                    rate,
                    fps: video_file::output_fps(probe.fps),
                    max,
                    looped,
                    audio: probe.audio && !silent,
                    volume: vol,
                })
            })
            .await;
        cx.update(|cx| {
            let mut failed = false;
            if let Some(p) = store(cx).players.get_mut(&k) {
                p.spawning = false;
                let current = p.state == State::Playing && (p.position - start).abs() < 1e-6 && (p.rate - rate).abs() < 1e-9;
                match session {
                    Some(s) if current => {
                        p.session = Some(s);
                        p.started = false;
                    }
                    Some(_) => {}
                    None => failed = true,
                }
            }
            if failed {
                set_state(cx, &k, State::Paused, false);
            }
            ensure_ticker(cx);
        });
    })
    .detach();
}

fn frame_image(f: video_file::Frame) -> Option<Arc<RenderImage>> {
    let frame = image::Frame::new(image::RgbaImage::from_raw(f.width, f.height, f.data)?);
    Some(Arc::new(RenderImage::new(vec![frame])))
}

fn tick(cx: &mut App) -> bool {
    let now = Instant::now();
    let reduced = crate::ui::settings::effects::reduced_motion();
    let fullscreen = store(cx).fullscreen.clone();
    let keys: Vec<SharedString> = store(cx).players.keys().cloned().collect();
    let mut busy = fullscreen.is_some();
    let mut dirty = false;
    for key in keys {
        let offscreen = store(cx).players.get(&key).is_some_and(|p| p.state == State::Playing && p.painted.is_none_or(|t| now.duration_since(t) > OFFSCREEN));
        if offscreen && fullscreen.as_ref() != Some(&key) {
            set_state(cx, &key, State::Paused, false);
            restart_session(cx, &key);
            dirty = true;
            continue;
        }
        let vol = effective_volume(cx, &key);
        let Some(p) = store(cx).players.get_mut(&key) else { continue };
        let target = p.controls_target(now);
        p.controls.set(if target { 1. } else { 0. }, now, reduced);
        let idle = if fullscreen.as_ref() == Some(&key) { LEGACY_IDLE_FULLSCREEN } else { IDLE_HIDE };
        let hide = p.state == State::Playing && p.seek.is_none() && (!p.hovered || now.duration_since(p.moved) > idle);
        p.legacy_bar.set(if hide { 1. } else { 0. }, now, p.state != State::Playing || reduced);
        if p.legacy_bar.moving(now) || (p.state == State::Playing && p.hovered) {
            busy = true;
            dirty = dirty || p.legacy_bar.moving(now);
        }
        if p.volume_hide_at.is_some_and(|t| now >= t) {
            p.volume_hide_at = None;
            p.volume_hover = false;
        }
        let vol_target = p.volume_hover || p.volume_drag.is_some();
        p.volume_open.set(if vol_target { 1. } else { 0. }, now, reduced);
        if p.controls.moving(now) || p.volume_open.moving(now) || p.volume_hide_at.is_some() {
            busy = true;
            dirty = true;
        }
        if p.pop.is_some_and(|(t, _)| now.duration_since(t).as_secs_f32() * 1000. < POP_MS) {
            busy = true;
            dirty = true;
        } else if p.pop.take().is_some() {
            dirty = true;
        }
        if p.state == State::Playing && (target || p.hovered) {
            busy = true;
        }
        if matches!(p.load, Load::Fetching) {
            busy = true;
        }
        if p.state != State::Playing || p.seek.is_some() {
            continue;
        }
        busy = true;
        dirty = true;
        if !matches!(p.load, Load::Ready(..)) {
            continue;
        }
        let too_small = p.started && p.session.as_ref().is_some_and(|s| s.size.0 < p.want_max.0 || s.size.1 < p.want_max.1);
        if too_small {
            restart_session(cx, &key);
        }
        let Some(p) = store(cx).players.get_mut(&key) else { continue };
        let looped = p.looped;
        let Some(s) = p.session.as_mut() else {
            if !p.spawning {
                spawn_session(cx, &key);
            }
            continue;
        };
        s.set_volume(vol);
        let mut fresh = None;
        let mut started = p.started;
        if !started {
            if let Some(f) = s.first_frame() {
                s.set_paused(false);
                started = true;
                fresh = s.poll().or(Some(f));
            } else if s.finished() {
                p.session = None;
                p.state = State::Paused;
                dirty = true;
                continue;
            }
        } else {
            fresh = s.poll();
        }
        let ended = !looped && s.finished();
        p.started = started;
        let old = match fresh.and_then(frame_image) {
            Some(img) => {
                dirty = true;
                p.image.replace(img)
            }
            None => None,
        };
        retire(cx, old);
        if ended {
            set_state(cx, &key, State::Ended, false);
        }
    }
    evict(cx, now);
    let st = store(cx);
    if !busy {
        st.ticking = false;
    }
    if dirty {
        cx.refresh_windows();
    }
    busy
}

fn tk(i: usize) -> Hsla {
    c(tok(i))
}

fn time_font(cx: &App) -> SharedString {
    static F: std::sync::OnceLock<SharedString> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let names = cx.text_system().all_font_names();
        ["Roboto", "Helvetica Neue", "Arial", "Arimo", "Liberation Sans"]
            .into_iter()
            .find(|f| names.iter().any(|n| n.as_str() == *f))
            .map_or_else(|| SharedString::from(crate::fonts::ui_family()), SharedString::new_static)
    })
    .clone()
}

fn quant(v: f32) -> u8 {
    (v.clamp(0., 1.) * 10.).round() as u8
}

pub fn volume_icon_path(volume: f32) -> SharedString {
    let w = |k: f32| quant((3. * volume - k).clamp(0., 1.));
    format!("icons/dvp/volume-{}-{}-{}-{}.svg", w(0.), w(1.), w(2.), u8::from(volume == 0.)).into()
}

const SEEK_DIGITS: &str = "M169.545 229.312v7.919l14.838-10.253v46.348h7.669v-55.434h-6.169l-16.338 11.42ZM225.979 274.576c13.087 0 21.34-11.003 21.34-28.842 0-17.756-8.253-29.093-21.34-29.093-13.004 0-21.173 11.254-21.173 29.009 0 17.923 8.169 28.926 21.173 28.926Zm0-7.335c-8.419 0-13.004-8.336-13.004-21.591 0-13.087 4.585-21.673 13.004-21.673 8.503 0 13.171 8.669 13.171 21.757 0 13.171-4.668 21.507-13.171 21.507Z";
const SEEK_ARC: &str = "M137.108 31.459a160.22 160.22 0 0 0-30.316-2.894c-88.439 0-160.24 71.801-160.24 160.241 0 88.439 71.801 160.24 160.24 160.24 88.499 0 160.241-71.742 160.241-160.24h-31.365c0 71.176-57.699 128.876-128.876 128.876-71.128 0-128.876-57.748-128.876-128.876 0-71.129 57.748-128.877 128.876-128.877 8.183 0 16.347.78 24.382 2.328l5.934-30.798Z";
const SEEK_HEAD: &str = "M144.616 190.007V96.608l197.822 69.184-197.822 69.185v-44.97Z";

pub fn icon_svg(path: &str) -> Option<String> {
    let name = path.strip_prefix("icons/dvp/")?.strip_suffix(".svg")?;
    if name == "seek-back" || name == "seek-forward" {
        let back = name == "seek-back";
        let (dx, arc, head) = if back {
            ("-25.102", "-.0886 .03775 .03775 .0886 17.716 -4.317", "-.0222 .01458 .04451 .06777 3.48 -8.53")
        } else {
            ("-24.102", ".0886 .03775 -.03775 .0886 14.284 -4.317", ".0222 .01458 -.04451 .06777 28.52 -8.53")
        };
        return Some(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32" fill="none"><path fill="white" d="{SEEK_DIGITS}" transform="translate({dx} -30.774) scale(.19361)"/><path fill="white" d="{SEEK_ARC}" transform="matrix({arc})"/><path fill="white" d="{SEEK_HEAD}" transform="matrix({head})"/></svg>"#
        ));
    }
    let parts: Vec<u8> = name.strip_prefix("volume-")?.split('-').map(|p| p.parse().ok()).collect::<Option<Vec<u8>>>()?;
    let [a, b, cc, m] = parts.as_slice() else { return None };
    let o = |v: &u8| f32::from(*v) / 10.;
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"><path fill="white" d="M10 4.5a.75.75 0 0 1 1.25.56v13.88a.75.75 0 0 1-1.25.56L5.5 16H3a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1h2.5L10 4.5Z"/><g stroke="white" stroke-width="1.8" stroke-linecap="round"><path stroke-opacity="{}" d="M14 12Q14 10.2 12.5 9"/><path stroke-opacity="{}" d="M14 12Q14 13.8 12.5 15"/><path stroke-opacity="{}" d="M17.5 12Q17.5 8.5 15 6.5"/><path stroke-opacity="{}" d="M17.5 12Q17.5 15.5 15 17.5"/><path stroke-opacity="{}" d="M21 12Q21 7 17.5 4"/><path stroke-opacity="{}" d="M21 12Q21 17 17.5 20"/><path stroke-opacity="{}" d="m15 9 6 6m-6 0 6-6"/></g></svg>"#,
        o(a),
        o(a),
        o(b),
        o(b),
        o(cc),
        o(cc),
        f32::from(*m)
    ))
}

fn icon(path: SharedString, size: f32) -> gpui::Svg {
    svg().path(path).flex_none().size(z(size)).text_color(gpui::white())
}

fn button(id: String, icon_path: SharedString, cfg: &Config, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let hover_bg = tk(idx::OPACITY_BLURPLE_16);
    div()
        .id(ElementId::Name(id.into()))
        .flex_none()
        .min_w(z(24.))
        .h(z(cfg.button_h))
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(global::RADIUS_SM))
        .cursor_pointer()
        .hover(move |s| s.bg(Hsla { a: hover_bg.a * 0.55, ..hover_bg }).mt(z(-1.)).mb(z(1.)))
        .active(|s| s.mt(z(0.)).mb(z(0.)))
        .tooltip(crate::ui::tooltip::tip(tip))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(icon(icon_path, cfg.icon))
}

struct Track {
    key: SharedString,
    frac: f32,
    duration: Option<f64>,
    cfg: Config,
    hover: Option<f32>,
    dragging: bool,
    visible: bool,
    shift: f32,
    legacy: bool,
}

fn paint_legacy_track(window: &mut Window, b: Bounds<Pixels>, t: &Track) {
    let h6 = f32::from(z(6.));
    let edge = f32::from(z(3.));
    let w = f32::from(b.size.width);
    let y = f32::from(b.origin.y) + (f32::from(b.size.height) - h6) / 2.;
    let x0 = f32::from(b.origin.x);
    let base = tk(idx::PRIMARY_300);
    let bar = Bounds::new(gpui::point(px(x0 - edge), px(y)), gpui::size(px(w + 2. * edge), px(h6)));
    let lit = t.hover.is_some() || t.dragging;
    if lit {
        let sh = Bounds::new(gpui::point(bar.origin.x, bar.origin.y + z(1.)), bar.size);
        window.paint_quad(gpui::fill(sh, tk(idx::OPACITY_BLACK_28)).corner_radii(px(edge)));
    }
    window.paint_quad(gpui::fill(bar, Hsla { a: base.a * 0.3, ..base }).corner_radii(px(edge)));
    if t.duration.is_some() {
        window.paint_quad(gpui::fill(bar, Hsla { a: 0.3, ..gpui::white() }).corner_radii(px(edge)));
    }
    if let Some(hf) = t.hover.filter(|_| !t.dragging) {
        let pv = Bounds::new(bar.origin, gpui::size(px(w * hf + 2. * edge), px(h6)));
        window.paint_quad(gpui::fill(pv, Hsla { a: 0.3, ..gpui::white() }).corner_radii(px(edge)));
    }
    let prog = Bounds::new(bar.origin, gpui::size(px(w * t.frac + 2. * edge), px(h6)));
    window.paint_quad(gpui::fill(prog, tk(idx::BRAND_500)).corner_radii(px(edge)));
    if lit {
        let d = f32::from(z(10.));
        let gx = x0 + w * t.frac;
        let gb = Bounds::new(gpui::point(px(gx - d / 2.), px(y + h6 / 2. - d / 2.)), gpui::size(px(d), px(d)));
        window.paint_quad(gpui::fill(gb, tk(idx::BRAND_560)).corner_radii(px(d / 2.)));
    }
}

fn timeline(t: Track) -> gpui::Canvas<Hitbox> {
    gpui::canvas(
        |b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal),
        move |b, hitbox, window, cx| {
            let cfg = t.cfg;
            let lit = t.hover.is_some() || t.dragging;
            if t.legacy {
                paint_legacy_track(window, b, &t);
            }
            let h = f32::from(z(if lit { cfg.timeline_hover_h } else { cfg.timeline_h }));
            let w = f32::from(b.size.width);
            let x0 = f32::from(b.origin.x);
            let y = f32::from(b.origin.y) + (f32::from(b.size.height) - h) / 2. + f32::from(z(t.shift));
            let r = px(h / 2.);
            let fade = |col: Hsla| Hsla { a: col.a * if t.visible && !t.legacy { 1. } else { 0. }, ..col };
            let bar = Bounds::new(gpui::point(px(x0), px(y)), gpui::size(px(w), px(h)));
            window.paint_quad(gpui::fill(bar, fade(tk(idx::OPACITY_BLURPLE_16))).corner_radii(r));
            if t.duration.is_some() {
                window.paint_quad(gpui::fill(bar, fade(tk(idx::OPACITY_BLURPLE_16))).corner_radii(r));
            }
            let fw = (w * t.frac).max(0.);
            if fw > 0. && !t.legacy {
                let fill = Bounds::new(bar.origin, gpui::size(px(fw), px(h)));
                window.paint_quad(
                    gpui::fill(
                        fill,
                        gpui::linear_gradient(90., gpui::linear_color_stop(tk(idx::BACKGROUND_BRAND), 0.), gpui::linear_color_stop(tk(idx::BRAND_400), 1.)),
                    )
                    .corner_radii(r),
                );
            }
            if t.duration.is_some_and(|d| d > 0.) && !t.legacy {
                let scale = if t.dragging { 1.3 } else if lit { 1.15 } else { 1. };
                let d = f32::from(z(cfg.playhead)) * scale;
                let cx0 = x0 + fw;
                let cy = y + h / 2.;
                let ring = f32::from(z(2.));
                let glow = f32::from(z(4.));
                let brand = tk(idx::BACKGROUND_BRAND);
                let circle = |rad: f32| Bounds::new(gpui::point(px(cx0 - rad), px(cy - rad)), gpui::size(px(2. * rad), px(2. * rad)));
                window.paint_quad(gpui::fill(circle(d / 2. + ring + glow / 2.), Hsla { a: 0.35, ..brand }).corner_radii(px(d)));
                window.paint_quad(gpui::fill(circle(d / 2. + ring), brand).corner_radii(px(d)));
                window.paint_quad(gpui::fill(circle(d / 2.), gpui::white()).corner_radii(px(d)));
            }
            let (k, hb) = (t.key.clone(), hitbox.clone());
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let over = hb.is_hovered(window);
                let Some(p) = store(cx).players.get_mut(&k) else { return };
                let next = over.then(|| frac_in(b, e.position.x));
                if next.is_some() || p.track_hover.is_some() {
                    p.track_hover = next;
                    p.moved = Instant::now();
                    window.refresh();
                }
            });
            let Some(duration) = t.duration else { return };
            let k = t.key.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hitbox.is_hovered(window) {
                    cx.stop_propagation();
                    let resume = cx.try_global::<Store>().and_then(|s| s.players.get(&k)).is_some_and(|p| p.state == State::Playing);
                    let was_ended = cx.try_global::<Store>().and_then(|s| s.players.get(&k)).is_some_and(|p| p.state == State::Ended);
                    if resume {
                        set_state(cx, &k, State::Paused, false);
                    }
                    seek_to(cx, &k, f64::from(frac_in(b, e.position.x)) * duration);
                    if let Some(p) = store(cx).players.get_mut(&k) {
                        p.seek = Some(Seek { bounds: b, resume: resume || was_ended });
                    }
                    window.refresh();
                }
            });
            if !t.dragging {
                return;
            }
            let drag_b = cx.try_global::<Store>().and_then(|s| s.players.get(&t.key)).and_then(|p| p.seek.as_ref()).map_or(b, |s| s.bounds);
            let k = t.key.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    return;
                }
                if let Some(p) = store(cx).players.get_mut(&k) {
                    p.position = f64::from(frac_in(drag_b, e.position.x)) * duration;
                    p.track_hover = Some(frac_in(drag_b, e.position.x));
                }
                window.refresh();
            });
            let k = t.key.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let Some(p) = store(cx).players.get_mut(&k) else { return };
                let Some(s) = p.seek.take() else { return };
                let pos = p.position;
                seek_to(cx, &k, pos);
                if pos >= duration - 0.01 {
                    set_state(cx, &k, State::Ended, false);
                } else if s.resume {
                    set_state(cx, &k, State::Playing, false);
                }
                window.refresh();
            });
        },
    )
}

fn frac_in(b: Bounds<Pixels>, x: Pixels) -> f32 {
    (f32::from(x - b.origin.x) / f32::from(b.size.width).max(1.)).clamp(0., 1.)
}

fn time_tooltip(text: String, frac: f32, font: SharedString) -> AnyElement {
    div()
        .absolute()
        .bottom(z(15.))
        .left(gpui::relative(frac))
        .child(
            div()
                .relative()
                .left(z(-40.))
                .w(z(80.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(z(8.))
                        .py(z(4.))
                        .rounded(z(global::RADIUS_SM))
                        .bg(tk(idx::BACKGROUND_SCRIM_LIGHTBOX))
                        .border_1()
                        .border_color(tk(idx::OPACITY_BLURPLE_32))
                        .shadow(crate::ui::inbox::shadow_high())
                        .font_family(font)
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .text_color(tk(idx::TEXT_OVERLAY_LIGHT))
                        .whitespace_nowrap()
                        .child(text),
                ),
        )
        .into_any_element()
}

fn mini_slider(key: SharedString, value: f32, width: f32) -> AnyElement {
    let k = key.clone();
    div()
        .relative()
        .w(z(width))
        .h(z(24.))
        .child(
            gpui::canvas(
                |b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal),
                move |b, hitbox, window, cx| {
                    let bar_h = f32::from(z(4.));
                    let w = f32::from(b.size.width);
                    if w < 1. {
                        return;
                    }
                    let y = f32::from(b.origin.y) + (f32::from(b.size.height) - bar_h) / 2.;
                    let x0 = f32::from(b.origin.x);
                    let bar = Bounds::new(gpui::point(px(x0), px(y)), gpui::size(px(w), px(bar_h)));
                    window.paint_quad(gpui::fill(bar, tk(idx::OPACITY_BLURPLE_24)).corner_radii(px(bar_h)));
                    let fill = Bounds::new(bar.origin, gpui::size(px(w * value), px(bar_h)));
                    window.paint_quad(gpui::fill(fill, tk(idx::BACKGROUND_BRAND)).corner_radii(px(bar_h)).border_widths(px(1.)).border_color(Hsla { a: 0.1, ..gpui::white() }));
                    let g = f32::from(z(16.));
                    let ring = f32::from(z(2.));
                    let gx = x0 + w * value;
                    let cy = y + bar_h / 2.;
                    let circle = |rad: f32| Bounds::new(gpui::point(px(gx - rad), px(cy - rad)), gpui::size(px(2. * rad), px(2. * rad)));
                    window.paint_quad(gpui::fill(circle(g / 2. + ring), tk(idx::OPACITY_BLURPLE_32)).corner_radii(px(g)));
                    window.paint_quad(gpui::fill(circle(g / 2.), gpui::white()).corner_radii(px(g)).border_widths(px(1.)).border_color(tk(idx::BACKGROUND_BRAND)));
                    let k1 = k.clone();
                    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hitbox.is_hovered(window) {
                            cx.stop_propagation();
                            if let Some(p) = store(cx).players.get_mut(&k1) {
                                p.volume_drag = Some(b);
                            }
                            set_volume(cx, frac_in(b, e.position.x));
                            ensure_ticker(cx);
                            window.refresh();
                        }
                    });
                    let drag = cx.try_global::<Store>().and_then(|s| s.players.get(&k)).and_then(|p| p.volume_drag);
                    if let Some(db) = drag {
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture {
                                set_volume(cx, frac_in(db, e.position.x));
                                window.refresh();
                            }
                        });
                        let k2 = k.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                if let Some(p) = store(cx).players.get_mut(&k2) {
                                    p.volume_drag = None;
                                }
                                window.refresh();
                            }
                        });
                    }
                },
            )
            .size_full(),
        )
        .into_any_element()
}

fn popout_surface() -> gpui::Div {
    div()
        .rounded(z(global::RADIUS_MD))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::OPACITY_BLURPLE_32))
        .shadow(crate::ui::inbox::shadow_high())
        .child(div().absolute().inset_0().rounded(z(global::RADIUS_MD)).bg(gpui::linear_gradient(
            135.,
            gpui::linear_color_stop(tk(idx::OPACITY_BLURPLE_16), 0.),
            gpui::linear_color_stop(tk(idx::OPACITY_BLURPLE_16).opacity(0.), 0.7),
        )))
}

fn volume_hover(cx: &mut App, key: &SharedString, over: bool, floating: bool) {
    let Some(p) = store(cx).players.get_mut(key) else { return };
    if over {
        p.volume_hover = true;
        p.volume_hide_at = None;
    } else if floating {
        p.volume_hide_at = Some(Instant::now() + VOLUME_HIDE);
    } else {
        p.volume_hover = false;
    }
    ensure_ticker(cx);
}

struct View {
    key: SharedString,
    cfg: Config,
    plan: Plan,
    state: State,
    current: f64,
    duration: Option<f64>,
    controls: f32,
    volume_open: f32,
    remaining: bool,
    rate: f64,
    hover: Option<f32>,
    dragging: bool,
    fullscreen: bool,
    speed_open: bool,
    viewer: Option<crate::media::ViewerImage>,
    tiny: bool,
}

fn volume_group(v: &View, cx: &App) -> AnyElement {
    let key = v.key.clone();
    let vol = shown_volume(cx);
    let tip = if vol == 0. { crate::t!("video.unmute") } else { crate::t!("video.mute") };
    let mute = button(format!("dvp-vol-{key}"), volume_icon_path(vol), &v.cfg, tip).on_click(|_, window, cx| {
        toggle_mute(cx);
        window.refresh();
    });
    let floating = v.plan.floating_volume;
    let (k1, k2) = (key.clone(), key.clone());
    let open = if v.plan.autohide_volume { v.volume_open } else { 1. };
    let group = div()
        .id(ElementId::Name(format!("dvp-volgroup-{key}").into()))
        .relative()
        .flex()
        .items_center()
        .flex_none()
        .when(!floating, |d| d.mr(z(v.cfg.volume_me)))
        .on_hover(move |over, window, cx| {
            volume_hover(cx, &k1, *over, floating);
            window.refresh();
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            let k = e.keystroke.key.as_str();
            if k == "up" || k == "down" {
                cx.stop_propagation();
                let t = (shown_volume(cx) * 100.).round();
                let n = if k == "up" { (t / 10.).floor() + 1. } else { (t / 10.).ceil() - 1. };
                set_volume(cx, (n / 10.).clamp(0., 1.));
                window.refresh();
            }
        })
        .child(mute);
    if floating {
        let shown = v.volume_open > 0.01;
        return group
            .when(shown, |d| {
                d.child(
                    div().absolute().bottom(gpui::relative(1.)).left(z(-48.)).w(z(120.)).pb(z(8.)).child(
                        div()
                            .id(ElementId::Name(format!("dvp-volpop-{k2}").into()))
                            .relative()
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_hover({
                                let k = k2.clone();
                                move |over, window, cx| {
                                    volume_hover(cx, &k, *over, true);
                                    window.refresh();
                                }
                            })
                            .child(popout_surface().relative().p(z(12.)).child(mini_slider(k2.clone(), vol, 96.))),
                    ),
                )
            })
            .into_any_element();
    }
    let width = v.cfg.volume_w * open;
    group
        .child(
            div()
                .flex_none()
                .overflow_hidden()
                .opacity(open)
                .ml(z(v.cfg.volume_ms * open.min(1.)))
                .px(z(v.cfg.volume_px * open))
                .w(z(width + 2. * v.cfg.volume_px * open))
                .child(div().w(z(width)).child(mini_slider(k2, vol, width))),
        )
        .into_any_element()
}

fn duration_text(v: &View, font: SharedString) -> AnyElement {
    let cur = match (v.remaining, v.duration) {
        (true, Some(d)) => format!("-{}", format_time((d - v.current).max(0.))),
        (true, None) => "-:--".into(),
        (false, _) => format_time(v.current),
    };
    let total = v.duration.map_or_else(|| "-:--".to_owned(), format_time);
    let lh = if v.cfg.time_px <= 12. { 16. } else { 18. };
    let key = v.key.clone();
    let text = |s: String, col: Hsla| div().text_size(z(v.cfg.time_px)).line_height(z(lh)).text_color(col).child(s);
    div()
        .id(ElementId::Name(format!("dvp-time-{key}").into()))
        .flex_none()
        .min_h(z(24.))
        .h(z(24.))
        .ml(z(v.cfg.duration_margin))
        .flex()
        .items_center()
        .gap(z(4.))
        .whitespace_nowrap()
        .cursor_pointer()
        .font_family(font)
        .font_weight(FontWeight::NORMAL)
        .child(text(cur, gpui::white()))
        .when(!v.plan.compact_time, |d| d.child(text("/".into(), tk(idx::OPACITY_WHITE_60))).child(text(total, gpui::white())))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            if let Some(p) = store(cx).players.get_mut(&key) {
                p.remaining = !p.remaining;
            }
            window.refresh();
        })
        .into_any_element()
}

fn speed_button(v: &View) -> AnyElement {
    let key = v.key.clone();
    let active = (v.rate - 1.).abs() > 1e-6;
    let badge = format!("{}x", v.rate);
    let b = button(format!("dvp-speed-{key}"), crate::icons::path("media/speedometer"), &v.cfg, crate::t!("video.speed"))
        .relative()
        .when(active, |d| {
            d.bg(tk(idx::OPACITY_BLURPLE_32)).child(
                div().absolute().bottom(z(v.cfg.button_h * 0.125 - 4.)).left_0().right_0().flex().justify_center().child(
                    div()
                        .px(z(4.))
                        .py(z(1.))
                        .rounded(z(global::RADIUS_XS))
                        .bg(tk(idx::BACKGROUND_BRAND))
                        .text_color(tk(idx::TEXT_OVERLAY_LIGHT))
                        .text_size(z(10.))
                        .line_height(z(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .whitespace_nowrap()
                        .child(badge),
                ),
            )
        })
        .on_click(move |_, window, cx| {
            if let Some(p) = store(cx).players.get_mut(&key) {
                let just_closed = p.speed_closed.is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                p.speed_open = !p.speed_open && !just_closed;
            }
            ensure_ticker(cx);
            window.refresh();
        });
    let pop = v.speed_open.then(|| speed_popout(v.key.clone(), v.rate));
    div().relative().flex_none().child(b).children(pop).into_any_element()
}

fn speed_popout(key: SharedString, rate: f64) -> AnyElement {
    let marks = RATES.iter().enumerate().map(|(i, r)| {
        let k = key.clone();
        let r = *r;
        let selected = (r - rate).abs() < 1e-6;
        div()
            .id(ElementId::Name(format!("dvp-rate-{key}-{i}").into()))
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .gap(z(4.))
            .cursor_pointer()
            .child(
                div()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if (r - 1.).abs() < 1e-6 { tk(idx::TEXT_DEFAULT) } else { tk(idx::TEXT_SUBTLE) })
                    .child(format!("{r}x")),
            )
            .child(
                div()
                    .size(z(16.))
                    .rounded_full()
                    .when(selected, |d| d.bg(gpui::white()).border_1().border_color(tk(idx::BACKGROUND_BRAND)))
                    .when(!selected, |d| d.child(div().mx_auto().mt(z(6.)).size(z(4.)).rounded_full().bg(tk(idx::OPACITY_BLURPLE_24)))),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| {
                set_rate(cx, &k, r);
                window.refresh();
            })
    });
    div()
        .absolute()
        .bottom(gpui::relative(1.))
        .right_0()
        .pb(z(8.))
        .child(
            popout_surface()
                .id(ElementId::Name(format!("dvp-speedpop-{key}").into()))
                .relative()
                .occlude()
                .w(z(260.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .pt(z(16.))
                .px(z(16.))
                .pb(z(12.))
                .flex()
                .flex_col()
                .gap(z(12.))
                .on_mouse_down_out({
                    let k = key.clone();
                    move |_, window, cx| {
                        if let Some(p) = store(cx).players.get_mut(&k)
                            && p.speed_open
                        {
                            p.speed_open = false;
                            p.speed_closed = Some(Instant::now());
                        }
                        window.refresh();
                    }
                })
                .child(
                    div()
                        .relative()
                        .text_center()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tk(idx::TEXT_DEFAULT))
                        .child(crate::t!("video.speed")),
                )
                .child(div().relative().flex().children(marks)),
        )
        .into_any_element()
}

fn controls(v: &View, cx: &App) -> AnyElement {
    let font = time_font(cx);
    let key = v.key.clone();
    let cfg = v.cfg;
    let (play_icon, tip) = match v.state {
        State::Playing => (crate::icons::path("pause"), crate::t!("video.pause")),
        State::Paused => (crate::icons::path("play"), crate::t!("video.play")),
        State::Ended => (crate::icons::path("media/retry"), crate::t!("video.replay")),
    };
    let k = key.clone();
    let play = button(format!("dvp-play-{key}"), play_icon, &cfg, tip).on_click(move |_, window, cx| {
        toggle(cx, &k);
        window.refresh();
    });
    let (kb, kf) = (key.clone(), key.clone());
    let start = div().flex().items_center().gap(z(cfg.gap)).child(play).when(v.plan.skip, |d| {
        d.child(button(format!("dvp-back-{key}"), "icons/dvp/seek-back.svg".into(), &cfg, crate::t!("video.rewind")).on_click(move |_, window, cx| {
            skip(cx, &kb, -10.);
            window.refresh();
        }))
        .child(button(format!("dvp-fwd-{key}"), "icons/dvp/seek-forward.svg".into(), &cfg, crate::t!("video.skip")).on_click(move |_, window, cx| {
            skip(cx, &kf, 10.);
            window.refresh();
        }))
    });
    let mid = div().flex_1().min_w_0().ml(z(cfg.gap)).flex().items_center().gap(z(cfg.mid_gap)).child(volume_group(v, cx)).child(duration_text(v, font));
    let kfs = key.clone();
    let fs_icon = crate::icons::path(if v.fullscreen { "call/fullscreen-exit" } else { "call/fullscreen" });
    let end = div()
        .ml_auto()
        .flex()
        .items_center()
        .justify_end()
        .gap(z(cfg.gap))
        .when_some(cfg.trailing_w, |d, w| d.w(z(w)))
        .when(v.plan.speed, |d| d.child(speed_button(v)))
        .when_some(v.viewer.clone().filter(|_| !v.tiny && !v.fullscreen), |d, viewer| {
            let k = key.clone();
            d.child(button(format!("dvp-viewer-{key}"), crate::icons::path("media/window-top"), &cfg, crate::t!("video.open_viewer")).on_click(move |_, window, cx| {
                open_in_viewer(cx, &k, viewer.clone());
                window.refresh();
            }))
        })
        .child(button(format!("dvp-fs-{key}"), fs_icon, &cfg, crate::t!("video.fullscreen")).on_click(move |_, window, cx| {
            let on = cx.try_global::<Store>().and_then(|s| s.fullscreen.clone()).is_none();
            set_fullscreen(cx, window, on.then(|| kfs.clone()));
        }));
    let pad = v.controls * v.controls * cfg.timeline_pad;
    let frac = match v.duration {
        Some(d) if d > 0. => (v.current / d).clamp(0., 1.) as f32,
        _ => 0.,
    };
    let tooltip = match (v.hover, v.duration) {
        (Some(f), Some(d)) if v.controls > 0.99 || v.dragging => Some(time_tooltip(format_time(f64::from(f) * d), f, time_font(cx))),
        _ => None,
    };
    let hitbox_h = 30.;
    let shift = -cfg.hitbox_offset;
    let track = div()
        .absolute()
        .left_0()
        .right_0()
        .top(z(-cfg.timeline_pad * (1. - v.controls) - 4. - (hitbox_h - cfg.timeline_h) / 2. - shift))
        .h(z(hitbox_h))
        .opacity(v.controls)
        .cursor_pointer()
        .child(
            timeline(Track { key: key.clone(), frac, duration: v.duration, cfg, hover: v.hover, dragging: v.dragging, visible: v.controls > 0.01, shift, legacy: false })
                .size_full(),
        )
        .child(div().absolute().left_0().right_0().top(z((hitbox_h - cfg.timeline_h) / 2. + shift)).children(tooltip));
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(z(cfg.bottom))
        .px(z(cfg.pad_x))
        .h(z(v.controls * cfg.bar_h))
        .child(
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(z(2.))
                        .pt(z(pad))
                        .pb(z(pad))
                        .h(z(v.controls * cfg.bar_h))
                        .opacity(v.controls)
                        .flex()
                        .items_center()
                        .child(div().w_full().h(z(cfg.button_h)).flex().items_center().child(start).child(mid).child(end)),
                )
                .child(track),
        )
        .into_any_element()
}

fn scrim(h: f32, opacity: f32) -> AnyElement {
    let top = tk(idx::OPACITY_BLACK_68);
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(z(h))
        .opacity(opacity)
        .flex()
        .flex_col()
        .child(div().h(gpui::relative(0.45)).w_full().bg(gpui::linear_gradient(180., gpui::linear_color_stop(top.opacity(0.), 0.), gpui::linear_color_stop(top, 1.))))
        .child(div().flex_1().w_full().bg(gpui::linear_gradient(
            180.,
            gpui::linear_color_stop(top, 0.),
            gpui::linear_color_stop(tk(idx::OPACITY_BLACK_92), 1.),
        )))
        .into_any_element()
}

fn play_pop(state: State, t: f32) -> AnyElement {
    let k = (t / POP_MS).clamp(0., 1.);
    let opacity = if k < 0.1 { k / 0.1 } else { 1. - (k - 0.1) / 0.9 };
    let scale = if k < 0.1 { 1. } else { 1. + 0.5 * (k - 0.1) / 0.9 };
    let size = 48. * scale;
    let icon_px = 24. * scale;
    let path = crate::icons::path(if state == State::Playing { "play" } else { "pause" });
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(z(size))
                .rounded_full()
                .opacity(opacity)
                .bg(tk(idx::OPACITY_BLACK_60))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(path, icon_px)),
        )
        .into_any_element()
}

fn inactive_button() -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .p(z(12.))
                .rounded_full()
                .bg(gpui::black())
                .opacity(0.6)
                .group_hover("dvp-inactive", |s| s.opacity(0.8))
                .child(div().ml(z(1.)).mr(z(-1.)).child(icon(crate::icons::path("play"), 24.))),
        )
        .into_any_element()
}

fn spinner() -> AnyElement {
    div()
        .absolute()
        .top(z(32.))
        .left(z(32.))
        .p(z(8.))
        .rounded(z(12.))
        .bg(crate::theme::ca(0x00000073))
        .border_1()
        .border_color(crate::theme::ca(0xffffff0d))
        .child(crate::ui::spinner::wandering_cubes())
        .into_any_element()
}

fn tracker(key: SharedString, src: SharedString, looped: bool, plays: Option<bool>, fullscreen: bool) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| {},
        move |b, _, window, cx| {
            ensure(cx, &key, &src, looped);
            release(window, cx);
            let scale = window.scale_factor();
            let want = if fullscreen {
                (1920, 1080)
            } else {
                ((f32::from(b.size.width) * scale).ceil().clamp(64., 1920.) as u32, (f32::from(b.size.height) * scale).ceil().clamp(64., 1080.) as u32)
            };
            let st = store(cx);
            let is_fs = st.fullscreen.as_ref() == Some(&key);
            if is_fs != fullscreen {
                return;
            }
            if fullscreen {
                st.fs_width = f32::from(window.viewport_size().width);
            }
            let Some(p) = st.players.get_mut(&key) else { return };
            p.painted = Some(Instant::now());
            p.want_max = want;
            if !fullscreen {
                p.measured = Some(f32::from(b.size.width) / f32::from(z(1.)).max(0.01));
                p.shown_at = Some(b);
            }
            if let Some(play) = plays {
                p.active = true;
                let state = p.state;
                if play && state != State::Playing {
                    set_state(cx, &key, State::Playing, false);
                    fetch(cx, &key);
                } else if !play && state == State::Playing {
                    set_state(cx, &key, State::Paused, false);
                }
            }
            if store(cx).players.get(&key).is_some_and(|p| p.state == State::Playing || p.pop.is_some()) {
                ensure_ticker(cx);
            }
        },
    )
}

pub struct Spec {
    pub key: SharedString,
    pub src: SharedString,
    pub poster: SharedString,
    pub layout: Layout,
    pub portrait: bool,
    pub width: f32,
    pub radius: f32,
    pub viewer: Option<crate::media::ViewerImage>,
}

fn view(cx: &App, key: &SharedString, cfg: Config, plan: Plan, fullscreen: bool) -> Option<View> {
    let now = Instant::now();
    let p = cx.try_global::<Store>()?.players.get(key)?;
    Some(View {
        key: key.clone(),
        cfg,
        plan,
        state: p.state,
        current: p.current(),
        duration: p.duration(),
        controls: p.controls.value(now),
        volume_open: p.volume_open.value(now),
        remaining: p.remaining,
        rate: p.rate,
        hover: p.track_hover,
        dragging: p.seek.is_some(),
        fullscreen,
        speed_open: p.speed_open,
        viewer: None,
        tiny: false,
    })
}

fn frame_layer(cx: &App, key: &SharedString, poster: Option<SharedString>, active: bool) -> AnyElement {
    let image = cx.try_global::<Store>().and_then(|s| s.players.get(key)).and_then(|p| p.image.clone());
    match (image, poster) {
        (Some(i), _) if active => img(gpui::ImageSource::Render(i)).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Contain).into_any_element(),
        (_, Some(p)) => {
            let mut im = img(p);
            if let Some(cache) = crate::media::lru::chat() {
                im = im.image_cache(&cache);
            }
            im.absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover).into_any_element()
        }
        _ => div().into_any_element(),
    }
}

fn dvp_attachment(spec: Spec, cx: &App) -> AnyElement {
    let key = spec.key.clone();
    let st = cx.try_global::<Store>();
    let p = st.and_then(|s| s.players.get(&key));
    let fs_here = st.and_then(|s| s.fullscreen.as_ref()) == Some(&key);
    let active = p.is_some_and(|p| p.active) && !fs_here;
    let duration = p.and_then(Player::duration).unwrap_or(0.);
    let width = p.and_then(|p| p.measured).unwrap_or(spec.width);
    let plan = plan(spec.layout, spec.portrait, width, duration, false);
    let v = view(cx, &key, ATTACHMENT, plan, false).map(|v| View { viewer: spec.viewer.clone(), tiny: width < TINY_MAX_W, ..v });
    let now = Instant::now();
    let pop = p.and_then(|p| p.pop).map(|(t, s)| (s, now.duration_since(t).as_secs_f32() * 1000.));
    let loading = p.is_some_and(Player::loading);
    let failed = p.is_some_and(|p| matches!(p.load, Load::Failed));
    let (k_hover, k_move, k_click, k_key) = (key.clone(), key.clone(), key.clone(), key.clone());
    let focus = p.and_then(|p| p.focus.clone());
    let src = spec.src.clone();
    let clip = div()
        .absolute()
        .inset_0()
        .rounded(z(spec.radius))
        .overflow_hidden()
        .bg(if active { gpui::black() } else { tk(idx::BACKGROUND_BASE_LOWER) })
        .child(frame_layer(cx, &key, (!active || p.is_none_or(|p| p.image.is_none())).then(|| spec.poster.clone()), active))
        .when(active, |d| d.child(scrim(ATTACHMENT.scrim_h, v.as_ref().map_or(0., |v| v.controls))))
        .when(!active, |d| d.child(inactive_button()))
        .when(active && loading, |d| d.child(spinner()))
        .when_some(pop.filter(|_| active), |d, (s, t)| d.child(play_pop(s, t)));
    div()
        .id(ElementId::Name(format!("dvp-{key}").into()))
        .when_some(focus, |d, h| d.track_focus(&h))
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            if handle_key(cx, window, &k_key, e.keystroke.key.as_str()) {
                cx.stop_propagation();
                window.refresh();
            }
        })
        .group("dvp-inactive")
        .relative()
        .size_full()
        .cursor_pointer()
        .child(tracker(key.clone(), spec.src.clone(), false, None, false).absolute().inset_0())
        .child(clip)
        .when_some(v.filter(|v| active && (v.controls > 0.01 || v.dragging)), |d, v| d.child(controls(&v, cx)))
        .on_hover(move |over, window, cx| {
            if let Some(p) = store(cx).players.get_mut(&k_hover) {
                p.hovered = *over;
                p.moved = Instant::now();
                if !*over {
                    p.track_hover = None;
                }
            }
            ensure_ticker(cx);
            window.refresh();
        })
        .on_mouse_move(move |_, _, cx| {
            if let Some(p) = store(cx).players.get_mut(&k_move) {
                p.moved = Instant::now();
                if !p.controls_target(Instant::now()) || p.controls.value(Instant::now()) < 1. {
                    ensure_ticker(cx);
                }
            }
        })
        .on_click(move |_, window, cx| {
            if failed {
                cx.open_url(&src);
                return;
            }
            ensure(cx, &k_click, &src, false);
            focus_player(cx, window, &k_click);
            toggle(cx, &k_click);
            window.refresh();
        })
        .into_any_element()
}

pub fn gifv(key: SharedString, src: SharedString, poster: SharedString, plays: bool, cx: &App) -> AnyElement {
    let image = cx.try_global::<Store>().and_then(|s| s.players.get(&key)).and_then(|p| p.image.clone());
    let failed = cx.try_global::<Store>().and_then(|s| s.players.get(&key)).is_some_and(|p| matches!(p.load, Load::Failed));
    div()
        .relative()
        .size_full()
        .child(tracker(key.clone(), src, true, Some(plays && !failed), false).absolute().inset_0())
        .child(frame_layer(cx, &key, Some(poster), false))
        .when_some(image.filter(|_| !failed), |d, i| d.child(img(gpui::ImageSource::Render(i)).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)))
        .into_any_element()
}

fn dvp_fullscreen_layer(cx: &App) -> Option<AnyElement> {
    let key = cx.try_global::<Store>()?.fullscreen.clone()?;
    let p = cx.try_global::<Store>()?.players.get(&key)?;
    let portrait = p.probe().is_some_and(|pr| pr.height > pr.width);
    let duration = p.duration().unwrap_or(0.);
    let now = Instant::now();
    let pop = p.pop.map(|(t, s)| (s, now.duration_since(t).as_secs_f32() * 1000.));
    let loading = p.loading();
    let src = p.src.clone();
    let focus = cx.try_global::<Store>()?.focus.clone();
    let (k_hover, k_move, k_click) = (key.clone(), key.clone(), key.clone());
    let body = |plan: Plan, cx: &App| {
        let v = view(cx, &key, FULLSCREEN, plan, true);
        div()
            .id("dvp-fullscreen")
            .absolute()
            .inset_0()
            .occlude()
            .when_some(focus.as_ref(), |d, h| d.track_focus(h))
            .on_key_down({
                let k = key.clone();
                move |e: &KeyDownEvent, window, cx| {
                    match e.keystroke.key.as_str() {
                        "escape" | "f" => set_fullscreen(cx, window, None),
                        "space" | "k" => toggle(cx, &k),
                        "m" => toggle_mute(cx),
                        "left" | "j" => skip(cx, &k, -10.),
                        "right" | "l" => skip(cx, &k, 10.),
                        _ => return,
                    }
                    cx.stop_propagation();
                    window.refresh();
                }
            })
            .bg(gpui::black())
            .child(tracker(key.clone(), src.clone(), false, None, true).absolute().inset_0())
            .child(frame_layer(cx, &key, None, true))
            .child(scrim(FULLSCREEN.scrim_h, v.as_ref().map_or(0., |v| v.controls)))
            .when(loading, |d| d.child(spinner()))
            .when_some(pop, |d, (s, t)| d.child(play_pop(s, t)))
            .when_some(v.filter(|v| v.controls > 0.01 || v.dragging), |d, v| d.child(controls(&v, cx)))
            .on_hover({
                let k = k_hover.clone();
                move |over, _, cx| {
                    if let Some(p) = store(cx).players.get_mut(&k) {
                        p.hovered = *over;
                        p.moved = Instant::now();
                    }
                    ensure_ticker(cx);
                }
            })
            .on_mouse_move({
                let k = k_move.clone();
                move |_, _, cx| {
                    if let Some(p) = store(cx).players.get_mut(&k) {
                        p.hovered = true;
                        p.moved = Instant::now();
                    }
                    ensure_ticker(cx);
                }
            })
            .on_click({
                let k = k_click.clone();
                move |_, window, cx| {
                    toggle(cx, &k);
                    window.refresh();
                }
            })
    };
    let window_w = cx.try_global::<Store>().map_or(1280., |s| s.fs_width);
    Some(body(plan(Layout::Single, portrait, window_w, duration, true), cx).into_any_element())
}

pub fn attachment(spec: Spec, cx: &App) -> AnyElement {
    if dvp_enabled() { dvp_attachment(spec, cx) } else { legacy_attachment(spec, cx) }
}

pub fn fullscreen_layer(cx: &App) -> Option<AnyElement> {
    if dvp_enabled() { dvp_fullscreen_layer(cx) } else { legacy_fullscreen_layer(cx) }
}

fn focus_player(cx: &mut App, window: &mut Window, key: &SharedString) {
    let existing = cx.try_global::<Store>().and_then(|s| s.players.get(key)).and_then(|p| p.focus.clone());
    let h = existing.unwrap_or_else(|| {
        let h = cx.focus_handle();
        if let Some(p) = store(cx).players.get_mut(key) {
            p.focus = Some(h.clone());
        }
        h
    });
    window.focus(&h, cx);
}

pub fn handle_key(cx: &mut App, window: &mut Window, key: &SharedString, pressed: &str) -> bool {
    let Some(p) = cx.try_global::<Store>().and_then(|s| s.players.get(key)) else { return false };
    let (cur, duration) = (p.current(), p.duration().unwrap_or(0.));
    if dvp_enabled() {
        match pressed {
            "space" | "k" => toggle(cx, key),
            "j" | "left" => skip(cx, key, -10.),
            "l" | "right" => skip(cx, key, 10.),
            "m" => toggle_mute(cx),
            "f" => {
                let on = cx.try_global::<Store>().and_then(|s| s.fullscreen.clone()).is_none();
                set_fullscreen(cx, window, on.then(|| key.clone()));
            }
            "escape" if cx.try_global::<Store>().is_some_and(|s| s.fullscreen.is_some()) => set_fullscreen(cx, window, None),
            _ => return false,
        }
        return true;
    }
    match pressed {
        "space" => toggle(cx, key),
        "left" => seek_keep(cx, key, (cur - 5.).max(0.)),
        "right" => seek_keep(cx, key, (cur + 5.).min(duration)),
        "escape" if cx.try_global::<Store>().is_some_and(|s| s.fullscreen.is_some()) => set_fullscreen(cx, window, None),
        d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() => {
            let n = f64::from(d.as_bytes()[0] - b'0');
            seek_keep(cx, key, duration * n / 10.);
            if cx.try_global::<Store>().and_then(|s| s.players.get(key)).is_some_and(|p| p.state != State::Playing) {
                toggle(cx, key);
            }
        }
        _ => return false,
    }
    true
}

fn seek_keep(cx: &mut App, key: &SharedString, secs: f64) {
    let playing = cx.try_global::<Store>().and_then(|s| s.players.get(key)).is_some_and(|p| p.state == State::Playing);
    seek_to(cx, key, secs);
    if playing && let Some(p) = store(cx).players.get_mut(key) {
        p.state = State::Playing;
    }
}

fn legacy_button(id: String, icon_path: SharedString, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let group = SharedString::from(id);
    div()
        .id(ElementId::Name(group.clone()))
        .group(group.clone())
        .flex_none()
        .size(z(32.))
        .p(z(4.))
        .cursor_pointer()
        .active(|s| s.mt(z(1.)).mb(z(-1.)))
        .tooltip(crate::ui::tooltip::tip(tip))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            svg()
                .path(icon_path)
                .flex_none()
                .size(z(24.))
                .text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE))
                .group_hover(group, |s| s.text_color(gpui::white())),
        )
}

pub fn legacy_time(secs: Option<f64>) -> String {
    secs.map_or_else(
        || "-:--".to_owned(),
        |t| {
            let n = t.max(0.) as u64;
            format!("{}:{:02}", n / 60, n % 60)
        },
    )
}

pub fn legacy_time_pair(current: Option<f64>, duration: Option<f64>) -> (String, String) {
    let d = legacy_time(duration);
    let c = legacy_time(current);
    let width = d.chars().count();
    (format!("{c:0>width$}"), d)
}

struct LegacyBar {
    key: SharedString,
    state: State,
    current: f64,
    duration: Option<f64>,
    offset: f32,
    width: f32,
    hover: Option<f32>,
    dragging: bool,
    fullscreen: bool,
}

fn legacy_controls(b: LegacyBar, cx: &App) -> AnyElement {
    let key = b.key.clone();
    let (icon_name, tip) = match b.state {
        State::Playing => ("pause", crate::t!("video.pause")),
        State::Ended => ("media/replay", crate::t!("video.replay")),
        State::Paused => ("play", crate::t!("video.play")),
    };
    let k = key.clone();
    let play = legacy_button(format!("lvplay-{key}"), crate::icons::path(icon_name), tip).mr(z(8.)).on_click(move |_, window, cx| {
        toggle(cx, &k);
        window.refresh();
    });
    let (cur, dur) = legacy_time_pair(b.duration.map(|_| b.current.floor()), b.duration.map(f64::floor));
    let time = div()
        .flex_none()
        .whitespace_nowrap()
        .m(z(4.))
        .h(z(12.))
        .flex()
        .font_family(crate::theme::FONT_MONO())
        .text_size(z(12.))
        .line_height(z(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
        .child(cur)
        .child(div().mx(z(2.)).child("/"))
        .child(dur);
    let frac = match b.duration {
        Some(d) if d > 0. => (b.current / d).clamp(0., 1.) as f32,
        _ => 0.,
    };
    let bubble = match (if b.dragging { Some(frac) } else { b.hover }, b.duration) {
        (Some(f), Some(d)) => Some(super::voice_message::track_bubble(legacy_time(Some(f64::from(f) * d)), f)),
        _ => None,
    };
    let track = div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(z(32.))
        .mx(z(7.))
        .cursor_pointer()
        .child(
            timeline(Track {
                key: key.clone(),
                frac,
                duration: b.duration,
                cfg: ATTACHMENT,
                hover: b.hover,
                dragging: b.dragging,
                visible: true,
                shift: 0.,
                legacy: true,
            })
            .size_full(),
        )
        .child(div().absolute().left_0().right_0().top(z(13.)).children(bubble));
    let volume = super::voice_message::media_volume_control(legacy_volume_key(&key), format!("lv{key}"), 32., 24., cx);
    let kfs = key.clone();
    let fs_icon = crate::icons::path(if b.fullscreen { "call/fullscreen-exit" } else { "call/fullscreen" });
    let fullscreen = legacy_button(format!("lvfs-{key}"), fs_icon, crate::t!("video.fullscreen")).rounded(z(8.)).on_click(move |_, window, cx| {
        let on = cx.try_global::<Store>().and_then(|s| s.fullscreen.clone()).is_none();
        set_fullscreen(cx, window, on.then(|| kfs.clone()));
    });
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(z(-10. - b.offset * 42.))
        .h(z(42.))
        .pb(z(10.))
        .bg(tk(idx::OPACITY_BLACK_60))
        .flex()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(play)
        .when(b.fullscreen || b.width > 250., |d| d.child(time))
        .child(track)
        .child(volume)
        .child(fullscreen)
        .into_any_element()
}

fn legacy_pop(state: State, t: f32) -> AnyElement {
    let opacity = 0.8 * (if t < 200. { t / 200. } else { (1. - (t - 200.) / 200.).max(0.) });
    let scale = 1. + 1.5 * Spring::free(200., 80., t / 1000.);
    let path = crate::icons::path(if state == State::Playing { "play" } else { "pause" });
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(z(48. * scale))
                .rounded_full()
                .opacity(opacity)
                .bg(tk(idx::OPACITY_BLACK_60))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(path, 24. * scale)),
        )
        .into_any_element()
}

fn legacy_bar_for(cx: &App, key: &SharedString, width: f32, fullscreen: bool) -> Option<LegacyBar> {
    let p = cx.try_global::<Store>()?.players.get(key)?;
    p.clicked.then(|| LegacyBar {
        key: key.clone(),
        state: p.state,
        current: p.current(),
        duration: p.duration(),
        offset: p.legacy_bar.value(Instant::now()),
        width,
        hover: p.track_hover,
        dragging: p.seek.is_some(),
        fullscreen,
    })
}

fn legacy_surface(key: SharedString, src: SharedString, fullscreen: bool, cx: &App) -> gpui::Stateful<gpui::Div> {
    let focus = cx.try_global::<Store>().and_then(|s| s.players.get(&key)).and_then(|p| p.focus.clone());
    let (k_hover, k_move, k_click, k_key) = (key.clone(), key.clone(), key.clone(), key.clone());
    let failed = cx.try_global::<Store>().and_then(|s| s.players.get(&key)).is_some_and(|p| matches!(p.load, Load::Failed));
    div()
        .id(ElementId::Name(format!("{}-{key}", if fullscreen { "lvfs" } else { "lv" }).into()))
        .when_some(focus.filter(|_| !fullscreen), |d, h| d.track_focus(&h))
        .on_hover(move |over, window, cx| {
            if let Some(p) = store(cx).players.get_mut(&k_hover) {
                p.hovered = *over;
                p.moved = Instant::now();
                if !*over {
                    p.track_hover = None;
                }
            }
            ensure_ticker(cx);
            window.refresh();
        })
        .on_mouse_move(move |_, _, cx| {
            if let Some(p) = store(cx).players.get_mut(&k_move) {
                p.hovered = true;
                p.moved = Instant::now();
            }
            ensure_ticker(cx);
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            if handle_key(cx, window, &k_key, e.keystroke.key.as_str()) {
                cx.stop_propagation();
                window.refresh();
            }
        })
        .on_click(move |_, window, cx| {
            if failed {
                cx.open_url(&src);
                return;
            }
            ensure(cx, &k_click, &src, false);
            if !fullscreen {
                focus_player(cx, window, &k_click);
            }
            toggle(cx, &k_click);
            window.refresh();
        })
}

fn legacy_attachment(spec: Spec, cx: &App) -> AnyElement {
    let key = spec.key.clone();
    let st = cx.try_global::<Store>();
    let p = st.and_then(|s| s.players.get(&key));
    let fs_here = st.and_then(|s| s.fullscreen.as_ref()) == Some(&key);
    let clicked = p.is_some_and(|p| p.clicked) && !fs_here;
    let width = p.and_then(|p| p.measured).unwrap_or(spec.width);
    let now = Instant::now();
    let pop = p.and_then(|p| p.pop).map(|(t, s)| (s, now.duration_since(t).as_secs_f32() * 1000.));
    let show_frame = clicked && p.is_some_and(|p| p.image.is_some());
    let bar = legacy_bar_for(cx, &key, width, false).filter(|_| clicked);
    legacy_surface(key.clone(), spec.src.clone(), false, cx)
        .group("dvp-inactive")
        .relative()
        .size_full()
        .rounded(z(spec.radius))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .cursor_pointer()
        .child(tracker(key.clone(), spec.src.clone(), false, None, false).absolute().inset_0())
        .child(frame_layer(cx, &key, (!show_frame).then(|| spec.poster.clone()), show_frame))
        .when(!clicked, |d| d.child(inactive_button()))
        .when_some(bar, |d, b| d.child(legacy_controls(b, cx)))
        .when_some(pop.filter(|(_, t)| clicked && *t < 450.), |d, (s, t)| d.child(legacy_pop(s, t)))
        .into_any_element()
}

fn legacy_fullscreen_layer(cx: &App) -> Option<AnyElement> {
    let st = cx.try_global::<Store>()?;
    let key = st.fullscreen.clone()?;
    let p = st.players.get(&key)?;
    let src = p.src.clone();
    let focus = st.focus.clone();
    let now = Instant::now();
    let pop = p.pop.map(|(t, s)| (s, now.duration_since(t).as_secs_f32() * 1000.));
    let bar = legacy_bar_for(cx, &key, st.fs_width, true);
    Some(
        legacy_surface(key.clone(), src.clone(), true, cx)
            .absolute()
            .inset_0()
            .occlude()
            .when_some(focus, |d, h| d.track_focus(&h))
            .bg(gpui::black())
            .overflow_hidden()
            .child(tracker(key.clone(), src, false, None, true).absolute().inset_0())
            .child(frame_layer(cx, &key, None, true))
            .when_some(bar, |d, b| d.child(legacy_controls(b, cx)))
            .when_some(pop.filter(|(_, t)| *t < 450.), |d, (s, t)| d.child(legacy_pop(s, t)))
            .into_any_element(),
    )
}

pub fn viewer_key(key: &str) -> SharedString {
    format!("viewer:{key}").into()
}

pub fn open_in_viewer(cx: &mut App, key: &SharedString, mut viewer: crate::media::ViewerImage) {
    let Some(p) = cx.try_global::<Store>().and_then(|s| s.players.get(key)) else { return };
    let (src, at) = (p.src.clone(), if p.state == State::Ended { 0. } else { p.current() });
    if p.state == State::Playing {
        set_state(cx, key, State::Paused, false);
    }
    let vk = viewer_key(key);
    let mut player = Player::new(src, false);
    player.position = at;
    player.active = true;
    player.clicked = true;
    store(cx).players.insert(vk.clone(), player);
    viewer.video = Some(key.clone());
    crate::media::open_viewer(viewer, cx);
    set_state(cx, &vk, State::Playing, false);
    fetch(cx, &vk);
}

pub fn viewer_element(key: &SharedString, poster: SharedString, (w, h): (f32, f32), portrait: bool, cx: &App) -> AnyElement {
    let vk = viewer_key(key);
    let src = cx.try_global::<Store>().and_then(|s| s.players.get(&vk)).map_or_else(|| key.clone(), |p| p.src.clone());
    div()
        .w(px(w))
        .h(px(h))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(attachment(Spec { key: vk, src, poster, layout: Layout::Single, portrait, width: w / f32::from(z(1.)).max(0.01), radius: 8., viewer: None }, cx))
        .into_any_element()
}

pub fn shot(spec: &str, window: &mut Window, cx: &mut App) {
    let now = Instant::now();
    store(cx).silent = true;
    let keys: Vec<SharedString> = store(cx).players.iter().filter(|(_, p)| !p.looped && p.painted.is_some()).map(|(k, _)| k.clone()).collect();
    for key in keys {
        match spec {
            "play" => {
                if let Some(p) = store(cx).players.get_mut(&key) {
                    p.hovered = true;
                    p.moved = now;
                }
                if cx.try_global::<Store>().and_then(|s| s.players.get(&key)).is_some_and(|p| p.state != State::Playing) {
                    toggle(cx, &key);
                }
            }
            "pause" => {
                if cx.try_global::<Store>().and_then(|s| s.players.get(&key)).is_some_and(|p| p.state == State::Playing) {
                    toggle(cx, &key);
                }
            }
            "hover" => {
                if let Some(p) = store(cx).players.get_mut(&key) {
                    p.hovered = true;
                    p.moved = now;
                }
            }
            "fullscreen" => {
                set_fullscreen(cx, window, Some(key.clone()));
                break;
            }
            "exit" => {
                set_fullscreen(cx, window, None);
                break;
            }
            _ => {
                if let Some(t) = spec.strip_prefix("seek=").and_then(|v| v.parse::<f64>().ok()) {
                    seek_keep(cx, &key, t);
                }
            }
        }
    }
    ensure_ticker(cx);
    window.refresh();
}

pub fn exit_fullscreen(window: &mut Window, cx: &mut App) -> bool {
    if cx.try_global::<Store>().is_none_or(|s| s.fullscreen.is_none()) {
        return false;
    }
    set_fullscreen(cx, window, None);
    true
}

pub fn shot_bounds(cx: &App) -> Option<Bounds<Pixels>> {
    let fresh = |p: &&Player| p.painted.is_some_and(|t| t.elapsed() < OFFSCREEN);
    cx.try_global::<Store>()?.players.values().filter(|p| !p.looped).filter(fresh).max_by_key(|p| p.painted).and_then(|p| p.shown_at)
}

pub fn shot_time(cx: &App) -> Option<f64> {
    cx.try_global::<Store>()?.players.values().filter(|p| !p.looped && p.clicked).map(Player::current).reduce(f64::max)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_follow_the_client_format() {
        assert_eq!(format_time(0.), "0:00");
        assert_eq!(format_time(65.9), "1:05");
        assert_eq!(format_time(3725.), "1:02:05");
        assert_eq!(format_time(-12.), "-0:12");
    }

    #[test]
    fn control_bar_min_width_matches_the_client_formula() {
        let s = Shown { skip: true, inline_volume: true, compact_time: false, trailing: 0, speed: true, fullscreen: true };
        let t = time_width(65., 12., false);
        assert!((t - ((0.389 + 2. * (0.562 * 3. + 0.278) + 0.333) * 12. + 8.)).abs() < 1e-4);
        let w = min_width(&ATTACHMENT, &s, 65.);
        let expect = (2. * 4. + (24. + 2. * 28.) + (4. + 24. + 66. + 4. + 0. + t + 8.) + (2. * 24. + 4.)).ceil();
        assert_eq!(w, expect);
        assert!(min_width(&FULLSCREEN, &s, 65.) > w);
        assert!(time_width(3600., 12., true) > time_width(59., 12., true));
    }

    #[test]
    fn narrow_bars_drop_controls_like_the_client() {
        let wide = plan(Layout::Single, false, 550., 30., false);
        assert!(wide.skip && wide.speed && !wide.floating_volume && !wide.autohide_volume);
        let mid = plan(Layout::Single, false, 300., 30., false);
        assert!(mid.skip && mid.floating_volume && mid.speed);
        let tiny = plan(Layout::Single, false, 200., 30., false);
        assert!(!tiny.skip && !tiny.speed && !tiny.compact_time);
        assert!(plan(Layout::Single, false, 150., 30., false).compact_time);
        let tile = plan(Layout::Tile, false, 550., 30., false);
        assert!(!tile.skip && tile.autohide_volume);
        let portrait = plan(Layout::Single, true, 300., 30., false);
        assert!(!portrait.skip && !portrait.speed && portrait.floating_volume);
        let fs = plan(Layout::Single, true, 1920., 30., true);
        assert!(fs.speed && !fs.skip && !fs.floating_volume);
        let narrow_fs = plan(Layout::Single, false, 300., 30., true);
        assert!(narrow_fs.floating_volume && narrow_fs.compact_time && !narrow_fs.skip);
        assert!(!collapses(3, false));
        assert!(collapses(2, true));
        assert!(!collapses(1, true));
    }

    #[test]
    fn clamped_springs_settle_on_their_first_crossing() {
        assert_eq!(Spring::progress(250., 5., 0.), 0.);
        let mid = Spring::progress(250., 5., 0.05);
        assert!(mid > 0.2 && mid < 1., "{mid}");
        assert_eq!(Spring::progress(250., 5., 0.12), 1.);
        assert_eq!(Spring::progress(100., 3., 0.18), 1.);
        assert!(Spring::progress(100., 3., 0.1) < 1.);
        let mut s = Spring::new(0., 250., 5.);
        let t = Instant::now();
        s.set(1., t, false);
        assert!(s.moving(t + Duration::from_millis(30)));
        assert_eq!(s.value(t + Duration::from_millis(200)), 1.);
        s.set(0., t, true);
        assert_eq!(s.value(t), 0.);
    }

    #[test]
    fn generated_icons() {
        assert_eq!(volume_icon_path(1.).as_ref(), "icons/dvp/volume-10-10-10-0.svg");
        assert_eq!(volume_icon_path(0.5).as_ref(), "icons/dvp/volume-10-5-0-0.svg");
        assert_eq!(volume_icon_path(0.).as_ref(), "icons/dvp/volume-0-0-0-1.svg");
        let v = icon_svg("icons/dvp/volume-10-5-0-0.svg").unwrap();
        assert!(v.contains(r#"stroke-opacity="0.5" d="M17.5 12Q17.5 8.5 15 6.5""#));
        assert!(v.contains(r#"stroke-opacity="0" d="m15 9 6 6m-6 0 6-6""#));
        assert!(icon_svg("icons/dvp/seek-back.svg").unwrap().contains("scale(.19361)"));
        assert!(icon_svg("icons/dvp/volume-1-2.svg").is_none());
        assert!(icon_svg("icons/other.svg").is_none());
    }
}
