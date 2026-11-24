use crate::ui::tooltip::Tipped as _;
use gpui::{Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    theme::{c, generated::idx, m, tok, z, zoom},
    ui::{home::data::HomeView, icon},
};

fn title(app: &PhoenixApp) -> (Icon, SharedString) {
    if let Some(g) = app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) {
        return (Icon::Hash, g.name.clone());
    }
    if app.nav.home == HomeView::Dm {
        return (Icon::Members, crate::t!("dm.title"));
    }
    if app.nav.home == HomeView::Nitro {
        return (Icon::Nitro, crate::t!("settings.page.nitro"));
    }
    (Icon::Members, crate::t!("friends.title"))
}

fn bar_button(id: &'static str, i: Icon, active: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .size(z(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(m::TITLE_BAR_ITEM_RADIUS))
        .occlude()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .child(icon(i, 18. * zoom(), c(tok(if active { idx::INTERACTIVE_ICON_ACTIVE } else { idx::ICON_MUTED }))))
        .tip(tip)
}

fn nav_arrow(id: &'static str, name: &'static str, back: bool, enabled: bool) -> gpui::Stateful<gpui::Div> {
    let d = div()
        .id(id)
        .w(z(26.))
        .h(z(24.))
        .flex_none()
        .occlude()
        .flex()
        .items_center()
        .when(back, |d| d.pl(z(6.)).pr(z(2.)).rounded_l(z(6.)))
        .when(!back, |d| d.pl(z(2.)).pr(z(6.)).rounded_r(z(6.)))
        .child(
            gpui::svg()
                .path(crate::icons::path(name))
                .size(z(18.))
                .flex_none()
                .text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT))),
        );
    if enabled {
        d.cursor_pointer().hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
    } else {
        d.opacity(0.5)
    }
}

pub static FORCE_CLIENT_DECORATIONS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn client_decorated(window: &gpui::Window) -> bool {
    FORCE_CLIENT_DECORATIONS.load(std::sync::atomic::Ordering::Relaxed)
        || matches!(window.window_decorations(), gpui::Decorations::Client { .. })
}

static DREW_WIN_BUTTONS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn inbox_right_inset() -> f32 {
    let tail = if DREW_WIN_BUTTONS.load(std::sync::atomic::Ordering::Relaxed) { 117. + 12. } else { 12. };
    tail + 24. + 12.
}

static VIEWPORT_H_CSS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn viewport_h_css() -> f32 {
    f32::from_bits(VIEWPORT_H_CSS.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn render(app: &PhoenixApp, window: &gpui::Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let vh = f32::from(window.viewport_size().height) / zoom();
    VIEWPORT_H_CSS.store(vh.to_bits(), std::sync::atomic::Ordering::Relaxed);
    DREW_WIN_BUTTONS.store(client_decorated(window), std::sync::atomic::Ordering::Relaxed);
    let (glyph, text) = title(app);
    let guild = app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g));
    crate::ui::window_chrome::drag_region(div().id("app-title-bar"))
        .h(z(m::TITLE_BAR_H))
        .flex_none()
        .w_full()
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .gap(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .map(|d| match guild {
                    Some(g) => d.child(
                        div().rounded(z(4.)).overflow_hidden().child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 16. * zoom())),
                    ),
                    None => d.child(icon(glyph, 16. * zoom(), c(tok(idx::INTERACTIVE_ICON_DEFAULT)))),
                })
                .child(div().max_w(z(480.)).text_ellipsis().child(text)),
        )
        .child(
            div()
                .relative()
                .w(z(80.))
                .h(z(24.))
                .flex()
                .justify_center()
                .gap(z(2.))
                .child(nav_arrow("title-back", "nav/back", true, true))
                .child(nav_arrow("title-forward", "nav/forward", false, false)),
        )
        .child(
            div()
                .relative()
                .when(!client_decorated(window), |d| d.pr(z(12.)))
                .flex()
                .items_center()
                .gap(z(12.))
                .child(
                    bar_button("title-inbox", Icon::Inbox, app.parity.inbox.open, crate::t!("inbox.title"))
                        .on_click(cx.listener(|a, _, _, cx| a.toggle_inbox(cx))),
                )
                .child(bar_button("title-help", Icon::Help, false, crate::t!("title.help")))
                .when(client_decorated(window), |d| d.child(win_buttons(cx))),
        )
}

pub fn login_bar(window: &gpui::Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    crate::ui::window_chrome::drag_region(div().id("login-title-bar"))
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(z(m::TITLE_BAR_H))
        .flex()
        .items_center()
        .justify_end()
        .when(client_decorated(window), |d| d.child(win_buttons(cx)))
}

fn win_buttons(cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let btn = |id: &'static str, close: bool| {
        div()
            .id(id)
            .group(id)
            .size(z(m::TITLE_BAR_H))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .cursor_pointer()
            .map(|d| {
                if close {
                    d.hover(|s| s.bg(c(tok(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT))))
                } else {
                    d.hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                        .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
                }
            })
    };
    let fg = c(tok(idx::INTERACTIVE_TEXT_DEFAULT));
    let fg_hover = c(tok(idx::INTERACTIVE_TEXT_HOVER));
    let fg_active = c(tok(idx::INTERACTIVE_TEXT_ACTIVE));
    let glyph_box = || div().relative().size(z(12.)).flex_none();
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(z(4.))
        .child(div().mx(z(4.)).w(z(1.)).h(z(m::TITLE_BAR_H - 12.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            btn("win-min", false)
                .child(glyph_box().child(
                    div()
                        .id("win-min-g")
                        .absolute()
                        .left(z(1.))
                        .top(z(6.))
                        .w(z(10.))
                        .h(z(1.))
                        .bg(fg)
                        .group_hover("win-min", |s| s.bg(fg_hover))
                        .group_active("win-min", |s| s.bg(fg_active)),
                ))
                .on_click(cx.listener(|_, _, w, _| w.minimize_window())),
        )
        .child(
            btn("win-max", false)
                .child(glyph_box().child(
                    div()
                        .id("win-max-g")
                        .absolute()
                        .left(z(1.))
                        .top(z(1.))
                        .size(z(10.))
                        .border_1()
                        .border_color(fg)
                        .group_hover("win-max", |s| s.border_color(fg_hover))
                        .group_active("win-max", |s| s.border_color(fg_active)),
                ))
                .on_click(cx.listener(|_, _, w, _| w.zoom_window())),
        )
        .child(
            btn("win-close", true)
                .child(
                    gpui::svg()
                        .path(crate::icons::path("window/close"))
                        .size(z(12.))
                        .flex_none()
                        .text_color(fg)
                        .group_hover("win-close", |s| s.text_color(c(tok(idx::CONTROL_CRITICAL_PRIMARY_ICON_DEFAULT)))),
                )
                .on_click(cx.listener(|a, _, w, _| {
                    if a.prefs.local.minimize_to_tray && crate::tray::running() {
                        w.minimize_window();
                    } else {
                        w.remove_window();
                    }
                })),
        )
}
