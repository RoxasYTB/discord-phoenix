use std::rc::Rc;

use gpui::Context;
use phoenix_api::rest::reporting::ReportTarget;

use super::{ActionReply, ActionRequest, now_unix, open_modal};
use crate::{
    app::PhoenixApp,
    model::{Id, Reaction},
    ui::modals::{Field, Form, Modal, forward::Forward},
};

pub const REMINDERS: [(&str, Option<u64>); 4] =
    [("msg.remind.30m", Some(1800)), ("msg.remind.1h", Some(3600)), ("msg.remind.4h", Some(14_400)), ("msg.remind.tomorrow", None)];

pub const CUSTOM_REMINDERS: [(&str, u64); 6] = [
    ("menu.mute.15m", 900),
    ("menu.mute.3h", 10_800),
    ("menu.mute.8h", 28_800),
    ("menu.mute.24h", 86_400),
    ("msg.remind.3d", 259_200),
    ("msg.remind.1w", 604_800),
];

pub fn tomorrow_9am(now: chrono::DateTime<chrono::Local>) -> u64 {
    use chrono::{Duration, TimeZone};
    let day = now.date_naive() + Duration::days(1);
    let t = day.and_hms_opt(9, 0, 0).expect("valid time");
    chrono::Local.from_local_datetime(&t).earliest().map(|d| d.timestamp() as u64).unwrap_or(now.timestamp() as u64 + 86_400)
}

pub fn next_monday_9am(now: chrono::DateTime<chrono::Local>) -> Option<u64> {
    use chrono::{Datelike, Duration, TimeZone};
    let ahead = 7 - now.weekday().num_days_from_monday() as i64;
    if ahead == 1 {
        return None;
    }
    let t = (now.date_naive() + Duration::days(ahead)).and_hms_opt(9, 0, 0)?;
    chrono::Local.from_local_datetime(&t).earliest().map(|d| d.timestamp() as u64)
}

pub fn due_for(secs: Option<u64>) -> u64 {
    match secs {
        Some(s) => now_unix() + s,
        None => tomorrow_9am(chrono::Local::now()),
    }
}

thread_local! {
    static REMINDER_LIST: std::cell::RefCell<Vec<crate::model::Message>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn reminders() -> Vec<crate::model::Message> {
    REMINDER_LIST.with(|l| l.borrow().clone())
}

impl PhoenixApp {
    pub fn open_forward(&mut self, channel: Id, msg: Id, cx: &mut Context<Self>) {
        self.chat.menu = None;
        open_modal(Modal::Forward(Forward::new(channel, msg)), None);
        cx.notify();
    }

    pub fn open_thread_form(&mut self, channel: Id, msg: Id, cx: &mut Context<Self>) {
        self.chat.menu = None;
        let initial: String =
            self.messages.get(&channel).and_then(|x| x.msgs.iter().find(|m| m.id == msg)).map(|m| m.content.chars().take(40).collect()).unwrap_or_default();
        open_modal(
            Modal::Form(Form {
                title: crate::t!("thread.create_title"),
                subtitle: None,
                fields: vec![Field::new(crate::t!("thread.name"), crate::t!("thread.name_placeholder"), initial)],
                choices: Vec::new(),
                choice: None,
                check: None,
                submit: crate::t!("thread.create"),
                danger: false,
                preview: None,
                width: 440.,
                on_submit: Rc::new(move |a, v, _, cx| {
                    let name = v.into_iter().next().unwrap_or_default();
                    a.action(ActionRequest::CreateThread { channel, msg, name }, cx, |a, r, cx| match r {
                        Ok(ActionReply::Channel(id)) => {
                            a.parity_toast(crate::t!("thread.created"), cx);
                            a.select_channel(id, cx);
                        }
                        Ok(_) => {}
                        Err(e) => a.parity_toast(e.0, cx),
                    });
                }),
            }),
            None,
        );
        cx.notify();
    }

    pub fn remind_message(&mut self, channel: Id, msg: Id, due_unix: u64, cx: &mut Context<Self>) {
        self.chat.menu = None;
        self.action_toast(ActionRequest::Remind { channel, msg, due_unix }, Some(crate::t!("msg.remind_set")), cx);
    }

    pub fn open_custom_reminder(&mut self, channel: Id, msg: Id, cx: &mut Context<Self>) {
        self.chat.menu = None;
        open_modal(
            Modal::Form(Form {
                title: crate::t!("msg.remind"),
                subtitle: Some(crate::t!("msg.remind_custom_sub")),
                fields: Vec::new(),
                choices: CUSTOM_REMINDERS.iter().map(|(k, _)| crate::t!(k)).collect(),
                choice: Some(0),
                check: None,
                submit: crate::t!("msg.remind_save"),
                danger: false,
                preview: None,
                width: 440.,
                on_submit: Rc::new(move |a, _, choice, cx| {
                    let secs = CUSTOM_REMINDERS[choice.unwrap_or(0).min(CUSTOM_REMINDERS.len() - 1)].1;
                    a.remind_message(channel, msg, now_unix() + secs, cx);
                }),
            }),
            None,
        );
        cx.notify();
    }

    pub fn remove_reactions(&mut self, channel: Id, msg: Id, emoji: Option<Reaction>, cx: &mut Context<Self>) {
        self.chat.menu = None;
        if let Some(m) = self.messages.get_mut(&channel).and_then(|x| x.msgs.iter_mut().find(|m| m.id == msg)) {
            match &emoji {
                Some(e) => m.reactions.retain(|r| r.emoji != e.emoji || r.emoji_id != e.emoji_id),
                None => m.reactions.clear(),
            }
        }
        self.action_toast(ActionRequest::RemoveReactions { channel, msg, emoji }, None, cx);
        cx.notify();
    }

    pub fn report_message(&mut self, channel: Id, msg: Id, cx: &mut Context<Self>) {
        self.chat.menu = None;
        let t = ReportTarget::Message { channel: crate::discord_backend::convert::sf(channel), message: crate::discord_backend::convert::sf(msg) };
        self.open_report(t, cx);
    }

    pub fn load_reminders_tab(&mut self, cx: &mut Context<Self>) {
        self.action(ActionRequest::Reminders, cx, move |a, r, cx| match r {
            Ok(ActionReply::Messages(m)) => REMINDER_LIST.with(|l| *l.borrow_mut() = m),
            Ok(_) => {}
            Err(e) => a.parity_toast(e.0, cx),
        });
    }

    pub fn clear_reminder(&mut self, channel: Id, msg: Id, cx: &mut Context<Self>) {
        REMINDER_LIST.with(|l| l.borrow_mut().retain(|m| m.id != msg));
        self.parity_request(crate::commands::ParityRequest::SetSaved { channel, message: msg, saved: false }, cx, |_, _, _| {});
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn tomorrow_is_9am() {
        let now = chrono::Local.with_ymd_and_hms(2026, 3, 10, 22, 15, 0).unwrap();
        let t = tomorrow_9am(now);
        let d = chrono::Local.timestamp_opt(t as i64, 0).unwrap();
        assert_eq!(d.format("%Y-%m-%d %H:%M").to_string(), "2026-03-11 09:00");
    }
}
