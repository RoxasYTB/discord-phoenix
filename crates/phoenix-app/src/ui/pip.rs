use std::cell::Cell;

use gpui::{
    AnyElement, Context, FontWeight, Hsla, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, SharedString,
    Styled, Window, div, prelude::*,
};

use crate::app::PhoenixApp;
use crate::assets::Icon;
use crate::model::Id;
use crate::theme::{DEFAULT_AVATAR, c, generated::idx, tok, z, zoom};
use crate::ui::tooltip::Tipped as _;

pub const PIP_W: f32 = 320.;
pub const PIP_H: f32 = 180.;
pub const MARGIN: f32 = 16.;
pub const TOP_INSET: f32 = 48.;
const DRAG_THRESHOLD: f32 = 6.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Default)]
struct DragState {
    active: bool,
    moved: bool,
    start_mouse: (f32, f32),
    start_pos: (f32, f32),
    cur_pos: (f32, f32),
}

thread_local! {
    static CORNER: Cell<Corner> = const { Cell::new(Corner::BottomRight) };
    static DRAG: Cell<DragState> = Cell::new(DragState::default());
}

fn t(i: usize) -> Hsla {
    c(tok(i))
}

pub fn corner() -> Corner {
    CORNER.with(Cell::get)
}

pub fn set_corner(corner: Corner) {
    CORNER.with(|c| c.set(corner));
}

pub fn corner_position(corner: Corner, vw: f32, vh: f32) -> (f32, f32) {
    let max_x = (vw - PIP_W - MARGIN).max(MARGIN);
    let max_y = (vh - PIP_H - MARGIN).max(TOP_INSET);
    match corner {
        Corner::TopLeft => (MARGIN, TOP_INSET),
        Corner::TopRight => (max_x, TOP_INSET),
        Corner::BottomLeft => (MARGIN, max_y),
        Corner::BottomRight => (max_x, max_y),
    }
}

pub fn snap_corner(center_x: f32, center_y: f32, vw: f32, vh: f32) -> Corner {
    let right = center_x >= vw * 0.5;
    let bottom = center_y >= vh * 0.5;
    match (right, bottom) {
        (false, false) => Corner::TopLeft,
        (true, false) => Corner::TopRight,
        (false, true) => Corner::BottomLeft,
        (true, true) => Corner::BottomRight,
    }
}

pub fn clamp_position(x: f32, y: f32, vw: f32, vh: f32) -> (f32, f32) {
    let max_x = (vw - PIP_W - MARGIN).max(MARGIN);
    let max_y = (vh - PIP_H - MARGIN).max(TOP_INSET);
    (x.clamp(MARGIN, max_x), y.clamp(TOP_INSET, max_y))
}

pub fn has_watched_stream(app: &PhoenixApp, ch_id: Id) -> bool {
    app.channels.get(&ch_id).is_some_and(|ch| {
        ch.voice_users.iter().any(|&u| {
            let f = crate::ui::voice_panel::flags(u);
            f.stream && crate::ui::call_view::stream_key(app, u).is_some_and(|k| app.voice.watching.contains(&k))
        })
    })
}

pub fn active(app: &PhoenixApp) -> bool {
    app.voice_channel.is_some_and(|ch| has_watched_stream(app, ch))
        && !crate::ui::call_view::active(app)
        && !crate::ui::call_popout::is_open()
}

pub fn select_subject(app: &PhoenixApp, ch_id: Id) -> Option<(Id, bool)> {
    let ch = app.channels.get(&ch_id)?;
    let users = &ch.voice_users;
    if users.is_empty() {
        return Some((app.me.id, false));
    }
    if let Some(&u) = users.iter().find(|&&u| crate::ui::voice_panel::flags(u).stream && crate::ui::call_view::stream_key(app, u).is_some_and(|k| app.voice.watching.contains(&k))) {
        return Some((u, true));
    }
    if let Some(&u) = users.iter().find(|&&u| crate::ui::call_view::camera_visible(app, u)) {
        return Some((u, false));
    }
    if let Some(&u) = users.iter().find(|&&u| app.voice.is_speaking(u)) {
        return Some((u, false));
    }
    if let Some(&u) = users.iter().find(|&&u| u != app.me.id) {
        return Some((u, false));
    }
    Some((users[0], false))
}

fn tile_color(app: &PhoenixApp, user: Id) -> Hsla {
    match app.users.get(&user).map(|u| &u.avatar) {
        Some(crate::model::ImageRef::Url(url)) => crate::ui::avatar_color::get(url).map(c).unwrap_or(t(idx::BACKGROUND_BASE_LOWER)),
        Some(crate::model::ImageRef::Default(ix)) => c(DEFAULT_AVATAR[*ix as usize % DEFAULT_AVATAR.len()]),
        None => t(idx::BACKGROUND_BASE_LOWER),
    }
}

fn pip_btn(id: &'static str, icon: Icon, danger: bool, active_state: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let bg = if danger {
        t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT)
    } else if active_state {
        t(idx::OPACITY_RED_16)
    } else {
        t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT)
    };
    let hover = if danger {
        t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER)
    } else if active_state {
        t(idx::OPACITY_RED_12)
    } else {
        t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)
    };
    let fg = if active_state && !danger { t(idx::RED_400) } else { t(idx::WHITE) };
    div()
        .id(id)
        .size(z(32.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .tip(tip)
        .child(crate::ui::icon(icon, 16. * zoom(), fg))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    crate::ui::call_view::release_video_images(window, cx);
    if !active(app) {
        return None;
    }
    let ch_id = app.voice_channel?;
    let ch = app.channels.get(&ch_id)?;
    let (user_id, is_stream) = select_subject(app, ch_id)?;
    let k = zoom();
    let vp = window.viewport_size();
    let vw = (f32::from(vp.width) / k).max(PIP_W + MARGIN * 2.);
    let vh = (f32::from(vp.height) / k).max(PIP_H + TOP_INSET + MARGIN);

    let drag = DRAG.with(Cell::get);
    let (left, top) = if drag.active {
        clamp_position(drag.cur_pos.0, drag.cur_pos.1, vw, vh)
    } else {
        corner_position(corner(), vw, vh)
    };

    let me = user_id == app.me.id;
    let speaking = !is_stream && app.voice.is_speaking(user_id);
    let user = app.users.get(&user_id);
    let name = crate::ui::sidebar::voice_user_name(app, user_id);
    let video = if is_stream {
        let skey = crate::ui::call_view::stream_key(app, user_id);
        let watching = skey.as_ref().is_some_and(|k| app.voice.watching.contains(k));
        if me || watching {
            crate::ui::call_view::stream_image(user_id, me)
        } else {
            None
        }
    } else if crate::ui::call_view::camera_visible(app, user_id) {
        if me {
            crate::ui::call_view::camera_image()
        } else {
            crate::ui::call_view::remote_image(user_id)
        }
    } else {
        None
    };

    if video.is_some() {
        window.request_animation_frame();
    }
    crate::voice::call::set_pip_view();
    if is_stream
        && !me
        && let Some(key) = crate::ui::call_view::stream_key(app, user_id)
    {
        let (pw, ph) = crate::ui::call_view::device_pixels(PIP_W * k, PIP_H * k, window.scale_factor());
        crate::voice::call::set_stream_view(&key, pw, ph);
    }

    let muted = app.mic_muted || app.deafened;
    let white = t(idx::WHITE);
    let channel_name = ch.name.clone();

    let card = div()
        .id("voice-pip-card")
        .group("pip-card")
        .absolute()
        .left(z(left))
        .top(z(top))
        .w(z(PIP_W))
        .h(z(PIP_H))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(if video.is_some() || is_stream {
            t(idx::BACKGROUND_BASE_LOWER)
        } else {
            tile_color(app, user_id)
        })
        .border_1()
        .border_color(if speaking { t(idx::STATUS_POSITIVE) } else { t(idx::BORDER_SUBTLE) })
        .shadow(crate::ui::inbox::popout_shadow())
        .cursor_grab()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, ev: &gpui::MouseDownEvent, _, cx| {
                let mx = f32::from(ev.position.x) / k;
                let my = f32::from(ev.position.y) / k;
                DRAG.with(|d| {
                    d.set(DragState {
                        active: true,
                        moved: false,
                        start_mouse: (mx, my),
                        start_pos: (left, top),
                        cur_pos: (left, top),
                    });
                });
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match video {
            Some(img) => d.child(gpui::img(img).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Contain)),
            None if is_stream => d.child(gpui::svg().path("icons/voice/screen.svg").size(z(36.)).text_color(white)),
            None => d.child(crate::ui::avatar(user, 64. * k)),
        })
        .when(is_stream, |d| {
            d.child(
                div()
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .px(z(6.))
                    .rounded(z(4.))
                    .bg(t(idx::STATUS_DANGER))
                    .text_size(z(11.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white)
                    .child(crate::t!("voice.live")),
            )
        })
        .child(
            div()
                .absolute()
                .left(z(8.))
                .bottom(z(8.))
                .px(z(8.))
                .py(z(4.))
                .rounded(z(6.))
                .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                .text_color(white)
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .group_hover("pip-card", |s| s.opacity(0.))
                .child(name),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(8.))
                .flex()
                .flex_col()
                .justify_between()
                .bg(gpui::hsla(0., 0., 0., 0.45))
                .opacity(0.)
                .group_hover("pip-card", |s| s.opacity(1.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(z(6.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(z(13.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(white)
                                .text_ellipsis()
                                .child(channel_name),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(z(4.))
                                .child(
                                    div()
                                        .id("pip-return")
                                        .size(z(28.))
                                        .rounded(z(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                                        .hover(|s| s.bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)))
                                        .cursor_pointer()
                                        .tip(crate::t!("pip.return_to_call"))
                                        .child(gpui::svg().path("icons/call/fullscreen.svg").size(z(16.)).text_color(white))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .on_click(cx.listener(|a, _, _, cx| a.jump_to_call(cx))),
                                )
                                .child(
                                    div()
                                        .id("pip-popout")
                                        .size(z(28.))
                                        .rounded(z(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                                        .hover(|s| s.bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)))
                                        .cursor_pointer()
                                        .tip(crate::t!("call.popout"))
                                        .child(gpui::svg().path("icons/call/popout.svg").size(z(16.)).text_color(white))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            let app_entity = cx.entity();
                                            cx.defer(move |cx| crate::ui::call_popout::open(app_entity, cx));
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(z(8.))
                        .child(
                            pip_btn(
                                "pip-mic",
                                if muted { Icon::MicOff } else { Icon::Mic },
                                false,
                                muted,
                                if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                            )
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|a, _, _, cx| {
                                a.mic_muted = !a.mic_muted;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            })),
                        )
                        .child(
                            pip_btn(
                                "pip-deafen",
                                if app.deafened { Icon::HeadphonesOff } else { Icon::Headphones },
                                false,
                                app.deafened,
                                if app.deafened { crate::t!("voice.undeafen") } else { crate::t!("voice.deafen") },
                            )
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|a, _, _, cx| {
                                a.deafened = !a.deafened;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            })),
                        )
                        .child(
                            pip_btn("pip-hangup", Icon::PhoneHangup, true, false, crate::t!("voice.disconnect"))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|a, _, _, cx| a.join_voice(None, cx))),
                        ),
                ),
        );

    if drag.active {
        Some(
            div()
                .id("voice-pip-drag-layer")
                .absolute()
                .inset_0()
                .on_mouse_move(cx.listener(move |_, ev: &MouseMoveEvent, _, cx| {
                    let mut st = DRAG.with(Cell::get);
                    if !st.active {
                        return;
                    }
                    let mx = f32::from(ev.position.x) / k;
                    let my = f32::from(ev.position.y) / k;
                    let dx = mx - st.start_mouse.0;
                    let dy = my - st.start_mouse.1;
                    if dx.abs() > DRAG_THRESHOLD || dy.abs() > DRAG_THRESHOLD {
                        st.moved = true;
                    }
                    st.cur_pos = clamp_position(st.start_pos.0 + dx, st.start_pos.1 + dy, vw, vh);
                    DRAG.with(|d| d.set(st));
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |a, _: &MouseUpEvent, _, cx| {
                        let st = DRAG.with(|d| d.replace(DragState::default()));
                        if st.moved {
                            let c = snap_corner(st.cur_pos.0 + PIP_W * 0.5, st.cur_pos.1 + PIP_H * 0.5, vw, vh);
                            set_corner(c);
                        } else {
                            a.jump_to_call(cx);
                        }
                        cx.notify();
                    }),
                )
                .child(card)
                .into_any_element(),
        )
    } else {
        Some(card.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{Corner, MARGIN, PIP_H, PIP_W, TOP_INSET, clamp_position, corner_position, snap_corner};

    #[test]
    fn snaps_to_all_four_corners() {
        assert_eq!(snap_corner(100., 100., 1280., 800.), Corner::TopLeft);
        assert_eq!(snap_corner(1100., 100., 1280., 800.), Corner::TopRight);
        assert_eq!(snap_corner(100., 700., 1280., 800.), Corner::BottomLeft);
        assert_eq!(snap_corner(1100., 700., 1280., 800.), Corner::BottomRight);
    }

    #[test]
    fn corner_positions_respect_margins_and_top_inset() {
        let (vw, vh) = (1280., 800.);
        assert_eq!(corner_position(Corner::TopLeft, vw, vh), (MARGIN, TOP_INSET));
        assert_eq!(corner_position(Corner::TopRight, vw, vh), (vw - PIP_W - MARGIN, TOP_INSET));
        assert_eq!(corner_position(Corner::BottomLeft, vw, vh), (MARGIN, vh - PIP_H - MARGIN));
        assert_eq!(corner_position(Corner::BottomRight, vw, vh), (vw - PIP_W - MARGIN, vh - PIP_H - MARGIN));
    }

    #[test]
    fn clamp_keeps_card_inside_viewport() {
        let (x, y) = clamp_position(-50., 0., 1280., 800.);
        assert_eq!((x, y), (MARGIN, TOP_INSET));
        let (x2, y2) = clamp_position(2000., 2000., 1280., 800.);
        assert_eq!((x2, y2), (1280. - PIP_W - MARGIN, 800. - PIP_H - MARGIN));
    }
}
