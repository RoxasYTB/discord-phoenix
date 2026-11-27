use std::collections::HashSet;

use gpui::{AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Styled, Window, div, prelude::*, px};
use phoenix_api::rest::guild_manage::AdminCall;
use serde_json::Value;

use crate::{
    actions::admin::sf,
    app::PhoenixApp,
    commands::{GuildEvent, ParityReply, ParityRequest},
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::BtnKind,
};

#[derive(Default)]
struct Slot(Option<Events>);
impl gpui::Global for Slot {}

struct Events {
    guild: Id,
    list: Option<Vec<GuildEvent>>,
    error: Option<SharedString>,
    copied: Option<Id>,
    detail: Option<Detail>,
    menu: Option<(Id, gpui::Point<Pixels>)>,
}

struct Detail {
    event: Id,
    tab: u8,
    users: Users,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Users {
    Idle,
    Loading,
    Done(Vec<EventUser>),
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventUser {
    pub id: Id,
    pub name: SharedString,
    pub avatar: SharedString,
}

thread_local! {
    static RSVP: std::cell::RefCell<HashSet<Id>> = std::cell::RefCell::new(HashSet::new());
}

pub fn rsvped(event: Id) -> bool {
    RSVP.with(|r| r.borrow().contains(&event))
}

const TAB_INFO: u8 = 0;
const TAB_RSVP: u8 = 1;
const PAGE: usize = 100;

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn can_create(perms: u64) -> bool {
    crate::ui::inbox::has_perm(perms, (1 << 44) | (1 << 33))
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Slot>().is_some_and(|s| s.0.is_some())
}

pub fn close(cx: &mut App) {
    let detail = cx.try_global::<Slot>().is_some_and(|s| s.0.as_ref().is_some_and(|e| e.detail.is_some()));
    if detail {
        with(cx, |e| e.detail = None);
    } else {
        cx.set_global(Slot(None));
    }
}

fn with(cx: &mut App, f: impl FnOnce(&mut Events)) {
    if !cx.has_global::<Slot>() {
        return;
    }
    if let Some(e) = cx.global_mut::<Slot>().0.as_mut() {
        f(e);
    }
}

fn guild_of(cx: &App) -> Option<Id> {
    cx.try_global::<Slot>()?.0.as_ref().map(|e| e.guild)
}

pub fn parse_rsvps(v: &Value) -> HashSet<Id> {
    let id = |e: &Value, k: &str| e.get(k).and_then(Value::as_str).and_then(|x| x.parse::<u64>().ok()).map(Id);
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|e| id(e, "guild_scheduled_event_exception_id").is_none())
        .filter(|e| e.get("response").and_then(Value::as_u64).is_none_or(|r| r == 1))
        .filter_map(|e| id(e, "guild_scheduled_event_id"))
        .collect()
}

pub fn parse_users(v: &Value) -> Vec<EventUser> {
    use crate::ui::modals::server_settings::kit;
    let mut seen = HashSet::new();
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let u = e.get("user")?;
            let id = Id(kit::id_of(u, "id")?);
            let nick = e.get("member").and_then(|m| m.get("nick")).and_then(Value::as_str).filter(|n| !n.is_empty());
            let name = nick.map(str::to_owned).unwrap_or_else(|| kit::user_name(u));
            seen.insert(id).then(|| EventUser { id, name: name.into(), avatar: kit::user_avatar(u, 48) })
        })
        .collect()
}

pub fn not_shown(shown: usize, count: u32) -> u32 {
    if shown >= PAGE && count as usize > PAGE { count.saturating_sub(shown as u32) } else { 0 }
}

pub fn open(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    cx.set_global(Slot(Some(Events { guild, list: None, error: None, copied: None, detail: None, menu: None })));
    app.parity_request(ParityRequest::ScheduledEvents { guild }, cx, move |_, r, cx| {
        if guild_of(cx) != Some(guild) {
            return;
        }
        let (list, error) = match r {
            Ok(ParityReply::Events(mut v)) => {
                v.retain(|e| e.status <= 2);
                v.sort_by_key(|e| (e.status != 2, e.start));
                (Some(v), None)
            }
            Ok(_) => (Some(vec![]), None),
            Err(e) => (Some(vec![]), Some(SharedString::from(e.0))),
        };
        with(cx, |e| {
            e.list = list;
            e.error = error;
        });
        cx.notify();
    });
    load_rsvps(app, guild, cx);
    cx.notify();
}

fn load_rsvps(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    app.admin(AdminCall::MyEventRsvps { guild: sf(guild) }, cx, move |_, r, cx| {
        if let Ok(v) = r {
            let got = parse_rsvps(&v);
            RSVP.with(|set| set.borrow_mut().extend(got));
            cx.notify();
        }
    });
}

fn set_count(e: &mut Events, event: Id, up: bool) {
    if let Some(ev) = e.list.iter_mut().flatten().find(|x| x.id == event) {
        ev.user_count = if up { ev.user_count + 1 } else { ev.user_count.saturating_sub(1) };
    }
    if let Some(d) = e.detail.as_mut().filter(|d| d.event == event) {
        d.users = Users::Idle;
    }
}

fn apply_rsvp(set: &mut HashSet<Id>, e: Option<&mut Events>, event: Id, on: bool) {
    let changed = if on { set.insert(event) } else { set.remove(&event) };
    if let (true, Some(e)) = (changed, e) {
        set_count(e, event, on);
    }
}

fn apply(cx: &mut App, event: Id, on: bool) {
    let mut set = RSVP.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let open = cx.has_global::<Slot>();
    let e = if open { cx.global_mut::<Slot>().0.as_mut() } else { None };
    apply_rsvp(&mut set, e, event, on);
    RSVP.with(|r| *r.borrow_mut() = set);
}

pub fn toggle_rsvp(app: &mut PhoenixApp, guild: Id, event: Id, cx: &mut Context<PhoenixApp>) {
    let on = !rsvped(event);
    apply(cx, event, on);
    cx.notify();
    app.admin(AdminCall::EventRsvp { guild: sf(guild), event: sf(event), interested: on }, cx, move |a, r, cx| {
        if let Err(err) = r {
            apply(cx, event, !on);
            a.parity_toast(err, cx);
        }
        cx.notify();
    });
}

pub fn open_detail(event: Id, cx: &mut App) {
    with(cx, |e| e.detail = Some(Detail { event, tab: TAB_INFO, users: Users::Idle }));
}

fn select_tab(app: &mut PhoenixApp, tab: u8, cx: &mut Context<PhoenixApp>) {
    let Some(guild) = guild_of(cx) else { return };
    let mut load = None;
    with(cx, |e| {
        if let Some(d) = e.detail.as_mut() {
            d.tab = tab;
            if tab == TAB_RSVP && matches!(d.users, Users::Idle | Users::Failed) {
                d.users = Users::Loading;
                load = Some(d.event);
            }
        }
    });
    cx.notify();
    let Some(event) = load else { return };
    app.admin(AdminCall::EventUsers { guild: sf(guild), event: sf(event) }, cx, move |_, r, cx| {
        with(cx, |e| {
            if let Some(d) = e.detail.as_mut().filter(|d| d.event == event) {
                d.users = match r {
                    Ok(v) => Users::Done(parse_users(&v)),
                    Err(_) => Users::Failed,
                };
            }
        });
        cx.notify();
    });
}

fn copy_link(guild: Id, event: Id, cx: &mut App) {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(format!("https://discord.com/events/{}/{}", guild.0, event.0)));
    with(cx, |e| e.copied = Some(event));
    cx.refresh_windows();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
        cx.update(|cx| {
            with(cx, |e| {
                if e.copied == Some(event) {
                    e.copied = None;
                }
            });
            cx.refresh_windows();
        });
    })
    .detach();
}

fn uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn query(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", uri_component(k), uri_component(v))).collect::<Vec<_>>().join("&")
}

fn local_stamp(ts: i64, fmt: &str) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).format(fmt).to_string()).unwrap_or_default()
}

const STAMP: &str = "%Y%m%dT%H%M%S";
const ISO_LOCAL: &str = "%Y-%m-%dT%H:%M:%S";

pub fn event_location(e: &GuildEvent, guild_name: &str, channel_name: Option<&str>) -> String {
    match (&e.location, channel_name) {
        (Some(l), _) => l.clone(),
        (None, Some(ch)) => crate::t!("events.location_in_guild", guild = guild_name, channel = ch).to_string(),
        (None, None) => "Discord".to_owned(),
    }
}

pub fn google_calendar_url(e: &GuildEvent, location: &str) -> String {
    let end = e.end.unwrap_or(e.start);
    let dates = format!("{}/{}", local_stamp(e.start, STAMP), local_stamp(end, STAMP));
    let q = query(&[("text", &e.name), ("dates", &dates), ("details", &e.description), ("action", "TEMPLATE"), ("location", location)]);
    format!("https://calendar.google.com/calendar/render?{q}")
}

pub fn yahoo_calendar_url(e: &GuildEvent, location: &str) -> String {
    let end = e.end.unwrap_or(e.start);
    let (st, et) = (local_stamp(e.start, STAMP), local_stamp(end, STAMP));
    let q = query(&[("v", "60"), ("title", &e.name), ("st", &st), ("et", &et), ("desc", &e.description), ("in_loc", location)]);
    format!("https://calendar.yahoo.com/?{q}")
}

pub fn outlook_calendar_url(e: &GuildEvent, location: &str) -> String {
    let end = e.end.unwrap_or(e.start);
    let (st, et) = (local_stamp(e.start, ISO_LOCAL), local_stamp(end, ISO_LOCAL));
    let q = query(&[
        ("path", "/calendar/action/compose"),
        ("rru", "addevent"),
        ("startdt", &st),
        ("enddt", &et),
        ("subject", &e.name),
        ("body", &e.description),
        ("location", location),
        ("allday", "false"),
    ]);
    format!("https://outlook.live.com/calendar/0/deeplink/compose?{q}")
}

fn ics_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

fn utc_stamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.format("%Y%m%dT%H%M%SZ").to_string()).unwrap_or_default()
}

pub fn ics(e: &GuildEvent, location: &str, now: i64) -> String {
    let end = e.end.unwrap_or(e.start);
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        "PRODID:-//sebbo.net//ical-generator//EN".to_owned(),
        "BEGIN:VEVENT".to_owned(),
        format!("UID:{}@discord.com", e.id.0),
        "SEQUENCE:0".to_owned(),
        format!("DTSTAMP:{}", utc_stamp(now)),
        format!("DTSTART:{}", utc_stamp(e.start)),
        format!("DTEND:{}", utc_stamp(end)),
        format!("SUMMARY:{}", ics_text(&e.name)),
        format!("LOCATION:{}", ics_text(location)),
    ];
    if !e.description.is_empty() {
        lines.push(format!("DESCRIPTION:{}", ics_text(&e.description)));
    }
    lines.push("END:VEVENT".to_owned());
    lines.push("END:VCALENDAR".to_owned());
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out
}

fn save_ics(text: String, cx: &mut App) {
    let rx = cx.prompt_for_new_path(&crate::paths::downloads_dir(), Some("discord-event.ics"));
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(path))) = rx.await else { return };
        let _ = cx.background_executor().spawn(async move { std::fs::write(path, text) }).await;
    })
    .detach();
}

fn event_menu(app: &PhoenixApp, guild: Id, e: &GuildEvent) -> Vec<crate::ui::context_menu::MenuItem> {
    use crate::ui::context_menu::MenuItem;
    let id = e.id;
    let guild_name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.to_string()).unwrap_or_default();
    let channel_name = e.channel.and_then(|c| app.channels.get(&c)).map(|c| c.name.to_string());
    let location = event_location(e, &guild_name, channel_name.as_deref());
    let mut items = Vec::new();
    if e.status != 2 {
        let (g, y, o) = (google_calendar_url(e, &location), yahoo_calendar_url(e, &location), outlook_calendar_url(e, &location));
        let text = ics(e, &location, chrono::Utc::now().timestamp());
        let text2 = text.clone();
        items.push(MenuItem::Submenu {
            label: crate::t!("events.add_to_calendar"),
            icon: None,
            subtext: None,
            on_click: Some(std::rc::Rc::new(move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| save_ics(text.clone(), cx))),
            items: vec![
                MenuItem::action(crate::t!("events.calendar_google"), None, move |_, _, cx| cx.open_url(&g)),
                MenuItem::action(crate::t!("events.calendar_ics"), None, move |_, _, cx| save_ics(text2.clone(), cx)),
                MenuItem::action(crate::t!("events.calendar_yahoo"), None, move |_, _, cx| cx.open_url(&y)),
                MenuItem::action(crate::t!("events.calendar_outlook"), None, move |_, _, cx| cx.open_url(&o)),
            ],
        });
    }
    items.push(MenuItem::action(crate::t!("events.copy_event_link"), None, move |_, _, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(format!("https://discord.com/events/{}/{}", guild.0, id.0)));
    }));
    if app.prefs.synced.developer_mode {
        items.push(MenuItem::Separator);
        items.push(
            MenuItem::action(crate::t!("events.copy_event_id"), None, move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(id.0.to_string())))
                .with_icon("id"),
        );
    }
    items
}

fn when_label(e: &GuildEvent) -> String {
    when_label_at(e.start)
}

pub fn when_label_at(ts: i64) -> String {
    let Some(d) = chrono::DateTime::from_timestamp(ts, 0) else { return String::new() };
    let local = d.with_timezone(&chrono::Local);
    format!("{} {}", crate::i18n::long_date(local.date_naive()), local.format("%H:%M"))
}

fn sm_button(id: impl Into<gpui::ElementId>, icon: Option<&'static str>, label: Option<SharedString>, kind: BtnKind) -> gpui::Stateful<gpui::Div> {
    let fg = match kind {
        BtnKind::Active => idx::CONTROL_CONNECTED_TEXT_DEFAULT,
        BtnKind::Primary => idx::CONTROL_PRIMARY_TEXT_DEFAULT,
        _ => idx::CONTROL_SECONDARY_TEXT_DEFAULT,
    };
    let b = crate::ui::controls::button(id, SharedString::default(), kind).h(z(32.)).text_size(z(14.)).line_height(z(18.));
    let has_label = label.is_some();
    let b = if has_label { b.min_w(z(60.)).px(z(12.)) } else { b.min_w(z(32.)).w(z(32.)).px(z(0.)) };
    b.when_some(icon, |d, i| d.child(gpui::svg().path(crate::icons::path(i)).size(z(16.)).flex_none().when(has_label, |s| s.mr(z(6.))).text_color(t(fg))))
        .when_some(label, |d, l| d.child(l))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinLabel {
    GoToChannel,
    Join,
    Listen,
    Locked,
}

pub fn join_label(joined: bool, can_join: bool, stage: bool) -> JoinLabel {
    match (can_join, joined, stage) {
        (false, _, _) => JoinLabel::Locked,
        (true, true, _) => JoinLabel::GoToChannel,
        (true, false, true) => JoinLabel::Listen,
        (true, false, false) => JoinLabel::Join,
    }
}

fn event_channel(app: &PhoenixApp, e: &GuildEvent) -> Option<(Id, bool)> {
    let ch = e.channel?;
    match app.channels.get(&ch)?.kind {
        crate::model::ChannelKind::Voice => Some((ch, false)),
        crate::model::ChannelKind::Stage => Some((ch, true)),
        _ => None,
    }
}

fn actions(app: &PhoenixApp, guild: Id, e: &GuildEvent, st: Option<(&Events, &Entity<PhoenixApp>)>) -> Vec<AnyElement> {
    let id = e.id;
    let live = e.status == 2;
    let copied = st.is_some_and(|(s, _)| s.copied == Some(id));
    let mut out = Vec::new();
    if st.is_some() {
        let more = sm_button(("event-more", id.0), Some("more-horizontal"), None, BtnKind::Secondary).on_click(move |ev, w, cx| {
            cx.stop_propagation();
            with(cx, |s| s.menu = Some((id, ev.position())));
            w.refresh();
        });
        out.push(more.into_any_element());
    }
    let link = sm_button(("event-link", id.0), Some(if copied { "check" } else { "chat/link" }), None, BtnKind::Secondary)
        .tooltip(crate::ui::tooltip::tip(if copied { crate::t!("events.copied") } else { crate::t!("events.copy_link") }))
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            copy_link(guild, id, cx);
        });
    out.push(link.into_any_element());
    if live {
        if let Some((ch, stage)) = event_channel(app, e) {
            let can = app.backend.channel_permissions(ch).is_none_or(|p| crate::ui::inbox::has_perm(p, 1 << 20));
            let joined = app.voice_channel == Some(ch);
            let label = match join_label(joined, can, stage) {
                JoinLabel::GoToChannel => crate::t!("events.go_to_channel"),
                JoinLabel::Join => crate::t!("events.join"),
                JoinLabel::Listen => crate::t!("events.listen"),
                JoinLabel::Locked => crate::t!("events.channel_locked"),
            };
            let mut b = sm_button(("event-join", id.0), None, Some(label), BtnKind::Active);
            if can {
                if let Some((_, ent)) = st {
                    let ent = ent.clone();
                    b = b.on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        cx.set_global(Slot(None));
                        ent.update(cx, |a, cx| if joined { a.select_channel(ch, cx) } else { a.join_voice(Some(ch), cx) });
                    });
                }
            } else {
                b = b.opacity(0.5).cursor_not_allowed();
            }
            out.push(b.into_any_element());
        }
    } else {
        let on = rsvped(id);
        let mut b = sm_button(
            ("event-interested", id.0),
            Some(if on { "check" } else { "bell" }),
            Some(crate::t!("events.interested")),
            if on { BtnKind::Active } else { BtnKind::Secondary },
        );
        if let Some((_, ent)) = st {
            let ent = ent.clone();
            b = b.on_click(move |_, _, cx| {
                cx.stop_propagation();
                ent.update(cx, |a, cx| toggle_rsvp(a, guild, id, cx));
            });
        }
        out.push(b.into_any_element());
    }
    out
}

fn place(app: &PhoenixApp, e: &GuildEvent) -> (&'static str, String) {
    let ch_kind = e.channel.and_then(|ch| app.channels.get(&ch)).map(|ch| ch.kind);
    let icon = match ch_kind {
        Some(crate::model::ChannelKind::Voice) => "speaker",
        Some(crate::model::ChannelKind::Stage) => "stage",
        _ if e.location.is_some() => "event/location",
        _ => "calendar",
    };
    let name = e.location.clone().or_else(|| e.channel.and_then(|ch| app.channels.get(&ch)).map(|ch| ch.name.to_string())).unwrap_or_default();
    (icon, name)
}

fn status_line(e: &GuildEvent) -> gpui::Div {
    let live = e.status == 2;
    div()
        .flex()
        .items_center()
        .gap(z(8.))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if live { t(idx::TEXT_FEEDBACK_POSITIVE) } else { t(idx::TEXT_MUTED) })
        .child(gpui::svg().path(crate::icons::path("calendar")).size(z(16.)).flex_none().text_color(if live { t(idx::TEXT_FEEDBACK_POSITIVE) } else { t(idx::ICON_MUTED) }))
        .child(if live { crate::t!("events.live").to_string() } else { when_label(e) })
}

fn banner(e: &GuildEvent, h: f32) -> Option<gpui::Img> {
    let hash = e.image.as_ref()?;
    let url: SharedString = format!("https://cdn.discordapp.com/guild-events/{}/{hash}.png?size=600", e.id.0).into();
    Some(gpui::img(url).w_full().h(z(h)).flex_none().object_fit(gpui::ObjectFit::Cover))
}

pub fn card(app: &PhoenixApp, guild: Id, e: &GuildEvent) -> gpui::Div {
    card_el(app, guild, e, None)
}

fn card_el(app: &PhoenixApp, guild: Id, e: &GuildEvent, st: Option<(&Events, &Entity<PhoenixApp>)>) -> gpui::Div {
    let (loc_icon, place) = place(app, e);
    let body = div()
        .p(z(16.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(status_line(e).child(div().flex_1()).when(e.user_count > 0, |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t(idx::TEXT_MUTED))
                    .child(gpui::svg().path(crate::icons::path("members")).size(z(16.)).flex_none().text_color(t(idx::ICON_MUTED)))
                    .child(e.user_count.to_string()),
            )
        }))
        .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(e.name.clone()))
        .when(!e.description.is_empty(), |d| {
            d.child(div().text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_DEFAULT)).child(crate::ui::emoji_text::inline_markdown(app, &e.description, t(idx::TEXT_DEFAULT), gpui::FontWeight::NORMAL, 14., true)))
        })
        .child(
            div()
                .mt(z(4.))
                .pt(z(12.))
                .border_t_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .flex()
                .items_center()
                .justify_between()
                .gap(z(8.))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(6.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(t(idx::TEXT_MUTED))
                        .when(!place.is_empty(), |d| {
                            d.child(gpui::svg().path(crate::icons::path(loc_icon)).size(z(16.)).flex_none().text_color(t(idx::ICON_MUTED)))
                                .child(div().min_w_0().text_ellipsis().child(place))
                        }),
                )
                .child(div().flex_none().flex().items_center().gap(z(8.)).children(actions(app, guild, e, st))),
        );
    div()
        .rounded(z(12.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::BACKGROUND_BASE_LOW))
        .overflow_hidden()
        .flex()
        .flex_col()
        .children(banner(e, 160.))
        .child(body)
}

pub fn modal(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let s = cx.try_global::<Slot>()?.0.as_ref()?;
    let guild = s.guild;
    if let Some((d, ev)) = s.detail.as_ref().and_then(|d| s.list.iter().flatten().find(|e| e.id == d.event).map(|ev| (d, ev))) {
        return Some(detail_modal(app, s, d, ev, cx));
    }
    let perms = app.backend.guild_permissions(guild);
    let e = cx.entity();
    let on_close: crate::ui::modal::Action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
        close(cx);
        e.update(cx, |_, cx| cx.notify());
    });
    let close2 = on_close.clone();
    let header = div()
        .pt(z(16.))
        .px(z(24.))
        .pb(z(8.))
        .flex()
        .items_center()
        .child(gpui::svg().path(crate::icons::path("calendar")).size(z(24.)).mr(z(8.)).flex_none().text_color(t(idx::ICON_MUTED)))
        .child(
            div()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t(idx::TEXT_STRONG))
                .child(crate::t!("events.title")),
        )
        .when(can_create(perms), |d| {
            d.child(div().w(px(1.)).h(z(16.)).mx(z(16.)).bg(t(idx::BORDER_SUBTLE))).child(
                div()
                    .id("events-create")
                    .h(z(32.))
                    .px(z(12.))
                    .rounded(z(8.))
                    .bg(t(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT))
                    .hover(|s| s.bg(t(idx::CONTROL_PRIMARY_BACKGROUND_HOVER)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(gpui::white())
                    .on_click(cx.listener(move |a, _, _, cx| {
                        close(cx);
                        a.open_create_event(guild, cx);
                    }))
                    .child(crate::t!("events.create")),
            )
        })
        .child(div().flex_1())
        .child(
            div()
                .id("events-close")
                .cursor_pointer()
                .on_click(move |_, w, cx| close2(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(24.)).flex_none().text_color(t(idx::ICON_MUTED))),
        );
    let body: AnyElement = match &s.list {
        None => div().size_full().flex().items_center().justify_center().text_color(t(idx::TEXT_MUTED)).child(crate::t!("common.loading")).into_any_element(),
        Some(list) if list.is_empty() => {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .child(
                    div()
                        .mb(z(8.))
                        .text_size(z(24.))
                        .line_height(z(32.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t(idx::TEXT_STRONG))
                        .child(crate::t!("events.empty_title")),
                )
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_DEFAULT)).child(crate::t!("events.empty_sub")))
                .when(can_create(perms), |d| {
                    d.child(
                        div()
                            .mt(z(4.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(t(idx::TEXT_DEFAULT))
                            .child(crate::t!("events.role_tip"))
                            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_LINK)).child(crate::t!("events.role_tip_link"))),
                    )
                })
                .when_some(s.error.clone(), |d, er| d.child(div().mt(z(8.)).text_size(z(14.)).text_color(t(idx::TEXT_FEEDBACK_CRITICAL)).child(er)))
                .into_any_element()
        }
        Some(list) => {
            let mut col = div().id("events-list").size_full().overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("events-list")).flex().flex_col().gap(z(16.));
            let ent = cx.entity();
            for ev in list {
                let id = ev.id;
                col = col.child(
                    div()
                        .id(("event-card", id.0))
                        .cursor_pointer()
                        .on_click(move |_, w, cx| {
                            open_detail(id, cx);
                            w.refresh();
                        })
                        .child(card_el(app, guild, ev, Some((s, &ent)))),
                );
            }
            col.into_any_element()
        }
    };
    let card = div()
        .w(z(602.))
        .h(z(400.))
        .rounded(z(12.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::MODAL_BACKGROUND))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.16),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(8.),
            spread_radius: px(0.),
            inset: false,
        }])
        .flex()
        .flex_col()
        .child(header)
        .child(
            crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin)
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_h_0().pt(z(8.)).pr(z(16.)).pb(z(16.)).pl(z(24.)).child(body))
                .children(crate::ui::chat::scrollbar::bar_for("events-list", crate::ui::chat::scrollbar::Bar::Thin)),
        );
    Some(crate::ui::modal::frame("events-modal", on_close, card))
}

pub fn menu_layer(app: &PhoenixApp, cx: &App) -> Option<AnyElement> {
    let s = cx.try_global::<Slot>()?.0.as_ref()?;
    let (id, pos) = s.menu?;
    let ev = s.list.iter().flatten().find(|e| e.id == id)?;
    let items = event_menu(app, s.guild, ev);
    let dismiss: crate::ui::context_menu::Dismiss = std::rc::Rc::new(|w: &mut Window, cx: &mut App| {
        with(cx, |s| s.menu = None);
        w.refresh();
    });
    Some(crate::ui::context_menu::render(pos, items, dismiss))
}

fn info_row(icon: AnyElement, content: impl IntoElement) -> gpui::Div {
    div().my(z(8.)).flex().items_center().text_color(t(idx::TEXT_MUTED)).child(div().mr(z(8.)).flex_none().child(icon)).child(content)
}

fn row_icon(path: &'static str) -> AnyElement {
    gpui::svg().path(crate::icons::path(path)).size(z(20.)).flex_none().text_color(t(idx::ICON_MUTED)).into_any_element()
}

fn body_sm(text: impl Into<SharedString>) -> gpui::Div {
    div().min_w_0().text_size(z(14.)).line_height(z(18.)).text_color(t(idx::TEXT_DEFAULT)).child(text.into())
}

fn info_panel(app: &PhoenixApp, ev: &GuildEvent, guild: Id, ent: &Entity<PhoenixApp>) -> gpui::Div {
    let g = app.guilds.iter().find(|g| g.id == guild);
    let (loc_icon, place) = place(app, ev);
    let count = ev.user_count;
    let creator = ev.creator.and_then(|u| app.users.get(&u));
    let mut body = div().pt(z(8.)).pb(z(24.)).flex().flex_col();
    if let Some(g) = g {
        let icon = div().size(z(20.)).rounded_full().overflow_hidden().child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, f32::from(z(20.)))).into_any_element();
        body = body.child(info_row(icon, body_sm(g.name.clone())));
    }
    if !place.is_empty() {
        body = body.child(info_row(row_icon(loc_icon), body_sm(place)));
    }
    if count > 0 {
        let e2 = ent.clone();
        let label = if count == 1 { crate::t!("invite.card.event_people_one", n = count) } else { crate::t!("invite.card.event_people", n = count) };
        body = body.child(info_row(
            row_icon("members"),
            body_sm(label).id("event-interested-count").cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| {
                e2.update(cx, |a, cx| select_tab(a, TAB_RSVP, cx));
            }),
        ));
    }
    if let Some(u) = creator {
        let uid = u.id;
        let e2 = ent.clone();
        let name = if u.display_name.is_empty() { u.name.clone() } else { u.display_name.clone() };
        body = body.child(info_row(
            crate::ui::avatar(Some(u), f32::from(z(20.))),
            body_sm(crate::t!("events.created_by", name = name)).id("event-creator").cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| {
                e2.update(cx, |a, cx| a.open_profile_modal(uid, cx));
            }),
        ));
    }
    if !ev.description.is_empty() {
        body = body.child(div().mt(z(16.)).text_size(z(16.)).line_height(z(20.)).text_color(t(idx::TEXT_DEFAULT)).child(crate::ui::emoji_text::inline_markdown(
            app,
            &ev.description,
            t(idx::TEXT_DEFAULT),
            FontWeight::NORMAL,
            16.,
            true,
        )));
    }
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .pt(z(12.))
                .flex()
                .flex_col()
                .child(status_line(ev))
                .child(div().mt(z(16.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(ev.name.clone())),
        )
        .child(body)
}

fn users_panel(ev: &GuildEvent, d: &Detail, ent: &Entity<PhoenixApp>) -> AnyElement {
    let centered = |el: AnyElement| div().h(z(240.)).flex().flex_col().items_center().justify_center().text_center().child(el).into_any_element();
    let group_icon = || {
        div()
            .mb(z(16.))
            .size(z(80.))
            .rounded_full()
            .bg(t(idx::BACKGROUND_MOD_NORMAL))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::svg().path(crate::icons::path("members")).size(z(40.)).text_color(t(idx::ICON_MUTED)))
    };
    match &d.users {
        Users::Idle | Users::Loading => centered(crate::ui::spinner::spinner()),
        Users::Failed => centered(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(group_icon())
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_DEFAULT)).child(crate::t!("events.users_error")))
                .into_any_element(),
        ),
        Users::Done(list) if list.is_empty() => centered(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(group_icon())
                .child(div().text_size(z(24.)).line_height(z(30.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).child(crate::t!("events.nobody")))
                .into_any_element(),
        ),
        Users::Done(list) => {
            let more = not_shown(list.len(), ev.user_count);
            let mut col = div().id("event-users").max_h(z(400.)).overflow_y_scroll().flex().flex_col().py(z(8.));
            for u in list {
                let uid = u.id;
                let e2 = ent.clone();
                col = col.child(
                    div()
                        .id(("event-user", uid.0))
                        .h(z(40.))
                        .px(z(8.))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .cursor_pointer()
                        .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                        .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
                        .child(crate::ui::modals::server_settings::kit::avatar(u.avatar.clone(), 24.))
                        .child(div().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_STRONG)).child(u.name.clone())),
                );
            }
            if more > 0 {
                let label = if more == 1 { crate::t!("events.more_one", n = more) } else { crate::t!("events.more", n = more) };
                col = col.child(
                    div()
                        .h(z(40.))
                        .px(z(8.))
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .child(div().size(z(24.)).rounded_full().bg(t(idx::BACKGROUND_MOD_NORMAL)).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("members")).size(z(14.)).text_color(t(idx::ICON_MUTED))))
                        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(t(idx::TEXT_DEFAULT)).child(label)),
                );
            }
            col.into_any_element()
        }
    }
}

fn detail_modal(app: &PhoenixApp, s: &Events, d: &Detail, ev: &GuildEvent, cx: &Context<PhoenixApp>) -> AnyElement {
    let ent = cx.entity();
    let guild = s.guild;
    let on_close: crate::ui::modal::Action = {
        let ent = ent.clone();
        std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
            with(cx, |e| e.detail = None);
            ent.update(cx, |_, cx| cx.notify());
        })
    };
    let tab = |n: u8, label: SharedString| {
        let on = d.tab == n;
        let e2 = ent.clone();
        let group = SharedString::from(format!("event-tab-{n}"));
        div()
            .id(("event-tab", n as u64))
            .group(group.clone())
            .relative()
            .flex_none()
            .when(n != TAB_INFO, |d| d.ml(z(30.)))
            .pb(z(16.))
            .mb(z(-1.))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if on { t(idx::TEXT_STRONG) } else { t(idx::TEXT_SUBTLE) })
            .hover(|s| s.text_color(t(idx::TEXT_STRONG)))
            .on_click(move |_, _, cx| e2.update(cx, |a, cx| select_tab(a, n, cx)))
            .child(label)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(z(2.))
                    .rounded_t(z(2.))
                    .when(on, |d| d.bg(t(idx::INTERACTIVE_TEXT_ACTIVE)))
                    .when(!on, |d| d.group_hover(group.clone(), |s| s.bg(t(idx::INTERACTIVE_TEXT_ACTIVE)))),
            )
    };
    let tabs = div()
        .flex()
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .child(tab(TAB_INFO, crate::t!("events.info")))
        .child(tab(TAB_RSVP, crate::t!("events.interested_tab", n = ev.user_count)));
    let panel = if d.tab == TAB_RSVP { users_panel(ev, d, &ent) } else { info_panel(app, ev, guild, &ent).into_any_element() };
    let close = on_close.clone();
    let card = div()
        .relative()
        .w(z(680.))
        .max_h(z(700.))
        .rounded(z(12.))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
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
        .overflow_hidden()
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .pr(z(60.))
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t(idx::TEXT_STRONG))
                .truncate()
                .child(ev.name.clone()),
        )
        .children(banner(ev, 240.).map(|b| div().mx(z(24.)).mt(z(8.)).rounded(z(8.)).overflow_hidden().child(b)))
        .child(div().pt(z(16.)).px(z(24.)).flex_1().min_h_0().flex().flex_col().child(tabs).child(div().id("event-panel").flex_1().min_h_0().overflow_y_scroll().child(panel)))
        .child(div().pt(z(16.)).px(z(24.)).pb(z(8.)).flex().justify_end().gap(z(8.)).children(actions(app, guild, ev, Some((s, &ent)))))
        .child(
            div()
                .id("event-detail-close")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(t(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER)))
                .on_click(move |_, w, cx| close(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(t(idx::CONTROL_ICON_ONLY_ICON_DEFAULT))),
        );
    crate::ui::modal::frame("event-detail-modal", on_close, card)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GuildEvent {
        GuildEvent { id: Id(42), name: "Movie night".into(), description: "Bring snacks, please".into(), start: 1_700_000_000, end: Some(1_700_003_600), ..Default::default() }
    }

    #[test]
    fn calendar_links_follow_the_client_queries() {
        let e = sample();
        let start = local_stamp(e.start, STAMP);
        let end = local_stamp(1_700_003_600, STAMP);
        assert_eq!(
            google_calendar_url(&e, "Discord"),
            format!("https://calendar.google.com/calendar/render?text=Movie%20night&dates={start}%2F{end}&details=Bring%20snacks%2C%20please&action=TEMPLATE&location=Discord")
        );
        assert!(yahoo_calendar_url(&e, "x").starts_with(&format!("https://calendar.yahoo.com/?v=60&title=Movie%20night&st={start}&et={end}&")));
        let outlook = outlook_calendar_url(&e, "x");
        assert!(outlook.starts_with("https://outlook.live.com/calendar/0/deeplink/compose?path=%2Fcalendar%2Faction%2Fcompose&rru=addevent&startdt="));
        assert!(outlook.ends_with("&allday=false"));
        assert_eq!(uri_component("a b'(c)~*!"), "a%20b'(c)~*!");
    }

    #[test]
    fn ics_has_one_escaped_event() {
        let ics = ics(&sample(), "Discord", 1_700_000_000);
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n"));
        assert!(ics.contains("\r\nDTSTART:20231114T221320Z\r\nDTEND:20231114T231320Z\r\n"));
        assert!(ics.contains("\r\nDESCRIPTION:Bring snacks\\, please\r\n"));
        assert!(ics.ends_with("END:VEVENT\r\nEND:VCALENDAR\r\n"));
    }

    #[test]
    fn external_events_use_their_location() {
        let mut e = sample();
        assert_eq!(event_location(&e, "G", None), "Discord");
        e.location = Some("Paris".into());
        assert_eq!(event_location(&e, "G", Some("general")), "Paris");
    }

    #[test]
    fn rsvps_keep_interested_base_events_only() {
        let v = serde_json::json!([
            { "guild_scheduled_event_id": "5", "user_id": "1", "response": 1 },
            { "guild_scheduled_event_id": "6", "user_id": "1", "response": 0 },
            { "guild_scheduled_event_id": "7", "user_id": "1", "guild_scheduled_event_exception_id": "9", "response": 1 },
            { "guild_scheduled_event_id": "8", "user_id": "1" }
        ]);
        let got = parse_rsvps(&v);
        assert_eq!(got, [Id(5), Id(8)].into_iter().collect::<HashSet<_>>());
    }

    #[test]
    fn event_users_prefer_the_server_nickname() {
        let v = serde_json::json!([
            { "user": { "id": "3", "username": "ann", "global_name": "Ann", "avatar": null }, "member": { "nick": "Annie" } },
            { "user": { "id": "4", "username": "bob", "global_name": null, "avatar": "abc" } },
            { "user": { "id": "4", "username": "bob", "global_name": null, "avatar": "abc" } }
        ]);
        let got = parse_users(&v);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name.as_ref(), "Annie");
        assert_eq!(got[1].name.as_ref(), "bob");
        assert_eq!(got[1].avatar.as_ref(), "https://cdn.discordapp.com/avatars/4/abc.webp?size=48");
    }

    #[test]
    fn hidden_users_count_only_past_the_first_page() {
        assert_eq!(not_shown(40, 40), 0);
        assert_eq!(not_shown(100, 100), 0);
        assert_eq!(not_shown(100, 130), 30);
    }

    #[test]
    fn join_label_follows_the_client() {
        assert_eq!(join_label(false, false, false), JoinLabel::Locked);
        assert_eq!(join_label(true, true, true), JoinLabel::GoToChannel);
        assert_eq!(join_label(false, true, true), JoinLabel::Listen);
        assert_eq!(join_label(false, true, false), JoinLabel::Join);
    }

    #[test]
    fn rsvp_toggle_moves_the_count_once() {
        let mut e = Events { guild: Id(1), list: Some(vec![GuildEvent { id: Id(5), user_count: 2, ..Default::default() }]), error: None, copied: None, detail: None, menu: None };
        let mut set = HashSet::new();
        apply_rsvp(&mut set, Some(&mut e), Id(5), true);
        apply_rsvp(&mut set, Some(&mut e), Id(5), true);
        assert_eq!(e.list.as_ref().unwrap()[0].user_count, 3);
        apply_rsvp(&mut set, Some(&mut e), Id(5), false);
        assert_eq!(e.list.as_ref().unwrap()[0].user_count, 2);
        assert!(set.is_empty());
    }
}
