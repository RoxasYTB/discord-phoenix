use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::app::{PhoenixApp, SidebarRow};
use crate::theme::{c, generated::idx, tok, z};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Unread,
    Mentions(u32),
}

#[derive(Default)]
struct Side {
    unread: Option<usize>,
    mention: Option<usize>,
    mentions: u32,
}

type Bar = Option<(usize, Mode)>;

fn bars(app: &PhoenixApp) -> (Bar, Bar) {
    let Some(g) = app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) else { return (None, None) };
    if !g.community {
        return (None, None);
    }
    let list = &app.sidebar_list;
    let (mut above, mut below) = (Side::default(), Side::default());
    let (mut unread_visible, mut mention_visible) = (false, false);
    for (r, row) in app.sidebar_rows.iter().enumerate() {
        let (SidebarRow::Channel(id) | SidebarRow::Thread(id)) = *row else { continue };
        let Some(ch) = app.channels.get(&id) else { continue };
        let unread = ch.unread && !ch.muted && !matches!(ch.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage);
        if !unread && ch.mentions == 0 {
            continue;
        }
        let ix = r + 1;
        let (side, is_above) = match (list.item_is_above_viewport(ix), list.item_is_below_viewport(ix)) {
            (Some(true), _) => (&mut above, true),
            (_, Some(true)) => (&mut below, false),
            (Some(false), Some(false)) => {
                unread_visible |= unread;
                mention_visible |= ch.mentions > 0;
                continue;
            }
            _ => continue,
        };
        if unread && (is_above || side.unread.is_none()) {
            side.unread = Some(ix);
        }
        if ch.mentions > 0 {
            if is_above || side.mention.is_none() {
                side.mention = Some(ix);
            }
            side.mentions += ch.mentions;
        }
    }
    let pick = |s: &Side| -> Bar {
        if !mention_visible && s.mentions > 0 {
            return s.mention.map(|ix| (ix, Mode::Mentions(s.mentions)));
        }
        if !unread_visible { s.unread.map(|ix| (ix, Mode::Unread)) } else { None }
    };
    let bottom = pick(&below);
    let top = pick(&above);
    let is_m = |b: &Bar| matches!(b, Some((_, Mode::Mentions(_))));
    let show_top = top.is_some() && (bottom.is_none() || (!is_m(&bottom) && is_m(&top)));
    let show_bottom = bottom.is_some() && (is_m(&bottom) || !show_top);
    (top.filter(|_| show_top), bottom.filter(|_| show_bottom))
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
    let mention = matches!(mode, Mode::Mentions(_));
    let label = match mode {
        Mode::Unread => crate::t!("channels.unread_bar"),
        Mode::Mentions(1) => crate::t!("channels.mention_bar_one"),
        Mode::Mentions(n) => crate::t!("channels.mention_bar", count = n),
    };
    div()
        .absolute()
        .left_0()
        .right_0()
        .p(z(8.))
        .when(top, |d| d.top_0())
        .when(!top, |d| d.bottom_0())
        .child(
            div()
                .id(if top { "channels-unread-top" } else { "channels-unread-bottom" })
                .h(z(24.))
                .rounded(z(12.))
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
                                offset: gpui::point(px(0.), z(2.)),
                                blur_radius: z(6.),
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
