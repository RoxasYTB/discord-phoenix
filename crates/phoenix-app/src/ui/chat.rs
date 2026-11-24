//! Chat area: header, virtualized message list, composer, typing indicator.

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

// Messaging features live in submodules (composer moved to `composer.rs`).
pub mod app_launcher;
pub mod attach;
pub mod composer;
pub mod icons;
pub mod mention;
pub mod forward;
pub mod message_ext;
pub mod mosaic;
pub mod panels;
pub mod pending;
pub mod scrollbar;
pub mod state;
pub mod thread_banner;
pub mod blocked;
pub mod slash;
pub mod spoiler;
pub mod system;
pub use state::ChatState;

/// Colour of a generated CSS token (`--name` → `idx::NAME`) for the active theme.
pub(crate) fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let entity = cx.entity();
    let show_members = app.show_members && app.selected_guild.is_some();
    let show_members = show_members && app.chat.thread.is_none() && !crate::ui::thread_create::is_open(cx);
    // The call view's side chat has no member list.
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
                        .children(discovery_notice(app, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .relative()
                                // scroller_ scrollbarGutterStable_: messageListItem_ is 579px in a
                                // 595px chat (vanilla-guild-text-groups), 16px reserved for the bar.
                                .pr(z(16.))
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
                // Search results take the right column (the panel itself is an overlay).
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

/// Guilds whose Discovery notice was closed this session (Discord keeps it client-side).
static NOTICE_CLOSED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

/// Discovery channel notice. vanilla-guild-text #1101 `bar_8PBDGr`: --background-feedback-mod-info,
/// border-bottom 1px --inlinenotice-border-info, margin-top -1, padding 4 8 4 16, gap 8;
/// #1103 text_ body-sm/normal 14/18 --text-default with an underlined "En savoir plus" (#1104);
/// #1105 close_ 32×32, --icon-muted (hover --icon-default), 18px glyph.
/// Window x of the "Mode Découverte" label centre (f32 bits), set while painting the header.
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

/// vanilla-guild-text #1102 `arrow_8PBDGr`: 16×10, fixed, centred under the header's
/// Discovery label (#1061, centre 666 vs arrow 657+8), top = bar top − 9 so its base
/// sits on the bar's 1px top border. Filled --background-base-lower then
/// --background-feedback-mod-info, edges stroked --inlinenotice-border-info.
/// Our own path (rounded apex), not the client's SVG.
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
    // It points at the header label, which locked channels don't show.
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

/// `--chat-input-icon-size` glyph in a 32px toolbar button.
const HDR_ICON: f32 = 20.;

/// --custom-message-margin-left-content-cozy = avatar + 2 × --space-md
/// (density-aware; cozy: dump -channels-N-N+ #1184 padding-left 80px).
/// `.a11y-font-scaled-down` (font under 16px) pins it to 4.5rem.
pub(crate) fn msg_left() -> f32 {
    if crate::theme::font_scaled_down() { 72. * crate::theme::font_scale() } else { m::AVATAR + 2. * space::md() }
}

/// `--chat-avatar-size` of a cozy message: 40px, 2.5rem under `.a11y-font-scaled-down`.
fn msg_avatar() -> f32 {
    if crate::theme::font_scaled_down() { 40. * crate::theme::font_scale() } else { m::AVATAR }
}

fn hdr_button(id: &'static str, i: Icon, color: gpui::Hsla, size: f32) -> gpui::Stateful<gpui::Div> {
    let _ = size;
    hdr(id, i.path(), color)
}

/// Pins button with the client's unread-pins badge (`badge: {type: "unread",
/// position: "bottom"}` while `ReadStateStore.hasUnreadPins`): `iconBadge_ybUK7Z`
/// 8×8, radius 8, `iconBadgeStyleUnread_` --background-brand, and the 20px icon
/// cut by `svg-mask-header-bar-badge-bottom` (circle cx=cy=.75, r=.25 → centre
/// 15,15 r 5 in the icon, read live over CDP). The icon sits at 6,6 in the 32px
/// box, so the cut is a 10px disc of the header background at 16,16 and the dot
/// is centred in it at 17,17.
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

/// Header toolbar button whose glyph comes from the runtime icon provider by name
/// (iconWrapper_ .clickable_: 32px box, 20px svg).
fn hdr_named(id: &'static str, name: &str, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    hdr(id, crate::icons::path(name), color)
}

/// `.selected_ybUK7Z` buttons are drawn `--icon-strong`; the tooltip is the button's aria-label.
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
        // aria-labels of vanilla-dm / vanilla-group-dm toolbars.
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
    /// Last painted border-box width of the channel header (`container_ybUK7Z`, the
    /// `container-type: inline-size` box the search width is resolved against).
    static HEADER_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

/// An invisible full-size child that records its parent's painted width (CSS px, page
/// zoom already divided out) into `cell`, and repaints once when it changed: the
/// client's container queries, resolved one frame late.
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

/// `search_bWCaDq` width: `clamp(64px, 100cqi - 320px, 244px)`, where 100cqi is the
/// header's content box (border box minus the 20px + 8px inline padding). Measured on the
/// live client (zoom 125 %, CDP viewport emulation): header 583 → 235, 543 → 195,
/// 483 → 135, 433 → 85, 383 → 64; see docs/parity/responsive.md.
fn header_search_w(pad_start: f32) -> f32 {
    let w = HEADER_W.with(std::cell::Cell::get);
    if w <= 0. { 244. } else { (w - pad_start - 8. - 320.).clamp(64., 244.) }
}

/// Channel / DM header. Dump -channels-N-N+ #1040 (guild) and -channels-me-N+ #1088 (DM):
/// section title_ padding 8px 8px 8px 20px, height 49px, border-bottom --border-subtle;
/// upperContainer_ → children_ (icon/avatar, title, dot, topic) + toolbar_ (gap 10px, padding-left 10px).
fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c));
    let is_dm = ch.is_some_and(|c| c.guild.is_none());
    let name = ch.map(|c| c.name.clone()).unwrap_or_default();
    let topic = ch.and_then(|c| c.topic.clone());
    // Forum channels: header has no topic and only bell + member list (vanilla-forum
    // section "En-tête du salon": h1, Paramètres de notification, Masquer la liste des membres).
    let is_forum = ch.is_some_and(|c| c.kind == ChannelKind::Forum);
    let topic = topic.filter(|_| !is_forum);
    // Thread: breadcrumb parent › thread, the icon is the parent's (vanilla-thread-channel #1190).
    let thread_parent = ch.and_then(|c| app.nav.thread_parent(c.id)).and_then(|p| app.channels.get(&p));
    let kind = thread_parent.or(ch).map(|c| c.kind).unwrap_or(ChannelKind::Text);
    // A private (locked) channel gets the *LockIcon glyph and no "Mode Découverte" label
    // (real compare 20261123-170026: locked #général shows hash-lock + dot, no label).
    let locked = ch.is_some_and(|c| app.nav.extras.get(&c.id).is_some_and(|e| e.locked));
    // Threads never show it either (vanilla-thread-channel, compare thread-more-1713).
    let discoverable = !is_dm && !locked && thread_parent.is_none() && guild_discoverable(app);
    // container_ .clickable_ .icon_ color: --icon-muted; .selected_: --icon-strong.
    let ic = tk(idx::ICON_MUTED);
    let on = tk(idx::ICON_STRONG);
    let is_group_dm = ch.is_some_and(|c| crate::ui::dm_profile_panel::is_group_dm(app, c.id));
    let peer = ch.filter(|c| !crate::ui::dm_profile_panel::is_group_dm(app, c.id)).and_then(|c| c.voice_users.first()).and_then(|u| app.users.get(u));
    let scope: SharedString = if is_dm {
        peer.map(|u| u.name.clone()).unwrap_or_else(|| name.clone())
    } else {
        app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone()).unwrap_or_default()
    };
    let search_ph = SharedString::from(format!("{} {}", crate::t!("search.placeholder"), scope));
    let pad_start = if is_group_dm { space::md() / 2. } else { space::md() };
    div()
        .relative()
        .child(width_probe(&HEADER_W))
        .h(z(m::HEADER_H + 1.))
        .flex_none()
        // padding-inline-start --custom-message-margin-horizontal (--space-md); a group DM's
        // section is half that (vanilla-group-dm #1059: 10px), its hoverable name box adds the rest.
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
            // children_: flex 1, items centred, overflow hidden (#1189 clips the Discovery label).
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .child(if let Some(gc) = ch.map(|c| c.id).filter(|_| is_group_dm) {
                    // vanilla-group-dm #1064 hoverableContainer_9yrrgG: h 32, radius 8, padding-inline and
                    // gap --custom-message-margin-horizontal / 2, hover --background-mod-subtle with a 16px
                    // pencil (editIcon_, display none until hover); #1065 gdmIcon_ 24px, margin-inline-end 4px;
                    // #1067 name body-md/medium 16/20 --text-strong.
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
                    // avatar_ 20px, margin-inline-end 8px (#1092).
                    div().mr(z(8.)).child(avatar(peer, 20. * crate::theme::zoom())).into_any_element()
                } else {
                    // channelIcon_ 20px glyph in a 20×32 box, margin 0 11px 0 -1px (#1043); --channel-icon.
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
                    // vanilla-thread-channel #1193/#1194: parent titleWrapper_ mr 8, h2 body-md/medium
                    // 16px, 24px tall, --text-default, nowrap, clickable; #1195 caret_ 10×24, mr 8, --icon-muted.
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
                    // titleWrapper_ margin-inline-end 8px; h1 body-md/medium 16px/20px --text-strong (#1047);
                    // thread title #1198 is flex 0 1 auto, overflow hidden, nowrap (clipped by children_).
                    // 1:1 DM: the recipient's name style font (vanilla-dm header `dnsFont_`).
                    crate::fonts::styled_name(
                        div(),
                        peer.filter(|_| is_dm),
                        false,
                    )
                        // Channel/DM titleWrapper_ is flex-shrink 0 (live, every width down to a
                        // 9px children_): it is clipped by children_, never ellipsized. Only a
                        // thread title shrinks.
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
                    // vanilla-guild-text #1060 dot_GxzgQj: 4px, --background-mod-strong, margin-inline 4 12;
                    // #1061/#1062 label_GxzgQj > body-sm/medium 14px/18px --text-muted "Mode Découverte".
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
                    // #1068 leadingBar_pVbL3l after the Discovery label: 1×16, --border-subtle, ml 8.
                    // Otherwise dot_ 4px, margin 0 4px, --text-muted 24% (#1049); topic_ margin-inline-start 8px,
                    // body-sm/medium 14px/18px --text-muted (#1050).
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
                                // Inline markdown stripped, mentions resolved and custom emoji
                                // drawn (a real topic began with a raw `<:icon_…:id>` token).
                                .child(crate::ui::emoji_text::label_with_custom(reply_preview_text(app, &t), 14.)),
                        )
                }),
        )
        .child(
            // toolbar_ybUK7Z: gap and padding-inline-start --space-xs (cozy 10, default 8, compact 6).
            div()
                .flex_none()
                .pl(z(space::xs()))
                .flex()
                .items_center()
                .gap(z(space::xs()))
                .when(is_dm, |d| {
                    // DM toolbar (#1101–#1109): voice call, video call, pins, add to DM, user profile.
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
                    // Guild toolbar (#1055–#1061): threads, notifications, pins, member list.
                    // Inside a thread the toolbar has no Threads button (vanilla-thread-channel #1208).
                    d.when(thread_parent.is_none() && !is_forum, |d| d.child(hdr_button("threads", Icon::Threads, ic, HDR_ICON).on_click(cx.listener(|a, _, _, cx| a.toggle_threads_popout(cx)))))
                        .map(|d| {
                            // Chat header `nH`: BellSlashIcon when `isChannelMuted ||
                            // resolvedMessageNotifications !== ALL_MESSAGES` (the level is
                            // inherited from the category, then the server). A muted
                            // thread too (compare thread-more-1713).
                            let slashed = ch.is_some_and(|c| {
                                c.muted
                                    || app.backend.notify_prefs().is_some_and(|p| {
                                        p.header_bell_slashed(c.guild.map(|g| g.0), c.id.0, c.parent.map(|p| p.0))
                                    })
                            });
                            if slashed {
                                d.child(hdr_named("bell", "bell-slash", ic))
                            } else {
                                d.child(hdr_button("bell", Icon::Bell, ic, HDR_ICON))
                            }
                        })
                        .when(!is_forum, |d| {
                            d.child(
                                pin_button(app, if app.chat.pins.is_some() { on } else { ic })
                                    .on_click(cx.listener(|a, _, _, cx| a.toggle_pins(cx))),
                            )
                        })
                        .child(
                            hdr_button("members", Icon::Members, if app.show_members { on } else { ic }, HDR_ICON).on_click(
                                cx.listener(|a, _, _, cx| {
                                    a.show_members = !a.show_members;
                                    cx.notify();
                                }),
                            ),
                        )
                        // Thread toolbar ends with "Plus" 32×32 (vanilla-thread-channel, x=982).
                        .when_some(thread_parent.and(ch.map(|c| c.id)), |d, t| {
                            d.child(hdr_named("thread-more", "more-horizontal", ic).on_click(move |ev, w, _| {
                                crate::ui::thread_menu::open(t, ev.position(), w)
                            }))
                        })
                })
                .child(
                    // search_ margin-inline-start 4px; searchBar_ 244×32, radius 8px,
                    // --input-background-default, 1px --input-border-default (#1063–#1073);
                    // editor_ padding 0 6px, 14px/22px medium; icon_ 24px box, padding 4px, 16px glyph.
                    div()
                        .ml(z(4.))
                        .flex_none()
                        .w(z(header_search_w(pad_start)))
                        // `.density-compact .searchBar_i16Ad0` is 28px tall.
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
                                .px(z(6.))
                                .text_size(z(14.))
                                .line_height(z(22.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(search_ph),
                        )
                        .child(div().size(z(24.)).p(z(4.)).flex().items_center().child(icon(Icon::Search, 16. * crate::theme::zoom(), tk(idx::ICON_DEFAULT)))),
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
            // expanded_cKojhe: the run sits on --message-background-hover.
            row_wrapper().bg(tk(idx::MESSAGE_BACKGROUND_HOVER)).children(bar).child(m).into_any_element()
        }
    };
    if i + 1 == entry.msgs.len() {
        // scrollerSpacer_8AE1Qq height 40px under the last message, overlapped 16px by
        // the form_ (vanilla-guild-text-groups #2777 at y609, form #2779 at y633): 24px
        // visible. Our list runs 9px under the composer, so pad 24 + 9 = 33px
        // (real compare msg-1 at dpr 1.5: last embed ended 18px lower than vanilla with 15px).
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
        // the recipient / group name is a `<strong>` inside the description (vanilla-group-dm #1112)
        let bold = desc.find(&*name).filter(|_| !name.is_empty()).map(|i| i..i + name.len());
        let desc_el = gpui::StyledText::new(desc.clone()).with_highlights(
            bold.map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })),
        );
        let e1 = entity.clone();
        let e2 = entity.clone();
        // `container_w6h4R2` (vanilla-group-dm #1103): column, margin 20.
        return div()
            .m(z(20.))
            .flex()
            .flex_col()
            // vanilla-group-dm #1104 editableGdmIcon_5yEg41 80px.
            .child(if group { crate::ui::group_dm::group_avatar(app, chan.id, 80., c(Palette::BG_PRIMARY)) } else { avatar(peer, 80.) })
            .child(
                // `header_w6h4R2 heading-xxl/semibold` (#1110): 32/40, --font-weight-bold,
                // margin 8 0, --text-strong.
                div()
                    .my(z(8.))
                    .text_size(z(32.))
                    .line_height(z(40.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(tk(idx::TEXT_STRONG))
                    .child(name.clone()),
            )
            .child(
                // `description_w6h4R2 body-md/normal` (#1111): 16/20 --text-default.
                // 1:1 (vanilla-dm-start #1139): first the username as `heading-xl/medium`
                // 24/32 500 --text-strong, margin-bottom 20; after the sentence the
                // `responsive_Vu5YPS` row (margin-top 8, gap 8).
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
                // `stack_ buttonContainer_KT4tG0` (#1113): margin-top 16, gap 8; md
                // primary "Invite to Group DM" (UserPlusIcon) and secondary "Edit Group".
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
                                .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.stub("Edit Group", cx))),
                        ),
                )
            });
    }
    // A thread opens on its own start (ThreadIcon, the thread name as the heading), not on
    // "Welcome to #thread name!" with the text channel #.
    let is_thread = app.nav.thread_parent(ch).is_some();
    // container_ (welcome) margin: --custom-message-margin-horizontal; header_ margin 8px 0.
    div()
        .m(z(16.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .mt(z(16.)) // emptyChannelIcon_ margin-top
                .size(z(68.)) // emptyChannelIcon_ 68px
                .rounded_full()
                .bg(tk(idx::BACKGROUND_MOD_MUTED)) // emptyChannelIcon_ background-color
                .flex()
                .items_center()
                .justify_center()
                // the channel's own type icon (voice text chat, announcements), not always #
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
                    .text_color(tk(idx::TEXT_DEFAULT)) // description_ color
                    .child(crate::t!("chat.channel_start", name = name)),
            )
        })
}

/// 1:1 DM intro row (vanilla-dm-start #1142..#1156): mutual servers
/// (`container_nM9YUb clickable_ divider_`: gap 8, 24px guild icon stack, `body-sm/normal`
/// `mutualGuildsLink_` --interactive-text-default → hover / active, then a 4px
/// --border-strong dot with 4px margins), then `stack_TWWX0S` gap 8 of sm secondary
/// buttons: Remove Friend (or Add Friend) and Block.
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
                // divider_nM9YUb:after
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
            .child(button_sm("dm-intro-friend", friend_label.clone(), BtnKind::Secondary).on_click(move |_, _, cx| {
                let l = friend_label.clone();
                e1.update(cx, |a, cx| a.stub(&l, cx))
            }))
            .child(
                button_sm("dm-intro-block", crate::t!("menu.block"), BtnKind::Secondary)
                    .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.stub("Block", cx))),
            ),
    )
}

fn day_divider(label: SharedString) -> gpui::Div {
    dated_divider(label, false)
}

/// Date divider; `unread` is the merged `.isUnread_` form (first unread message is the
/// first of its day): line and date in --divider-color (--background-feedback-notification)
/// plus the "NEW" end cap.
fn dated_divider(label: SharedString, unread: bool) -> gpui::Div {
    let line = if unread { idx::BACKGROUND_FEEDBACK_NOTIFICATION } else { idx::BORDER_SUBTLE };
    // divider_ (message list) margin-inline 1rem .875rem; .group-spacing-16
    // .divider_.hasContent_ margin 1.5rem top / .5rem bottom; divider_ border-top
    // --border-subtle; content_ 12px semibold, line-height 13px, padding 2px 4px,
    // radius 8px, --text-muted on --background-base-lower.
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
                .mt(z(-1.)) // content_4TfzT5 margin-top -1px
                .px(z(4.))
                .py(z(2.))
                .text_size(z(12.))
                .line_height(z(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(if unread { idx::BACKGROUND_FEEDBACK_NOTIFICATION } else { idx::TEXT_MUTED }))
                .child(label),
        )
}

/// "Monday, November 23, 2026 at 3:04 PM" (timestamp tooltip).
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

/// Server tag chiplet after a username: clanTagChiplet_ (--background-mod-strong, radius 4px,
/// padding 0 4px, .72rem, margin-inline-start 6px; 2px in reply lines) holding
/// chipletContainerInner_ (line-height 16px) with the guild badge and tag text.
pub(crate) fn server_tag_chip(tag: &(SharedString, Option<SharedString>), in_reply: bool) -> gpui::Div {
    div()
        .flex_none()
        // vanilla-dm #1270 (message): margin -2 0 0 6, box h 15; #1424 (reply): margin -2 4 0 2, opacity .64.
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
        .when_some(tag.1.clone(), |d, u| d.child(img(u).size(z(if in_reply { 12. } else { 14. })).flex_none())) // #1192 12px / #1208 14px
        .child(tag.0.clone())
}

/// roleIcon_2i8Bfm: 1rem + 4px square, margin-inline-start .25rem, top 1px (centred in the
/// 22px header row); after the
/// server tag (vanilla-inbox-1 headerText_ hasRoleIcon_).
pub(crate) fn role_icon(url: SharedString) -> gpui::Img {
    img(url).flex_none().size(z(20.)).ml(z(4.)).object_fit(gpui::ObjectFit::Contain)
}

/// "New member" clover after the author name: newMemberBadge_ (calc(1rem + 4px) square,
/// margin-inline-start .25rem, top 1px, color --text-feedback-positive) holding the
/// NewUserIcon at 20px; tooltip "New member" (client module HU decides who gets it).
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

/// Discord's "APP" tag next to bot authors (vanilla-guild-text-groups botTagCozy_ /
/// botTagVerified_ / botText_: 15px tall, 46px wide for a verified "APP").
pub(crate) fn bot_tag(verified: bool) -> gpui::Div {
    div()
        .flex_none()
        .ml(z(4.)) // botTagCozy_ margin-inline-start .25rem
        .mt(z(1.6)) // botTagCozy_ position relative, top .1rem
        .px(z(4.4)) // .rem_.botTag_ padding 0 .275rem
        .h(z(15.)) // .rem_.botTag_ height .9375rem
        .rounded(z(4.)) // .rem_.botTag_ border-radius 4px
        .bg(tk(idx::BACKGROUND_BRAND)) // botTagRegular_
        .flex()
        .items_center()
        .text_size(z(12.8)) // botText_ font-size .8rem
        .line_height(z(15.))
        .font_weight(FontWeight::SEMIBOLD) // botText_ 600
        .text_color(gpui::white())
        // botTagVerified_ 16px, margin -.02rem 0 0 -.2rem, fill --white
        .when(verified, |d| d.child(crate::ui::members::verified_check().mt(z(-0.32)).ml(z(-3.2))))
        .child(crate::t!("common.app"))
}

fn message(
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
    // username_ color: role colour, else --text-strong.
    let color = app.author_color(msg.author).map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG));
    let id = msg.id;
    if let Some(sys) = system::render(app, entity, msg, r) {
        return row_wrapper().child(sys).into_any_element();
    }
    let compact = app.prefs.synced.compact;
    let editing = app.chat.edit.as_ref().is_some_and(|e| e.id == id);
    let replying = app.replying_to == Some(id);
    // mentioned_: content mentions, @everyone, or a pinging reply to one of my messages.
    let reply_pings_me = msg.reply_mentions_author
        && msg.reply_to.and_then(|rid| entry.index_of(rid)).is_some_and(|p| entry.msgs[p].author == app.me.id);
    let mentioned = r.mentions_me || msg.mention_everyone || reply_pings_me;
    let ts = msg.timestamp;

    let mut content = div().flex().flex_col().min_w_0().text_size(fz(16.)).text_color(c(Palette::TEXT_NORMAL));
    let last_text = r.blocks.iter().rposition(|b| matches!(b, RBlock::Text(_)));
    let n_blocks = r.blocks.len();
    // Discord exposes `display_compact_avatars` only as an option for compact
    // message display. Cozy mode always shows its normal group avatar.
    let compact_avatars = compact && app.prefs.synced.flag("avatars");
    // Compact first line starts 1rem in (contents_ text-indent -(5rem - 1rem)).
    let compact_header_left = if compact_avatars { 64. } else { 16. * crate::theme::font_scale() };
    // Compact: the absolute "[time] Name" header overlaps the first line by
    // `overflow` px; pad the first text line with thin spaces to clear it.
    let is_bot = author.is_some_and(|a| a.bot);
    let overflow = if compact {
        compact_header_overflow(
            &r.time_short,
            &name,
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
    // A message that is only a gifv link (Tenor/Giphy) shows just the gif: the link
    // text is hidden (vanilla #discussion, real compare 20261123-161135).
    // Text & images settings (settings-proto text_and_images): render_embeds,
    // inline_embed_media (image/gifv/video embeds) and render_reactions.
    let show_embed = |e: &&crate::model::Embed| {
        let sy = &app.prefs.synced;
        let media = matches!(e.kind.as_deref(), Some("image" | "gifv" | "video"));
        sy.flag("embeds") && (!media || sy.flag("media-links"))
    };
    let gif_only = matches!(msg.embeds.as_slice(), [e] if e.kind.as_deref() == Some("gifv") && e.url.as_deref() == Some(msg.content.trim()) && show_embed(&e));
    for (bi, b) in r.blocks.iter().enumerate().filter(|_| !gif_only) {
        let edited_here = msg.edited && last_text == Some(bi) && bi + 1 == n_blocks;
        match b {
            RBlock::Text(t) if compact && bi == 0 => {
                content = content.child(rich(entity, id, bi, &indent_rich(t, overflow, cx), edited_here));
            }
            _ => content = content.child(block(entity, id, bi, b, edited_here)),
        }
    }
    // A message with no text keeps its `edited_` span after the embeds
    // (vanilla-guild-chat-busy: honeypot embed, `timestamp_ > edited_` below the embed).
    let edited_after_embeds = msg.edited && last_text.is_none();
    if msg.edited && !edited_after_embeds && !last_text.is_some_and(|l| l + 1 == n_blocks) {
        content = content.child(edited_marker(ts));
    }
    media::set_current_author(Some(media::ViewerAuthor {
        user: author.cloned(),
        name: name.clone(),
        color,
        time: r.time_full.clone(),
    }));
    // Two or more images/videos share one mosaic; files follow (nonVisualMediaItem_ margin-top 8px).
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
    // Forward: the snapshot sits inside the quote frame; reactions/threads stay outside.
    if let Some(f) = &msg.forward {
        content = div().flex().flex_col().min_w_0().text_size(fz(16.)).text_color(c(Palette::TEXT_NORMAL)).child(forward::wrap(app, f, content));
    }
    content = content.children(crate::ui::components::render(app, entity, msg, cx)).children(crate::ui::components::render_poll(app, entity, msg));
    if edited_after_embeds {
        // accessories container_ row gap 4px, then timestamp_ 20px (dump: embed ends 219, timestamp_ 223).
        content = content.child(div().mt(z(4.)).h(z(20.)).flex().items_center().child(edited_marker(ts)));
    }
    if !msg.reactions.is_empty() && app.prefs.synced.flag("reactions") {
        content = content.child(message_ext::reactions(app, entity, msg));
    }
    if let Some(t) = &msg.thread {
        content = content.child(message_ext::thread_chip(app, entity, t));
    }
    // Ephemeral (local system reply to a failed send): "Only you can see this — Dismiss message".
    if msg.flags & crate::model::FLAG_EPHEMERAL != 0 {
        content = content.child(crate::ui::chat::pending::ephemeral_footer(entity, msg));
    }
    let content = match message_ext::edit_box(app, entity, msg) {
        Some(b) => b,
        None => content,
    };
    // messageContent_.isSending_: the optimistic copy is drawn at 50 % opacity, header and
    // avatar untouched (live client, 20261124 pending test). Failed sends are coloured in render_msg.
    let content = if msg.send_state == crate::model::SendState::Sending { content.opacity(0.5) } else { content };
    // Upload in flight: the file card with its progress bar sits under the dimmed text.
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
                // referenced_message has no `member`: reuse the nick from any loaded message by that author.
                let a = entry.msgs[p].author;
                entry.msgs.iter().rev().filter(|m| m.author == a).find_map(|m| m.author_nick.clone())
            })),
            // Not in the loaded history: the reply's own `referenced_message`.
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
        let found = found.or_else(|| msg.reply_preview.as_ref().map(|_| usize::MAX));
        // repliedMessage_: 14px, line-height --custom-message-reply-message-preview-line-height
        // (1.125rem), cozy margin-bottom --reply-spacing (4px). Compact: margin-inline-start
        // timestamp-width + meta-space - (compact-indent - 1rem) = -24px, padding-inline-start
        // --custom-message-reply-indent (.625rem).
        div()
            .relative()
            .h(fz(if compact { 20. } else { 18. })) // cozy: vanilla-guild-text-groups #2624 repliedMessage_ 18px (line-height 1.125rem); compact #1185 20px
            .when(!compact, |d| d.mb(z(4.)))
            .when(compact, |d| d.ml(z(-24.)).pl(z(10.)))
            .flex()
            .items_center()
            .gap(z(4.)) // replyAvatar_ margin-inline-end --custom-message-meta-space
            .text_size(fz(14.))
            .line_height(fz(18.))
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(
                // messageSpine_:before — 2px --spine-default, radius 6px, top 50%, bottom 0,
                // from the avatar centre (avatar/2 + gutter, minus half the spine) to 4px
                // (--reply-spacing) before the reply. Compact: avatar = 2.25rem timestamp,
                // gutter = meta-space, ends at the reply indent.
                div()
                    .absolute()
                    .left(z(if compact { -23. } else { -41. })) // #1186 left -40px, margin-left -1px
                    .top(z(9.)) // #1186 top 10px, margin-top -1px
                    .w(z(if compact { 29. } else { 37. })) // #1186 37px
                    .h(z(if compact { 11. } else { 13. }))
                    .border_l_2()
                    .border_t_2()
                    .rounded_tl(z(6.))
                    .border_color(tk(idx::SPINE_DEFAULT))
                    .hover(|s| s.border_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))), // repliedMessageClickableSpine_:hover
            )
            .when(!compact, |d| d.child(avatar(ru.as_deref(), 16.)))
            // vanilla-guild-text #1283: `botTagCompact_` sits before the replied username.
            .when(ru.as_deref().is_some_and(|u| u.bot), |d| d.child(bot_tag(ru.as_deref().is_some_and(|u| u.verified_bot)).ml_0()))
            .child(
                // repliedMessage_ username: like the message header, a display name
                // style switches the font (live client) while the color stays the role's.
                crate::fonts::styled_name(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ru.as_deref().and_then(|u| app.author_color(u.id)).map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG)))
                        .opacity(0.64),
                    ru.as_deref(),
                    false,
                )
                .child(crate::fonts::name_text(
                    SharedString::from(format!(
                        "{}{}",
                        if msg.reply_mentions_author { "@" } else { "" }, // "@" only when the reply pinged
                        rnick.map(|n| n.to_string()).or_else(|| ru.as_deref().map(|u| u.display_name.to_string())).unwrap_or_else(|| crate::t!("common.unknown").to_string())
                    )),
                    ru.as_deref(),
                    14.,
                )),
            )
            .when_some(ru.and_then(|u| u.tag.clone()), |d, tg| d.child(server_tag_chip(&tg, true).opacity(0.64)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(tk(idx::TEXT_DEFAULT)) // repliedTextContent_ color: inherit
                    .hover(|s| s.text_color(tk(idx::TEXT_STRONG))) // repliedTextPreview_.clickable_:hover
                    .id(("reply-jump", msg.id.0))
                    .cursor_pointer()
                    .on_click({
                        let (e, ch) = (entity.clone(), msg.channel);
                        move |_, _, cx| e.update(cx, |a, cx| a.jump_to_message(ch, rid, cx))
                    })
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    // #1291 `repliedTextPlaceholder_`: no text content (embeds, attachments only).
                    .when(found.is_none() || preview.is_empty(), |d| d.italic())
                    .map(|d| {
                        let text = if found.is_some() && preview.is_empty() { crate::t!("chat.reply_click_to_see") } else { preview };
                        // Mentions keep the message mention colours (repliedTextContent_ .mention).
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
                        // repliedTextContent_ custom emoji: img.emoji 18×18 inline (vanilla-guild-text-groups).
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
        .child(crate::ui::emoji_text::label(name, 16.))
        // roleDot_: 8px circle in the role colour, 4px after the name.
        .children(role_dot.map(|col| div().flex_none().ml(z(4.)).size(z(8.)).rounded_full().bg(c(col))));

    let base = div()
        .id(("msg", id.0))
        .group("msg")
        // Author decorations play while the message is hovered (ui::avatar_deco).
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
        .pr(z(space::xl())) // message_ padding-inline-end --space-xl (#1184)
        .map(|d| {
            // message_:hover / .selected_ --message-background-hover; .mentioned_ and
            // .replying_ have their own backgrounds, hover shades and a 2px bar
            // (--icon-feedback-warning / --brand-500) drawn over the row.
            let ephemeral = msg.flags & crate::model::FLAG_EPHEMERAL != 0;
            let (bg, hover, bar) = if ephemeral {
                // ephemeral_: --brand-05a background with the 2px --brand-500 bar (live Clyde reply).
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
        // Compact (Discord's IRC-like layout): "[time] Name" sits inline at the
        // start of the first line and wrapped lines are indented to 80 px,
        // like the web client's negative text-indent.
        // compact_ wrapper: padding-inline-start --custom-message-margin-compact-indent (5rem),
        // padding-block --custom-message-padding-vertical-container-compact (.125rem).
        base.pl(fz(80.))
            .py(fz(2.))
            .when_some(reply, |d, reply| d.child(reply))
            .when(compact_avatars, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(z(16.))
                        .top(fz(if msg.reply_to.is_some() { 24. } else { 2. }))
                        .id(("msg-avatar-compact", id.0))
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
                        .child(avatar(author, m::AVATAR))
                        .when_some(author.and_then(|u| u.decoration.clone()), |d, deco| {
                            let size = m::AVATAR * 1.2;
                            let offset = (size - m::AVATAR) / 2.;
                            d.child(
                                gpui::img(crate::ui::avatar_deco::src_for(&deco, crate::ui::avatar_deco::hover_key(2, id.0)))
                                    .id("deco-frames")
                                    .absolute()
                                    .size(z(size))
                                    .left(z(-offset))
                                    .top(z(-offset)),
                            )
                        }),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(z(compact_header_left))
                    .top(fz(if msg.reply_to.is_some() { 24. } else { 2. }))
                    .h(fz(22.))
                    .flex()
                    .items_center()
                    .child(
                        // .compact_ .timestamp_: .6875rem, --text-muted, margin-inline-end
                        // --custom-message-meta-space.
                        div()
                            .id(("msg-time", id.0))
                            .flex_none()
                            .mr(fz(4.))
                            .text_size(fz(11.))
                            .text_color(tk(idx::TEXT_MUTED))
                            .tooltip(time_tooltip(ts))
                            .child(r.time_short.clone()),
                    )
                    .child(author_name)
                    .when_some(author.and_then(|u| u.tag.clone()), |d, tg| d.child(server_tag_chip(&tg, false)))
                    .when_some(app.author_role_icon(msg.author), |d, u| d.child(role_icon(u)))
                    .when(app.is_new_member(msg.author), |d| d.child(new_member_badge(msg.id)))
                    .when(is_bot, |d| d.child(bot_tag(author.is_some_and(|u| u.verified_bot))))
                    .when(crate::ui::forum::is_op(app, msg.channel, msg.author), |d| d.child(crate::ui::forum::op_tag())),
            )
            .child(content)
    } else {
        base.pl(z(msg_left())) // --custom-message-margin-left-content-cozy (#1184 80px)
            .py(fz(2.)) // --custom-message-spacing-vertical-container-cozy (.125rem)
            .when_some(reply, |d, reply| d.child(reply))
            .when(r.group_start, |d| {
                d.child(
                    div()
                        .absolute()
                        // avatar_: inset-inline-start --custom-message-margin-horizontal,
                        // margin-top 4px - cozy vertical padding (so 4px below the row top,
                        // after the reply line when there is one).
                        // Under .a11y-font-scaled-down: inset 1rem, margin-top .25rem - .125rem.
                        .left(if crate::theme::font_scaled_down() { fz(16.) } else { z(space::md()) }) // #1200 left 20px
                        // #1200: .125rem row padding + 1.125rem reply + 4px --reply-spacing + (4px - .125rem)
                        // margin-top; #2640: 2 + 18 + 4 + 2 at 16px.
                        .top({
                            let pad = if crate::theme::font_scaled_down() { 4. * crate::theme::font_scale() } else { 4. };
                            z(if msg.reply_to.is_some() { 18. * crate::theme::font_scale() + 4. + pad } else { pad })
                        })
                        .id(("msg-avatar", id.0))
                        .cursor_pointer()
                        .rounded_full()
                        // .full-motion avatar_2i8Bfm.clickable_:hover drop-shadow(--elevation-medium: 0 4px 4px --opacity-black-16)
                        .hover(|s| {
                            s.shadow(vec![gpui::BoxShadow {
                                color: gpui::hsla(0., 0., 0., 0.16),
                                offset: gpui::point(z(0.), z(4.)),
                                blur_radius: z(4.),
                                spread_radius: z(0.),
                                inset: false,
                            }])
                        })
                        // avatar_2i8Bfm.clickable_:active translateY(1px)
                        .active(|s| s.mt(z(1.)))
                        .on_click(message_ext::open_author(entity, msg))
                        .child(avatar(author, msg_avatar()))
                        .when_some(author.and_then(|u| u.decoration.clone()), |d, deco| {
                            // avatarDecoration_2i8Bfm: --custom-message-avatar-decoration-size = avatar × 1.2
                            // (--decoration-to-avatar-ratio), centred on the avatar (−(deco−avatar)/2),
                            // z-index 1, no pointer events. Drawn inside the avatar box so the
                            // :active translateY(1px) moves it too, like the client.
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
                            // .cozy_ .timestamp_: .75rem, --chat-text-muted, medium;
                            // timestampInline_ margin-inline-start .25rem.
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
                        // .cozy_ .timestamp_.alt_: .6875rem, --text-muted, width 56px
                        // (3.5rem under .a11y-font-scaled-down).
                        .text_size(fz(11.))
                        .text_color(gpui::transparent_black())
                        .group_hover("msg", |s| s.text_color(tk(idx::TEXT_MUTED)))
                        .tooltip(time_tooltip(ts))
                        .child(r.time_short.clone()),
                )
            })
            .child(content)
    }
    .child(message_ext::toolbar(app, entity, msg, app.chat.shift_held));
    let new_marker = app.chat.unread_marker.is_some_and(|(_, m)| m == id);
    // The list measures each row with `layout_as_root`, which ignores the
    // root's own margins: group spacing must be padding on a wrapper or rows
    // get painted over each other.
    // groupStart_ margin-top --custom-group-spacing-start (1.0625rem with group-spacing-16).
    let gap = if !r.group_start { 0. } else if compact { 4. } else { 17. * crate::theme::font_scale() };
    row_wrapper()
        .when_some(r.day_divider.clone(), |d, label| d.child(dated_divider(label, new_marker)))
        .when(new_marker && r.day_divider.is_none(), |d| d.child(message_ext::new_divider(r.group_start)))
        .child(div().pt(z(gap)).child(body))
        .into_any_element()
}

/// Root of a list row: a flex column so children's margins stay inside the
/// measured box (no margin collapsing through the root).
fn row_wrapper() -> gpui::Div {
    div().w_full().flex().flex_col()
}

/// Width in px of `s` drawn in `font` at `size` (no kerning; close enough
/// for layout offsets).
fn text_width(cx: &App, font: &gpui::Font, size: gpui::Pixels, s: &str) -> f32 {
    let ts = cx.text_system();
    let id = ts.resolve_font(font);
    s.chars().map(|ch| ts.advance(id, size, ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum()
}

/// How far the compact "[time] Name " header runs past the 80 px content
/// indent, i.e. the first-line text indent.
fn compact_header_overflow(time: &str, name: &str, bot: bool, tag: Option<&str>, left: f32, cx: &App) -> f32 {
    let mut name_font = crate::richtext::ui_font();
    name_font.weight = FontWeight::MEDIUM;
    let mut w = text_width(cx, &crate::richtext::ui_font(), fz(11.), time) + f32::from(fz(4.));
    w += text_width(cx, &name_font, fz(16.), name);
    if bot {
        let mut f = name_font.clone();
        f.weight = FontWeight::SEMIBOLD;
        w += text_width(cx, &f, z(12.8), "APP") + f32::from(z(12.8));
    }
    if let Some(tg) = tag {
        let mut f = name_font.clone();
        f.weight = FontWeight::SEMIBOLD;
        w += text_width(cx, &f, fz(11.52), tg) + f32::from(z(14.)); // ml 6 + px 4*2
    }
    w += f32::from(z(4.));
    (f32::from(z(left)) + w - f32::from(fz(80.))).max(0.)
}

/// Prefix `t` with transparent thin spaces at least `width` px wide.
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
        .text_size(z(10.)) // edited_ font-size .625rem
        .line_height(z(10.)) // edited_ line-height 1
        .font_weight(FontWeight::NORMAL) // edited_ --font-weight-normal
        .text_color(tk(idx::TEXT_MUTED))
        .child(crate::t!("chat.edited"))
}

/// Append Discord's small muted " (edited)" after the last line of text.
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

fn rich(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, t: &RichText, edited: bool) -> AnyElement {
    if !t.emoji.is_empty() {
        let el = message_ext::rich_with_emoji(entity, msg, bi, t);
        if edited {
            // max-width 100%: as a bare flex item the text row took its unwrapped width and
            // ran past the column (edited announcements with a #channel mention).
            return div()
                .flex()
                .flex_wrap()
                .items_end()
                .child(div().max_w_full().child(el))
                .child(div().ml(z(4.)).mb(z(5.)).child(edited_marker(msg.0 as i64)))
                .into_any_element();
        }
        return el;
    }
    // StyledText runs cannot change size, so a short one-line message gets the real 10px
    // `edited_` span next to it (baseline-aligned: 22px line, 16px text); longer text keeps the
    // inline suffix so it follows the last wrapped line.
    if edited && t.click_ranges.is_empty() && !t.text.contains('\n') && t.text.chars().count() <= 60 {
        let plain = div().line_height(gpui::relative(1.375)).child(StyledText::new(t.text.clone()).with_runs(t.runs.clone()));
        return div()
            .flex()
            .flex_wrap()
            .items_end()
            .child(plain)
            .child(div().ml(z(4.)).mb(z(5.)).child(edited_marker(msg.0 as i64))) // " " text node + baseline
            .into_any_element();
    }
    let (text, runs) = if edited { with_edited(t) } else { (t.text.clone(), t.runs.clone()) };
    let runs = mention::hovered_runs(msg, bi, 0, &t.click_ranges, &t.click_targets, &runs).unwrap_or(runs);
    let styled = StyledText::new(text).with_runs(runs);
    let lh = gpui::relative(1.375);
    if t.click_ranges.is_empty() {
        return div().line_height(lh).child(styled).into_any_element();
    }
    let targets = t.click_targets.clone();
    let e = entity.clone();
    div()
        .line_height(lh)
        // Right-click on a link: the message menu (opened by the row) gets the link rows.
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
                .on_click(
                t.click_ranges.clone(),
                move |ix, w, cx| match &targets[ix] {
                    ClickTarget::Link(url) => cx.open_url(url),
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

fn block(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, b: &RBlock, edited: bool) -> AnyElement {
    match b {
        RBlock::Text(t) => rich(entity, msg, bi, t, edited),
        // blockquoteContainer_: margin-block --space-4; blockquoteDivider_ 4px, radius 4px,
        // --spine-default; blockquote padding-inline 12px 8px, max-width 90%.
        RBlock::Quote(t) => div()
            .flex()
            .my(z(4.))
            .max_w(gpui::relative(0.9))
            .child(div().w(z(4.)).rounded(z(4.)).bg(tk(idx::SPINE_DEFAULT)).flex_none())
            .child(div().flex_1().min_w_0().pl(z(12.)).pr(z(8.)).child(rich(entity, msg, bi, t, false)))
            .into_any_element(),
        RBlock::Heading(level, t) => {
            let size = match level {
                1 => 24.,
                2 => 20.,
                _ => 16.,
            };
            // markup_ h1 1.5rem / h2 1.25rem / h3 1rem, margin 16px 0 8px; first child
            // margin-top 8px (h1, h2) or 4px (h3).
            let first = if *level <= 2 { 8. } else { 4. };
            div()
                .mt(z(if bi == 0 { first } else { 16. }))
                .mb(z(8.))
                .text_size(fz(size))
                .child(rich(entity, msg, bi, t, edited))
                .into_any_element()
        }
        // markup_ small: .875rem, line-height 1.20313rem, --text-subtle.
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
        // markup_ pre: margin-top 6px, radius 4px, max-width 90%; markup_ code: .875rem /
        // 1.125rem, 1px --border-normal; .hljs: padding .5em, --background-base-lower,
        // --text-code.
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
            // codeContainer_: relative, shows codeActions_ (copy button) on hover
            .relative()
            .group(crate::ui::code_copy::GROUP)
            .child(StyledText::new(code.clone()).with_runs(runs.clone()))
            .child(crate::ui::code_copy::CodeCopy::new(code.clone()))
            .into_any_element(),
    }
}

/// Clickable image (opens the viewer). `w x h` is the drawn size; the proxy
/// is asked for 2x for HiDPI.
fn media_image(url: &str, original: SharedString, filename: SharedString, size_o: (u32, u32), size: (f32, f32)) -> gpui::Stateful<gpui::Img> {
    media_image_with(url, original, filename, size_o, size, false)
}

/// [`media_image`]; `still` asks the proxy for the first frame (`format=webp`).
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
    // Bounded LRU instead of the window's keep-forever asset cache.
    if let Some(cache) = media::lru::chat() {
        im = im.image_cache(&cache);
    }
    // visualMediaItemContainer_ width 100% / max-width 550px: media shrinks with the message column, keeping its ratio.
    im.id(gpui::ElementId::Name(src))
        .w(z(w))
        .max_w_full()
        .aspect_ratio(w / h.max(1.))
        .rounded(z(8.))
        // Unknown size: the box is a guess, so show the whole image rather than crop it.
        .object_fit(if ow == 0 || oh == 0 { gpui::ObjectFit::Contain } else { gpui::ObjectFit::Cover })
        .cursor_pointer()
        // `imageContainer_` --background-mod-normal while loading (base-lower is the chat
        // background itself, so the slot looked empty); same box when the image fails.
        .with_loading(|| div().size_full().rounded(z(8.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).into_any_element())
        .with_fallback(|| div().size_full().rounded(z(8.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).into_any_element())
        .on_click(move |_, _, cx| media::open_viewer(viewer.clone(), cx))
}

/// Spoiler cover: blurred (tiny upscaled proxy image) + dark veil + pill.
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
            // `spoilerContainer_0_7_Rr.attachmentContainer_` radius-sm 8; hidden bg
            // `--__spoiler-background-color--hidden`.
            .bg(tk(idx::SPOILER_BACKGROUND_COLOR_HIDDEN))
            .group("spoiler-cover")
            // `hiddenSpoiler_` blur(--custom-message-attachment-spoiler-blur-radius 44px):
            // approximated by a 1/24 thumbnail upscaled; no dark overlay in the client.
            .child(img(tiny).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
            .child(
                div().absolute().inset_0().flex().items_center().justify_center().child(
                    // `spoilerWarning_0_7_Rr`: radius 20, 15px semibold uppercase,
                    // bg --__spoiler-warning-background-color (black-60, hover black-88),
                    // text --__spoiler-warning-text-color (primary-200, hover white).
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

/// Accessibility "Sticker animations" other than Always: the media proxy's
/// first frame (`passthrough=false`) instead of the animated file.
fn still_sticker(u: SharedString, fmt: media::StickerFormat) -> SharedString {
    let animated = matches!(fmt, media::StickerFormat::Apng | media::StickerFormat::Gif);
    if !animated || crate::ui::settings::effects::sticker_autoplay() || !u.contains("media.discordapp.net") {
        return u;
    }
    // APNG stickers play as `.webp?…&animated=true`: the still is the same URL without it.
    if u.contains("&animated=true") {
        return u.replace("&animated=true", "").into();
    }
    let sep = if u.contains('?') { '&' } else { '?' };
    format!("{u}{sep}passthrough=false").into()
}

pub(crate) fn attachment(a: &Attachment, cx: &App) -> AnyElement {
    let remote = attach::loadable(&a.url);
    let spoiler = a.is_spoiler();
    // "Display images, videos and lolcats: when uploaded directly" (inline_attachment_media).
    let inline = crate::ui::settings::effects::inline_attachments();
    match a.kind {
        AttachmentKind::Sticker(media::StickerFormat::Lottie) if !a.url.is_empty() => {
            let name = a.filename.clone();
            // 160px sticker, rasterized at 200 device px (scale 1.25) and played in a loop.
            return media::lottie::image(a.url.clone(), 200, false)
                .id(gpui::ElementId::Name(a.url.clone()))
                .mt(z(4.))
                .size(z(160.))
                .object_fit(gpui::ObjectFit::Contain)
                .with_fallback(move || sticker_name(name.clone()))
                .into_any_element();
        }
        AttachmentKind::Sticker(fmt) => {
            let name = a.filename.clone();
            return match media::sticker_url(0, fmt).and(Some(a.url.clone())).filter(|u| !u.is_empty()) {
                Some(u) => img(still_sticker(u.clone(), fmt))
                    .id(media::anim_id(&u))
                    .mt(z(4.))
                    .size(z(160.))
                    .object_fit(gpui::ObjectFit::Contain)
                    .with_fallback(move || sticker_name(name.clone()))
                    .into_any_element(),
                None => div().mt(z(4.)).child(sticker_name(name)).into_any_element(),
            };
        }
        AttachmentKind::Image | AttachmentKind::Video if remote && inline => {
            let size = media::fit(a.width, a.height, media::MAX_W, media::MAX_H);
            if spoiler && let Some(cover) = spoiler_cover(a.url.clone(), &a.url, size, cx) {
                return div().mt(z(4.)).child(cover).into_any_element();
            }
            if a.kind == AttachmentKind::Video {
                return div().mt(z(4.)).child(attach::video_card(a, size, cx)).into_any_element();
            }
            let link = if a.original.is_empty() { a.url.clone() } else { a.original.clone() };
            return div()
                .mt(z(4.))
                // Right-click on the image adds the image rows to the message menu.
                .on_mouse_down(gpui::MouseButton::Right, move |_, _, _| panels::set_menu_media(link.clone()))
                // GIF uploads follow "Play GIFs when Discord is focused": off = first frame, played on hover.
                .child({
                    let gif = a.filename.to_ascii_lowercase().ends_with(".gif");
                    let still = gif && !media::gif_plays(&a.url);
                    let im = media_image_with(&a.url, a.original.clone(), a.filename.clone(), (a.width, a.height), size, still);
                    if gif { media::gif_hover(a.url.clone(), im).into_any_element() } else { im.into_any_element() }
                })
                .into_any_element();
        }
        AttachmentKind::Audio => return div().mt(z(4.)).child(attach::audio_card(a)).into_any_element(),
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

/// Embed description / field values: Discord renders them as markdown.
/// Parsed embed markdown, keyed by source text and style generation, so
/// visible embeds are not re-parsed on every frame. Cleared when full.
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
        // Channel / role / user mentions resolved like message text (bots' embeds showed
        // "#unknown" and "@unknown-role" for channels and roles of the server).
        let parsed = crate::markdown::parse(src, &app.resolver());
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

/// Embed text block with its inline emoji drawn as images (custom `<:name:id>` tokens
/// showed raw in bot embeds): text segments, emoji and newlines as flex-wrap items, like
/// message text but without click targets.
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
            // one item per word so the text flows after an inline emoji
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
        // `.emoji` 1.375em of the embed text
        row = row.child(div().mx(z(1.)).child(crate::media::emoji::emoji_element(token, size * 1.375)));
        last = r.end;
    }
    push(row, last..t.text.len())
}

pub(crate) fn embed(app: &PhoenixApp, e: &Embed, cx: &App) -> AnyElement {
    let has_text = e.title.is_some() || e.description.is_some() || e.author.is_some() || !e.fields.is_empty();
    // gifv embeds always render as bare media, even with a provider/title (Tenor).
    let media_only = e.kind.as_deref() == Some("gifv") || (e.kind.as_deref() == Some("image") && !has_text);
    // Image-only embeds (links to pictures/gifs) render bare, like attachments.
    if media_only && let Some(m) = e.thumbnail.as_ref().or(e.image.as_ref()) {
        // gifv embeds (embedVideo_) are capped at 300px tall: the Tenor gif renders
        // 372x300 in vanilla #discussion (live CDP box); image links keep the 550x350 box.
        let max_h = if e.kind.as_deref() == Some("gifv") { 300. } else { media::MAX_H };
        let size = media::fit(m.width, m.height, media::MAX_W, max_h);
        let original = e.url.clone().unwrap_or_else(|| m.url.clone());
        // gifv: play the animated gif rendition (GPUI animates GIF frames only while the
        // row is laid out, i.e. on screen in the virtual list, and the window is active).
        // Accessibility "Play GIFs when Discord is focused" (gif_auto_play): off = still frame.
        // Off: the still frame, playing while hovered (client gifv rule).
        let gifv = e.kind.as_deref() == Some("gifv");
        let animated = (gifv && media::gif_plays(&original))
            .then(|| e.video.iter().chain(e.thumbnail.iter()).find_map(|v| media::gifv_animated(&v.url, size.0)))
            .flatten();
        let Some(src) = animated else {
            let still = media_image(&m.url, original.clone(), "".into(), (m.width, m.height), size);
            return div().mt(z(4.)).child(media::gif_hover(original, still)).into_any_element();
        };
        // While the animated rendition downloads/decodes, show the embed's static
        // thumbnail (proxied, resized) instead of an empty box, then swap. The thumbnail
        // of a gifv is the whole .gif: without `format=webp` the proxy resized every frame
        // (megabytes, tens of seconds), so the box stayed empty. The client asks for the
        // single-frame webp (`?format=webp&width=&height=`, ~4 KB).
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
    // Small thumbnail on the right for article/link embeds, big image below otherwise.
    let side_thumb = e.image.is_none() && e.kind.as_deref() != Some("video");
    let thumb = e.thumbnail.clone().filter(|_| side_thumb);
    let big = e.image.clone().or_else(|| e.thumbnail.clone().filter(|_| !side_thumb));
    // Merged multi-image posts (model::merge_embeds_on_url) draw their images in
    // the attachment media mosaic, like the client's embed `renderImages`.
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

    // Fields: inline ones share a row (up to 3, or 2 with a side thumbnail).
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
                                // embedFieldName_ body-sm/semibold --text-strong, margin-bottom 2
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
                // flex 1 / min-width 0: as a shrunk flex item the text kept its wider wrap
                // and was clipped in narrow windows ("… membres • Hier" lost its time)
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    // embedFooterText_ body-xs/medium --text-default
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
                d.child(img(media::resized(&u, 48, 48)).size(z(24.)).rounded_full().flex_none()) // embedAuthorIcon_ 24, mr 8
            })
            .child(
                div()
                    .text_size(z(14.))
                    // embedAuthorName_ .875rem semibold --text-strong
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
            // embedTitle_ 1rem semibold --text-strong; embedTitleLink_ is an anchor (--text-link)
            .text_color(if url.is_some() { tk(idx::TEXT_LINK) } else { tk(idx::TEXT_STRONG) })
            .when(url.is_some(), |d| d.hover(|s| s.underline()))
            .child(t)
            .when_some(url, |d, u| d.cursor_pointer().on_click(move |_, _, cx| cx.open_url(&u)))
    });

    // embed_ radius 4px; gridContainer_ max-width 516px; embedFull_ --background-surface-high,
    // 1px --border-subtle, 4px start border (embed colour or --border-normal); grid_ padding
    // .125rem 1rem .5rem .75rem; children embedMargin_ margin-top 8px.
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
                            // embedProvider_ .embedLink_ --interactive-text-default
                            d.child(div().text_size(z(12.)).line_height(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)).child(p))
                        })
                        .children(author)
                        .children(title)
                        .when_some(e.description.clone(), |d, t| d.child(embed_markdown(app, &t, 14.)))
                        .children(fields)
                        .when_some(big, |d, m| {
                            let size = media::fit(m.width, m.height, 400., 300.);
                            let original = m.url.clone();
                            // embedFull_ .embedMedia_ margin-top 16px, radius 4px.
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

/// One-line reply preview of raw message `content` (mentions resolved, markdown stripped).
pub(crate) fn reply_preview_text(app: &PhoenixApp, content: &SharedString) -> SharedString {
    let m = crate::model::Message { content: content.clone(), ..Default::default() };
    app.render_msg(&m, None).preview
}

/// Jumped-to message highlight. flash_: `transition: background-color .6s ease`;
/// flashOverlay_: --message-highlight-background-default, border-inline-start 2px
/// --text-brand, `transition: opacity .6s ease`. Fades in, holds, fades out when
/// `data-flash` drops (see `FLASH_HOLD`).
fn flash_overlay(id: Id) -> AnyElement {
    use gpui::AnimationExt;
    const IN: f32 = 600.;
    const OUT_AT: f32 = 2000.;
    const TOTAL: f32 = 2600.;
    let ease = crate::ui::states::cubic_bezier(0.25, 0.1, 0.25, 1.0); // CSS `ease`
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
                // flashPulse .6s ease-in-out overrides the fade-in (full motion):
                // opacity 1 at 0/40/80/100 %, 0 at 20/60 %.
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
