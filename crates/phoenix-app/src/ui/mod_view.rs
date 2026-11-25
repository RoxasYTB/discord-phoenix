use std::cell::RefCell;

use gpui::{AnyElement, ClipboardItem, Context, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Stateful, Styled, div, prelude::*};
use phoenix_api::{models::Snowflake, rest::guild_manage::AdminCall};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    commands::state::ModKind,
    model::{Id, Message},
    theme::{c, generated::idx, role_color, tok, z},
};

pub const WIDTH: f32 = 480.;
pub const AREA_W: f32 = WIDTH - 8. - 16.;
pub const SLIDE_GAP: f32 = 12.;
pub const HISTORY_PAGE: u32 = 25;
pub const SPRING_TENSION: f32 = 170.;
pub const SPRING_FRICTION: f32 = 26.;
const SPRING_PRECISION: f32 = 0.001;
const SPRING_MAX_SECS: f32 = 1.5;
pub const MEDIA_EXTENSIONS: [&str; 12] = ["png", "jpg", "webp", "mov", "mp4", "qt", "gif", "mp3", "wav", "flac", "ogg", "opus"];

pub const ADMINISTRATOR: u32 = 3;
pub const KICK_MEMBERS: u32 = 1;
pub const BAN_MEMBERS: u32 = 2;
pub const MODERATE_MEMBERS: u32 = 40;
pub const MANAGE_ROLES: u32 = 28;

pub const MOD_PERMISSIONS: [u32; 16] = [3, 1, 2, 5, 4, 28, 13, 30, 43, 33, 44, 40, 17, 51, 34, 29];
pub const ELEVATED_PERMISSIONS: [u32; 9] = [1, 2, 3, 4, 5, 28, 13, 34, 40];

const FLAG_COMPLETED_ONBOARDING: u64 = 2;
const FLAG_BYPASSES_VERIFICATION: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Info,
    MessageHistory,
    Permissions,
    AuditLog,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Messages,
    Links,
    Media,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemberInfo {
    pub nick: Option<String>,
    pub joined_at: Option<i64>,
    pub roles: Vec<Id>,
    pub flags: u64,
    pub pending: Option<bool>,
    pub timeout_until: Option<i64>,
    pub unusual_dm_until: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoleInfo {
    pub id: Id,
    pub name: SharedString,
    pub color: Option<u32>,
    pub position: i32,
    pub permissions: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct JoinInfo {
    pub source_type: Option<u8>,
    pub invite_code: Option<String>,
    pub inviter: Option<Id>,
}

#[derive(Default)]
pub struct Data {
    pub member: Option<Option<MemberInfo>>,
    pub roles: Option<Vec<RoleInfo>>,
    pub counts: [Option<u32>; 3],
    pub recent: [Option<Vec<Message>>; 3],
    pub history_tab: Tab,
    pub history_page: u32,
    pub history_page_results: Option<Option<crate::ui::search::SearchPage>>,
    pub audit: Option<Result<Vec<Value>, ()>>,
    pub audit_users: Value,
    pub join: Option<JoinInfo>,
}

pub struct ModView {
    pub guild: Id,
    pub user: Id,
    pub panel: Panel,
    pub data: Data,
    pub refreshing: bool,
    pub perm_query: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub slide: Option<Slide>,
    seq: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Slide {
    pub from: Panel,
    pub forwards: bool,
    pub start: std::time::Instant,
}

pub fn spring(t: f32) -> f32 {
    let w0 = SPRING_TENSION.sqrt();
    let zeta = SPRING_FRICTION / (2. * SPRING_TENSION.sqrt());
    if zeta < 1. {
        let wd = w0 * (1. - zeta * zeta).sqrt();
        (-zeta * w0 * t).exp() * ((wd * t).cos() + (zeta * w0 / wd) * (wd * t).sin())
    } else {
        (-w0 * t).exp() * (1. + w0 * t)
    }
}

pub fn spring_done(t: f32) -> bool {
    let v = (spring(t + 0.001) - spring(t)) / 0.001;
    t >= SPRING_MAX_SECS || (t > 0. && spring(t).abs() < SPRING_PRECISION && v.abs() < SPRING_PRECISION * 10.)
}

pub fn slide_offsets(x: f32, forwards: bool) -> (f32, f32) {
    let d = AREA_W + SLIDE_GAP;
    if forwards { (x * d, -(1. - x) * d) } else { (-x * d, (1. - x) * d) }
}

thread_local! {
    static VIEW: RefCell<Option<ModView>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut ModView) -> R) -> Option<R> {
    VIEW.with(|v| v.borrow_mut().as_mut().map(f))
}

pub fn current() -> Option<(Id, Id, Panel)> {
    VIEW.with(|v| v.borrow().as_ref().map(|m| (m.guild, m.user, m.panel)))
}

pub fn is_open() -> bool {
    current().is_some()
}

pub fn close() {
    VIEW.with(|v| *v.borrow_mut() = None);
}

pub fn escape() -> bool {
    match current() {
        None => false,
        Some((_, _, Panel::Info)) => {
            close();
            true
        }
        Some(_) => {
            navigate(Panel::Info);
            true
        }
    }
}

pub fn settle() {
    with(|m| m.slide = None);
}

pub fn navigate(p: Panel) {
    with(|m| {
        if m.panel != p {
            m.slide = Some(Slide { from: m.panel, forwards: p != Panel::Info, start: std::time::Instant::now() });
            m.panel = p;
        }
    });
}

pub fn open(app: &mut PhoenixApp, guild: Id, user: Id, cx: &mut Context<PhoenixApp>) {
    app.people.close_overlays();
    let seq = VIEW.with(|v| v.borrow().as_ref().map_or(0, |m| m.seq) + 1);
    VIEW.with(|v| *v.borrow_mut() = Some(ModView { guild, user, panel: Panel::Info, data: Data::default(), refreshing: false, perm_query: None, slide: None, seq }));
    load(app, guild, user, seq, cx);
    cx.notify();
}

pub fn refresh(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some((guild, user, seq)) = with(|m| {
        m.refreshing = true;
        m.seq += 1;
        (m.guild, m.user, m.seq)
    }) else {
        return;
    };
    load(app, guild, user, seq, cx);
    cx.notify();
}

fn live(seq: u64) -> bool {
    VIEW.with(|v| v.borrow().as_ref().is_some_and(|m| m.seq == seq))
}

fn update(seq: u64, f: impl FnOnce(&mut Data)) {
    if live(seq) {
        with(|m| f(&mut m.data));
    }
}

pub fn search_request(guild: Id, user: Id, tab: Tab) -> crate::ui::search::SearchRequest {
    let mut r = crate::ui::search::SearchRequest { guild: Some(guild), authors: vec![user], ..Default::default() };
    match tab {
        Tab::Messages => {}
        Tab::Links => r.has = vec!["link".into()],
        Tab::Media => r.attachment_extensions = MEDIA_EXTENSIONS.iter().map(|s| (*s).to_owned()).collect(),
    }
    r
}

fn load(app: &mut PhoenixApp, guild: Id, user: Id, seq: u64, cx: &mut Context<PhoenixApp>) {
    let (g, u) = (Snowflake(guild.0), Snowflake(user.0));
    if !app.people.profiles.contains_key(&user) {
        let fut = app.backend.user_profile(user, Some(guild));
        cx.spawn(async move |this, cx| {
            let Ok(p) = fut.await else { return };
            let _ = this.update(cx, |a, cx| {
                a.people.profiles.insert(user, p);
                cx.notify();
            });
        })
        .detach();
    }
    app.admin(AdminCall::Member { guild: g, user: u }, cx, move |_, r, cx| {
        update(seq, |d| d.member = Some(r.ok().map(|v| parse_member(&v))));
        cx.notify();
    });
    app.admin(AdminCall::Roles { guild: g }, cx, move |_, r, cx| {
        update(seq, |d| d.roles = Some(r.map(|v| parse_roles(&v)).unwrap_or_default()));
        cx.notify();
    });
    app.admin(AdminCall::AuditLogTarget { guild: g, target: u, limit: 50 }, cx, move |_, r, cx| {
        update(seq, |d| match r {
            Ok(v) => {
                d.audit = Some(Ok(v.get("audit_log_entries").and_then(Value::as_array).cloned().unwrap_or_default()));
                d.audit_users = v.get("users").cloned().unwrap_or(Value::Null);
            }
            Err(_) => d.audit = Some(Err(())),
        });
        with(|m| m.refreshing = false);
        cx.notify();
    });
    app.admin(AdminCall::MemberSupplemental { guild: g, users: vec![u] }, cx, move |_, r, cx| {
        update(seq, |d| d.join = Some(r.ok().and_then(|v| parse_supplemental(&v, user)).unwrap_or_default()));
        cx.notify();
    });
    for (i, tab) in [Tab::Messages, Tab::Links, Tab::Media].into_iter().enumerate() {
        let fut = app.backend.search_messages(search_request(guild, user, tab));
        cx.spawn(async move |this, cx| {
            let r = fut.await;
            let _ = this.update(cx, |_, cx| {
                update(seq, |d| {
                    d.counts[i] = Some(r.as_ref().map(|p| p.total).unwrap_or(0));
                    d.recent[i] = Some(r.map(|p| p.messages).unwrap_or_default());
                });
                cx.notify();
            });
        })
        .detach();
    }
}

fn ts(v: Option<&Value>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(v?.as_str()?).ok().map(|d| d.timestamp())
}

fn sf(v: &Value) -> Option<u64> {
    v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_u64())
}

pub fn parse_member(v: &Value) -> MemberInfo {
    MemberInfo {
        nick: v.get("nick").and_then(Value::as_str).map(str::to_owned),
        joined_at: ts(v.get("joined_at")),
        roles: v.get("roles").and_then(Value::as_array).into_iter().flatten().filter_map(sf).map(Id).collect(),
        flags: v.get("flags").and_then(Value::as_u64).unwrap_or(0),
        pending: v.get("pending").and_then(Value::as_bool),
        timeout_until: ts(v.get("communication_disabled_until")),
        unusual_dm_until: ts(v.get("unusual_dm_activity_until")),
    }
}

pub fn parse_roles(v: &Value) -> Vec<RoleInfo> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let color = r.get("colors").and_then(|c| c.get("primary_color")).and_then(Value::as_u64).or_else(|| r.get("color").and_then(Value::as_u64)).filter(|c| *c != 0);
            Some(RoleInfo {
                id: Id(sf(r.get("id")?)?),
                name: r.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(),
                color: color.map(|c| c as u32),
                position: r.get("position").and_then(Value::as_i64).unwrap_or(0) as i32,
                permissions: r.get("permissions").and_then(|p| p.as_str().and_then(|s| s.parse().ok()).or_else(|| p.as_u64())).unwrap_or(0),
            })
        })
        .collect()
}

pub fn parse_supplemental(v: &Value, user: Id) -> Option<JoinInfo> {
    let e = v.as_array()?.iter().find(|e| e.get("user_id").and_then(sf) == Some(user.0))?;
    let code = e.get("source_invite_code").and_then(Value::as_str).map(str::to_owned);
    let mut kind = e.get("join_source_type").and_then(Value::as_u64).map(|k| k as u8);
    if kind.is_none_or(|k| k == 0) && code.is_some() {
        kind = Some(5);
    }
    Some(JoinInfo { source_type: kind, invite_code: code, inviter: e.get("inviter_id").and_then(sf).map(Id) })
}

fn has(perms: u64, bit: u32) -> bool {
    perms & (1u64 << bit) != 0
}

pub fn member_permissions(roles: &[RoleInfo], guild: Id, member_roles: &[Id], owner: bool) -> u64 {
    if owner {
        return u64::MAX;
    }
    let mut p = roles.iter().find(|r| r.id == guild).map_or(0, |r| r.permissions);
    for r in roles.iter().filter(|r| member_roles.contains(&r.id)) {
        p |= r.permissions;
    }
    if has(p, ADMINISTRATOR) { u64::MAX } else { p }
}

pub fn granted_by(roles: &[RoleInfo], guild: Id, member_roles: &[Id], user: Id, owner: bool, bit: u32) -> Option<Vec<Id>> {
    if !has(member_permissions(roles, guild, member_roles, owner), bit) {
        return None;
    }
    let grants = |r: &RoleInfo| has(r.permissions, ADMINISTRATOR) || has(r.permissions, bit);
    let mut out: Vec<Id> = member_roles.iter().filter(|id| roles.iter().any(|r| r.id == **id && grants(r))).copied().collect();
    if roles.iter().any(|r| r.id == guild && grants(r)) {
        out.push(guild);
    }
    if owner {
        out.push(user);
    }
    Some(out)
}

pub fn spec_bits() -> impl Iterator<Item = (&'static str, u32)> {
    crate::ui::modals::server_settings::roles::PERMS.iter().flat_map(|(t, bits)| bits.iter().map(move |b| (*t, *b)))
}

pub fn mod_chips(roles: &[RoleInfo], guild: Id, member_roles: &[Id], owner: bool) -> Vec<u32> {
    let p = member_permissions(roles, guild, member_roles, owner);
    spec_bits().map(|(_, b)| b).filter(|b| MOD_PERMISSIONS.contains(b) && has(p, *b)).collect()
}

pub fn granted_count(roles: &[RoleInfo], guild: Id, member_roles: &[Id], owner: bool) -> usize {
    let p = member_permissions(roles, guild, member_roles, owner);
    spec_bits().filter(|(_, b)| has(p, *b)).count()
}

pub fn rules_state(features: &[String], m: &MemberInfo) -> Option<bool> {
    if !features.iter().any(|f| f == "MEMBER_VERIFICATION_GATE_ENABLED") {
        return None;
    }
    Some(m.flags & (FLAG_BYPASSES_VERIFICATION | FLAG_COMPLETED_ONBOARDING) != 0 || m.pending == Some(false))
}

fn highest(roles: &[RoleInfo], ids: &[Id]) -> i32 {
    roles.iter().filter(|r| ids.contains(&r.id)).map(|r| r.position).max().unwrap_or(0)
}

pub fn outranks(roles: &[RoleInfo], my_roles: &[Id], me_owner: bool, target_roles: &[Id], target_owner: bool) -> bool {
    !target_owner && (me_owner || highest(roles, my_roles) > highest(roles, target_roles))
}

pub fn countdown(secs: i64) -> String {
    let s = secs.max(0);
    let (d, h, m, s) = (s / 86_400, s / 3600 % 24, s / 60 % 60, s % 60);
    let mut parts = Vec::new();
    for (v, u) in [(d, "d"), (h, "h"), (m, "m")] {
        if v > 0 || !parts.is_empty() {
            parts.push(format!("{v}{u}"));
        }
    }
    parts.push(format!("{}s", s.max(1)));
    parts.join(" ")
}

fn tk(i: usize) -> Hsla {
    c(tok(i))
}

fn mix80(i: usize) -> Hsla {
    let mut h = tk(i);
    h.a *= 0.8;
    h
}

fn shadow(stroke: bool, low: bool) -> Vec<gpui::BoxShadow> {
    let light = crate::theme::is_light();
    let mk = |y: f32, spread: f32, a: f32| gpui::BoxShadow { color: gpui::hsla(0., 0., 0., a), offset: gpui::point(z(0.), z(y)), blur_radius: z(0.), spread_radius: z(spread), inset: false };
    let mut v = Vec::new();
    if stroke {
        v.push(mk(0., 1., if light { 0.08 } else { 0.16 }));
    }
    if low {
        v.push(mk(1., 0., if light { 0.12 } else { 0.2 }));
        v.push(mk(1.5, 0., 0.04));
        v.push(mk(2., 0., 0.04));
    }
    v
}

fn icon(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(color)
}

fn text(t: impl Into<SharedString>, size: f32, line: f32, weight: FontWeight, color: usize) -> Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(tk(color)).child(t.into())
}

fn body_sm(t: impl Into<SharedString>, weight: FontWeight, color: usize) -> Div {
    text(t, 14., 18., weight, color)
}

fn spinner() -> Div {
    div().size(z(16.)).flex_none().rounded_full().border_2().border_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)).opacity(0.5)
}

fn key_combo(id: &'static str, cx: &mut Context<PhoenixApp>, back: bool) -> Stateful<Div> {
    div()
        .id(id)
        .rounded(z(8.))
        .bg(tk(idx::INTERACTIVE_BACKGROUND_SELECTED))
        .cursor_pointer()
        .child(
            div()
                .h(z(23.))
                .min_w(z(14.))
                .pt(z(3.))
                .pb(z(4.))
                .px(z(6.))
                .rounded(z(4.))
                .border_1()
                .border_color(tk(idx::INTERACTIVE_BACKGROUND_HOVER))
                .bg(tk(if back { idx::BACKGROUND_MOD_SUBTLE } else { idx::INTERACTIVE_BACKGROUND_SELECTED }))
                .shadow(vec![gpui::BoxShadow { color: tk(idx::INTERACTIVE_BACKGROUND_SELECTED), offset: gpui::point(z(0.), z(-4.)), blur_radius: z(0.), spread_radius: z(0.), inset: true }])
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(12.))
                .line_height(z(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(crate::t!("modview.esc").to_uppercase()),
        )
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.stop_propagation();
            if back {
                navigate(Panel::Info);
            } else {
                close();
            }
            cx.notify();
        }))
}

fn panel_header(title: Div, trailing: Option<AnyElement>, back: bool, cx: &mut Context<PhoenixApp>) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(z(8.))
        .px(z(16.))
        .py(z(12.))
        .bg(mix80(idx::BACKGROUND_BASE_LOWER))
        .shadow(shadow(true, true))
        .child(title.flex_1())
        .child(div().flex().items_center().gap(z(8.)).children(trailing).child(key_combo(if back { "modview-esc-back" } else { "modview-esc" }, cx, back)))
}

fn back_title(cx: &mut Context<PhoenixApp>) -> Div {
    div().flex().items_center().child(
        div()
            .id("modview-back")
            .flex()
            .items_center()
            .gap(z(4.))
            .cursor_pointer()
            .child(icon("settings/arrow-left", 20., tk(idx::TEXT_STRONG)))
            .child(text(crate::t!("modview.back"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG))
            .on_click(cx.listener(|_, _, _, cx| {
                navigate(Panel::Info);
                cx.notify();
            })),
    )
}

fn container(children: Vec<AnyElement>) -> Div {
    div().flex().flex_col().rounded(z(8.)).overflow_hidden().shadow(shadow(true, false)).children(children)
}

struct Row {
    icon: Option<AnyElement>,
    name: Option<AnyElement>,
    value: AnyElement,
    nav: Option<Panel>,
    tab: Option<Tab>,
    id: &'static str,
}

fn row(r: Row, last: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (nav, tab, group) = (r.nav, r.tab, SharedString::from(r.id));
    let wide = r.name.is_none();
    div()
        .id(r.id)
        .flex()
        .items_center()
        .gap(z(8.))
        .p(z(12.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .shadow(shadow(true, false))
        .when(!last, |d| d.mb(z(1.)))
        .children(r.icon.map(|i| div().flex_none().size(z(16.)).flex().items_center().justify_center().child(i)))
        .children(r.name.map(|n| div().flex_1().min_w_0().child(n)))
        .child(div().when(wide, |d| d.flex_1().min_w_0()).child(r.value))
        .when_some(nav, |d, p| {
            d.cursor_pointer()
                .group(group.clone())
                .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                .child(icon("mod/chevron-large-right", 16., tk(idx::TEXT_MUTED)).group_hover(group, |s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    if let Some(t) = tab {
                        with(|m| {
                            m.data.history_tab = t;
                            m.data.history_page = 0;
                            m.data.history_page_results = None;
                        });
                    }
                    navigate(p);
                    cx.notify();
                }))
        })
        .into_any_element()
}

fn rows(list: Vec<Row>, cx: &mut Context<PhoenixApp>) -> Div {
    let n = list.len();
    container(list.into_iter().enumerate().map(|(i, r)| row(r, i + 1 == n, cx)).collect())
}

fn section(label: SharedString, body: impl IntoElement) -> Div {
    div().flex().flex_col().gap(z(8.)).child(text(label, 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(body)
}

fn check(ok: bool) -> AnyElement {
    if ok {
        icon("check", 16., tk(idx::STATUS_POSITIVE)).into_any_element()
    } else {
        icon("close", 16., tk(idx::ICON_FEEDBACK_CRITICAL)).into_any_element()
    }
}

fn value(t: impl Into<SharedString>) -> AnyElement {
    body_sm(t, FontWeight::MEDIUM, idx::TEXT_DEFAULT).into_any_element()
}

fn role_pill(r: &RoleInfo, highest: bool) -> Div {
    div()
        .flex()
        .items_center()
        .min_h(z(24.))
        .max_w(z(400.))
        .pl(z(2.))
        .pr(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::USER_PROFILE_BORDER))
        .when(highest, |d| d.shadow(shadow(true, false)))
        .text_size(z(12.))
        .line_height(z(16.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .child(div().size(z(12.)).mx(z(4.)).flex_none().rounded_full().bg(role_color(r.color.or(Some(0x99AAB5)))))
        .child(div().min_w_0().truncate().child(r.name.clone()))
}

fn chip(bit: u32) -> Div {
    let elevated = ELEVATED_PERMISSIONS.contains(&bit);
    div()
        .flex()
        .items_center()
        .gap(z(4.))
        .p(z(4.))
        .rounded(z(4.))
        .bg(tk(if elevated { idx::BACKGROUND_MOD_STRONG } else { idx::BACKGROUND_MOD_SUBTLE }))
        .when(elevated, |d| d.child(icon("shield-lock", 16., tk(idx::TEXT_BRAND))))
        .child(text(crate::t!(&format!("gs.perm.p{bit}")), 12., 16., FontWeight::MEDIUM, idx::INTERACTIVE_TEXT_DEFAULT))
}

struct Ctx<'a> {
    guild: Id,
    user: Id,
    owner: bool,
    member: Option<&'a MemberInfo>,
    roles: &'a [RoleInfo],
    member_roles: Vec<Id>,
}

fn header_card(app: &PhoenixApp, x: &Ctx, cx: &mut Context<PhoenixApp>) -> Div {
    let u = app.member_user(Some(x.guild), x.user);
    let u = u.as_deref();
    let name: SharedString = x
        .member
        .and_then(|m| m.nick.clone())
        .map(SharedString::from)
        .or_else(|| u.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("common.unknown_user"));
    let username: SharedString = u.map(|u| u.name.clone()).unwrap_or_default();
    let top = div()
        .flex()
        .items_center()
        .justify_between()
        .px(z(8.))
        .py(z(12.))
        .bg(mix80(idx::BACKGROUND_BASE_LOWER))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .min_w_0()
                .child(crate::ui::avatar_with_status(u, 48., app.presence_of(x.user), mix80(idx::BACKGROUND_BASE_LOWER)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(z(8.))
                                .min_w_0()
                                .child(text(name, 20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG).min_w_0().truncate())
                                .when_some(u.and_then(|u| u.tag.clone()), |d, t| {
                                    let gid = u.and_then(|u| u.tag_guild);
                                    let tip = crate::t!("tag_popout.tooltip", tag = t.0.as_ref());
                                    let (tag_s, badge_s) = (t.0.clone(), t.1.clone());
                                    d.child(
                                        clan_tag(&t)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_MUTED)))
                                            .tooltip(crate::ui::tooltip::tip(tip))
                                            .when_some(gid, |el, gid| {
                                                el.on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(move |a, ev: &gpui::ClickEvent, w, cx| {
                                                    cx.stop_propagation();
                                                    a.open_tag_popout(gid, tag_s.clone(), badge_s.clone(), ev.position(), w, cx);
                                                }))
                                            }),
                                    )
                                }),
                        )
                        .child(text(username, 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).truncate()),
                ),
        )
        .child(
            div()
                .id("modview-close")
                .p(z(4.))
                .cursor_pointer()
                .child(icon("close-small", 24., tk(idx::TEXT_DEFAULT)))
                .on_click(cx.listener(|_, _, _, cx| {
                    close();
                    cx.notify();
                })),
        );
    let me = app.me.id;
    let my_roles = app.members.get(&x.guild).and_then(|m| m.iter().find(|m| m.user == me)).map(|m| m.roles.clone()).unwrap_or_default();
    let me_owner = app.guilds.iter().find(|g| g.id == x.guild).and_then(|g| g.owner) == Some(me);
    let perms = app.backend.guild_permissions(x.guild);
    let above = outranks(x.roles, &my_roles, me_owner, &x.member_roles, x.owner);
    let can = |bit: u32| (has(perms, ADMINISTRATOR) || has(perms, bit)) && above && x.user != me;
    let in_guild = x.member.is_some();
    let timed_out = x.member.and_then(|m| m.timeout_until).is_some_and(|t| t > chrono::Utc::now().timestamp());
    let (g, uid) = (x.guild, x.user);
    let mut actions = vec![action("modview-dm", "chat", crate::t!("modview.message"), x.user == me, cx, move |a, cx| a.message_user(uid, cx))];
    if in_guild {
        actions.push(action("modview-kick", "mod/user-minus", crate::t!("modview.kick"), !can(KICK_MEMBERS), cx, move |a, cx| a.open_moderation(g, uid, ModKind::Kick, cx)));
    }
    actions.push(action("modview-ban", "mod/hammer", crate::t!("modview.ban"), !can(BAN_MEMBERS), cx, move |a, cx| a.open_moderation(g, uid, ModKind::Ban(0), cx)));
    if in_guild {
        let (label, kind) = if timed_out {
            (crate::t!("modview.remove_timeout"), ModKind::Timeout(0))
        } else {
            (crate::t!("modview.timeout"), ModKind::Timeout(crate::ui::inbox::TIMEOUTS[0].1))
        };
        actions.push(action("modview-timeout", "mod/clock-warning", label, !can(MODERATE_MEMBERS), cx, move |a, cx| a.open_moderation(g, uid, kind, cx)));
    }
    actions.push(action("modview-copy-id", "id", crate::t!("modview.copy_id"), false, cx, move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(uid.0.to_string()));
    }));
    div()
        .flex_none()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .shadow(shadow(false, true))
        .child(top)
        .child(div().mt(z(1.)).flex().items_center().justify_between().gap(z(1.)).children(actions))
}

pub fn clan_tag(tag: &(SharedString, Option<SharedString>)) -> Stateful<Div> {
    div()
        .id("modview-clan-tag")
        .flex_none()
        .mt(z(-2.))
        .p(z(4.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_MOD_STRONG))
        .flex()
        .items_center()
        .text_size(z(16.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::TEXT_DEFAULT))
        .when_some(tag.1.clone(), |d, url| d.child(gpui::img(url).size(z(16.)).mr(z(2.)).flex_none()))
        .child(div().max_w(z(70.)).overflow_hidden().whitespace_nowrap().child(tag.0.clone()))
}

fn action(
    id: &'static str,
    glyph: &'static str,
    label: SharedString,
    disabled: bool,
    cx: &mut Context<PhoenixApp>,
    f: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
) -> AnyElement {
    let inner = div()
        .id((id, 1usize))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(z(4.))
        .h(z(52.))
        .px(z(8.))
        .py(z(12.))
        .w_full()
        .map(|d| {
            if disabled {
                d.bg(tk(idx::BACKGROUND_MOD_SUBTLE)).text_color(tk(idx::TEXT_MUTED)).opacity(0.5)
            } else {
                d.text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)).hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_SELECTED)).text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
            }
        })
        .child(gpui::svg().path(crate::icons::path(glyph)).size(z(24.)).flex_none().text_color(tk(if disabled { idx::TEXT_MUTED } else { idx::TEXT_DEFAULT })));
    div()
        .id(id)
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(mix80(idx::BACKGROUND_BASE_LOWER))
        .child(inner)
        .tooltip(crate::ui::tooltip::tip(label))
        .map(|d| {
            if disabled {
                d.cursor(gpui::CursorStyle::OperationNotAllowed)
            } else {
                d.cursor_pointer().on_click(cx.listener(move |a, _, _, cx| f(a, cx)))
            }
        })
        .into_any_element()
}

fn signals(x: &Ctx, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let m = x.member?;
    let now = chrono::Utc::now().timestamp();
    let mut list = Vec::new();
    if let Some(t) = m.unusual_dm_until {
        list.push(Row {
            icon: Some(icon("mod/chat-warning", 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
            name: Some(body_sm(crate::t!("modview.unusual_dm"), FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).into_any_element()),
            value: body_sm(crate::presence::format_date(t), FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).into_any_element(),
            nav: None,
            tab: None,
            id: "modview-unusual-dm",
        });
    }
    if let Some(t) = m.timeout_until.filter(|t| *t > now) {
        list.push(Row {
            icon: Some(icon("mod/clock-warning", 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
            name: Some(body_sm(crate::t!("modview.timed_out"), FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).into_any_element()),
            value: body_sm(countdown(t - now), FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).into_any_element(),
            nav: None,
            tab: None,
            id: "modview-timed-out",
        });
    }
    (!list.is_empty()).then(|| section(crate::t!("modview.signals"), rows(list, cx)).into_any_element())
}

fn count_value(n: Option<u32>) -> AnyElement {
    match n {
        None => spinner().into_any_element(),
        Some(n) => body_sm(crate::ui::search::group_digits(n), FontWeight::SEMIBOLD, idx::INTERACTIVE_TEXT_DEFAULT).into_any_element(),
    }
}

fn activity(d: &Data, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let item = |id: &'static str, glyph: &str, label: SharedString, n: Option<u32>, tab: Tab| Row {
        icon: Some(icon(glyph, 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
        name: Some(body_sm(label, FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).into_any_element()),
        value: count_value(n),
        nav: n.filter(|n| *n > 0).map(|_| Panel::MessageHistory),
        tab: Some(tab),
        id,
    };
    let mut list = vec![
        item("modview-messages", "chat", crate::t!("modview.messages"), d.counts[0], Tab::Messages),
        item("modview-links", "chat/link", crate::t!("modview.links"), d.counts[1], Tab::Links),
        item("modview-media", "images", crate::t!("modview.media"), d.counts[2], Tab::Media),
    ];
    let (audit_value, audit_nav) = match &d.audit {
        None => (spinner().into_any_element(), None),
        Some(Err(())) => (body_sm(crate::t!("modview.awkward"), FontWeight::SEMIBOLD, idx::TEXT_FEEDBACK_CRITICAL).into_any_element(), None),
        Some(Ok(v)) => (count_value(Some(v.len() as u32)), (!v.is_empty()).then_some(Panel::AuditLog)),
    };
    list.push(Row {
        icon: Some(icon("settings/shield-user", 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
        name: Some(body_sm(crate::t!("modview.audit_log"), FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).into_any_element()),
        value: audit_value,
        nav: audit_nav,
        tab: None,
        id: "modview-audit",
    });
    section(crate::t!("modview.server_activity"), rows(list, cx)).into_any_element()
}

fn mod_permissions(x: &Ctx, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    x.member?;
    let chips = mod_chips(x.roles, x.guild, &x.member_roles, x.owner);
    let all = granted_count(x.roles, x.guild, &x.member_roles, x.owner);
    let head = div()
        .mb(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(8.))
        .child(text(crate::t!("modview.mod_permissions"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG))
        .child(
            div()
                .id("modview-all-perms")
                .flex()
                .items_center()
                .cursor_pointer()
                .child(body_sm(crate::t!("modview.all_count", n = all), FontWeight::MEDIUM, idx::INTERACTIVE_TEXT_DEFAULT))
                .child(icon("chevron-right", 16., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                .on_click(cx.listener(|_, _, _, cx| {
                    navigate(Panel::Permissions);
                    cx.notify();
                })),
        );
    let body = if chips.is_empty() {
        div().flex().p(z(4.)).rounded(z(4.)).bg(tk(idx::BACKGROUND_MOD_SUBTLE)).child(body_sm(crate::t!("modview.no_mod_permissions"), FontWeight::NORMAL, idx::TEXT_FEEDBACK_POSITIVE))
    } else {
        div().flex().flex_wrap().gap(z(8.)).children(chips.into_iter().map(chip))
    };
    let item = div().flex().items_start().p(z(12.)).bg(tk(idx::BACKGROUND_BASE_LOWER)).shadow(shadow(true, false)).child(div().w_full().child(body));
    Some(div().flex().flex_col().child(head).child(container(vec![item.into_any_element()])).into_any_element())
}

fn roles_section(app: &PhoenixApp, x: &Ctx, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    x.member?;
    let mut mine: Vec<&RoleInfo> = x.roles.iter().filter(|r| x.member_roles.contains(&r.id)).collect();
    mine.sort_by_key(|r| -r.position);
    let can_add = has(app.backend.guild_permissions(x.guild), ADMINISTRATOR) || has(app.backend.guild_permissions(x.guild), MANAGE_ROLES);
    let body = if mine.is_empty() && !can_add {
        div().flex().p(z(4.)).rounded(z(4.)).bg(tk(idx::BACKGROUND_MOD_SUBTLE)).child(body_sm(crate::t!("modview.no_roles"), FontWeight::NORMAL, idx::TEXT_DEFAULT))
    } else {
        let (g, u) = (x.guild, x.user);
        let add = can_add.then(|| {
            div()
                .id("modview-add-role")
                .cursor_pointer()
                .tooltip(crate::ui::tooltip::tip(crate::t!("modview.add_role")))
                .child(icon("plus-circle", 20., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                .on_click(cx.listener(move |_, ev: &gpui::ClickEvent, w, cx| {
                    crate::ui::extra_menus::open(crate::ui::extra_menus::Target::ModViewRoles { user: u, guild: g }, ev.position(), w);
                    cx.notify();
                }))
        });
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(z(4.))
            .children(mine.iter().enumerate().map(|(i, r)| {
                let pill = role_pill(r, i == 0);
                if i == 0 {
                    div().id("modview-highest-role").child(pill).tooltip(crate::ui::tooltip::tip(crate::t!("modview.highest_role"))).into_any_element()
                } else {
                    pill.into_any_element()
                }
            }))
            .children(add)
    };
    Some(section(crate::t!("modview.roles"), rows(vec![Row { icon: None, name: None, value: body.into_any_element(), nav: None, tab: None, id: "modview-roles" }], cx)).into_any_element())
}

fn join_method(app: &PhoenixApp, x: &Ctx, d: &Data) -> AnyElement {
    let unknown = || {
        div().id("modview-join-unknown").child(body_sm(crate::t!("modview.join_unknown"), FontWeight::MEDIUM, idx::TEXT_MUTED)).tooltip(crate::ui::tooltip::tip(crate::t!("modview.join_unavailable"))).into_any_element()
    };
    let Some(j) = &d.join else { return spinner().into_any_element() };
    let Some(kind) = j.source_type.filter(|k| *k != 0) else { return unknown() };
    let (glyph, label): (Option<&str>, SharedString) = match kind {
        1 => (Some("mod/robot"), crate::t!("modview.join_bot")),
        2 => (Some("settings/webhook"), crate::t!("modview.join_integration")),
        3 => (Some("compass"), crate::t!("modview.join_discovery")),
        4 => (None, crate::t!("modview.join_hub")),
        5 | 6 => (Some("chat/link"), j.invite_code.clone().map(SharedString::from).unwrap_or_else(|| crate::t!("modview.join_invite"))),
        7 => (None, j.invite_code.clone().map(|c| crate::t!("modview.join_manual_code", code = c)).unwrap_or_else(|| crate::t!("modview.join_manual"))),
        8 => (None, crate::t!("modview.join_linked")),
        _ => return unknown(),
    };
    let pill = div()
        .flex()
        .items_center()
        .gap(z(4.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .children(glyph.map(|g| icon(g, 12., tk(if kind == 1 { idx::TEXT_LINK } else { idx::TEXT_DEFAULT }))))
        .child(body_sm(label, FontWeight::MEDIUM, idx::TEXT_DEFAULT).whitespace_nowrap());
    let inviter = j.inviter.and_then(|i| app.member_user(Some(x.guild), i).map(|u| u.into_owned()));
    let footer = inviter.map(|u| {
        div()
            .mt(z(4.))
            .flex()
            .items_center()
            .gap(z(4.))
            .child(text(if kind == 1 { crate::t!("modview.added_by_bot") } else { crate::t!("modview.invited_by") }, 12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
            .child(crate::ui::avatar(Some(&u), 16.))
            .child(text(u.display_name.clone(), 12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
    });
    div().flex().flex_col().items_end().child(pill).children(footer).into_any_element()
}

fn account(app: &PhoenixApp, x: &Ctx, d: &Data, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = |t: SharedString| body_sm(t, FontWeight::MEDIUM, idx::TEXT_DEFAULT).into_any_element();
    let mut list = Vec::new();
    if let Some(m) = x.member {
        list.push(Row {
            icon: Some(icon("settings/shield-user", 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
            name: Some(name(crate::t!("modview.verification"))),
            value: check(m.joined_at.is_some() || m.flags & FLAG_BYPASSES_VERIFICATION != 0),
            nav: None,
            tab: None,
            id: "modview-verified",
        });
        if let Some(agreed) = rules_state(&app.backend.guild_features(x.guild), m) {
            list.push(Row { icon: Some(icon("forum/book-check", 16., tk(idx::TEXT_DEFAULT)).into_any_element()), name: Some(name(crate::t!("modview.rules"))), value: check(agreed), nav: None, tab: None, id: "modview-rules" });
        }
    }
    list.push(Row {
        icon: Some(icon("discord-logo", 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
        name: Some(name(crate::t!("modview.discord_join"))),
        value: value(crate::presence::format_date(crate::ui::modals::server_settings::kit::snowflake_unix(x.user.0))),
        nav: None,
        tab: None,
        id: "modview-created",
    });
    if let Some(m) = x.member {
        let g = app.guilds.iter().find(|g| g.id == x.guild);
        list.push(Row {
            icon: Some(div().size(z(16.)).rounded(z(8.)).overflow_hidden().child(crate::ui::invite::guild_icon(g.map(|g| &g.icon), g.map_or("", |g| g.name.as_ref()), 16. * crate::theme::zoom())).into_any_element()),
            name: Some(name(crate::t!("modview.server_join"))),
            value: value(m.joined_at.map(crate::presence::format_date).unwrap_or_default()),
            nav: None,
            tab: None,
            id: "modview-joined",
        });
        list.push(Row {
            icon: Some(icon("add-to-dm", 16., tk(idx::TEXT_DEFAULT)).into_any_element()),
            name: Some(name(crate::t!("modview.join_method"))),
            value: join_method(app, x, d),
            nav: None,
            tab: None,
            id: "modview-join-method",
        });
    }
    section(crate::t!("modview.account"), rows(list, cx)).into_any_element()
}

fn scroller(id: &'static str, body: Div) -> AnyElement {
    let h = crate::ui::chat::scrollbar::handle(id);
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .child(div().id(id).size_full().overflow_y_scroll().track_scroll(&h).child(body))
        .children(crate::ui::chat::scrollbar::thin_scrollbar_div(&h))
        .into_any_element()
}

fn info_panel(app: &PhoenixApp, m: &ModView, x: &Ctx, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let refreshing = m.refreshing;
    let refresh = div()
        .id("modview-refresh")
        .size(z(24.))
        .rounded(z(4.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
        .tooltip(crate::ui::tooltip::tip(crate::t!("modview.refresh")))
        .map(|d| {
            if refreshing {
                d.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed).child(div().size(z(16.)).flex().items_center().justify_center().child(spinner().size(z(8.))))
            } else {
                d.cursor_pointer()
                    .hover(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
                    .child(icon("refresh", 12., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .on_click(cx.listener(|a, _, _, cx| refresh(a, cx)))
            }
        });
    let title = div()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(icon("mod/shield-user", 16., tk(idx::TEXT_STRONG)))
        .child(text(crate::t!("modview.title"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG));
    let body = div()
        .flex()
        .flex_col()
        .gap(z(20.))
        .px(z(16.))
        .py(z(12.))
        .children(signals(x, cx))
        .child(activity(&m.data, cx))
        .children(mod_permissions(x, cx))
        .children(roles_section(app, x, cx))
        .child(account(app, x, &m.data, cx));
    panel_frame(vec![panel_header(title, Some(refresh.into_any_element()), false, cx).into_any_element(), scroller("modview-info-scroll", body)])
}

fn panel_frame(children: Vec<AnyElement>) -> AnyElement {
    div().flex_1().min_h_0().flex().flex_col().rounded(z(8.)).overflow_hidden().bg(mix80(idx::BACKGROUND_SURFACE_HIGH)).shadow(shadow(false, true)).children(children).into_any_element()
}

fn permissions_panel(m: &ModView, x: &Ctx, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let q = m.perm_query.as_ref().map(|i| i.read(cx).value().to_lowercase()).unwrap_or_default();
    let mut list = div().flex().flex_col().gap(z(12.));
    if let Some(i) = &m.perm_query {
        list = list.child(crate::ui::modals::server_settings::kit::search_box(Some(i)));
    }
    if x.member.is_none() {
        list = list.child(body_sm(crate::t!("modview.not_member"), FontWeight::NORMAL, idx::TEXT_MUTED));
    } else {
        let mut n = 0;
        for (cat, bit) in spec_bits() {
            let Some(by) = granted_by(x.roles, x.guild, &x.member_roles, x.user, x.owner, bit) else { continue };
            let title = crate::t!(&format!("gs.perm.p{bit}"));
            let desc = crate::t!(&format!("gs.perm.p{bit}_desc"));
            let category = crate::t!(cat);
            let elevated = ELEVATED_PERMISSIONS.contains(&bit);
            let moderator = MOD_PERMISSIONS.contains(&bit);
            let tag = if elevated { Some(crate::t!("modview.elevated")) } else if moderator { Some(crate::t!("modview.moderator")) } else { None };
            if !q.is_empty() && ![&title, &desc, &category].iter().any(|s| s.to_lowercase().contains(&q)) && !tag.as_ref().is_some_and(|t| t.to_lowercase().contains(&q)) {
                continue;
            }
            n += 1;
            list = list.child(permission_item(x, title, desc, category, tag, elevated, by));
        }
        if n == 0 {
            list = list.child(body_sm(crate::t!("modview.no_permissions_found"), FontWeight::NORMAL, idx::TEXT_DEFAULT));
        }
    }
    let body = div().p(z(16.)).flex().flex_col().gap(z(8.)).child(text(crate::t!("modview.all_granted"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(list);
    panel_frame(vec![panel_header(back_title(cx), None, true, cx).into_any_element(), scroller("modview-perms-scroll", body)])
}

#[allow(clippy::too_many_arguments)]
fn permission_item(x: &Ctx, title: SharedString, desc: SharedString, category: SharedString, tag: Option<SharedString>, elevated: bool, by: Vec<Id>) -> Div {
    let dot = || div().size(z(4.)).rounded_full().bg(tk(idx::ICON_MUTED));
    let sub = div()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(text(category, 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED))
        .when_some(tag, |d, t| {
            d.child(dot()).child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(2.))
                    .when(elevated, |d| d.child(icon("shield-lock", 12., tk(idx::TEXT_BRAND))))
                    .child(text(t, 12., 16., FontWeight::NORMAL, if elevated { idx::TEXT_BRAND } else { idx::TEXT_MUTED })),
            )
        });
    let chips = by.iter().filter_map(|id| {
        let pill = |label: SharedString, color: Option<u32>| {
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .p(z(4.))
                .rounded(z(4.))
                .bg(tk(idx::BACKGROUND_MOD_STRONG))
                .when_some(color, |d, c| d.child(div().size(z(12.)).rounded_full().bg(role_color(Some(c)))))
                .child(text(label, 12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
        };
        if *id == x.user {
            Some(pill(crate::t!("modview.server_owner"), None))
        } else {
            x.roles.iter().find(|r| r.id == *id).map(|r| pill(r.name.clone(), Some(r.color.unwrap_or(0x99AAB5))))
        }
    });
    div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .p(z(12.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .when(elevated, |d| d.shadow(shadow(true, false)))
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .child(div().flex().flex_col().min_w_0().child(text(title, 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG).truncate()).child(sub))
                .child(body_sm(crate::t!("modview.role_count", n = by.len()), FontWeight::MEDIUM, idx::TEXT_DEFAULT).flex_none()),
        )
        .when(!desc.is_empty(), |d| d.child(body_sm(desc, FontWeight::NORMAL, idx::TEXT_SUBTLE)))
        .child(div().flex().flex_wrap().items_center().gap(z(8.)).children(chips))
}

fn audit_panel(app: &PhoenixApp, m: &ModView, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let entries = match &m.data.audit {
        None => vec![div().p(z(12.)).bg(tk(idx::BACKGROUND_BASE_LOWER)).child(spinner()).into_any_element()],
        Some(Err(())) => vec![div()
            .p(z(12.))
            .flex()
            .flex_col()
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .child(body_sm(crate::t!("modview.awkward"), FontWeight::SEMIBOLD, idx::TEXT_DEFAULT))
            .child(body_sm(crate::t!("modview.broke"), FontWeight::NORMAL, idx::TEXT_MUTED))
            .into_any_element()],
        Some(Ok(list)) => list.iter().map(|e| audit_entry(app, m, e)).collect(),
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .mt(z(1.))
        .px(z(16.))
        .py(z(12.))
        .child(div().flex().flex_col().child(text(crate::t!("modview.action_log"), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT).mb(z(8.))).child(container(entries)));
    panel_frame(vec![panel_header(back_title(cx), None, true, cx).into_any_element(), scroller("modview-audit-scroll", body)])
}

fn audit_entry(app: &PhoenixApp, m: &ModView, e: &Value) -> AnyElement {
    use crate::ui::modals::server_settings::audit;
    let action = e.get("action_type").and_then(Value::as_u64).unwrap_or(0) as u32;
    let actor = e.get("user_id").and_then(sf);
    let users = m.data.audit_users.as_array().cloned().unwrap_or_default();
    let name_of = |id: u64| -> String {
        users
            .iter()
            .find(|u| u.get("id").and_then(sf) == Some(id))
            .map(crate::ui::modals::server_settings::kit::user_name)
            .or_else(|| app.users.get(&Id(id)).map(|u| u.display_name.to_string()))
            .unwrap_or_default()
    };
    let log = serde_json::json!({ "users": m.data.audit_users });
    let target = audit::target_name(&log, e, app);
    let count = e.get("options").and_then(|o| o.get("count")).and_then(|c| c.as_str().and_then(|s| s.parse().ok()).or_else(|| c.as_u64())).unwrap_or(0);
    let title = audit::sentence(action, &actor.map(name_of).unwrap_or_default(), &target, "", count);
    let id = e.get("id").and_then(sf).unwrap_or(0);
    let date = chrono::DateTime::from_timestamp(crate::ui::modals::server_settings::kit::snowflake_unix(id), 0).map(|d| crate::i18n::short_date(d.with_timezone(&chrono::Local))).unwrap_or_default();
    let reason = e.get("reason").and_then(Value::as_str).map(str::to_owned);
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(z(8.))
        .p(z(12.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .shadow(shadow(true, false))
        .mb(z(1.))
        .child(
            div()
                .w_full()
                .flex()
                .gap(z(4.))
                .child(div().flex_1().min_w_0().child(body_sm(title, FontWeight::SEMIBOLD, idx::TEXT_STRONG)))
                .child(body_sm(date, FontWeight::MEDIUM, idx::TEXT_STRONG).flex_none()),
        )
        .when_some(reason, |d, r| {
            d.child(
                div()
                    .flex()
                    .gap(z(4.))
                    .when_some(actor, |d, a| d.child(body_sm(format!("@{}", name_of(a)), FontWeight::MEDIUM, idx::TEXT_DEFAULT)))
                    .child(body_sm(r, FontWeight::NORMAL, idx::TEXT_MUTED)),
            )
        })
        .into_any_element()
}

fn tab_index(t: Tab) -> usize {
    match t {
        Tab::Messages => 0,
        Tab::Links => 1,
        Tab::Media => 2,
    }
}

pub fn history_pages(total: u32) -> u32 {
    total.div_ceil(HISTORY_PAGE).max(1)
}

pub fn go_history_page(app: &mut PhoenixApp, page: u32, cx: &mut Context<PhoenixApp>) {
    let Some((guild, user, tab, seq)) = with(|m| {
        m.data.history_page = page;
        m.data.history_page_results = if page == 0 { None } else { Some(None) };
        (m.guild, m.user, m.data.history_tab, m.seq)
    }) else {
        return;
    };
    crate::ui::chat::scrollbar::handle("modview-history-scroll").set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
    cx.notify();
    if page == 0 {
        return;
    }
    let mut req = search_request(guild, user, tab);
    req.offset = page * HISTORY_PAGE;
    let fut = app.backend.search_messages(req);
    cx.spawn(async move |this, cx| {
        let r = fut.await;
        let _ = this.update(cx, |_, cx| {
            update(seq, |d| {
                if d.history_page == page && d.history_tab == tab {
                    d.history_page_results = Some(Some(r.unwrap_or_default()));
                }
            });
            cx.notify();
        });
    })
    .detach();
}

fn history_panel(app: &PhoenixApp, m: &ModView, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let i = tab_index(m.data.history_tab);
    let total = m.data.counts[i].unwrap_or(0);
    let page = m.data.history_page;
    let list = if page == 0 { m.data.recent[i].as_ref() } else { m.data.history_page_results.as_ref().and_then(|r| r.as_ref()).map(|p| &p.messages) };
    let body = match list {
        None => div().p(z(16.)).child(spinner()),
        Some(list) => {
            let pages = history_pages(total);
            let pager = (pages > 1).then(|| {
                crate::ui::search::pager_with(page + 1, pages, std::rc::Rc::new(|a: &mut PhoenixApp, p: u32, cx: &mut Context<PhoenixApp>| go_history_page(a, p.saturating_sub(1), cx)), cx)
            });
            div().flex().flex_col().gap(z(16.)).mt(z(1.)).px(z(16.)).py(z(12.)).child(crate::ui::search::grouped_results(app, list, cx)).children(pager)
        }
    };
    panel_frame(vec![panel_header(back_title(cx), None, true, cx).into_any_element(), scroller("modview-history-scroll", body)])
}

pub fn ensure_inputs(window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    let need = with(|m| m.panel == Panel::Permissions && m.perm_query.is_none()).unwrap_or(false);
    if need {
        let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(crate::t!("modview.search_permissions")));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        with(|m| m.perm_query = Some(input));
    }
}

pub fn render(app: &PhoenixApp, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (guild, _, _) = current()?;
    if app.selected_guild != Some(guild) {
        return None;
    }
    ensure_inputs(window, cx);
    VIEW.with(|v| {
        let v = v.borrow();
        let m = v.as_ref()?;
        let member = m.data.member.as_ref().and_then(|o| o.as_ref());
        let roles = m.data.roles.as_deref().unwrap_or_default();
        let owner = app.guilds.iter().find(|g| g.id == m.guild).and_then(|g| g.owner) == Some(m.user);
        let member_roles = member.map(|mm| mm.roles.clone()).unwrap_or_default();
        let x = Ctx { guild: m.guild, user: m.user, owner, member, roles, member_roles };
        let theme = app.people.profiles.get(&m.user).and_then(|p| p.theme);
        let bg = theme.map(|t| c(crate::ui::profile_theme::composite(t, t.0))).unwrap_or(tk(idx::BACKGROUND_SURFACE_HIGH));
        let loading = m.data.member.is_none() || m.data.roles.is_none();
        let inner = if loading {
            div().flex_1().flex().items_center().justify_center().child(spinner())
        } else {
            let panel = |p: Panel, cx: &mut Context<PhoenixApp>| match p {
                Panel::Info => info_panel(app, m, &x, cx),
                Panel::Permissions => permissions_panel(m, &x, cx),
                Panel::AuditLog => audit_panel(app, m, cx),
                Panel::MessageHistory => history_panel(app, m, cx),
            };
            let elapsed = m.slide.map(|s| s.start.elapsed().as_secs_f32());
            let area = match (m.slide, elapsed) {
                (Some(s), Some(t)) if !spring_done(t) && s.from != m.panel => {
                    window.request_animation_frame();
                    let x = spring(t);
                    let reduced = crate::ui::settings::effects::reduced_motion();
                    let (enter, leave) = if reduced { (0., 0.) } else { slide_offsets(x, s.forwards) };
                    let layer = |child: AnyElement, left: f32, opacity: f32| {
                        div().absolute().top_0().bottom_0().left(z(left)).w(z(AREA_W)).flex().flex_col().opacity(opacity).child(child)
                    };
                    let old = panel(s.from, cx);
                    let new = panel(m.panel, cx);
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .overflow_hidden()
                        .child(layer(old, leave, if reduced { x.clamp(0., 1.) } else { 1. }))
                        .child(layer(new, enter, if reduced { (1. - x.abs()).clamp(0., 1.) } else { 1. }))
                }
                _ => div().flex_1().min_h_0().flex().flex_col().child(panel(m.panel, cx)),
            };
            div().flex_1().min_h_0().relative().flex().flex_col().gap(z(8.)).p(z(8.)).text_color(tk(idx::TEXT_DEFAULT)).child(header_card(app, &x, cx)).child(area)
        };
        Some(
            div()
                .id("mod-view")
                .w(z(WIDTH))
                .flex_none()
                .h_full()
                .child(div().ml(z(8.)).h_full().flex().flex_col().gap(z(8.)).child(div().flex_1().min_h_0().flex().flex_col().rounded_l(z(8.)).overflow_hidden().bg(bg).child(inner)))
                .into_any_element(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn role(id: u64, position: i32, permissions: u64) -> RoleInfo {
        RoleInfo { id: Id(id), name: SharedString::from(format!("r{id}")), color: None, position, permissions }
    }

    #[test]
    fn member_and_roles_parse() {
        let m = parse_member(&json!({
            "nick": "Nyx", "joined_at": "2025-01-02T03:04:05.000000+00:00", "roles": ["5", "6"], "flags": 2, "pending": false,
            "communication_disabled_until": "2030-01-01T00:00:00+00:00", "unusual_dm_activity_until": null
        }));
        assert_eq!(m.nick.as_deref(), Some("Nyx"));
        assert_eq!(m.roles, vec![Id(5), Id(6)]);
        assert_eq!(m.joined_at, Some(1_735_787_045));
        assert_eq!(m.timeout_until, Some(1_893_456_000));
        assert_eq!(m.unusual_dm_until, None);
        let r = parse_roles(&json!([{ "id": "5", "name": "Mod", "color": 0, "colors": { "primary_color": 16711680 }, "position": 3, "permissions": "1099511627782" }]));
        assert_eq!(r[0].color, Some(0xFF0000));
        assert_eq!(r[0].permissions, (1 << 40) | 6);
    }

    #[test]
    fn supplemental_join_source() {
        let v = json!([{ "user_id": "7", "join_source_type": 0, "source_invite_code": "abc", "inviter_id": "9" }]);
        assert_eq!(parse_supplemental(&v, Id(7)), Some(JoinInfo { source_type: Some(5), invite_code: Some("abc".into()), inviter: Some(Id(9)) }));
        assert_eq!(parse_supplemental(&v, Id(8)), None);
    }

    #[test]
    fn permissions_follow_roles_and_ownership() {
        let roles = vec![role(1, 0, 1 << 10), role(5, 3, (1 << 1) | (1 << 40)), role(6, 2, 1 << 3)];
        assert_eq!(granted_by(&roles, Id(1), &[Id(5)], Id(7), false, 1), Some(vec![Id(5)]));
        assert_eq!(granted_by(&roles, Id(1), &[Id(5)], Id(7), false, 2), None);
        assert_eq!(granted_by(&roles, Id(1), &[Id(5), Id(6)], Id(7), false, 2), Some(vec![Id(6)]));
        assert_eq!(granted_by(&roles, Id(1), &[], Id(7), true, 2), Some(vec![Id(7)]));
        assert_eq!(granted_by(&roles, Id(1), &[], Id(7), false, 10), Some(vec![Id(1)]));
        assert_eq!(mod_chips(&roles, Id(1), &[Id(5)], false), vec![1, 40]);
        assert!(mod_chips(&roles, Id(1), &[], false).is_empty());
    }

    #[test]
    fn hierarchy_and_rules_gate() {
        let roles = vec![role(5, 3, 0), role(6, 2, 0)];
        assert!(outranks(&roles, &[Id(5)], false, &[Id(6)], false));
        assert!(!outranks(&roles, &[Id(6)], false, &[Id(5)], false));
        assert!(!outranks(&roles, &[Id(5)], true, &[], true));
        assert!(outranks(&roles, &[], true, &[Id(5)], false));
        let m = MemberInfo { pending: Some(true), ..Default::default() };
        assert_eq!(rules_state(&[], &m), None);
        let gate = vec!["MEMBER_VERIFICATION_GATE_ENABLED".to_string()];
        assert_eq!(rules_state(&gate, &m), Some(false));
        assert_eq!(rules_state(&gate, &MemberInfo { flags: 2, ..m.clone() }), Some(true));
        assert_eq!(rules_state(&gate, &MemberInfo { pending: Some(false), ..m }), Some(true));
    }

    #[test]
    fn panel_slide_follows_the_default_spring() {
        assert!((spring(0.) - 1.).abs() < 1e-6);
        assert!(spring(0.1) > 0. && spring(0.1) < 1.);
        assert!(spring(0.2) < spring(0.1));
        assert!(!spring_done(0.2));
        assert!(spring_done(0.9));
        assert!(spring_done(SPRING_MAX_SECS));
        let d = AREA_W + SLIDE_GAP;
        assert_eq!(slide_offsets(1., true), (d, 0.));
        assert_eq!(slide_offsets(0., true), (0., -d));
        assert_eq!(slide_offsets(1., false), (-d, 0.));
        assert_eq!(slide_offsets(0., false), (0., d));
        assert_eq!(history_pages(0), 1);
        assert_eq!(history_pages(25), 1);
        assert_eq!(history_pages(26), 2);
    }

    #[test]
    fn search_tabs_match_the_client() {
        let r = search_request(Id(1), Id(2), Tab::Media);
        assert_eq!((r.guild, r.authors.clone()), (Some(Id(1)), vec![Id(2)]));
        assert_eq!(r.attachment_extensions.len(), 12);
        assert_eq!(search_request(Id(1), Id(2), Tab::Links).has, vec!["link".to_string()]);
        assert_eq!(countdown(90_061), "1d 1h 1m 1s");
        assert_eq!(countdown(59), "59s");
    }
}
