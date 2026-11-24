use chrono::{Datelike, Days, Months, NaiveDate};
use gpui::{AnyElement, App, Context, Entity, FontWeight, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, button_icon},
};

const FIELDS: [(&str, &str, &str); 6] = [
    ("from:", "search.m_from", "search.m_from_desc"),
    ("in:", "search.m_in", "search.m_in_desc"),
    ("has:", "search.m_has", "search.m_has_desc"),
    ("mentions:", "search.m_mentions", "search.m_mentions_desc"),
    ("authorType:", "search.m_author", "search.m_author_desc"),
    ("pinned:", "search.m_pinned", "search.m_pinned_desc"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateOp {
    Before,
    After,
    During,
}

const DATE_OPS: [DateOp; 3] = [DateOp::Before, DateOp::After, DateOp::During];

impl DateOp {
    fn key(self) -> &'static str {
        match self {
            Self::Before => "before:",
            Self::After => "after:",
            Self::During => "during:",
        }
    }
    fn label(self) -> SharedString {
        match self {
            Self::Before => crate::t!("search.m_date_before"),
            Self::After => crate::t!("search.m_date_after"),
            Self::During => crate::t!("search.m_date_during"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DateRow {
    op: DateOp,
    day: NaiveDate,
}

pub struct FiltersModal {
    fields: Vec<Entity<InputState>>,
    picks: Vec<Vec<String>>,
    list: Option<usize>,
    _subs: Vec<gpui::Subscription>,
    dates: Vec<DateRow>,
    calendar: Option<(usize, NaiveDate)>,
    op_menu: Option<usize>,
}

thread_local! {
    static ANCHOR: std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>> = const { std::cell::Cell::new(None) };
    static CARD: std::cell::Cell<Option<gpui::Point<gpui::Pixels>>> = const { std::cell::Cell::new(None) };
}

fn probe(card: bool) -> impl IntoElement {
    gpui::canvas(
        move |b, window, _| {
            let changed =
                if card { CARD.with(|c| c.replace(Some(b.origin))) != Some(b.origin) } else { ANCHOR.with(|c| c.replace(Some(b))) != Some(b) };
            if changed {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

fn with_modal(cx: &mut App, f: impl FnOnce(&mut FiltersModal)) {
    if let Some(m) = cx.default_global::<Filters>().0.as_mut() {
        f(m);
    }
}

#[derive(Default)]
pub struct Filters(pub Option<FiltersModal>);

impl gpui::Global for Filters {}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Filters>().is_some_and(|f| f.0.is_some())
}

fn placeholder(app: &PhoenixApp, key: &str) -> SharedString {
    match key {
        "from:" | "mentions:" => crate::t!("search.m_ph_user", u = app.me.name),
        "in:" => {
            let ch = app.selected_channel.and_then(|c| app.channels.get(&c)).map(|c| c.name.to_string()).unwrap_or_default();
            crate::t!("search.m_ph_channel", c = ch)
        }
        "has:" => crate::t!("search.m_ph_has"),
        "authorType:" => crate::t!("search.m_ph_author"),
        _ => crate::t!("search.m_ph_pinned"),
    }
}

pub fn open(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let mut fields = Vec::with_capacity(FIELDS.len());
    let mut subs = Vec::with_capacity(FIELDS.len());
    for (i, (k, _, _)) in FIELDS.into_iter().enumerate() {
        let ph = placeholder(app, k);
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(ph));
        subs.push(cx.subscribe_in(&input, window, move |_, inp, ev: &InputEvent, _, cx| {
            if let InputEvent::Change = ev
                && !inp.read(cx).value().is_empty()
            {
                with_modal(cx, |m| open_list(m, i));
                cx.notify();
            }
        }));
        fields.push(input);
    }
    cx.default_global::<Filters>().0 = Some(FiltersModal {
        fields,
        picks: vec![Vec::new(); FIELDS.len()],
        list: None,
        _subs: subs,
        dates: Vec::new(),
        calendar: None,
        op_menu: None,
    });
    cx.notify();
}

pub fn add_date(cx: &mut App, open_calendar: bool) {
    with_modal(cx, |m| {
        m.dates.push(DateRow { op: DateOp::Before, day: today() });
        m.calendar = open_calendar.then(|| (m.dates.len() - 1, month_start(today())));
        m.op_menu = None;
        m.list = None;
    });
}

pub fn show_list(cx: &mut App, i: usize) {
    with_modal(cx, |m| {
        if i < FIELDS.len() {
            open_list(m, i);
        }
    });
}

pub fn close_popover(cx: &mut Context<PhoenixApp>) -> bool {
    let mut closed = false;
    with_modal(cx, |m| {
        closed = m.calendar.take().is_some() | m.op_menu.take().is_some() | m.list.take().is_some();
    });
    if closed {
        cx.notify();
    }
    closed
}

pub fn close(cx: &mut Context<PhoenixApp>) {
    cx.default_global::<Filters>().0 = None;
    cx.notify();
}


fn quoted(v: &str) -> String {
    let v = v.trim();
    if v.contains(' ') { format!("\"{v}\"") } else { v.to_string() }
}

fn tokens(m: &FiltersModal) -> String {
    let mut out = Vec::new();
    for ((key, _, _), picks) in FIELDS.iter().zip(&m.picks) {
        for v in picks {
            out.push(format!("{key}{}", quoted(v)));
        }
    }
    for d in &m.dates {
        out.push(format!("{}{}", d.op.key(), d.day.format("%Y-%m-%d")));
    }
    out.join(" ")
}

fn open_list(m: &mut FiltersModal, i: usize) {
    m.list = Some(i);
    m.calendar = None;
    m.op_menu = None;
}

#[derive(Clone)]
enum Lead {
    Avatar(Id),
    Channel(ChannelKind),
    Glyph(&'static str),
    None,
}

#[derive(Clone)]
struct Opt {
    value: String,
    label: SharedString,
    sub: Option<SharedString>,
    lead: Lead,
}

const HAS: [(&str, &str, &str); 8] = [
    ("image", "search.has_image", "image"),
    ("video", "search.has_video", "voice/video-small"),
    ("link", "search.has_link", "link"),
    ("file", "search.has_file", "file"),
    ("embed", "search.has_embed", "angle-brackets"),
    ("sound", "search.has_sound", "music"),
    ("sticker", "search.has_sticker", "sticker"),
    ("poll", "search.has_poll", "chat-check"),
];

fn score(name: &str, q: &str) -> Option<u8> {
    if q.is_empty() {
        return Some(3);
    }
    let n = name.to_lowercase();
    if n == q {
        Some(0)
    } else if n.starts_with(q) {
        Some(1)
    } else if n.contains(q) {
        Some(2)
    } else {
        None
    }
}

fn options(app: &PhoenixApp, key: &str, query: &str) -> Vec<Opt> {
    let q = query.trim().to_lowercase();
    match key {
        "from:" | "mentions:" => {
            let mut ids: Vec<Id> = vec![app.me.id];
            if let Some(msgs) = app.selected_channel.and_then(|c| app.messages.get(&c)) {
                ids.extend(msgs.msgs.iter().rev().take(200).map(|m| m.author));
            }
            let nick = |u: Id| {
                app.selected_guild.and_then(|g| app.members.get(&g)).and_then(|ms| ms.iter().find(|m| m.user == u)).and_then(|m| m.nick.clone())
            };
            if let Some(g) = app.selected_guild {
                ids.extend(app.members.get(&g).into_iter().flatten().map(|m| m.user));
            }
            let mut seen = std::collections::HashSet::new();
            let mut out: Vec<(u8, usize, Opt)> = Vec::new();
            for (order, id) in ids.into_iter().enumerate() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(u) = (if id == app.me.id { Some(&app.me) } else { app.users.get(&id) }) else { continue };
                let shown = nick(id).unwrap_or_else(|| if u.display_name.is_empty() { u.name.clone() } else { u.display_name.clone() });
                let Some(sc) = [score(&shown, &q), score(&u.name, &q)].into_iter().flatten().min() else { continue };
                out.push((sc, order, Opt { value: u.name.to_string(), label: shown, sub: Some(u.name.clone()), lead: Lead::Avatar(id) }));
            }
            out.sort_by_key(|(sc, order, _)| (*sc, *order));
            out.into_iter().take(50).map(|(_, _, o)| o).collect()
        }
        "in:" => {
            let Some(g) = app.selected_guild else { return Vec::new() };
            let mut list: Vec<(u8, usize, Opt)> = Vec::new();
            for (order, ch) in app.guild_channels.get(&g).into_iter().flatten().filter_map(|id| app.channels.get(id)).enumerate() {
                if matches!(ch.kind, ChannelKind::Category | ChannelKind::Voice | ChannelKind::Stage) {
                    continue;
                }
                let Some(sc) = score(&ch.name, &q) else { continue };
                let first = Some(ch.id) == app.selected_channel;
                list.push((sc, if first { 0 } else { order + 1 }, Opt { value: ch.name.to_string(), label: ch.name.clone(), sub: None, lead: Lead::Channel(ch.kind) }));
            }
            list.sort_by_key(|(sc, order, _)| (*sc, *order));
            list.into_iter().map(|(_, _, o)| o).collect()
        }
        "has:" => HAS
            .iter()
            .filter_map(|(v, label, glyph)| {
                let l = crate::i18n::t(label, &[]);
                score(&l, &q).or_else(|| score(v, &q)).map(|_| Opt { value: (*v).into(), label: l, sub: None, lead: Lead::Glyph(glyph) })
            })
            .collect(),
        "authorType:" => [("user", "search.author_user"), ("bot", "search.author_bot"), ("webhook", "search.author_webhook")]
            .into_iter()
            .filter_map(|(v, label)| {
                let l = crate::i18n::t(label, &[]);
                score(&l, &q).map(|_| Opt { value: v.into(), label: l, sub: None, lead: Lead::None })
            })
            .collect(),
        _ => [("true", "search.pinned_true"), ("false", "search.pinned_false")]
            .into_iter()
            .map(|(v, label)| Opt { value: v.into(), label: crate::i18n::t(label, &[]), sub: None, lead: Lead::None })
            .collect(),
    }
}

fn pick_label(app: &PhoenixApp, key: &str, v: &str) -> SharedString {
    options(app, key, "").into_iter().find(|o| o.value == v).map(|o| o.label).unwrap_or_else(|| SharedString::from(v.to_string()))
}

fn label(title: SharedString, desc: SharedString) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(title))
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(desc))
}

fn field(app: &PhoenixApp, i: usize, input: &Entity<InputState>, picks: &[String], open: bool, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let key = FIELDS[i].0;
    let select = key == "pinned:";
    let mut content = div().flex_1().min_w_0().h_full().pl(z(12.)).pr(z(8.)).flex().items_center().gap(z(4.)).overflow_hidden();
    if select {
        content = content.child(match picks.first() {
            Some(v) => div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(pick_label(app, key, v)),
            None => div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(placeholder(app, key)),
        });
    } else {
        for (pi, v) in picks.iter().enumerate() {
            let val = v.clone();
            content = content.child(
                div()
                    .id(gpui::ElementId::Name(format!("search-pill-{i}-{pi}").into()))
                    .flex_none()
                    .px(z(6.))
                    .py(z(2.))
                    .rounded(z(4.))
                    .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .cursor_pointer()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .whitespace_nowrap()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
                        with_modal(cx, |m| {
                            m.picks[i].retain(|x| x != &val);
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(pick_label(app, key, v))
                    .child(div().text_size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).child("×")),
            );
        }
        content = content.child(div().flex_1().min_w(z(40.)).child(Input::new(input).appearance(false).bordered(false).px(z(0.)).text_size(z(16.)).line_height(z(20.))));
    }
    let wrap = div()
        .id(gpui::ElementId::Name(format!("search-field-{i}").into()))
        .relative()
        .h(z(40.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if open { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT })))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .when(select, |d| d.cursor_pointer())
        .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
            with_modal(cx, |m| if select && m.list == Some(i) { m.list = None } else { open_list(m, i) });
            cx.stop_propagation();
            cx.notify();
        }))
        .child(content)
        .child(
            div()
                .w(z(36.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .pr(z(4.))
                .child(gpui::svg().path(crate::icons::path(if open { "chevron-up" } else { "chevron" })).size(z(20.)).text_color(c(tok(idx::TEXT_STRONG)))),
        );
    div().mt(z(8.)).relative().child(wrap).when(open, |d| d.child(probe(false)))
}

fn option_list(app: &PhoenixApp, i: usize, width: gpui::Pixels, cx: &mut Context<PhoenixApp>) -> Option<gpui::Stateful<gpui::Div>> {
    let m = cx.try_global::<Filters>()?.0.as_ref()?;
    let key = FIELDS[i].0;
    let query = m.fields[i].read(cx).value().to_string();
    let picked = m.picks[i].clone();
    let opts = options(app, key, if key == "pinned:" { "" } else { &query });
    let single = key == "pinned:";
    let rows: Vec<AnyElement> = opts
        .into_iter()
        .enumerate()
        .map(|(k, o)| {
            let on = picked.contains(&o.value);
            let lead: Option<AnyElement> = match o.lead {
                Lead::Avatar(u) => Some(crate::ui::avatar(if u == app.me.id { Some(&app.me) } else { app.users.get(&u) }, 24.)),
                Lead::Channel(kind) => Some(
                    gpui::svg().path(crate::ui::sidebar::channel_icon(kind).path()).flex_none().size(z(20.)).text_color(c(tok(idx::CHANNEL_ICON))).into_any_element(),
                ),
                Lead::Glyph(g) => Some(gpui::svg().path(crate::icons::path(g)).flex_none().size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element()),
                Lead::None => None,
            };
            let value = o.value.clone();
            let check = if single {
                on.then(|| gpui::svg().path(crate::icons::path("check")).flex_none().size(z(16.)).text_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))).into_any_element())
            } else {
                Some(
                    div()
                        .size(z(20.))
                        .flex_none()
                        .rounded(z(4.))
                        .border_1()
                        .border_color(c(tok(if on { idx::CHECKBOX_BORDER_SELECTED_DEFAULT } else { idx::CHECKBOX_BORDER_DEFAULT })))
                        .bg(c(tok(if on { idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT } else { idx::CHECKBOX_BACKGROUND_DEFAULT })))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(c(tok(idx::CHECKBOX_ICON_ACTIVE)))))
                        .into_any_element(),
                )
            };
            div()
                .id(gpui::ElementId::Name(format!("search-opt-{i}-{k}").into()))
                .flex_none()
                .h(z(40.))
                .pt(z(3.))
                .pb(z(3.))
                .pl(z(8.))
                .pr(z(4.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .on_click(cx.listener(move |_, _, w, cx| {
                    let mut clear = None;
                    with_modal(cx, |m| {
                        let p = &mut m.picks[i];
                        if single {
                            *p = vec![value.clone()];
                            m.list = None;
                        } else if let Some(at) = p.iter().position(|x| *x == value) {
                            p.remove(at);
                        } else {
                            p.push(value.clone());
                        }
                        clear = (!single).then(|| m.fields[i].clone());
                    });
                    if let Some(inp) = clear {
                        inp.update(cx, |s, cx| s.set_value("", w, cx));
                        with_modal(cx, |m| m.list = Some(i));
                    }
                    cx.notify();
                }))
                .children(lead)
                .child(div().flex_1().min_w_0().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).truncate().child(o.label))
                .children(o.sub.map(|u| div().flex_none().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(u)))
                .children(check)
                .into_any_element()
        })
        .collect();
    let n = rows.len();
    let body: AnyElement = if n == 0 {
        div().h(z(40.)).px(z(8.)).flex().items_center().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("search.m_no_options")).into_any_element()
    } else {
        div().id(gpui::ElementId::Name(format!("search-opts-{i}").into())).max_h(z(200.)).overflow_y_scroll().flex().flex_col().children(rows).into_any_element()
    };
    Some(
        div()
            .id(gpui::ElementId::Name(format!("search-list-{i}").into()))
            .w(width)
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0., 0., 0., 0.16),
                offset: gpui::point(z(0.), z(2.)),
                blur_radius: z(8.),
                spread_radius: z(0.),
                inset: false,
            }])
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(body),
    )
}

fn list_h(app: &PhoenixApp, i: usize, cx: &App) -> f32 {
    let Some(m) = cx.try_global::<Filters>().and_then(|f| f.0.as_ref()) else { return 0. };
    let key = FIELDS[i].0;
    let q = m.fields[i].read(cx).value().to_string();
    let n = options(app, key, if key == "pinned:" { "" } else { &q }).len().clamp(1, 5);
    40. * n as f32 + 10.
}

fn date_section(dates: &[DateRow], calendar: Option<(usize, NaiveDate)>, op_menu: Option<usize>, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let mut sec = div().pb(z(20.)).flex().flex_col().child(label(crate::t!("search.m_date"), crate::t!("search.m_date_desc")));
    for (i, row) in dates.iter().enumerate() {
        sec = sec.child(date_row(i, *row, op_menu == Some(i), calendar.is_some_and(|c| c.0 == i), cx));
    }
    sec.child(div().mt(z(if dates.is_empty() { 8. } else { 12. })).child(
        button_icon("search-filters-date", "plus", crate::t!("search.m_add_date"), BtnKind::Secondary).w_full().justify_center().on_click(
            cx.listener(|_, _, _, cx| {
                add_date(cx, false);
                cx.notify();
            }),
        ),
    ))
}

fn date_control(id: gpui::ElementId) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(40.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .cursor_pointer()
}

fn date_row(i: usize, row: DateRow, menu_open: bool, cal: bool, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let op = date_control(gpui::ElementId::Name(format!("search-date-op-{i}").into()))
        .w(z(120.))
        .flex_none()
        .on_click(cx.listener(move |_, _, _, cx| {
            with_modal(cx, |m| {
                m.op_menu = if m.op_menu == Some(i) { None } else { Some(i) };
                m.calendar = None;
                m.list = None;
            });
            cx.notify();
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pl(z(12.))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_STRONG)))
                .truncate()
                .child(row.op.label()),
        )
        .child(
            div()
                .w(z(36.))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .pr(z(4.))
                .child(gpui::svg().path("icons/chevron.svg").size(z(20.)).text_color(c(tok(idx::TEXT_STRONG)))),
        );
    let op_wrap = div().relative().flex_none().child(op).when(menu_open, |d| d.child(probe(false)));
    let field = date_control(gpui::ElementId::Name(format!("search-date-field-{i}").into()))
        .px(z(12.))
        .gap(z(8.))
        .on_click(cx.listener(move |_, _, _, cx| {
            with_modal(cx, |m| {
                let open = m.calendar.is_some_and(|c| c.0 == i);
                m.calendar = if open { None } else { m.dates.get(i).map(|r| (i, month_start(r.day))) };
                m.op_menu = None;
                m.list = None;
            });
            cx.notify();
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .truncate()
                .child(crate::i18n::medium_date(row.day)),
        )
        .child(gpui::svg().path(crate::icons::path("calendar")).flex_none().size(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))));
    let field_wrap = div().relative().flex_1().min_w_0().child(field).when(cal, |d| d.child(probe(false)));
    div().mt(z(8.)).h(z(40.)).flex().gap(z(8.)).child(op_wrap).child(field_wrap).child(
        div()
            .id(gpui::ElementId::Name(format!("search-date-remove-{i}").into()))
            .size(z(40.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                with_modal(cx, |m| {
                    if i < m.dates.len() {
                        m.dates.remove(i);
                    }
                    m.calendar = None;
                    m.op_menu = None;
                });
                cx.notify();
            }))
            .child(gpui::svg().path(crate::icons::path("chat/trash")).size(z(24.)).text_color(c(tok(idx::ICON_MUTED)))),
    )
}

fn op_list(i: usize, row: DateRow, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(gpui::ElementId::Name(format!("search-date-ops-{i}").into()))
        .w(z(120.))
        .p(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .shadow_lg()
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(DATE_OPS.into_iter().map(|o| {
            div()
                .id(gpui::ElementId::Name(format!("search-date-op-{i}-{o:?}").into()))
                .h(z(40.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .cursor_pointer()
                .when(o == row.op, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    with_modal(cx, |m| {
                        if let Some(r) = m.dates.get_mut(i) {
                            r.op = o;
                        }
                        m.op_menu = None;
                    });
                    cx.notify();
                }))
                .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).truncate().child(o.label()))
        }))
}

fn month_weeks(month: NaiveDate, sunday_first: bool) -> Vec<[NaiveDate; 7]> {
    let lead = if sunday_first { month.weekday().num_days_from_sunday() } else { month.weekday().num_days_from_monday() };
    let mut d = month - Days::new(u64::from(lead));
    let next = month + Months::new(1);
    let mut weeks = Vec::new();
    while d < next {
        weeks.push(std::array::from_fn(|k| d + Days::new(k as u64)));
        d = d.checked_add_days(Days::new(7)).unwrap_or(next);
    }
    weeks
}

fn calendar_el(i: usize, month: NaiveDate, selected: NaiveDate, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let light = crate::theme::is_light();
    let edge = c(tok(if light { idx::PRIMARY_200 } else { idx::PRIMARY_800 }));
    let rule = if light { c(tok(idx::PRIMARY_200)).opacity(0.2) } else { c(tok(idx::PRIMARY_800)).opacity(0.3) };
    let ink = c(tok(if light { idx::PRIMARY_500 } else { idx::PRIMARY_100 }));
    let now = today();
    let sunday_first = crate::i18n::locale() == "en";
    let weeks = month_weeks(month, sunday_first);
    let n_weeks = weeks.len();
    let title = format!("{} {}", crate::t!(&format!("month.{}", month.month())).to_uppercase(), month.year());
    let nav = |id: &'static str, to: NaiveDate, next: bool, cx: &mut Context<PhoenixApp>| {
        let arrow = gpui::svg().path(crate::icons::path("chevron-right")).size(z(12.)).text_color(c(tok(idx::TEXT_STRONG)));
        div()
            .id(gpui::ElementId::Name(format!("{id}-{i}").into()))
            .absolute()
            .top(z(20.))
            .when(next, |d| d.right(z(20.)))
            .when(!next, |d| d.left(z(20.)))
            .size(z(18.))
            .rounded(z(2.))
            .border_1()
            .border_color(edge)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                with_modal(cx, |m| m.calendar = Some((i, to)));
                cx.notify();
            }))
            .child(if next { arrow } else { arrow.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))) })
    };
    let first_wd = weeks.first().map(|w| w[0]).unwrap_or(month);
    let names = div().mb(z(10.)).flex().children((0..7u64).map(|k| {
        let wd = (first_wd + Days::new(k)).weekday().num_days_from_monday();
        let name: String = crate::t!(&format!("weekday.{wd}")).chars().take(2).collect::<String>().to_uppercase();
        div()
            .w(z(40.))
            .flex()
            .justify_center()
            .text_size(z(13.))
            .line_height(z(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(ink.opacity(0.6))
            .child(name)
    }));
    let mut grid = div().flex().flex_col();
    for (wi, week) in weeks.into_iter().enumerate() {
        let last_row = wi + 1 == n_weeks;
        let mut row = div().flex();
        for (k, d) in week.into_iter().enumerate() {
            let disabled = d > now;
            let outside = d.month() != month.month();
            let muted = disabled || outside;
            let fg = if disabled && light {
                c(tok(idx::OPACITY_BLACK_20))
            } else if muted {
                ink.opacity(0.3)
            } else {
                c(tok(idx::TEXT_STRONG))
            };
            let weight = if d == now {
                FontWeight::BOLD
            } else if muted {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            };
            let cell = div()
                .id(gpui::ElementId::Name(format!("search-date-day-{i}-{d}").into()))
                .relative()
                .size(z(40.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .border_l_1()
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .when(wi == 0 && k == 0, |e| e.rounded_tl(z(3.)))
                .when(wi == 0 && k == 6, |e| e.rounded_tr(z(3.)))
                .when(last_row && k == 0, |e| e.rounded_bl(z(3.)))
                .when(last_row && k == 6, |e| e.rounded_br(z(3.)))
                .when(disabled, |e| e.bg(c(tok(if light { idx::OPACITY_BLACK_4 } else { idx::PRIMARY_630 }))))
                .text_size(z(12.8))
                .font_weight(weight)
                .text_color(fg)
                .child(d.day().to_string())
                .when(k == 6, |e| e.child(div().absolute().top_0().bottom_0().right_0().w(z(1.)).bg(edge)))
                .when(last_row, |e| e.child(div().absolute().left_0().right_0().bottom_0().h(z(1.)).bg(edge)))
                .when(d == selected, |e| e.child(div().absolute().left_0().right_0().bottom_0().h(z(3.)).bg(c(tok(idx::BRAND_500)))))
                .when(!disabled, |e| {
                    e.cursor_pointer().hover(|s| s.bg(c(tok(idx::BRAND_500))).text_color(c(tok(idx::WHITE)))).on_click(cx.listener(
                        move |_, _, _, cx| {
                            with_modal(cx, |m| {
                                if let Some(r) = m.dates.get_mut(i) {
                                    r.day = d;
                                }
                                m.calendar = None;
                            });
                            cx.notify();
                        },
                    ))
                });
            row = row.child(cell);
        }
        grid = grid.child(row);
    }
    let prev = nav("search-date-prev", month - Months::new(1), false, cx);
    let next = (month < month_start(now)).then(|| nav("search-date-next", month + Months::new(1), true, cx));
    div()
        .id(gpui::ElementId::Name(format!("search-date-calendar-{i}").into()))
        .w(z(322.))
        .p(z(20.))
        .rounded(z(4.8))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .shadow(vec![
            gpui::BoxShadow {
                color: gpui::Rgba { r: 30. / 255., g: 31. / 255., b: 34. / 255., a: 0.6 }.into(),
                offset: gpui::point(z(0.), z(0.)),
                blur_radius: z(0.),
                spread_radius: z(1.),
                inset: false,
            },
            gpui::BoxShadow {
                color: gpui::hsla(0., 0., 0., 0.2),
                offset: gpui::point(z(0.), z(2.)),
                blur_radius: z(10.),
                spread_radius: z(0.),
                inset: false,
            },
        ])
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .pb(z(20.))
                .mb(z(10.))
                .border_b_1()
                .border_color(rule)
                .flex()
                .justify_center()
                .text_size(z(13.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(title),
        )
        .child(names)
        .child(grid)
        .child(prev)
        .children(next)
}

fn calendar_h(month: NaiveDate) -> f32 {
    let weeks = month_weeks(month, crate::i18n::locale() == "en").len();
    2. + 40. + 51. + 23. + 40. * weeks as f32
}

pub fn render(app: &PhoenixApp, window: &Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let m = cx.try_global::<Filters>()?.0.as_ref()?;
    let fields = m.fields.clone();
    let picks = m.picks.clone();
    let (dates, calendar, op_menu, list) = (m.dates.clone(), m.calendar, m.op_menu, m.list);
    let set = picks.iter().filter(|p| !p.is_empty()).count() + dates.len();
    let mut body = div().flex().flex_col().pt(z(12.)).pb(z(12.));
    for (i, ((key, title, desc), f)) in FIELDS.iter().zip(&fields).enumerate() {
        if *key == "authorType:" {
            body = body.child(date_section(&dates, calendar, op_menu, cx));
        }
        let last = i + 1 == FIELDS.len();
        body = body.child(
            div().pb(z(if last { 0. } else { 20. })).flex().flex_col().child(label(crate::t!(title), crate::t!(desc))).child(field(app, i, f, &picks[i], list == Some(i), cx)),
        );
    }
    let pop: Option<AnyElement> = match (calendar, op_menu, ANCHOR.with(|c| c.get()), CARD.with(|c| c.get())) {
        (Some((i, month)), _, Some(b), Some(o)) => dates.get(i).map(|r| {
            let h = z(calendar_h(month));
            let below = b.bottom() + z(8.);
            let top = if below + h > window.viewport_size().height && b.top() - z(8.) - h >= gpui::px(0.) { b.top() - z(8.) - h } else { below };
            div().absolute().left(b.left() - o.x).top(top - o.y).child(calendar_el(i, month, r.day, cx)).into_any_element()
        }),
        (None, Some(i), Some(b), Some(o)) => dates.get(i).map(|r| {
            let h = z(130.);
            let below = b.bottom() + z(4.);
            let top = if below + h > window.viewport_size().height && b.top() - z(4.) - h >= gpui::px(0.) { b.top() - z(4.) - h } else { below };
            div().absolute().left(b.left() - o.x).top(top - o.y).child(op_list(i, *r, cx)).into_any_element()
        }),
        (None, None, Some(b), Some(o)) => list.and_then(|i| {
            let h = z(list_h(app, i, cx));
            let below = b.bottom() + z(4.);
            let top = if below + h > window.viewport_size().height && b.top() - z(4.) - h >= gpui::px(0.) { b.top() - z(4.) - h } else { below };
            option_list(app, i, b.size.width, cx).map(|l| div().absolute().left(b.left() - o.x).top(top - o.y).child(l).into_any_element())
        }),
        _ => None,
    };
    if calendar.is_none() && op_menu.is_none() && list.is_none() {
        ANCHOR.with(|c| c.set(None));
    }
    let card = div()
        .id("search-filters-card")
        .relative()
        .child(probe(true))
        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
            let mut was = false;
            with_modal(cx, |m| was = m.list.take().is_some());
            if was {
                cx.notify();
            }
        }))
        .w(z(480.))
        .max_h(z(728.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(crate::t!("search.filters")),
        )
        .child(div().mt(z(16.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(div().id("search-filters-body").flex_1().min_h_0().overflow_y_scroll().pl(z(24.)).pr(z(8.)).child(div().pr(z(16.)).child(body)))
        .child(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(
                    div()
                        .id("search-filters-clear")
                        .flex_1()
                        .cursor_pointer()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_BRAND)))
                        .hover(|s| s.underline())
                        .on_click(cx.listener(|_, _, w, cx| {
                            let Some(m) = cx.global::<Filters>().0.as_ref() else {
                                return;
                            };
                            let inputs = m.fields.clone();
                            for i in inputs {
                                i.update(cx, |s, cx| s.set_value("", w, cx));
                            }
                            with_modal(cx, |m| {
                                m.picks.iter_mut().for_each(Vec::clear);
                                m.dates.clear();
                                m.calendar = None;
                                m.op_menu = None;
                                m.list = None;
                            });
                            cx.notify();
                        }))
                        .child(if set > 0 { format!("{} ({set})", crate::t!("search.m_clear")).into() } else { crate::t!("search.m_clear") }),
                )
                .child(button("search-filters-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| close(cx))))
                .child(button("search-filters-apply", crate::t!("search.m_apply"), BtnKind::Primary).on_click(cx.listener(|a, _, w, cx| {
                    let acx: &App = cx;
                    let t = acx.global::<Filters>().0.as_ref().map(tokens).unwrap_or_default();
                    close(cx);
                    crate::ui::search::append_and_submit(a, &t, w, cx);
                }))),
        )
        .child(
            div()
                .id("search-filters-close")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click(cx.listener(|_, _, _, cx| close(cx)))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        );
    let weak = cx.entity().downgrade();
    Some(crate::ui::modal::frame(
        "search-filters-modal",
        std::rc::Rc::new(move |_, cx| {
            let _ = weak.update(cx, |_, cx| close(cx));
        }),
        div().relative().child(card).children(pop),
    ))
}

#[cfg(test)]
mod tests {
    use super::{month_weeks, quoted, score};
    use chrono::NaiveDate;

    #[test]
    fn calendar_weeks() {
        let nov = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let w = month_weeks(nov, false);
        assert_eq!(w.len(), 6);
        assert_eq!(w[0][0], NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        assert_eq!(w[5][6], NaiveDate::from_ymd_opt(2026, 12, 6).unwrap());
        assert_eq!(month_weeks(nov, true).len(), 5);
    }

    #[test]
    fn quotes_and_scores() {
        assert_eq!(quoted(" Some Name "), "\"Some Name\"");
        assert_eq!(quoted("kasiopy"), "kasiopy");
        assert_eq!(score("Kasiopy", "kasiopy"), Some(0));
        assert_eq!(score("Kasiopy", "kas"), Some(1));
        assert_eq!(score("Kasiopy", "opy"), Some(2));
        assert_eq!(score("Kasiopy", "x"), None);
        assert_eq!(score("Kasiopy", ""), Some(3));
    }
}
