use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::{Modal, card, close_action};
use crate::{
    actions::with_modal,
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button, radio_option, switch},
        home::data::NavRequest,
    },
};

pub const FLAGS: [(&str, &str); 7] = [
    ("gn.in_app", "gn.in_app_desc"),
    ("gn.push", "gn.push_desc"),
    ("gn.suppress_everyone", ""),
    ("gn.suppress_roles", ""),
    ("gn.highlights", "gn.highlights_desc"),
    ("gn.mute_events", ""),
    ("gn.mobile_push", ""),
];

pub struct State {
    pub guild: Id,
    pub muted: bool,
    pub level: u8,
    pub flags: [bool; 7],
}

impl State {
    pub fn new(app: &PhoenixApp, guild: Id) -> Self {
        let p = app.backend.notify_prefs().map(|p| p.guild(Some(guild.0))).unwrap_or_default();
        use crate::notifications::Level::*;
        let level = match p.level {
            All => 0,
            Mentions | Inherit => 1,
            Nothing => 2,
        };
        let flags = [true, true, p.suppress_everyone, p.suppress_roles, false, false, true];
        Self { guild, muted: p.muted, level, flags }
    }
}

fn with_state(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::GuildNotify(s)) = m {
            f(s)
        }
    });
}

fn row(id: impl Into<gpui::ElementId>, title: SharedString, desc: Option<SharedString>, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .items_start()
        .gap(z(24.))
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(title))
                .children(desc.map(|d| div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(d))),
        )
        .group(crate::ui::controls::SWITCH_ROW)
        .child(div().flex_none().child(switch("notify-switch", on)))
}

fn label(s: SharedString) -> gpui::Div {
    div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(s)
}

fn divider() -> gpui::Div {
    div().my(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.guilds.iter().find(|g| g.id == s.guild).map(|g| g.name.to_string()).unwrap_or_default();
    let guild = s.guild;
    let mute = row("gn-mute", crate::t!("gn.mute", name = name), Some(crate::t!("gn.mute_desc")), s.muted).on_click(cx.listener(move |a, _, _, cx| {
        let mut now = false;
        with_state(|s| {
            s.muted = !s.muted;
            now = s.muted;
        });
        a.nav_request(NavRequest::Mute { guild: Some(guild), channel: None, muted: now, secs: None }, cx, |_, _, _| {});
        cx.notify();
    }));
    let radios = div().flex().flex_col().gap(z(16.)).child(label(crate::t!("gn.server_level")));
    let mut opts = div().flex().flex_col().gap(z(16.));
    for (n, k) in ["menu.notify.all", "gn.mentions", "gn.nothing"].into_iter().enumerate() {
        let lv = n as u8;
        opts = opts.child(radio_option(("gn-level", n), crate::t!(k), s.level == lv).on_click(cx.listener(move |a, _, _, cx| {
            with_state(|s| s.level = lv);
            a.nav_request(NavRequest::NotifyLevel { guild: Some(guild), channel: None, level: lv }, cx, |_, _, _| {});
            cx.notify();
        })));
    }
    let flag = |i: usize, cx: &mut Context<PhoenixApp>| {
        let (t, d) = FLAGS[i];
        row(("gn-flag", i), crate::t!(t), (!d.is_empty()).then(|| crate::t!(d)), s.flags[i]).on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| s.flags[i] = !s.flags[i]);
            cx.notify();
        }))
    };
    let community = div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(label(crate::t!("gn.community")))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("gn.community_desc"))),
        )
        .child(flag(0, cx))
        .child(div().h(z(8.)))
        .child(flag(1, cx));
    let suppress = div().flex().flex_col().gap(z(32.)).child(flag(2, cx)).child(flag(3, cx)).child(flag(4, cx)).child(flag(5, cx)).child(flag(6, cx));
    let overrides = div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(label(crate::t!("gn.override")))
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("gn.override_desc")))
        .child(override_table(app, s, cx));
    let body = div()
        .id("gn-body")
        .max_h(z(560.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(mute)
        .child(divider())
        .child(radios.child(opts))
        .child(divider())
        .child(community)
        .child(divider())
        .child(suppress)
        .child(divider())
        .child(overrides);
    let footer = button("gn-done", crate::t!("common.done"), BtnKind::Primary).flex_1().on_click({
        let close = close_action(cx);
        move |_, w, cx| close(w, cx)
    });
    card(480., crate::t!("menu.notify_settings"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

fn override_table(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::notifications::Level;
    let prefs = app.backend.notify_prefs().map(|p| p.guild(Some(s.guild.0))).unwrap_or_default();
    let mut rows: Vec<_> = prefs
        .channels
        .iter()
        .filter_map(|(id, p)| app.channels.get(&Id(*id)).filter(|ch| ch.guild == Some(s.guild)).map(|ch| (ch.clone(), *p)))
        .collect();
    if rows.is_empty() {
        return div().into_any_element();
    }
    rows.sort_by_key(|(ch, _)| (ch.position, ch.id));
    let head = |k: &str, w: f32| {
        div().w(z(w)).flex_none().h(z(44.)).child(
            div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!(k)),
        )
    };
    let mut table = div()
        .mt(z(30.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(div().ml(z(8.)).w(z(122.)).flex_none().h(z(44.)).child(
                    div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("gn.ov_name")),
                ))
                .child(head("gn.ov_all", 75.))
                .child(head("gn.ov_mentions", 75.))
                .child(head("gn.ov_nothing", 75.))
                .child(head("gn.ov_mute", 75.)),
        );
    let guild = s.guild;
    let default = s.level;
    for (ch, p) in rows {
        let cid = ch.id;
        let level = match p.level {
            Level::All => 0,
            Level::Mentions => 1,
            Level::Nothing => 2,
            Level::Inherit => default,
        };
        let byline = ch
            .parent
            .and_then(|pid| app.channels.get(&pid))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| crate::t!("gn.ov_no_category"));
        let check = |n: usize, on: bool| crate::ui::controls::checkbox_option(("gn-ov", (cid.0 as usize).wrapping_mul(8) + n), SharedString::default(), on).w(z(20.)).overflow_hidden();
        let mut opts = div().w(z(300.)).flex_none().flex().items_center().justify_around();
        for n in 0..3u8 {
            opts = opts.child(check(n as usize, level == n).on_click(cx.listener(move |a, _, _, cx| {
                a.nav_request(NavRequest::NotifyLevel { guild: Some(guild), channel: Some(cid), level: n }, cx, |_, _, _| {});
                cx.notify();
            })));
        }
        let muted = p.muted;
        opts = opts.child(check(3, muted).on_click(cx.listener(move |a, _, _, cx| {
            a.nav_request(NavRequest::Mute { guild: Some(guild), channel: Some(cid), muted: !muted, secs: None }, cx, |_, _, _| {});
            cx.notify();
        })));
        table = table.child(
            div()
                .mb(z(8.))
                .flex()
                .pl(z(10.))
                .py(z(10.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(
                    div()
                        .w(z(120.))
                        .flex_none()
                        .flex()
                        .child(
                            div().w(z(24.)).h(z(20.)).mt(z(8.)).flex_none().opacity(0.6).child(
                                gpui::svg()
                                    .path(crate::icons::path(super::create_channel::icon_for(ch.kind)))
                                    .size(z(20.))
                                    .text_color(c(tok(idx::TEXT_DEFAULT))),
                            ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .px(z(10.))
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(z(16.))
                                        .line_height(z(20.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c(tok(idx::TEXT_STRONG)))
                                        .child(ch.name.clone()),
                                )
                                .child(div().truncate().opacity(0.6).text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(byline)),
                        ),
                )
                .child(opts),
        );
    }
    table.into_any_element()
}
