use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::{AutomodRule, ServerCall, action, parse_rules, trigger};

use super::{back, fetch, icon, lead, link, page, sf, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::switch,
};

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n}\u{a0}{}", if n <= 1 { one } else { many })
}

pub fn summary(r: &AutomodRule) -> Option<String> {
    if r.trigger_type != trigger::KEYWORD {
        return None;
    }
    let k = r.trigger_metadata.keyword_filter.len();
    let x = r.trigger_metadata.regex_patterns.len();
    let words = plural(k, "mot", "mots");
    Some(if x == 0 { format!("({words})") } else { format!("({words}, {})", plural(x, "expression régulière", "expressions régulières")) })
}

pub fn duration(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{s}\u{a0}sec"),
        60 => "60\u{a0}sec".into(),
        s if s < 3600 => format!("{}\u{a0}min", s / 60),
        s if s < 86_400 => plural((s / 3600) as usize, "heure", "heures"),
        s if s < 604_800 => plural((s / 86_400) as usize, "jour", "jours"),
        s => plural((s / 604_800) as usize, "semaine", "semaines"),
    }
}

pub fn trigger_icon(kind: u8) -> &'static str {
    match kind {
        trigger::MENTION_SPAM => "user",
        trigger::SPAM => "flag",
        trigger::KEYWORD_PRESET => "warning",
        trigger::MEMBER_PROFILE => "user-profile",
        _ => "chat",
    }
}

fn action_lines(app: &PhoenixApp, r: &AutomodRule) -> Vec<(SharedString, Option<String>)> {
    let mut out = Vec::new();
    for a in &r.actions {
        match a.kind {
            action::BLOCK_MESSAGE => out.push((crate::t!("gs2.am_block"), None)),
            action::SEND_ALERT => {
                let ch = a.metadata.get("channel_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).map(Id);
                out.push((crate::t!("gs2.am_alert"), Some(format!("({})", super::channel_label(app, ch)))));
            }
            action::TIMEOUT => {
                let s = a.metadata.get("duration_seconds").and_then(|v| v.as_u64()).unwrap_or(60);
                out.push((crate::t!("gs2.am_timeout"), Some(format!("({})", duration(s)))));
            }
            action::BLOCK_INTERACTION => out.push((crate::t!("gs2.am_block_interaction"), None)),
            _ => {}
        }
    }
    out
}

fn action_icon(kind: u8) -> &'static str {
    match kind {
        action::SEND_ALERT => "bell",
        action::TIMEOUT => "lock",
        action::BLOCK_INTERACTION => "deny",
        _ => "close-small",
    }
}

fn header(app: &PhoenixApp, guild: Id, r: &AutomodRule, open: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let id = r.id.map(|i| i.0).unwrap_or(0);
    let enabled = r.enabled;
    let rule = r.clone();
    let mut actions = div().flex().flex_wrap().gap_x(z(16.)).gap_y(z(4.)).mt(z(8.));
    for (n, (a, (label, extra))) in r.actions.iter().zip(action_lines(app, r)).enumerate() {
        actions = actions.child(
            div()
                .id(("gs2-am-act", n))
                .flex()
                .items_center()
                .gap(z(6.))
                .child(icon(action_icon(a.kind), 16., idx::ICON_SUBTLE))
                .child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_SUBTLE, label.to_lowercase()))
                .children(extra.map(|e| text(12., 16., FontWeight::SEMIBOLD, idx::TEXT_MUTED, e))),
        );
    }
    let head = div()
        .flex()
        .items_center()
        .gap(z(4.))
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, r.name.clone()).max_w(z(475.)).truncate())
        .children(summary(r).map(|s| text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, s)));
    div()
        .id(SharedString::from(format!("gs2-am-rule-{id}")))
        .px(z(16.))
        .py(z(12.))
        .flex()
        .items_center()
        .when(!open, |d| d.cursor_pointer())
        .on_click(cx.listener(move |_, _, _, cx| {
            super::with(|s| {
                if s.rule_draft.as_ref().is_some_and(|(g, d)| *g == guild && d.id == rule.id) {
                    return;
                }
                s.rule_draft = Some((guild, rule.clone()));
            });
            cx.notify();
        }))
        .child(div().mr(z(16.)).p(z(8.)).rounded_full().bg(t(idx::BACKGROUND_MOD_SUBTLE)).child(icon(trigger_icon(r.trigger_type), 24., idx::ICON_DEFAULT)))
        .child(div().flex_1().min_w_0().flex().flex_col().child(head).when(!open, |d| d.child(actions)))
        .child(
            div()
                .flex()
                .items_center()
                .child(switch(SharedString::from(format!("gs2-am-sw-{id}")), enabled).on_click({
                    let r = r.clone();
                    cx.listener(move |a, _, _, cx| {
                        cx.stop_propagation();
                        let mut r = r.clone();
                        r.enabled = !r.enabled;
                        write(a, ServerCall::EditAutomodRule { guild: sf(guild), rule: r }, None, cx);
                    })
                }))
                .child(
                    div()
                        .id(SharedString::from(format!("gs2-am-more-{id}")))
                        .ml(z(8.))
                        .size(z(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_click({
                            let r = r.clone();
                            cx.listener(move |_, ev: &gpui::ClickEvent, _, cx| {
                                cx.stop_propagation();
                                let pos = ev.position();
                                super::with(|s| s.menu = Some((guild, super::AutomodMenu { rule: r.clone(), pos })));
                                cx.notify();
                            })
                        })
                        .child(icon("more-horizontal", 20., idx::ICON_DEFAULT)),
                ),
        )
}

fn step(n: usize, head: SharedString, body: AnyElement) -> gpui::Div {
    div()
        .px(z(16.))
        .py(z(12.))
        .flex()
        .gap(z(16.))
        .child(div().size(z(32.)).flex_none().rounded_full().bg(t(idx::BACKGROUND_MOD_NORMAL)).flex().items_center().justify_center().child(text(
            16.,
            20.,
            FontWeight::SEMIBOLD,
            idx::TEXT_STRONG,
            n.to_string(),
        )))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(z(8.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_STRONG, head)).child(body))
}

fn field(v: impl Into<SharedString>, h: f32, count: String) -> gpui::Div {
    div()
        .relative()
        .min_h(z(h))
        .p(z(10.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::INPUT_BORDER_DEFAULT))
        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
        .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, v))
        .child(div().absolute().right(z(12.)).bottom(z(8.)).child(text(12., 16., FontWeight::NORMAL, idx::TEXT_MUTED, count)))
}

pub fn add_regex(v: &str) {
    super::with(|s| {
        if let Some((_, r)) = s.rule_draft.as_mut() {
            let list = &mut r.trigger_metadata.regex_patterns;
            if list.len() < REGEX_MAX && !list.iter().any(|x| x == v) {
                list.push(v.to_owned());
            }
        }
    });
}

pub fn rule_key(r: &AutomodRule) -> u64 {
    r.id.map(|s| s.0).unwrap_or(0)
}

pub const KEYWORDS_MAX: usize = 1000;
pub const REGEX_MAX: usize = 10;
pub const ALLOW_MAX: usize = 100;

pub fn sync_draft(guild: Id, cx: &gpui::App) {
    let Some(id) = super::with(|s| s.rule_draft.as_ref().filter(|(g, _)| *g == guild).map(|(_, r)| rule_key(r))) else { return };
    let kw = super::area_value(guild, &format!("am-kw:{id}"), cx);
    let allow = super::area_value(guild, &format!("am-allow:{id}"), cx);
    super::with(|s| {
        if let Some((_, r)) = s.rule_draft.as_mut() {
            if let Some(v) = kw {
                r.trigger_metadata.keyword_filter = super::split_list(&v, KEYWORDS_MAX);
            }
            if let Some(v) = allow {
                r.trigger_metadata.allow_list = super::split_list(&v, ALLOW_MAX);
            }
        }
    });
}

fn chip(s: &str) -> gpui::Div {
    div()
        .px(z(4.))
        .py(z(4.))
        .rounded(z(4.))
        .bg(t(idx::BACKGROUND_MOD_NORMAL))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, s.to_owned()))
        .child(icon("close-small", 12., idx::ICON_SUBTLE))
}

fn editor(app: &PhoenixApp, guild: Id, d: &AutomodRule, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let m = &d.trigger_metadata;
    let mut out = div().flex().flex_col();
    if d.trigger_type == trigger::KEYWORD || d.trigger_type == trigger::MEMBER_PROFILE {
        let mut chips = div().flex().flex_wrap().items_center().gap(z(8.));
        for (i, r) in m.regex_patterns.iter().enumerate() {
            let pat = r.clone();
            chips = chips.child(chip(r).id(("am-rx-chip", i)).cursor_pointer().on_click(cx.listener(move |_, _, _, cx| {
                super::with(|s| {
                    if let Some((_, r)) = s.rule_draft.as_mut() {
                        r.trigger_metadata.regex_patterns.retain(|x| *x != pat);
                    }
                });
                cx.notify();
            })));
        }
        if m.regex_patterns.len() < REGEX_MAX {
            let key = format!("am-rx:{}", rule_key(d));
            chips = chips.child(match super::input(guild, &key, "", cx) {
                Some(e) => div().flex_1().min_w(z(120.)).child(gpui_component::input::Input::new(&e).appearance(false).bordered(false)).into_any_element(),
                None => div().into_any_element(),
            });
        }
        let kw_key = format!("am-kw:{}", rule_key(d));
        let keywords = div()
            .flex()
            .flex_col()
            .gap(z(8.))
            .child(super::area_field(guild, &kw_key, &m.keyword_filter.join(", "), 4, 98., cx).child(div().absolute().right(z(12.)).bottom(z(8.)).child(text(
                12.,
                16.,
                FontWeight::NORMAL,
                idx::TEXT_MUTED,
                format!("{} / {KEYWORDS_MAX}", m.keyword_filter.len()),
            ))))
            .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.am_keywords_hint")))
            .child(
                div()
                    .mt(z(12.))
                    .p(z(12.))
                    .rounded(z(8.))
                    .bg(t(idx::BACKGROUND_BASE_LOWER))
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("gs2.am_regex")))
                    .child(div().relative().min_h(z(105.)).p(z(10.)).rounded(z(8.)).border_1().border_color(t(idx::INPUT_BORDER_DEFAULT)).child(chips).child(
                        div().absolute().right(z(12.)).bottom(z(8.)).child(text(
                            12.,
                            16.,
                            FontWeight::NORMAL,
                            idx::TEXT_MUTED,
                            format!("{} / {REGEX_MAX}", m.regex_patterns.len()),
                        )),
                    ))
                    .child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!("gs2.am_regex_hint"))),
            )
            .child(div().mt(z(12.)).flex().flex_col().gap(z(8.)).child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("gs2.am_allow"))).child(
                super::area_field(guild, &format!("am-allow:{}", rule_key(d)), &m.allow_list.join(", "), 3, 78., cx).child(
                    div().absolute().right(z(12.)).bottom(z(8.)).child(text(
                        12.,
                        16.,
                        FontWeight::NORMAL,
                        idx::TEXT_MUTED,
                        format!("{} / {ALLOW_MAX}", m.allow_list.len()),
                    )),
                ),
            ));
        out = out.child(step(1, crate::t!("gs2.am_step_keywords"), keywords.into_any_element()));
    } else if d.trigger_type == trigger::MENTION_SPAM {
        let n = m.mention_total_limit.unwrap_or(20);
        out = out.child(step(1, crate::t!("gs2.am_step_mentions"), field(n.to_string(), 40., String::new()).into_any_element()));
    } else {
        out = out.child(step(1, crate::t!("gs2.am_step_detect"), lead(crate::t!("gs2.am_detect_desc")).into_any_element()));
    }
    let mut responses = div().flex().flex_col().gap(z(16.));
    let opts: &[(u8, &str, &str)] = if d.event_type == 2 {
        &[(action::BLOCK_INTERACTION, "gs2.am_block_interaction", "gs2.am_block_interaction_desc")]
    } else {
        &[
            (action::BLOCK_MESSAGE, "gs2.am_block", "gs2.am_block_desc"),
            (action::SEND_ALERT, "gs2.am_alert", "gs2.am_alert_desc"),
            (action::TIMEOUT, "gs2.am_timeout", "gs2.am_timeout_desc"),
        ]
    };
    for (i, (kind, h, desc)) in opts.iter().copied().enumerate() {
        let on = d.has_action(kind);
        let extra: Option<SharedString> = match kind {
            action::SEND_ALERT => Some(format!("{} {}", crate::t!("gs2.am_alert_to"), super::channel_label(app, d.alert_channel().map(|s| Id(s.0)))).into()),
            action::TIMEOUT => Some(format!("{} {}", crate::t!("gs2.am_timeout_for"), duration(d.timeout_seconds().unwrap_or(60))).into()),
            _ => None,
        };
        responses = responses.child(
            div()
                .id(("gs2-am-resp", i))
                .flex()
                .items_start()
                .gap(z(16.))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    super::with(|s| {
                        if let Some((_, r)) = s.rule_draft.as_mut() {
                            toggle_action(r, kind);
                        }
                    });
                    cx.notify();
                }))
                .child(icon(action_icon(kind), 24., idx::ICON_DEFAULT))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!(h)))
                        .child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_SUBTLE, crate::t!(desc)))
                        .children(extra.map(|e| text(12., 16., FontWeight::MEDIUM, idx::TEXT_MUTED, e))),
                )
                .child(crate::ui::community::checkbox(on)),
        );
    }
    out = out.child(step(2, crate::t!("gs2.am_step_response"), responses.into_any_element()));
    let mut exempt = div().flex().flex_wrap().gap(z(8.));
    for r in &d.exempt_roles {
        let name = app.roles.get(&guild).and_then(|v| v.iter().find(|x| x.id.0 == r.0)).map(|x| x.name.to_string()).unwrap_or_default();
        exempt = exempt.child(chip(&name));
    }
    for ch in &d.exempt_channels {
        exempt = exempt.child(chip(&super::channel_label(app, Some(Id(ch.0)))));
    }
    out.child(step(
        3,
        crate::t!("gs2.am_step_exempt"),
        div()
            .flex()
            .flex_col()
            .gap(z(8.))
            .child(div().min_h(z(38.)).p(z(6.)).rounded(z(8.)).border_1().border_color(t(idx::INPUT_BORDER_DEFAULT)).child(exempt))
            .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.am_exempt_hint")))
            .into_any_element(),
    ))
}

pub fn rule_menu(guild: Id, m: super::AutomodMenu, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::ui::context_menu::{Dismiss, MenuItem};
    let e = cx.entity();
    let dismiss: Dismiss = {
        let e = e.clone();
        Rc::new(move |_, cx| {
            super::with(|s| s.menu = None);
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let (edit_rule, del_rule) = (m.rule.clone(), m.rule.clone());
    let rid = m.rule.id.map(|i| i.0.to_string()).unwrap_or_default();
    let (e1, e2) = (e.clone(), e.clone());
    let items = vec![
        MenuItem::action(crate::t!("gs2.am_edit_rule"), None, move |_, _, cx| {
            super::with(|s| {
                s.menu = None;
                s.rule_draft = Some((guild, edit_rule.clone()));
            });
            e1.update(cx, |_, cx| cx.notify());
        }),
        MenuItem::danger(crate::t!("gs2.am_delete_rule"), None, move |_, _, cx| {
            super::with(|s| s.menu = None);
            let rule = del_rule.id.unwrap_or_default();
            e2.update(cx, |a, cx| write(a, ServerCall::DeleteAutomodRule { guild: sf(guild), rule }, None, cx));
        }),
        MenuItem::Separator,
        MenuItem::action(crate::t!("gs2.am_copy_id"), None, move |_, _, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(rid.clone()));
            super::with(|s| s.menu = None);
            e.update(cx, |_, cx| cx.notify());
        })
        .with_icon("id"),
    ];
    crate::ui::context_menu::render_anchored_at(m.pos, gpui::Anchor::TopRight, items, dismiss, 30)
}

pub fn toggle_action(r: &mut AutomodRule, kind: u8) {
    if r.has_action(kind) {
        r.actions.retain(|a| a.kind != kind);
    } else {
        let metadata = match kind {
            action::TIMEOUT => serde_json::json!({ "duration_seconds": 60 }),
            _ => serde_json::json!({}),
        };
        r.actions.push(phoenix_api::rest::guild_server::AutomodAction { kind, metadata });
    }
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let rules = fetch(app, guild, ServerCall::AutomodRules { guild: sf(guild) }, cx);
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx);
    let mut p = page()
        .child(back(guild, super::safety::SECTION, cx))
        .child(title(crate::t!("gs2.automod_title")))
        .child(div().mt(z(8.)).child(lead(crate::t!("gs2.automod_lead"))))
        .child(div().mt(z(4.)).child(link(crate::t!("gs2.learn_more"))));
    let Some(rules) = rules else {
        return p.child(super::loading()).into_any_element();
    };
    let rules = parse_rules(&rules);
    sync_draft(guild, cx);
    let draft = super::with(|s| s.rule_draft.clone()).filter(|(g, _)| *g == guild).map(|(_, r)| r);
    let groups = [("gs2.am_members", true), ("gs2.am_content", false)];
    for (key, members) in groups {
        let list: Vec<&AutomodRule> = rules.iter().filter(|r| (r.trigger_type == trigger::MEMBER_PROFILE) == members).collect();
        if list.is_empty() {
            continue;
        }
        p = p.child(div().mt(z(16.)).mb(z(12.)).child(text(20., 24., FontWeight::NORMAL, idx::TEXT_STRONG, crate::t!(key))));
        for r in list {
            let open = draft.as_ref().is_some_and(|d| d.id == r.id);
            let shown = if open { draft.clone().unwrap_or_else(|| r.clone()) } else { r.clone() };
            let mut card = div()
                .mb(z(8.))
                .bg(t(if open { idx::BACKGROUND_BASE_LOWER } else { idx::BACKGROUND_SURFACE_HIGH }))
                .border_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .rounded(z(8.))
                .flex()
                .flex_col()
                .child(header(app, guild, &shown, open, cx));
            if open {
                card = card.child(editor(app, guild, &shown, cx));
            }
            p = p.child(card);
        }
    }
    if let Some(g) = g {
        p = p.child(super::safety::content_filter(guild, &g, cx));
    }
    let dirty = draft.as_ref().is_some_and(|d| rules.iter().any(|r| r.id == d.id && r != d));
    if let Some(d) = draft.filter(|_| dirty) {
        let reset: crate::ui::modals::settings_layout::Act = Rc::new(move |_, _, _| {
            super::with(|s| s.rule_draft = None);
            super::forget_prefix(guild, "am-");
        });
        let save: crate::ui::modals::settings_layout::Act = Rc::new(move |a, _, cx| {
            let r = d.clone();
            super::with(|s| s.rule_draft = None);
            super::forget_prefix(guild, "am-");
            write(a, ServerCall::EditAutomodRule { guild: sf(guild), rule: r }, None, cx);
        });
        p = p.child(div().mt(z(16.)).child(crate::ui::modals::settings_layout::unsaved_bar(reset, save, cx)));
    }
    p.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_api::rest::guild_server::TriggerMetadata;

    #[test]
    fn rule_summaries_match_the_client() {
        let mut r = AutomodRule { trigger_type: trigger::KEYWORD, ..Default::default() };
        r.trigger_metadata = TriggerMetadata { keyword_filter: vec!["a".into(), "b".into()], regex_patterns: vec!["x".into()], ..Default::default() };
        assert_eq!(summary(&r).unwrap(), "(2\u{a0}mots, 1\u{a0}expression régulière)");
        r.trigger_metadata.keyword_filter.clear();
        assert_eq!(summary(&r).unwrap(), "(0\u{a0}mot, 1\u{a0}expression régulière)");
        r.trigger_metadata.regex_patterns.clear();
        r.trigger_metadata.keyword_filter.push("dox".into());
        assert_eq!(summary(&r).unwrap(), "(1\u{a0}mot)");
        r.trigger_type = trigger::SPAM;
        assert!(summary(&r).is_none());
    }

    #[test]
    fn durations_match_the_client() {
        assert_eq!(duration(60), "60\u{a0}sec");
        assert_eq!(duration(300), "5\u{a0}min");
        assert_eq!(duration(3600), "1\u{a0}heure");
        assert_eq!(duration(86_400), "1\u{a0}jour");
        assert_eq!(duration(604_800), "1\u{a0}semaine");
    }

    #[test]
    fn keyword_lists_split_like_the_client() {
        assert_eq!(super::super::split_list("dox, *spam*,\n  dox ,, test", KEYWORDS_MAX), vec!["dox", "*spam*", "test"]);
        assert_eq!(super::super::split_list("a,b,c", 2), vec!["a", "b"]);
        assert!(super::super::split_list(" , ", 10).is_empty());
    }

    #[test]
    fn toggling_actions() {
        let mut r = AutomodRule::default();
        toggle_action(&mut r, action::TIMEOUT);
        assert_eq!(r.timeout_seconds(), Some(60));
        toggle_action(&mut r, action::TIMEOUT);
        assert!(r.actions.is_empty());
    }
}
