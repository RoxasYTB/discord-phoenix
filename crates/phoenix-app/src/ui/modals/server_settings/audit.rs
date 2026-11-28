use std::rc::Rc;

use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use serde_json::Value;

use super::{
    State,
    kit::{self, Cx, text},
    with_state,
};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

pub const ACTIONS: [u32; 56] = [
    1, 10, 11, 12, 13, 14, 15, 20, 21, 22, 23, 24, 25, 26, 27, 28, 30, 31, 32, 40, 41, 42, 50, 51, 52, 60, 61, 62, 72, 73, 74, 75, 80, 81, 82, 83, 84, 85, 90, 91, 92, 100, 101, 102, 110, 111, 112, 121, 130, 131,
    132, 140, 141, 142, 192, 193,
];

pub fn icon_for(action: u32) -> &'static str {
    match action {
        1 => "server/server",
        10..=15 => "hash",
        20..=28 => "members",
        30..=32 => "server/badge",
        40..=42 => "invite",
        50..=52 => "settings/webhook",
        60..=62 => "emoji",
        72..=75 => "chat",
        80..=82 => "settings/puzzle",
        83..=85 => "stage",
        90..=92 => "sticker",
        100..=102 => "calendar",
        110..=112 => "threads",
        130..=132 => "voice/soundboard",
        140..=145 => "shield-lock",
        _ => "server/clipboard",
    }
}

pub fn kind_color(action: u32) -> u32 {
    tok(match action {
        1 => idx::ICON_SUBTLE,
        20..=22 | 27 => idx::ICON_FEEDBACK_CRITICAL,
        23 | 28 => idx::ICON_FEEDBACK_POSITIVE,
        24..=26 => idx::ICON_FEEDBACK_WARNING,
        a if a.is_multiple_of(10) => idx::ICON_FEEDBACK_POSITIVE,
        a if a % 10 == 2 => idx::ICON_FEEDBACK_CRITICAL,
        _ => idx::ICON_FEEDBACK_WARNING,
    })
}

pub fn target_name(log: &Value, e: &Value, app: &PhoenixApp) -> String {
    let tid = kit::s(e, "target_id");
    let action = kit::n(e, "action_type") as u32;
    let find = |k: &str| log.get(k).and_then(Value::as_array).and_then(|l| l.iter().find(|x| kit::s(x, "id") == tid)).cloned();
    match action {
        20..=28 => find("users").map(|u| kit::user_name(&u)).unwrap_or(tid),
        10..=15 | 72..=75 | 110..=112 => {
            let name = changed(e, "name").or_else(|| tid.parse().ok().and_then(|id: u64| app.channels.get(&Id(id)).map(|c| c.name.to_string())));
            name.map(|n| if (110..=112).contains(&action) { n } else { format!("#{n}") }).unwrap_or(tid)
        }
        30..=32 => changed(e, "name").or_else(|| app.roles.values().flatten().find(|r| r.id.0.to_string() == tid).map(|r| r.name.to_string())).unwrap_or(tid),
        50..=52 => find("webhooks").map(|w| kit::s(&w, "name")).or_else(|| changed(e, "name")).unwrap_or(tid),
        80..=82 => find("integrations").map(|w| kit::s(&w, "name")).unwrap_or(tid),
        140..=142 => find("auto_moderation_rules").map(|w| kit::s(&w, "name")).or_else(|| changed(e, "name")).unwrap_or(tid),
        40..=42 => changed(e, "code").unwrap_or(tid),
        _ => changed(e, "name").unwrap_or(tid),
    }
}

fn changed(e: &Value, key: &str) -> Option<String> {
    e.get("changes")?.as_array()?.iter().find(|c| kit::s(c, "key") == key).and_then(|c| c.get("new_value").or_else(|| c.get("old_value"))).and_then(|v| v.as_str().map(str::to_owned))
}

pub fn sentence(action: u32, user: &str, target: &str, channel: &str, count: u64) -> String {
    let key = format!("gs.audit.t{action}");
    let t = crate::t!(&key, user = user, target = target, channel = channel, count = count);
    if t.as_ref() == key { format!("{user} · {action}") } else { t.to_string() }
}

fn value_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "—".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.iter().map(|x| x.get("name").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>().join(", "),
        Some(x) => x.to_string(),
    }
}

fn change_rows(e: &Value) -> AnyElement {
    let mut col = div().pl(z(56.)).pr(z(16.)).pb(z(16.)).flex().flex_col().gap(z(4.));
    for ch in e.get("changes").and_then(Value::as_array).into_iter().flatten() {
        let key = kit::s(ch, "key");
        let old = ch.get("old_value");
        let new = ch.get("new_value");
        let line = match (old.is_some(), new.is_some()) {
            (true, true) => crate::t!("gs.audit.changed_from", key = key, old = value_text(old), new = value_text(new)),
            (false, true) => crate::t!("gs.audit.set_to", key = key, new = value_text(new)),
            _ => crate::t!("gs.audit.removed", key = key, old = value_text(old)),
        };
        col = col.child(div().flex().gap(z(8.)).child(div().mt(z(7.)).size(z(4.)).flex_none().rounded_full().bg(c(tok(idx::TEXT_MUTED)))).child(text(line, 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT)));
    }
    if let Some(r) = e.get("reason").and_then(Value::as_str) {
        col = col.child(text(crate::t!("gs.audit.reason", reason = r), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE));
    }
    col.into_any_element()
}

fn when(id: u64) -> String {
    let t = chrono::DateTime::from_timestamp(kit::snowflake_unix(id), 0).unwrap_or_default().with_timezone(&chrono::Local);
    crate::i18n::calendar(t, chrono::Local::now()).to_string()
}

fn user_menu(user: Option<u64>, guild: Id) -> impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    move |ev, w, cx| {
        if let Some(u) = user {
            cx.stop_propagation();
            crate::ui::user_menu::open(Id(u), Some(guild), ev.position, w);
        }
    }
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let log = s.get("audit").cloned().unwrap_or(Value::Null);
    let entries: Vec<Value> = log.get("audit_log_entries").and_then(Value::as_array).cloned().unwrap_or_default();
    let users: Vec<Value> = log.get("users").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut user_opts: Vec<SharedString> = vec![crate::t!("gs.audit.all_users")];
    let mut user_ids: Vec<Option<u64>> = vec![None];
    for u in &users {
        user_opts.push(kit::user_name(u).into());
        user_ids.push(kit::id_of(u, "id"));
    }
    let mut action_opts: Vec<SharedString> = vec![crate::t!("gs.audit.all_actions")];
    let mut action_ids: Vec<Option<u32>> = vec![None];
    for a in ACTIONS {
        if a != 142 {
            action_opts.push(crate::t!(&format!("gs.audit.a{a}")));
            action_ids.push(Some(a));
        }
    }
    let ucur = user_ids.iter().position(|u| *u == s.audit_user);
    let acur = action_ids.iter().position(|a| *a == s.audit_action);
    let uval = user_opts.get(ucur.unwrap_or(0)).cloned().unwrap_or_default();
    let aval = action_opts.get(acur.unwrap_or(0)).cloned().unwrap_or_default();
    let filters = div()
        .flex()
        .gap(z(16.))
        .child(div().w(z(249.)).flex().flex_col().gap(z(8.)).child(text(crate::t!("gs.audit.filter_user"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(kit::dropdown(
            s,
            "audit_user",
            text(uval, 16., 20., FontWeight::NORMAL, idx::TEXT_STRONG).into_any_element(),
            user_opts,
            ucur,
            Rc::new(move |a: &mut PhoenixApp, i: usize, cx: &mut Cx| {
                let v = user_ids.get(i).copied().flatten();
                with_state(|s| s.audit_user = v);
                super::reload(a, "audit", cx);
            }),
            cx,
        )))
        .child(div().w(z(249.)).flex().flex_col().gap(z(8.)).child(text(crate::t!("gs.audit.filter_action"), 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG)).child(kit::dropdown(
            s,
            "audit_action",
            text(aval, 16., 20., FontWeight::NORMAL, idx::TEXT_STRONG).into_any_element(),
            action_opts,
            acur,
            Rc::new(move |a: &mut PhoenixApp, i: usize, cx: &mut Cx| {
                let v = action_ids.get(i).copied().flatten();
                with_state(|s| s.audit_action = v);
                super::reload(a, "audit", cx);
            }),
            cx,
        )));
    let mut list = div().flex().flex_col().gap(z(8.));
    for (i, e) in entries.iter().enumerate() {
        let id = kit::s(e, "id");
        let action = kit::n(e, "action_type") as u32;
        let uid = kit::s(e, "user_id");
        let uid_n: Option<u64> = uid.parse().ok();
        let guild = s.guild;
        let user = users.iter().find(|u| kit::s(u, "id") == uid).cloned().unwrap_or(Value::Null);
        let target = target_name(&log, e, app);
        let opt = e.get("options");
        let channel = opt.and_then(|o| o.get("channel_id")).and_then(Value::as_str).and_then(|c| c.parse().ok()).and_then(|c: u64| app.channels.get(&Id(c))).map(|c| format!("#{}", c.name)).unwrap_or_default();
        let count = opt.and_then(|o| kit::id_of(o, "count").or_else(|| kit::id_of(o, "members_removed"))).unwrap_or(1);
        let has_changes = e.get("changes").and_then(Value::as_array).is_some_and(|c| !c.is_empty()) || e.get("reason").is_some();
        let open = s.expanded.contains(&id);
        let key = id.clone();
        list = list.child(
            div()
                .id(("ss-audit", i))
                .rounded(z(8.))
                .border_1()
                .p(z(4.))
                .border_color(c(tok(if open { idx::BORDER_NORMAL } else { idx::BORDER_SUBTLE })))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(("ss-audit-head", i))
                        .min_h(z(60.))
                        .pt(z(10.))
                        .pb(z(10.))
                        .pl(z(10.))
                        .pr(z(20.))
                        .rounded(z(4.))
                        .flex()
                        .items_center()
                        .when(has_changes, |d| d.cursor_pointer().hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
                        .child(gpui::svg().path(crate::icons::path(icon_for(action))).flex_none().size(z(24.)).text_color(c(kind_color(action))))
                        .child(div().ml(z(10.)).on_mouse_down(gpui::MouseButton::Right, user_menu(uid_n, guild)).child(kit::avatar(kit::user_avatar(&user, 80), 40.)))
                        .child(
                            div()
                                .on_mouse_down(gpui::MouseButton::Right, user_menu(uid_n, guild))
                                .mx(z(10.))
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(text(sentence(action, &kit::user_name(&user), &target, &channel, count), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).truncate())
                                .child(text(when(kit::n(e, "id")), 14., 18., FontWeight::NORMAL, idx::TEXT_MUTED)),
                        )
                        .when(has_changes, |d| d.child(gpui::svg().path(crate::icons::path(if open { "chevron" } else { "chevron-right" })).flex_none().size(z(24.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            let k = key.clone();
                            with_state(|s| {
                                if !s.expanded.remove(&k) {
                                    s.expanded.insert(k);
                                }
                            });
                            cx.notify();
                        })),
                )
                .when(open, |d| d.child(change_rows(e))),
        );
    }
    div()
        .flex()
        .flex_col()
        .child(div().flex().items_center().justify_between().child(kit::h2(crate::t!("menu.gs.audit_log"))).child(filters))
        .child(div().my(z(20.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(if entries.is_empty() && !s.pending.contains("audit") {
            text(crate::t!("gs.audit.empty"), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE).into_any_element()
        } else {
            list.into_any_element()
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_action_has_an_icon_and_colour() {
        for a in ACTIONS {
            assert!(!icon_for(a).is_empty());
            let _ = kind_color(a);
        }
        assert_eq!(kind_color(10), kind_color(23));
        assert_eq!(kind_color(22), kind_color(12));
        assert_ne!(kind_color(20), kind_color(10));
    }

    #[test]
    fn change_target_lookup() {
        let e = json!({ "action_type": 11, "target_id": "5", "changes": [{ "key": "name", "old_value": "a", "new_value": "b" }] });
        assert_eq!(changed(&e, "name").as_deref(), Some("b"));
        assert_eq!(value_text(Some(&json!([{ "name": "Mods" }]))), "Mods");
        assert_eq!(value_text(None), "—");
    }
}
