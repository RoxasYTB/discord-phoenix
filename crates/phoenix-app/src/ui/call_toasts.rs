use std::cell::RefCell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::app::PhoenixApp;
use crate::model::{Id, Message};
use crate::theme::{c, generated::idx, tok, z};

pub const MAX: usize = 3;
pub const LINGER: Duration = Duration::from_secs(10);
const STAGE_TYPES: [u8; 5] = [27, 28, 29, 30, 31];

#[derive(Default)]
struct Toasts {
    channel: Option<Id>,
    ids: Vec<Id>,
    msgs: Vec<Message>,
    hide_at: Option<Instant>,
    frozen: bool,
}

thread_local! {
    static TOASTS: RefCell<Toasts> = RefCell::new(Toasts::default());
}

pub fn accepts(m: &Message, voice: Option<Id>) -> bool {
    voice == Some(m.channel) && !STAGE_TYPES.contains(&m.type_id) && m.send_state != crate::model::SendState::Sending
}

pub fn push(channel: Id, id: Id, now: Instant) {
    TOASTS.with_borrow_mut(|t| {
        if t.channel != Some(channel) {
            *t = Toasts { channel: Some(channel), ..Default::default() };
        }
        t.ids.push(id);
        let extra = t.ids.len().saturating_sub(MAX);
        t.ids.drain(..extra);
        let keep = t.ids.clone();
        t.msgs.retain(|m| keep.contains(&m.id));
        t.hide_at = Some(now + LINGER);
    });
}

pub fn shown(channel: Id, now: Instant) -> Vec<Id> {
    TOASTS.with_borrow_mut(|t| {
        if t.channel != Some(channel) {
            return Vec::new();
        }
        if !t.frozen && t.hide_at.is_some_and(|h| now >= h) {
            t.ids.clear();
            t.msgs.clear();
            t.hide_at = None;
        }
        t.ids.clone()
    })
}

fn set_frozen(on: bool) {
    TOASTS.with_borrow_mut(|t| t.frozen = on);
}

pub fn on_message(app: &PhoenixApp, m: &Message, cx: &mut Context<PhoenixApp>) {
    if !accepts(m, app.voice_channel) {
        return;
    }
    push(m.channel, m.id, Instant::now());
    TOASTS.with_borrow_mut(|t| t.msgs.push(m.clone()));
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(LINGER + Duration::from_millis(50)).await;
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn preview(app: &PhoenixApp, m: &Message) -> SharedString {
    let text = m.content.as_ref();
    if text.is_empty() {
        return m.attachments.first().map(|a| a.filename.clone()).unwrap_or_default();
    }
    let mut out = String::new();
    let mut at = 0;
    for (r, k) in crate::ui::chat::rich_input::spans(text) {
        out.push_str(&text[at..r.start]);
        match crate::ui::chat::rich_input::plain_text(&k, app) {
            Some(s) => out.push_str(&s),
            None => out.push_str(&text[r.clone()]),
        }
        at = r.end;
    }
    out.push_str(&text[at..]);
    out.replace('\n', " ").into()
}

fn mentioned(app: &PhoenixApp, m: &Message) -> bool {
    m.mention_everyone || m.content.contains(&format!("<@{}>", app.me.id.0)) || m.content.contains(&format!("<@!{}>", app.me.id.0))
}

fn toast(app: &PhoenixApp, m: &Message, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let shadow = vec![gpui::BoxShadow { color: c(tok(idx::OPACITY_BLACK_20)), offset: gpui::point(z(0.), z(2.)), blur_radius: z(10.), spread_radius: z(0.), inset: false }];
    let group: SharedString = format!("call-toast-{}", m.id.0).into();
    let (channel, id) = (m.channel, m.id);
    div()
        .id(("call-toast", m.id.0))
        .w_full()
        .flex()
        .justify_end()
        .child(
            div()
                .group(group.clone())
                .max_w_full()
                .flex()
                .flex_row()
                .overflow_hidden()
                .cursor_pointer()
                .child(
                    div()
                        .relative()
                        .min_w_0()
                        .min_h(z(32.))
                        .px(z(12.))
                        .py(z(4.))
                        .rounded(z(16.))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                        .group_hover(group, |s| s.bg(c(tok(idx::BACKGROUND_BASE_LOWER))))
                        .shadow(shadow.clone())
                        .text_size(z(16.))
                        .line_height(z(22.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .when(mentioned(app, m), |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .rounded(z(16.))
                                    .bg(c(tok(idx::YELLOW_300)).opacity(0.15))
                                    .border(z(2.))
                                    .border_color(c(tok(idx::YELLOW_300)).opacity(0.3)),
                            )
                        })
                        .child(div().min_w_0().truncate().child(preview(app, m))),
                )
                .child(div().flex_none().ml(z(8.)).size(z(32.)).rounded_full().shadow(shadow).child(crate::ui::avatar(app.users.get(&m.author), 32. * crate::theme::zoom()))),
        )
        .on_click(cx.listener(move |a, _, _, cx| {
            crate::ui::call_view::set_chat_open(true);
            a.jump_to_message(channel, id, cx);
        }))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, channel: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if crate::ui::call_view::chat_open() || !crate::ui::call_menus::chat_toasts(channel) {
        return None;
    }
    let ids = shown(channel, Instant::now());
    let kept = TOASTS.with_borrow(|t| t.msgs.clone());
    let stored = app.messages.get(&channel);
    let list: Vec<Message> = ids
        .iter()
        .filter_map(|id| stored.and_then(|e| e.msgs.iter().rev().find(|m| m.id == *id)).or_else(|| kept.iter().find(|m| m.id == *id)).cloned())
        .collect();
    if list.is_empty() {
        return None;
    }
    Some(
        div()
            .id("call-chat-toasts")
            .absolute()
            .top(z(48.))
            .right(z(12.))
            .w(z(304.))
            .h(gpui::relative(0.5))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(z(8.))
            .on_hover(|on, _, _| set_frozen(*on))
            .children(list.iter().map(|m| toast(app, m, cx)))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_three_and_hides_after_ten_seconds() {
        let (ch, t0) = (Id(1), Instant::now());
        for i in 0..5 {
            push(ch, Id(10 + i), t0);
        }
        assert_eq!(shown(ch, t0), vec![Id(12), Id(13), Id(14)]);
        push(ch, Id(20), t0 + Duration::from_secs(9));
        assert_eq!(shown(ch, t0 + Duration::from_secs(18)).len(), 3);
        assert!(shown(ch, t0 + Duration::from_secs(19)).is_empty());
        assert!(shown(Id(2), t0).is_empty());
    }

    #[test]
    fn only_messages_of_the_call_channel() {
        let m = Message { channel: Id(5), ..Default::default() };
        assert!(accepts(&m, Some(Id(5))));
        assert!(!accepts(&m, Some(Id(6))));
        assert!(!accepts(&Message { type_id: 27, ..m.clone() }, Some(Id(5))));
    }
}
