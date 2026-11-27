use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, deferred, div, prelude::*};
use gpui_component::input::InputState;
use phoenix_api::rest::settings::RaidAlerts;

use super::{Modal, card, close_action};
use crate::{
    actions::with_modal,
    app::PhoenixApp,
    model::{ChannelKind, Id},
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
    pub window: i64,
    pub level: u8,
    pub flags: [bool; 7],
    pub search: Option<Entity<InputState>>,
    pub picker: bool,
    pub mute_open: bool,
    pub added: Vec<Id>,
    pub removed: Vec<Id>,
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
        let raid = app.prefs.synced.raid_alerts.get(&guild.0).copied().unwrap_or_default();
        let flags = [!raid.nag_off, !raid.push_off, p.suppress_everyone, p.suppress_roles, p.highlights_off, p.mute_events, p.mobile_push];
        Self { guild, muted: p.muted, window: p.window, level, flags, search: None, picker: false, mute_open: false, added: Vec::new(), removed: Vec::new() }
    }
}

pub fn prepare(s: &mut State, mk: &mut dyn FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    if s.search.is_none() {
        s.search = Some(mk(crate::t!("gn.override"), "", false));
    }
}

pub fn raid_alerts(flags: &[bool; 7]) -> RaidAlerts {
    RaidAlerts { nag_off: !flags[0], push_off: !flags[1] }
}

pub fn sees_raid_alerts(perms: u64) -> bool {
    use crate::ui::inbox::{BAN_MEMBERS, KICK_MEMBERS, MODERATE_MEMBERS, has_perm};
    [BAN_MEMBERS, KICK_MEMBERS, MODERATE_MEMBERS, crate::actions::MANAGE_GUILD].into_iter().any(|b| has_perm(perms, b))
}

pub fn override_targets(app: &PhoenixApp, guild: Id) -> Vec<Id> {
    let all: Vec<&crate::model::Channel> = app.guild_channels.get(&guild).into_iter().flatten().filter_map(|id| app.channels.get(id)).collect();
    let selectable = |k: ChannelKind| matches!(k, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum);
    let key = |c: &&crate::model::Channel| (c.position, c.id);
    let mut top: Vec<_> = all.iter().filter(|c| c.parent.is_none() && c.kind != ChannelKind::Category && selectable(c.kind)).collect();
    top.sort_by_key(|c| key(c));
    let mut cats: Vec<_> = all.iter().filter(|c| c.kind == ChannelKind::Category).collect();
    cats.sort_by_key(|c| key(c));
    let mut out: Vec<Id> = top.into_iter().map(|c| c.id).collect();
    for cat in cats {
        let mut kids: Vec<_> = all.iter().filter(|c| c.parent == Some(cat.id)).collect();
        if kids.is_empty() {
            continue;
        }
        kids.sort_by_key(|c| key(c));
        out.push(cat.id);
        out.extend(kids.into_iter().filter(|c| selectable(c.kind)).map(|c| c.id));
    }
    out
}

pub fn remove_override_fields(channel: Id) -> serde_json::Value {
    serde_json::json!({ "channel_overrides": { channel.0.to_string(): { "message_notifications": 3, "muted": false } } })
}

pub fn matches_query(name: &str, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

pub fn flag_fields(i: usize, on: bool) -> Option<serde_json::Value> {
    Some(match i {
        2 => serde_json::json!({ "suppress_everyone": on }),
        3 => serde_json::json!({ "suppress_roles": on }),
        4 => serde_json::json!({ "notify_highlights": if on { 1 } else { 2 } }),
        5 => serde_json::json!({ "mute_scheduled_events": on }),
        6 => serde_json::json!({ "mobile_push": on }),
        _ => return None,
    })
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
    let mute = div().flex().flex_col().gap(z(20.)).child(mute).when(s.muted, |d| d.child(mute_window(s, cx)));
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
        let on = !s.flags[i];
        row(("gn-flag", i), crate::t!(t), (!d.is_empty()).then(|| crate::t!(d)), s.flags[i]).on_click(cx.listener(move |a, _, _, cx| {
            let mut flags = [false; 7];
            with_state(|s| {
                s.flags[i] = on;
                flags = s.flags;
            });
            if i < 2 {
                crate::ui::settings::save(a, crate::ui::settings::SettingsPatch::RaidAlerts(guild.0, raid_alerts(&flags)), cx);
            }
            if let Some(fields) = flag_fields(i, on) {
                a.nav_request(NavRequest::GuildNotify { guild, fields }, cx, |_, _, _| {});
            }
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
        .child(picker(app, s, cx))
        .child(override_table(app, s, cx));
    let community_visible = sees_raid_alerts(app.backend.guild_permissions(s.guild));
    let body = div()
        .id("gn-body")
        .max_h(z(560.))
        .overflow_y_scroll()
        .track_scroll(&crate::ui::chat::scrollbar::handle("gn-body"))
        .flex()
        .flex_col()
        .child(mute)
        .child(divider())
        .child(radios.child(opts))
        .child(divider())
        .when(community_visible, |d| d.child(community).child(divider()))
        .child(suppress)
        .child(divider())
        .child(overrides);
    let footer = button("gn-done", crate::t!("common.done"), BtnKind::Primary).flex_1().on_click({
        let close = close_action(cx);
        move |_, w, cx| close(w, cx)
    });
    let body = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Auto).mr(z(-24.)).pr(z(24.)).child(body).children(crate::ui::chat::scrollbar::bar_for("gn-body", crate::ui::chat::scrollbar::Bar::Auto));
    card(480., crate::t!("menu.notify_settings"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

fn mute_window(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::ui::thread_menu::MUTE_DURATIONS;
    let guild = s.guild;
    let options: Vec<SharedString> = MUTE_DURATIONS.iter().map(|(_, k)| crate::t!(k)).collect();
    let selected = MUTE_DURATIONS.iter().position(|(w, _)| *w == s.window).unwrap_or(MUTE_DURATIONS.len() - 1);
    let toggle: std::rc::Rc<dyn Fn(&mut gpui::App)> = std::rc::Rc::new(|_| with_state(|s| s.mute_open = !s.mute_open));
    let e = cx.entity();
    let pick: super::settings_form::OnPick = std::rc::Rc::new(move |k, _, cx| {
        let secs = MUTE_DURATIONS[k].0;
        with_state(|s| {
            s.window = secs;
            s.mute_open = false;
        });
        let req = NavRequest::Mute { guild: Some(guild), channel: None, muted: true, secs: u32::try_from(secs).ok() };
        e.update(cx, |a, cx| {
            a.nav_request(req, cx, |_, _, _| {});
            cx.notify();
        });
    });
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(label(crate::t!("gn.mute_window")))
        .child(super::settings_form::dropdown("gn-mute-window", options, selected, s.mute_open, toggle, pick, cx))
        .into_any_element()
}

fn picker(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(search) = s.search.clone() else { return div().into_any_element() };
    let q = search.read(cx).value().to_string();
    let open = s.picker || !q.trim().is_empty();
    let field = div()
        .id("gn-ov-select")
        .mt(z(4.))
        .h(z(40.))
        .pl(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if open { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT })))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(|_, _, _, cx| {
            with_state(|s| s.picker = true);
            cx.notify();
        }))
        .child(div().flex_1().min_w_0().child(gpui_component::input::Input::new(&search).appearance(false).bordered(false).px(z(0.)).text_size(z(16.)).line_height(z(20.))))
        .child(
            div().id("gn-ov-chevron").flex_none().w(z(26.)).mr(z(8.)).h_full().flex().items_center().justify_end().cursor_pointer().on_click(cx.listener(|_, _, _, cx| {
                cx.stop_propagation();
                with_state(|s| s.picker = !s.picker);
                cx.notify();
            })).child(
                gpui::svg()
                    .path(crate::icons::path("settings/chevron-down"))
                    .size(z(18.))
                    .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
            ),
        );
    let mut wrap = div().relative().w_full().child(field);
    if open {
        wrap = wrap.child(deferred(div().absolute().top(z(48.)).left_0().w_full().child(option_list(app, s.guild, &q, cx))).with_priority(30));
    }
    wrap.into_any_element()
}

fn option_list(app: &PhoenixApp, guild: Id, q: &str, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let rows: Vec<_> = override_targets(app, guild).into_iter().filter_map(|id| app.channels.get(&id)).filter(|ch| matches_query(&ch.name, q)).cloned().collect();
    let mut list = div()
        .id("gn-ov-options")
        .occlude()
        .max_h(z(44. * 7.))
        .overflow_y_scroll()
        .rounded_b(z(4.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .on_mouse_down_out(cx.listener(|_, _, _, cx| {
            with_state(|s| s.picker = false);
            cx.notify();
        }))
        .flex()
        .flex_col();
    if rows.is_empty() {
        list = list.child(
            div().p(z(12.)).flex().justify_center().bg(c(tok(idx::BACKGROUND_BASE_LOWER))).text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("gn.no_results")),
        );
    }
    for ch in rows {
        let id = ch.id;
        let category = ch.parent.and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
        list = list.child(
            div()
                .id(("gn-ov-option", id.0))
                .flex_none()
                .h(z(44.))
                .px(z(12.))
                .flex()
                .items_center()
                .overflow_hidden()
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .hover(|d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))).text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                .child(gpui::svg().path(crate::icons::path(super::create_channel::icon_for(ch.kind))).flex_none().min_w(z(16.)).size(z(16.)).opacity(0.3).text_color(c(tok(idx::TEXT_DEFAULT))))
                .child(div().mx(z(4.)).flex_shrink().min_w_0().truncate().child(ch.name.clone()))
                .children(category.map(|n| {
                    div()
                        .ml(z(8.))
                        .min_w(z(48.))
                        .flex_shrink()
                        .truncate()
                        .text_size(z(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT)))
                        .child(n)
                }))
                .on_click(cx.listener(move |_, _, w, cx| {
                    let mut search = None;
                    with_state(|s| {
                        if !s.added.contains(&id) {
                            s.added.push(id);
                        }
                        s.removed.retain(|x| *x != id);
                        s.picker = false;
                        search = s.search.clone();
                    });
                    if let Some(i) = search {
                        i.update(cx, |st, cx| st.set_value("", w, cx));
                    }
                    cx.notify();
                })),
        );
    }
    list.into_any_element()
}

fn override_table(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::notifications::Level;
    let prefs = app.backend.notify_prefs().map(|p| p.guild(Some(s.guild.0))).unwrap_or_default();
    let order = override_targets(app, s.guild);
    let rows: Vec<_> = order
        .iter()
        .filter(|id| !s.removed.contains(id))
        .filter_map(|id| {
            let p = prefs.channels.get(&id.0).copied().filter(|p| p.muted || p.level != Level::Inherit);
            let p = p.or_else(|| s.added.contains(id).then_some(crate::notifications::ChannelPrefs { level: Level::Inherit, muted: false, until: None, flags: 0 }))?;
            app.channels.get(id).map(|ch| (ch.clone(), p))
        })
        .collect();
    let head = |k: &str, w: f32| {
        div().w(z(w)).flex_none().h(z(44.)).child(
            div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!(k)),
        )
    };
    let mut table = div()
        .mt(z(26.))
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
    if rows.is_empty() {
        table = table.child(
            div()
                .h(z(56.))
                .mb(z(8.))
                .rounded(z(3.))
                .border_1()
                .border_dashed()
                .border_color(c(tok(idx::PRIMARY_800)))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .opacity(0.6)
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(crate::t!("gn.override_desc")),
                ),
        );
    }
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
        let remove = div()
            .id(("gn-ov-remove", cid.0))
            .group("gn-ov-rm")
            .absolute()
            .top(z(-12.))
            .right(z(-8.))
            .size(z(16.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .opacity(0.)
            .group_hover("gn-ov-card", |s| s.opacity(1.))
            .child(
                gpui::svg()
                    .path(crate::icons::path("settings/circle-x"))
                    .size(z(16.))
                    .text_color(c(tok(idx::ICON_SUBTLE)))
                    .group_hover("gn-ov-rm", |s| s.text_color(c(tok(idx::ICON_STRONG)))),
            )
            .on_click(cx.listener(move |a, _, _, cx| {
                with_state(|s| {
                    s.added.retain(|x| *x != cid);
                    if !s.removed.contains(&cid) {
                        s.removed.push(cid);
                    }
                });
                a.nav_request(NavRequest::GuildNotify { guild, fields: remove_override_fields(cid) }, cx, |_, _, _| {});
                cx.notify();
            }));
        table = table.child(
            div()
                .id(("gn-ov-card", cid.0))
                .group("gn-ov-card")
                .relative()
                .mb(z(8.))
                .flex()
                .pl(z(10.))
                .py(z(10.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .hover(|d| d.bg(c(tok(idx::PRIMARY_700)).opacity(0.1)))
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
                .child(opts)
                .child(remove),
        );
    }
    table.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{flag_fields, matches_query, raid_alerts, remove_override_fields, sees_raid_alerts};
    use crate::model::Id;

    #[test]
    fn flags_patch_the_client_fields() {
        assert_eq!(flag_fields(2, true), Some(serde_json::json!({ "suppress_everyone": true })));
        assert_eq!(flag_fields(4, true), Some(serde_json::json!({ "notify_highlights": 1 })));
        assert_eq!(flag_fields(4, false), Some(serde_json::json!({ "notify_highlights": 2 })));
        assert_eq!(flag_fields(6, false), Some(serde_json::json!({ "mobile_push": false })));
        assert_eq!(flag_fields(0, true), None);
    }

    #[test]
    fn community_alerts_follow_the_client_rules() {
        let mut f = [true; 7];
        assert_eq!(raid_alerts(&f), phoenix_api::rest::settings::RaidAlerts { push_off: false, nag_off: false });
        f[1] = false;
        assert_eq!(raid_alerts(&f), phoenix_api::rest::settings::RaidAlerts { push_off: true, nag_off: false });
        assert!(sees_raid_alerts(1 << 2));
        assert!(sees_raid_alerts(1 << 5));
        assert!(sees_raid_alerts(1 << 3));
        assert!(!sees_raid_alerts(1 << 10));
    }

    #[test]
    fn override_picker_filters_and_removal_resets_the_channel() {
        assert!(matches_query("général", "GÉN"));
        assert!(matches_query("annonces", " "));
        assert!(!matches_query("annonces", "vocal"));
        assert_eq!(remove_override_fields(Id(5)), serde_json::json!({ "channel_overrides": { "5": { "message_notifications": 3, "muted": false } } }));
    }
}
