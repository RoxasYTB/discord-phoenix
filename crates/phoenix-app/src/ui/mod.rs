pub mod autocomplete;
pub mod avatar_deco;
pub mod mobile_status;
pub mod profile_theme;
pub mod profile_games;
pub mod profile_board;
pub mod profile_wishlist;
pub mod call_view;
pub mod incoming_call;
pub mod dm_call;
pub mod chat;
pub mod code_copy;
pub mod components;
pub mod community;
pub mod guide;
pub mod events;
pub mod focus_ring;
pub mod forum;
pub mod group_dm;
pub mod inbox;
pub mod context_menu;
pub mod controls;
pub mod guild_menu;
pub mod home;
pub mod locale_inputs;
pub mod invite;
pub mod image_viewer;
pub mod login;
pub mod settings;
pub mod members;
pub mod activity_traits;
pub mod nameplate;
pub mod profile_fx;
pub mod modal;
pub mod pickers;
pub mod modals;
pub mod profile;
pub mod dm_profile_panel;
pub mod avatar_color;
pub mod profile_bg;
pub mod profile_note;
pub mod profile_bio;
pub mod search;
pub mod search_filters;
pub mod server_bar;
pub mod sidebar;
pub mod sidebar_resize;
pub mod states;
pub mod status_picker;
pub mod status_svg;
pub mod role_gradient;
pub mod switcher;
pub mod nav_menus;
pub mod thread_menu;
pub mod thread_create;
pub mod title_bar;
pub mod window_chrome;
pub mod tooltip;
pub mod channel_unread_bar;
pub mod user_menu;
pub mod voice_panel;
pub mod voice_row;
pub mod shop_page;
pub mod emoji_text;

use crate::theme::z;
use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px, svg,
};

use crate::{
    assets::Icon,
    model::{ImageRef, Presence, User},
    theme::{DEFAULT_AVATAR, Palette, c, presence_color},
};

pub fn icon(i: Icon, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(i.path()).size(px(size)).flex_none().text_color(color)
}

/// Circle avatar. URL images go through GPUI's asset cache, which fetches
/// lazily only when the element is actually painted (rows are virtualized).
pub fn avatar(user: Option<&User>, size: f32) -> AnyElement {
    let img_ref = user.map(|u| u.avatar.clone()).unwrap_or(ImageRef::Default(0));
    match img_ref {
        ImageRef::Url(url) => crate::media::avatar_image(url, size),
        // The client shows the CDN default avatar (embed/avatars/<n>.png), fetched at runtime;
        // the colour shows while it loads.
        ImageRef::Default(ix) => div()
            .size(px(size))
            .flex_none()
            .rounded_full()
            .bg(c(DEFAULT_AVATAR[ix as usize % DEFAULT_AVATAR.len()]))
            .child(crate::media::avatar_image(format!("https://cdn.discordapp.com/embed/avatars/{ix}.png").into(), size))
            .into_any_element(),
    }
}

/// Avatar with the presence status cut-out (bottom-right) like the client's
/// `svg-mask-avatar-status-round-<size>` + status rect (see [`status_svg`]).
/// GPUI has no masks: the cut is painted with `ring`, the colour behind the avatar.
pub fn avatar_with_status(user: Option<&User>, size: f32, p: Presence, ring: Hsla) -> Div {
    avatar_with_status_overlay(user, size, p, ring, |d| d)
}

/// [`avatar_with_status`] whose cut also shows the row's hover / selected
/// background: `overlay` styles a layer over `ring` (e.g. `group_hover`), so
/// the cut keeps matching a translucent row background like a real mask.
pub fn avatar_with_status_overlay(user: Option<&User>, size: f32, p: Presence, ring: Hsla, overlay: impl FnOnce(Div) -> Div) -> Div {
    avatar_with_status_deco(user, size, p, ring, overlay, None)
}

/// [`avatar_with_status_overlay`] with an avatar decoration (`deco`, the image
/// source to draw). Layers, bottom to top: avatar, decoration, status cut-out,
/// status glyph. The client masks the decoration with
/// `svg-mask-avatar-decoration-status-round-<size>`, whose hole is the same
/// circle as the avatar's cut (32: centre 27, r 8; 80: 68/14; 96: 80/18;
/// 120: 100/20, in avatar coordinates), so painting the cut over the
/// decoration reproduces the mask and the status always stays on top.
pub fn avatar_with_status_deco(
    user: Option<&User>,
    size: f32,
    p: Presence,
    ring: Hsla,
    overlay: impl FnOnce(Div) -> Div,
    deco: Option<SharedString>,
) -> Div {
    let (cx, r, s) = status_svg::geometry(size);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(avatar(user, size))
        .when_some(deco, |d, src| d.child(avatar_deco::layer(src, size)))
        .child(
            div()
                .absolute()
                .left(px(cx - r))
                .top(px(cx - r))
                .size(px(2. * r))
                .rounded_full()
                .bg(ring)
                .child(overlay(div().size_full().rounded_full())),
        )
        .child(div().absolute().left(px(cx - s / 2.)).top(px(cx - s / 2.)).child(status_dot(p, s, ring)))
}

/// The status glyph (`svg-mask-status-*` shapes) in the presence colour.
/// `_ring` is kept for callers: the shapes are real holes now.
pub fn status_dot(p: Presence, size: f32, _ring: Hsla) -> gpui::Svg {
    gpui::svg().path(status_svg::path(p, false)).size(px(size)).flex_none().text_color(presence_color(p))
}

/// Red mention badge ("3").
pub fn badge(n: u32) -> Div {
    let label: SharedString = if n > 99 { "99+".into() } else { n.to_string().into() };
    div()
        .h(z(16.))
        .min_w(z(16.))
        .px(z(4.))
        .rounded(z(8.))
        .bg(c(Palette::RED))
        .text_color(gpui::white())
        .text_size(z(12.))
        .font_weight(FontWeight::BOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(label)
}

/// 32px square icon button with hover background (user panel, header).
pub fn icon_button(id: &'static str, i: Icon, color: Hsla, size: f32) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .size(px(size + 8.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(4.))
        .cursor_pointer()
        .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
        .child(icon(i, size, color))
}
