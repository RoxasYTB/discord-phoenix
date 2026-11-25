use std::{cell::RefCell, collections::HashMap};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::{models::Snowflake, rest::guild_manage::AdminCall};

use crate::{
    actions::{ActionRequest, open_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        context_menu::MenuItem,
        controls::{BtnKind, button},
        modals::{Modal, card, close_action, field_label},
    },
};

pub const MIN_AHEAD: i64 = 600;
pub const MAX_AHEAD: i64 = 691_200;
const SLOT: i64 = 15 * 60;

thread_local! {
    static DRAFTS: RefCell<HashMap<Id, i64>> = RefCell::new(HashMap::new());
}

pub fn draft(channel: Id) -> Option<i64> {
    DRAFTS.with(|d| d.borrow().get(&channel).copied())
}

pub fn set_draft(channel: Id, at: Option<i64>) {
    DRAFTS.with(|d| {
        let mut d = d.borrow_mut();
        match at {
            Some(t) => d.insert(channel, t),
            None => d.remove(&channel),
        };
    });
}

fn at_hour(day: DateTime<Local>, hour: u32) -> DateTime<Local> {
    let naive = day.date_naive().and_hms_opt(hour, 0, 0).unwrap_or_default();
    Local.from_local_datetime(&naive).earliest().unwrap_or(day)
}

pub fn presets(now: DateTime<Local>) -> Vec<(&'static str, i64)> {
    let min = now + Duration::seconds(MIN_AHEAD);
    let pick = |hour: u32, today: &'static str, tomorrow: &'static str| {
        let t = at_hour(now, hour);
        if t > min { (today, t.timestamp()) } else { (tomorrow, at_hour(now + Duration::days(1), hour).timestamp()) }
    };
    let monday = now.date_naive() - Duration::days(now.weekday().num_days_from_monday() as i64) + Duration::weeks(1);
    let monday = Local.from_local_datetime(&monday.and_hms_opt(9, 0, 0).unwrap_or_default()).earliest().map(|d| d.timestamp()).unwrap_or_default();
    vec![
        pick(9, "sched.today_9", "sched.tomorrow_9"),
        pick(13, "sched.today_13", "sched.tomorrow_13"),
        ("sched.monday_9", monday),
    ]
}

pub fn default_at(now: DateTime<Local>) -> i64 {
    let hour = now.with_minute(0).and_then(|d| d.with_second(0)).and_then(|d| d.with_nanosecond(0)).unwrap_or(now) + Duration::hours(1);
    let t = hour.timestamp();
    if t < now.timestamp() + MIN_AHEAD { t + 3600 } else { t }
}

pub fn problem(at: i64, now: i64) -> Option<&'static str> {
    if at < now + MIN_AHEAD {
        Some("sched.too_close")
    } else if at > now + MAX_AHEAD {
        Some("sched.too_far")
    } else {
        None
    }
}

pub fn iso(at: i64) -> String {
    chrono::Utc.timestamp_opt(at, 0).single().map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default()
}

pub fn day_month(d: NaiveDate) -> String {
    let month = crate::t!(&format!("month.{}", d.month()));
    if crate::i18n::locale() == "en" { format!("{month} {}", d.day()) } else { format!("{} {month}", d.day()) }
}

fn local(at: i64) -> DateTime<Local> {
    Local.timestamp_opt(at, 0).single().unwrap_or_else(Local::now)
}

pub fn bar_text(at: i64) -> SharedString {
    let d = local(at);
    crate::t!("sched.bar", date = day_month(d.date_naive()), time = crate::i18n::clock(d))
}

pub fn done_text(at: i64) -> SharedString {
    let d = local(at);
    crate::t!("sched.done", date = crate::i18n::long_date(d.date_naive()), time = crate::i18n::clock(d))
}

pub fn menu_items(e: &gpui::Entity<PhoenixApp>, channel: Id) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = presets(Local::now())
        .into_iter()
        .map(|(key, at)| {
            let e = e.clone();
            MenuItem::action(crate::t!(key), None, move |_, _, cx: &mut App| {
                set_draft(channel, Some(at));
                e.update(cx, |a, cx| {
                    a.chat.attach_menu = false;
                    cx.notify();
                });
            })
        })
        .collect();
    items.push(MenuItem::Separator);
    let e = e.clone();
    items.push(MenuItem::action(crate::t!("sched.custom"), None, move |_, _, cx: &mut App| {
        e.update(cx, |a, cx| {
            a.chat.attach_menu = false;
            a.open_schedule(channel, cx)
        })
    }));
    items
}

pub fn bar(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let ch = app.selected_channel?;
    let at = draft(ch)?;
    Some(
        div()
            .id("sched-bar")
            .h(z(34.))
            .rounded_t(z(8.))
            .flex()
            .items_center()
            .cursor_pointer()
            .on_click(cx.listener(move |a, _, _, cx| a.open_schedule(ch, cx)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .ml(z(16.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(bar_text(at)),
            )
            .child(
                div()
                    .id("sched-clear")
                    .mr(z(4.))
                    .px(z(12.))
                    .h(z(32.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.stop_propagation();
                        set_draft(ch, None);
                        cx.notify();
                    }))
                    .child(gpui::svg().path(crate::icons::path("close")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
            )
            .into_any_element(),
    )
}

pub struct Picker {
    pub channel: Id,
    pub at: i64,
    pub editing: bool,
    pub open: Option<u8>,
    pub existing: Option<Id>,
}

pub fn days(now: i64) -> Vec<NaiveDate> {
    let first = local(now + MIN_AHEAD).date_naive();
    let last = local(now + MAX_AHEAD).date_naive();
    first.iter_days().take_while(|d| *d <= last).collect()
}

pub fn slots(day: NaiveDate) -> Vec<i64> {
    let Some(start) = Local.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap_or_default()).earliest() else { return Vec::new() };
    (0..96).map(|i| start.timestamp() + i * SLOT).collect()
}

pub fn with_day(at: i64, day: NaiveDate) -> i64 {
    let t = local(at).time();
    Local.from_local_datetime(&day.and_time(t)).earliest().map(|d| d.timestamp()).unwrap_or(at)
}

fn date_field(value: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id("sched-date")
        .h(z(40.))
        .px(z(12.))
        .py(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .hover(|s| s.border_color(c(tok(idx::INPUT_BORDER_HOVER))))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(div().flex_1().min_w_0().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(value))
        .child(gpui::svg().path(crate::icons::path("calendar")).size(z(20.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
}

fn field(id: &'static str, value: SharedString, open: bool, which: u8, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let base = if which == 0 { date_field(value) } else { crate::ui::controls::select_field(id, value) };
    base.when(open, |d| d.border_color(c(tok(idx::INPUT_BORDER_ACTIVE))))
        .on_click(cx.listener(move |_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::Schedule(p)) = m {
                    p.open = if p.open == Some(which) { None } else { Some(which) };
                }
            });
            cx.notify();
        }))
}

fn list(options: Vec<(SharedString, i64, bool)>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut col = div()
        .id("sched-list")
        .absolute()
        .top(z(44.))
        .left_0()
        .right_0()
        .max_h(z(220.))
        .overflow_y_scroll()
        .p(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .flex()
        .flex_col();
    for (n, (text, at, sel)) in options.into_iter().enumerate() {
        col = col.child(
            div()
                .id(("sched-opt", n))
                .h(z(40.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(z(16.))
                .text_color(c(tok(if sel { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT })))
                .when(sel, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_modal(|m| {
                        if let Some(Modal::Schedule(p)) = m {
                            p.at = at;
                            p.open = None;
                        }
                    });
                    cx.notify();
                }))
                .child(text),
        );
    }
    col.into_any_element()
}

pub fn render(p: &Picker, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let now = Local::now().timestamp();
    let cur = local(p.at);
    let err = problem(p.at, now);
    let date_opts = days(now)
        .into_iter()
        .map(|d| (SharedString::from(crate::i18n::medium_date(d)), with_day(p.at, d), d == cur.date_naive()))
        .collect();
    let time_opts = slots(cur.date_naive())
        .into_iter()
        .filter(|t| problem(*t, now).is_none())
        .map(|t| (SharedString::from(crate::i18n::clock(local(t))), t, t == p.at))
        .collect();
    let date = div()
        .flex_1()
        .relative()
        .flex()
        .flex_col()
        .child(field_label(crate::t!("sched.date"), true))
        .child(field("sched-date", crate::i18n::medium_date(cur.date_naive()).into(), p.open == Some(0), 0, cx))
        .when(p.open == Some(0), |d| d.child(div().relative().child(list(date_opts, cx))));
    let time = div()
        .flex_1()
        .relative()
        .flex()
        .flex_col()
        .child(field_label(crate::t!("sched.time"), true))
        .child(field("sched-time", crate::i18n::clock(cur).into(), p.open == Some(1), 1, cx))
        .when_some(err, |d, k| {
            d.child(div().mt(z(8.)).text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(crate::t!(k)))
        })
        .when(p.open == Some(1), |d| d.child(div().relative().child(list(time_opts, cx))));
    let body = div().flex().gap(z(16.)).pb(z(16.)).child(date).child(time);
    let (channel, at, editing, existing) = (p.channel, p.at, p.editing && p.existing.is_none(), p.existing);
    let close = close_action(cx);
    let secondary = if editing { crate::t!("sched.clear") } else { crate::t!("common.cancel") };
    let (title, primary) = if existing.is_some() { (crate::t!("sched.update_title"), crate::t!("sched.update")) } else { (crate::t!("sched.title"), crate::t!("sched.set")) };
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("sched-cancel", secondary, BtnKind::Secondary).flex_1().on_click(move |_, w, cx| {
            if editing {
                set_draft(channel, None);
            }
            close(w, cx)
        }))
        .child(
            button("sched-set", primary, BtnKind::Primary)
                .flex_1()
                .when(err.is_some(), |d| d.opacity(0.5).cursor_default())
                .on_click(cx.listener(move |a, _, _, cx| {
                    if problem(at, Local::now().timestamp()).is_none() {
                        crate::actions::close_modal();
                        match existing {
                            Some(id) => a.reschedule(id, at, cx),
                            None => set_draft(channel, Some(at)),
                        }
                    }
                    cx.notify();
                })),
        );
    card(480., title, None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

pub fn create_call(channel: Id, at: i64, message: &phoenix_api::rest::CreateMessage) -> AdminCall {
    AdminCall::ScheduleMessage { channel: Snowflake(channel.0), at: iso(at), message: serde_json::to_value(message).unwrap_or_default() }
}

impl PhoenixApp {
    pub fn open_schedule(&mut self, channel: Id, cx: &mut Context<Self>) {
        let editing = draft(channel).is_some();
        let at = draft(channel).unwrap_or_else(|| default_at(Local::now()));
        open_modal(Modal::Schedule(Picker { channel, at, editing, open: None, existing: None }), None);
        cx.notify();
    }

    pub fn edit_scheduled_time(&mut self, id: Id, at: i64, cx: &mut Context<Self>) {
        open_modal(Modal::Schedule(Picker { channel: Id(0), at, editing: true, open: None, existing: Some(id) }), None);
        cx.notify();
    }

    fn scheduled_call(&mut self, call: AdminCall, toast: SharedString, cx: &mut Context<Self>) {
        self.action(ActionRequest::Admin(call), cx, move |a, r, cx| {
            match r {
                Ok(_) => a.parity_toast(toast, cx),
                Err(e) => a.parity_toast(e.0, cx),
            }
            a.reload_scheduled(cx);
        });
    }

    pub fn reschedule(&mut self, id: Id, at: i64, cx: &mut Context<Self>) {
        let call = AdminCall::EditScheduled { id: Snowflake(id.0), at: Some(iso(at)), content: None, flags: None };
        self.scheduled_call(call, crate::t!("sched.updated"), cx);
    }

    pub fn scheduled_send_now(&mut self, id: Id, cx: &mut Context<Self>) {
        self.scheduled_call(AdminCall::SendScheduledNow { id: Snowflake(id.0) }, crate::t!("sched.sent"), cx);
    }

    pub fn cancel_scheduled(&mut self, id: Id, cx: &mut Context<Self>) {
        self.parity.inbox.scheduled.retain(|m| m.id != id);
        self.scheduled_call(AdminCall::DeleteScheduled { id: Snowflake(id.0) }, crate::t!("sched.cancelled"), cx);
    }

    pub fn send_scheduled(&mut self, channel: Id, at: i64, message: phoenix_api::rest::CreateMessage, cx: &mut Context<Self>) {
        set_draft(channel, None);
        let call = create_call(channel, at, &message);
        self.action(ActionRequest::Admin(call), cx, move |a, r, cx| match r {
            Ok(_) => a.parity_toast(done_text(at), cx),
            Err(e) => a.parity_toast(crate::t!("sched.failed", e = e.0), cx),
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, mo, d, h, mi, 0).single().unwrap()
    }

    #[test]
    fn presets_follow_the_client_rules() {
        let wed_8 = at(2026, 11, 25, 8, 0);
        let p = presets(wed_8);
        assert_eq!(p[0], ("sched.today_9", at(2026, 11, 25, 9, 0).timestamp()));
        assert_eq!(p[1], ("sched.today_13", at(2026, 11, 25, 13, 0).timestamp()));
        assert_eq!(p[2], ("sched.monday_9", at(2026, 11, 30, 9, 0).timestamp()));
        let wed_855 = at(2026, 11, 25, 8, 55);
        assert_eq!(presets(wed_855)[0], ("sched.tomorrow_9", at(2026, 11, 26, 9, 0).timestamp()));
        let wed_14 = at(2026, 11, 25, 14, 0);
        assert_eq!(presets(wed_14)[1], ("sched.tomorrow_13", at(2026, 11, 26, 13, 0).timestamp()));
        let mon_8 = at(2026, 11, 30, 8, 0);
        assert_eq!(presets(mon_8)[2].1, at(2026, 12, 7, 9, 0).timestamp());
    }

    #[test]
    fn default_and_bounds() {
        assert_eq!(default_at(at(2026, 11, 25, 8, 20)), at(2026, 11, 25, 9, 0).timestamp());
        assert_eq!(default_at(at(2026, 11, 25, 8, 55)), at(2026, 11, 25, 10, 0).timestamp());
        let now = at(2026, 11, 25, 8, 0).timestamp();
        assert_eq!(problem(now + 599, now), Some("sched.too_close"));
        assert_eq!(problem(now + 600, now), None);
        assert_eq!(problem(now + MAX_AHEAD + 1, now), Some("sched.too_far"));
        assert_eq!(days(now).len(), 9);
        assert_eq!(slots(at(2026, 11, 25, 0, 0).date_naive()).len(), 96);
    }

    #[test]
    fn scheduled_body_matches_the_client() {
        let msg = phoenix_api::rest::CreateMessage::text("[Test Phoenix] hi").reply_to(Snowflake(5), Snowflake(6), false);
        let call = create_call(Id(5), 1_795_600_800, &msg);
        let r = phoenix_api::rest::guild_manage::build(&call);
        assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/users/@me/scheduled-messages"));
        let b = r.body.unwrap();
        assert_eq!(b["channel_id"], "5");
        assert_eq!(b["content"], "[Test Phoenix] hi");
        assert_eq!(b["scheduled_timestamp"], "2026-11-25T10:00:00.000Z");
        assert_eq!(b["flags"], 0);
        assert_eq!(b["attachments"], serde_json::json!([]));
        assert_eq!(b["message_reference"]["message_id"], "6");
        assert_eq!(b["allowed_mentions"]["replied_user"], false);
        assert!(b.get("nonce").is_none());
    }
}
