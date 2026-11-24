use crate::ui::tooltip::Tipped as _;
use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, svg};

use crate::app::PhoenixApp;
use crate::model::{ChannelKind, Id};
use crate::theme::{DEFAULT_AVATAR, c, generated::idx, tok, z};
use crate::ui::avatar;

fn t(i: usize) -> Hsla {
    c(tok(i))
}

thread_local! {
    static SHOWN: Cell<Option<Id>> = const { Cell::new(None) };
    static CHAT_OPEN: Cell<bool> = const { Cell::new(false) };
    static FOCUS: Cell<Focus> = const { Cell::new(Focus::Auto) };
    static LAST_MOVE: Cell<Option<Instant>> = const { Cell::new(None) };
    static WAKE: Cell<Option<Instant>> = const { Cell::new(None) };
    static STRIP_OPEN: Cell<bool> = const { Cell::new(true) };
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
        && app.channels.get(&ch).is_some_and(|c| matches!(c.kind, ChannelKind::Voice | ChannelKind::Stage))
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
        _ => return None,
    })
}

fn glyph(path: impl Into<SharedString>, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(path.into()).size(z(size)).flex_none().text_color(color)
}

fn named(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    glyph(crate::icons::path(name), size, color)
}

struct Tile {
    user: Id,
    stream: bool,
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
    if !active(app) {
        return None;
    }
    let ch_id = app.voice_channel?;
    let ch = app.channels.get(&ch_id)?;
    let zoom = crate::theme::zoom();

    let mut tiles = Vec::new();
    for &u in &ch.voice_users {
        let f = crate::ui::voice_panel::flags(u);
        if f.stream {
            tiles.push(Tile { user: u, stream: true });
        }
        tiles.push(Tile { user: u, stream: false });
    }

    let vp = window.viewport_size();
    let side = if chat_open() { CHAT_W } else { 0. };
    let w = f32::from(vp.width) / zoom - 72. - crate::ui::sidebar_resize::width() - side - 16.;
    let h = f32::from(vp.height) / zoom - 32. - 33. - 8. - 80. - 8.;
    let w = w.max(100.);
    let h = h.max(60.);
    let focus = match FOCUS.with(|c| c.get()) {
        Focus::Grid => None,
        Focus::Tile(u, s) => tiles.iter().position(|t| t.user == u && t.stream == s),
        Focus::Auto => tiles.iter().position(|t| t.stream),
    };
        let (fade, fading) = overlay_opacity();
    if fading {
        window.request_animation_frame();
    }

    let tiles_el = if let Some(fi) = focus.filter(|_| tiles.len() > 1) {
        let open = strip_open();
        let band = if open { h - STRIP_H - 8. - 72. } else { h - 72. };
        let fw = w.min(band.max(40.) * 16. / 9.);
        let rest: Vec<&Tile> = tiles.iter().enumerate().filter(|(i, _)| *i != fi).map(|(_, t)| t).collect();
        let n = rest.len().max(1) as f32;
        let sw = ((w - 8. * (n - 1.)) / n).min(STRIP_H * 16. / 9.);
        div()
            .flex_1()
            .min_h_0()
            .px(z(8.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .pt(z(72.))
                    .pb(z(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(render_tile(app, &tiles[fi], fw, fw * 9. / 16., fade, cx)),
            )
            .relative()
            .when(open, |d| {
                d.child(
                    div()
                        .h(z(STRIP_H))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .gap(z(8.))
                        .children(rest.into_iter().map(|tile| render_tile(app, tile, sw, STRIP_H, fade, cx))),
                )
            })
            .child(strip_button(open, fade, cx))
    } else {
        let (_cols, tw) = grid(tiles.len(), w, h, 8.);
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
            .children(tiles.iter().map(|tile| render_tile(app, tile, tw, th, fade, cx)))
    };

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
        .on_mouse_move(cx.listener(|_, _, _, cx| {
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
        }))
        .child(top_bar(app, ch.name.clone(), cx))
        .child(div().h(z(8.)))
        .child(tiles_el)
        .child(bottom_controls(app, collapse(w + 16.), cx));

    let root = div().flex_1().min_w_0().size_full().overflow_hidden().flex().flex_row().bg(t(idx::BACKGROUND_BASE_LOWEST)).child(call);
    Some(if chat_open() {
        root.child(
            div()
                .w(z(CHAT_W))
                .flex_none()
                .min_w_0()
                .h_full()
                .flex()
                .bg(t(idx::BACKGROUND_BASE_LOWEST))
                .when(crate::theme::frame_borders(), |d| d.border_l_1().border_color(t(idx::APP_FRAME_BORDER)))
                .overflow_hidden()
                .child(div().w(z(CHAT_W)).h_full().flex().flex_col().child(crate::ui::chat::render(app, window, cx))),
        )
        .into_any_element()
    } else {
        root.into_any_element()
    })
}

fn strip_button(open: bool, fade: f32, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let white = t(idx::WHITE);
    let row = div().absolute().left_0().right_0().flex().justify_center().opacity(fade);
    let row = if open { row.bottom(z(STRIP_H - 8. - 32.)) } else { row.bottom(z(8.)) };
    row.child(
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

const STRIP_H: f32 = 112.;

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

fn tile_color(app: &PhoenixApp, user: Id) -> Hsla {
    match app.users.get(&user).map(|u| &u.avatar) {
        Some(crate::model::ImageRef::Url(url)) => crate::ui::avatar_color::get(url).map(c).unwrap_or(t(idx::BACKGROUND_BASE_LOWER)),
        Some(crate::model::ImageRef::Default(ix)) => c(DEFAULT_AVATAR[*ix as usize % DEFAULT_AVATAR.len()]),
        None => t(idx::BACKGROUND_BASE_LOWER),
    }
}

fn render_tile(app: &PhoenixApp, tile: &Tile, w: f32, h: f32, fade: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let u = app.users.get(&tile.user);
    let me = tile.user == app.me.id;
    let mut f = crate::ui::voice_panel::flags(tile.user);
    if me {
        f.self_mute = app.mic_muted;
        f.self_deaf = app.deafened;
    }
    let speaking = !tile.stream && app.voice.is_speaking(tile.user);
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

    let pill = |icon: Option<AnyElement>, label: Option<SharedString>| {
        div()
            .flex()
            .items_center()
            .gap(z(4.))
            .min_w_0()
            .px(z(8.))
            .py(z(6.))
            .rounded(z(8.))
            .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
            .text_color(t(idx::CONTROL_OVERLAY_SECONDARY_TEXT_DEFAULT))
            .text_size(z(14.))
            .line_height(z(18.))
            .children(icon)
            .children(label.map(|l| div().min_w_0().text_ellipsis().child(l)))
    };
    let white = t(idx::WHITE);

    let inner = if tile.stream {
        base.bg(t(idx::BACKGROUND_BASE_LOWER))
            .flex()
            .items_center()
            .justify_center()
            .child(glyph("icons/voice/screen.svg", 40., white))
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
            .child(
                div()
                    .absolute()
                    .left(z(4.))
                    .bottom(z(4.))
                    .opacity(fade)
                    .child(pill(Some(glyph("icons/voice/screen.svg", 16., white).into_any_element()), Some(name))),
            )
    } else {
        let state_icon = if f.self_deaf || f.server_deaf {
            Some(crate::ui::icon(crate::assets::Icon::HeadphonesOff, 16. * crate::theme::zoom(), white).into_any_element())
        } else if f.self_mute || f.server_mute {
            Some(crate::ui::icon(crate::assets::Icon::MicOff, 16. * crate::theme::zoom(), white).into_any_element())
        } else {
            None
        };
        base.bg(tile_color(app, tile.user))
            .flex()
            .items_center()
            .justify_center()
            .child(avatar(u, 80. * crate::theme::zoom()))
            .child(
                div()
                    .absolute()
                    .left(z(4.))
                    .bottom(z(4.))
                    .right(z(4.))
                    .flex()
                    .opacity(fade)
                    .child(pill(state_icon, Some(name))),
            )
            .when(speaking, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(z(8.))
                        .border(z(2.))
                        .border_color(t(idx::STATUS_POSITIVE))
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

fn center_button(
    id: &'static str,
    icon: gpui::Svg,
    look: Look,
    caret: bool,
    tip: SharedString,
    on_click: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let (bg, hover) = match look {
        Look::Dark => (gpui::transparent_black(), t(idx::BACKGROUND_MOD_MUTED)),
        Look::Red => (t(idx::OPACITY_RED_12), t(idx::OPACITY_RED_16)),
        Look::Green => (t(idx::OPACITY_GREEN_12), t(idx::OPACITY_GREEN_24)),
    };
    let group: SharedString = format!("{id}-grp").into();
    let btn = div()
        .id(id)
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
                .child(crate::ui::icon(crate::assets::Icon::Chevron, 16. * crate::theme::zoom(), t(idx::WHITE))),
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

fn tray_icon(id: &'static str, icon: gpui::Svg, tip: SharedString) -> AnyElement {
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
        .into_any_element()
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
    let screen_look = if me.stream { Look::Green } else { Look::Dark };
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
                    d.child(tray_icon("call-invite", named("invite", 24., tray), crate::t!("call.invite")))
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
                        .child(center_button(
                            "call-mic",
                            crate::ui::icon(mic_i, 20. * crate::theme::zoom(), icon_color(mic_look)),
                            mic_look,
                            true,
                            if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                            |a, cx| {
                                a.mic_muted = !a.mic_muted;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            },
                            cx,
                        ))
                        .child(center_button(
                            "call-camera",
                            glyph(if me.video { "icons/voice/camera.svg" } else { CAMERA_OFF }, 20., icon_color(cam_look)),
                            cam_look,
                            true,
                            crate::t!("voice.camera_on"),
                            |_, _| {},
                            cx,
                        )),
                )
                .when(level < Collapse::Minimal, |d| d.child(
                    section()
                        .child(center_button(
                            "call-screen",
                            glyph("icons/voice/screen.svg", 20., icon_color(screen_look)),
                            screen_look,
                            true,
                            crate::t!("voice.share_screen"),
                            |_, _| {},
                            cx,
                        ))
                        .child(center_button(
                            "call-activity",
                            glyph("icons/voice/activity.svg", 20., white),
                            Look::Dark,
                            false,
                            crate::t!("voice.start_activity"),
                            |_, _| {},
                            cx,
                        ))
                        .child(center_button(
                            "call-soundboard",
                            glyph("icons/voice/soundboard.svg", 20., white),
                            Look::Dark,
                            false,
                            crate::t!("voice.soundboard"),
                            |_, _| {},
                            cx,
                        ))
                        .child(center_button(
                            "call-more",
                            named("more-horizontal", 20., white),
                            Look::Dark,
                            false,
                            crate::t!("call.more"),
                            |_, _| {},
                            cx,
                        )),
                ))
                .child(
                    div()
                        .id("call-disconnect")
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
                        .child(crate::ui::icon(
                            crate::assets::Icon::PhoneHangup,
                            20. * crate::theme::zoom(),
                            t(idx::CONTROL_CRITICAL_PRIMARY_TEXT_DEFAULT),
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
                .child(tray_icon("call-popout", glyph(POPOUT, 24., tray), crate::t!("call.popout")))
                .child(tray_icon("call-fullscreen", glyph(FULLSCREEN, 24., tray), crate::t!("call.fullscreen"))),
        )
}

#[cfg(test)]
mod tests {
    use super::grid;

    #[test]
    fn grid_prefers_biggest_tiles() {
        assert_eq!(grid(6, 1600., 600., 8.).0, 3);
        let (cols, w) = grid(1, 800., 300., 8.);
        assert_eq!(cols, 1);
        assert!((w - 300. * 16. / 9.).abs() < 0.01);
    }
}
