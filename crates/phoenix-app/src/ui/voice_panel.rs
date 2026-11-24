use crate::ui::tooltip::Tipped as _;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, Hsla, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, Styled, Window, div, prelude::*, px, svg,
};

use crate::app::PhoenixApp;
use crate::assets::Icon;
use crate::theme::{Palette, c, generated::idx, tok, z};

fn t(i: usize) -> Hsla {
    c(tok(i))
}
use crate::model::{Id, VoiceFlags};
use crate::ui::context_menu::{self, MenuItem};
use crate::ui::{avatar, icon};
use crate::voice::prefs::{prefs, update};
use crate::voice::{Quality, VoiceStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Connection,
    Privacy,
}

thread_local! {
    static USER_MENU: Cell<Option<(Id, Point<Pixels>)>> = const { Cell::new(None) };
    static POPOUT: Cell<Option<Tab>> = const { Cell::new(None) };
    static AUDIO_MENU: RefCell<Option<AudioMenu>> = const { RefCell::new(None) };
    static FLAGS: RefCell<HashMap<Id, VoiceFlags>> = RefCell::new(HashMap::new());
}

pub fn set_flags(user: Id, flags: VoiceFlags) {
    FLAGS.with(|f| f.borrow_mut().insert(user, flags));
}

pub fn flags(user: Id) -> VoiceFlags {
    FLAGS.with(|f| f.borrow().get(&user).copied().unwrap_or_default())
}

type AudioMenu = (Audio, Point<Pixels>, Vec<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audio {
    Mic,
    Deafen,
}

const CAMERA: &str = "icons/voice/camera.svg";
const SCREEN: &str = "icons/voice/screen.svg";
const SOUNDBOARD: &str = "icons/voice/soundboard.svg";
const ACTIVITY: &str = "icons/voice/activity.svg";
const SHIELD: &str = "icons/voice/shield.svg";
const NOISE: &str = "icons/voice/noise.svg";
const VIDEO: &str = "icons/voice/video-small.svg";

pub fn svg_asset(path: &str) -> Option<&'static str> {
    Some(match path {
        CAMERA => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M4 5h11a2 2 0 0 1 2 2v2.4l3.6-2.6A1 1 0 0 1 22 7.6v8.8a1 1 0 0 1-1.4.8L17 14.6V17a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/>"#,
            "</svg>"
        ),
        SCREEN => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path fill-rule="evenodd" d="M4 3h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-6v2h3a1 1 0 1 1 0 2H7a1 1 0 1 1 0-2h3v-2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm8 3-4 4h3v3h2v-3h3z"/>"#,
            "</svg>"
        ),
        SOUNDBOARD => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M12 3a9 9 0 0 1 9 9 1 1 0 1 1-2 0 7 7 0 0 0-14 0v2h2a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 9-9zm3 9h2a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2z"/>"#,
            "</svg>"
        ),
        ACTIVITY => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path fill-rule="evenodd" d="M7 6h10a5 5 0 0 1 5 5v2a5 5 0 0 1-8.5 3.5h-3A5 5 0 0 1 2 13v-2a5 5 0 0 1 5-5zm0 3v1.5H5.5v2H7V14h2v-1.5h1.5v-2H9V9zm9.5 0a1.2 1.2 0 1 0 0 2.4 1.2 1.2 0 0 0 0-2.4zm-2 2.6a1.2 1.2 0 1 0 0 2.4 1.2 1.2 0 0 0 0-2.4z"/>"#,
            "</svg>"
        ),
        NOISE => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<rect x="3" y="9" width="2" height="6" rx="1"/><rect x="7" y="6" width="2" height="12" rx="1"/>"#,
            r#"<rect x="11" y="3" width="2" height="18" rx="1"/><rect x="15" y="6" width="2" height="12" rx="1"/>"#,
            r#"<rect x="19" y="9" width="2" height="6" rx="1"/>"#,
            "</svg>"
        ),
        VIDEO => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M4 6h10a2 2 0 0 1 2 2v1.5l4.4-2.9A1 1 0 0 1 22 7.4v9.2a1 1 0 0 1-1.6.8L16 14.5V16a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z"/>"#,
            "</svg>"
        ),
        SHIELD => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M12 2 4 5v6c0 5 3.4 9.4 8 11 4.6-1.6 8-6 8-11V5z"/>"#,
            "</svg>"
        ),
        _ => return None,
    })
}

fn asset_icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(SharedString::new_static(path)).size(px(size)).flex_none().text_color(color)
}

pub fn panel_button(id: impl Into<gpui::ElementId>, glyph: AnyElement, red: bool) -> gpui::Stateful<gpui::Div> {
    let id: gpui::ElementId = id.into();
    let key = crate::ui::states::key(("panel-btn", id.to_string(), red));
    let (rest, hover) = if red {
        (t(idx::OPACITY_RED_12), t(idx::OPACITY_RED_24))
    } else {
        (gpui::transparent_black(), t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))
    };
    crate::ui::states::track(div().id(id), key)
        .relative()
        .child(crate::ui::states::fade_layer(key, rest, hover, 200, 8.))
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .when(crate::theme::high_contrast(), |d| {
            d.border_1().border_color(if red { t(idx::OPACITY_RED_52) } else { gpui::transparent_black() })
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group("panel-btn")
        .child(div().relative().child(glyph))
        .map(|d| crate::ui::focus_ring::ring_inset_edges(d, key, 8., -1., -1., 1., 1.))
}

pub fn panel_glyph(i: Icon, red: bool) -> AnyElement {
    let g = icon(i, 20. * crate::theme::zoom(), t(if red { idx::ICON_FEEDBACK_CRITICAL } else { idx::ICON_SUBTLE }));
    if red { g.into_any_element() } else { g.group_hover("panel-btn", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))).into_any_element() }
}

pub fn audio_button(
    kind: Audio,
    i: Icon,
    red: bool,
    tip: SharedString,
    on_click: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let group: SharedString = format!("audio-{kind:?}").into();
    let base = |d: gpui::Stateful<gpui::Div>| {
        d.h(z(32.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(red, |d| d.bg(t(idx::OPACITY_RED_12)).hover(|s| s.bg(t(idx::OPACITY_RED_24))))
            .when(!red, |d| {
                d.group_hover(group.clone(), |s| s.bg(t(idx::BACKGROUND_MOD_MUTED)))
                    .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
            })
    };
    let open = AUDIO_MENU.with(|m| m.borrow().as_ref().is_some_and(|(k, _, _)| *k == kind));
    div()
        .group(group.clone())
        .flex()
        .flex_none()
        .gap(z(1.))
        .child(
            base(div().id(SharedString::from(format!("audio-btn-{kind:?}"))))
                .w(z(32.))
                .rounded_l(z(8.))
                .child(panel_glyph(i, red))
                .tip(tip)
                .on_click(cx.listener(move |a, _, _, cx| on_click(a, cx))),
        )
        .child(
            base(div().id(SharedString::from(format!("audio-chev-{kind:?}"))))
                .w(z(16.))
                .rounded_r(z(8.))
                .when(open && !red, |d| d.bg(t(idx::BACKGROUND_MOD_MUTED)))
                .child(icon(
                    Icon::Chevron,
                    16. * crate::theme::zoom(),
                    t(if red { idx::ICON_FEEDBACK_CRITICAL } else { idx::ICON_SUBTLE }),
                ))
                .tip(match kind {
                    Audio::Mic => crate::t!("voice.input_options"),
                    Audio::Deafen => crate::t!("voice.output_options"),
                })
                .on_click(cx.listener(move |_, ev: &gpui::ClickEvent, _, cx| {
                    let now = AUDIO_MENU.with(|m| m.borrow().as_ref().map(|(k, _, _)| *k));
                    let next = (now != Some(kind)).then(|| {
                        let (inputs, outputs) = crate::voice::call::devices();
                        (kind, ev.position(), if kind == Audio::Mic { inputs } else { outputs })
                    });
                    AUDIO_MENU.with(|m| *m.borrow_mut() = next);
                    cx.notify();
                })),
        )
        .into_any_element()
}

fn audio_menu(kind: Audio, pos: Point<Pixels>, devices: Vec<String>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let dismiss: context_menu::Dismiss = Rc::new(move |_: &mut Window, cx: &mut App| {
        AUDIO_MENU.with(|m| *m.borrow_mut() = None);
        e.update(cx, |_, cx| cx.notify());
    });
    let current = {
        let p = prefs().lock();
        if kind == Audio::Mic { p.input_device.clone() } else { p.output_device.clone() }
    };
    let mut list = vec![MenuItem::radio(crate::t!("voice.default"), current.is_none(), move |_, _, _| {
        update(move |p| if kind == Audio::Mic { p.input_device = None } else { p.output_device = None }, true)
    })];
    for d in devices {
        let on = current.as_deref() == Some(d.as_str());
        let name = d.clone();
        list.push(MenuItem::radio(d, on, move |_, _, _| {
            let name = name.clone();
            update(move |p| if kind == Audio::Mic { p.input_device = Some(name) } else { p.output_device = Some(name) }, true)
        }));
    }
    let device_label: SharedString = current.clone().map(Into::into).unwrap_or_else(|| crate::t!("voice.default"));
    let e = cx.entity();
    let vol_now = {
        let p = prefs().lock();
        if kind == Audio::Mic { p.input_volume } else { p.output_volume }
    };
    let max = if kind == Audio::Mic { 100 } else { 200 };
    let frac = (vol_now as f32 / max as f32).clamp(0., 1.);
    let volume = div()
        .flex()
        .flex_col()
        .child(
            div()
                .p(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(t(idx::TEXT_STRONG))
                .child(if kind == Audio::Mic { crate::t!("voice.input_volume_menu") } else { crate::t!("voice.output_volume_menu") }),
        )
        .child(
            div().px(z(8.)).mt(z(-4.)).h(z(24.)).relative().child(
                div()
                    .absolute()
                    .top(z(10.))
                    .left_0()
                    .right_0()
                    .h(z(4.))
                    .rounded(z(4.))
                    .bg(t(idx::SLIDER_TRACK_BACKGROUND))
                    .child(div().h_full().w(gpui::relative(frac)).rounded(z(4.)).bg(t(idx::SLIDER_INDICATOR_BACKGROUND))),
            )
            .child(
                div().absolute().top(z(4.)).left(gpui::relative(frac)).ml(z(-8.)).child(
                    div()
                        .size(z(16.))
                        .rounded_full()
                        .bg(gpui::white())
                        .border_1()
                        .border_color(t(if crate::theme::high_contrast() { idx::BORDER_STRONG } else { idx::BORDER_NORMAL }))
                        .shadow_sm(),
                ),
            ),
        )
        .pb(z(4.));
    let items = vec![
        MenuItem::Submenu {
            label: if kind == Audio::Mic { crate::t!("voice.input_device_menu") } else { crate::t!("voice.output_device_menu") },
            items: list,
            icon: None,
            subtext: Some(device_label),
        },
        MenuItem::Separator,
        MenuItem::Custom(volume.into_any_element()),
        MenuItem::Separator,
        MenuItem::action(crate::t!("voice.voice_settings"), Some(crate::icons::path("gear")), move |_, _, cx| {
            AUDIO_MENU.with(|m| *m.borrow_mut() = None);
            e.update(cx, |a, cx| {
                crate::ui::settings::open(a, cx);
                if let Some(s) = a.settings.as_mut() {
                    s.page = crate::ui::settings::Page::VoiceVideo;
                }
            });
        }),
    ];
    context_menu::render(pos - gpui::point(px(0.), px(200.)), items, dismiss)
}

fn action_button(id: &'static str, path: &'static str, label: SharedString, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .h(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(Palette::BORDER_MUTED))
        .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(asset_icon(path, 20. * crate::theme::zoom(), t(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
        .tip(label.clone())
        .on_click(cx.listener(move |a, _, _, cx| a.stub(&label, cx)))
}

fn large_ping(color: Hsla, bg: Hsla) -> impl IntoElement {
    div()
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .bg(bg)
        .border_1()
        .border_color(c(Palette::BORDER_MUTED))
        .flex()
        .items_center()
        .justify_center()
        .child(signal_bars(color))
}

fn signal_bars(color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_end()
        .gap(z(2.))
        .h(z(14.))
        .flex_none()
        .children([6., 10., 14.].map(|h| div().w(z(3.)).h(z(h)).rounded(z(1.)).bg(color)))
}

pub fn speaking_ring(avatar: AnyElement, size: f32, speaking: bool) -> AnyElement {
    div()
        .size(px(size))
        .flex_none()
        .relative()
        .child(avatar)
        .when(speaking, |d| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .border_2()
                    .border_color(t(idx::GREEN_360)),
            )
        })
        .into_any_element()
}

pub fn voice_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let ch = app.voice_channel.and_then(|v| app.channels.get(&v));
    let guild = ch
        .and_then(|c| c.guild)
        .and_then(|g| app.guilds.iter().find(|x| x.id == g))
        .map(|g| g.name.clone())
        .unwrap_or_default();
    let name = ch.map(|c| c.name.to_string()).unwrap_or_default();
    let label: SharedString = if guild.is_empty() { name.into() } else { format!("{name} / {guild}").into() };
    let v = &app.voice;
    const OK: (usize, usize) = (idx::TEXT_FEEDBACK_POSITIVE, idx::BACKGROUND_FEEDBACK_MOD_POSITIVE);
    const WARN: (usize, usize) = (idx::TEXT_FEEDBACK_WARNING, idx::BACKGROUND_FEEDBACK_MOD_WARNING);
    const BAD: (usize, usize) = (idx::TEXT_FEEDBACK_CRITICAL, idx::BACKGROUND_FEEDBACK_MOD_CRITICAL);
    let (title, (fg, bg)) = match v.status {
        VoiceStatus::Connected | VoiceStatus::Idle => (
            crate::t!("voice.connected"),
            match v.quality() {
                Quality::Fair => WARN,
                Quality::Bad => BAD,
                _ => OK,
            },
        ),
        VoiceStatus::Connecting => (crate::t!("voice.rtc_connecting"), WARN),
        VoiceStatus::Reconnecting => (crate::t!("voice.reconnecting"), WARN),
        VoiceStatus::Failed => (crate::t!("voice.no_route"), BAD),
    };
    let color = t(fg);
    let ping: SharedString = match (v.ping_ms, &v.last_error) {
        (_, Some(e)) if v.status == VoiceStatus::Failed => e.clone().into(),
        (Some(p), _) => if v.e2ee { crate::t!("voice.ping_e2ee", p = p) } else { crate::t!("voice.ping", p = p) },
        (None, _) => crate::t!("voice.waiting_server"),
    };
    let noise = prefs().lock().noise_suppression;
    div()
        .flex_none()
        .border_b_1()
        .border_color(c(Palette::BORDER_MUTED))
        .p(z(12.))
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(
            div()
                .h(z(32.))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(
                    div()
                        .id("voice-status")
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .cursor_pointer()
                        .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(ping.clone()).build(window, cx))
                        .on_click(cx.listener(|_, _, _, cx| {
                            POPOUT.with(|p| p.set(if p.get().is_some() { None } else { Some(Tab::Connection) }));
                            cx.notify();
                        }))
                        .child(large_ping(color, t(bg)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .h(z(20.))
                                        .flex()
                                        .items_center()
                                        .gap(z(4.))
                                        .child(
                                            div()
                                                .text_size(z(16.))
                                                .line_height(z(20.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(color)
                                                .text_ellipsis()
                                                .child(title),
                                        )
                                        .when(v.e2ee, |d| {
                                            d.child(
                                                div()
                                                    .id("voice-e2ee")
                                                    .cursor_pointer()
                                                    .child(asset_icon(SHIELD, 12., t(idx::TEXT_FEEDBACK_POSITIVE)))
                                                    .tooltip(|window, cx| {
                                                        gpui_component::tooltip::Tooltip::new(crate::t!("voice.e2ee"))
                                                            .build(window, cx)
                                                    })
                                                    .on_click(cx.listener(|_, _, _, cx| {
                                                        cx.stop_propagation();
                                                        POPOUT.with(|p| p.set(Some(Tab::Privacy)));
                                                        cx.notify();
                                                    })),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(z(12.))
                                        .line_height(z(16.))
                                        .text_color(c(Palette::TEXT_MUTED))
                                        .text_ellipsis()
                                        .hover(|s| s.underline())
                                        .child(label),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(z(4.))
                        .child(
                            panel_button(
                                "voice-noise",
                                asset_icon(NOISE, 20. * crate::theme::zoom(), t(idx::ICON_SUBTLE)).into_any_element(),
                                false,
                            )
                            .when(noise, |d| d.bg(t(idx::BACKGROUND_MOD_MUTED)))
                            .tip(if noise {
                                crate::t!("voice.noise_off")
                            } else {
                                crate::t!("voice.noise_on")
                            })
                            .on_click(cx.listener(|_, _, _, cx| {
                                update(|p| p.noise_suppression = !p.noise_suppression, true);
                                cx.notify();
                            })),
                        )
                        .child(
                            panel_button("hangup", panel_glyph(Icon::PhoneHangup, false), false)
                                .tip(crate::t!("voice.disconnect"))
                                .on_click(cx.listener(|a, _, _, cx| a.join_voice(None, cx))),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(action_button("voice-camera", CAMERA, crate::t!("voice.camera_on"), cx))
                .child(action_button("voice-screen", SCREEN, crate::t!("voice.share_screen"), cx))
                .child(action_button("voice-activity", ACTIVITY, crate::t!("voice.start_activity"), cx))
                .child(action_button("voice-soundboard", SOUNDBOARD, crate::t!("voice.soundboard"), cx)),
        )
}

pub fn server_tag(tag: &(SharedString, Option<SharedString>), size: f32) -> AnyElement {
    div()
        .flex_none()
        .ml(z(4.))
        .h(z(16.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(t(idx::BACKGROUND_MOD_STRONG))
        .flex()
        .items_center()
        .gap(z(2.))
        .when_some(tag.1.clone(), |d, url| d.child(gpui::img(url).size(z(size)).flex_none()))
        .child(
            div()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_NORMAL))
                .child(tag.0.clone()),
        )
        .into_any_element()
}

pub fn voice_user_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, user: Id) -> AnyElement {
    let u = app.users.get(&user);
    let me = user == app.me.id;
    let in_call = app.voice_channel.is_some_and(|ch| app.channels.get(&ch).is_some_and(|c| c.voice_users.contains(&user)));
    let local_muted = !me && prefs().lock().user(user).muted;
    let speaking = app.voice.is_speaking(user) && !local_muted;
    let mut f = flags(user);
    if me && app.voice.status != crate::voice::VoiceStatus::Idle {
        f.self_mute = app.mic_muted;
        f.self_deaf = app.deafened;
    }
    let state = t(idx::CHANNELS_DEFAULT);
    let red = t(idx::ICON_FEEDBACK_CRITICAL);
    let tag = u.and_then(|u| u.tag.clone());
    let e = entity.clone();
    let e2 = entity.clone();
    let selected = app.people.popout.as_ref().is_some_and(|p| p.user == user);
    let guild = app.selected_guild;
    let name_color = if speaking || selected { t(idx::INTERACTIVE_TEXT_ACTIVE) } else { state };
    div()
        .id(("voice-user-row", user.0))
        .group("voice-user-row")
        .cursor_pointer()
        .child(
            gpui::canvas(move |b, _, _| crate::ui::voice_row::record(user, b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .ml(z(38.))
        .mr(z(8.))
        .h(z(32.))
        .px(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .relative()
        .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
        .when(!selected, |d| d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
        .on_click(move |ev, window, cx| {
            let at = crate::ui::voice_row::popout_anchor(user, ev.position());
            e2.update(cx, |a, cx| a.open_profile(user, guild, at, window, cx));
        })
        .on_hover({
            let e = entity.clone();
            move |h: &bool, _, cx| {
                if crate::ui::voice_row::set_stream_hover(user, *h) {
                    e.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .children(crate::ui::voice_row::stream_preview(app, user))
        .child(speaking_ring(avatar(u, 24.), 24., speaking))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .child(
                    div()
                        .min_w_0()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(name_color)
                        .group_hover("voice-user-row", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                        .text_ellipsis()
                        .child(crate::ui::emoji_text::label(crate::ui::sidebar::voice_user_name(app, user), 14.)),
                )
                .when_some(tag, |d, t| d.child(server_tag(&t, 12.))),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(z(4.))
                .when(f.stream, |d| {
                    d.child(
                        div()
                            .h(z(16.))
                            .px(z(6.))
                            .rounded_full()
                            .bg(t(idx::BACKGROUND_FEEDBACK_NOTIFICATION))
                            .flex()
                            .items_center()
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child(crate::t!("voice.live")),
                    )
                })
                .when(f.video, |d| d.child(asset_icon(VIDEO, 16., state)))
                .when(local_muted, |d| d.child(icon(Icon::MicOff, 16., red)))
                .when(f.server_mute && !local_muted, |d| d.child(icon(Icon::MicOff, 16., red)))
                .when((f.self_mute || f.self_deaf) && !f.server_mute && !local_muted, |d| {
                    d.child(icon(Icon::MicOff, 16., state))
                })
                .when(f.server_deaf, |d| d.child(icon(Icon::HeadphonesOff, 16., red)))
                .when(f.self_deaf && !f.server_deaf, |d| d.child(icon(Icon::HeadphonesOff, 16., state))),
        )
        .child(
            div()
                .hidden()
                .group_hover("voice-user-row", |s| s.flex())
                .child(crate::ui::voice_row::options_button(user, guild, in_call)),
        )
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            cx.stop_propagation();
            if in_call {
                USER_MENU.with(|m| m.set(Some((user, ev.position))));
                e.update(cx, |_, cx| cx.notify());
            } else {
                crate::ui::user_menu::open(user, guild, ev.position, window);
            }
        })
        .into_any_element()
}

fn check_row(
    id: &'static str,
    label: SharedString,
    on: bool,
    f: impl Fn(&mut PhoenixApp) + 'static,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    div()
        .id(id)
        .h(z(32.))
        .px(z(8.))
        .my(z(2.))
        .rounded(z(2.))
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .text_size(z(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(Palette::INTERACTIVE))
        .hover(|s| s.bg(c(Palette::BLURPLE)).text_color(gpui::white()))
        .child(label)
        .child(
            div()
                .size(z(18.))
                .rounded(z(4.))
                .border_1()
                .border_color(c(if on { Palette::BLURPLE } else { 0xB5BAC1 }))
                .when(on, |d| d.bg(c(Palette::BLURPLE)))
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| d.child(div().size(z(8.)).rounded(z(1.)).bg(gpui::white()))),
        )
        .on_click(cx.listener(move |a, _, _, cx| {
            f(a);
            cx.notify();
        }))
        .into_any_element()
}

pub fn user_menu_items(app: &PhoenixApp, user: Id, cx: &mut Context<PhoenixApp>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if user == app.me.id {
        items.push(MenuItem::Custom(check_row(
            "vm-self-mute",
            crate::t!("voice.mute"),
            app.mic_muted,
            |a| {
                a.mic_muted = !a.mic_muted;
                crate::voice::sync_self_state(a);
            },
            cx,
        )));
        items.push(MenuItem::Custom(check_row(
            "vm-self-deafen",
            crate::t!("voice.deafen"),
            app.deafened,
            |a| {
                a.deafened = !a.deafened;
                crate::voice::sync_self_state(a);
            },
            cx,
        )));
        let e = cx.entity();
        items.push(MenuItem::Separator);
        items.push(MenuItem::action(crate::t!("voice.voice_settings"), None, move |_, _, cx| {
            e.update(cx, |a, cx| {
                crate::ui::settings::open(a, cx);
                if let Some(s) = a.settings.as_mut() {
                    s.page = crate::ui::settings::Page::VoiceVideo;
                }
            });
        }));
        return items;
    }
    let ua = prefs().lock().user(user);
    let vol = ua.volume;
    items.push(MenuItem::Custom(
        div()
            .px(z(8.))
            .py(z(6.))
            .flex()
            .flex_col()
            .gap(z(8.))
            .child(div().text_size(z(14.)).font_weight(FontWeight::MEDIUM).text_color(c(Palette::INTERACTIVE)).child(crate::t!("voice.user_volume")))
            .child(crate::ui::settings::voice::slider(
                "user-volume",
                vol as f32 / 200.,
                format!("{vol}%").into(),
                None,
                Rc::new(move |_, f, done, _| {
                    let v = (f * 200.).round() as u32;
                    update(move |p| p.users.entry(user.0).or_default().volume = v, done);
                }),
                cx,
            ))
            .into_any_element(),
    ));
    items.push(MenuItem::Separator);
    items.push(MenuItem::Custom(check_row(
        "vm-user-mute",
        crate::t!("voice.mute"),
        ua.muted,
        move |_| {
            update(
                |p| {
                    let u = p.users.entry(user.0).or_default();
                    u.muted = !u.muted;
                },
                true,
            );
        },
        cx,
    )));
    items
}

fn user_menu(app: &PhoenixApp, user: Id, pos: Point<Pixels>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let dismiss: context_menu::Dismiss = Rc::new(move |_: &mut Window, cx: &mut App| {
        USER_MENU.with(|m| m.set(None));
        e.update(cx, |_, cx| cx.notify());
    });
    context_menu::render(pos, user_menu_items(app, user, cx), dismiss)
}

fn ping_graph(history: &[u32]) -> impl IntoElement {
    let n = crate::voice::PING_HISTORY;
    let max = history.iter().copied().max().unwrap_or(0).max(100) as f32;
    let off = n - history.len().min(n);
    div()
        .h(z(80.))
        .p(z(8.))
        .rounded(z(4.))
        .bg(c(Palette::BG_TERTIARY))
        .flex()
        .items_end()
        .gap(z(2.))
        .children((0..n).map(|i| {
            let p = i.checked_sub(off).and_then(|j| history.get(j)).copied();
            let col = match p {
                Some(p) if p < 150 => Palette::GREEN,
                Some(p) if p < 400 => Palette::YELLOW,
                Some(_) => Palette::RED,
                None => Palette::BG_MOD_HOVER,
            };
            let h = p.map(|p| (p as f32 / max * 64.).max(2.)).unwrap_or(2.);
            div().flex_1().h(px(h)).rounded(z(1.)).bg(c(col))
        }))
}

fn stat(label: SharedString, value: String) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .gap(z(8.))
        .text_size(z(14.))
        .line_height(z(20.))
        .child(div().flex_none().text_color(c(Palette::TEXT_MUTED)).child(label))
        .child(div().min_w_0().text_ellipsis().font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_NORMAL)).child(value))
}

fn dash<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| "—".into())
}

fn privacy_body(v: &crate::voice::VoiceUi) -> AnyElement {
    let groups: Vec<String> = v.privacy_code.as_deref().map(|c| c.split(' ').map(str::to_string).collect()).unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(asset_icon(SHIELD, 20., c(if v.e2ee { Palette::GREEN } else { Palette::TEXT_MUTED })))
                .child(
                    div()
                        .text_size(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .child(if v.e2ee { crate::t!("voice.e2ee") } else { crate::t!("voice.not_e2ee") }),
                ),
        )
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(Palette::TEXT_MUTED)).child(if v.e2ee {
            crate::t!("voice.e2ee_note")
        } else {
            crate::t!("voice.transport_only")
        }))
        .when(!groups.is_empty(), |d| {
            d.child(div().text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("voice.privacy_code")))
                .child(div().flex().flex_wrap().gap(z(8.)).children(groups.into_iter().map(|g| {
                    div()
                        .px(z(8.))
                        .py(z(4.))
                        .rounded(z(4.))
                        .bg(c(Palette::BG_TERTIARY))
                        .font_family(crate::theme::FONT_MONO())
                        .text_size(z(16.))
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(g)
                })))
                .child(
                    div()
                        .text_size(z(12.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("voice.code_changes")),
                )
        })
        .into_any_element()
}

fn connection_body(v: &crate::voice::VoiceUi) -> AnyElement {
    let history: Vec<u32> = v.ping_history.iter().copied().collect();
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(ping_graph(&history))
        .child(stat(crate::t!("voice.avg_ping"), dash(v.average_ping().map(|p| format!("{p} ms")))))
        .child(stat(crate::t!("voice.last_ping"), dash(v.ping_ms.map(|p| format!("{p} ms")))))
        .child(stat(crate::t!("voice.out_loss"), dash(v.stats.outbound_loss_pct.map(|l| format!("{l:.1}%")))))
        .child(stat(crate::t!("voice.in_loss"), format!("{:.1}%", v.inbound_loss_pct())))
        .child(stat(crate::t!("voice.packets"), format!("{} / {}", v.stats.packets_sent, v.stats.packets_received)))
        .child(div().h(z(1.)).my(z(4.)).bg(c(Palette::DIVIDER)))
        .child(stat(crate::t!("voice.server"), dash(v.endpoint.clone())))
        .child(stat(crate::t!("voice.encryption"), dash(v.mode.clone())))
        .into_any_element()
}

fn popout(app: &PhoenixApp, tab: Tab, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let tab_button = |id: &'static str, label: SharedString, t: Tab, cx: &mut Context<PhoenixApp>| {
        let sel = t == tab;
        div()
            .id(id)
            .pb(z(6.))
            .border_b_2()
            .border_color(if sel { c(Palette::BLURPLE) } else { gpui::transparent_black() })
            .cursor_pointer()
            .text_size(z(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(if sel { Palette::TEXT_HEADER } else { Palette::TEXT_MUTED }))
            .hover(|s| s.text_color(c(Palette::TEXT_NORMAL)))
            .child(label)
            .on_click(cx.listener(move |_, _, _, cx| {
                POPOUT.with(|p| p.set(Some(t)));
                cx.notify();
            }))
    };
    let body = match tab {
        Tab::Connection => connection_body(&app.voice),
        Tab::Privacy => privacy_body(&app.voice),
    };
    let close = cx.listener(|_, _, _, cx| {
        POPOUT.with(|p| p.set(None));
        cx.notify();
    });
    gpui::deferred(
        div()
            .id("voice-popout-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_click(close)
            .child(
                div()
                    .id("voice-popout")
                    .absolute()
                    .left(z(80.))
                    .bottom(z(150.))
                    .w(z(320.))
                    .p(z(16.))
                    .rounded(z(8.))
                    .bg(c(Palette::BG_FLOATING))
                    .border_1()
                    .border_color(c(Palette::DIVIDER))
                    .shadow_lg()
                    .occlude()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .flex()
                    .flex_col()
                    .gap(z(12.))
                    .child(
                        div()
                            .flex()
                            .gap(z(16.))
                            .border_b_1()
                            .border_color(c(Palette::DIVIDER))
                            .child(tab_button("vp-tab-conn", crate::t!("voice.connection"), Tab::Connection, cx))
                            .child(tab_button("vp-tab-privacy", crate::t!("voice.privacy"), Tab::Privacy, cx)),
                    )
                    .child(body),
            ),
    )
    .with_priority(3)
    .into_any_element()
}

pub fn overlays(app: &PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    if let Some((kind, pos, devices)) = AUDIO_MENU.with(|m| m.borrow().clone()) {
        out.push(audio_menu(kind, pos, devices, cx));
    }
    if app.voice_channel.is_none() {
        USER_MENU.with(|m| m.set(None));
        POPOUT.with(|p| p.set(None));
        return out;
    }
    if let Some(tab) = POPOUT.with(Cell::get) {
        out.push(popout(app, tab, cx));
    }
    if let Some((user, pos)) = USER_MENU.with(Cell::get) {
        out.push(user_menu(app, user, pos, cx));
    }
    out
}

pub fn open_user_menu(user: Id, pos: Point<Pixels>, cx: &mut App) {
    USER_MENU.with(|m| m.set(Some((user, pos))));
    cx.refresh_windows();
}
