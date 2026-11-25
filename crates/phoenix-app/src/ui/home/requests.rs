use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{Palette, c, generated::idx, m, tok, z},
    ui::{
        controls::{BtnKind, button_sm},
        home::data::{NavRequest, NavReply},
    },
};

pub const EMPTY_DARK: &str = "https://discord.com/assets/4e11cf8e80a816c6.svg";
pub const EMPTY_LIGHT: &str = "https://discord.com/assets/a09cfd7678b20f8c.svg";

pub fn sorted(ids: &[Id], last: impl Fn(Id) -> Option<Id>) -> Vec<Id> {
    let mut v = ids.to_vec();
    v.sort_by_key(|c| std::cmp::Reverse(last(*c).unwrap_or(*c).0));
    v
}

pub fn ago(secs: i64, date: chrono::NaiveDate) -> SharedString {
    if secs.max(0) >= 31_104_000 { crate::i18n::long_date(date).into() } else { crate::thread_recent::ago(secs) }
}

fn snowflake_secs(id: Id) -> i64 {
    ((id.0 >> 22) / 1000) as i64 + 1_420_070_400
}

pub fn spam_tab_label(n: usize) -> SharedString {
    if n == 0 { crate::t!("requests.spam") } else { crate::t!("requests.spam_n", n = n) }
}

pub fn render(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let spam = app.nav.requests_spam_tab;
    let source = if spam { app.nav.spam_requests.clone() } else { app.nav.message_requests.clone() };
    let present: Vec<Id> = source.into_iter().filter(|c| app.channels.contains_key(c)).collect();
    let ids = sorted(&present, |c| app.backend.channel_last_message(c));
    for ch in &ids {
        if let Some(u) = app.channels.get(ch).and_then(|c| c.voice_users.first().copied()) {
            crate::ui::dm_profile_panel::ensure_profile(app, u, cx);
        }
    }
    let body = if ids.is_empty() {
        empty(spam).into_any_element()
    } else {
        let title: SharedString = if spam {
            crate::t!("requests.spam_section", n = ids.len())
        } else {
            crate::t!("requests.pending_section", n = ids.len())
        };
        let mut list = div().id("msg-requests").flex_1().min_h_0().overflow_y_scroll().px(z(24.)).pt(z(24.)).pb(z(24.)).flex().flex_col().child(
            div()
                .h(z(26.))
                .flex_none()
                .flex()
                .items_center()
                .ml(z(10.))
                .mb(z(10.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(title),
        );
        for (i, ch) in ids.iter().enumerate() {
            list = list.child(row(app, *ch, i == 0, spam, cx));
        }
        list.into_any_element()
    };
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_PRIMARY))
        .child(header(app, spam, cx))
        .child(body)
        .into_any_element()
}

fn header(app: &PhoenixApp, spam: bool, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let pill = |id: &'static str, label: SharedString, sel: bool, to_spam: bool, cx: &mut Context<PhoenixApp>| {
        div()
            .id(id)
            .mx(z(8.))
            .px(z(12.))
            .py(z(4.))
            .h(z(32.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(z(16.))
            .line_height(z(20.))
            .text_color(c(tok(if sel { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
            .when(sel, |d| d.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
            .when(!sel, |d| {
                d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).text_color(c(tok(idx::TEXT_STRONG))))
                    .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))).text_color(c(tok(idx::TEXT_STRONG))))
            })
            .on_click(cx.listener(move |a, _, _, cx| {
                a.nav.requests_spam_tab = to_spam;
                cx.notify();
            }))
            .child(label)
    };
    let spam_n = app.nav.spam_requests.iter().filter(|c| app.channels.contains_key(c)).count();
    div()
        .h(px(m::HEADER_H))
        .flex_none()
        .pl(z(20.))
        .pr(z(8.))
        .py(z(8.))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(c(Palette::HEADER_SHADOW))
        .child(
            div()
                .size(z(32.))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("server/envelope")).size(z(24.)).text_color(c(Palette::CHANNEL_ICON))),
        )
        .child(
            div()
                .mr(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::TEXT_HEADER))
                .child(crate::t!("friends.message_requests")),
        )
        .child(div().mx(z(4.)).size(z(4.)).rounded_full().bg(c(Palette::BG_MOD_SELECTED)))
        .child(pill("req-tab-requests", crate::t!("requests.tab"), !spam, false, cx))
        .child(pill("req-tab-spam", spam_tab_label(spam_n), spam, true, cx))
}

fn empty(spam: bool) -> impl IntoElement {
    let light = crate::theme::is_light();
    div().flex_1().flex().items_center().justify_center().child(
        div()
            .w(z(440.))
            .flex()
            .flex_col()
            .items_center()
            .child(div().w(z(415.)).h(z(200.)).mb(z(40.)).child(gpui::img(if light { EMPTY_LIGHT } else { EMPTY_DARK }).size_full()))
            .child(
                div()
                    .mt(z(8.))
                    .text_center()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_MUTED)))
                    .child(if spam { crate::t!("friends.spam_empty") } else { crate::t!("friends.no_message_requests") }),
            ),
    )
}

fn row(app: &PhoenixApp, ch: Id, first: bool, spam: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let uid = app.channels.get(&ch).and_then(|c| c.voice_users.first().copied());
    let u = uid.and_then(|id| app.users.get(&id));
    let nick = uid.and_then(|id| app.nav.relationship(id)).and_then(|r| r.nickname.clone());
    let name = nick.or_else(|| u.map(|u| u.display_name.clone())).unwrap_or_default();
    let username = u.map(|u| u.name.clone()).unwrap_or_default();
    let show_username = u.is_some_and(|u| u.display_name != u.name);
    let now = chrono::Utc::now();
    let last = app.backend.channel_last_message(ch).unwrap_or(ch);
    let secs = snowflake_secs(last);
    let date = chrono::DateTime::from_timestamp(secs, 0).map(|d| d.with_timezone(&chrono::Local).date_naive()).unwrap_or_default();
    let stamp = ago(now.timestamp() - secs, date);
    let mutual: Vec<Id> = uid.and_then(|id| app.people.profiles.get(&id)).map(|p| p.mutual_guild_ids.clone()).unwrap_or_default();
    let mutual_line = if mutual.is_empty() {
        div().pt(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("requests.no_mutual")).into_any_element()
    } else {
        let icons = mutual.iter().filter_map(|g| app.guilds.iter().find(|x| x.id == *g)).take(3).map(|g| crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 16.));
        let label = if mutual.len() == 1 { crate::t!("requests.mutual_one", n = 1) } else { crate::t!("requests.mutual_n", n = mutual.len()) };
        div()
            .pt(z(4.))
            .flex()
            .items_center()
            .gap(z(4.))
            .child(div().flex().items_center().gap(z(4.)).children(icons))
            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(label))
            .into_any_element()
    };
    let actions = if spam {
        div()
            .flex()
            .flex_none()
            .ml_auto()
            .gap(z(16.))
            .child(button_sm(("req-accept", ch.0), crate::t!("requests.accept"), BtnKind::Secondary).on_click(cx.listener(move |a, _, _, cx| {
                cx.stop_propagation();
                a.answer_message_request(ch, true, cx);
            })))
            .child(button_sm(("req-report", ch.0), crate::t!("requests.report_spam"), BtnKind::Critical).opacity(0.5).cursor_default())
    } else {
        div()
            .flex()
            .flex_none()
            .ml_auto()
            .gap(z(16.))
            .child(button_sm(("req-ignore", ch.0), crate::t!("requests.ignore"), BtnKind::Secondary).on_click(cx.listener(move |a, _, _, cx| {
                cx.stop_propagation();
                a.answer_message_request(ch, false, cx);
            })))
            .child(button_sm(("req-accept", ch.0), crate::t!("requests.accept"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| {
                cx.stop_propagation();
                a.answer_message_request(ch, true, cx);
            })))
    };
    let deco = crate::ui::avatar_deco::source(u, None);
    div()
        .id(("req", ch.0))
        .flex_none()
        .px(z(10.))
        .py(z(16.))
        .flex()
        .items_center()
        .cursor_pointer()
        .when(!first, |d| d.border_t_1().border_color(c(tok(idx::BORDER_SUBTLE))))
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))).rounded(z(8.)).border_color(gpui::transparent_black()))
        .on_click(cx.listener(move |a, _, _, cx| a.select_channel(ch, cx)))
        .child(
            div()
                .relative()
                .mr(z(12.))
                .size(z(40.))
                .flex_none()
                .child(crate::ui::avatar(u, 40.))
                .when_some(deco, |d, src| d.child(crate::ui::avatar_deco::layer(src, 40.))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(gpui::relative(0.25))
                .mr(z(16.))
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .mt(z(2.))
                                .overflow_hidden()
                                .gap(z(4.))
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c(tok(idx::TEXT_STRONG)))
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(name),
                                )
                                .when(show_username, |d| {
                                    d.child(div().font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_MUTED))).whitespace_nowrap().child(username))
                                }),
                        )
                        .child(div().whitespace_nowrap().text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).child(stamp)),
                )
                .child(mutual_line),
        )
        .child(actions)
        .into_any_element()
}

impl PhoenixApp {
    pub fn answer_message_request(&mut self, ch: Id, accept: bool, cx: &mut gpui::Context<Self>) {
        let single = self.nav.message_requests.len() == 1;
        self.nav_request(NavRequest::MessageRequest { channel: ch, accept }, cx, move |a, res, cx| {
            if let Ok(NavReply::Done) = res {
                a.nav.message_requests.retain(|c| *c != ch);
                a.nav.spam_requests.retain(|c| *c != ch);
                if accept && single {
                    a.select_channel(ch, cx);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_request_first() {
        let last = |c: Id| match c.0 {
            1 => Some(Id(500)),
            2 => Some(Id(900)),
            _ => None,
        };
        assert_eq!(sorted(&[Id(1), Id(2), Id(3)], last), vec![Id(2), Id(1), Id(3)]);
    }

    #[test]
    fn relative_stamp_then_long_date() {
        let d = chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        assert_eq!(ago(31_104_000, d).as_ref(), crate::i18n::long_date(d));
        assert_eq!(ago(120, d), crate::thread_recent::ago(120));
    }
}
