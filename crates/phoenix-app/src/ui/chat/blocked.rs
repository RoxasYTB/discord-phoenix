use gpui::{AnyElement, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div};

use super::tk;
use crate::app::{ChannelMessages, PhoenixApp};
use crate::model::Id;
use crate::theme::{generated::idx, z};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Why {
    Blocked,
    Ignored,
}

fn why(app: &PhoenixApp, author: Id) -> Option<Why> {
    if app.nav.is_blocked(author) {
        Some(Why::Blocked)
    } else if app.nav.is_ignored(author) {
        Some(Why::Ignored)
    } else {
        None
    }
}

pub enum Row {
    Normal,
    Hidden,
    Collapsed(AnyElement),
    Expanded(Option<AnyElement>),
}

pub fn row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, entry: &ChannelMessages, i: usize) -> Row {
    let msgs = &entry.msgs;
    let Some(w) = why(app, msgs[i].author) else { return Row::Normal };
    let mut start = i;
    while start > 0 && why(app, msgs[start - 1].author) == Some(w) {
        start -= 1;
    }
    let key = msgs[start].id;
    let open = app.chat.hidden_open.contains(&key);
    if i != start {
        return if open { Row::Expanded(None) } else { Row::Hidden };
    }
    let mut end = i + 1;
    while end < msgs.len() && why(app, msgs[end].author) == Some(w) {
        end += 1;
    }
    let el = bar(entity, key, w, end - start, open);
    if open { Row::Expanded(Some(el)) } else { Row::Collapsed(el) }
}

fn bar(entity: &Entity<PhoenixApp>, key: Id, w: Why, n: usize, open: bool) -> AnyElement {
    let label = match (w, n) {
        (Why::Blocked, 1) => crate::t!("chat.blocked_one", n = n),
        (Why::Blocked, _) => crate::t!("chat.blocked_other", n = n),
        (Why::Ignored, 1) => crate::t!("chat.ignored_one", n = n),
        (Why::Ignored, _) => crate::t!("chat.ignored_other", n = n),
    };
    let icon = if w == Why::Blocked { "deny" } else { "chat/eye-slash" };
    let action = if open { crate::t!("chat.hidden_hide") } else { crate::t!("chat.hidden_show") };
    let e = entity.clone();
    let group = gpui::SharedString::from(format!("blocked-{}", key.0));
    let g2 = group.clone();
    let g3 = group.clone();
    div()
        .id(("blocked-run", key.0 as usize))
        .group(group)
        .relative()
        .mt(z(17.))
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .min_h(z(22.))
        .flex()
        .items_center()
        .text_size(z(16.))
        .line_height(z(22.))
        .child(
            div().absolute().left(z(36.)).top(z(5.)).child(
                gpui::svg()
                    .path(crate::icons::path(icon))
                    .size(z(16.))
                    .text_color(tk(idx::TEXT_MUTED))
                    .group_hover(g2, |s| s.text_color(tk(idx::ICON_FEEDBACK_CRITICAL))),
            ),
        )
        .child(div().text_color(tk(idx::TEXT_DEFAULT)).child(label).child(" \u{2014} "))
        .child(
            div()
                .id(("blocked-toggle", key.0 as usize))
                .cursor_pointer()
                .text_color(tk(idx::TEXT_MUTED))
                .group_hover(g3, |s| s.text_color(tk(idx::TEXT_LINK)))
                .hover(|s| s.underline())
                .on_click(move |_, _, cx| {
                    e.update(cx, |a, cx| {
                        if !a.chat.hidden_open.remove(&key) {
                            a.chat.hidden_open.insert(key);
                        }
                        cx.notify();
                    })
                })
                .child(action),
        )
        .into_any_element()
}
