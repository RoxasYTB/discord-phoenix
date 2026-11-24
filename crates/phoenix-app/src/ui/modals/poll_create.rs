use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::{Input, InputState};

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

pub struct State {
    pub channel: Id,
    pub question: Option<Entity<InputState>>,
    pub answers: Vec<Option<Entity<InputState>>>,
    pub duration: usize,
    pub multi: bool,
    pub list_open: bool,
    pub sending: bool,
}

impl State {
    pub fn new(channel: Id) -> Self {
        Self { channel, question: None, answers: vec![None, None], duration: 3, multi: false, list_open: false, sending: false }
    }
}

pub fn prepare(s: &mut State, mk: &mut impl FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    if s.question.is_none() {
        s.question = Some(mk(crate::t!("poll.question_ph"), "", true));
    }
    for a in s.answers.iter_mut().filter(|a| a.is_none()) {
        *a = Some(mk(crate::t!("poll.answer_ph"), "", false));
    }
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
            .child(field_box().h(z(48.)).px(z(10.)).child(div().flex_1().child(Input::new(q).appearance(false).bordered(false))))
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
    for (i, a) in s.answers.iter().enumerate() {
        let Some(a) = a else { continue };
        answers = answers.child(
            field_box()
                .id(("poll-answer-field", i))
                .h(z(40.))
                .px(z(4.))
                .gap(z(4.))
                .child(div().w(z(38.)).h(z(18.)).flex().items_center().justify_center().child(
                    gpui::svg().path(crate::icons::path("emoji")).size(z(18.)).text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT))),
                ))
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
                    update(|s| s.answers.push(None));
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
                    button("poll-schedule", SharedString::default(), BtnKind::Secondary)
                        .w(z(40.))
                        .child(gpui::svg().path(crate::icons::path("inbox/schedule")).size(z(20.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
                        .tooltip(crate::ui::tooltip::tip(crate::t!("poll.schedule")))
                        .on_click(cx.listener(|a, _, _, cx| a.stub(&crate::t!("poll.schedule"), cx))),
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
        let answers: Vec<String> = s
            .answers
            .iter()
            .flatten()
            .map(|a| a.read(cx).value().trim().chars().take(MAX_ANSWER).collect::<String>())
            .filter(|a| !a.is_empty())
            .collect();
        if question.is_empty() || answers.is_empty() || question.chars().count() > MAX_QUESTION {
            return;
        }
        s.sending = true;
        req = Some(crate::commands::ParityRequest::CreatePoll {
            channel: s.channel,
            question,
            answers: answers.into_iter().map(|a| (a, None)).collect(),
            hours: DURATIONS[s.duration],
            multi: s.multi,
        });
    });
    let Some(req) = req else { return };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_match_the_client_list() {
        assert_eq!(DURATIONS[State::new(Id(1)).duration], 24);
        let labels: Vec<String> = DURATIONS.iter().map(|h| duration_label(*h).to_string()).collect();
        assert_eq!(labels.len(), 7);
        assert!(labels[3].starts_with("24"));
        assert!(labels[6].starts_with('2'));
    }
}
