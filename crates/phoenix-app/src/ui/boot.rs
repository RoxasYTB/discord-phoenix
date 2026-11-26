use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*, px};
use serde_json::Value;

use crate::{
    app::{PhoenixApp, Screen},
    theme::{c, generated::idx, tok},
};

pub const CONTAINER_FADE: Duration = Duration::from_millis(200);
pub const CONTENT_FADE: Duration = Duration::from_millis(150);
pub const PROBLEMS_AFTER: Duration = Duration::from_secs(10);
pub const PROBLEMS_SLIDE: Duration = Duration::from_millis(200);
pub const TIPS: usize = 58;
const EN_ONLY_TIP: usize = 58;
const MAIN_CAP: Duration = Duration::from_secs(5);
const TWITTER_SUPPORT: &str = "https://twitter.com/discord_support";
const STATUS_PAGE: &str = "https://discordstatus.com";

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed) || std::env::var_os("PHX_BOOT_SCREEN").is_some_and(|v| v != "0")
}

fn hold() -> Option<Duration> {
    std::env::var("PHX_BOOT_HOLD_MS").ok().and_then(|v| v.parse().ok()).map(Duration::from_millis)
}

pub struct Boot {
    started: Instant,
    tip: usize,
    hide_at: Option<Instant>,
    main_since: Option<Instant>,
    done: bool,
    offline: bool,
    shown_at: Option<Instant>,
}

impl Default for Boot {
    fn default() -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        Self { started: Instant::now(), tip: pick_tip(seed, crate::i18n::locale()), hide_at: None, main_since: None, done: !enabled(), offline: false, shown_at: None }
    }
}

pub fn pick_tip(seed: u64, locale: &str) -> usize {
    let n = if locale.starts_with("en-") { TIPS + 1 } else { TIPS };
    let mut r = crate::ui::chat::skeleton::Rng::new(seed);
    r.int(0, n as u32 - 1) as usize
}

pub fn ease_out(t: f32) -> f32 {
    crate::ui::states::cubic_bezier(0., 0., 0.58, 1.)(t.clamp(0., 1.))
}

pub fn stable(app: &PhoenixApp) -> bool {
    if !crate::fonts::ready() {
        return false;
    }
    if app.screen == Screen::Login {
        return !app.logging_in;
    }
    let text_like = |ch: crate::model::Id| {
        app.channels.get(&ch).is_none_or(|c| {
            !matches!(c.kind, crate::model::ChannelKind::Forum | crate::model::ChannelKind::Voice | crate::model::ChannelKind::Category)
        })
    };
    match app.selected_channel {
        Some(ch) if text_like(ch) => app.messages.get(&ch).is_some_and(|e| e.loaded_once),
        _ => true,
    }
}

pub struct Frame {
    pub container: f32,
    pub content: f32,
    pub spinner_scale: f32,
    pub problems: f32,
}

impl Boot {
    pub fn showing(&self) -> bool {
        !self.done
    }

    pub fn offline(&self) -> bool {
        self.offline
    }

    pub fn connection(&mut self, up: bool, now: Instant) {
        self.connection_with(up, now, enabled());
    }

    fn connection_with(&mut self, up: bool, now: Instant, screen: bool) {
        if up || self.offline {
            self.offline = !up && self.offline;
            return;
        }
        self.offline = true;
        if !screen {
            return;
        }
        if self.done || self.hide_at.is_some() {
            self.done = false;
            self.hide_at = None;
            self.started = now;
            self.shown_at = Some(now);
        }
    }

    pub fn frame(&mut self, ready: bool, now: Instant) -> Option<Frame> {
        if self.done {
            return None;
        }
        if ready && self.hide_at.is_none() {
            self.hide_at = Some(now);
            self.shown_at = None;
            crate::perf::mark("boot_screen_fade");
        }
        let (container, content) = match self.hide_at {
            Some(at) => {
                let t = now.duration_since(at);
                if t >= CONTAINER_FADE {
                    self.done = true;
                    crate::perf::mark("boot_screen_gone");
                    return None;
                }
                let k = ease_out(t.as_secs_f32() / CONTAINER_FADE.as_secs_f32());
                let j = ease_out(t.as_secs_f32() / CONTENT_FADE.as_secs_f32());
                (1. - k, 1. - j)
            }
            None => match self.shown_at {
                Some(at) => {
                    let t = now.duration_since(at);
                    if t >= CONTAINER_FADE {
                        self.shown_at = None;
                    }
                    let k = ease_out(t.as_secs_f32() / CONTAINER_FADE.as_secs_f32());
                    let j = ease_out(t.as_secs_f32() / CONTENT_FADE.as_secs_f32());
                    (k, j)
                }
                None => (1., 1.),
            },
        };
        let problems = match now.duration_since(self.started).checked_sub(PROBLEMS_AFTER) {
            Some(t) if self.hide_at.is_none() => ease_out(t.as_secs_f32() / PROBLEMS_SLIDE.as_secs_f32()),
            _ => 0.,
        };
        Some(Frame { container, content, spinner_scale: 1. - 0.5 * (1. - content), problems })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Text { text: String, bold: bool, strike: bool },
    Key(&'static str),
}

pub fn hook_keys(name: &str) -> Option<&'static str> {
    Some(match name {
        "tabHook" => "tab",
        "F6Hook" => "f6",
        "quickSwitcherHook" => "ctrl-k",
        "markUnreadHook" => "alt-click",
        "markServerUnreadHook" => "shift-escape",
        "navigateUnreadHook" => "shift-alt-up",
        "downHook" => "down",
        "keyboardShortcutsHook" => "ctrl-/",
        "messageNewlineHook" => "shift-enter",
        "shiftHook" => "shift",
        "upHook" => "up",
        _ => return None,
    })
}

fn arg(name: &str) -> &'static str {
    if name == "asterisks" { "**" } else { "" }
}

fn push_text(out: &mut Vec<Seg>, s: &str, bold: bool, strike: bool) {
    if s.is_empty() {
        return;
    }
    if let Some(Seg::Text { text, bold: b, strike: k }) = out.last_mut()
        && *b == bold
        && *k == strike
    {
        text.push_str(s);
        return;
    }
    out.push(Seg::Text { text: s.to_owned(), bold, strike });
}

pub fn parse_markup(s: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let (mut bold, mut strike) = (false, false);
    let mut rest = s;
    while !rest.is_empty() {
        let next = rest.find(['{', '<']).unwrap_or(rest.len());
        push_text(&mut out, &rest[..next], bold, strike);
        rest = &rest[next..];
        if rest.is_empty() {
            break;
        }
        let close = if rest.starts_with('{') { '}' } else { '>' };
        let Some(end) = rest.find(close) else {
            push_text(&mut out, rest, bold, strike);
            break;
        };
        let tag = &rest[1..end];
        match (close, tag) {
            ('}', name) => match hook_keys(name) {
                Some(k) => out.push(Seg::Key(k)),
                None => push_text(&mut out, arg(name), bold, strike),
            },
            (_, "b") => bold = true,
            (_, "/b") => bold = false,
            (_, "del") => strike = true,
            (_, "/del") => strike = false,
            _ => push_text(&mut out, &rest[..=end], bold, strike),
        }
        rest = &rest[end + 1..];
    }
    out
}

pub fn walk_ast(node: &Value, bold: bool, strike: bool, out: &mut Vec<Seg>) {
    match node {
        Value::String(s) => push_text(out, s, bold, strike),
        Value::Array(items) if items.first().and_then(Value::as_u64).is_some() => {
            let name = items.get(1).and_then(Value::as_str).unwrap_or("");
            match items[0].as_u64() {
                Some(1) => push_text(out, arg(name), bold, strike),
                Some(8) => match (name, hook_keys(name)) {
                    (_, Some(k)) => out.push(Seg::Key(k)),
                    ("$b", _) => walk_ast(items.get(2).unwrap_or(&Value::Null), true, strike, out),
                    ("$del", _) => walk_ast(items.get(2).unwrap_or(&Value::Null), bold, true, out),
                    _ => walk_ast(items.get(2).unwrap_or(&Value::Null), bold, strike, out),
                },
                _ => {}
            }
        }
        Value::Array(items) => items.iter().for_each(|i| walk_ast(i, bold, strike, out)),
        _ => {}
    }
}

pub fn tip_key(i: usize) -> String {
    format!("boot.tip_{:02}", i + 1)
}

pub fn tip_segments(i: usize) -> Vec<Seg> {
    let key = tip_key(i.min(EN_ONLY_TIP));
    let loc = crate::i18n::locale();
    if loc != "en-US"
        && let Some(m) = crate::i18n::messages::mappings().get(key.as_str())
        && let Some(table) = crate::i18n::remote::get(loc)
        && let Some(ast) = table.get(m.hash)
    {
        let mut out = Vec::new();
        walk_ast(ast, false, false, &mut out);
        return out;
    }
    parse_markup(&crate::t!(key.as_str()))
}

fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if ch == ' ' {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn tip_line(segs: &[Seg], color: gpui::Hsla) -> gpui::Div {
    let mut row = div().max_w(px(300.)).flex().flex_wrap().justify_center().items_center().text_size(px(16.)).line_height(px(20.)).text_color(color);
    for s in segs {
        match s {
            Seg::Text { text, bold, strike } => {
                for w in words(text) {
                    let w = w.replace(' ', "\u{a0}");
                    row = row.child(
                        div()
                            .when(*bold, |d| d.font_weight(FontWeight::BOLD))
                            .when(*strike, |d| d.line_through())
                            .child(SharedString::from(w)),
                    );
                }
            }
            Seg::Key(k) => {
                row = row.child(div().mr(px(-2.)).relative().top(px(-1.)).child(crate::shortcuts::combo(k)));
            }
        }
    }
    row
}

fn link(label: SharedString, icon: Option<&'static str>, url: &'static str) -> gpui::Stateful<gpui::Div> {
    let color = c(tok(idx::TEXT_LINK));
    div()
        .id(url)
        .flex()
        .items_center()
        .text_size(px(16.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .cursor_pointer()
        .hover(|d| d.opacity(0.8))
        .when_some(icon, |d, i| d.child(gpui::svg().path(crate::icons::path(i)).w(px(16.)).h(px(16.)).mr(px(4.)).text_color(color)))
        .child(label)
        .on_click(move |_, _, cx| cx.open_url(url))
}

pub fn render(app: &mut PhoenixApp, window: &mut Window, _cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if app.boot.done {
        return None;
    }
    if app.screen == Screen::Main && app.boot.main_since.is_none() {
        app.boot.main_since = Some(Instant::now());
    }
    let capped = app.boot.main_since.is_some_and(|t| t.elapsed() >= MAIN_CAP);
    let held = hold().is_some_and(|h| app.boot.started.elapsed() < h);
    let ready = (stable(app) || capped) && !held && !app.boot.offline;
    let f = app.boot.frame(ready, Instant::now())?;
    window.request_animation_frame();
    let spinner = match crate::splash::spinner() {
        Some(image) => img(image).size_full().into_any_element(),
        None => div().size_full().into_any_element(),
    };
    let spin_size = 200. * f.spinner_scale;
    let segs = tip_segments(app.boot.tip);
    let text = div()
        .relative()
        .top(px(-20.))
        .opacity(f.content)
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .max_w(px(300.))
                .mb(px(8.))
                .text_size(px(16.))
                .line_height(px(21.33))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(crate::t!("boot.did_you_know")),
        )
        .child(tip_line(&segs, c(tok(idx::TEXT_DEFAULT))));
    let problems_h = 32. + 14. * 1.2 + 8. + 16.;
    let problems = div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(-problems_h * (1. - f.problems)))
        .pb(px(32.))
        .flex()
        .flex_col()
        .items_center()
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(div().mb(px(8.)).text_size(px(14.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("boot.problems")))
        .child(
            div()
                .flex()
                .child(link(crate::t!("boot.tweet_us"), Some("x-neutral"), TWITTER_SUPPORT))
                .child(div().ml(px(20.)).child(link(crate::t!("boot.server_status"), None, STATUS_PAGE))),
        );
    Some(
        div()
            .id("boot-screen")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .overflow_hidden()
            .opacity(f.container)
            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .flex()
            .items_center()
            .justify_center()
            .font_family(crate::fonts::ui_family())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(div().size(px(200.)).flex().items_center().justify_center().child(div().size(px(spin_size)).opacity(f.content).child(spinner)))
                    .child(text),
            )
            .child(problems)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_keeps_keys_bold_and_strike() {
        assert_eq!(
            parse_markup("{tabHook} to get started."),
            vec![Seg::Key("tab"), Seg::Text { text: " to get started.".into(), bold: false, strike: false }]
        );
        assert_eq!(
            parse_markup("type {asterisks} to make them <b>bold</b>."),
            vec![
                Seg::Text { text: "type ** to make them ".into(), bold: false, strike: false },
                Seg::Text { text: "bold".into(), bold: true, strike: false },
                Seg::Text { text: ".".into(), bold: false, strike: false },
            ]
        );
        assert_eq!(parse_markup("a <del>secret</del>")[1], Seg::Text { text: "secret".into(), bold: false, strike: true });
    }

    #[test]
    fn client_ast_walks_to_the_same_segments() {
        let ast = serde_json::json!(["Utilise ", [8, "keyboardShortcutsHook", []], " pour ", [8, "$b", ["gras"]]]);
        let mut out = Vec::new();
        walk_ast(&ast, false, false, &mut out);
        assert_eq!(out[1], Seg::Key("ctrl-/"));
        assert_eq!(out[3], Seg::Text { text: "gras".into(), bold: true, strike: false });
    }

    #[test]
    fn every_tip_has_a_string_and_the_english_only_one_is_last() {
        for i in 0..=EN_ONLY_TIP {
            let s = crate::i18n::t_in("en-US", &tip_key(i), &[]);
            assert!(!s.starts_with("boot."), "{s}");
        }
        for seed in 0..200 {
            assert!(pick_tip(seed, "fr") < TIPS);
            assert!(pick_tip(seed, "en-US") <= EN_ONLY_TIP);
        }
    }

    #[test]
    fn fade_runs_like_the_client_and_then_unmounts() {
        let mut b = Boot { started: Instant::now(), tip: 0, hide_at: None, main_since: None, done: false, offline: false, shown_at: None };
        let t0 = Instant::now();
        let f = b.frame(false, t0).unwrap();
        assert_eq!((f.container, f.content, f.spinner_scale), (1., 1., 1.));
        let f = b.frame(true, t0).unwrap();
        assert_eq!(f.container, 1.);
        let f = b.frame(true, t0 + CONTENT_FADE).unwrap();
        assert!(f.content.abs() < 1e-3 && (f.spinner_scale - 0.5).abs() < 1e-3 && f.container > 0.);
        assert!(b.frame(true, t0 + CONTAINER_FADE).is_none());
        assert!(!b.showing());
    }

    #[test]
    fn problems_slide_in_after_ten_seconds() {
        let start = Instant::now();
        let mut b = Boot { started: start, tip: 0, hide_at: None, main_since: None, done: false, offline: false, shown_at: None };
        assert_eq!(b.frame(false, start + Duration::from_secs(9)).unwrap().problems, 0.);
        assert!((b.frame(false, start + PROBLEMS_AFTER + PROBLEMS_SLIDE).unwrap().problems - 1.).abs() < 1e-3);
    }

    #[test]
    fn a_reset_session_brings_the_screen_back_until_the_gateway_is_up() {
        let t0 = Instant::now();
        let mut b = Boot { started: t0, tip: 0, hide_at: None, main_since: None, done: true, offline: false, shown_at: None };
        b.connection_with(true, t0, true);
        assert!(!b.showing());
        b.connection_with(false, t0, true);
        assert!(b.showing() && b.offline());
        let f = b.frame(false, t0).unwrap();
        assert!(f.container.abs() < 1e-3 && f.content.abs() < 1e-3 && (f.spinner_scale - 0.5).abs() < 1e-3);
        let f = b.frame(false, t0 + CONTAINER_FADE).unwrap();
        assert!((f.container - 1.).abs() < 1e-3 && (f.content - 1.).abs() < 1e-3);
        assert!(b.frame(false, t0 + CONTAINER_FADE * 2).is_some_and(|f| f.container == 1.));
        assert_eq!(b.frame(false, t0 + Duration::from_secs(9)).unwrap().problems, 0.);
        assert!(b.frame(false, t0 + PROBLEMS_AFTER + PROBLEMS_SLIDE).unwrap().problems > 0.99);
        b.connection_with(false, t0 + Duration::from_secs(20), true);
        assert_eq!(b.started, t0);
        b.connection_with(true, t0 + Duration::from_secs(30), true);
        assert!(!b.offline());
        let t1 = t0 + Duration::from_secs(30);
        assert!((b.frame(true, t1).unwrap().container - 1.).abs() < 1e-3);
        assert!(b.frame(true, t1 + CONTAINER_FADE).is_none());
    }

    #[test]
    fn no_screen_when_the_boot_screen_is_off() {
        let t0 = Instant::now();
        let mut b = Boot { started: t0, tip: 0, hide_at: None, main_since: None, done: true, offline: false, shown_at: None };
        b.connection_with(false, t0, false);
        assert!(!b.showing() && b.offline());
    }

    #[test]
    fn going_offline_during_start_up_keeps_the_screen() {
        let t0 = Instant::now();
        let mut b = Boot { started: t0, tip: 0, hide_at: None, main_since: None, done: false, offline: false, shown_at: None };
        b.connection_with(false, t0, true);
        assert_eq!(b.shown_at, None);
        assert!(b.offline());
        assert_eq!(b.frame(false, t0).unwrap().container, 1.);
    }
}
