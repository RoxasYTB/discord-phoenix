use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};

use crate::{
    actions::{close_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, button_icon, checkbox_option, select_field},
};

use super::Modal;

pub const MAX_QUESTION: usize = 300;
pub const MAX_ANSWER: usize = 55;
pub const MAX_ANSWERS: usize = 10;
pub const DURATIONS: [u32; 7] = [1, 4, 8, 24, 72, 168, 336];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerEmoji {
    pub id: Option<Id>,
    pub name: SharedString,
    pub animated: bool,
}

impl AnswerEmoji {
    pub fn token(&self) -> String {
        match self.id {
            Some(id) => format!("<{}:{}:{}>", if self.animated { "a" } else { "" }, self.name, id.0),
            None => self.name.to_string(),
        }
    }
}

#[derive(Default)]
pub struct AnswerSlot {
    pub input: Option<Entity<InputState>>,
    pub emoji: Option<AnswerEmoji>,
}

pub struct State {
    pub channel: Id,
    pub question: Option<Entity<TextareaState>>,
    pub answers: Vec<AnswerSlot>,
    pub duration: usize,
    pub multi: bool,
    pub list_open: bool,
    pub sending: bool,
    pub scheduled: Option<i64>,
}

thread_local! {
    static PENDING: std::cell::RefCell<Option<State>> = const { std::cell::RefCell::new(None) };
}

pub fn has_pending() -> bool {
    PENDING.with(|p| p.borrow().is_some())
}

pub fn restore(at: Option<Option<i64>>) {
    let Some(mut s) = PENDING.with(|p| p.borrow_mut().take()) else { return };
    if let Some(at) = at {
        s.scheduled = at;
    }
    crate::actions::open_modal(Modal::CreatePoll(s), None);
}

pub fn restore_if_dismissed() {
    if with_modal(|m| m.is_none()) {
        restore(None);
    }
}

pub fn set_scheduled(at: Option<i64>) {
    update(|s| s.scheduled = at);
}

pub fn open_schedule(cx: &mut Context<PhoenixApp>) {
    let Some(s) = with_modal(|m| match m.take() {
        Some(Modal::CreatePoll(s)) => Some(s),
        other => {
            *m = other;
            None
        }
    }) else {
        return;
    };
    let (channel, scheduled) = (s.channel, s.scheduled);
    PENDING.with(|p| *p.borrow_mut() = Some(s));
    let at = scheduled.unwrap_or_else(|| crate::ui::chat::scheduled::default_at(chrono::Local::now()));
    crate::actions::open_modal(
        Modal::Schedule(crate::ui::chat::scheduled::Picker { channel, at, editing: scheduled.is_some(), open: None, existing: None, month: None, reminder: None, poll: true }),
        None,
    );
    cx.notify();
}

pub fn schedule_label(at: Option<i64>) -> SharedString {
    match at {
        None => crate::t!("poll.schedule"),
        Some(at) => {
            let d = crate::ui::chat::scheduled::local(at);
            crate::t!("poll.scheduling", date = crate::ui::chat::scheduled::day_month(d.date_naive()), time = crate::i18n::clock(d))
        }
    }
}

impl State {
    pub fn new(channel: Id) -> Self {
        Self {
            channel,
            question: None,
            answers: vec![AnswerSlot::default(), AnswerSlot::default()],
            duration: 3,
            multi: false,
            list_open: false,
            sending: false,
            scheduled: None,
        }
    }
}

pub fn prepare_question(s: &mut State, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if s.question.is_none() {
        let ph = crate::t!("poll.question_ph");
        let q = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 5).placeholder(ph));
        q.update(cx, |st, cx| st.focus(window, cx));
        s.question = Some(q);
    }
}

pub fn prepare(s: &mut State, mk: &mut impl FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    for a in s.answers.iter_mut().filter(|a| a.input.is_none()) {
        a.input = Some(mk(crate::t!("poll.answer_ph"), "", false));
    }
}

pub fn pick_emoji(index: usize, emoji: AnswerEmoji) {
    update(|s| {
        if let Some(slot) = s.answers.get_mut(index) {
            slot.emoji = Some(emoji);
        }
    });
}

pub fn duration_label(hours: u32) -> SharedString {
    let n = |k: &str, v: u32| crate::t!(k, n = v);
    match hours {
        1 => n("poll.dur_hour_one", 1),
        h if h < 72 => n("poll.dur_hours", h),
        72 => n("poll.dur_days", 3),
        168 => n("poll.dur_week_one", 1),
        h => n("poll.dur_weeks", h / 168),
    }
}

fn update(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::CreatePoll(s)) = m {
            f(s);
        }
    });
}

fn label(t: SharedString) -> gpui::Div {
    div().mb(z(8.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t)
}

fn field_box() -> gpui::Div {
    div()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .text_size(z(16.))
}

pub fn render(_app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let qlen = s.question.as_ref().map_or(0, |q| q.read(cx).value().chars().count());
    let mut body = div().pt(z(8.)).flex().flex_col();
    if let Some(q) = &s.question {
        body = body
            .child(label(crate::t!("poll.question")))
            .child(field_box().min_h(z(48.)).py(z(6.)).px(z(10.)).child(div().flex_1().min_w_0().child(Textarea::new(q).appearance(false).bordered(false))))
            .child(
                div()
                    .mt(z(4.))
                    .flex()
                    .justify_end()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(c(tok(if qlen > MAX_QUESTION { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT })))
                    .child(format!("{qlen} / {MAX_QUESTION}")),
            );
    }
    let n = s.answers.len();
    let mut answers = div().mt(z(26.)).flex().flex_col().gap(z(4.)).child(label(crate::t!("poll.answers")).mb(z(4.)));
    for (i, slot) in s.answers.iter().enumerate() {
        let Some(a) = &slot.input else { continue };
        let emoji_el = match &slot.emoji {
            Some(e) => crate::media::emoji::emoji_element(&e.token(), 18. * crate::theme::zoom()),
            None => gpui::svg().path(crate::icons::path("emoji")).size(z(18.)).text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT))).into_any_element(),
        };
        answers = answers.child(
            field_box()
                .id(("poll-answer-field", i))
                .h(z(40.))
                .px(z(4.))
                .gap(z(4.))
                .child(
                    div()
                        .id(("poll-answer-emoji", i))
                        .w(z(38.))
                        .h(z(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(emoji_el)
                        .on_click(cx.listener(move |a, _, w, cx| {
                            crate::ui::pickers::open_emoji_picker(a, crate::ui::pickers::EmojiTarget::PollAnswer(i), w, cx);
                        })),
                )
                .child(div().flex_1().min_w_0().child(Input::new(a).appearance(false).bordered(false)))
                .child(
                    div()
                        .id(("poll-answer-del", i))
                        .size(z(32.))
                        .rounded(z(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                        .child(gpui::svg().path(crate::icons::path("chat/trash")).size(z(16.)).text_color(c(tok(idx::TEXT_STRONG))))
                        .tooltip(crate::ui::tooltip::tip(crate::t!("poll.remove_answer", n = i + 1)))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            update(|s| {
                                if s.answers.len() > 1 && i < s.answers.len() {
                                    s.answers.remove(i);
                                }
                            });
                            cx.notify();
                        })),
                ),
        );
    }
    if n < MAX_ANSWERS {
        answers = answers.child(
            div().flex().justify_end().child(button_icon("poll-add-answer", "plus", crate::t!("poll.add_answer"), BtnKind::Secondary).on_click(cx.listener(
                |_, _, _, cx| {
                    update(|s| s.answers.push(AnswerSlot::default()));
                    cx.notify();
                },
            ))),
        );
    }
    body = body.child(answers);
    let dur = s.duration;
    let mut duration = div()
        .mt(z(18.))
        .relative()
        .flex()
        .flex_col()
        .child(label(crate::t!("poll.duration")))
        .child(select_field("poll-duration", duration_label(DURATIONS[dur])).on_click(cx.listener(|_, _, _, cx| {
            update(|s| s.list_open = !s.list_open);
            cx.notify();
        })));
    if s.list_open {
        let mut list = div()
            .mt(z(4.))
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .flex()
            .flex_col();
        for (k, h) in DURATIONS.iter().enumerate() {
            list = list.child(
                div()
                    .id(("poll-dur", k))
                    .h(z(36.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .when(k == dur, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                    .cursor_pointer()
                    .child(duration_label(*h))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        update(|s| {
                            s.duration = k;
                            s.list_open = false;
                        });
                        cx.notify();
                    })),
            );
        }
        duration = duration.child(list);
    }
    body = body.child(duration);
    let multi = s.multi;
    let scheduled = s.scheduled;
    let footer = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .child(checkbox_option("poll-multi", crate::t!("poll.allow_multi"), multi).on_click(cx.listener(|_, _, _, cx| {
            update(|s| s.multi = !s.multi);
            cx.notify();
        })))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(
                    button("poll-schedule", SharedString::default(), if scheduled.is_some() { BtnKind::Active } else { BtnKind::Secondary })
                        .w(z(40.))
                        .min_w(z(40.))
                        .px(z(0.))
                        .child(gpui::svg().path(crate::icons::path("inbox/schedule")).size(z(20.)).text_color(c(tok(if scheduled.is_some() {
                            idx::CONTROL_CONNECTED_TEXT_DEFAULT
                        } else {
                            idx::CONTROL_SECONDARY_TEXT_DEFAULT
                        }))))
                        .tooltip(crate::ui::tooltip::tip(schedule_label(scheduled)))
                        .on_click(cx.listener(|_, _, _, cx| open_schedule(cx))),
                )
                .child(button("poll-post", crate::t!("poll.post"), BtnKind::Primary).min_w(z(100.)).on_click(cx.listener(|a, _, _, cx| submit(a, cx)))),
        );
    super::card(480., crate::t!("poll.create_title"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

fn submit(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let mut req = None;
    with_modal(|m| {
        let Some(Modal::CreatePoll(s)) = m else { return };
        if s.sending {
            return;
        }
        let question: String = s.question.as_ref().map(|q| q.read(cx).value().trim().to_string()).unwrap_or_default();
        let answers: Vec<crate::commands::PollAnswerDraft> = s
            .answers
            .iter()
            .filter_map(|slot| {
                let text: String = slot.input.as_ref()?.read(cx).value().trim().chars().take(MAX_ANSWER).collect();
                if text.is_empty() {
                    return None;
                }
                let emoji = slot.emoji.as_ref().map(|e| (e.id, e.name.to_string()));
                Some((text, emoji))
            })
            .collect();
        if question.is_empty() || answers.is_empty() || question.chars().count() > MAX_QUESTION {
            return;
        }
        s.sending = true;
        req = Some((
            s.scheduled,
            crate::commands::ParityRequest::CreatePoll {
                channel: s.channel,
                question,
                answers,
                hours: DURATIONS[s.duration],
                multi: s.multi,
            },
        ));
    });
    let Some((scheduled, req)) = req else { return };
    if let (Some(at), crate::commands::ParityRequest::CreatePoll { channel, question, answers, hours, multi }) = (scheduled, &req) {
        let call = schedule_call(*channel, at, question, answers, *hours, *multi);
        app.action(crate::actions::ActionRequest::Admin(call), cx, move |a, r, cx| {
            match r {
                Ok(_) => {
                    close_modal();
                    a.parity_toast(crate::ui::chat::scheduled::done_text(at), cx);
                }
                Err(e) => {
                    update(|s| s.sending = false);
                    a.parity_toast(crate::t!("sched.failed", e = e.0), cx);
                }
            }
            cx.notify();
        });
        return;
    }
    app.parity_request(req, cx, |a, res, cx| {
        match res {
            Ok(_) => close_modal(),
            Err(e) => {
                update(|s| s.sending = false);
                a.parity_toast(e.0, cx);
            }
        }
        cx.notify();
    });
}

pub fn schedule_call(channel: Id, at: i64, question: &str, answers: &[crate::commands::PollAnswerDraft], hours: u32, multi: bool) -> phoenix_api::rest::guild_manage::AdminCall {
    use phoenix_api::models::Snowflake;
    let answers: Vec<phoenix_api::rest::forum::PollAnswerInput> = answers.iter().map(|(t, em)| (t.clone(), em.clone().map(|(i, n)| (i.map(|i| Snowflake(i.0)), n)))).collect();
    let message = phoenix_api::rest::forum::poll_body(question, &answers, hours, multi);
    phoenix_api::rest::guild_manage::AdminCall::ScheduleMessage { channel: Snowflake(channel.0), at: crate::ui::chat::scheduled::iso(at), message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_poll_goes_to_scheduled_messages_and_is_guarded() {
        let answers = vec![("A".to_string(), None), ("B".to_string(), Some((None, "🔥".to_string())))];
        let call = schedule_call(Id(5), 1_795_600_800, "Q?", &answers, 24, true);
        let r = phoenix_api::rest::guild_manage::build(&call);
        assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/users/@me/scheduled-messages"));
        let b = r.body.unwrap();
        assert_eq!(b["channel_id"], "5");
        assert_eq!(b["content"], "");
        assert_eq!(b["scheduled_timestamp"], "2026-11-25T10:00:00.000Z");
        assert_eq!(b["poll"]["question"]["text"], "Q?");
        assert_eq!(b["poll"]["answers"][1]["poll_media"]["emoji"]["name"], "🔥");
        assert_eq!(b["poll"]["duration"], 24);
        assert_eq!(b["poll"]["allow_multiselect"], true);
        assert_eq!(b["poll"]["layout_type"], 1);
        assert!(b.get("nonce").is_none());
        assert!(!crate::actions::guard::read_only_action(&crate::actions::ActionRequest::Admin(call)));
    }

    #[test]
    fn schedule_state_survives_the_picker() {
        PENDING.with(|p| *p.borrow_mut() = Some(State::new(Id(3))));
        assert!(has_pending());
        restore(Some(Some(42)));
        assert!(!has_pending());
        let at = with_modal(|m| match m.take() {
            Some(Modal::CreatePoll(s)) => s.scheduled,
            _ => None,
        });
        assert_eq!(at, Some(42));
    }

    #[test]
    fn durations_match_the_client_list() {
        assert_eq!(DURATIONS[State::new(Id(1)).duration], 24);
        let labels: Vec<String> = DURATIONS.iter().map(|h| duration_label(*h).to_string()).collect();
        assert_eq!(labels.len(), 7);
        assert!(labels[3].starts_with("24"));
        assert!(labels[6].starts_with('2'));
    }

    #[test]
    fn answer_emoji_token_formats_unicode_and_custom() {
        let u = AnswerEmoji { id: None, name: "🔥".into(), animated: false };
        assert_eq!(u.token(), "🔥");
        let c = AnswerEmoji { id: Some(Id(42)), name: "party".into(), animated: true };
        assert_eq!(c.token(), "<a:party:42>");
    }
}
