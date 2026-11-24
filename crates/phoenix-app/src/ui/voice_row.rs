use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{AnyElement, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, Point, StatefulInteractiveElement, Styled, div, point, px, svg};

use crate::theme::{c, generated::idx, tok, z};

use crate::model::Id;

thread_local! {
    static ROWS: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

thread_local! {
    static HIDDEN_NAMES: std::cell::RefCell<Option<std::collections::BTreeSet<u64>>> = const { std::cell::RefCell::new(None) };
}

fn hidden_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join(crate::APP_ID).join("voice-hidden-names.txt"))
}

fn with_hidden<R>(f: impl FnOnce(&mut std::collections::BTreeSet<u64>) -> R) -> R {
    HIDDEN_NAMES.with(|h| {
        let mut h = h.borrow_mut();
        let set = h.get_or_insert_with(|| {
            hidden_path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect())
                .unwrap_or_default()
        });
        f(set)
    })
}

pub fn names_hidden(channel: Id) -> bool {
    with_hidden(|h| h.contains(&channel.0))
}

pub fn set_names_hidden(channel: Id, hidden: bool) {
    let text = with_hidden(|h| {
        if hidden {
            h.insert(channel.0);
        } else {
            h.remove(&channel.0);
        }
        h.iter().map(u64::to_string).collect::<Vec<_>>().join(" ")
    });
    if let Some(p) = hidden_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, text);
    }
}

pub fn collapsed_list(app: &crate::app::PhoenixApp, users: &[Id]) -> AnyElement {
    div()
        .w_full()
        .pl(z(38.))
        .pb(z(10.))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_start()
        .children(users.iter().map(|u| {
            let speaking = app.voice.is_speaking(*u);
            div()
                .h(z(32.))
                .ml(z(8.))
                .flex()
                .items_center()
                .child(crate::ui::voice_panel::speaking_ring(crate::ui::avatar(app.users.get(u), 24.), 24., speaking))
        }))
        .into_any_element()
}

pub fn record(user: Id, b: Bounds<Pixels>) {
    ROWS.with(|r| r.borrow_mut().insert(user.0, b));
}

pub fn popout_anchor(user: Id, fallback: Point<Pixels>) -> Point<Pixels> {
    let b = ROWS.with(|r| r.borrow().get(&user.0).copied());
    match b {
        Some(b) => point(b.right() + px(8. + 300. + 16.), b.top() + px(60.)),
        None => point(fallback.x + px(300. + 16. + 8.), fallback.y + px(60.)),
    }
}

pub fn options_button(user: Id, guild: Option<Id>, in_call: bool) -> AnyElement {
    div()
        .id(("voice-user-options", user.0))
        .size(z(24.))
        .flex_none()
        .rounded(z(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        .child(
            svg()
                .path(crate::icons::path("more-horizontal"))
                .size(z(16.))
                .text_color(c(tok(idx::CHANNELS_DEFAULT))),
        )
        .on_click(move |ev, window, cx| {
            cx.stop_propagation();
            if in_call {
                crate::ui::voice_panel::open_user_menu(user, ev.position(), cx);
            } else {
                crate::ui::user_menu::open(user, guild, ev.position(), window);
            }
        })
        .into_any_element()
}

pub fn voice_activity_card(app: &crate::app::PhoenixApp, user: Id, entity: gpui::Entity<crate::app::PhoenixApp>) -> Option<AnyElement> {
    use gpui::{FontWeight, prelude::FluentBuilder};
    let ch = app.channels.values().find(|c| c.voice_users.contains(&user))?;
    let (cid, cname) = (ch.id, ch.name.clone());
    let g = ch.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let gname = g.map(|g| g.name.clone());
    let acronym = g.map(|g| g.acronym.clone()).unwrap_or_default();
    let icon = g.and_then(|g| match &g.icon {
        crate::model::ImageRef::Url(u) => Some(u.clone()),
        _ => None,
    });
    let light = crate::ui::profile_theme::is_light();
    Some(
        div()
            .id(("profile-voice-card", user.0))
            .pb(z(8.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .p(z(8.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)))
                    .hover(|s| s.bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER))))
                    .when(light, |d| d.border_1().border_color(c(tok(idx::BORDER_MUTED))))
                    .cursor_pointer()
                    .child(
                        div()
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(tok(idx::TEXT_DEFAULT)))
                            .child(crate::t!("members.in_voice")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .child(
                                div()
                                    .size(z(60.))
                                    .flex_none()
                                    .rounded(z(8.))
                                    .border_1()
                                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                                    .bg(c(tok(idx::CARD_BACKGROUND_DEFAULT)))
                                    .child(div().absolute().inset_0().rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                                    .relative()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(z(14.))
                                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                                    .overflow_hidden()
                                    .map(|d| match icon {
                                        Some(u) => d.child(gpui::img(u).absolute().inset_0().size_full().rounded(z(8.))),
                                        None => d.child(acronym),
                                    }),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(z(4.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(z(4.))
                                            .child(svg().path(crate::icons::path("speaker")).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_DEFAULT))))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .text_size(z(14.))
                                                    .line_height(z(18.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(c(tok(idx::TEXT_STRONG)))
                                                    .text_ellipsis()
                                                    .child(cname),
                                            ),
                                    )
                                    .when_some(gname, |d, n| {
                                        d.child(
                                            div()
                                                .text_size(z(12.))
                                                .line_height(z(16.))
                                                .text_color(c(tok(idx::TEXT_MUTED)))
                                                .text_ellipsis()
                                                .child(n),
                                        )
                                    }),
                            ),
                    ),
            )
            .on_click(move |_, _, cx| {
                entity.update(cx, |a, cx| {
                    a.people.popout = None;
                    a.open_channel_chat(cid, cx);
                })
            })
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn anchor_is_right_of_row() {
        record(Id(5), Bounds::new(point(px(118.), px(290.)), size(px(289.), px(32.))));
        let a = popout_anchor(Id(5), point(px(0.), px(0.)));
        assert_eq!(a.x - px(316.), px(415.));
        assert_eq!(a.y - px(60.), px(290.));
    }
}

pub fn stage_audience(app: &crate::app::PhoenixApp, channel: Id) -> AnyElement {
    use crate::theme::generated::global::SPACE_XS;
    let audience: Vec<Id> = app
        .channels
        .get(&channel)
        .map(|c| c.voice_users.iter().copied().filter(|u| crate::ui::voice_panel::flags(*u).suppress).collect())
        .unwrap_or_default();
    let shown = audience.len().min(3);
    let more = audience.len() - shown;
    let ring = |child: AnyElement| {
        div()
            .ml(z(-8.))
            .rounded_full()
            .border(z(4.))
            .border_color(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .child(child)
    };
    div()
        .w_full()
        .pl(z(38.))
        .pb(z(10.))
        .child(
            div()
                .mt(z(4.))
                .h(z(24.))
                .pl(z(SPACE_XS))
                .flex()
                .flex_row()
                .items_center()
                .gap(z(8.))
                .child(
                    div()
                        .size(z(24.))
                        .flex_none()
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::icon(crate::assets::Icon::Headphones, 16. * crate::theme::zoom(), c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .pl(z(8.))
                        .children(audience.iter().take(shown).map(|u| ring(crate::ui::avatar(app.users.get(u), 24.))))
                        .children((more > 0).then(|| {
                            div()
                                .ml(z(-8.))
                                .rounded(z(16.))
                                .border(z(4.))
                                .border_color(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                                .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                                .px(z(8.))
                                .py(z(3.))
                                .text_size(z(12.))
                                .line_height(z(16.))
                                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                                .child(format!("+{more}"))
                        })),
                ),
        )
        .into_any_element()
}

thread_local! {
    static STREAM_HOVER: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

pub fn set_stream_hover(user: Id, hovered: bool) -> bool {
    STREAM_HOVER.with(|h| {
        let before = h.get();
        if hovered {
            h.set(Some(user.0));
        } else if before == Some(user.0) {
            h.set(None);
        }
        before != h.get()
    })
}

pub fn stream_preview(app: &crate::app::PhoenixApp, user: Id) -> Option<AnyElement> {
    use gpui::{AnimationExt, FontWeight};
    if STREAM_HOVER.with(|h| h.get()) != Some(user.0) || !crate::ui::voice_panel::flags(user).stream {
        return None;
    }
    let b = ROWS.with(|r| r.borrow().get(&user.0).copied())?;
    let name = crate::ui::sidebar::voice_user_name(app, user);
    let card = div()
        .id(("stream-preview", user.0))
        .occlude()
        .w(z(252.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow_lg()
        .child(
            div()
                .id(("stream-preview-img", user.0))
                .group("stream-preview-img")
                .relative()
                .w_full()
                .h(z(142.))
                .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .bg(c(tok(idx::OPACITY_BLACK_60)))
                        .opacity(0.)
                        .group_hover("stream-preview-img", |s| s.opacity(1.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(z(18.))
                        .text_size(z(14.))
                        .text_color(c(tok(idx::WHITE)))
                        .child(crate::t!("voice.watch_stream")),
                ),
        )
        .child(
            div()
                .p(z(10.))
                .flex()
                .items_center()
                .child(
                    div()
                        .mr(z(8.))
                        .h(z(16.))
                        .px(z(6.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
                        .flex()
                        .items_center()
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(tok(idx::WHITE)))
                        .child(crate::t!("voice.live")),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_ellipsis()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(name),
                ),
        )
        .with_animation(
            ("stream-preview-in", user.0),
            gpui::Animation::new(std::time::Duration::from_millis(150)),
            |d, t| d.opacity(t).ml(z(8. * (1. - t))),
        );
    Some(
        gpui::deferred(gpui::anchored().position(point(b.right() + px(8.), b.top())).child(card))
            .with_priority(1)
            .into_any_element(),
    )
}
