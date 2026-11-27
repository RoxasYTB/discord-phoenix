use crate::ui::tooltip::Tipped as _;
use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, svg};

use crate::app::PhoenixApp;
use crate::model::{ChannelKind, Id};
use crate::theme::{DEFAULT_AVATAR, c, generated::idx, tok, z};
use crate::ui::avatar;

fn t(i: usize) -> Hsla {
    crate::theme::c_for(tok(i), crate::ui::settings::AppTheme::Onyx)
}

thread_local! {
    static SHOWN: Cell<Option<Id>> = const { Cell::new(None) };
    static CHAT_OPEN: Cell<bool> = const { Cell::new(false) };
    static FOCUS: Cell<Focus> = const { Cell::new(Focus::Auto) };
    static LAST_MOVE: Cell<Option<Instant>> = const { Cell::new(None) };
    static WAKE: Cell<Option<Instant>> = const { Cell::new(None) };
    static STRIP_OPEN: Cell<bool> = const { Cell::new(true) };
    static SPLIT_H: Cell<Option<f32>> = const { Cell::new(None) };
    static SPLIT_LOADED: Cell<bool> = const { Cell::new(false) };
    static SPLIT_DRAG: Cell<bool> = const { Cell::new(false) };
    static SPLIT_TOP: Cell<f32> = const { Cell::new(0.) };
    static IN_STRIP: Cell<bool> = const { Cell::new(false) };
    static DRAWN: std::cell::RefCell<Vec<(Id, bool, f32, f32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn video_fit(in_strip: bool, aspect: Option<f32>) -> gpui::ObjectFit {
    match aspect {
        Some(a) if in_strip && a >= 1. => gpui::ObjectFit::Cover,
        _ => gpui::ObjectFit::Contain,
    }
}

pub fn device_pixels(w: f32, h: f32, scale: f32) -> (u32, u32) {
    ((w * scale).round().max(0.) as u32, (h * scale).round().max(0.) as u32)
}

fn report_video_view(app: &PhoenixApp, voice_users: &[Id], focus: Option<&Tile>, scale: f32) {
    let drawn = DRAWN.with(|d| std::mem::take(&mut *d.borrow_mut()));
    let mut view = phoenix_voice::sink_wants::CallView { incoming_video: true, ..Default::default() };
    view.others = voice_users.iter().filter(|u| **u != app.me.id).count();
    view.selected = focus.filter(|t| !t.stream && t.user != app.me.id).map(|t| t.user.0);
    {
        let prefs = crate::voice::prefs::prefs().lock();
        view.local_mutes = voice_users.iter().filter(|u| **u != app.me.id && prefs.user(**u).muted).map(|u| u.0).collect();
    }
    for (user, stream, w, h) in drawn {
        let (pw, ph) = device_pixels(w, h, scale);
        if stream {
            if user != app.me.id
                && let Some(key) = stream_key(app, user)
            {
                crate::voice::call::set_stream_view(&key, pw, ph);
            }
        } else if user != app.me.id {
            let px = pw as u64 * ph as u64;
            let e = view.pixels.entry(user.0).or_insert(0);
            *e = (*e).max(px);
        }
    }
    crate::voice::call::set_call_view(view);
}

pub const SPLIT_MIN_H: f32 = 230.;
pub const SPLIT_VOICE_H: f32 = 240.;
pub const SPLIT_POPPED_H: f32 = 154.;
pub const SPLIT_CHAT_RESERVE: f32 = 200.;

pub fn private_split(app: &PhoenixApp) -> bool {
    active(app) && app.voice_channel.and_then(|c| app.channels.get(&c)).is_some_and(|c| c.guild.is_none())
}

fn split_path() -> Option<std::path::PathBuf> {
    crate::paths::config_file("call_header_height")
}

pub fn stored_split_height() -> Option<f32> {
    if !SPLIT_LOADED.with(|l| l.replace(true)) {
        let v = split_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| s.trim().parse::<f32>().ok());
        SPLIT_H.with(|h| h.set(v));
    }
    SPLIT_H.with(Cell::get)
}

pub fn set_split_height(h: Option<f32>, persist: bool) {
    SPLIT_LOADED.with(|l| l.set(true));
    SPLIT_H.with(|c| c.set(h));
    if persist && let Some(p) = split_path() {
        let _ = p.parent().map(std::fs::create_dir_all);
        let _ = match h {
            Some(h) => std::fs::write(p, format!("{}\n", h.round())),
            None => std::fs::remove_file(p),
        };
    }
}

pub fn split_height(stored: Option<f32>, column_h: f32, video: bool) -> f32 {
    let max = column_h - SPLIT_CHAT_RESERVE;
    match stored {
        Some(h) => h.min(max).max(SPLIT_MIN_H),
        None if video => column_h * 0.5,
        None => SPLIT_VOICE_H,
    }
}

pub fn voice_avatar_size(n: usize, width: f32) -> f32 {
    if (80. + 16.) * n as f32 > width { 40. } else { 80. }
}

pub fn set_strip_open(open: bool) {
    STRIP_OPEN.with(|c| c.set(open));
}

pub fn strip_open() -> bool {
    STRIP_OPEN.with(|c| c.get())
}

pub const FADE: Duration = Duration::from_millis(200);

fn ease_in_out(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

pub fn overlay_opacity() -> (f32, bool) {
    let Some(last) = LAST_MOVE.with(|c| c.get()) else { return (0., false) };
    let since = last.elapsed();
    if since >= IDLE_TIMEOUT {
        let p = (since - IDLE_TIMEOUT).as_secs_f32() / FADE.as_secs_f32();
        return (1. - ease_in_out(p), p < 1.);
    }
    match WAKE.with(|c| c.get()) {
        Some(w) if w.elapsed() < FADE => (ease_in_out(w.elapsed().as_secs_f32() / FADE.as_secs_f32()), true),
        _ => (1., false),
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum Collapse {
    None,
    NoInvite,
    Minimal,
}

pub fn collapse(width: f32) -> Collapse {
    if width <= 550. {
        Collapse::Minimal
    } else if width <= 650. {
        Collapse::NoInvite
    } else {
        Collapse::None
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Focus {
    Auto,
    Grid,
    Tile(Id, bool),
}

pub fn set_focus(f: Focus) {
    FOCUS.with(|c| c.set(f));
}

pub fn focus() -> Focus {
    FOCUS.with(|c| c.get())
}

pub const IDLE_TIMEOUT: Duration = Duration::from_millis(2000);

pub fn idle() -> bool {
    LAST_MOVE.with(|c| c.get()).is_none_or(|t| t.elapsed() >= IDLE_TIMEOUT)
}

pub fn touch() {
    LAST_MOVE.with(|c| c.set(Some(Instant::now())));
}

pub fn show(ch: Id) {
    SHOWN.with(|s| s.set(Some(ch)));
}

pub fn set_chat_open(open: bool) {
    CHAT_OPEN.with(|c| c.set(open));
}

pub fn chat_open() -> bool {
    CHAT_OPEN.with(|c| c.get())
}

pub fn active(app: &PhoenixApp) -> bool {
    let Some(ch) = SHOWN.with(|s| s.get()) else { return false };
    app.selected_channel == Some(ch)
        && app.voice_channel == Some(ch)
        && app.channels.get(&ch).is_some_and(|c| c.guild.is_none() || matches!(c.kind, ChannelKind::Voice | ChannelKind::Stage))
}

const POPOUT: &str = "icons/call/popout.svg";
const FULLSCREEN: &str = "icons/call/fullscreen.svg";
const CAMERA_OFF: &str = "icons/call/camera-off.svg";

pub fn svg_asset(path: &str) -> Option<&'static str> {
    Some(match path {
        POPOUT => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M14 4h6v6M20 4l-8 8M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>"#,
            "</svg>"
        ),
        FULLSCREEN => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4"/>"#,
            "</svg>"
        ),
        CAMERA_OFF => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M4 5h11a2 2 0 0 1 2 2v2.4l3.6-2.6A1 1 0 0 1 22 7.6v8.8a1 1 0 0 1-1.4.8L17 14.6V17a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/>"#,
            r#"<path d="M3 3l18 18" stroke="black" stroke-width="2.4" stroke-linecap="round"/>"#,
            "</svg>"
        ),
        "icons/call/fullscreen-exit.svg" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            r#"<path d="M9 4v4a1 1 0 0 1-1 1H4M20 9h-4a1 1 0 0 1-1-1V4M15 20v-4a1 1 0 0 1 1-1h4M4 15h4a1 1 0 0 1 1 1v4"/>"#,
            "</svg>"
        ),
        "icons/golive/screen.svg" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<rect x="2" y="3" width="20" height="13" rx="3"/><rect x="11" y="16" width="2" height="4"/><rect x="8" y="19" width="8" height="2" rx="1"/>"#,
            "</svg>"
        ),
        "icons/golive/window.svg" => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<rect x="1" y="3" width="22" height="18" rx="3"/>"#,
            "</svg>"
        ),
        _ => return None,
    })
}

fn glyph(path: impl Into<SharedString>, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(path.into()).size(z(size)).flex_none().text_color(color)
}

fn call_key(id: &'static str) -> u64 {
    crate::ui::states::key(("call-btn", id))
}

fn anim(id: &'static str, kind: crate::icons::lottie_icons::Lottie, on: bool, color: Hsla, fallback: gpui::Svg) -> AnyElement {
    crate::ui::hover_icon::icon(call_key(id), kind, on, 20. * crate::theme::zoom(), (color, color), || fallback.into_any_element())
}

fn named(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    glyph(crate::icons::path(name), size, color)
}

struct Tile {
    user: Id,
    stream: bool,
    invite: bool,
}

pub fn grid(n: usize, w: f32, h: f32, gap: f32) -> (usize, f32) {
    let n = n.max(1);
    let mut best = (1, 0.0f32);
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let tw = ((w - gap * (cols - 1) as f32) / cols as f32)
            .min((h - gap * (rows - 1) as f32) / rows as f32 * 16. / 9.);
        if tw > best.1 {
            best = (cols, tw);
        }
    }
    best
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !active(app) || private_split(app) {
        return None;
    }
    let ch_id = app.voice_channel?;
    let ch = app.channels.get(&ch_id)?;
    if crate::ui::call_popout::is_open() {
        return Some(popped_out(cx).flex_1().min_w_0().h_full().into_any_element());
    }
    let zoom = crate::theme::zoom();

    let tiles = call_tiles(app, ch_id, &ch.voice_users);

    let vp = window.viewport_size();
    let side = if chat_open() { CHAT_W } else { 0. };
    let gw = (f32::from(vp.width) / zoom - 72. - crate::ui::sidebar_resize::width() - side).max(100.);
    let gh = (f32::from(vp.height) / zoom - 32. - if crate::theme::frame_borders() { 1. } else { 0. }).max(60.);
    let focus = match FOCUS.with(|c| c.get()) {
        Focus::Grid => None,
        Focus::Tile(u, s) => tiles.iter().position(|t| t.user == u && t.stream == s),
        Focus::Auto => tiles.iter().position(|t| t.stream),
    };
    let (fade, fading) = overlay_opacity();
    let ring_fading = ch.voice_users.iter().any(|&u| {
        let active = app.voice.is_speaking(u) || crate::voice::soundboard::is_user_playing(u);
        crate::ui::voice_panel::speaking_fade(u, active).1
    });
    if fading || ring_fading || crate::voice::call::camera_active() || crate::voice::call::remote_video_active() || crate::ui::voice_effects::has_effects() {
        window.request_animation_frame();
    }

    DRAWN.with(|d| d.borrow_mut().clear());
    let focused = focus.filter(|_| tiles.len() > 1);
    let tiles_el = if let Some(fi) = focused {
        focused_layer(app, &tiles, fi, gw, gh, fade, cx)
    } else {
        grid_layer(app, &tiles, gw, gh, fade, cx)
    };
    report_video_view(app, &ch.voice_users, focused.and_then(|i| tiles.get(i)), zoom * window.scale_factor());

    let toasts = crate::ui::call_toasts::render(app, ch_id, window, cx);
    let call = div()
        .id("call-view")
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .h_full()
        .flex()
        .flex_col()
        .bg(t(idx::BLACK))
        .when(crate::theme::frame_borders(), |d| d.border_t_1().border_color(t(idx::APP_FRAME_BORDER)))
        .on_mouse_move(cx.listener(|_, _, _, cx| woke(cx)))
        .child(tiles_el)
        .child(top_bar(app, crate::ui::dm_profile_panel::channel_name(app, ch), cx))
        .children(header_status(ch_id))
        .child(div().h(z(8.)))
        .child(div().flex_1().min_h_0())
        .child(bottom_controls(app, collapse(gw), cx))
        .relative()
        .children(toasts);

    let root = div()
        .flex_1()
        .min_w_0()
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_row()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .child(call)
        .children(stream_menu(app, cx))
        .children(crate::ui::call_menus::render(app, ch_id, cx));
    Some(if chat_open() {
        root.child(
            div()
                .w(z(CHAT_W))
                .flex_none()
                .min_w_0()
                .h_full()
                .flex()
                .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                .when(crate::theme::frame_borders(), |d| d.border_l_1().border_color(c(tok(idx::APP_FRAME_BORDER))))
                .overflow_hidden()
                .child(div().w(z(CHAT_W)).h_full().flex().flex_col().child(crate::ui::chat::render(app, window, cx, true))),
        )
        .into_any_element()
    } else {
        root.into_any_element()
    })
}

pub fn render_private(app: &PhoenixApp, header: gpui::Div, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !private_split(app) {
        return None;
    }
    let ch_id = app.voice_channel?;
    let ch = app.channels.get(&ch_id)?;
    let zoom = crate::theme::zoom();
    let vp = window.viewport_size();
    let column_h = f32::from(vp.height) / zoom - 32.;
    let w = (f32::from(vp.width) / zoom - 72. - crate::ui::sidebar_resize::width() - 16.).max(100.);
    let popped = crate::ui::call_popout::is_open();
    let tiles = call_tiles(app, ch_id, &ch.voice_users);
    let video = tiles.iter().any(|tile| tile.stream || crate::ui::voice_panel::flags(tile.user).video);
    let max = (column_h - SPLIT_CHAT_RESERVE).max(SPLIT_MIN_H);
    let h = if popped { SPLIT_POPPED_H } else { split_height(stored_split_height(), column_h, video) };
    let (fade, fading) = overlay_opacity();
    let ring_fading = ch.voice_users.iter().any(|&u| {
        let active = app.voice.is_speaking(u) || crate::voice::soundboard::is_user_playing(u);
        crate::ui::voice_panel::speaking_fade(u, active).1
    });
    if ring_fading || (video && (fading || crate::voice::call::camera_active() || crate::voice::call::remote_video_active())) {
        window.request_animation_frame();
    }
    let header = header.bg(gpui::transparent_black()).border_color(gpui::transparent_black());
    let body: AnyElement = if popped {
        popped_out(cx).flex_1().min_h_0().w_full().into_any_element()
    } else if video {
        let avail = (h - (m_header() + 8.) - (49. + 19. + 12.)).max(40.);
        let (_cols, tw) = grid(tiles.len(), w, avail, 8.);
        let th = tw * 9. / 16.;
        div()
            .flex_1()
            .min_h_0()
            .px(z(8.))
            .flex()
            .flex_wrap()
            .content_center()
            .justify_center()
            .gap(z(8.))
            .children(tiles.iter().map(|tile| render_tile(app, tile, tw, th, tw, fade, cx)))
            .into_any_element()
    } else {
        let users: Vec<Id> = ch.voice_users.clone();
        let size = voice_avatar_size(users.len(), w);
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .pb(z(24.))
            .flex()
            .items_center()
            .justify_center()
            .children(users.into_iter().map(|u| voice_avatar(app, u, size)))
            .into_any_element()
    };
    let container = div()
        .id("dm-call-container")
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .when(video, |d| d.bg(t(idx::BLACK)))
        .when(crate::theme::frame_borders(), |d| d.border_t_1().border_color(t(idx::APP_FRAME_BORDER)))
        .on_mouse_move(cx.listener(|_, _, _, cx| woke(cx)))
        .child(header)
        .child(body)
        .when(!popped, |d| d.child(bottom_controls(app, collapse(w + 16.), cx)));
    Some(
        div()
            .id("dm-call")
            .relative()
            .w_full()
            .flex_none()
            .h(z(h))
            .bg(c(tok(if video { idx::BACKGROUND_BASE_LOWEST } else { idx::BACKGROUND_SURFACE_HIGH })))
            .child(container)
            .children(stream_menu(app, cx))
            .children(crate::ui::call_menus::render(app, ch_id, cx))
            .when(!popped, |d| d.child(split_handle(max, cx)))
            .into_any_element(),
    )
}

fn m_header() -> f32 {
    crate::theme::m::HEADER_H + 1.
}

fn voice_status_icon(f: crate::model::VoiceFlags, local_muted: bool) -> Option<(&'static str, bool)> {
    if f.server_deaf {
        Some(("headphones-deny", true))
    } else if f.server_mute || local_muted {
        Some(("mic-deny", true))
    } else if f.self_deaf {
        Some(("headphones-off", false))
    } else if f.self_mute {
        Some(("mic-off", false))
    } else {
        None
    }
}

fn voice_avatar(app: &PhoenixApp, user: Id, size: f32) -> AnyElement {
    let me = user == app.me.id;
    let mut f = crate::ui::voice_panel::flags(user);
    if me {
        f.self_mute = app.mic_muted;
        f.self_deaf = app.deafened;
    }
    let local_muted = !me && crate::voice::prefs::prefs().lock().user(user).muted;
    let muted = f.self_mute || f.server_mute;
    let deaf = f.self_deaf || f.server_deaf;
    let can_ring = !muted && !deaf && !local_muted;
    let sb = crate::voice::soundboard::is_user_playing(user);
    let active = can_ring && (app.voice.is_speaking(user) || sb);
    let (ring_alpha, _) = crate::ui::voice_panel::speaking_fade(user, active);
    let ring_color = t(if sb { idx::BRAND_500 } else { idx::STATUS_SPEAKING });
    let status = voice_status_icon(f, local_muted);
    let (badge_box, badge_icon) = if size >= 72. { (24., 16.) } else { (16., 10.) };
    let cutout = c(tok(idx::BACKGROUND_SURFACE_HIGH));
    div()
        .id(SharedString::from(format!("dm-call-avatar-{}", user.0)))
        .relative()
        .mx(z(8.))
        .size(z(size))
        .flex_none()
        .rounded_full()
        .child(avatar(app.users.get(&user), size * crate::theme::zoom()))
        .when(ring_alpha > 0.001, |d| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .opacity(ring_alpha)
                    .border(z(2.))
                    .border_color(ring_color)
                    .child(div().size_full().rounded_full().border(z(1.)).border_color(t(idx::BACKGROUND_BASE_LOWER))),
            )
        })
        .when(f.stream, |d| {
            d.child(
                div()
                    .absolute()
                    .top(z(-2.))
                    .right(z(-6.))
                    .h(z(16.))
                    .px(z(6.))
                    .rounded_full()
                    .bg(t(idx::BACKGROUND_FEEDBACK_NOTIFICATION))
                    .flex()
                    .items_center()
                    .text_size(z(10.))
                    .line_height(z(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t(idx::WHITE))
                    .child(crate::t!("voice.live")),
            )
        })
        .when(f.video && !f.stream, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .size(z(badge_box))
                    .rounded_full()
                    .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                    .border(z(2.))
                    .border_color(cutout)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph("icons/voice/camera.svg", badge_icon - 2., t(idx::WHITE))),
            )
        })
        .when_some(status, |d, (icon_name, _)| {
            d.child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .size(z(badge_box))
                    .rounded_full()
                    .bg(t(idx::RED_400))
                    .border(z(2.))
                    .border_color(cutout)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(named(icon_name, badge_icon, t(idx::WHITE))),
            )
        })
        .into_any_element()
}

fn split_handle(max: f32, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    use gpui::{Bounds, DispatchPhase, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, canvas};
    let weak = cx.entity().downgrade();
    let probe = canvas(
        |b: Bounds<Pixels>, _, _| SPLIT_TOP.with(|t| t.set(f32::from(b.origin.y))),
        move |_, _, window, _| {
            if !SPLIT_DRAG.with(Cell::get) {
                return;
            }
            let w1 = weak.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !SPLIT_DRAG.with(Cell::get) {
                    return;
                }
                let h = (f32::from(ev.position.y) - SPLIT_TOP.with(Cell::get)) / crate::theme::zoom();
                set_split_height(Some(h.min(max).max(SPLIT_MIN_H)), false);
                let _ = w1.update(cx, |_, cx| cx.notify());
            });
            let w2 = weak.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !SPLIT_DRAG.with(Cell::get) {
                    return;
                }
                SPLIT_DRAG.with(|d| d.set(false));
                set_split_height(SPLIT_H.with(Cell::get), true);
                let _ = w2.update(cx, |_, cx| cx.notify());
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    div().absolute().top_0().left_0().size_full().child(probe).child(
        div()
            .id("dm-call-resize")
            .absolute()
            .left_0()
            .w_full()
            .bottom(z(-2.))
            .h(z(4.))
            .cursor_ns_resize()
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                SPLIT_DRAG.with(|d| d.set(true));
                cx.stop_propagation();
            }),
    )
}

fn popped_out(cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id("call-popped-out")
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(z(16.))
        .bg(t(idx::BLACK))
        .child(glyph(POPOUT, 40., t(idx::INTERACTIVE_TEXT_DEFAULT)))
        .child(
            div()
                .text_size(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::TEXT_STRONG))
                .child(crate::t!("call.popped_out_notice")),
        )
        .child(
            div()
                .id("call-bring-back")
                .px(z(16.))
                .py(z(8.))
                .rounded(z(8.))
                .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
                .text_size(z(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
                .cursor_pointer()
                .child(crate::t!("call.return_main"))
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.defer(crate::ui::call_popout::close);
                    cx.notify();
                })),
        )
}

pub fn name_key(name: &str, user: Id) -> String {
    let lower = name.to_lowercase();
    let pad = 32usize.saturating_sub(lower.encode_utf16().count());
    format!("{lower}{}{}", "!".repeat(pad), user.0)
}

pub fn sort_key(stream: bool, video: bool, streaming: bool, name: &str, user: Id) -> String {
    let n = name_key(name, user);
    if stream {
        format!("{}{n}\u{3}", if video { '\u{2}' } else { '\u{3}' })
    } else {
        let lead = if video {
            '\u{3}'
        } else if streaming {
            '\u{4}'
        } else {
            '\u{5}'
        };
        format!("{lead}{n}")
    }
}

fn call_tiles(app: &PhoenixApp, ch_id: Id, users: &[Id]) -> Vec<Tile> {
    let mut tiles = Vec::new();
    let hide_no_video = crate::ui::call_menus::no_video_hidden(ch_id);
    let mut keyed = Vec::new();
    for &u in users {
        let f = crate::ui::voice_panel::flags(u);
        let name = crate::ui::sidebar::voice_user_name(app, u);
        if f.stream {
            keyed.push((sort_key(true, f.video, true, &name, u), Tile { user: u, stream: true, invite: false }));
        }
        if !hide_no_video || f.video {
            keyed.push((sort_key(false, f.video, f.stream, &name, u), Tile { user: u, stream: false, invite: false }));
        }
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    tiles.extend(keyed.into_iter().map(|(_, t)| t));
    let solo = match tiles.as_slice() {
        [only] if !only.stream => Some(only.user),
        _ => None,
    };
    if let Some(user) = solo {
        tiles.push(Tile { user, stream: false, invite: true });
    }
    tiles
}

fn woke(cx: &mut Context<PhoenixApp>) {
    let was_idle = idle();
    if was_idle {
        WAKE.with(|c| c.set(Some(Instant::now())));
    }
    let stale = LAST_MOVE.with(|c| c.get()).is_none_or(|t| t.elapsed() >= Duration::from_millis(250));
    touch();
    if stale {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(IDLE_TIMEOUT + Duration::from_millis(20)).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    if was_idle {
        cx.notify();
    }
}

thread_local! {
    static STREAM_MENU: Cell<Option<(Id, gpui::Point<gpui::Pixels>)>> = const { Cell::new(None) };
    static STREAM_VOLUME: std::cell::RefCell<std::collections::HashMap<u64, u32>> = Default::default();
}

pub fn stream_volume(owner: Id) -> u32 {
    STREAM_VOLUME.with(|v| v.borrow().get(&owner.0).copied().unwrap_or(100))
}

fn stream_menu(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    use crate::ui::context_menu::{Dismiss, MenuItem};
    let (owner, pos) = STREAM_MENU.with(Cell::get)?;
    let key = stream_key(app, owner)?;
    let watching = app.voice.watching.contains(&key);
    let e = cx.entity();
    let dismiss: Dismiss = std::rc::Rc::new(move |_, cx| {
        STREAM_MENU.with(|m| m.set(None));
        e.update(cx, |_, cx| cx.notify());
    });
    let vol = stream_volume(owner);
    let mut items = vec![
        MenuItem::Custom(
            div()
                .px(z(8.))
                .py(z(6.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(div().text_size(z(14.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::INTERACTIVE_TEXT_DEFAULT)).child(crate::t!("voice.stream_volume")))
                .child(crate::ui::settings::voice::slider(
                    "stream-volume",
                    vol as f32 / 200.,
                    format!("{vol}%").into(),
                    None,
                    std::rc::Rc::new(move |app: &mut PhoenixApp, f: f32, _: bool, cx: &mut Context<PhoenixApp>| {
                        let v = (f * 200.).round() as u32;
                        STREAM_VOLUME.with(|m| m.borrow_mut().insert(owner.0, v));
                        drop(app.backend.voice_control(crate::voice::VoiceControl::SetStreamVolume(owner, v as f32 / 100.)));
                        cx.notify();
                    }),
                    cx,
                ))
                .into_any_element(),
        ),
        MenuItem::Separator,
    ];
    let e = cx.entity();
    let k = key.clone();
    items.push(MenuItem::action(if watching { crate::t!("voice.stop_watching") } else { crate::t!("voice.watch_stream") }, None, move |_, _, cx| {
        let k = k.clone();
        e.update(cx, |a, cx| {
            let c = if watching { crate::voice::VoiceControl::StopWatching(k) } else { crate::voice::VoiceControl::WatchStream(k) };
            drop(a.backend.voice_control(c));
            cx.notify();
        });
    }));
    items.push(MenuItem::action(crate::t!("call.fullscreen"), None, move |_, w, _| {
        set_focus(Focus::Tile(owner, true));
        w.toggle_fullscreen();
    }));
    Some(crate::ui::context_menu::render(pos, items, dismiss))
}

fn header_status(channel: Id) -> Option<gpui::Div> {
    let status = crate::voice::start_time::status(channel.0)?;
    Some(
        div().flex_none().pl(z(24. + 28.)).pr(z(24.)).flex().child(
            div()
                .min_w_0()
                .max_w(gpui::relative(0.75))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(t(idx::CHANNEL_ICON))
                .child(crate::ui::emoji_text::markup_label(status.into(), 12.)),
        ),
    )
}

fn grid_layer(app: &PhoenixApp, tiles: &[Tile], gw: f32, gh: f32, fade: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let n = tiles.len();
    let g = crate::ui::call_layout::grid(n, gw, gh);
    let (tw, th) = (g.tile_w, g.tile_h());
    let spare = g.top - crate::ui::call_layout::GRID_PAD_Y;
    div()
        .id("call-grid")
        .absolute()
        .top_0()
        .left_0()
        .w(z(gw))
        .h(z(gh))
        .overflow_y_scroll()
        .pt(z(g.top))
        .pb(z(crate::ui::call_layout::GRID_PAD_Y + spare - 8.))
        .px(z(8.))
        .flex()
        .flex_col()
        .children(tiles.chunks(g.columns.max(1)).enumerate().map(|(r, row)| {
            div().flex_none().flex().justify_center().children(row.iter().enumerate().map(|(c, tile)| {
                let i = r * g.columns + c;
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .when(g.pad_column, |d| d.px(z(2.)))
                    .when(!g.last_column(i, n), |d| d.mr(z(8.)))
                    .when(!g.last_row(i), |d| d.mb(z(8.)))
                    .child(render_tile(app, tile, tw, th, tw, fade, cx))
            }))
        }))
        .into_any_element()
}

fn focused_layer(app: &PhoenixApp, tiles: &[Tile], fi: usize, gw: f32, gh: f32, fade: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::ui::call_layout as cl;
    let open = strip_open();
    let rest: Vec<&Tile> = tiles.iter().enumerate().filter(|(i, _)| *i != fi).map(|(_, t)| t).collect();
    let ft = &tiles[fi];
    let aspect = if ft.stream {
        let frame = if ft.user == app.me.id { crate::voice::call::own_stream_frame() } else { crate::voice::call::stream_frame(ft.user.0) };
        frame.filter(|f| f.width > 0 && f.height > 0).map_or(16. / 9., |f| f.width as f32 / f.height as f32)
    } else {
        16. / 9.
    };
    let f = cl::focus(gw, gh, aspect, open, rest.len());
    let strip_shown = f.strip.is_some();
    div()
        .absolute()
        .top_0()
        .left_0()
        .w(z(gw))
        .h(z(gh))
        .child(div().absolute().left(z(f.x)).top(z(f.y)).child(render_tile(app, ft, f.w, f.h, gw, fade, cx)))
        .children(f.strip.map(|s| {
            IN_STRIP.with(|c| c.set(true));
            let strip = div()
                .absolute()
                .left(z(cl::STRIP_GAP))
                .right(z(cl::STRIP_GAP))
                .top(z(s.top))
                .h(z(cl::STRIP_TILE_H))
                .flex()
                .justify_center()
                .children(rest.iter().take(s.count).enumerate().map(|(i, tile)| {
                    div()
                        .flex_none()
                        .when(i > 0, |d| d.ml(z(cl::STRIP_GAP)))
                        .child(render_tile(app, tile, s.tile_w, cl::STRIP_TILE_H, s.rule_w, fade, cx))
                }));
            IN_STRIP.with(|c| c.set(false));
            strip
        }))
        .child(strip_button(open && strip_shown, f.action_top, fade, cx))
        .into_any_element()
}

fn strip_button(open: bool, top: f32, fade: f32, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let white = t(idx::WHITE);
    div().absolute().left_0().right_0().top(z(top)).flex().justify_center().opacity(fade).child(
        div()
            .id("call-participants")
            .w(z(60.))
            .h(z(32.))
            .p(z(4.))
            .rounded(z(48.))
            .flex()
            .items_center()
            .justify_center()
            .bg(t(idx::PRIMARY_630))
            .hover(|s| s.bg(t(idx::PRIMARY_700)))
            .shadow(crate::ui::inbox::popout_shadow())
            .cursor_pointer()
            .tip(if open {
                crate::t!("call.hide_members")
            } else {
                crate::t!("call.show_members")
            })
            .child(named(if open { "chevron" } else { "chevron-up" }, 24., white))
            .child(named("members", 24., white))
            .on_click(cx.listener(|_, _, _, cx| {
                set_strip_open(!strip_open());
                cx.notify();
            })),
    )
}

const CHAT_W: f32 = 480.;

fn top_bar(_app: &PhoenixApp, name: SharedString, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let open = chat_open();
    div()
        .h(z(33.))
        .flex_none()
        .px(z(24.))
        .flex()
        .items_center()
        .gap(z(10.))
        .child(named("speaker", 20., t(idx::CHANNEL_ICON)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::TEXT_STRONG))
                .text_ellipsis()
                .child(name),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(20.))
                .child(
                    div()
                        .id("call-chat")
                        .size(z(24.))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .tip(if open {
                            crate::t!("call.hide_chat")
                        } else {
                            crate::t!("call.show_chat")
                        })
                        .child(named(
                            "chat",
                            24.,
                            t(if open { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT }),
                        ))
                        .on_click(cx.listener(|_, _, _, cx| {
                            set_chat_open(!chat_open());
                            cx.notify();
                        })),
                ),
        )
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TileFill {
    Theme(u32, u32),
    Avatar(Hsla),
    Loading,
}

fn tile_fill(theme: Option<(u32, u32)>, dominant: Option<Hsla>) -> TileFill {
    match (theme, dominant) {
        (Some((a, b)), _) => TileFill::Theme(a, b),
        (None, Some(d)) => TileFill::Avatar(d),
        (None, None) => TileFill::Loading,
    }
}

type Theme = Option<(u32, u32)>;

thread_local! {
    static GUILD_THEMES: std::cell::RefCell<std::collections::HashMap<(Id, Id), Theme>> = Default::default();
    static GUILD_THEME_REQUESTS: std::cell::RefCell<std::collections::HashSet<(Id, Id)>> = Default::default();
}

fn guild_theme(app: &PhoenixApp, guild: Id, user: Id, cx: &mut Context<PhoenixApp>) -> Option<Theme> {
    if let Some(t) = GUILD_THEMES.with(|m| m.borrow().get(&(guild, user)).copied()) {
        return Some(t);
    }
    if GUILD_THEME_REQUESTS.with(|r| r.borrow_mut().insert((guild, user))) {
        let fut = app.backend.user_profile_without_note(user, Some(guild));
        cx.spawn(async move |this, cx| {
            let Ok(p) = fut.await else { return };
            GUILD_THEMES.with(|m| m.borrow_mut().insert((guild, user), p.theme));
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    None
}

fn tile_theme(global: Theme, guild: Option<Theme>) -> Theme {
    guild.unwrap_or(global)
}

fn tile_background(app: &PhoenixApp, user: Id, cx: &mut Context<PhoenixApp>) -> gpui::Background {
    crate::ui::dm_profile_panel::ensure_profile(app, user, cx);
    let guild = app.voice_channel.and_then(|ch| app.channels.get(&ch)).and_then(|c| c.guild);
    let guild_theme = guild.and_then(|g| guild_theme(app, g, user, cx));
    let theme = tile_theme(app.people.profiles.get(&user).and_then(|p| p.theme), guild_theme);
    let dominant = match app.users.get(&user).map(|u| &u.avatar) {
        Some(crate::model::ImageRef::Url(url)) => {
            crate::ui::avatar_color::ensure(url, cx);
            crate::ui::avatar_color::get(url).map(c)
        }
        Some(crate::model::ImageRef::Default(ix)) => Some(c(DEFAULT_AVATAR[*ix as usize % DEFAULT_AVATAR.len()])),
        None => None,
    };
    match tile_fill(theme, dominant) {
        TileFill::Theme(a, b) => {
            let (top, bottom) = crate::ui::profile_theme::video_tile_stops((a, b));
            gpui::linear_gradient(180., gpui::linear_color_stop(c(top), 0.), gpui::linear_color_stop(c(bottom), 1.))
        }
        TileFill::Avatar(d) => d.into(),
        TileFill::Loading => t(idx::PRIMARY_800).into(),
    }
}

thread_local! {
    static CAMERA_FRAME: std::cell::RefCell<Option<(u64, std::sync::Arc<gpui::RenderImage>)>> = const { std::cell::RefCell::new(None) };
}

pub fn camera_image() -> Option<std::sync::Arc<gpui::RenderImage>> {
    let Some(f) = crate::voice::call::camera_preview() else {
        retire(CAMERA_FRAME.with(|c| c.borrow_mut().take()));
        return None;
    };
    if let Some((pts, img)) = CAMERA_FRAME.with(|c| c.borrow().clone())
        && pts == f.pts_us
    {
        return Some(img);
    }
    let (w, h) = (f.width / 2, f.height / 2);
    let mut buf = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let row = (y * 2 * f.width * 4) as usize;
        for x in (0..w).rev() {
            let i = row + (x * 2 * 4) as usize;
            let p = f.data.get(i..i + 4)?;
            buf.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    let frame = image::Frame::new(image::RgbaImage::from_raw(w, h, buf)?);
    let img = std::sync::Arc::new(gpui::RenderImage::new(vec![frame]));
    retire(CAMERA_FRAME.with(|c| c.borrow_mut().replace((f.pts_us, img.clone()))));
    Some(img)
}

const SOLO_ART: &str = "https://discord.com/assets/664390de11a80444.svg";

fn solo_button(id: &'static str, icon: &str, label: SharedString) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
        .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
        .px(z(15.))
        .py(z(7.))
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .text_color(t(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .child(named(icon, 20., t(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
        .child(label)
        .into_any_element()
}

pub fn solo_veil_alpha(x: f32, y: f32) -> f32 {
    let dx = (x - 0.5) / 1.1079;
    let dy = y - 1.;
    let r = (dx * dx + dy * dy).sqrt();
    (1. - (r - 0.6) / 0.4).clamp(0., 1.)
}

thread_local! {
    static SOLO_VEILS: std::cell::RefCell<std::collections::HashMap<u32, std::sync::Arc<gpui::RenderImage>>> = Default::default();
}

fn solo_veil(rgb: u32) -> std::sync::Arc<gpui::RenderImage> {
    if let Some(v) = SOLO_VEILS.with(|m| m.borrow().get(&rgb).cloned()) {
        return v;
    }
    let (w, h) = (320u32, 180u32);
    let [r, g, b] = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, px) in out.enumerate_pixels_mut() {
        let a = solo_veil_alpha((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
        *px = image::Rgba([b, g, r, (a * 255.).round() as u8]);
    }
    let arc = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(out)]));
    SOLO_VEILS.with(|m| m.borrow_mut().insert(rgb, arc.clone()));
    arc
}

fn solo_tile(w: f32, h: f32) -> AnyElement {
    let lower = t(idx::BACKGROUND_BASE_LOWER);
    div()
        .relative()
        .w(z(w))
        .h(z(h))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(lower)
        .child(div().absolute().inset_0().bg(gpui::linear_gradient(
            270.,
            gpui::linear_color_stop(t(idx::BACKGROUND_TILE_GRADIENT_PINK_END), 0.),
            gpui::linear_color_stop(t(idx::BACKGROUND_TILE_GRADIENT_PINK_START), 0.8),
        )))
        .child(
            gpui::img(gpui::ImageSource::Render(solo_veil(crate::theme::resolve(tok(idx::BACKGROUND_BASE_LOWER)) >> 8)))
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(gpui::ObjectFit::Fill),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(16.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(gpui::img(SOLO_ART).w(z(400f32.min((w - 32.).max(0.)))).h(z(210f32.min((h - 32. - 40. - 24.).max(0.)))).mb(z(24.)))
                .child(
                    div()
                        .flex()
                        .gap(z(8.))
                        .child(solo_button("call-solo-invite", "invite", crate::t!("call.invite_voice")))
                        .child(solo_button("call-solo-activity", "apps", crate::t!("call.choose_activity"))),
                ),
        )
        .into_any_element()
}

type FrameCache = std::collections::HashMap<u64, (u64, std::sync::Arc<gpui::RenderImage>)>;

thread_local! {
    static REMOTE_FRAMES: std::cell::RefCell<FrameCache> = std::cell::RefCell::new(FrameCache::new());
    static RETIRED: std::cell::RefCell<(Vec<VideoImage>, Vec<VideoImage>)> = Default::default();
}

type VideoImage = std::sync::Arc<gpui::RenderImage>;

fn retire(old: Option<(u64, std::sync::Arc<gpui::RenderImage>)>) {
    if let Some((_, img)) = old {
        RETIRED.with(|r| r.borrow_mut().0.push(img));
    }
}

pub fn release_video_images(window: &mut Window, cx: &mut gpui::App) {
    let aged = RETIRED.with(|r| {
        let mut r = r.borrow_mut();
        let fresh = std::mem::take(&mut r.0);
        std::mem::replace(&mut r.1, fresh)
    });
    for img in aged {
        cx.drop_image(img, Some(window));
    }
}

pub fn remote_aspect(user: Id) -> Option<f32> {
    crate::voice::call::remote_frame(user.0).filter(|f| f.height > 0).map(|f| f.width as f32 / f.height as f32)
}

fn video_loading() -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .child(
            div()
                .w_full()
                .aspect_ratio(16. / 9.)
                .opacity(0.6)
                .bg(gpui::linear_gradient(
                    120.,
                    gpui::linear_color_stop(t(idx::PRIMARY_730), 0.),
                    gpui::linear_color_stop(t(idx::PRIMARY_660), 1.),
                )),
        )
        .into_any_element()
}

pub fn remote_image(user: Id) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let Some(f) = crate::voice::call::remote_frame(user.0) else {
        retire(REMOTE_FRAMES.with(|c| c.borrow_mut().remove(&user.0)));
        return None;
    };
    if let Some((pts, img)) = REMOTE_FRAMES.with(|c| c.borrow().get(&user.0).cloned())
        && pts == f.pts_us
    {
        return Some(img);
    }
    let frame = image::Frame::new(image::RgbaImage::from_raw(f.width, f.height, f.data.to_vec())?);
    let img = std::sync::Arc::new(gpui::RenderImage::new(vec![frame]));
    retire(REMOTE_FRAMES.with(|c| c.borrow_mut().insert(user.0, (f.pts_us, img.clone()))));
    Some(img)
}

thread_local! {
    static STREAM_FRAMES: std::cell::RefCell<FrameCache> = Default::default();
}

thread_local! {
    static DEMO_STILLS: std::cell::RefCell<std::collections::HashMap<u64, std::sync::Arc<gpui::RenderImage>>> = Default::default();
}

pub fn set_demo_still(owner: Id, img: Option<std::sync::Arc<gpui::RenderImage>>) {
    DEMO_STILLS.with(|m| match img {
        Some(i) => {
            m.borrow_mut().insert(owner.0, i);
        }
        None => {
            m.borrow_mut().remove(&owner.0);
        }
    });
}

pub fn demo_still(owner: Id) -> Option<std::sync::Arc<gpui::RenderImage>> {
    DEMO_STILLS.with(|m| m.borrow().get(&owner.0).cloned())
}

pub fn stream_image(owner: Id, own: bool) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let f = if own { crate::voice::call::own_stream_frame() } else { crate::voice::call::stream_frame(owner.0) };
    let Some(f) = f else {
        retire(STREAM_FRAMES.with(|c| c.borrow_mut().remove(&owner.0)));
        return None;
    };
    if let Some((pts, img)) = STREAM_FRAMES.with(|c| c.borrow().get(&owner.0).cloned())
        && pts == f.pts_us
    {
        return Some(img);
    }
    let rgba = if own {
        let step = if f.width > 1280 { 2 } else { 1 };
        let (w, h) = (f.width / step, f.height / step);
        let mut buf = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let row = (y * step * f.width * 4) as usize;
            for x in 0..w {
                let i = row + (x * step * 4) as usize;
                let p = f.data.get(i..i + 4)?;
                buf.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        image::RgbaImage::from_raw(w, h, buf)?
    } else {
        image::RgbaImage::from_raw(f.width, f.height, f.data.to_vec())?
    };
    let img = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(rgba)]));
    retire(STREAM_FRAMES.with(|c| c.borrow_mut().insert(owner.0, (f.pts_us, img.clone()))));
    Some(img)
}

pub fn stream_key_in(app: &PhoenixApp, owner: Id) -> Option<String> {
    let (id, ch) = app.channels.iter().find(|(_, c)| c.voice_users.contains(&owner))?;
    Some(crate::voice::stream::key(ch.guild.map(|g| g.0), id.0, owner.0))
}

pub fn stream_key(app: &PhoenixApp, owner: Id) -> Option<String> {
    let ch = app.voice_channel?;
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    Some(crate::voice::stream::key(guild.map(|g| g.0), ch.0, owner.0))
}

fn render_tile(app: &PhoenixApp, tile: &Tile, w: f32, h: f32, rule_w: f32, fade: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if tile.invite {
        return solo_tile(w, h);
    }
    let u = app.users.get(&tile.user);
    let me = tile.user == app.me.id;
    let in_strip = IN_STRIP.with(|c| c.get());
    DRAWN.with(|d| d.borrow_mut().push((tile.user, tile.stream, w, h)));
    let mut f = crate::ui::voice_panel::flags(tile.user);
    if me {
        f.self_mute = app.mic_muted;
        f.self_deaf = app.deafened;
    }
    let speaking = !tile.stream && app.voice.is_speaking(tile.user);
    let sb = !tile.stream && crate::voice::soundboard::is_user_playing(tile.user);
    let name = crate::ui::sidebar::voice_user_name(app, tile.user);
    let key = (tile.user, tile.stream);
    let base = div()
        .id(SharedString::from(format!("call-tile-{}-{}", tile.user.0, tile.stream)))
        .on_click(cx.listener(move |_, _, _, cx| {
            let cur = FOCUS.with(|c| c.get());
            let focused = matches!(cur, Focus::Tile(u, s) if (u, s) == key) || (cur == Focus::Auto && key.1);
            set_focus(if focused { Focus::Grid } else { Focus::Tile(key.0, key.1) });
            cx.notify();
        }))
        .relative()
        .w(z(w))
        .h(z(h))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .cursor_pointer();

    let look = crate::ui::call_layout::tile_look(rule_w);
    let pill = |icon: Option<AnyElement>, label: Option<SharedString>| {
        div()
            .flex()
            .flex_initial()
            .items_center()
            .gap(z(4.))
            .min_w_0()
            .min_h(z(20.))
            .px(z(8.))
            .py(z(6.))
            .rounded(z(8.))
            .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
            .text_color(t(idx::CONTROL_OVERLAY_SECONDARY_TEXT_DEFAULT))
            .text_size(z(look.name_size.0))
            .line_height(z(look.name_size.1))
            .children(icon)
            .children(label.map(|l| div().min_w_0().text_ellipsis().whitespace_nowrap().child(l)))
    };
    let white = t(idx::WHITE);

    let owner = tile.user;
    let base = if tile.stream && !me {
        base.on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |_, ev: &gpui::MouseDownEvent, _, cx| {
                STREAM_MENU.with(|m| m.set(Some((owner, ev.position))));
                cx.notify();
            }),
        )
    } else {
        base
    };
    let inner = if tile.stream {
        let skey = stream_key(app, tile.user);
        let watching = skey.as_ref().is_some_and(|k| app.voice.watching.contains(k));
        let video = (if me || watching { stream_image(tile.user, me) } else { None }).or_else(|| demo_still(tile.user));
        let preview = match (&video, &skey) {
            (None, Some(k)) => {
                crate::ui::voice_row::fetch_preview(app, k.clone(), cx);
                crate::ui::voice_row::preview_url(k)
            }
            _ => None,
        };
        let still: Option<gpui::ImageSource> = video.map(Into::into).or_else(|| preview.map(Into::into));
        let has_still = still.is_some();
        base.bg(t(idx::BACKGROUND_BASE_LOWER))
            .flex()
            .items_center()
            .justify_center()
            .children(still.map(|src| {
                let aspect = if me { crate::voice::call::own_stream_frame() } else { crate::voice::call::stream_frame(tile.user.0) }
                    .filter(|f| f.height > 0)
                    .map(|f| f.width as f32 / f.height as f32);
                gpui::img(src).absolute().inset_0().size_full().object_fit(video_fit(in_strip, aspect))
            }))
            .map(|d| {
                if !me && !watching {
                    d.child(
                    div()
                        .id(SharedString::from(format!("call-watch-{}", tile.user.0)))
                        .px(z(16.))
                        .py(z(8.))
                        .rounded(z(8.))
                        .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                        .hover(|s| s.bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t(idx::CONTROL_OVERLAY_SECONDARY_TEXT_DEFAULT))
                        .child(crate::t!("voice.watch_stream"))
                        .on_click(cx.listener({
                            let skey = skey.clone();
                            move |a, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(k) = skey.clone() {
                                    drop(a.backend.voice_control(crate::voice::VoiceControl::WatchStream(k)));
                                }
                                cx.notify();
                            }
                        })),
                    )
                } else if has_still {
                    d
                } else {
                    d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(z(10.))
                        .child(
                            div().w(gpui::relative(0.8)).h(gpui::relative(0.6)).child(
                                gpui::img(SharedString::from(stream_empty_preview(crate::theme::is_light())))
                                    .size_full()
                                    .object_fit(gpui::ObjectFit::Contain),
                            ),
                        )
                        .child(
                            div()
                                .text_center()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .text_color(t(idx::TEXT_DEFAULT))
                                .child(crate::t!("voice.stream_just_started")),
                        ),
                    )
                }
            })
            .child(
                div()
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .bg(t(idx::STATUS_DANGER))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white)
                    .child(crate::t!("voice.live")),
            )
            .when(look.show_name, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(z(look.inset))
                        .bottom(z(look.inset))
                        .right(z(look.inset))
                        .flex()
                        .opacity(fade)
                        .child(pill(Some(glyph("icons/voice/screen.svg", 16., white).into_any_element()), Some(name))),
                )
            })
    } else {
        let local_muted = !me && crate::voice::prefs::prefs().lock().user(tile.user).muted;
        let state_icon = voice_status_icon(f, local_muted).map(|(name, critical)| {
            let color = if critical { t(idx::ICON_FEEDBACK_CRITICAL) } else { white };
            named(name, 16., color).into_any_element()
        });
        let (ring_alpha, _) = crate::ui::voice_panel::speaking_fade(tile.user, speaking || sb);
        base.bg(tile_background(app, tile.user, cx))
            .flex()
            .items_center()
            .justify_center()
            .map(|d| match f.video.then(|| if me { camera_image().filter(|_| !crate::ui::call_menus::self_video_hidden()) } else { remote_image(tile.user) }).flatten() {
                Some(img) => {
                    let aspect = if me { None } else { remote_aspect(tile.user) };
                    d.bg(t(idx::BACKGROUND_BASE_LOWER)).child(gpui::img(img).absolute().inset_0().size_full().object_fit(video_fit(in_strip, aspect)))
                }
                None if f.video && !me && crate::voice::call::in_call() => d.bg(t(idx::BLACK)).child(video_loading()),
                None => d.child(avatar(u, look.avatar * crate::theme::zoom())),
            })
            .children(crate::ui::voice_effects::overlay(tile.user, w, h))
            .when(f.video, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(z(8.))
                        .right(z(8.))
                        .p(z(6.))
                        .rounded_full()
                        .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(glyph("icons/voice/camera.svg", 16., t(idx::CONTROL_OVERLAY_SECONDARY_TEXT_DEFAULT))),
                )
            })
            .when(look.show_name, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(z(look.inset))
                        .bottom(z(look.inset))
                        .right(z(look.inset))
                        .flex()
                        .opacity(fade)
                        .child(pill(state_icon, Some(name))),
                )
            })
            .when(ring_alpha > 0.001, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(z(8.))
                        .opacity(ring_alpha)
                        .border(z(2.))
                        .border_color(t(if sb { idx::BRAND_500 } else { idx::STATUS_SPEAKING }))
                        .child(div().size_full().rounded(z(6.)).border(z(1.)).border_color(t(idx::BLACK))),
                )
            })
    };
    inner.into_any_element()
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Dark,
    Red,
    Green,
}

type CaretClick = std::rc::Rc<dyn Fn(&mut PhoenixApp, gpui::Point<gpui::Pixels>, &mut Context<PhoenixApp>)>;

fn center_button(
    id: &'static str,
    icon: impl IntoElement,
    look: Look,
    caret: bool,
    tip: SharedString,
    on_click: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    center_button_ex(id, icon, look, caret, None, false, tip, on_click, cx)
}

#[allow(clippy::too_many_arguments)]
fn center_button_ex(
    id: &'static str,
    icon: impl IntoElement,
    look: Look,
    caret: bool,
    on_caret: Option<CaretClick>,
    active: bool,
    tip: SharedString,
    on_click: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let (bg, hover) = match look {
        Look::Dark => (gpui::transparent_black(), t(idx::BACKGROUND_MOD_MUTED)),
        Look::Red => (t(idx::OPACITY_RED_12), t(idx::OPACITY_RED_16)),
        Look::Green => (t(idx::OPACITY_GREEN_12), t(idx::OPACITY_GREEN_24)),
    };
    let bg = if active { hover } else { bg };
    let group: SharedString = format!("{id}-grp").into();
    let btn = crate::ui::states::track(div().id(id), call_key(id))
        .size(z(40.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .group_hover(group.clone(), |s| s.bg(hover))
        .when(caret, |d| d.rounded_l(z(8.)))
        .when(!caret, |d| d.rounded(z(8.)))
        .cursor_pointer()
        .tip(tip)
        .child(icon)
        .on_click(cx.listener(move |a, _, _, cx| on_click(a, cx)));
    let mut wrap = div().group(group.clone()).flex().items_center().child(btn);
    if caret {
        wrap = wrap.child(
            div()
                .w(z(20.))
                .h(z(40.))
                .ml(z(1.))
                .rounded_r(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .group_hover(group, |s| s.bg(hover))
                .cursor_pointer()
                .child(crate::ui::icon(crate::assets::Icon::Chevron, 16. * crate::theme::zoom(), t(idx::WHITE)))
                .id(SharedString::from(format!("{id}-caret")))
                .when_some(on_caret, |d, f| d.on_click(cx.listener(move |a, ev: &gpui::ClickEvent, _, cx| f(a, ev.position(), cx)))),
        );
    }
    wrap.into_any_element()
}

fn section() -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(z(8.))
        .p(z(4.))
        .rounded(z(12.))
        .border_1()
        .border_color(t(idx::BORDER_MUTED))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
}

fn invite(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(ch) = a.voice_channel else { return };
    match a.channels.get(&ch).and_then(|c| c.guild) {
        Some(g) => a.open_invite_people(g, cx),
        None => a.open_add_to_dm(ch, cx),
    }
}

fn tray_icon(id: &'static str, icon: gpui::Svg, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(24.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tip(tip)
        .child(icon)
}

fn bottom_controls(app: &PhoenixApp, level: Collapse, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let white = t(idx::WHITE);
    let tray = t(idx::INTERACTIVE_TEXT_DEFAULT);
    let muted = app.mic_muted || app.deafened;
    let me = crate::ui::voice_panel::flags(app.me.id);
    let (mic_i, mic_look) = if muted { (crate::assets::Icon::MicOff, Look::Red) } else { (crate::assets::Icon::Mic, Look::Dark) };
    let icon_color = |l: Look| match l {
        Look::Red => t(idx::RED_400),
        Look::Green => t(idx::GREEN_300),
        Look::Dark => white,
    };
    let live = crate::ui::call_menus::stream_live(app);
    let open_menu = crate::ui::call_menus::opened();
    let screen_look = if live { Look::Green } else { Look::Dark };
    let cam_look = if me.video { Look::Green } else { Look::Dark };
    div()
        .h(z(49.))
        .mt(z(19.))
        .mb(z(12.))
        .px(z(20.))
        .flex_none()
        .flex()
        .items_center()
        .gap(z(24.))
        .child(
            div()
                .flex_1()
                .flex()
                .gap(z(24.))
                .when(level < Collapse::NoInvite, |d| {
                    d.child(tray_icon("call-invite", named("invite", 24., tray), crate::t!("call.invite")).on_click(cx.listener(|a, _, _, cx| invite(a, cx))))
                }),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .justify_center()
                .gap(z(12.))
                .child(
                    section()
                        .child(center_button_ex(
                            "call-mic",
                            anim("call-mic", crate::icons::lottie_icons::Lottie::Mute, muted, icon_color(mic_look), crate::ui::icon(mic_i, 20. * crate::theme::zoom(), icon_color(mic_look))),
                            mic_look,
                            true,
                            Some(std::rc::Rc::new(|_: &mut PhoenixApp, pos: gpui::Point<gpui::Pixels>, cx: &mut Context<PhoenixApp>| {
                                crate::ui::voice_panel::toggle_call_audio_menu(pos);
                                cx.notify();
                            })),
                            crate::ui::voice_panel::audio_menu_open(crate::ui::voice_panel::Audio::Mic),
                            if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                            |a, cx| {
                                a.mic_muted = !a.mic_muted;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            },
                            cx,
                        ))
                        .child(center_button_ex(
                            "call-camera",
                            anim(
                                "call-camera",
                                crate::icons::lottie_icons::Lottie::Camera,
                                me.video,
                                icon_color(cam_look),
                                glyph(if me.video { "icons/voice/camera.svg" } else { CAMERA_OFF }, 20., icon_color(cam_look)),
                            ),
                            cam_look,
                            true,
                            Some(std::rc::Rc::new(|_: &mut PhoenixApp, pos: gpui::Point<gpui::Pixels>, cx: &mut Context<PhoenixApp>| {
                                crate::ui::voice_panel::toggle_camera_menu(pos);
                                cx.notify();
                            })),
                            crate::ui::voice_panel::camera_menu_open(),
                            if me.video { crate::t!("voice.camera_off") } else { crate::t!("voice.camera_on") },
                            |a, cx| {
                                let on = !crate::ui::voice_panel::flags(a.me.id).video;
                                drop(a.backend.voice_control(crate::voice::VoiceControl::SetCamera(on)));
                                cx.notify();
                            },
                            cx,
                        )),
                )
                .when(level < Collapse::Minimal, |d| d.child(
                    section()
                        .child(
                            div()
                                .relative()
                                .child(center_button_ex(
                                    "call-screen",
                                    anim("call-screen", crate::icons::lottie_icons::Lottie::Stream, live, icon_color(screen_look), glyph("icons/voice/screen.svg", 20., icon_color(screen_look))),
                                    screen_look,
                                    live,
                                    Some(std::rc::Rc::new(|_: &mut PhoenixApp, _: gpui::Point<gpui::Pixels>, cx: &mut Context<PhoenixApp>| {
                                        crate::ui::call_menus::toggle(crate::ui::call_menus::TrayMenu::Streams);
                                        cx.notify();
                                    })),
                                    open_menu == Some(crate::ui::call_menus::TrayMenu::Streams),
                                    crate::ui::go_live::label(app),
                                    crate::ui::go_live::toggle,
                                    cx,
                                ))
                                .child(crate::ui::call_menus::probe(crate::ui::call_menus::TrayMenu::Streams)),
                        )
                        .child(center_button(
                            "call-activity",
                            anim("call-activity", crate::icons::lottie_icons::Lottie::Activities, false, white, glyph("icons/voice/activity.svg", 20., white)),
                            Look::Dark,
                            false,
                            crate::t!("voice.start_activity"),
                            |_, _| crate::ui::hover_icon::clicked(call_key("call-activity")),
                            cx,
                        ))
                        .child(crate::ui::pickers::soundboard::call_button(app, cx))
                        .child(
                            div()
                                .relative()
                                .child(center_button_ex(
                                    "call-more",
                                    named("more-horizontal", 20., white),
                                    Look::Dark,
                                    false,
                                    None,
                                    open_menu == Some(crate::ui::call_menus::TrayMenu::More),
                                    crate::t!("call.more"),
                                    |_, cx| {
                                        crate::ui::call_menus::toggle(crate::ui::call_menus::TrayMenu::More);
                                        cx.notify();
                                    },
                                    cx,
                                ))
                                .child(crate::ui::call_menus::probe(crate::ui::call_menus::TrayMenu::More)),
                        ),
                ))
                .child(
                    crate::ui::states::track(div().id("call-disconnect"), call_key("call-disconnect"))
                        .w(z(65.))
                        .h(z(49.))
                        .rounded(z(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT))
                        .hover(|s| s.bg(t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER)))
                        .cursor_pointer()
                        .tip(crate::t!("voice.disconnect"))
                        .child(anim(
                            "call-disconnect",
                            crate::icons::lottie_icons::Lottie::Disconnect,
                            false,
                            t(idx::CONTROL_CRITICAL_PRIMARY_TEXT_DEFAULT),
                            crate::ui::icon(crate::assets::Icon::PhoneHangup, 20. * crate::theme::zoom(), t(idx::CONTROL_CRITICAL_PRIMARY_TEXT_DEFAULT)),
                        ))
                        .on_click(cx.listener(|a, _, _, cx| a.join_voice(None, cx))),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .justify_end()
                .gap(z(24.))
                .child(
                    tray_icon("call-popout", glyph(POPOUT, 24., tray), crate::t!("call.popout")).on_click(cx.listener(|_, _, _, cx| {
                        let app_entity = cx.entity();
                        cx.defer(move |cx| crate::ui::call_popout::open(app_entity, cx));
                        cx.notify();
                    })),
                )
                .child(
                    tray_icon("call-fullscreen", glyph(FULLSCREEN, 24., tray), crate::t!("call.fullscreen"))
                        .on_click(cx.listener(|_, _, w, _| w.toggle_fullscreen())),
                ),
        )
}

pub(crate) fn stream_empty_preview(light: bool) -> &'static str {
    if light { "https://discord.com/assets/bc689a4cf705a445.svg" } else { "https://discord.com/assets/6b1a461f35c05c7a.svg" }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tile_theme_uses_the_server_profile_once_loaded() {
        use super::tile_theme;
        assert_eq!(tile_theme(Some((1, 2)), None), Some((1, 2)));
        assert_eq!(tile_theme(Some((1, 2)), Some(Some((3, 4)))), Some((3, 4)));
        assert_eq!(tile_theme(None, Some(Some((3, 4)))), Some((3, 4)));
        assert_eq!(tile_theme(Some((1, 2)), Some(None)), None);
    }

    #[test]
    fn tile_background_prefers_the_profile_theme() {
        use super::{TileFill, tile_fill};
        let d = gpui::hsla(0.5, 0.5, 0.5, 1.);
        assert_eq!(tile_fill(Some((0x0B_3D_4F, 0x22_C7_D8)), Some(d)), TileFill::Theme(0x0B_3D_4F, 0x22_C7_D8));
        assert_eq!(tile_fill(Some((1, 2)), None), TileFill::Theme(1, 2));
        assert_eq!(tile_fill(None, Some(d)), TileFill::Avatar(d));
        assert_eq!(tile_fill(None, None), TileFill::Loading);
    }

    #[test]
    fn private_call_height_follows_the_client_wrapper() {
        use super::{split_height, voice_avatar_size};
        assert_eq!(split_height(None, 800., false), 240.);
        assert_eq!(split_height(None, 800., true), 400.);
        assert_eq!(split_height(Some(500.), 800., false), 500.);
        assert_eq!(split_height(Some(700.), 800., true), 600.);
        assert_eq!(split_height(Some(100.), 800., true), 230.);
        assert_eq!(split_height(Some(300.), 380., true), 230.);
        assert_eq!(voice_avatar_size(2, 600.), 80.);
        assert_eq!(voice_avatar_size(8, 600.), 40.);
    }

    #[test]
    fn solo_veil_is_the_client_ellipse() {
        let a = super::solo_veil_alpha;
        let near = |x: f32, y: f32| (x - y).abs() < 1e-3;
        assert!(near(a(0.5, 1.), 1.));
        assert!(near(a(0.5, 0.4), 1.));
        assert!(near(a(0.5, 0.), 0.));
        assert!(near(a(0.5, 0.2), 0.5));
        assert!(near(a(0., 0.), 0.));
        assert!((a(0., 0.3) - 0.417).abs() < 0.01);
    }

    use super::grid;

    #[test]
    fn replaced_video_frames_are_queued_for_atlas_release() {
        let img = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(image::RgbaImage::new(2, 2))]));
        super::retire(Some((1, img.clone())));
        super::retire(None);
        assert_eq!(super::RETIRED.with(|r| r.borrow().0.len()), 1);
        assert_eq!(std::sync::Arc::strong_count(&img), 2);
    }

    #[test]
    fn tiles_sort_like_the_rtc_store() {
        use super::{Id, name_key, sort_key};
        let a = Id(2);
        let b = Id(1);
        let mut keys = [
            sort_key(false, false, false, "Alice", a),
            sort_key(false, false, true, "Zed", b),
            sort_key(true, false, true, "Zed", b),
            sort_key(false, true, false, "bob", a),
            sort_key(true, true, true, "Carl", b),
        ];
        let expect = [keys[4].clone(), keys[3].clone(), keys[2].clone(), keys[1].clone(), keys[0].clone()];
        keys.sort();
        assert_eq!(keys, expect);
        assert!(sort_key(false, true, false, "Bob", a) < sort_key(true, false, true, "Bob", a));
        assert_eq!(name_key("Ab", Id(7)), format!("ab{}7", "!".repeat(30)));
    }

    #[test]
    fn grid_prefers_biggest_tiles() {
        assert_eq!(grid(6, 1600., 600., 8.).0, 3);
        let (cols, w) = grid(1, 800., 300., 8.);
        assert_eq!(cols, 1);
        assert!((w - 300. * 16. / 9.).abs() < 0.01);
    }

    #[test]
    fn voice_status_icon_follows_client_priority() {
        use crate::model::VoiceFlags;
        assert_eq!(super::voice_status_icon(VoiceFlags::default(), false), None);
        assert_eq!(super::voice_status_icon(VoiceFlags { self_mute: true, ..Default::default() }, false), Some(("mic-off", false)));
        assert_eq!(super::voice_status_icon(VoiceFlags { self_deaf: true, self_mute: true, ..Default::default() }, false), Some(("headphones-off", false)));
        assert_eq!(super::voice_status_icon(VoiceFlags { server_mute: true, self_deaf: true, ..Default::default() }, false), Some(("mic-deny", true)));
        assert_eq!(super::voice_status_icon(VoiceFlags { server_deaf: true, server_mute: true, ..Default::default() }, false), Some(("headphones-deny", true)));
        assert_eq!(super::voice_status_icon(VoiceFlags::default(), true), Some(("mic-deny", true)));
    }
}
