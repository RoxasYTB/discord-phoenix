use std::{cell::RefCell, collections::HashMap, time::Duration};

use gpui::{Animation, AnimationExt, AnyElement, App, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder};

use crate::{
    app::PhoenixApp,
    model::{CommandUse, FLAG_LOADING, Id, Message, SendState},
    theme::{c, fz, generated::idx, tok, z},
};

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Sending,
    Created,
    Failed(Option<String>),
}

#[derive(Clone, Debug)]
pub struct Pending {
    pub channel: Id,
    pub app_name: String,
    pub phase: Phase,
    pub interaction: Option<Id>,
}

thread_local! {
    static PENDING: RefCell<HashMap<u64, Pending>> = RefCell::new(HashMap::new());
}

pub fn track(nonce: u64, p: Pending) {
    PENDING.with_borrow_mut(|m| m.insert(nonce, p));
}

pub fn get(nonce: u64) -> Option<Pending> {
    PENDING.with_borrow(|m| m.get(&nonce).cloned())
}

pub fn set_phase(nonce: u64, phase: Phase) -> Option<Pending> {
    PENDING.with_borrow_mut(|m| {
        let p = m.get_mut(&nonce)?;
        p.phase = phase;
        Some(p.clone())
    })
}

pub fn set_interaction(nonce: u64, id: Id) -> Option<Pending> {
    PENDING.with_borrow_mut(|m| {
        let p = m.get_mut(&nonce)?;
        p.interaction = Some(id);
        if p.phase == Phase::Sending {
            p.phase = Phase::Created;
        }
        Some(p.clone())
    })
}

pub fn take(nonce: u64) -> Option<Pending> {
    PENDING.with_borrow_mut(|m| m.remove(&nonce))
}

pub fn nonce_for_interaction(id: Id) -> Option<u64> {
    PENDING.with_borrow(|m| m.iter().find(|(_, p)| p.interaction == Some(id)).map(|(n, _)| *n))
}

pub fn placeholder(app: &PhoenixApp, channel: Id, nonce: u64, bot: Id, name: &str, command_type: u8, target: Option<Id>) -> Message {
    Message {
        id: Id(nonce),
        channel,
        author: bot,
        timestamp: (phoenix_api::models::Snowflake(nonce).timestamp_ms() / 1000) as i64,
        nonce: Some(nonce),
        send_state: SendState::Sending,
        type_id: if command_type == 1 { 20 } else { 23 },
        application_id: Some(bot),
        interaction: Some(CommandUse { id: Id(0), user: app.me.id, name: name.to_owned().into(), command_type, target }),
        ..Default::default()
    }
}

pub const LOADING_TTL_MS: u64 = 900_000;

fn timed_out(msg: &Message, now_ms: u64) -> bool {
    msg.flags & FLAG_LOADING != 0 && phoenix_api::models::Snowflake(msg.id.0).timestamp_ms() + LOADING_TTL_MS < now_ms
}

pub enum Status {
    Busy(SharedString),
    Error(SharedString),
}

pub fn status(app: &PhoenixApp, msg: &Message) -> Option<Status> {
    let author = || app.users.get(&msg.author).map(|u| u.name.to_string());
    if let Some(n) = msg.nonce
        && msg.send_state != SendState::Sent
        && let Some(p) = get(n)
    {
        return Some(match p.phase {
            Phase::Sending => Status::Busy(crate::t!("slash.sending")),
            Phase::Created => Status::Busy(crate::t!("slash.thinking", app = author().unwrap_or(p.app_name))),
            Phase::Failed(Some(e)) => Status::Error(e.into()),
            Phase::Failed(None) => Status::Error(crate::t!("slash.interaction_failed")),
        });
    }
    if msg.flags & FLAG_LOADING == 0 || msg.interaction.is_none() {
        return None;
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    if timed_out(msg, now) {
        return Some(Status::Error(crate::t!("slash.no_response")));
    }
    Some(Status::Busy(crate::t!("slash.thinking", app = author().unwrap_or_default())))
}

fn dots() -> AnyElement {
    let dot = |i: usize| {
        div().size(z(7.)).flex().items_center().justify_center().child(
            div().rounded_full().bg(c(tok(idx::TEXT_DEFAULT))).with_animation(
                ("cmd-dot", i),
                Animation::new(Duration::from_millis(1200)).repeat(),
                move |d, t| {
                    let (o, s) = super::composer::dot_frame(i, t);
                    d.size(z(7. * s)).opacity(o)
                },
            ),
        )
    };
    div().flex_none().mr(z(8.)).flex().items_center().gap(z(2.)).child(dot(0)).child(dot(1)).child(dot(2)).into_any_element()
}

pub fn status_line(st: Status) -> AnyElement {
    let (lead, text, color) = match st {
        Status::Busy(t) => (dots(), t, idx::TEXT_DEFAULT),
        Status::Error(t) => (
            gpui::svg().path(crate::icons::path("settings/circle-error")).size(z(12.)).flex_none().mr(z(8.)).text_color(c(tok(idx::ICON_FEEDBACK_CRITICAL))).into_any_element(),
            t,
            idx::TEXT_FEEDBACK_CRITICAL,
        ),
    };
    div()
        .mt(z(4.))
        .flex()
        .items_center()
        .child(lead)
        .child(div().text_size(fz(16.)).line_height(fz(20.)).text_color(c(tok(color))).child(text))
        .into_any_element()
}

pub fn header(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, cu: &CommandUse, compact: bool, cx: &App) -> gpui::Div {
    let _ = cx;
    let user = app.member_user(app.selected_guild, cu.user);
    let user = user.as_deref();
    let nick = app.member_nick(cu.user).or_else(|| user.map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("common.unknown"));
    let (role_color, role_gradient) = app.author_role_style(cu.user);
    let color = role_color.map(c).unwrap_or_else(|| c(tok(idx::TEXT_STRONG)));
    let (lead, tail) = crate::t!("slash.used").split_once("{cmd}").map(|(a, b)| (a.to_owned(), b.to_owned())).unwrap_or_default();
    let target = cu.target.and_then(|t| app.users.get(&t)).map(|u| app.member_nick(u.id).unwrap_or_else(|| u.display_name.clone()));
    let name: SharedString = if cu.command_type == 1 { format!("/{}", cu.name).into() } else { cu.name.clone() };
    div()
        .relative()
        .h(fz(if compact { 20. } else { 18. }))
        .when(!compact, |d| d.mb(z(4.)))
        .when(compact, |d| d.ml(z(-24.)).pl(z(10.)))
        .flex()
        .items_center()
        .text_size(fz(14.))
        .line_height(fz(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .whitespace_nowrap()
        .overflow_hidden()
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
                .border_color(c(tok(idx::SPINE_DEFAULT))),
        )
        .when(!compact, |d| d.child(div().flex_none().mr(z(4.)).child(crate::ui::avatar(user, 16.))))
        .child(
            crate::fonts::styled_name(div().flex_none().mr(z(4.)).font_weight(FontWeight::MEDIUM).text_color(color).opacity(0.64), user, false)
                .id(("cmd-user", msg.id.0))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_click(super::message_ext::open_user(entity, cu.user, msg.channel))
                .child(crate::fonts::name_text_with(nick, user, 14., role_gradient)),
        )
        .child(div().flex_none().mr(z(4.)).child(SharedString::from(lead.trim().to_owned())))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_LINK)))
                .opacity(0.64)
                .child(name),
        )
        .when_some(target, |d, t| {
            d.child(div().flex_none().mx(z(2.)).child(" › ")).child(div().flex_none().font_weight(FontWeight::MEDIUM).opacity(0.64).child(t))
        })
        .when(!tail.trim().is_empty(), |d| d.child(div().flex_none().ml(z(4.)).child(SharedString::from(tail.trim().to_owned()))))
}

pub fn dismiss_button(entity: &Entity<PhoenixApp>, msg: &Message) -> Option<AnyElement> {
    let n = msg.nonce?;
    let p = get(n)?;
    if !matches!(p.phase, Phase::Failed(_)) {
        return None;
    }
    let (e, ch, id) = (entity.clone(), msg.channel, msg.id);
    Some(
        div()
            .id(("cmd-dismiss", id.0))
            .mt(z(4.))
            .text_size(fz(12.))
            .text_color(c(tok(idx::TEXT_LINK)))
            .cursor_pointer()
            .hover(|s| s.underline())
            .on_click(move |_, _, cx| {
                e.update(cx, |a, cx| {
                    take(n);
                    a.discard_local(ch, id, cx);
                })
            })
            .child(crate::t!("chat.ephemeral_dismiss"))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_follow_the_gateway() {
        track(5, Pending { channel: Id(1), app_name: "Bot".into(), phase: Phase::Sending, interaction: None });
        assert_eq!(set_interaction(5, Id(9)).map(|p| p.phase), Some(Phase::Created));
        assert_eq!(nonce_for_interaction(Id(9)), Some(5));
        assert!(set_phase(5, Phase::Failed(None)).is_some());
        assert!(take(5).is_some());
        assert!(get(5).is_none());
    }

    #[test]
    fn loading_messages_time_out_after_15_minutes() {
        let base = 1_700_000_000_000;
        let id = phoenix_api::models::Snowflake::from_timestamp_ms(base);
        let m = Message { id: Id(id.0), flags: FLAG_LOADING, ..Default::default() };
        assert!(!timed_out(&m, base + LOADING_TTL_MS));
        assert!(timed_out(&m, base + 1 + LOADING_TTL_MS));
    }
}
