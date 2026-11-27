use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::app::{PhoenixApp, SidebarRow};
use crate::theme::{c, generated::idx, tok, z};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Unread,
    Mentions,
}

type Bar = Option<(usize, Mode)>;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Place {
    Above,
    Visible,
    Below,
}

#[derive(Clone, Copy, Debug)]
struct Item {
    ix: usize,
    place: Place,
    unread: bool,
    mentioned: bool,
    row: bool,
}

fn counts_as_unread(unread: bool, muted: bool, unreads_all: bool) -> bool {
    unread && !muted && unreads_all
}

fn bars(app: &PhoenixApp) -> (Bar, Bar) {
    let Some(g) = app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) else { return (None, None) };
    if !g.community {
        return (None, None);
    }
    let list = &app.sidebar_list;
    let laid_out = list.item_is_above_viewport(0).is_some();
    let mut items = Vec::new();
    for (r, row) in app.sidebar_rows.iter().enumerate() {
        let (thread, id) = match *row {
            SidebarRow::Channel(id) => (false, id),
            SidebarRow::Thread(id) => (true, id),
            _ => continue,
        };
        let Some(ch) = app.channels.get(&id) else { continue };
        if matches!(ch.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage) {
            continue;
        }
        if !ch.unread && ch.mentions == 0 {
            continue;
        }
        let unreads_all = app.backend.unreads_all(id);
        let unread = counts_as_unread(ch.unread, ch.muted, unreads_all);
        let ix = r + 1;
        let place = match (list.item_is_above_viewport(ix), list.bounds_for_item(ix)) {
            (Some(true), _) => Some(Place::Above),
            (_, Some(b)) => {
                let vp = list.viewport_bounds();
                Some(place_of(f32::from(b.top()), f32::from(b.bottom()), f32::from(vp.top()), f32::from(vp.bottom())))
            }
            (_, None) if laid_out => Some(Place::Below),
            _ => None,
        };
        tracing::debug!(
            target: "phoenix::unread_bar",
            channel = id.0,
            name = %ch.name,
            unread = ch.unread,
            muted = ch.muted,
            unreads_all,
            mention_count = ch.mentions,
            last_read = app.backend.last_read(id).map_or(0, |m| m.0),
            place = ?place,
            counted = unread,
        );
        if let Some(place) = place
            && (unread || ch.mentions > 0)
        {
            items.push(Item { ix, place, unread, mentioned: ch.mentions > 0, row: !thread });
        }
    }
    let out = pick(&items);
    tracing::debug!(target: "phoenix::unread_bar", top = ?out.0, bottom = ?out.1);
    out
}

fn place_of(top: f32, bottom: f32, view_top: f32, view_bottom: f32) -> Place {
    if top < view_top {
        Place::Above
    } else if bottom > view_bottom {
        Place::Below
    } else {
        Place::Visible
    }
}

fn pick(items: &[Item]) -> (Bar, Bar) {
    let first = |place: Place, rev: bool, f: fn(&Item) -> bool| {
        let hit = |i: &&Item| i.place == place && f(i);
        let found = if rev { items.iter().rev().find(hit) } else { items.iter().find(hit) };
        found.map(|i| i.ix)
    };
    let top_mention = first(Place::Above, false, |i| i.mentioned);
    let top_unread = first(Place::Above, false, |i| i.unread);
    let bottom_mention = first(Place::Below, true, |i| i.mentioned);
    let bottom_unread = first(Place::Below, true, |i| i.unread);
    let unread_visible = items.iter().any(|i| i.place == Place::Visible && i.row && i.unread);
    let bar = |mention: Option<usize>, unread: Option<usize>| -> Bar {
        mention.map(|ix| (ix, Mode::Mentions)).or(unread.map(|ix| (ix, Mode::Unread)))
    };
    let hide_top = top_mention.is_none() && (unread_visible || bottom_unread.is_some() || bottom_mention.is_some());
    let hide_bottom = bottom_mention.is_none() && unread_visible;
    (
        bar(top_mention, top_unread).filter(|_| !hide_top),
        bar(bottom_mention, bottom_unread).filter(|_| !hide_bottom),
    )
}

pub fn render(app: &PhoenixApp) -> Vec<gpui::AnyElement> {
    let (top, bottom) = bars(app);
    let mut out = Vec::new();
    for (bar, is_top) in [(top, true), (bottom, false)] {
        if let Some((ix, mode)) = bar {
            out.push(bar_el(app, ix, mode, is_top));
        }
    }
    out
}

fn bar_el(app: &PhoenixApp, ix: usize, mode: Mode, top: bool) -> gpui::AnyElement {
    let list = app.sidebar_list.clone();
    let mention = mode == Mode::Mentions;
    let label = match mode {
        Mode::Unread => crate::t!("channels.unread_bar"),
        Mode::Mentions => crate::t!("channels.mentions_bar"),
    };
    div()
        .absolute()
        .left_0()
        .right_0()
        .p(z(8.))
        .flex()
        .justify_center()
        .when(top, |d| d.top_0())
        .when(!top, |d| d.bottom_0())
        .child(
            div()
                .id(if top { "channels-unread-top" } else { "channels-unread-bottom" })
                .px(z(8.))
                .py(z(4.))
                .rounded(z(20.))
                .flex()
                .items_center()
                .justify_center()
                .gap(z(4.))
                .cursor_pointer()
                .border_1()
                .map(|d| {
                    if mention {
                        d.bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
                            .border_color(c(tok(idx::BORDER_SUBTLE)))
                            .shadow(crate::ui::inbox::shadow_medium())
                    } else {
                        d.bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST))).border_color(c(tok(idx::BORDER_NORMAL))).shadow(vec![
                            gpui::BoxShadow {
                                color: c(tok(idx::OPACITY_BLACK_24)),
                                offset: gpui::point(px(0.), z(12.)),
                                blur_radius: z(24.),
                                spread_radius: px(0.),
                                inset: false,
                            },
                        ])
                    }
                })
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(if mention { idx::TEXT_OVERLAY_LIGHT } else { idx::INTERACTIVE_TEXT_DEFAULT })))
                .when(!mention, |d| {
                    d.child(
                        gpui::svg()
                            .path(crate::icons::path(if top { "chevron-up" } else { "chevron" }))
                            .size(z(14.))
                            .flex_none()
                            .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))),
                    )
                })
                .child(label.to_uppercase())
                .on_click(move |_, w, _| {
                    list.scroll_to_reveal_item(ix);
                    w.refresh();
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(ix: usize, place: Place, unread: bool, mentioned: bool) -> Item {
        Item { ix, place, unread, mentioned, row: true }
    }

    #[test]
    fn only_mentions_and_muted_channels_do_not_count() {
        assert!(counts_as_unread(true, false, true));
        assert!(!counts_as_unread(true, false, false));
        assert!(!counts_as_unread(true, true, true));
    }

    #[test]
    fn top_bar_needs_no_unread_visible_or_below() {
        let above = it(2, Place::Above, true, false);
        assert_eq!(pick(&[above]), (Some((2, Mode::Unread)), None));
        assert_eq!(pick(&[above, it(5, Place::Visible, true, false)]), (None, None));
        assert_eq!(pick(&[above, it(9, Place::Below, true, false)]), (None, Some((9, Mode::Unread))));
        assert_eq!(pick(&[above, it(5, Place::Visible, false, true)]), (Some((2, Mode::Unread)), None));
    }

    #[test]
    fn a_visible_unread_thread_does_not_hide_the_bars() {
        let thread = Item { row: false, ..it(5, Place::Visible, true, false) };
        assert_eq!(pick(&[it(2, Place::Above, true, false), thread]), (Some((2, Mode::Unread)), None));
    }

    #[test]
    fn bars_jump_to_the_farthest_channel_like_the_client_walk() {
        let r = pick(&[it(1, Place::Above, true, false), it(3, Place::Above, true, false)]);
        assert_eq!(r, (Some((1, Mode::Unread)), None));
        let r = pick(&[it(8, Place::Below, true, false), it(12, Place::Below, true, false)]);
        assert_eq!(r, (None, Some((12, Mode::Unread))));
    }

    #[test]
    fn partly_hidden_rows_count_as_hidden_like_the_client() {
        assert_eq!(place_of(94., 128., 100., 700.), Place::Above);
        assert_eq!(place_of(100., 134., 100., 700.), Place::Visible);
        assert_eq!(place_of(680., 714., 100., 700.), Place::Below);
        assert_eq!(place_of(666., 700., 100., 700.), Place::Visible);
        assert_eq!(place_of(20., 54., 100., 700.), Place::Above);
    }

    #[test]
    fn mentions_win_over_unreads_and_show_even_when_an_unread_is_visible() {
        let r = pick(&[it(2, Place::Above, true, true), it(9, Place::Below, true, false)]);
        assert_eq!(r, (Some((2, Mode::Mentions)), Some((9, Mode::Unread))));
        let r = pick(&[it(2, Place::Above, true, false), it(9, Place::Below, false, true)]);
        assert_eq!(r, (None, Some((9, Mode::Mentions))));
        let r = pick(&[it(4, Place::Visible, true, false), it(9, Place::Below, false, true)]);
        assert_eq!(r, (None, Some((9, Mode::Mentions))));
    }
}
