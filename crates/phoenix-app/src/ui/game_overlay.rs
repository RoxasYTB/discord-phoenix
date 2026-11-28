use std::cell::Cell;

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Entity, FontWeight, MouseButton, ParentElement, Styled, Subscription, Window, WindowBounds, WindowKind,
    WindowOptions, div, point, prelude::*, px, size,
};

use crate::{
    app::{PhoenixApp, Screen},
    assets::Icon,
    theme::{c, generated::idx, tok},
    ui::icon,
};

pub const ENABLED: &str = "game-overlay-enabled";
pub const VOICE: &str = "game-overlay-voice";
pub const SPEAKERS: &str = "game-overlay-active-speakers";
pub const NOTIFICATIONS: &str = "game-overlay-notifications";
pub const RIGHT: &str = "game-overlay-right";

thread_local! {
    static HANDLE: Cell<Option<AnyWindowHandle>> = const { Cell::new(None) };
}

pub fn is_open() -> bool {
    HANDLE.with(|handle| handle.get().is_some())
}

pub fn close(cx: &mut App) {
    if let Some(handle) = HANDLE.with(Cell::take) {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

fn widget_enabled(app: &PhoenixApp) -> bool {
    cfg!(windows) && app.screen == Screen::Main && app.prefs.local.flag(ENABLED, false)
}

pub fn toggle(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if is_open() {
        close(cx);
    } else if widget_enabled(app) {
        let app = cx.entity();
        cx.defer(move |cx| open(app, cx));
    }
    cx.notify();
}

fn open(app: Entity<PhoenixApp>, cx: &mut App) {
    if is_open() || !widget_enabled(app.read(cx)) {
        return;
    }
    let panel_size = size(px(246.), px(194.));
    let right = app.read(cx).prefs.local.flag(RIGHT, false);
    let display = cx.primary_display();
    let bounds = display.as_ref().map_or_else(
        || Bounds::centered(None, panel_size, cx),
        |display| {
            let bounds = display.bounds();
            let left = if right {
                bounds.origin.x + bounds.size.width - panel_size.width - px(20.)
            } else {
                bounds.origin.x + px(20.)
            };
            Bounds {
                origin: point(left, bounds.origin.y + px(48.)),
                size: panel_size,
            }
        },
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: false,
        kind: WindowKind::PopUp,
        is_resizable: false,
        is_minimizable: false,
        display_id: display.map(|display| display.id()),
        ..Default::default()
    };
    let main = app.clone();
    match cx.open_window(options, move |window, cx| {
        window.set_window_title("Phoenix — Overlay");
        window.on_window_should_close(cx, |_, _| {
            HANDLE.with(Cell::take);
            true
        });
        cx.new(|cx| {
            let refresh = cx.observe(&app, |_, _, cx| cx.notify());
            Overlay { app, _refresh: refresh }
        })
    }) {
        Ok(handle) => HANDLE.with(|slot| slot.set(Some(handle.into()))),
        Err(_) => main.update(cx, |app, cx| app.parity_toast(crate::t!("game.overlay.failed"), cx)),
    }
    main.update(cx, |_, cx| cx.notify());
}

struct Overlay {
    app: Entity<PhoenixApp>,
    _refresh: Subscription,
}

impl gpui::Render for Overlay {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app.read(cx);
        let enabled = widget_enabled(app);
        let show_voice = app.prefs.local.flag(VOICE, true);
        let speaking_only = app.prefs.local.flag(SPEAKERS, false);
        let notify = app.prefs.local.flag(NOTIFICATIONS, true);
        let channel = app.voice_channel.and_then(|channel| app.channels.get(&channel));
        let mut people = channel.map(|channel| channel.voice_users.clone()).unwrap_or_default();
        if channel.is_some() && !people.contains(&app.me.id) {
            people.push(app.me.id);
        }
        people.sort_unstable_by_key(|id| (!app.voice.is_speaking(*id), id.0));
        people.dedup();
        let participants: Vec<_> = people
            .into_iter()
            .filter(|user| !speaking_only || app.voice.is_speaking(*user))
            .map(|id| {
                (
                    crate::ui::sidebar::voice_user_name(app, id),
                    app.voice.is_speaking(id),
                    crate::ui::voice_panel::flags(id),
                )
            })
            .collect();
        let message = app.clips.message.clone();
        let title = channel.map(|ch| ch.name.clone()).unwrap_or_else(|| crate::t!("game.overlay.no_voice"));
        if !enabled {
            cx.defer(|cx| close(cx));
        }
        div()
            .size_full()
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .text_color(c(tok(idx::TEXT_STRONG)))
            .text_size(px(13.))
            .child(
                div()
                    .id("overlay-header")
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if show_voice { title } else { crate::t!("game.overlay") }),
                    )
                    .child(
                        div()
                            .id("overlay-close")
                            .flex_none()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _, window, cx| {
                                HANDLE.with(Cell::take);
                                let main = this.app.clone();
                                cx.defer(move |cx| main.update(cx, |_, cx| cx.notify()));
                                window.remove_window();
                            }))
                            .child(icon(Icon::Close, 17., c(tok(idx::TEXT_MUTED)))),
                    ),
            )
            .when(show_voice, |d| {
                d.child(
                    div()
                        .id("overlay-members")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(participants.into_iter().map(|(name, speaking, flags)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .py(px(3.))
                                .child(
                                    div()
                                        .size(px(8.))
                                        .flex_none()
                                        .rounded_full()
                                        .bg(if speaking { gpui::rgb(0x39a96b) } else { gpui::rgb(0x666973) }),
                                )
                                .child(div().flex_1().min_w_0().whitespace_nowrap().text_ellipsis().overflow_hidden().child(name))
                                .when(flags.self_mute || flags.server_mute, |d| {
                                    d.child(icon(Icon::MicOff, 14., c(tok(idx::TEXT_MUTED))))
                                })
                        })),
                )
            })
            .when_some(message.filter(|_| notify), |d, message| {
                d.child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(c(tok(idx::TEXT_MUTED)))
                        .child(message),
                )
            })
    }
}
