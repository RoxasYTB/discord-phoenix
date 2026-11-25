use std::rc::Rc;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{InputState, Textarea, TextareaState};
use serde_json::{Value, json};

use crate::{
    actions::{ActionRequest, EventPlace, close_modal, open_modal, with_modal},
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::{
        chat::scheduled::{CalPick, calendar_view, date_field, local, slots, time_list},
        controls::{BtnKind, button, radio_indicator, select_field},
        modals::{Modal, field_label, input_box},
    },
};

pub const TOPIC_MAX: usize = 100;
pub const LOCATION_MAX: usize = 100;
pub const DESCRIPTION_MAX: usize = 1000;
const CREATE_EVENTS: u64 = 1 << 44;
const MANAGE_EVENTS: u64 = 1 << 33;
const CREATE_INVITE: u64 = 1;
pub const COVER_EDIT_ICON: &str = "https://discord.com/assets/8f18dfd4baacebf4.svg";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stage,
    Voice,
    External,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recur {
    None,
    Weekly,
    Biweekly,
    Monthly,
    Yearly,
    Daily,
    Weekdays,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Open {
    Channel,
    StartDate,
    StartTime,
    EndDate,
    EndTime,
    Recur,
}

pub struct State {
    pub guild: Id,
    pub step: usize,
    pub kind: Option<Kind>,
    pub channel: Option<Id>,
    pub start: i64,
    pub end: Option<i64>,
    pub recur: Recur,
    pub open: Option<Open>,
    pub month: Option<NaiveDate>,
    pub location: Option<Entity<InputState>>,
    pub topic: Option<Entity<InputState>>,
    pub description: Option<Entity<TextareaState>>,
    pub saving: bool,
    pub error: Option<SharedString>,
    pub cover: Option<std::path::PathBuf>,
    pub done: bool,
    pub link: Option<SharedString>,
}

pub fn default_start(now: DateTime<Local>) -> i64 {
    let later = now + Duration::hours(1);
    let hour = later.with_minute(0).and_then(|d| d.with_second(0)).and_then(|d| d.with_nanosecond(0)).unwrap_or(later);
    let hour = if later.minute() >= 30 { hour + Duration::hours(1) } else { hour };
    hour.timestamp()
}

pub fn entity_error(kind: Option<Kind>, channel: Option<Id>, location: &str) -> Option<&'static str> {
    match kind {
        None => Some("event.err_type"),
        Some(_) if location.trim().is_empty() && channel.is_none() => Some("event.err_place"),
        _ => None,
    }
}

pub fn schedule_error(kind: Option<Kind>, start: i64, end: Option<i64>, now: i64) -> Option<&'static str> {
    if kind == Some(Kind::External) && end.is_none() {
        return Some("event.err_end_required");
    }
    if start < now {
        return Some("event.err_start_past");
    }
    match end {
        Some(e) if e < start => Some("event.err_end_before"),
        Some(e) if e < now => Some("event.err_end_past"),
        _ => None,
    }
}

pub fn topic_error(name: &str) -> Option<&'static str> {
    name.trim().is_empty().then_some("event.err_topic")
}

fn utc(at: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(at, 0).single().unwrap_or_default()
}

fn rrule_weekday(js_day: u32) -> u32 {
    (js_day + 6) % 7
}

fn shifted(at: i64, base: [u32; 5], ahead: [u32; 5], behind: [u32; 5]) -> [u32; 5] {
    let l = rrule_weekday(local(at).weekday().num_days_from_sunday()) as i32;
    let u = rrule_weekday(utc(at).weekday().num_days_from_sunday()) as i32;
    if u - l > 0 {
        ahead
    } else if u - l < 0 {
        behind
    } else {
        base
    }
}

pub fn recurrence_rule(recur: Recur, start: i64) -> Option<Value> {
    let d = utc(start);
    let wd = rrule_weekday(d.weekday().num_days_from_sunday());
    let (frequency, interval) = match recur {
        Recur::None => return None,
        Recur::Weekly => (2, 1),
        Recur::Biweekly => (2, 2),
        Recur::Monthly => (1, 1),
        Recur::Yearly => (0, 1),
        Recur::Daily | Recur::Weekdays => (3, 1),
    };
    let mut v = json!({
        "start": d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "frequency": frequency,
        "interval": interval,
        "by_weekday": null,
        "by_month": null,
        "by_month_day": null,
        "by_year_day": null,
        "count": null,
    });
    match recur {
        Recur::Weekly | Recur::Biweekly => v["by_weekday"] = json!([wd]),
        Recur::Monthly => v["by_n_weekday"] = json!([{ "n": d.day().div_ceil(7), "day": wd }]),
        Recur::Yearly => {
            v["by_month"] = json!([d.month()]);
            v["by_month_day"] = json!([d.day()]);
        }
        Recur::Weekdays => v["by_weekday"] = json!(shifted(start, [0, 1, 2, 3, 4], [1, 2, 3, 4, 5], [6, 0, 1, 2, 3])),
        Recur::None | Recur::Daily => {}
    }
    Some(v)
}

pub fn recur_options(start: i64) -> Vec<(Recur, SharedString)> {
    let d = local(start);
    let weekday = crate::t!(&format!("weekday.{}", d.weekday().num_days_from_monday())).to_string();
    let nth = d.day().div_ceil(7);
    let month = crate::t!(&format!("month_short.{}", d.month()));
    let date = if crate::i18n::locale() == "en" { format!("{month} {:02}", d.day()) } else { format!("{:02} {month}", d.day()) };
    let mut v = vec![
        (Recur::None, crate::t!("event.recur.none")),
        (Recur::Weekly, crate::t!("event.recur.weekly", weekday = weekday)),
        (Recur::Biweekly, crate::t!("event.recur.biweekly", weekday = weekday)),
        (Recur::Monthly, crate::t!(&format!("event.recur.monthly_{}", nth.min(5)), weekday = weekday)),
        (Recur::Yearly, crate::t!("event.recur.yearly", date = date)),
        (Recur::Daily, crate::t!("event.recur.daily")),
    ];
    if !matches!(d.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
        v.push((Recur::Weekdays, crate::t!("event.recur.weekdays")));
    }
    v
}

fn is_locked(app: &PhoenixApp, id: Id) -> bool {
    app.nav.extras.get(&id).is_some_and(|e| e.locked)
}

fn channel_icon(kind: Kind, locked: bool) -> &'static str {
    match (kind, locked) {
        (Kind::Stage, false) => "stage",
        (Kind::Stage, true) => "stage-lock",
        (_, false) => "speaker",
        (_, true) => "voice-lock",
    }
}

pub fn event_link(code: &str, event: Option<Id>) -> String {
    match event {
        Some(e) => format!("https://discord.gg/{code}?event={}", e.0),
        None => format!("https://discord.gg/{code}"),
    }
}

fn default_text_channel(app: &PhoenixApp, guild: Id) -> Option<Id> {
    app.guild_channels.get(&guild).into_iter().flatten().filter_map(|id| app.channels.get(id)).find(|c| c.kind == ChannelKind::Text).map(|c| c.id)
}

fn voice_channels(app: &PhoenixApp, guild: Id, kind: ChannelKind) -> Vec<(Id, SharedString)> {
    app.guild_channels
        .get(&guild)
        .into_iter()
        .flatten()
        .filter_map(|id| app.channels.get(id))
        .filter(|c| c.kind == kind)
        .map(|c| (c.id, c.name.clone()))
        .collect()
}

fn kind_channel(kind: Kind) -> Option<ChannelKind> {
    match kind {
        Kind::Voice => Some(ChannelKind::Voice),
        Kind::Stage => Some(ChannelKind::Stage),
        Kind::External => None,
    }
}

fn set_state(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::CreateEvent(s)) = m {
            f(s);
        }
    });
}

fn text(t: impl Into<SharedString>, size: f32, line: f32, color: usize) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).text_color(c(tok(color))).child(t.into())
}

fn heading(title: SharedString, sub: SharedString) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(text(title, 24., 32., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD))
        .child(text(sub, 14., 18., idx::TEXT_SUBTLE))
}

fn value(i: &Option<Entity<InputState>>, max: usize, cx: &gpui::App) -> String {
    i.as_ref().map(|i| i.read(cx).value().chars().take(max).collect()).unwrap_or_default()
}

pub fn prepare(s: &mut State, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if s.location.is_none() {
        s.location = Some(cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("event.location_ph"))));
    }
    if s.topic.is_none() {
        s.topic = Some(cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("event.topic_ph"))));
    }
    if s.description.is_none() {
        s.description = Some(cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 10)));
    }
}

fn steps(s: &State, valid: [bool; 3], cx: &mut Context<PhoenixApp>) -> AnyElement {
    let labels = [crate::t!("event.step_location"), crate::t!("event.step_info"), crate::t!("event.step_review")];
    let step = s.step;
    let reachable = valid.iter().take(step + 1).all(|v| *v);
    let mut row = div().w_full().flex().flex_row().gap(z(8.));
    for (n, label) in labels.into_iter().enumerate() {
        let on = n == step;
        row = row.child(
            div()
                .id(("event-step", n))
                .flex_1()
                .flex()
                .flex_col()
                .items_start()
                .when(n < step || (n > step && reachable), |d| d.cursor_pointer())
                .on_click(cx.listener(move |a, _, _, cx| {
                    if n < step {
                        set_state(|s| {
                            s.step -= 1;
                            s.open = None;
                        });
                    } else if n > step {
                        a.event_next(cx);
                    }
                    cx.notify();
                }))
                .child(div().w_full().h(z(4.)).mb(z(8.)).rounded(z(4.)).bg(c(tok(if on { idx::TEXT_BRAND } else { idx::TEXT_MUTED }))))
                .child(text(label, 12., 16., if on { idx::TEXT_BRAND } else { idx::TEXT_MUTED })),
        );
    }
    row.into_any_element()
}

type KindOption = (Kind, &'static str, SharedString, SharedString, bool);

fn radio_option(id: usize, on: bool, opt: KindOption, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (kind, icon, name, desc, disabled) = opt;
    div()
        .id(("event-kind", id))
        .relative()
        .pl(z(56.))
        .rounded(z(8.))
        .when(disabled, |d| d.opacity(0.5))
        .when(!disabled, |d| {
            d.cursor_pointer().on_click(cx.listener(move |a, _, _, cx| {
                a.set_event_kind(kind, cx);
            }))
        })
        .child(div().absolute().top_0().left_0().child(radio_indicator(on)))
        .child(div().absolute().top_0().left(z(32.)).child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT)))))
        .child(text(name, 16., 20., idx::TEXT_DEFAULT))
        .child(text(desc, 14., 18., idx::TEXT_SUBTLE).mt(z(4.)).ml(z(-24.)))
        .into_any_element()
}

fn dropdown(options: Vec<(SharedString, i64, bool)>, pick: Rc<dyn Fn(i64)>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    thread_local! {
        static SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
    }
    let scroll = SCROLL.with(|h| h.clone());
    div().relative().child(time_list(options, &scroll, pick, cx)).into_any_element()
}

fn toggle(which: Open) -> impl Fn(&mut PhoenixApp, &gpui::ClickEvent, &mut Window, &mut Context<PhoenixApp>) + 'static {
    move |_, _, _, cx| {
        set_state(|s| {
            s.open = if s.open == Some(which) { None } else { Some(which) };
            s.month = None;
        });
        cx.notify();
    }
}

fn location_step(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let perms = app.backend.guild_permissions(s.guild);
    let can = crate::ui::inbox::has_perm(perms, CREATE_EVENTS) || crate::ui::inbox::has_perm(perms, MANAGE_EVENTS);
    let community = app.guilds.iter().find(|g| g.id == s.guild).is_some_and(|g| g.community);
    let voices = voice_channels(app, s.guild, ChannelKind::Voice);
    let stages = voice_channels(app, s.guild, ChannelKind::Stage);
    let missing = |list: &Vec<(Id, SharedString)>| if can && list.is_empty() { crate::t!("event.no_channels") } else { crate::t!("event.no_permission") };
    let mut options: Vec<KindOption> = Vec::new();
    if community {
        let off = !can || stages.is_empty();
        options.push((Kind::Stage, "stage", crate::t!("event.kind_stage"), if off { missing(&stages) } else { crate::t!("event.kind_stage_desc") }, off));
    }
    let off = !can || voices.is_empty();
    options.push((Kind::Voice, "speaker", crate::t!("event.kind_voice"), if off { missing(&voices) } else { crate::t!("event.kind_voice_desc") }, off));
    options.push((Kind::External, "event/location", crate::t!("event.kind_external"), if can { crate::t!("event.kind_external_desc") } else { crate::t!("event.no_permission") }, !can));
    let mut group = div().pt(z(8.)).pb(z(4.)).flex().flex_col().gap(z(16.));
    for (n, opt) in options.into_iter().enumerate() {
        group = group.child(radio_option(n, s.kind == Some(opt.0), opt, cx));
    }
    let mut col = div().mt(z(24.)).flex().flex_col().gap(z(16.)).child(heading(crate::t!("event.where_title"), crate::t!("event.where_sub"))).child(group);
    match s.kind {
        Some(Kind::External) => {
            col = col.child(div().flex().flex_col().child(field_label(crate::t!("event.location_label"), true)).children(s.location.as_ref().map(input_box)));
        }
        Some(k) => {
            let list = if k == Kind::Stage { stages } else { voices };
            let label = if k == Kind::Stage { crate::t!("event.stage_channel") } else { crate::t!("event.select_channel") };
            let current = s.channel.and_then(|id| list.iter().find(|(c, _)| *c == id)).map(|(id, n)| (*id, n.clone()));
            let locked = current.as_ref().is_some_and(|(id, _)| is_locked(app, *id));
            let open = s.open == Some(Open::Channel);
            let opts = list.iter().map(|(id, n)| (channel_icon(k, is_locked(app, *id)), n.clone(), id.0 as i64, Some(*id) == s.channel)).collect();
            col = col.child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(field_label(label, true))
                    .child(
                        channel_field(current.map(|(_, n)| (channel_icon(k, locked), n)))
                            .when(open, |d| d.border_color(c(tok(idx::INPUT_BORDER_ACTIVE))))
                            .on_click(cx.listener(toggle(Open::Channel))),
                    )
                    .when(open, |d| {
                        d.child(channel_list(
                            opts,
                            Rc::new(|v| {
                                set_state(|s| {
                                    s.channel = Some(Id(v as u64));
                                    s.open = None;
                                })
                            }),
                            cx,
                        ))
                    })
                    .when(locked, |d| d.child(text(crate::t!("event.private_hint"), 14., 18., idx::TEXT_SUBTLE).mt(z(8.)))),
            );
        }
        None => {}
    }
    col.into_any_element()
}

fn channel_field(current: Option<(&'static str, SharedString)>) -> gpui::Stateful<gpui::Div> {
    let (icon, name) = match current {
        Some((i, n)) => (Some(i), n),
        None => (None, SharedString::default()),
    };
    select_field("event-channel", SharedString::default()).child(
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(z(12.))
            .right(z(40.))
            .flex()
            .items_center()
            .gap(z(8.))
            .when_some(icon, |d, i| d.child(gpui::svg().path(crate::icons::path(i)).size(z(24.)).flex_none().text_color(c(tok(idx::TEXT_STRONG)))))
            .child(div().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(name)),
    )
    .relative()
}

fn channel_list(options: Vec<(&'static str, SharedString, i64, bool)>, pick: Rc<dyn Fn(i64)>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut col = div()
        .id("event-channel-list")
        .absolute()
        .top(z(8.))
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
    for (n, (icon, name, v, sel)) in options.into_iter().enumerate() {
        let pick = pick.clone();
        col = col.child(
            div()
                .id(("event-channel-opt", n))
                .h(z(40.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .text_size(z(16.))
                .text_color(c(tok(if sel { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT })))
                .when(sel, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    pick(v);
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).flex_none().text_color(c(tok(if sel { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT }))))
                .child(div().min_w_0().truncate().child(name)),
        );
    }
    div().relative().child(gpui::deferred(col).with_priority(30)).into_any_element()
}

fn pick_cover(cx: &mut Context<PhoenixApp>) {
    crate::ui::file_pick::pick(&crate::ui::file_pick::IMAGES, cx, |_, path, _| set_state(|s| s.cover = Some(path)));
}

fn cover_field(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(path) = s.cover.clone() else {
        return div()
            .flex()
            .child(button("event-cover", crate::t!("event.cover_upload"), BtnKind::Primary).h(z(32.)).text_size(z(14.)).on_click(cx.listener(|_, _, _, cx| pick_cover(cx))))
            .into_any_element();
    };
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id("event-cover-preview")
                .group("event-cover")
                .relative()
                .w_full()
                .h(z(138.))
                .mb(z(8.))
                .cursor_pointer()
                .on_click(cx.listener(|_, _, _, cx| pick_cover(cx)))
                .child(div().size_full().rounded(z(8.)).overflow_hidden().child(gpui::img(path).size_full().object_fit(gpui::ObjectFit::Cover)))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(z(8.))
                        .bg(gpui::hsla(0., 0., 0., 0.6))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(z(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .opacity(0.)
                        .group_hover("event-cover", |d| d.opacity(1.))
                        .child(crate::t!("event.cover_change")),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .size(z(28.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(gpui::img(SharedString::from(COVER_EDIT_ICON)).size(z(18.))),
                ),
        )
        .child(
            div().flex().child(
                button("event-cover-remove", crate::t!("event.cover_remove"), BtnKind::Primary).h(z(32.)).text_size(z(14.)).on_click(cx.listener(|_, _, _, cx| {
                    set_state(|s| s.cover = None);
                    cx.notify();
                })),
            ),
        )
        .into_any_element()
}

fn pick_date(which: Open) -> Rc<dyn Fn(CalPick)> {
    Rc::new(move |pk| {
        set_state(|s| match pk {
            CalPick::Day(d) => {
                let cur = if which == Open::EndDate { s.end.unwrap_or(s.start) } else { s.start };
                let t = local(cur).time();
                let at = Local.from_local_datetime(&d.and_time(t)).earliest().map(|x| x.timestamp()).unwrap_or(cur);
                if which == Open::EndDate {
                    s.end = Some(at);
                } else {
                    s.set_start(at);
                }
                s.open = None;
                s.month = None;
            }
            CalPick::Month(m) => s.month = Some(m),
        })
    })
}

impl State {
    fn set_start(&mut self, at: i64) {
        self.start = at;
        if let Some(e) = self.end
            && e < at
        {
            self.end = Some(at + 3600);
        }
    }
}

fn date_time(s: &State, now: i64, end: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (date_open, time_open) = if end { (Open::EndDate, Open::EndTime) } else { (Open::StartDate, Open::StartTime) };
    let at = if end { s.end.unwrap_or(s.start) } else { s.start };
    let cur = local(at);
    let today = local(now).date_naive();
    let (first, last) = if end { (local(s.start).date_naive(), today + Duration::days(366)) } else { (today, today + Duration::days(365)) };
    let last = if s.recur != Recur::None { last + Duration::days(4 * 365 + 1) } else { last };
    let shown = if end { s.end.is_some() } else { true };
    let date_label = if end { crate::t!("event.end_date") } else { crate::t!("event.start_date") };
    let time_label = if end { crate::t!("event.end_time") } else { crate::t!("event.start_time") };
    let time_value: SharedString = if shown && slots(cur.date_naive()).contains(&at) { crate::i18n::clock(cur).into() } else { SharedString::default() };
    let date_value: SharedString = if shown { crate::i18n::medium_date(cur.date_naive()).into() } else { SharedString::default() };
    let opts = slots(cur.date_naive()).into_iter().map(|t| (SharedString::from(crate::i18n::clock(local(t))), t, t == at)).collect();
    let date = div()
        .flex_1()
        .w_0()
        .mr(z(8.))
        .relative()
        .flex()
        .flex_col()
        .child(field_label(date_label, true))
        .child(date_field(if end { "event-end-date" } else { "event-start-date" }, date_value).when(s.open == Some(date_open), |d| d.border_color(c(tok(idx::INPUT_BORDER_ACTIVE)))).on_click(cx.listener(toggle(date_open))))
        .when(s.open == Some(date_open), |d| {
            d.child(div().relative().child(calendar_view(cur.date_naive(), s.month, first, Some(last), now, pick_date(date_open), cx)))
        });
    let time = div()
        .flex_1()
        .w_0()
        .ml(z(8.))
        .relative()
        .flex()
        .flex_col()
        .child(field_label(time_label, true))
        .child(select_field(if end { "event-end-time" } else { "event-start-time" }, time_value).when(s.open == Some(time_open), |d| d.border_color(c(tok(idx::INPUT_BORDER_ACTIVE)))).on_click(cx.listener(toggle(time_open))))
        .when(s.open == Some(time_open), |d| {
            d.child(dropdown(
                opts,
                Rc::new(move |t| {
                    set_state(|s| {
                        if end {
                            s.end = Some(t);
                        } else {
                            s.set_start(t);
                        }
                        s.open = None;
                    })
                }),
                cx,
            ))
        });
    div().relative().flex().flex_row().mb(z(16.)).child(date).child(time).into_any_element()
}

fn info_step(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let now = Local::now().timestamp();
    let sched_err = schedule_error(s.kind, s.start, s.end, now);
    let recur = recur_options(s.start);
    let recur_label = recur.iter().find(|(r, _)| *r == s.recur).map(|(_, l)| l.clone()).unwrap_or_default();
    let recur_open = s.open == Some(Open::Recur);
    let recur_opts = recur.iter().enumerate().map(|(n, (r, l))| (l.clone(), n as i64, *r == s.recur)).collect();
    let kinds: Vec<Recur> = recur.iter().map(|(r, _)| *r).collect();
    let schedule = div()
        .mt(z(24.))
        .w_full()
        .flex()
        .flex_col()
        .child(date_time(s, now, false, cx))
        .when(s.kind == Some(Kind::External), |d| d.child(date_time(s, now, true, cx)))
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .child(field_label(crate::t!("event.frequency"), true))
                .child(select_field("event-recur", recur_label).when(recur_open, |d| d.border_color(c(tok(idx::INPUT_BORDER_ACTIVE)))).on_click(cx.listener(toggle(Open::Recur))))
                .when(recur_open, |d| {
                    d.child(dropdown(
                        recur_opts,
                        Rc::new(move |n| {
                            let r = kinds.get(n as usize).copied().unwrap_or(Recur::None);
                            set_state(|s| {
                                s.recur = r;
                                s.open = None;
                            })
                        }),
                        cx,
                    ))
                }),
        );
    let cover = div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(text(crate::t!("event.cover"), 16., 20., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM))
        .child(text(crate::t!("event.cover_hint"), 14., 18., idx::TEXT_SUBTLE))
        .child(cover_field(s, cx));
    let form = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(div().flex().flex_col().child(field_label(crate::t!("event.topic"), true)).children(s.topic.as_ref().map(input_box)))
        .child(schedule)
        .when_some(sched_err, |d, e| d.child(text(crate::t!(e), 12., 16., idx::TEXT_FEEDBACK_CRITICAL).mb(z(8.))))
        .child(
            div()
                .flex()
                .flex_col()
                .child(field_label(crate::t!("event.description"), false))
                .children(s.description.as_ref().map(|a| {
                    let empty = a.read(cx).value().is_empty();
                    div()
                        .relative()
                        .min_h(z(44.))
                        .px(z(10.))
                        .py(z(12.))
                        .rounded(z(8.))
                        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                        .border_1()
                        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                        .text_size(z(16.))
                        .when(empty, |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .top(z(12.))
                                    .left(z(10.))
                                    .right(z(10.))
                                    .text_size(z(16.))
                                    .line_height(z(20.))
                                    .text_color(c(tok(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT)))
                                    .child(crate::t!("event.description_ph")),
                            )
                        })
                        .child(Textarea::new(a).appearance(false).bordered(false))
                })),
        )
        .child(cover);
    div()
        .flex()
        .flex_col()
        .child(div().my(z(16.)).child(heading(crate::t!("event.info_title"), crate::t!("event.info_sub"))))
        .child(form)
        .into_any_element()
}

fn review_step(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = value(&s.topic, TOPIC_MAX, cx);
    let description = s.description.as_ref().map(|a| a.read(cx).value().chars().take(DESCRIPTION_MAX).collect::<String>()).unwrap_or_default();
    let location = (s.kind == Some(Kind::External)).then(|| value(&s.location, LOCATION_MAX, cx));
    let ev = crate::commands::GuildEvent {
        name,
        description,
        start: s.start,
        end: s.end,
        status: 1,
        channel: if s.kind == Some(Kind::External) { None } else { s.channel },
        location,
        creator: Some(app.me.id),
        ..Default::default()
    };
    let card = crate::ui::events::card(app, s.guild, &ev).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).border_color(c(tok(idx::BORDER_SUBTLE)));
    div()
        .pt(z(24.))
        .flex()
        .flex_col()
        .child(card)
        .child(
            div()
                .pt(z(24.))
                .flex()
                .flex_col()
                .items_center()
                .child(text(crate::t!("event.preview_title"), 24., 32., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD))
                .child(text(crate::t!("event.preview_auto"), 16., 20., idx::TEXT_SUBTLE).mt(z(8.)).mb(z(16.)).text_center())
                .when_some(s.error.clone(), |d, e| d.child(text(e, 12., 16., idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.)).mb(z(16.)).text_center())),
        )
        .into_any_element()
}

pub fn validity(s: &State, cx: &gpui::App) -> [bool; 3] {
    let now = Local::now().timestamp();
    let location = value(&s.location, LOCATION_MAX, cx);
    let loc = if s.kind == Some(Kind::External) { location } else { String::new() };
    let channel = if s.kind == Some(Kind::External) { None } else { s.channel };
    [
        entity_error(s.kind, channel, &loc).is_none(),
        schedule_error(s.kind, s.start, s.end, now).is_none() && topic_error(&value(&s.topic, TOPIC_MAX, cx)).is_none(),
        true,
    ]
}

fn success_slide(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let link = s.link.clone();
    let copy = link.clone();
    div()
        .relative()
        .w(z(440.))
        .p(z(8.))
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .id("event-success-close")
                .absolute()
                .top(z(4.))
                .right(z(4.))
                .cursor_pointer()
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .hover(|d| d.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                .on_click(cx.listener(|_, _, _, cx| {
                    close_modal();
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path("close-small")).size(z(24.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        )
        .child(
            div()
                .size(z(64.))
                .rounded_full()
                .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("calendar")).size(z(30.)).text_color(c(tok(idx::TEXT_STRONG)))),
        )
        .child(text(crate::t!("event.success_title"), 24., 32., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD).mt(z(24.)).text_center())
        .child(text(crate::t!("event.success_body"), 16., 20., idx::TEXT_DEFAULT).mt(z(8.)).text_center())
        .child(
            div()
                .mt(z(24.))
                .w_full()
                .h(z(40.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                .flex()
                .items_center()
                .child(div().flex_1().min_w_0().pl(z(10.)).mr(z(10.)).truncate().text_size(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(link.unwrap_or_default()))
                .child(
                    button("event-success-copy", crate::t!("common.copy"), BtnKind::Primary)
                        .h(z(32.))
                        .mr(z(4.))
                        .text_size(z(14.))
                        .when(copy.is_none(), |d| d.opacity(0.5).cursor_default())
                        .on_click(cx.listener(move |a, _, _, cx| {
                            if let Some(l) = &copy {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(l.to_string()));
                                a.parity_toast(crate::t!("gs.eng.copied"), cx);
                            }
                        })),
                ),
        )
        .child(text(crate::t!("invite.expires"), 12., 16., idx::TEXT_DEFAULT).mt(z(8.)).w_full())
        .into_any_element()
}

const CONFETTI: usize = 160;
const CONFETTI_COLORS: [usize; 4] = [idx::TEXT_BRAND, idx::TEXT_FEEDBACK_POSITIVE, idx::TEXT_FEEDBACK_WARNING, idx::TEXT_FEEDBACK_CRITICAL];

pub fn confetti_at(n: usize, width: f32, frame: f32) -> (f32, f32) {
    let mut seed = (n as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_000) as f32 / 10_000.
    };
    let right = n % 2 == 1;
    let ox = if right { width } else { 0. } - 100. + 200. * rnd();
    let oy = -100. + 200. * rnd();
    let speed_x = 20. + 40. * rnd();
    let mut vx = if right { speed_x } else { -speed_x } * 0.25;
    let mut vy = -(20. + 40. * rnd()) * 0.25;
    let (mut x, mut y) = (ox, oy);
    let mut f = 0.;
    while f < frame {
        vx *= 0.96;
        vy = vy * 0.96 + 0.25;
        x += vx;
        y += vy;
        f += 1.;
    }
    (x, y)
}

fn confetti(width: f32) -> AnyElement {
    use gpui::{Animation, AnimationExt};
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .with_animation("event-confetti", Animation::new(std::time::Duration::from_millis(3000)), move |d, t| {
            let frame = t * 180.;
            let fade = if t > 0.8 { (1. - t) / 0.2 } else { 1. };
            d.children((0..CONFETTI).map(|n| {
                let (x, y) = confetti_at(n, width, frame);
                let size = if n % 3 == 0 { 6. } else { 8. };
                div()
                    .absolute()
                    .left(z(x))
                    .top(z(y))
                    .w(z(size))
                    .h(z(if n % 2 == 0 { size } else { size / 2. }))
                    .when(n % 4 == 0, |d| d.rounded_full())
                    .bg(c(tok(CONFETTI_COLORS[n % CONFETTI_COLORS.len()])))
                    .opacity(fade)
            }))
        })
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if s.done {
        let close = crate::ui::modals::close_action(cx);
        let el = div()
            .relative()
            .w(z(482.))
            .rounded(z(12.))
            .bg(c(tok(idx::MODAL_BACKGROUND)))
            .border_1()
            .border_color(c(tok(idx::BORDER_NORMAL)))
            .child(div().p(z(16.)).child(success_slide(s, cx)))
            .when(!crate::ui::settings::effects::reduced_motion(), |d| d.child(confetti(482.)));
        return crate::ui::modal::frame("event-backdrop", close, el);
    }
    let valid = validity(s, cx);
    let ok = valid.iter().take(s.step + 1).all(|v| *v);
    let slide = match s.step {
        0 => location_step(app, s, cx),
        1 => info_step(s, cx),
        _ => review_step(app, s, cx),
    };
    let content = div().id("event-content").p(z(16.)).flex().flex_col().max_h(z(620.)).overflow_y_scroll().child(steps(s, valid, cx)).child(div().w(z(440.)).child(slide));
    let primary = if s.step == 2 { crate::t!("menu.create_event") } else { crate::t!("event.next") };
    let saving = s.saving;
    let footer = div()
        .px(z(24.))
        .py(z(16.))
        .rounded_b(z(12.))
        .bg(c(tok(idx::MODAL_FOOTER_BACKGROUND)))
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .child(if s.step > 0 {
            div()
                .flex()
                .items_center()
                .py(z(3.))
                .px(z(11.))
                .child(
                    div()
                        .id("event-back")
                        .cursor_pointer()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .hover(|d| d.underline())
                        .on_click(cx.listener(|_, _, _, cx| {
                            set_state(|s| {
                                s.step = s.step.saturating_sub(1);
                                s.open = None;
                            });
                            cx.notify();
                        }))
                        .child(crate::t!("event.back")),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        })
        .child(
            div()
                .ml_auto()
                .flex()
                .items_center()
                .child(button("event-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
                    close_modal();
                    cx.notify();
                })))
                .child(
                    button("event-next", primary, BtnKind::Primary)
                        .ml(z(8.))
                        .when(!ok || saving, |d| d.opacity(0.5).cursor_default())
                        .on_click(cx.listener(|a, _, _, cx| a.event_next(cx))),
                ),
        );
    let close = crate::ui::modals::close_action(cx);
    let el = div()
        .relative()
        .w(z(482.))
        .rounded(z(12.))
        .bg(c(tok(idx::MODAL_BACKGROUND)))
        .border_1()
        .border_color(c(tok(idx::BORDER_NORMAL)))
        .flex()
        .flex_col()
        .child(content)
        .child(footer);
    crate::ui::modal::frame("event-backdrop", close, el)
}

impl PhoenixApp {
    pub fn open_create_event(&mut self, guild: Id, cx: &mut Context<Self>) {
        let state = State {
            guild,
            step: 0,
            kind: None,
            channel: None,
            start: default_start(Local::now()),
            end: None,
            recur: Recur::None,
            open: None,
            month: None,
            location: None,
            topic: None,
            description: None,
            saving: false,
            error: None,
            cover: None,
            done: false,
            link: None,
        };
        open_modal(Modal::CreateEvent(Box::new(state)), None);
        cx.notify();
    }

    pub fn set_event_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        let guild = with_modal(|m| match m {
            Some(Modal::CreateEvent(s)) => Some(s.guild),
            _ => None,
        });
        let Some(guild) = guild else { return };
        let first = kind_channel(kind).and_then(|k| voice_channels(self, guild, k).first().map(|(id, _)| *id));
        set_state(|s| {
            s.kind = Some(kind);
            s.channel = first;
            s.open = None;
            s.end = (kind == Kind::External).then_some(s.start + 7200);
        });
        cx.notify();
    }

    pub fn event_next(&mut self, cx: &mut Context<Self>) {
        let Some((ok, step, saving)) = with_modal(|m| match m {
            Some(Modal::CreateEvent(s)) => Some((validity(s, cx).iter().take(s.step + 1).all(|v| *v), s.step, s.saving)),
            _ => None,
        }) else {
            return;
        };
        if !ok || saving {
            return;
        }
        if step < 2 {
            set_state(|s| {
                s.step += 1;
                s.open = None;
            });
            cx.notify();
            return;
        }
        let Some(req) = with_modal(|m| match m {
            Some(Modal::CreateEvent(s)) => create_request(s, cx),
            _ => None,
        }) else {
            return;
        };
        set_state(|s| {
            s.saving = true;
            s.error = None;
        });
        let target = with_modal(|m| match m {
            Some(Modal::CreateEvent(s)) => Some((s.guild, if s.kind == Some(Kind::External) { None } else { s.channel })),
            _ => None,
        });
        self.action(req, cx, move |a, r, cx| {
            match r {
                Ok(reply) => {
                    let event = match &reply {
                        crate::actions::ActionReply::Json(v) => v.get("id").and_then(|v| v.as_str()).and_then(|v| v.parse().ok()).map(Id),
                        _ => None,
                    };
                    let (guild, channel) = target.unwrap_or((Id(0), None));
                    let invite_channel = channel.or_else(|| default_text_channel(a, guild));
                    let can = crate::ui::inbox::has_perm(a.backend.guild_permissions(guild), CREATE_INVITE);
                    let public = invite_channel.is_some_and(|ch| !is_locked(a, ch));
                    if let Some(ch) = invite_channel {
                        a.nav_request(crate::ui::home::data::NavRequest::CreateInvite(ch), cx, move |_, res, cx| {
                            if let Ok(crate::ui::home::data::NavReply::Text(url)) = res {
                                let code = url.rsplit('/').next().unwrap_or_default().to_owned();
                                set_state(|s| s.link = Some(event_link(&code, event).into()));
                            }
                            cx.notify();
                        });
                    }
                    if can && public {
                        set_state(|s| {
                            s.saving = false;
                            s.done = true;
                        });
                    } else {
                        close_modal();
                        a.parity_toast(crate::t!("event.created"), cx);
                    }
                }
                Err(e) => set_state(|s| {
                    s.saving = false;
                    s.error = Some(e.0.into());
                }),
            }
            cx.notify();
        });
        cx.notify();
    }
}

pub fn create_request(s: &State, cx: &gpui::App) -> Option<ActionRequest> {
    let place = match s.kind? {
        Kind::External => EventPlace::External(value(&s.location, LOCATION_MAX, cx)),
        Kind::Voice => EventPlace::Voice(s.channel?),
        Kind::Stage => EventPlace::Stage(s.channel?),
    };
    Some(ActionRequest::CreateEvent {
        guild: s.guild,
        name: value(&s.topic, TOPIC_MAX, cx),
        description: s.description.as_ref().map(|a| a.read(cx).value().chars().take(DESCRIPTION_MAX).collect()).unwrap_or_default(),
        start_unix: s.start.max(0) as u64,
        end_unix: s.end.map(|e| e.max(0) as u64),
        place,
        recurrence: recurrence_rule(s.recur, s.start),
        image: s.cover.as_deref().and_then(crate::ui::file_pick::data_uri),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_start_rounds_like_the_client() {
        let at = |h, m| Local.with_ymd_and_hms(2026, 11, 25, h, m, 0).unwrap();
        assert_eq!(default_start(at(10, 10)), at(11, 0).timestamp());
        assert_eq!(default_start(at(10, 40)), at(12, 0).timestamp());
    }

    #[test]
    fn validation_follows_the_client() {
        assert_eq!(entity_error(None, None, ""), Some("event.err_type"));
        assert_eq!(entity_error(Some(Kind::External), None, "  "), Some("event.err_place"));
        assert_eq!(entity_error(Some(Kind::Voice), Some(Id(1)), ""), None);
        assert_eq!(schedule_error(Some(Kind::External), 100, None, 0), Some("event.err_end_required"));
        assert_eq!(schedule_error(Some(Kind::Voice), 10, None, 50), Some("event.err_start_past"));
        assert_eq!(schedule_error(Some(Kind::External), 100, Some(90), 0), Some("event.err_end_before"));
        assert_eq!(schedule_error(Some(Kind::Voice), 100, None, 0), None);
        assert_eq!(topic_error(" "), Some("event.err_topic"));
    }

    #[test]
    fn event_links_carry_the_event_id() {
        assert_eq!(event_link("abc", Some(Id(42))), "https://discord.gg/abc?event=42");
        assert_eq!(event_link("abc", None), "https://discord.gg/abc");
    }

    #[test]
    fn confetti_rises_then_falls_from_both_corners() {
        let (lx, ly) = confetti_at(0, 482., 0.);
        let (rx, _) = confetti_at(1, 482., 0.);
        assert!(lx < 100. && rx > 382.);
        let (lx2, ly2) = confetti_at(0, 482., 20.);
        assert!(lx2 < lx && ly2 < ly);
        let (_, ly3) = confetti_at(0, 482., 180.);
        assert!(ly3 > ly2);
    }

    #[test]
    fn recurrence_matches_rrule_options() {
        let start = Utc.with_ymd_and_hms(2026, 11, 25, 18, 0, 0).unwrap().timestamp();
        assert!(recurrence_rule(Recur::None, start).is_none());
        let w = recurrence_rule(Recur::Biweekly, start).unwrap();
        assert_eq!((w["frequency"].clone(), w["interval"].clone(), w["by_weekday"].clone()), (json!(2), json!(2), json!([2])));
        assert_eq!(w["start"], "2026-11-25T18:00:00.000Z");
        assert!(w.get("by_n_weekday").is_none());
        let m = recurrence_rule(Recur::Monthly, start).unwrap();
        assert_eq!(m["by_n_weekday"], json!([{"n": 4, "day": 2}]));
        assert_eq!(m["by_weekday"], Value::Null);
        let y = recurrence_rule(Recur::Yearly, start).unwrap();
        assert_eq!((y["by_month"].clone(), y["by_month_day"].clone(), y["frequency"].clone()), (json!([11]), json!([25]), json!(0)));
        assert_eq!(recurrence_rule(Recur::Daily, start).unwrap()["frequency"], 3);
    }
}
