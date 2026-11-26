use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::{models::Snowflake, rest::guild_manage::AdminCall};
use serde_json::Value;

use super::{
    State,
    kit::{self, Cx, text},
    with_state,
};
use crate::{
    actions::admin::sf,
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::BtnKind,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sort {
    #[default]
    JoinedDesc,
    JoinedAsc,
    NameAsc,
    NameDesc,
}

impl Sort {
    pub fn next(self, col: u8) -> Self {
        match (col, self) {
            (0, Sort::NameAsc) => Sort::NameDesc,
            (0, _) => Sort::NameAsc,
            (_, Sort::JoinedDesc) => Sort::JoinedAsc,
            _ => Sort::JoinedDesc,
        }
    }
}

pub struct Row {
    pub id: u64,
    pub name: SharedString,
    pub tag: SharedString,
    pub joined: Option<i64>,
    pub created: i64,
    pub roles: Vec<u64>,
}

pub fn rows(app: &PhoenixApp, guild: Id, q: &str, sort: Sort) -> Vec<Row> {
    let q = q.trim().to_lowercase();
    let mut out: Vec<Row> = app
        .members
        .get(&guild)
        .into_iter()
        .flatten()
        .map(|m| {
            let u = app.users.get(&m.user);
            Row {
                id: m.user.0,
                name: m.nick.clone().or_else(|| u.map(|u| u.display_name.clone())).unwrap_or_else(|| m.user.0.to_string().into()),
                tag: u.map(|u| u.name.clone()).unwrap_or_default(),
                joined: m.joined_at,
                created: kit::snowflake_unix(m.user.0),
                roles: m.roles.iter().map(|r| r.0).collect(),
            }
        })
        .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q) || r.tag.to_lowercase().contains(&q) || r.id.to_string() == q)
        .collect();
    match sort {
        Sort::JoinedDesc => out.sort_by_key(|r| std::cmp::Reverse(r.joined.unwrap_or(0))),
        Sort::JoinedAsc => out.sort_by_key(|r| r.joined.unwrap_or(0)),
        Sort::NameAsc => out.sort_by_key(|r| r.name.to_lowercase()),
        Sort::NameDesc => out.sort_by_key(|r| std::cmp::Reverse(r.name.to_lowercase())),
    }
    out
}

fn ago(secs: i64) -> String {
    let now = crate::actions::now_unix() as i64;
    let d = (now - secs).max(0);
    let day = 86_400;
    if d < day {
        crate::t!("gs.members.today").to_string()
    } else if d < 30 * day {
        crate::t!("gs.members.days", n = d / day).to_string()
    } else if d < 365 * day {
        crate::t!("gs.members.months", n = d / (30 * day)).to_string()
    } else {
        crate::t!("gs.members.years", n = d / (365 * day)).to_string()
    }
}

fn role_chip(s: &State, id: u64) -> Option<AnyElement> {
    let r = s.list("roles").iter().find(|r| kit::n(r, "id") == id)?;
    Some(
        div()
            .h(z(24.))
            .px(z(6.))
            .rounded(z(4.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .flex()
            .items_center()
            .gap(z(4.))
            .child(super::roles::circle(&super::roles::colors_of(r), 12.))
            .child(text(kit::s(r, "name"), 12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT).truncate().max_w(z(120.)))
            .into_any_element(),
    )
}

fn col_head(id: (&'static str, usize), label: SharedString, sorted: Option<bool>, w: Option<f32>, cx: &mut Cx) -> AnyElement {
    let col = id.1 as u8;
    div()
        .id(id)
        .when_some(w, |d, w| d.w(z(w)).flex_none())
        .when(w.is_none(), |d| d.flex_1().min_w_0())
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .child(text(label, 12., 16., FontWeight::SEMIBOLD, idx::TEXT_SUBTLE))
        .when_some(sorted, |d, asc| d.child(gpui::svg().path(crate::icons::path(if asc { "chevron-up" } else { "chevron" })).size(z(16.)).text_color(c(tok(idx::ICON_SUBTLE)))))
        .on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| s.member_sort = s.member_sort.next(col));
            cx.notify();
        }))
        .into_any_element()
}

pub fn members(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let q = kit::input_value(s.inputs.get("member_search"), cx);
    let list = rows(app, s.guild, &q, s.member_sort);
    let total = app.members.get(&s.guild).map_or(0, Vec::len);
    let sort = s.member_sort;
    let head = div()
        .h(z(36.))
        .px(z(16.))
        .flex()
        .items_center()
        .gap(z(16.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(col_head(("ss-mcol", 0), crate::t!("gs.members.col_name"), matches!(sort, Sort::NameAsc | Sort::NameDesc).then_some(sort == Sort::NameAsc), None, cx))
        .child(col_head(("ss-mcol", 1), crate::t!("gs.members.col_joined"), matches!(sort, Sort::JoinedAsc | Sort::JoinedDesc).then_some(sort == Sort::JoinedAsc), Some(110.), cx))
        .child(div().w(z(110.)).flex_none().child(text(crate::t!("gs.members.col_age"), 12., 16., FontWeight::SEMIBOLD, idx::TEXT_SUBTLE)))
        .child(div().w(z(160.)).flex_none().child(text(crate::t!("gs.members.col_roles"), 12., 16., FontWeight::SEMIBOLD, idx::TEXT_SUBTLE)))
        .child(div().w(z(32.)).flex_none());
    let mut body = div().flex().flex_col();
    for (i, r) in list.iter().take(200).enumerate() {
        let u = app.users.get(&Id(r.id));
        let menu_key = format!("member-{}", r.id);
        let open = s.menu.as_deref() == Some(menu_key.as_str());
        let uid = r.id;
        let mut roles = div().w(z(160.)).flex_none().flex().gap(z(4.)).overflow_hidden();
        for rid in r.roles.iter().take(2) {
            if let Some(ch) = role_chip(s, *rid) {
                roles = roles.child(ch);
            }
        }
        if r.roles.len() > 2 {
            roles = roles.child(text(format!("+{}", r.roles.len() - 2), 12., 16., FontWeight::MEDIUM, idx::TEXT_SUBTLE));
        }
        let mk = menu_key.clone();
        body = body.child(
            div()
                .id(("ss-member", i))
                .relative()
                .h(z(56.))
                .px(z(16.))
                .flex()
                .items_center()
                .gap(z(16.))
                .border_b_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .child(crate::ui::avatar(u, 32.))
                        .child(div().flex().flex_col().min_w_0().child(text(r.name.clone(), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG).truncate()).child(text(r.tag.clone(), 12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE).truncate())),
                )
                .child(text(r.joined.map(ago).unwrap_or_default(), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).w(z(110.)).flex_none())
                .child(text(ago(r.created), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).w(z(110.)).flex_none())
                .child(roles)
                .child(
                    kit::icon_btn(("ss-member-more", i), "more-vertical", 32., 16., false).on_click(cx.listener(move |_, _, _, cx| {
                        let k = mk.clone();
                        with_state(|s| s.menu = if s.menu.as_deref() == Some(k.as_str()) { None } else { Some(k) });
                        cx.notify();
                    })),
                )
                .when(open, |d| d.child(gpui::deferred(member_menu(s.guild, uid, cx)).with_priority(30))),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(kit::page_head(crate::t!("menu.gs.members"), Some(crate::t!("gs.members.desc", n = total))))
        .child(div().flex().items_center().gap(z(16.)).child(kit::search_box(s.inputs.get("member_search")).flex_1()).child(kit::critical_secondary("ss-members-prune", crate::t!("gs.members.prune")).h(z(32.)).px(z(12.)).on_click(cx.listener(|_, _, _, cx| {
            with_state(|s| s.prune = Some(Prune::default()));
            cx.notify();
        }))))
        .child(div().rounded(z(8.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).flex().flex_col().child(head).child(body))
        .into_any_element()
}

fn menu_item(id: (&'static str, usize), label: SharedString, danger: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(32.))
        .px(z(8.))
        .rounded(z(4.))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(if danger { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT })))
        .hover(move |s| s.bg(c(tok(if danger { idx::BACKGROUND_FEEDBACK_MOD_CRITICAL } else { idx::BACKGROUND_MOD_NORMAL }))))
        .child(label)
}

pub fn member_calls(guild: Id, user: u64) -> [AdminCall; 2] {
    [AdminCall::Kick { guild: sf(guild), user: Snowflake(user) }, AdminCall::Ban { guild: sf(guild), user: Snowflake(user), delete_message_seconds: 0 }]
}

fn member_menu(guild: Id, user: u64, cx: &mut Cx) -> AnyElement {
    let [kick, ban] = member_calls(guild, user);
    div()
        .absolute()
        .top(z(48.))
        .right(z(16.))
        .w(z(188.))
        .p(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .occlude()
        .flex()
        .flex_col()
        .child(menu_item(("ss-mm", 0), crate::t!("common.copy_user_id"), false).on_click(cx.listener(move |a, _, _, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(user.to_string()));
            with_state(|s| s.menu = None);
            a.parity_toast(crate::t!("gs.eng.copied"), cx);
        })))
        .child(menu_item(("ss-mm", 1), crate::t!("gs.members.kick"), true).on_click(cx.listener(move |a, _, _, cx| {
            with_state(|s| s.menu = None);
            a.admin_toast(kick.clone(), Some(crate::t!("gs.members.kicked")), cx);
        })))
        .child(menu_item(("ss-mm", 2), crate::t!("gs.members.ban"), true).on_click(cx.listener(move |a, _, _, cx| {
            with_state(|s| s.menu = None);
            a.admin_toast(ban.clone(), Some(crate::t!("gs.members.banned")), cx);
        })))
        .into_any_element()
}

pub fn invite_rows(list: &[Value]) -> Vec<&Value> {
    let mut v: Vec<&Value> = list.iter().collect();
    v.sort_by_key(|i| std::cmp::Reverse(kit::s(i, "created_at")));
    v
}

fn expires(inv: &Value) -> SharedString {
    if inv.get("expires_at").and_then(Value::as_str).is_none() && kit::n(inv, "max_age") == 0 {
        return "∞".into();
    }
    let at = inv.get("expires_at").and_then(Value::as_str).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|t| t.timestamp()).unwrap_or(0);
    let left = at - crate::actions::now_unix() as i64;
    if left <= 0 {
        return crate::t!("gs.inv.expired");
    }
    let (d, h, m) = (left / 86_400, (left % 86_400) / 3600, (left % 3600) / 60);
    format!("{d:02}:{h:02}:{m:02}").into()
}

pub fn invites(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let list = invite_rows(s.list("invites"));
    let head = div()
        .flex()
        .pb(z(12.))
        .child(text(crate::t!("gs.inv.host"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).w(z(193.4)))
        .child(text(crate::t!("gs.inv.code"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).w(z(155.)))
        .child(text(crate::t!("gs.inv.uses"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).w(z(98.2)))
        .child(text(crate::t!("gs.inv.expires"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).w(z(116.7)))
        .child(text(crate::t!("gs.inv.roles"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).flex_1());
    let mut body = div().flex().flex_col();
    for (i, inv) in list.iter().enumerate() {
        let u = inv.get("inviter").cloned().unwrap_or(Value::Null);
        let ch = inv.get("channel").map(|c| kit::s(c, "name")).unwrap_or_default();
        let code = kit::s(inv, "code");
        let group: SharedString = format!("ss-inv-{code}").into();
        let revoke = AdminCall::DeleteInvite { code: code.clone() };
        let rid = code.clone();
        body = body.child(
            div()
                .id(("ss-inv", i))
                .group(group.clone())
                .relative()
                .h(z(62.))
                .flex()
                .items_center()
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(
                    div()
                        .w(z(193.4))
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .child(kit::avatar(kit::user_avatar(&u, 64), 24.))
                        .child(div().flex().flex_col().min_w_0().child(text(kit::user_name(&u), 16., 20., FontWeight::NORMAL, idx::TEXT_STRONG).truncate()).child(text(format!("#{ch}"), 12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE).truncate())),
                )
                .child(text(code.clone(), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).w(z(155.)).pl(z(16.)))
                .child(text(kit::n(inv, "uses").to_string(), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).w(z(98.2)).pl(z(24.)))
                .child(text(expires(inv), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).w(z(116.7)).pl(z(0.)))
                .child(div().flex_1())
                .child(
                    div()
                        .id(("ss-inv-revoke", i))
                        .absolute()
                        .right(z(-12.))
                        .size(z(24.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_FEEDBACK_CRITICAL)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                        .cursor_pointer()
                        .child(gpui::svg().path(crate::icons::path("close-small")).size(z(16.)).text_color(gpui::white()))
                        .on_click(cx.listener(move |a, _, _, cx| {
                            let code = rid.clone();
                            a.admin_toast(revoke.clone(), Some(crate::t!("gs.inv.revoked")), cx);
                            with_state(|s| {
                                if let Some(Value::Array(v)) = s.data.get_mut("invites") {
                                    v.retain(|i| kit::s(i, "code") != code);
                                }
                            });
                            cx.notify();
                        })),
                ),
        );
    }
    let guild = s.guild;
    let paused = app.backend.guild_features(guild).iter().any(|f| f == "INVITES_DISABLED");
    let pause = if paused {
        crate::ui::controls::button("ss-inv-pause", crate::t!("gs.inv.resume"), BtnKind::Primary)
    } else {
        kit::critical_secondary("ss-inv-pause", crate::t!("gs.inv.pause")).h(z(40.)).px(z(16.)).text_size(z(14.))
    };
    div()
        .flex()
        .flex_col()
        .child(kit::h2(crate::t!("menu.gs.invites")))
        .child(text(crate::t!("gs.inv.desc"), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).mt(z(8.)))
        .child(
            div()
                .mt(z(40.))
                .mb(z(24.))
                .flex()
                .items_center()
                .gap(z(16.))
                .child(text(crate::t!("gs.inv.active"), 14., 18.7, FontWeight::MEDIUM, idx::TEXT_STRONG).flex_1())
                .child(pause.on_click(cx.listener(move |a, _, _, cx| {
                    let sec = a.security_state(guild);
                    with_state(|s| s.security = Some(sec));
                    cx.notify();
                })))
                .child(crate::ui::controls::button("ss-inv-create", crate::t!("gs.inv.create"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| a.open_invite_people(guild, cx)))),
        )
        .child(head)
        .child(body)
        .into_any_element()
}

pub const BANS_PER_PAGE: usize = 100;

pub fn page_count(n: usize) -> usize {
    n.div_ceil(BANS_PER_PAGE).max(1)
}

fn page_button(id: (&'static str, usize), label: SharedString, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .m(z(4.))
        .min_w(z(28.))
        .h(z(28.))
        .px(z(6.))
        .rounded(z(14.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_size(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(active, |d| d.bg(c(tok(idx::BRAND_500))).text_color(c(tok(idx::WHITE))))
        .when(!active, |d| d.text_color(c(tok(idx::TEXT_STRONG))).hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
        .child(label)
}

pub fn bans(_app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let q = kit::input_value(s.inputs.get("ban_search"), cx).to_lowercase();
    let all = s.list("bans");
    let shown: Vec<&Value> = all
        .iter()
        .filter(|b| {
            let u = b.get("user").cloned().unwrap_or(Value::Null);
            q.is_empty() || kit::user_name(&u).to_lowercase().contains(&q) || kit::s(&u, "username").to_lowercase().contains(&q) || kit::s(&u, "id") == q
        })
        .collect();
    let pages = page_count(shown.len());
    let page = s.ban_page.min(pages - 1);
    let mut list = div().flex().flex_col();
    for (i, b) in shown.iter().enumerate().skip(page * BANS_PER_PAGE).take(BANS_PER_PAGE) {
        let u = b.get("user").cloned().unwrap_or(Value::Null);
        list = list.child(
            div()
                .id(("ss-ban", i))
                .h(z(56.))
                .mb(z(4.))
                .p(z(8.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .flex()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .child(kit::avatar(kit::user_avatar(&u, 80), 40.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pl(z(8.))
                        .flex()
                        .flex_col()
                        .child(text(kit::user_name(&u), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).truncate())
                        .child(text(kit::s(&u, "username"), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT).truncate()),
                ),
        );
    }
    let body: AnyElement = if shown.is_empty() && !s.pending.contains("bans") {
        text(crate::t!(if q.is_empty() { "gs.bans.empty" } else { "gs.bans.no_match" }), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE).into_any_element()
    } else {
        div()
            .h(z(760.))
            .p(z(8.))
            .rounded(z(12.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::CARD_BACKGROUND_DEFAULT)))
            .child(div().id("ss-bans-scroll").size_full().overflow_y_scroll().pr(z(8.)).child(list))
            .into_any_element()
    };
    let mut pager = div().flex().items_center().justify_end().py(z(4.));
    if pages > 1 {
        pager = pager.child(page_button(("ss-ban-prev", 0), crate::t!("gs.bans.prev"), false).px(z(12.)).on_click(cx.listener(move |_, _, _, cx| {
            super::with_state(|s| s.ban_page = s.ban_page.saturating_sub(1));
            cx.notify();
        })));
        for n in 0..pages {
            pager = pager.child(page_button(("ss-ban-page", n), SharedString::from((n + 1).to_string()), n == page).on_click(cx.listener(move |_, _, _, cx| {
                super::with_state(|s| s.ban_page = n);
                cx.notify();
            })));
        }
        pager = pager.child(page_button(("ss-ban-next", 0), crate::t!("gs.bans.next"), false).px(z(12.)).on_click(cx.listener(move |_, _, _, cx| {
            super::with_state(|s| s.ban_page = (s.ban_page + 1).min(pages - 1));
            cx.notify();
        })));
    }
    let desc = div()
        .mb(z(20.))
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(gpui::StyledText::new(format!("{}{}.", crate::t!("gs.bans.desc_lead"), crate::t!("gs.bans.desc_link"))).with_highlights([(
            crate::t!("gs.bans.desc_lead").len()..crate::t!("gs.bans.desc_lead").len() + crate::t!("gs.bans.desc_link").len(),
            gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() },
        )]));
    div()
        .pt(z(56.))
        .px(z(24.))
        .pb(z(24.))
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(div().flex().flex_col().child(text(crate::t!("gs.bans.title"), 20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG)).child(desc))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(div().flex_1().min_w_0().child(kit::search_box(s.inputs.get("ban_search"))))
                .child(crate::ui::controls::button("ss-ban-search", crate::t!("gs.bans.search_btn"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| {
                    super::with_state(|s| s.ban_page = 0);
                    cx.notify();
                }))),
        )
        .child(body)
        .child(pager)
        .into_any_element()
}

pub const PRUNE_DAYS: [u32; 2] = [7, 30];

#[derive(Clone, Debug, PartialEq)]
pub struct Prune {
    pub days: u32,
    pub roles: Vec<u64>,
    pub days_open: bool,
    pub roles_open: bool,
}

impl Default for Prune {
    fn default() -> Self {
        Self { days: PRUNE_DAYS[0], roles: Vec::new(), days_open: false, roles_open: false }
    }
}

impl Prune {
    pub fn count_call(&self, guild: Id) -> phoenix_api::rest::guild_server::ServerCall {
        let mut roles: Vec<u64> = self.roles.clone();
        roles.sort_unstable();
        phoenix_api::rest::guild_server::ServerCall::PruneCount { guild: sf(guild), days: self.days, roles: roles.into_iter().map(Snowflake).collect() }
    }

    pub fn write(&self, guild: Id) -> crate::actions::guard::GuardedWrite {
        crate::actions::guard::GuardedWrite::prune(guild.0, self.days, &self.roles)
    }

    pub fn toggle(&mut self, role: u64) {
        if let Some(i) = self.roles.iter().position(|r| *r == role) {
            self.roles.remove(i);
        } else {
            self.roles.push(role);
        }
    }
}

pub fn prunable_roles(roles: &[Value], guild: Id, mine: &[u64], owner: bool) -> Vec<(u64, String)> {
    let pos = |r: &Value| r.get("position").and_then(Value::as_i64).unwrap_or(0);
    let top = roles.iter().filter(|r| mine.contains(&kit::n(r, "id"))).map(pos).max().unwrap_or(0);
    let mut v: Vec<&Value> = roles
        .iter()
        .filter(|r| kit::n(r, "id") != guild.0 && !r.get("managed").and_then(Value::as_bool).unwrap_or(false) && (owner || pos(r) < top))
        .collect();
    v.sort_by_key(|r| (-pos(r), kit::n(r, "id")));
    v.into_iter().map(|r| (kit::n(r, "id"), kit::s(r, "name"))).collect()
}

fn prune_text(p: &Prune, count: Option<u64>) -> SharedString {
    let days = p.days.to_string();
    let members = count.map_or_else(|| "-1".to_owned(), |n| n.to_string());
    let key = match (p.roles.is_empty(), count.is_some()) {
        (true, true) => "gs.prune.count",
        (true, false) => "gs.prune.count_loading",
        (false, true) => "gs.prune.count_roles",
        (false, false) => "gs.prune.count_roles_loading",
    };
    crate::i18n::t(key, &[("members", &members), ("days", &days)])
}

fn prune_option(id: (&'static str, usize), label: SharedString, on: bool, multi: bool) -> gpui::Stateful<gpui::Div> {
    let name = div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(label);
    if multi {
        return crate::ui::controls::checkbox_option_el(id, name.into_any_element(), on).h(z(40.)).px(z(8.)).items_center().rounded(z(4.)).hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))));
    }
    div()
        .id(id)
        .h(z(40.))
        .pl(z(8.))
        .pr(z(4.))
        .rounded(z(4.))
        .flex()
        .items_center()
        .gap(z(8.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .child(name)
        .when(on, |d| d.child(gpui::svg().path(crate::icons::path("circle-check")).flex_none().size(z(20.)).text_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW)))))
}

fn prune_list(id: &'static str, rows: Vec<AnyElement>) -> AnyElement {
    gpui::deferred(
        div()
            .id(id)
            .absolute()
            .top(z(44.))
            .left_0()
            .w_full()
            .max_h(z(400.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .occlude()
            .children(rows),
    )
    .with_priority(40)
    .into_any_element()
}

fn with_prune(f: impl FnOnce(&mut Prune)) {
    with_state(|s| {
        if let Some(p) = s.prune.as_mut() {
            f(p)
        }
    });
}

pub fn prune_modal(app: &PhoenixApp, s: &State, p: &Prune, cx: &mut Cx) -> AnyElement {
    let guild = s.guild;
    let g = app.guilds.iter().find(|g| g.id == guild);
    let name = g.map(|g| g.name.to_string()).unwrap_or_default();
    let owner = g.and_then(|g| g.owner) == Some(app.me.id);
    let mine: Vec<u64> = app.members.get(&guild).into_iter().flatten().find(|m| m.user == app.me.id).map(|m| m.roles.iter().map(|r| r.0).collect()).unwrap_or_default();
    let roles = prunable_roles(s.list("roles"), guild, &mine, owner);
    let count = crate::ui::server_pages::fetch(app, guild, p.count_call(guild), cx).and_then(|v| v.get("pruned").and_then(Value::as_u64));
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        with_state(|s| s.prune = None);
        e.update(cx, |_, cx| cx.notify());
    });
    let days_label = |d: u32| crate::i18n::t("gs.prune.days", &[("days", &d.to_string())]);
    let mut days_box = div().relative().w_full().child(
        kit::select_box("ss-prune-days", days_label(p.days).into_any_element(), p.days_open).on_click(cx.listener(|_, _, _, cx| {
            with_prune(|p| {
                p.days_open = !p.days_open;
                p.roles_open = false;
            });
            cx.notify();
        })),
    );
    if p.days_open {
        let rows = PRUNE_DAYS
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let d = *d;
                prune_option(("ss-prune-day", i), days_label(d), d == p.days, false)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        with_prune(|p| {
                            p.days = d;
                            p.days_open = false;
                        });
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        days_box = days_box.child(prune_list("ss-prune-days-list", rows));
    }
    let picked: Vec<String> = roles.iter().filter(|(id, _)| p.roles.contains(id)).map(|(_, n)| n.clone()).collect();
    let mut roles_box = div().relative().w_full().child(
        kit::select_box("ss-prune-roles", SharedString::from(picked.join(", ")).into_any_element(), p.roles_open).on_click(cx.listener(|_, _, _, cx| {
            with_prune(|p| {
                p.roles_open = !p.roles_open;
                p.days_open = false;
            });
            cx.notify();
        })),
    );
    if p.roles_open {
        let rows = roles
            .iter()
            .enumerate()
            .map(|(i, (id, n))| {
                let id = *id;
                prune_option(("ss-prune-role", i), n.clone().into(), p.roles.contains(&id), true)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        with_prune(|p| p.toggle(id));
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        roles_box = roles_box.child(prune_list("ss-prune-roles-list", rows));
    }
    let body = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(div().flex().flex_col().gap(z(8.)).child(text(crate::t!("gs.prune.last_seen"), 14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(days_box))
        .child(div().flex().flex_col().gap(z(8.)).child(text(crate::t!("gs.prune.roles"), 14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(roles_box))
        .child(text(prune_text(p, count), 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT).whitespace_normal());
    let c2 = close.clone();
    let c3 = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .justify_end()
        .gap(z(8.))
        .child(crate::ui::controls::button("ss-prune-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(move |_, w, cx| c2(w, cx)))
        .child(crate::ui::controls::button("ss-prune-ok", crate::t!("gs.prune.submit"), BtnKind::Primary).on_click(cx.listener(move |a, _, w, cx| {
            let draft = with_state(|s| s.prune.clone()).flatten();
            c3(w, cx);
            if let Some(d) = draft {
                a.blocked_write(d.write(guild), cx);
            }
        })));
    crate::ui::modals::card_with(440., format!("{}\u{2014}{name}", crate::t!("gs.prune.title")).into(), None, body.into_any_element(), Some(footer.into_any_element()), close)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prune_follows_the_client() {
        let roles = vec![
            json!({ "id": "1", "name": "@everyone", "position": 0 }),
            json!({ "id": "2", "name": "Bot", "position": 3, "managed": true }),
            json!({ "id": "3", "name": "Mod", "position": 2 }),
            json!({ "id": "4", "name": "Membre", "position": 1 }),
            json!({ "id": "5", "name": "Admin", "position": 4 }),
        ];
        assert_eq!(prunable_roles(&roles, Id(1), &[3], false), vec![(4, "Membre".to_owned())]);
        assert_eq!(prunable_roles(&roles, Id(1), &[], true).iter().map(|r| r.0).collect::<Vec<_>>(), [5, 3, 4]);
        let mut p = Prune::default();
        assert_eq!(p.days, 7);
        p.toggle(4);
        p.toggle(3);
        p.toggle(4);
        assert_eq!(p.roles, [3]);
        let r = phoenix_api::rest::guild_server::build(&p.count_call(Id(9)));
        assert_eq!((r.method.as_str(), r.path.as_str()), ("GET", "/guilds/9/prune?days=7&include_roles=3"));
        let w = p.write(Id(9));
        assert_eq!((w.method.as_str(), w.path.as_str()), ("POST", "/guilds/9/prune"));
        assert!(w.refusal().is_write_blocked());
    }

    #[test]
    fn bans_paginate_by_100() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(100), 1);
        assert_eq!(page_count(101), 2);
    }

    #[test]
    fn sort_cycles_per_column() {
        assert_eq!(Sort::default().next(0), Sort::NameAsc);
        assert_eq!(Sort::NameAsc.next(0), Sort::NameDesc);
        assert_eq!(Sort::JoinedDesc.next(1), Sort::JoinedAsc);
        assert_eq!(Sort::JoinedAsc.next(1), Sort::JoinedDesc);
    }

    #[test]
    fn invites_newest_first_and_writes_are_coded() {
        let l = vec![json!({ "code": "a", "created_at": "2025-01-01T00:00:00Z" }), json!({ "code": "b", "created_at": "2026-01-01T00:00:00Z" })];
        assert_eq!(invite_rows(&l).iter().map(|i| kit::s(i, "code")).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(expires(&json!({ "max_age": 0, "expires_at": null })).as_ref(), "∞");
        let [kick, ban] = member_calls(Id(1), 2);
        assert_eq!(phoenix_api::rest::guild_manage::build(&kick).path, "/guilds/1/members/2");
        assert_eq!(phoenix_api::rest::guild_manage::build(&ban).method.as_str(), "PUT");
    }
}
