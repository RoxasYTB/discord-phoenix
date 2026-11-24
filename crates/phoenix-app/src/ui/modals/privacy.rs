use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_manage::AdminCall;
use serde_json::Value;

use super::{Modal, card, close_action};
use crate::{
    actions::{admin::sf, close_modal, now_unix, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, radio_option, switch},
};

pub const DMS_KEY: &str = "restricted_guilds";
pub const REQUESTS_KEY: &str = "message_request_restricted_guild_ids";
pub const ACTIVITY_KEY: &str = "activity_restricted_guild_ids";
pub const JOINING_KEY: &str = "activity_joining_restricted_guild_ids";

pub struct Privacy {
    pub guild: Id,
    pub settings: Option<Value>,
}

impl Privacy {
    pub fn new(guild: Id) -> Self {
        Self { guild, settings: None }
    }

    pub fn load(&mut self, v: &Value) {
        self.settings = Some(v.clone());
    }

    pub fn listed(&self, key: &str) -> bool {
        let g = self.guild.0.to_string();
        self.settings.as_ref().and_then(|s| s.get(key)).and_then(Value::as_array).is_some_and(|a| a.iter().any(|x| x.as_str() == Some(&g)))
    }

    fn flip(&mut self, key: &str) -> (bool, Value) {
        let now = !self.listed(key);
        let s = self.settings.get_or_insert_with(|| serde_json::json!({}));
        let prev = s.get(key).cloned().unwrap_or(Value::Null);
        s[key] = phoenix_api::rest::guild_manage::toggle_list(&prev, sf(self.guild), now);
        (now, prev)
    }
}

fn row(id: &'static str, title: SharedString, desc: SharedString, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_start()
        .gap(z(24.))
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(title))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(desc)),
        )
        .group(crate::ui::controls::SWITCH_ROW)
        .child(switch("privacy-switch", on))
}

pub fn render_privacy(app: &PhoenixApp, p: &Privacy, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.guilds.iter().find(|g| g.id == p.guild).map(|g| g.name.to_string()).unwrap_or_default();
    let toggle = |key: &'static str| {
        move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut Context<PhoenixApp>| {
            let r = with_modal(|m| match m {
                Some(Modal::Privacy(p)) if p.settings.is_some() => Some((p.guild, p.flip(key))),
                _ => None,
            });
            if let Some((g, (now, prev))) = r {
                a.set_guild_privacy(g, key, now, prev, cx);
            }
            cx.notify();
        }
    };
    let dms = !p.listed(DMS_KEY);
    let requests = row("priv-requests", crate::t!("admin.privacy_requests"), crate::t!("admin.privacy_requests_desc"), dms && !p.listed(REQUESTS_KEY));
    let body = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(row("priv-dms", crate::t!("admin.privacy_dms"), crate::t!("admin.privacy_dms_desc"), dms).on_click(cx.listener(toggle(DMS_KEY))))
        .child(if dms { requests.on_click(cx.listener(toggle(REQUESTS_KEY))) } else { requests.opacity(0.5).cursor_default() })
        .child(
            row("priv-activity", crate::t!("admin.privacy_activity"), crate::t!("admin.privacy_activity_desc"), !p.listed(ACTIVITY_KEY))
                .on_click(cx.listener(toggle(ACTIVITY_KEY))),
        )
        .child(
            row("priv-joining", crate::t!("admin.privacy_joining"), crate::t!("admin.privacy_joining_desc"), !p.listed(JOINING_KEY))
                .on_click(cx.listener(toggle(JOINING_KEY))),
        )
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("admin.privacy_safety_note")));
    let footer = button("priv-done", crate::t!("common.done"), BtnKind::Primary).flex_1().on_click({
        let close = close_action(cx);
        move |_, w, cx| close(w, cx)
    });
    let title = format!("{}—{}", crate::t!("menu.privacy_settings"), name);
    card(400., title.into(), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

pub const PAUSES: [(&str, u64); 4] = [("admin.pause_1h", 3600), ("admin.pause_6h", 21_600), ("admin.pause_12h", 43_200), ("admin.pause_24h", 86_400)];

pub struct Security {
    pub guild: Id,
    pub invites: bool,
    pub dms: bool,
    pub duration: usize,
}

impl Security {
    pub fn new(guild: Id) -> Self {
        Self { guild, invites: false, dms: false, duration: 3 }
    }

    pub fn call(&self, now: u64) -> AdminCall {
        let until = now + PAUSES[self.duration.min(PAUSES.len() - 1)].1;
        AdminCall::IncidentActions { guild: sf(self.guild), invites_until: self.invites.then_some(until), dms_until: self.dms.then_some(until) }
    }
}

fn with_sec(f: impl FnOnce(&mut Security)) {
    with_modal(|m| {
        if let Some(Modal::Security(s)) = m {
            f(s)
        }
    });
}

pub fn render_security(s: &Security, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut body = div()
        .flex()
        .flex_col()
        .child(row("sec-invites", crate::t!("admin.pause_invites"), crate::t!("admin.pause_invites_desc"), s.invites).on_click(cx.listener(|_, _, _, cx| {
            with_sec(|s| s.invites = !s.invites);
            cx.notify();
        })))
        .child(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(row("sec-dms", crate::t!("admin.pause_dms"), crate::t!("admin.pause_dms_desc"), s.dms).on_click(cx.listener(|_, _, _, cx| {
            with_sec(|s| s.dms = !s.dms);
            cx.notify();
        })))
        .child(div().pt(z(12.)).pb(z(8.)).text_size(z(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("admin.pause_for")));
    for (n, (k, _)) in PAUSES.iter().enumerate() {
        body = body.child(radio_option(("sec-dur", n), crate::t!(k), s.duration == n).on_click(cx.listener(move |_, _, _, cx| {
            with_sec(|s| s.duration = n);
            cx.notify();
        })));
    }
    let footer = div()
        .flex()
        .gap(z(8.))
        .child(button("sec-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click({
            let close = close_action(cx);
            move |_, w, cx| close(w, cx)
        }))
        .child(button("sec-save", crate::t!("common.save"), BtnKind::Critical).on_click(cx.listener(|a, _, _, cx| {
            let call = with_modal(|m| match m {
                Some(Modal::Security(s)) => Some(s.call(now_unix())),
                _ => None,
            });
            close_modal();
            if let Some(call) = call {
                a.admin_toast(call, Some(crate::t!("admin.security_saved")), cx);
            }
            cx.notify();
        })));
    card(440., crate::t!("menu.security_actions"), Some(crate::t!("admin.security_sub")), body.into_any_element(), Some(footer.into_any_element()), cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_flip() {
        let mut p = Privacy::new(Id(5));
        p.load(&serde_json::json!({ DMS_KEY: ["5"] }));
        assert!(p.listed(DMS_KEY) && !p.listed(REQUESTS_KEY));
        let (now, prev) = p.flip(DMS_KEY);
        assert!(!now && !p.listed(DMS_KEY));
        assert_eq!(prev, serde_json::json!(["5"]));
    }

    #[test]
    fn security_call() {
        let s = Security { guild: Id(1), invites: true, dms: false, duration: 0 };
        assert_eq!(s.call(0), AdminCall::IncidentActions { guild: sf(Id(1)), invites_until: Some(3600), dms_until: None });
    }
}
