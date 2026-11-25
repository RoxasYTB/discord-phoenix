use crate::ui::tooltip::Tipped as _;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement,
    SharedString, StyledText, Styled, InteractiveText, Window, div, img, list, prelude::*,
};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    media,
    model::{Attachment, AttachmentKind, ChannelKind, Embed, Id, Message},
    richtext::{ClickTarget, RBlock, RenderedMessage, RichText},
    theme::{FONT_MONO, Palette, c, fz, generated::idx, m, space, tok, z},
    ui::{avatar, icon, members, sidebar::channel_icon},
};

pub mod app_launcher;
pub mod attach;
pub mod plaintext;
pub mod composer;
pub mod icons;
pub mod invite_cards;
pub mod invite_embed;
pub mod mention;
pub mod forward;
pub mod message_ext;
pub mod mosaic;
pub mod panels;
pub mod pending;
pub mod rich_input;
pub mod scheduled;
pub mod scrollbar;
pub mod alt_text;
pub mod state;
pub mod thread_banner;
pub mod voice_message;
pub mod blocked;
pub mod slash;
pub mod spoiler;
pub mod system;
pub use state::ChatState;

pub(crate) fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let entity = cx.entity();
    let show_members = app.show_members && app.selected_guild.is_some();
    let show_members = show_members && app.chat.thread.is_none() && !crate::ui::thread_create::is_open(cx);
    let show_members = show_members && !crate::ui::call_view::active(app);
    let search_results = crate::ui::search::results_shown(cx);
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .relative()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_PRIMARY))
        .on_modifiers_changed(cx.listener(|a, ev: &gpui::ModifiersChangedEvent, _, cx| {
            if a.chat.shift_held != ev.modifiers.shift {
                a.chat.shift_held = ev.modifiers.shift;
                cx.notify();
            }
        }))
        .child(header(app, cx))
        .children(crate::ui::dm_call::render(app, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(
                    panels::track_drop(div().id("chat-column"))
                        .group("chat-drop")
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .on_drop(cx.listener(|a, paths: &gpui::ExternalPaths, _, cx| {
                            panels::drop_ended();
                            a.add_paths(paths.paths().to_vec(), cx)
                        }))
                        .children(thread_banner::render(app, cx))
                        .children(crate::ui::join_flow::lurker_banner(app, cx))
                        .children(crate::ui::join_flow::verification_banner(app, cx))
                        .children(discovery_notice(app, cx))
                        .children(crate::ui::join_flow::nsfw_gate(app, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .relative()
                                .pr(z(16.))
                                .group(scrollbar::AUTO_HOVER_GROUP)
                                .child(
                                    list(app.message_list.clone(), move |ix, _w, cx| row(&entity, ix, cx))
                                        .size_full(),
                                )
                                .children(scrollbar::auto_scrollbar(&app.message_list))
                                .children(panels::new_messages_bar(app, cx))
                                .children(panels::jump_bar(app, cx)),
                        )
                        .child(composer::render(app, window, cx))
                        .child(panels::drop_overlay(app))
                        .children(crate::ui::forum::render(app, cx)),
                )
                .when(search_results, |d| d.child(div().flex_none().w(gpui::px(crate::ui::search::WIDTH))))
                .when(show_members && !search_results, |d| d.child(members::render(app, cx)))
                .when(!search_results, |d| {
                    d.children(crate::ui::dm_profile_panel::render(app, cx)).children(crate::ui::group_dm::members_panel(app, cx))
                })
                .children(panels::thread_panel(app, cx))
                .children(crate::ui::thread_create::panel(app, cx)),
        )
        .children(panels::pins_popout(app, cx))
}

fn guild_discoverable(app: &PhoenixApp) -> bool {
    app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).is_some_and(|g| g.discoverable)
}

static NOTICE_CLOSED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

static LABEL_CX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn discovery_label_probe() -> impl IntoElement {
    gpui::canvas(
        |b, _, _| {
            let cx = f32::from(b.origin.x) + f32::from(b.size.width) / 2.;
            LABEL_CX.store(cx.to_bits(), std::sync::atomic::Ordering::Relaxed);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn discovery_arrow() -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        |b, _, window, _| {
            let lcx = f32::from_bits(LABEL_CX.load(std::sync::atomic::Ordering::Relaxed));
            if lcx <= 0. {
                return;
            }
            let k = crate::theme::zoom();
            let (x0, y0) = (lcx - 8. * k, f32::from(b.origin.y) - 9. * k);
            let p = |x: f32, y: f32| gpui::point(gpui::px(x0 + x * k), gpui::px(y0 + y * k));
            let shape = |mut pb: gpui::PathBuilder, close: bool| {
                pb.move_to(p(0., 10.));
                pb.line_to(p(6., 2.9));
                pb.curve_to(p(10., 2.9), p(8., 0.7));
                pb.line_to(p(16., 10.));
                if close {
                    pb.close();
                }
                pb.build().ok()
            };
            for col in [tk(idx::BACKGROUND_BASE_LOWER), tk(idx::BACKGROUND_FEEDBACK_MOD_INFO)] {
                if let Some(path) = shape(gpui::PathBuilder::fill(), true) {
                    window.paint_path(path, col);
                }
            }
            if let Some(path) = shape(gpui::PathBuilder::stroke(gpui::px(k)), false) {
                window.paint_path(path, tk(idx::INLINENOTICE_BORDER_INFO));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn discovery_notice(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    let gid = app.selected_guild?;
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c))?;
    let locked = app.nav.extras.get(&ch.id).is_some_and(|e| e.locked);
    let thread = app.nav.thread_parent(ch.id).is_some();
    if ch.guild.is_none() || locked || thread || !guild_discoverable(app) || NOTICE_CLOSED.lock().ok()?.contains(&gid.0) {
        return None;
    }
    Some(
        div()
            .flex_none()
            .mt(z(-1.))
            .pt(z(4.))
            .pb(z(4.))
            .pl(z(16.))
            .pr(z(8.))
            .flex()
            .items_center()
            .gap(z(8.))
            .bg(tk(idx::BACKGROUND_FEEDBACK_MOD_INFO))
            .border_t_1()
            .border_b_1()
            .border_color(tk(idx::INLINENOTICE_BORDER_INFO))
            .relative()
            .child(discovery_arrow())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_wrap()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(crate::t!("chat.discovery_notice"))
                    .child(div().underline().child(crate::t!("chat.discovery_more"))),
            )
            .child(
                div()
                    .id("discovery-notice-close")
                    .group("discovery-notice-close")
                    .flex_none()
                    .size(z(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        if let Ok(mut v) = NOTICE_CLOSED.lock() {
                            v.push(gid.0);
                        }
                        cx.notify();
                    }))
                    .child(
                        gpui::svg()
                            .path(crate::icons::path("close"))
                            .size(z(18.))
                            .text_color(tk(idx::ICON_MUTED))
                            .group_hover("discovery-notice-close", |s| s.text_color(tk(idx::ICON_DEFAULT))),
                    ),
            )
            .into_any_element(),
    )
}

const HDR_ICON: f32 = 20.;

pub(crate) fn msg_left() -> f32 {
    if crate::theme::font_scaled_down() { 72. * crate::theme::font_scale() } else { m::AVATAR + 2. * space::md() }
}

pub(crate) fn msg_avatar_size() -> f32 {
    msg_avatar()
}

fn msg_avatar() -> f32 {
    if crate::theme::font_scaled_down() { 40. * crate::theme::font_scale() } else { m::AVATAR }
}

fn hdr_button(id: &'static str, i: Icon, color: gpui::Hsla, size: f32) -> gpui::Stateful<gpui::Div> {
    let _ = size;
    hdr(id, i.path(), color)
}

fn pin_button(app: &PhoenixApp, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    let b = hdr_button("pin", Icon::Pin, color, HDR_ICON);
    let unread = app.selected_channel.is_some_and(|c| app.backend.has_unread_pins(c)) && app.chat.pins.is_none();
    b.when(unread, |b| {
        b.child(
            div()
                .absolute()
                .left(z(16.))
                .top(z(16.))
                .size(z(10.))
                .rounded_full()
                .bg(c(Palette::BG_PRIMARY))
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(z(8.)).rounded(z(8.)).bg(tk(idx::BACKGROUND_BRAND))),
        )
    })
}

fn hdr_named(id: &'static str, name: &str, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    hdr(id, crate::icons::path(name), color)
}

fn hdr(id: &'static str, path: SharedString, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    let selected = color == tk(idx::ICON_STRONG);
    let label = match id {
        "threads" => crate::t!("threads.title"),
        "bell" => crate::t!("menu.notify_settings"),
        "pin" => crate::t!("pins.title"),
        "members" if selected => crate::t!("header.members_hide"),
        "members" => crate::t!("header.members_show"),
        "dm-call" => crate::t!("header.voice_call"),
        "dm-video" => crate::t!("header.video_call"),
        "dm-add" => crate::t!("header.add_to_dm"),
        "gdm-add" => crate::t!("header.add_to_gdm"),
        "gdm-members" if selected => crate::t!("header.members_hide"),
        "gdm-members" => crate::t!("header.members_show"),
        "dm-profile" if selected => crate::t!("header.profile_hide"),
        "dm-profile" => crate::t!("header.profile_show"),
        _ => crate::t!("header.more"),
    };
    crate::ui::states::header_button(id, path, selected, label).debug_selector(move || format!("hdr-{id}"))
}

thread_local! {
    static HEADER_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

pub(crate) fn width_probe(cell: &'static std::thread::LocalKey<std::cell::Cell<f32>>) -> impl IntoElement {
    gpui::canvas(
        move |b, w, _| {
            let nw = f32::from(b.size.width);
            if (cell.with(|c| c.replace(nw)) - nw).abs() > 0.5 {
                w.on_next_frame(|w, _| w.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn header_search_w(pad_start: f32) -> f32 {
    let w = HEADER_W.with(std::cell::Cell::get);
    if w <= 0. { 244. } else { (w - pad_start - 8. - 320.).clamp(64., 244.) }
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c));
    let is_dm = ch.is_some_and(|c| c.guild.is_none());
    let name = ch.map(|c| c.name.clone()).unwrap_or_default();
    let topic = ch.and_then(|c| c.topic.clone());
    let is_forum = ch.is_some_and(|c| c.kind == ChannelKind::Forum);
    let topic = topic.filter(|_| !is_forum);
    let thread_parent = ch
        .and_then(|c| app.nav.thread_parent(c.id).or_else(|| app.backend.thread_info(c.id).and_then(|i| i.parent)))
        .and_then(|p| app.channels.get(&p));
    let thread_info = thread_parent.and(ch).and_then(|c| app.backend.thread_info(c.id));
    let parent_is_forum = thread_parent.is_some_and(|p| p.kind == ChannelKind::Forum);
    let kind = thread_parent.or(ch).map(|c| c.kind).unwrap_or(ChannelKind::Text);
    let locked = ch.is_some_and(|c| app.nav.extras.get(&c.id).is_some_and(|e| e.locked));
    let discoverable = !is_dm && !locked && thread_parent.is_none() && guild_discoverable(app);
    let ic = tk(idx::ICON_MUTED);
    let on = tk(idx::ICON_STRONG);
    let is_group_dm = ch.is_some_and(|c| crate::ui::dm_profile_panel::is_group_dm(app, c.id));
    let peer = ch.filter(|c| !crate::ui::dm_profile_panel::is_group_dm(app, c.id)).and_then(|c| c.voice_users.first()).and_then(|u| app.users.get(u));
    let scope: SharedString = if is_dm {
        peer.map(|u| u.name.clone()).unwrap_or_else(|| name.clone())
    } else {
        app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone()).unwrap_or_default()
    };
    let search_ph = if is_dm && crate::ui::search::cross_dm(app) {
        crate::t!("search.in_dms")
    } else {
        SharedString::from(format!("{} {}", crate::t!("search.placeholder"), scope))
    };
    let pad_start = if is_group_dm { space::md() / 2. } else { space::md() };
    div()
        .relative()
        .child(width_probe(&HEADER_W))
        .h(z(m::HEADER_H + 1.))
        .flex_none()
        .pl(z(pad_start))
        .pr(z(8.))
        .py(z(8.))
        .flex()
        .items_center()
        .bg(c(Palette::BG_PRIMARY))
        .border_t_1()
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .child(if let Some(gc) = ch.map(|c| c.id).filter(|_| is_group_dm) {
                    let half = space::md() / 2.;
                    div()
                        .id("gdm-name")
                        .group("gdm-name")
                        .flex_shrink_0()
                        .h(z(32.))
                        .px(z(half))
                        .gap(z(half))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
                        .child(div().mr(z(4.)).child(crate::ui::group_dm::group_avatar(app, gc, 24. * crate::theme::zoom(), c(Palette::BG_PRIMARY))))
                        .child(
                            div()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(crate::ui::emoji_text::label(name.clone(), 16.)),
                        )
                        .child(
                            div().hidden().group_hover("gdm-name", |s| s.flex()).child(
                                gpui::svg().path(crate::icons::path("pen")).size(z(16.)).flex_none().text_color(tk(idx::TEXT_DEFAULT)),
                            ),
                        )
                        .into_any_element()
                } else if is_dm {
                    div().mr(z(8.)).child(avatar(peer, 20. * crate::theme::zoom())).into_any_element()
                } else {
                    div()
                        .ml(z(-1.))
                        .mr(z(11.))
                        .w(z(20.))
                        .h(z(32.))
                        .flex()
                        .items_center()
                        .map(|d| match crate::ui::sidebar::lock_icon_name(kind).filter(|_| locked && thread_parent.is_none()) {
                            Some(n) => d.child(gpui::svg().path(crate::icons::path(n)).size(z(20.)).flex_none().text_color(tk(idx::CHANNEL_ICON))),
                            None => d.child(icon(channel_icon(kind), 20. * crate::theme::zoom(), tk(idx::CHANNEL_ICON))),
                        })
                        .into_any_element()
                })
                .when_some(thread_parent.map(|p| (p.id, p.name.clone())), |d, (pid, pname)| {
                    d.child(
                        div()
                            .id("thread-parent")
                            .flex_none()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .mr(z(8.))
                            .h(z(24.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(z(16.))
                            .line_height(z(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tk(idx::TEXT_DEFAULT))
                            .on_click(cx.listener(move |a, _, _, cx| a.select_channel(pid, cx)))
                            .child(pname),
                    )
                    .child(
                        div().flex_none().mr(z(8.)).w(z(10.)).h(z(24.)).flex().items_center().justify_center().child(
                            gpui::svg().path(crate::icons::path("chevron-right")).size(z(10.)).text_color(tk(idx::ICON_MUTED)),
                        ),
                    )
                })
                .when(!is_group_dm, |d| d.child(
                    crate::fonts::styled_name(
                        div(),
                        peer.filter(|_| is_dm),
                        false,
                    )
                        .flex_shrink(if thread_parent.is_some() { 1. } else { 0. })
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .mr(z(8.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(crate::ui::emoji_text::label(name.clone(), 16.)),
                ))
                .when(kind == ChannelKind::Announcement && thread_parent.is_none(), |d| d.child(crate::ui::forum::follow_button()))
                .when(discoverable, |d| {
                    d.child(div().flex_none().ml(z(4.)).mr(z(12.)).size(z(4.)).rounded_full().bg(tk(idx::BACKGROUND_MOD_STRONG)))
                        .child(
                            div()
                                .flex_none()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_MUTED))
                                .relative()
                                .child(crate::t!("chat.discovery_mode"))
                                .child(discovery_label_probe()),
                        )
                })
                .when_some(topic.filter(|_| !is_dm), |d, t| {
                    let topic_click = crate::ui::home::topic::on_click(app, cx);
                    d.child(if discoverable {
                        div().flex_none().ml(z(8.)).w(z(1.)).h(z(16.)).bg(tk(idx::BORDER_SUBTLE))
                    } else {
                        div().flex_none().mx(z(4.)).size(z(4.)).rounded_full().bg(tk(idx::BACKGROUND_MOD_STRONG))
                    })
                        .child(
                            div()
                                .id("channel-topic")
                                .ml(z(8.))
                                .flex_1()
                                .min_w_0()
                                .cursor_pointer()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_MUTED))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .on_click(topic_click)
                                .child(crate::ui::emoji_text::label_with_custom(reply_preview_text(app, &t), 14.)),
                        )
                }),
        )
        .child(
            div()
                .flex_none()
                .pl(z(space::xs()))
                .flex()
                .items_center()
                .gap(z(space::xs()))
                .when(is_dm, |d| {
                    d.child(hdr_named("dm-call", "phone-call", ic))
                        .child(hdr_named("dm-video", "video-call", ic))
                        .child(
                            pin_button(app, if app.chat.pins.is_some() { on } else { ic })
                                .on_click(cx.listener(|a, _, _, cx| a.toggle_pins(cx))),
                        )
                        .child(hdr_named(if is_group_dm { "gdm-add" } else { "dm-add" }, "add-to-dm", ic))
                        .child(hdr_named(if is_group_dm { "gdm-members" } else { "dm-profile" }, "user-profile", if app.show_members { on } else { ic }).on_click(cx.listener(|a, _, _, cx| {
                            a.show_members = !a.show_members;
                            cx.notify();
                        })))
                })
                .when(!is_dm, |d| {
                    d.when(!parent_is_forum && !is_forum, |d| d.child(hdr_button("threads", Icon::Threads, ic, HDR_ICON).on_click(cx.listener(|a, _, _, cx| a.toggle_threads_popout(cx)))))
                        .map(|d| {
                            let slashed = ch.is_some_and(|c| {
                                thread_info.as_ref().is_some_and(|i| !i.joined || i.member_muted || i.notify_level() != 2)
                                    || c.muted
                                    || app.backend.notify_prefs().is_some_and(|p| {
                                        p.header_bell_slashed(c.guild.map(|g| g.0), c.id.0, c.parent.map(|p| p.0))
                                    })
                            });
                            let bell_col = if crate::ui::nav_menus::is_bell_open() { on } else { ic };
                            let bell_ch = ch.map(|c| c.id);
                            let btn = if slashed {
                                hdr_named("bell", "bell-slash", bell_col)
                            } else {
                                hdr_button("bell", Icon::Bell, bell_col, HDR_ICON)
                            };
                            d.child(btn.on_click(move |ev, w, _| {
                                if let Some(id) = bell_ch {
                                    crate::ui::nav_menus::toggle_bell(id, ev.position(), w);
                                }
                            }))
                        })
                        .when(!is_forum, |d| {
                            d.child(
                                pin_button(app, if app.chat.pins.is_some() { on } else { ic })
                                    .on_click(cx.listener(|a, _, _, cx| a.toggle_pins(cx))),
                            )
                        })
                        .when(!thread_info.as_ref().is_some_and(|i| i.archived), |d| {
                            d.child(hdr_button("members", Icon::Members, if app.show_members { on } else { ic }, HDR_ICON).on_click(
                                cx.listener(|a, _, _, cx| {
                                    a.show_members = !a.show_members;
                                    cx.notify();
                                }),
                            ))
                        })
                        .when_some(thread_parent.and(ch.map(|c| c.id)), |d, t| {
                            d.child(hdr_named("thread-more", "more-horizontal", ic).on_click(move |ev, w, _| {
                                crate::ui::thread_menu::open(t, ev.position(), w)
                            }))
                        })
                })
                .child(
                    div()
                        .ml(z(4.))
                        .flex_none()
                        .w(z(header_search_w(pad_start)))
                        .h(z(if crate::theme::density() == crate::theme::Density::Compact { 28. } else { 32. }))
                        .rounded(z(8.))
                        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                        .border_1()
                        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .pl(z(8.))
                                .pr(z(6.))
                                .pb(z(1.))
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(search_ph),
                        )
                        .child(div().size(z(24.)).p(z(4.)).flex().items_center().child(icon(Icon::Search, 16. * crate::theme::zoom(), tk(idx::ICON_MUTED)))),
                ),
        )
}

fn row(entity: &Entity<PhoenixApp>, ix: usize, cx: &mut App) -> AnyElement {
    let app = entity.read(cx);
    let Some(ch_id) = app.selected_channel else { return div().into_any_element() };
    let Some(entry) = app.messages.get(&ch_id) else { return div().into_any_element() };
    if ix == 0 {
        return row_wrapper().child(intro(app, entity, ch_id, entry.reached_start, entry.loading)).into_any_element();
    }
    let i = ix - 1;
    let (Some(msg), Some(r)) = (entry.msgs.get(i), entry.rendered.get(i)) else {
        return div().into_any_element();
    };
    let el = match blocked::row(app, entity, entry, i) {
        blocked::Row::Normal => message(app, entity, msg, r, entry, cx),
        blocked::Row::Hidden => div().into_any_element(),
        blocked::Row::Collapsed(bar) => bar,
        blocked::Row::Expanded(bar) => {
            let m = message(app, entity, msg, r, entry, cx);
            row_wrapper().bg(tk(idx::MESSAGE_BACKGROUND_HOVER)).children(bar).child(m).into_any_element()
        }
    };
    if i + 1 == entry.msgs.len() {
        return row_wrapper().pb(z(20.)).child(el).into_any_element();
    }
    el
}

fn intro(app: &PhoenixApp, entity: &Entity<PhoenixApp>, ch: Id, reached_start: bool, loading: bool) -> gpui::Div {
    let channel = app.channels.get(&ch);
    let name = channel.map(|c| c.name.clone()).unwrap_or_default();
    if !reached_start {
        return div()
            .h(z(64.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(14.))
            .text_color(c(Palette::TEXT_MUTED))
            .child(if loading { crate::t!("chat.loading_messages") } else { "".into() });
    }
    if let Some(chan) = channel.filter(|c| c.guild.is_none()) {
        let group = crate::ui::dm_profile_panel::is_group_dm(app, chan.id);
        let peer = chan.voice_users.first().filter(|_| !group).and_then(|u| app.users.get(u));
        let strong = name.clone();
        let desc = if group { crate::t!("chat.group_start", name = strong) } else { crate::t!("chat.dm_start", name = strong) };
        let bold = desc.find(&*name).filter(|_| !name.is_empty()).map(|i| i..i + name.len());
        let desc_el = gpui::StyledText::new(desc.clone()).with_highlights(
            bold.map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })),
        );
        let e1 = entity.clone();
        let e2 = entity.clone();
        return div()
            .m(z(20.))
            .flex()
            .flex_col()
            .child(if group { crate::ui::group_dm::group_avatar(app, chan.id, 80., c(Palette::BG_PRIMARY)) } else { avatar(peer, 80.) })
            .child(
                div()
                    .my(z(8.))
                    .text_size(z(32.))
                    .line_height(z(40.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(tk(idx::TEXT_STRONG))
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .when_some(peer.map(|u| u.name.clone()), |d, uname| {
                        d.child(
                            div()
                                .mb(z(20.))
                                .text_size(z(24.))
                                .line_height(z(32.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(uname),
                        )
                    })
                    .child(desc_el)
                    .when_some(peer.map(|u| u.id), |d, uid| d.child(dm_intro_actions(app, entity, uid))),
            )
            .when(group, |d| {
                use crate::ui::controls::{BtnKind, button_icon};
                d.child(
                    div()
                        .mt(z(16.))
                        .flex()
                        .gap(z(8.))
                        .child(
                            button_icon("gdm-intro-invite", "invite", crate::t!("header.add_to_gdm"), BtnKind::Primary)
                                .on_click(move |_, _, cx| e1.update(cx, |a, cx| a.open_group_dm_picker(cx))),
                        )
                        .child(
                            button_icon("gdm-intro-edit", "pen", crate::t!("menu.edit_group"), BtnKind::Secondary)
                                .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.open_edit_group(ch, cx))),
                        ),
                )
            });
    }
    let is_thread = app.nav.thread_parent(ch).is_some();
    div()
        .m(z(16.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .mt(z(16.))
                .size(z(68.))
                .rounded_full()
                .bg(tk(idx::BACKGROUND_MOD_MUTED))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(
                    if is_thread { Icon::Threads } else { channel.map_or(Icon::Hash, |c| crate::ui::sidebar::channel_icon(c.kind)) },
                    42.,
                    gpui::white(),
                )),
        )
        .child(
            div()
                .text_size(z(32.))
                .line_height(z(40.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .child(if is_thread { name.clone() } else { crate::t!("chat.welcome", name = name) }),
        )
        .when(!is_thread, |d| {
            d.child(
                div()
                    .text_size(z(16.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(crate::t!("chat.channel_start", name = name)),
            )
        })
}

fn dm_intro_actions(app: &PhoenixApp, entity: &Entity<PhoenixApp>, uid: Id) -> gpui::Div {
    use crate::ui::controls::{BtnKind, button_sm};
    let mutual: Vec<Id> = app.people.profiles.get(&uid).map(|p| p.mutual_guild_ids.clone()).unwrap_or_default();
    let is_friend = app
        .nav
        .relationship(uid)
        .is_some_and(|r| r.kind == crate::ui::home::data::RelationshipKind::Friend);
    let mut row = div().mt(z(8.)).flex().flex_wrap().items_center().gap(z(8.));
    if !mutual.is_empty() {
        let label = if mutual.len() == 1 {
            crate::t!("chat.mutual_one")
        } else {
            crate::t!("profile.mutual_servers", n = mutual.len())
        };
        let icons = mutual
            .iter()
            .filter_map(|g| app.guilds.iter().find(|x| x.id == *g))
            .take(3)
            .map(|g| crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 24. * crate::theme::zoom()))
            .collect::<Vec<_>>();
        let e = entity.clone();
        row = row.child(
            div()
                .id("dm-intro-mutual")
                .group("dm-intro-mutual")
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .on_click(move |ev, window, cx| {
                    let at = ev.position();
                    e.update(cx, |a, cx| a.open_profile(uid, None, at, window, cx));
                })
                .child(div().flex().children(icons))
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                        .group_hover("dm-intro-mutual", |s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
                        .child(label),
                )
                .child(div().mx(z(4.)).size(z(4.)).rounded_full().bg(tk(idx::BORDER_STRONG))),
        );
    }
    let e1 = entity.clone();
    let e2 = entity.clone();
    let friend_label = if is_friend { crate::t!("menu.remove_friend") } else { crate::t!("chat.add_friend") };
    row.child(
        div()
            .flex()
            .gap(z(8.))
            .child(button_sm("dm-intro-friend", friend_label, BtnKind::Secondary).on_click(move |_, _, cx| {
                e1.update(cx, |a, cx| {
                    if is_friend {
                        a.confirm_remove_friend(uid, cx)
                    } else {
                        a.relationship_action(uid, crate::ui::home::data::RelAction::Accept, cx)
                    }
                })
            }))
            .child(
                button_sm("dm-intro-block", crate::t!("menu.block"), BtnKind::Secondary)
                    .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.confirm_block(uid, cx))),
            ),
    )
}

fn day_divider(label: SharedString) -> gpui::Div {
    dated_divider(label, false)
}

fn dated_divider(label: SharedString, unread: bool) -> gpui::Div {
    let line = if unread { idx::BACKGROUND_FEEDBACK_NOTIFICATION } else { idx::BORDER_SUBTLE };
    div()
        .ml(z(16.))
        .mr(z(14.))
        .mt(z(24.))
        .mb(z(8.))
        .h(z(1.))
        .bg(tk(line))
        .relative()
        .flex()
        .justify_center()
        .items_center()
        .when(unread, |d| d.child(message_ext::unread_end_cap()))
        .child(
            div()
                .bg(tk(idx::BACKGROUND_BASE_LOWER))
                .rounded(z(8.))
                .mt(z(-1.))
                .px(z(4.))
                .py(z(2.))
                .text_size(z(12.))
                .line_height(z(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(if unread { idx::BACKGROUND_FEEDBACK_NOTIFICATION } else { idx::TEXT_MUTED }))
                .child(label),
        )
}

pub fn format_time_tooltip(ts: i64) -> SharedString {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| crate::i18n::full_datetime(d).into())
        .unwrap_or_default()
}

fn time_tooltip(ts: i64) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    crate::ui::tooltip::tip(format_time_tooltip(ts))
}

pub(crate) fn server_tag_chip(tag: &(SharedString, Option<SharedString>), in_reply: bool) -> gpui::Div {
    div()
        .flex_none()
        .ml(z(if in_reply { 2. } else { 6. }))
        .when(in_reply, |d| d.mr(z(4.)))
        .h(z(15.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_MOD_STRONG))
        .flex()
        .items_center()
        .gap(z(2.))
        .text_size(fz(11.52))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::TEXT_DEFAULT))
        .when_some(tag.1.clone(), |d, u| d.child(img(u).size(z(if in_reply { 12. } else { 14. })).flex_none()))
        .child(tag.0.clone())
}

pub(crate) fn role_icon(url: SharedString) -> gpui::Img {
    img(url).flex_none().size(z(20.)).ml(z(4.)).object_fit(gpui::ObjectFit::Contain)
}

fn new_member_badge(id: Id) -> gpui::Stateful<gpui::Div> {
    div()
        .id(("new-member", id.0))
        .flex_none()
        .size(z(20.))
        .ml(z(4.))
        .mt(z(1.))
        .cursor_pointer()
        .child(gpui::svg().path(crate::icons::path("new-user")).size_full().text_color(tk(idx::TEXT_FEEDBACK_POSITIVE)))
        .tip(crate::t!("chat.new_member"))
}

pub(crate) fn bot_tag(verified: bool) -> gpui::Div {
    div()
        .flex_none()
        .ml(z(4.))
        .mt(z(1.6))
        .px(z(4.4))
        .h(z(15.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_BRAND))
        .flex()
        .items_center()
        .text_size(z(12.8))
        .line_height(z(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(gpui::white())
        .when(verified, |d| d.child(crate::ui::members::verified_check().mt(z(-0.32)).ml(z(-3.2))))
        .child(crate::t!("common.app"))
}

pub(crate) fn message(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    msg: &Message,
    r: &RenderedMessage,
    entry: &crate::app::ChannelMessages,
    cx: &App,
) -> AnyElement {
    let author_cow = app.member_user(app.selected_guild, msg.author);
    let author = author_cow.as_deref();
    let name = msg.author_nick.clone().or_else(|| app.member_nick(msg.author)).or_else(|| author.map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("common.unknown"));
    let color = app.author_color(msg.author).map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG));
    let id = msg.id;
    if let Some(sys) = system::render(app, entity, msg, r) {
        return row_wrapper().child(sys).into_any_element();
    }
    let compact = app.prefs.synced.compact;
    let editing = app.chat.edit.as_ref().is_some_and(|e| e.id == id);
    let replying = app.replying_to == Some(id);
    let reply_pings_me = msg.reply_mentions_author
        && msg.reply_to.and_then(|rid| entry.index_of(rid)).is_some_and(|p| entry.msgs[p].author == app.me.id);
    let mentioned = r.mentions_me || msg.mention_everyone || reply_pings_me;
    let ts = msg.timestamp;
    let edited_at = if msg.edited_at > 0 { msg.edited_at } else { ts };

    let mut content = div().flex().flex_col().min_w_0().text_size(fz(16.)).text_color(c(Palette::TEXT_NORMAL));
    let last_text = r.blocks.iter().rposition(|b| matches!(b, RBlock::Text(_)));
    let n_blocks = r.blocks.len();
    let compact_avatars = compact && app.prefs.synced.flag("avatars");
    let compact_header_left = 16. * crate::theme::font_scale();
    let is_bot = author.is_some_and(|a| a.bot);
    let overflow = if compact {
        compact_header_overflow(
            &r.time_short,
            &name,
            compact_avatars,
            is_bot,
            author.and_then(|u| u.tag.as_ref()).map(|t| t.0.as_ref()),
            compact_header_left,
            cx,
        )
    } else {
        0.
    };
    if compact && !matches!(r.blocks.first(), Some(RBlock::Text(_))) {
        content = content.child(div().flex_none().h(fz(22.)));
    }
    let show_embed = |e: &&crate::model::Embed| {
        let sy = &app.prefs.synced;
        let media = matches!(e.kind.as_deref(), Some("image" | "gifv" | "video"));
        sy.flag("embeds") && (!media || sy.flag("media-links"))
    };
    let gif_only = matches!(msg.embeds.as_slice(), [e] if e.kind.as_deref() == Some("gifv") && e.url.as_deref() == Some(msg.content.trim()) && show_embed(&e));
    for (bi, b) in r.blocks.iter().enumerate().filter(|_| !gif_only) {
        let edited_here = (msg.edited && last_text == Some(bi) && bi + 1 == n_blocks).then_some(edited_at);
        match b {
            RBlock::Text(t) if compact && bi == 0 => {
                content = content.child(rich(entity, id, bi, &indent_rich(t, overflow, cx), edited_here));
            }
            _ => content = content.child(block(entity, id, bi, b, edited_here)),
        }
    }
    let edited_after_embeds = msg.edited && last_text.is_none();
    if msg.edited && !edited_after_embeds && !last_text.is_some_and(|l| l + 1 == n_blocks) {
        content = content.child(edited_marker(edited_at));
    }
    media::set_current_author(Some(media::ViewerAuthor {
        user: author.cloned(),
        name: name.clone(),
        color,
        time: r.time_full.clone(),
    }));
    content = content.children(invite_embed::render(app, entity, msg.author, &msg.content, cx));
    let visual: Vec<&Attachment> = msg.attachments.iter().filter(|a| mosaic::is_visual(a)).collect();
    if visual.len() >= 2 && crate::ui::settings::effects::inline_attachments() {
        content = content.child(mosaic::render(&visual, cx));
        for a in msg.attachments.iter().filter(|a| !mosaic::is_visual(a)) {
            content = content.child(div().mt(z(4.)).child(attachment(a, cx)));
        }
    } else {
        for a in &msg.attachments {
            content = content.child(attachment(a, cx));
        }
    }
    for e in msg.embeds.iter().filter(show_embed) {
        content = content.child(embed(app, e, cx));
    }
    media::set_current_author(None);
    if let Some(f) = &msg.forward {
        content = div().flex().flex_col().min_w_0().text_size(fz(16.)).text_color(c(Palette::TEXT_NORMAL)).child(forward::wrap(app, f, content));
    }
    content = content.children(crate::ui::components::render(app, entity, msg, cx)).children(crate::ui::components::render_poll(app, entity, msg));
    if edited_after_embeds {
        content = content.child(div().mt(z(4.)).h(z(20.)).flex().items_center().child(edited_marker(edited_at)));
    }
    if !msg.reactions.is_empty() && app.prefs.synced.flag("reactions") {
        content = content.child(message_ext::reactions(app, entity, msg));
    }
    if let Some(t) = &msg.thread {
        content = content.child(message_ext::thread_chip(app, entity, t, false));
    }
    if msg.flags & crate::model::FLAG_EPHEMERAL != 0 {
        content = content.child(crate::ui::chat::pending::ephemeral_footer(entity, msg));
    }
    let content = match message_ext::edit_box(app, entity, msg) {
        Some(b) => b,
        None => content,
    };
    let content = if msg.send_state == crate::model::SendState::Sending { content.opacity(0.5) } else { content };
    let content = match attach::upload_progress_card(app, entity, msg) {
        Some(card) => div().flex().flex_col().min_w_0().child(content).child(card),
        None => content,
    };

    let reply = msg.reply_to.map(|rid| {
        let found = entry.index_of(rid);
        let (ru, (preview, mranges), rnick) = match found {
            Some(p) => (app.member_user(app.selected_guild, entry.msgs[p].author), {
                let (t, m) = crate::richtext::preview_with_mentions(&entry.rendered[p].parsed);
                (SharedString::from(t), m)
            }, entry.msgs[p].author_nick.clone().or_else(|| app.member_nick(entry.msgs[p].author)).or_else(|| {
                let a = entry.msgs[p].author;
                entry.msgs.iter().rev().filter(|m| m.author == a).find_map(|m| m.author_nick.clone())
            })),
            None => match &msg.reply_preview {
                Some((a, nick, text)) => (
                    app.member_user(app.selected_guild, *a),
                    {
                        let (t, m) = crate::richtext::preview_with_mentions(&crate::markdown::parse(text, &app.resolver()));
                        (SharedString::from(t), m)
                    },
                    nick.clone().or_else(|| app.member_nick(*a)),
                ),
                None => (None, (crate::t!("chat.reply_deleted"), Vec::new()), None),
            },
        };
        let reply_voice = match found {
            Some(p) => entry.msgs[p].flags & phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE != 0,
            None => msg.reply_voice,
        };
        let found = found.or_else(|| msg.reply_preview.as_ref().map(|_| usize::MAX));
        div()
            .relative()
            .h(fz(if compact { 20. } else { 18. }))
            .when(!compact, |d| d.mb(z(4.)))
            .when(compact, |d| d.ml(z(-24.)).pl(z(10.)))
            .flex()
            .items_center()
            .gap(z(4.))
            .text_size(fz(14.))
            .line_height(fz(18.))
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(
                div()
                    .absolute()
                    .left(z(if compact { -23. } else { -41. }))
                    .top(z(9.))
                    .w(z(if compact { 29. } else { 37. }))
                    .h(z(if compact { 11. } else { 13. }))
                    .border_l_2()
                    .border_t_2()
                    .rounded_tl(z(6.))
                    .border_color(tk(idx::SPINE_DEFAULT))
                    .hover(|s| s.border_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))),
            )
            .when(!compact, |d| d.child(avatar(ru.as_deref(), 16.)))
            .when(ru.as_deref().is_some_and(|u| u.bot), |d| d.child(bot_tag(ru.as_deref().is_some_and(|u| u.verified_bot)).ml_0()))
            .child(
                crate::fonts::styled_name(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ru.as_deref().and_then(|u| app.author_color(u.id)).map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG)))
                        .opacity(0.64),
                    ru.as_deref(),
                    false,
                )
                .id(("reply-author", msg.id.0))
                .when_some(ru.as_deref().map(|u| u.id), |d, uid| {
                    d.cursor_pointer().hover(|s| s.underline()).on_click(message_ext::open_user(entity, uid, msg.channel))
                })
                .child({
                    let reply_text = SharedString::from(format!(
                        "{}{}",
                        if msg.reply_mentions_author { "@" } else { "" },
                        rnick.map(|n| n.to_string()).or_else(|| ru.as_deref().map(|u| u.display_name.to_string())).unwrap_or_else(|| crate::t!("common.unknown").to_string())
                    ));
                    match ru.as_deref().and_then(|u| crate::ui::role_gradient::for_author(app, u.id)) {
                        Some(stops) => crate::fonts::name_text_with(reply_text, ru.as_deref(), 14., Some(stops)),
                        None => crate::fonts::name_text(reply_text, ru.as_deref(), 14.),
                    }
                }),
            )
            .when_some(ru.and_then(|u| u.tag.clone()), |d, tg| d.child(server_tag_chip(&tg, true).opacity(0.64)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
                    .id(("reply-jump", msg.id.0))
                    .cursor_pointer()
                    .on_click({
                        let (e, ch) = (entity.clone(), msg.channel);
                        move |_, _, cx| e.update(cx, |a, cx| a.jump_to_message(ch, rid, cx))
                    })
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(found.is_none() || preview.is_empty(), |d| d.italic())
                    .map(|d| {
                        let text = if found.is_some() && preview.is_empty() {
                            if reply_voice { crate::t!("chat.reply_click_to_see_voice") } else { crate::t!("chat.reply_click_to_see") }
                        } else {
                            preview
                        };
                        let seg = |a: usize, b: usize| -> gpui::AnyElement {
                            let hl: Vec<_> = mranges
                                .iter()
                                .filter(|r| r.start < b && r.end > a)
                                .map(|r| {
                                    (
                                        r.start.max(a) - a..r.end.min(b) - a,
                                        gpui::HighlightStyle {
                                            color: Some(c(Palette::MENTION_FG)),
                                            background_color: Some(crate::theme::ca(Palette::MENTION_BG)),
                                            font_weight: Some(FontWeight::MEDIUM),
                                            ..Default::default()
                                        },
                                    )
                                })
                                .collect();
                            let t = SharedString::from(text[a..b].to_string());
                            if hl.is_empty() { t.into_any_element() } else { gpui::StyledText::new(t).with_highlights(hl).into_any_element() }
                        };
                        let tokens = crate::media::emoji::find_custom(&text);
                        if tokens.is_empty() {
                            return d.child(seg(0, text.len()));
                        }
                        let mut row = d.flex().items_center();
                        let mut at = 0;
                        for (r, _) in &tokens {
                            if r.start > at {
                                row = row.child(div().flex_none().child(seg(at, r.start)));
                            }
                            row = row.child(div().flex_none().size(z(18.)).child(crate::media::emoji::emoji_element(&text[r.clone()], 18. * crate::theme::zoom())));
                            at = r.end;
                        }
                        if at < text.len() {
                            row = row.child(div().flex_none().child(seg(at, text.len())));
                        }
                        row
                    }),
            )
    });

    let role_dot = app.author_role_dot(msg.author);
    let author_name = crate::fonts::styled_name(div(), author, false)
        .flex_none()
        .when(role_dot.is_some(), |d| d.flex().items_center())
        .text_size(fz(16.))
        .line_height(fz(22.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .id(("msg-author", id.0))
        .cursor_pointer()
        .hover(|s| s.underline())
        .on_click(message_ext::open_author(entity, msg))
        .child(match crate::ui::role_gradient::for_author(app, msg.author) {
            Some(stops) => crate::fonts::name_text_with(name, author, 16., Some(stops)),
            None => crate::fonts::name_text(name, author, 16.),
        })
        .children(role_dot.map(|col| div().flex_none().ml(z(4.)).size(z(8.)).rounded_full().bg(c(col))));

    let base = div()
        .id(("msg", id.0))
        .group("msg")
        .on_hover({
            let e = entity.clone();
            move |h, _, cx| {
                if crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(2, id.0), *h) {
                    e.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .on_mouse_down(gpui::MouseButton::Right, message_ext::on_right_click(entity, msg))
        .relative()
        .pr(z(space::xl()))
        .map(|d| {
            let ephemeral = msg.flags & crate::model::FLAG_EPHEMERAL != 0;
            let (bg, hover, bar) = if ephemeral {
                (idx::BRAND_05A, idx::BRAND_10A, Some(idx::BRAND_500))
            } else if replying {
                (idx::MESSAGE_HIGHLIGHT_BACKGROUND_DEFAULT, idx::BRAND_10A, Some(idx::BRAND_500))
            } else if mentioned {
                (idx::MESSAGE_MENTIONED_BACKGROUND_DEFAULT, idx::MESSAGE_MENTIONED_BACKGROUND_HOVER, Some(idx::ICON_FEEDBACK_WARNING))
            } else {
                (usize::MAX, idx::MESSAGE_BACKGROUND_HOVER, None)
            };
            let selected = editing || app.chat.menu.is_some_and(|m| m.msg == id) || crate::shortcuts::message::focused().is_some_and(|f| f.1 == id);
            let d = match (bar, selected) {
                (_, true) => d.bg(tk(hover)),
                (Some(_), false) => d.bg(tk(bg)),
                (None, false) => d,
            };
            d.hover(move |s| s.bg(tk(hover))).when_some(bar, |d, b| {
                d.child(div().absolute().left_0().top_0().bottom_0().w(z(2.)).bg(tk(b)))
            })
            .when(app.chat.flash == Some(id), |d| d.child(flash_overlay(id)))
        });

    let body = if compact {
        base.pl(fz(80.))
            .py(fz(2.))
            .when_some(reply, |d, reply| d.child(reply))
            .child(
                div()
                    .absolute()
                    .left(z(compact_header_left))
                    .top(fz(if msg.reply_to.is_some() { 24. } else { 2. }))
                    .h(fz(22.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id(("msg-time", id.0))
                            .flex_none()
                            .mr(fz(4.))
                            .text_size(fz(11.))
                            .text_color(tk(idx::TEXT_MUTED))
                            .tooltip(time_tooltip(ts))
                            .child(r.time_short.clone()),
                    )
                    .when(compact_avatars, |d| {
                        d.child(
                            div()
                                .id(("msg-avatar-compact", id.0))
                                .flex_none()
                                .ml(fz(1.6))
                                .mr(fz(4.))
                                .cursor_pointer()
                                .rounded_full()
                                .on_click(message_ext::open_author(entity, msg))
                                .child(avatar(author, 16. * crate::theme::font_scale())),
                        )
                    })
                    .child(author_name)
                    .when_some(author.and_then(|u| u.tag.clone()), |d, tg| d.child(server_tag_chip(&tg, false)))
                    .when_some(app.author_role_icon(msg.author), |d, u| d.child(role_icon(u)))
                    .when(app.is_new_member(msg.author), |d| d.child(new_member_badge(msg.id)))
                    .when(is_bot, |d| d.child(bot_tag(author.is_some_and(|u| u.verified_bot))))
                    .when(crate::ui::forum::is_op(app, msg.channel, msg.author), |d| d.child(crate::ui::forum::op_tag())),
            )
            .child(content)
    } else {
        base.pl(z(msg_left()))
            .py(fz(2.))
            .when_some(reply, |d, reply| d.child(reply))
            .when(r.group_start, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(if crate::theme::font_scaled_down() { fz(16.) } else { z(space::md()) })
                        .top({
                            let pad = if crate::theme::font_scaled_down() { 4. * crate::theme::font_scale() } else { 4. };
                            z(if msg.reply_to.is_some() { 18. * crate::theme::font_scale() + 4. + pad } else { pad })
                        })
                        .id(("msg-avatar", id.0))
                        .cursor_pointer()
                        .rounded_full()
                        .hover(|s| {
                            s.shadow(vec![gpui::BoxShadow {
                                color: gpui::hsla(0., 0., 0., 0.16),
                                offset: gpui::point(z(0.), z(4.)),
                                blur_radius: z(4.),
                                spread_radius: z(0.),
                                inset: false,
                            }])
                        })
                        .active(|s| s.mt(z(1.)))
                        .on_click(message_ext::open_author(entity, msg))
                        .child(crate::ui::avatar_hover(author, msg_avatar(), crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(2, id.0))))
                        .when_some(author.and_then(|u| u.decoration.clone()), |d, deco| {
                            let s = msg_avatar() * 1.2;
                            let off = (s - msg_avatar()) / 2.;
                            d.child(
                                gpui::img(crate::ui::avatar_deco::src_for(&deco, crate::ui::avatar_deco::hover_key(2, id.0)))
                                    .id("deco-frames")
                                    .absolute()
                                    .size(z(s))
                                    .left(z(-off))
                                    .top(z(-off)),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .h(fz(22.))
                        .child(author_name)
                        .when_some(author.and_then(|u| u.tag.clone()), |d, tg| d.child(server_tag_chip(&tg, false)))
                        .when_some(app.author_role_icon(msg.author), |d, u| d.child(role_icon(u)))
                    .when(app.is_new_member(msg.author), |d| d.child(new_member_badge(msg.id)))
                        .when(is_bot, |d| d.child(bot_tag(author.is_some_and(|u| u.verified_bot))))
                        .when(crate::ui::forum::is_op(app, msg.channel, msg.author), |d| d.child(crate::ui::forum::op_tag()))
                        .child(
                            div()
                                .id(("msg-time", id.0))
                                .ml(fz(4.))
                                .text_size(fz(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::CHAT_TEXT_MUTED))
                                .tooltip(time_tooltip(ts))
                                .child(r.time_full.clone()),
                        ),
                )
            })
            .when(!r.group_start, |d| {
                d.child(
                    div()
                        .id(("msg-time", id.0))
                        .absolute()
                        .left_0()
                        .top(fz(2.))
                        .w(if crate::theme::font_scaled_down() { fz(56.) } else { z(56.) })
                        .h(fz(22.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .text_size(fz(11.))
                        .text_color(gpui::transparent_black())
                        .group_hover("msg", |s| s.text_color(tk(idx::TEXT_MUTED)))
                        .tooltip(time_tooltip(ts))
                        .child(r.time_short.clone()),
                )
            })
            .child(content)
    }
    .when(!compact && msg.thread.is_some(), |d| d.child(message_ext::thread_spine(if msg.reply_to.is_some() { 70. } else { 48. })))
    .child(message_ext::toolbar(app, entity, msg, app.chat.shift_held));
    let new_marker = app.chat.unread_marker.is_some_and(|(_, m)| m == id);
    let gap = if !r.group_start { 0. } else if compact { 4. } else { 17. * crate::theme::font_scale() };
    row_wrapper()
        .when_some(r.day_divider.clone(), |d, label| d.child(dated_divider(label, new_marker)))
        .when(new_marker && r.day_divider.is_none(), |d| d.child(message_ext::new_divider(r.group_start)))
        .child(div().pt(z(gap)).child(body))
        .into_any_element()
}

fn row_wrapper() -> gpui::Div {
    div().w_full().flex().flex_col()
}

fn text_width(cx: &App, font: &gpui::Font, size: gpui::Pixels, s: &str) -> f32 {
    let ts = cx.text_system();
    let id = ts.resolve_font(font);
    s.chars().map(|ch| ts.advance(id, size, ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum()
}

fn compact_header_overflow(time: &str, name: &str, compact_avatars: bool, bot: bool, tag: Option<&str>, left: f32, cx: &App) -> f32 {
    let mut name_font = crate::richtext::ui_font();
    name_font.weight = FontWeight::MEDIUM;
    let mut w = text_width(cx, &crate::richtext::ui_font(), fz(11.), time) + f32::from(fz(4.));
    if compact_avatars {
        w += f32::from(fz(16. + 1.6 + 4.));
    }
    w += text_width(cx, &name_font, fz(16.), name);
    if bot {
        let mut f = name_font.clone();
        f.weight = FontWeight::SEMIBOLD;
        w += text_width(cx, &f, z(12.8), "APP") + f32::from(z(12.8));
    }
    if let Some(tg) = tag {
        let mut f = name_font.clone();
        f.weight = FontWeight::SEMIBOLD;
        w += text_width(cx, &f, fz(11.52), tg) + f32::from(z(14.));
    }
    w += f32::from(z(4.));
    (f32::from(z(left)) + w - f32::from(fz(80.))).max(0.)
}

fn indent_rich(t: &RichText, width: f32, cx: &App) -> RichText {
    if width <= 0. {
        return t.clone();
    }
    const SP: char = '\u{2009}';
    let font = crate::richtext::ui_font();
    let adv = text_width(cx, &font, fz(16.), "\u{2009}").max(1.);
    let n = (width / adv).ceil() as usize;
    let prefix: String = std::iter::repeat_n(SP, n).collect();
    let shift = prefix.len();
    let mut out = t.clone();
    out.text = format!("{prefix}{}", t.text).into();
    out.runs.insert(
        0,
        gpui::TextRun { len: shift, font, color: gpui::transparent_black(), background_color: None, underline: None, strikethrough: None },
    );
    for r in &mut out.click_ranges {
        *r = r.start + shift..r.end + shift;
    }
    for (r, _) in &mut out.emoji {
        *r = r.start + shift..r.end + shift;
    }
    out
}

fn edited_marker(ts: i64) -> impl IntoElement {
    div()
        .id(("edited", ts as u64))
        .tooltip(time_tooltip(ts))
        .text_size(z(10.))
        .line_height(z(10.))
        .font_weight(FontWeight::NORMAL)
        .text_color(tk(idx::TEXT_MUTED))
        .child(crate::t!("chat.edited"))
}

fn with_edited(t: &RichText) -> (SharedString, Vec<gpui::TextRun>) {
    let mut text = t.text.to_string();
    text.push_str(&crate::t!("chat.edited_suffix"));
    let mut runs = t.runs.clone();
    let mut f = crate::richtext::ui_font();
    f.weight = FontWeight::NORMAL;
    runs.push(gpui::TextRun {
        len: crate::t!("chat.edited_suffix").len(),
        font: f,
        color: c(Palette::TEXT_MUTED),
        background_color: None,
        underline: None,
        strikethrough: None,
    });
    (text.into(), runs)
}

fn rich(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, t: &RichText, edited: Option<i64>) -> AnyElement {
    if !t.emoji.is_empty() || t.has_mention() || !message_ext::code_ranges(t).is_empty() {
        let el = message_ext::rich_with_emoji(entity, msg, bi, t);
        if let Some(at) = edited {
            return div()
                .flex()
                .flex_wrap()
                .items_end()
                .child(div().max_w_full().child(el))
                .child(div().ml(z(4.)).mb(z(5.)).child(edited_marker(at)))
                .into_any_element();
        }
        return el;
    }
    if let Some(at) = edited.filter(|_| t.click_ranges.is_empty() && !t.text.contains('\n') && t.text.chars().count() <= 60) {
        let plain = div().line_height(gpui::relative(1.375)).child(StyledText::new(t.text.clone()).with_runs(t.runs.clone()));
        return div()
            .flex()
            .flex_wrap()
            .items_end()
            .child(plain)
            .child(div().ml(z(4.)).mb(z(5.)).child(edited_marker(at)))
            .into_any_element();
    }
    let (text, runs) = if edited.is_some() { with_edited(t) } else { (t.text.clone(), t.runs.clone()) };
    let runs = mention::hovered_runs(msg, bi, 0, &t.click_ranges, &t.click_targets, &runs).unwrap_or(runs);
    let styled = StyledText::new(text).with_runs(runs);
    let lh = gpui::relative(1.375);
    let edited_tip = edited.map(|at| {
        let from = t.text.len();
        let tip = std::rc::Rc::new(time_tooltip(at));
        move |ix: usize, w: &mut Window, cx: &mut App| (ix >= from).then(|| tip(w, cx))
    });
    if t.click_ranges.is_empty() {
        return match edited_tip {
            Some(tip) => div()
                .line_height(lh)
                .child(InteractiveText::new(("rt", (msg.0 << 8) | bi as u64), styled).tooltip(tip))
                .into_any_element(),
            None => div().line_height(lh).child(styled).into_any_element(),
        };
    }
    let targets = t.click_targets.clone();
    let shown: Vec<String> = t.click_ranges.iter().map(|r| t.text.get(r.clone()).unwrap_or_default().to_owned()).collect();
    let e = entity.clone();
    div()
        .line_height(lh)
        .on_mouse_down(gpui::MouseButton::Right, move |_, _, _| {
            if let Some(url) = mention::hovered_link(msg, bi, 0) {
                panels::set_menu_link(url);
            }
        })
        .child(
            InteractiveText::new(("rt", (msg.0 << 8) | bi as u64), styled)
                .on_hover({
                    let (hr, ht) = (t.click_ranges.clone(), t.click_targets.clone());
                    move |ix, _ev, w, _cx| mention::on_hover(msg, bi, 0, &hr, &ht, ix, w)
                })
                .when_some(edited_tip, |el, tip| el.tooltip(tip))
                .on_click(
                t.click_ranges.clone(),
                move |ix, w, cx| match &targets[ix] {
                    ClickTarget::Link(url) => e.update(cx, |a, cx| crate::ui::modals::leave_link::click(a, &shown[ix], url, w, cx)),
                    ClickTarget::Spoiler(sp) => {
                        let sp = *sp;
                        e.update(cx, |a, cx| a.reveal_spoiler(msg, sp, cx));
                    }
                    ClickTarget::Mention(m) => mention::click(&e, m, w, cx),
                },
            ),
        )
        .into_any_element()
}

fn block(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, b: &RBlock, edited: Option<i64>) -> AnyElement {
    match b {
        RBlock::Text(t) => rich(entity, msg, bi, t, edited),
        RBlock::Quote(t) => div()
            .flex()
            .my(z(4.))
            .max_w(gpui::relative(0.9))
            .child(div().w(z(4.)).rounded(z(4.)).bg(tk(idx::SPINE_DEFAULT)).flex_none())
            .child(div().flex_1().min_w_0().pl(z(12.)).pr(z(8.)).child(rich(entity, msg, bi, t, None)))
            .into_any_element(),
        RBlock::Heading(level, t) => {
            let size = match level {
                1 => 24.,
                2 => 20.,
                _ => 16.,
            };
            let first = if *level <= 2 { 8. } else { 4. };
            div()
                .mt(z(if bi == 0 { first } else { 16. }))
                .mb(z(8.))
                .text_size(fz(size))
                .child(rich(entity, msg, bi, t, edited))
                .into_any_element()
        }
        RBlock::Subtext(t) => div()
            .text_size(fz(14.))
            .line_height(fz(19.25))
            .text_color(tk(idx::TEXT_SUBTLE))
            .child(rich(entity, msg, bi, t, edited))
            .into_any_element(),
        RBlock::ListItem { ordered, depth, text } => {
            let marker: SharedString = match ordered {
                Some(n) => format!("{n}.").into(),
                None if *depth == 0 => "\u{2022}".into(),
                None => "\u{25E6}".into(),
            };
            div()
                .flex()
                .ml(z(16. * (*depth as f32 + 1.) - 16.))
                .mb(z(4.))
                .child(
                    div()
                        .flex_none()
                        .w(z(if ordered.is_some() { 24. } else { 16. }))
                        .pr(z(4.))
                        .flex()
                        .justify_end()
                        .line_height(gpui::relative(1.375))
                        .child(marker),
                )
                .child(div().flex_1().min_w_0().child(rich(entity, msg, bi, text, edited)))
                .into_any_element()
        }
        RBlock::Code { code, runs, .. } => div()
            .mt(z(6.))
            .max_w(gpui::relative(0.9))
            .p(fz(7.))
            .rounded(z(4.))
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .border_1()
            .border_color(tk(idx::BORDER_NORMAL))
            .font_family(FONT_MONO())
            .text_size(fz(14.))
            .line_height(fz(18.))
            .text_color(tk(idx::TEXT_CODE))
            .relative()
            .group(crate::ui::code_copy::GROUP)
            .child(StyledText::new(code.clone()).with_runs(runs.clone()))
            .child(crate::ui::code_copy::CodeCopy::new(code.clone()))
            .into_any_element(),
    }
}

fn media_image(url: &str, original: SharedString, filename: SharedString, size_o: (u32, u32), size: (f32, f32)) -> gpui::Stateful<gpui::Img> {
    media_image_with(url, original, filename, size_o, size, false)
}

pub(crate) fn media_image_with(
    url: &str,
    original: SharedString,
    filename: SharedString,
    (ow, oh): (u32, u32),
    (w, h): (f32, f32),
    still: bool,
) -> gpui::Stateful<gpui::Img> {
    let src = if still { media::video_poster(url, (w * 2.) as u32, (h * 2.) as u32) } else { media::resized(url, (w * 2.) as u32, (h * 2.) as u32) };
    let viewer = media::ViewerImage {
        url: media::resized(url, ow.min(2048), oh.min(2048)),
        original: if original.is_empty() { url.to_owned().into() } else { original },
        filename,
        width: ow,
        height: oh,
        author: media::current_author(),
    };
    let mut im = img(src.clone());
    if let Some(cache) = media::lru::chat() {
        im = im.image_cache(&cache);
    }
    im.id(gpui::ElementId::Name(src))
        .w(z(w))
        .max_w_full()
        .aspect_ratio(w / h.max(1.))
        .rounded(z(8.))
        .object_fit(if ow == 0 || oh == 0 { gpui::ObjectFit::Contain } else { gpui::ObjectFit::Cover })
        .cursor_pointer()
        .with_loading(|| div().size_full().rounded(z(8.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).into_any_element())
        .with_fallback(|| div().size_full().rounded(z(8.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).into_any_element())
        .on_click(move |_, _, cx| media::open_viewer(viewer.clone(), cx))
}

pub(crate) fn spoiler_cover(key: SharedString, url: &str, (w, h): (f32, f32), cx: &App) -> Option<AnyElement> {
    if media::is_revealed(&key, cx) {
        return None;
    }
    let tiny = media::resized(url, ((w / 24.) as u32).max(2), ((h / 24.) as u32).max(2));
    Some(
        div()
            .id(gpui::ElementId::Name(format!("spoiler-{key}").into()))
            .relative()
            .w(z(w))
            .h(z(h))
            .rounded(z(8.))
            .overflow_hidden()
            .cursor_pointer()
            .bg(tk(idx::SPOILER_BACKGROUND_COLOR_HIDDEN))
            .group("spoiler-cover")
            .child(img(tiny).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
            .child(
                div().absolute().inset_0().flex().items_center().justify_center().child(
                    div()
                        .px(z(12.))
                        .py(z(6.))
                        .rounded(z(20.))
                        .bg(tk(idx::SPOILER_WARNING_BACKGROUND_COLOR))
                        .text_size(z(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tk(idx::SPOILER_WARNING_TEXT_COLOR))
                        .group_hover("spoiler-cover", |s| {
                            s.bg(tk(idx::SPOILER_WARNING_BACKGROUND_COLOR_HOVER))
                                .text_color(tk(idx::SPOILER_WARNING_TEXT_COLOR_HOVER))
                        })
                        .child(crate::t!("chat.spoiler")),
                ),
            )
            .on_click(move |_, _, cx| media::reveal(key.clone(), cx))
            .into_any_element(),
    )
}

fn still_sticker(u: SharedString, fmt: media::StickerFormat) -> SharedString {
    let animated = matches!(fmt, media::StickerFormat::Apng | media::StickerFormat::Gif);
    if !animated || media::sticker_plays(&u) || !u.contains("media.discordapp.net") {
        return u;
    }
    if u.contains("&animated=true") {
        return u.replace("&animated=true", "").into();
    }
    let sep = if u.contains('?') { '&' } else { '?' };
    format!("{u}{sep}passthrough=false").into()
}

pub(crate) fn attachment(a: &Attachment, cx: &App) -> AnyElement {
    let remote = attach::loadable(&a.url);
    let spoiler = a.is_spoiler();
    let inline = crate::ui::settings::effects::inline_attachments();
    match a.kind {
        AttachmentKind::Sticker(media::StickerFormat::Lottie) if !a.url.is_empty() => {
            let name = a.filename.clone();
            let im = media::lottie::image(a.url.clone(), 200, !media::sticker_plays(&a.url))
                .id(gpui::ElementId::Name(a.url.clone()))
                .mt(z(4.))
                .size(z(160.))
                .object_fit(gpui::ObjectFit::Contain)
                .with_fallback(move || sticker_name(name.clone()));
            return media::sticker_hover(a.url.clone(), im).into_any_element();
        }
        AttachmentKind::Sticker(fmt) => {
            let name = a.filename.clone();
            let animated = matches!(fmt, media::StickerFormat::Apng | media::StickerFormat::Gif);
            return match media::sticker_url(0, fmt).and(Some(a.url.clone())).filter(|u| !u.is_empty()) {
                Some(u) => {
                    let im = img(still_sticker(u.clone(), fmt))
                        .id(media::anim_id(&u))
                        .mt(z(4.))
                        .size(z(160.))
                        .object_fit(gpui::ObjectFit::Contain)
                        .with_fallback(move || sticker_name(name.clone()));
                    if animated { media::sticker_hover(u, im).into_any_element() } else { im.into_any_element() }
                }
                None => div().mt(z(4.)).child(sticker_name(name)).into_any_element(),
            };
        }
        AttachmentKind::Image | AttachmentKind::Video if remote && inline => {
            let size = media::fit(a.width, a.height, media::MAX_W, media::MAX_H);
            if spoiler && let Some(cover) = spoiler_cover(a.url.clone(), &a.url, size, cx) {
                return div().mt(z(4.)).child(cover).into_any_element();
            }
            if a.kind == AttachmentKind::Video {
                let alt = alt_text::description(a, false);
                return div()
                    .mt(z(4.))
                    .relative()
                    .child(attach::video_card(a, size, cx))
                    .children(alt.map(|t| alt_text::video_badge(a.url.clone(), t, attach::video_group(&a.url))))
                    .into_any_element();
            }
            let link = if a.original.is_empty() { a.url.clone() } else { a.original.clone() };
            let alt = alt_text::description(a, false);
            return div()
                .mt(z(4.))
                .relative()
                .on_mouse_down(gpui::MouseButton::Right, move |_, _, _| panels::set_menu_media(link.clone()))
                .child({
                    let gif = a.filename.to_ascii_lowercase().ends_with(".gif");
                    let still = gif && !media::gif_plays(&a.url);
                    let im = media_image_with(&a.url, a.original.clone(), a.filename.clone(), (a.width, a.height), size, still);
                    if gif { media::gif_hover(a.url.clone(), im).into_any_element() } else { im.into_any_element() }
                })
                .children(alt.map(|t| alt_text::badge(a.url.clone(), t)))
                .into_any_element();
        }
        AttachmentKind::Audio => {
            if let Some(meta) = a.voice.as_ref() {
                return voice_message::player(&a.url, meta, cx);
            }
            return div().mt(z(4.)).child(attach::audio_card(a)).into_any_element();
        }
        AttachmentKind::File if a.url.starts_with("https://") && inline && plaintext::previewable(&a.filename) => {
            if let Some(el) = plaintext::render(a, cx) {
                return el;
            }
        }
        _ => {}
    }
    if a.is_image && inline {
        let (w, h) = media::fit(a.width, a.height, media::MAX_W, media::MAX_H);
        return div()
            .mt(z(4.))
            .w(z(w))
            .h(z(h))
            .rounded(z(8.))
            .bg(c(Palette::BG_SECONDARY))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(z(4.))
            .child(icon(Icon::Image, 48., c(Palette::INTERACTIVE_MUTED)))
            .child(div().text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(a.filename.clone()))
            .into_any_element();
    }
    div().mt(z(4.)).child(attach::file_card(a)).into_any_element()
}

fn sticker_name(name: SharedString) -> AnyElement {
    div()
        .px(z(8.))
        .py(z(4.))
        .rounded(z(4.))
        .bg(c(Palette::BG_SECONDARY))
        .text_size(z(14.))
        .text_color(c(Palette::TEXT_MUTED))
        .child(crate::t!("chat.sticker", name = name))
        .into_any_element()
}

fn embed_blocks(app: &PhoenixApp, src: &str) -> std::rc::Rc<[RBlock]> {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    type EmbedCache = (u32, HashMap<SharedString, Rc<[RBlock]>>);
    thread_local! {
        static CACHE: RefCell<EmbedCache> = RefCell::default();
    }
    CACHE.with_borrow_mut(|(generation, map)| {
        let g = crate::theme::style_generation();
        if *generation != g || map.len() >= 512 {
            *generation = g;
            map.clear();
        }
        if let Some(b) = map.get(src) {
            return b.clone();
        }
        let parsed = crate::markdown::parse_emoji_links(src, &app.resolver());
        let blocks: Rc<[RBlock]> = crate::richtext::render_blocks(&parsed, &|_| true, c(Palette::TEXT_NORMAL)).into();
        map.insert(SharedString::from(src.to_owned()), blocks.clone());
        blocks
    })
}

fn embed_markdown(app: &PhoenixApp, src: &str, size: f32) -> AnyElement {
    let blocks = embed_blocks(app, src);
    let mut d = div().flex().flex_col().text_size(fz(size)).line_height(z(18.)).min_w_0();
    for b in blocks.iter().cloned() {
        d = d.child(match b {
            RBlock::Code { code, runs, .. } => div()
                .my(z(4.))
                .p(z(8.))
                .rounded(z(4.))
                .bg(c(Palette::BG_TERTIARY))
                .font_family(FONT_MONO())
                .text_size(z(12.))
                .line_height(z(16.))
                .child(StyledText::new(code).with_runs(runs))
                .into_any_element(),
            RBlock::Quote(t) => div()
                .flex()
                .child(div().w(z(4.)).rounded(z(4.)).bg(c(Palette::INTERACTIVE_MUTED)).flex_none())
                .child(div().pl(z(12.)).child(StyledText::new(t.text).with_runs(t.runs)))
                .into_any_element(),
            b => {
                let t = b.text().cloned().unwrap_or_else(|| RichText {
                    text: "".into(),
                    runs: vec![],
                    click_ranges: vec![],
                    click_targets: vec![],
                    emoji: vec![],
                });
                let el = embed_text(&t, size);
                match b {
                    RBlock::Heading(..) => el.font_weight(FontWeight::BOLD).text_size(z(size + 2.)),
                    RBlock::Subtext(_) => el.text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)),
                    RBlock::ListItem { .. } => div().flex().child(div().w(z(16.)).child("\u{2022}")).child(el),
                    _ => el,
                }
                .into_any_element()
            }
        });
    }
    d.into_any_element()
}

fn embed_text(t: &RichText, size: f32) -> gpui::Div {
    if t.emoji.is_empty() {
        return div().child(StyledText::new(t.text.clone()).with_runs(t.runs.clone()));
    }
    let mut row = div().flex().flex_wrap().items_center();
    let push = |mut row: gpui::Div, range: std::ops::Range<usize>| -> gpui::Div {
        let mut start = range.start;
        for (i, piece) in t.text[range.clone()].split('\n').enumerate() {
            let r = start..start + piece.len();
            if i > 0 {
                row = row.child(div().w_full().h_0());
            }
            for w in message_ext::words(&t.text, r.clone()) {
                let runs = message_ext::slice_runs(&t.runs, w.clone());
                let text: SharedString = t.text[w].to_owned().into();
                row = row.child(div().flex_none().max_w_full().child(StyledText::new(text).with_runs(runs)));
            }
            start = r.end + 1;
        }
        row
    };
    let mut last = 0;
    for (r, token) in &t.emoji {
        if r.start < last {
            continue;
        }
        row = push(row, last..r.start);
        row = row.child(div().mx(z(1.)).child(crate::media::emoji::emoji_element(token, size * 1.375)));
        last = r.end;
    }
    push(row, last..t.text.len())
}

pub(crate) fn embed(app: &PhoenixApp, e: &Embed, cx: &App) -> AnyElement {
    let has_text = e.title.is_some() || e.description.is_some() || e.author.is_some() || !e.fields.is_empty();
    let media_only = e.kind.as_deref() == Some("gifv") || (e.kind.as_deref() == Some("image") && !has_text);
    if media_only && let Some(m) = e.thumbnail.as_ref().or(e.image.as_ref()) {
        let max_h = if e.kind.as_deref() == Some("gifv") { 300. } else { media::MAX_H };
        let size = media::fit(m.width, m.height, media::MAX_W, max_h);
        let original = e.url.clone().unwrap_or_else(|| m.url.clone());
        let gifv = e.kind.as_deref() == Some("gifv");
        let animated = (gifv && media::gif_plays(&original))
            .then(|| e.video.iter().chain(e.thumbnail.iter()).find_map(|v| media::gifv_animated(&v.url, size.0)))
            .flatten();
        let Some(src) = animated else {
            let still = media_image(&m.url, original.clone(), "".into(), (m.width, m.height), size);
            return div().mt(z(4.)).child(media::gif_hover(original, still)).into_any_element();
        };
        let still = media::video_poster(&m.url, (size.0 * 2.) as u32, (size.1 * 2.) as u32);
        let key = original.clone();
        return div()
            .mt(z(4.))
            .child(media::gif_hover(key, media_image(&src, original, "".into(), (m.width, m.height), size).with_loading(move || {
                let mut im = img(still.clone());
                if let Some(cache) = media::lru::chat() {
                    im = im.image_cache(&cache);
                }
                im.size_full()
                    .rounded(z(8.))
                    .object_fit(gpui::ObjectFit::Cover)
                    .with_loading(|| div().size_full().rounded(z(8.)).bg(c(Palette::BG_SECONDARY)).into_any_element())
                    .into_any_element()
            })))
            .into_any_element();
    }
    let side_thumb = e.image.is_none() && !matches!(e.kind.as_deref(), Some("video" | "article"));
    let thumb = e.thumbnail.clone().filter(|_| side_thumb);
    let big = e.image.clone().or_else(|| e.thumbnail.clone().filter(|_| !side_thumb));
    let gallery: Vec<crate::model::Attachment> = e
        .images
        .iter()
        .map(|m| crate::model::Attachment {
            filename: "image.png".into(),
            url: m.url.clone(),
            width: m.width,
            height: m.height,
            is_image: true,
            kind: crate::model::AttachmentKind::Image,
            ..Default::default()
        })
        .collect();
    let big = big.filter(|_| gallery.len() < 2);

    let per_row = if thumb.is_some() { 2 } else { 3 };
    let mut rows: Vec<Vec<&crate::model::EmbedField>> = Vec::new();
    for f in &e.fields {
        match rows.last_mut() {
            Some(r) if f.inline && r.len() < per_row && r.iter().all(|x| x.inline) => r.push(f),
            _ => rows.push(vec![f]),
        }
    }
    let fields = (!rows.is_empty()).then(|| {
        let mut grid = div().flex().flex_col().gap(z(8.));
        for r in rows {
            let mut row = div().flex().gap(z(8.));
            for f in r {
                row = row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .mb(z(2.))
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(f.name.clone()),
                        )
                        .child(embed_markdown(app, &f.value, 14.)),
                );
            }
            grid = grid.child(row);
        }
        grid
    });

    let footer = (e.footer.is_some() || e.timestamp.is_some()).then(|| {
        let mut parts: Vec<String> = Vec::new();
        if let Some(f) = &e.footer
            && !f.text.is_empty()
        {
            parts.push(f.text.to_string());
        }
        if let Some(ts) = e.timestamp {
            parts.push(crate::app::format_time_full(ts).to_string());
        }
        div()
            .flex()
            .items_center()
            .gap(z(8.))
            .when_some(e.footer.as_ref().and_then(|f| f.icon.clone()), |d, u| {
                d.child(img(media::resized(&u, 40, 40)).size(z(20.)).rounded_full().flex_none())
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(SharedString::from(parts.join(" \u{2022} "))),
            )
    });

    let author = e.author.clone().map(|a| {
        let url = a.url.clone();
        div()
            .id(gpui::ElementId::Name(format!("embed-author-{}", a.name).into()))
            .flex()
            .items_center()
            .gap(z(8.))
            .when_some(a.icon.clone(), |d, u| {
                d.child(img(media::resized(&u, 48, 48)).size(z(24.)).rounded_full().flex_none())
            })
            .child(
                div()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tk(idx::TEXT_STRONG))
                    .when(url.is_some(), |d| d.hover(|s| s.underline()))
                    .child(a.name.clone()),
            )
            .when_some(url, |d, u| d.cursor_pointer().on_click(move |_, _, cx| cx.open_url(&u)))
    });

    let title = e.title.clone().map(|t| {
        let url = e.url.clone();
        div()
            .id(gpui::ElementId::Name(format!("embed-title-{t}").into()))
            .text_size(z(16.))
            .line_height(z(22.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if url.is_some() { tk(idx::TEXT_LINK) } else { tk(idx::TEXT_STRONG) })
            .when(url.is_some(), |d| d.hover(|s| s.underline()))
            .child(t)
            .when_some(url, |d, u| d.cursor_pointer().on_click(move |_, _, cx| cx.open_url(&u)))
    });

    div()
        .mt(z(4.))
        .max_w(z(516.))
        .flex()
        .rounded(z(4.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(div().w(z(4.)).flex_none().bg(e.color.map(c).unwrap_or_else(|| tk(idx::BORDER_NORMAL))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pt(z(2.))
                .pr(z(16.))
                .pb(z(16.))
                .pl(z(12.))
                .flex()
                .gap(z(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .pt(z(8.))
                        .when_some(e.provider.clone(), |d, p| {
                            d.child(div().text_size(z(12.)).line_height(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)).child(p))
                        })
                        .children(author)
                        .children(title)
                        .when_some(e.description.clone().filter(|_| !matches!(e.kind.as_deref(), Some("image" | "video" | "gifv"))), |d, t| d.child(embed_markdown(app, &t, 14.)))
                        .children(fields)
                        .when_some(big, |d, m| {
                            let size = media::fit(m.width, m.height, 400., 300.);
                            let original = m.url.clone();
                            let video = (e.kind.as_deref() == Some("video")).then(|| e.url.clone().unwrap_or_else(|| m.url.clone()));
                            d.child(
                                div()
                                    .mt(z(8.))
                                    .relative()
                                    .child(media_image(&m.url, original, "".into(), (m.width, m.height), size).rounded(z(4.)))
                                    .children(video.map(attach::video_embed_actions)),
                            )
                        })
                        .when(gallery.len() >= 2, |d| {
                            let refs: Vec<&crate::model::Attachment> = gallery.iter().collect();
                            d.child(div().mt(z(4.)).child(mosaic::render(&refs, cx)))
                        })
                        .children(footer),
                )
                .when_some(thumb, |d, m| {
                    let size = media::fit(m.width, m.height, 80., 80.);
                    let original = m.url.clone();
                    d.child(
                        div()
                            .flex_none()
                            .pt(z(8.))
                            .child(media_image(&m.url, original, "".into(), (m.width, m.height), size).rounded(z(4.))),
                    )
                }),
        )
        .into_any_element()
}

pub(crate) fn reply_preview_text(app: &PhoenixApp, content: &SharedString) -> SharedString {
    let m = crate::model::Message { content: content.clone(), ..Default::default() };
    app.render_msg(&m, None).preview
}

fn flash_overlay(id: Id) -> AnyElement {
    use gpui::AnimationExt;
    const IN: f32 = 600.;
    const OUT_AT: f32 = 2000.;
    const TOTAL: f32 = 2600.;
    let ease = crate::ui::states::cubic_bezier(0.25, 0.1, 0.25, 1.0);
    div()
        .absolute()
        .inset_0()
        .bg(tk(idx::MESSAGE_HIGHLIGHT_BACKGROUND_DEFAULT))
        .child(div().absolute().left_0().top_0().bottom_0().w(z(2.)).bg(tk(idx::TEXT_BRAND)))
        .with_animation(
            ("msg-flash", id.0),
            gpui::Animation::new(std::time::Duration::from_millis(TOTAL as u64)),
            move |d, t| {
                let ms = t * TOTAL;
                let o = if ms < IN {
                    let p = ms / IN;
                    let k = ((p % 0.4) / 0.2).min(2.);
                    let x = if k <= 1. { 1. - k } else { k - 1. };
                    let eio = crate::ui::states::cubic_bezier(0.42, 0., 0.58, 1.);
                    if p >= 0.8 { 1. } else { eio(x) }
                } else if ms < OUT_AT {
                    1.
                } else {
                    1. - ease((ms - OUT_AT) / (TOTAL - OUT_AT))
                };
                d.opacity(o)
            },
        )
        .into_any_element()
}
