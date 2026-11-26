use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_manage::AdminCall;
use serde_json::Value;

use super::{Modal, card, close_action};
use crate::{
    actions::{admin::sf, now_unix, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, switch},
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

pub const PAUSE_HOURS: [u32; 6] = [1, 2, 4, 6, 12, 24];

pub struct Security {
    pub guild: Id,
    pub invites: bool,
    pub dms: bool,
    pub invites0: bool,
    pub dms0: bool,
    pub hours: u32,
    pub hours0: u32,
    pub open: bool,
    pub locked: bool,
}

impl Security {
    pub fn new(guild: Id) -> Self {
        Self { guild, invites: false, dms: false, invites0: false, dms0: false, hours: 2, hours0: 2, open: false, locked: false }
    }

    pub fn with_state(guild: Id, invites: bool, dms: bool, hours: Option<u32>, locked: bool) -> Self {
        let hours = hours.filter(|h| PAUSE_HOURS.contains(h)).unwrap_or(2);
        Self { guild, invites, dms, invites0: invites, dms0: dms, hours, hours0: hours, open: false, locked }
    }

    pub fn dirty(&self) -> bool {
        self.invites != self.invites0 || self.dms != self.dms0 || self.hours != self.hours0
    }

    pub fn call(&self, now: u64) -> AdminCall {
        let any = self.invites || self.dms;
        let until = now + u64::from(self.hours) * 3600;
        AdminCall::IncidentActions {
            guild: sf(self.guild),
            invites_until: self.invites.then_some(until),
            dms_until: self.dms.then_some(until),
            lockdown_hours: any.then_some(self.hours),
        }
    }
}

pub fn hours_label(h: u32) -> SharedString {
    if h == 1 { crate::t!("admin.pause_one_hour") } else { crate::t!("admin.pause_n_hours", n = h) }
}

pub(crate) fn with_sec(f: impl FnOnce(&mut Security)) {
    with_modal(|m| match m {
        Some(Modal::Security(s)) => f(s),
        Some(Modal::ChannelSettings(cs)) => {
            if let Some(s) = cs.security.as_mut() {
                f(s)
            }
        }
        Some(Modal::ServerSettings(ss)) => {
            if let Some(s) = ss.security.as_mut() {
                f(s)
            }
        }
        _ => {}
    });
}

fn pause_card(id: &'static str, title: SharedString, desc: SharedString, on: bool, disabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .p(z(12.))
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .items_start()
        .gap(z(8.))
        .when(!disabled, |d| d.cursor_pointer())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(title))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(desc)),
        )
        .group(crate::ui::controls::SWITCH_ROW)
        .child(div().flex_none().when(disabled, |d| d.opacity(0.3)).child(switch(id, on)))
}

pub fn render_security(s: &Security, cx: &mut Context<PhoenixApp>) -> AnyElement {
    render_security_with(s, close_action(cx), true, cx)
}

pub fn render_security_with(s: &Security, close: crate::ui::modal::Action, framed: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let toggle: std::rc::Rc<dyn Fn(&mut gpui::App)> = std::rc::Rc::new(|_| with_sec(|s| s.open = !s.open));
    let pick: super::settings_form::OnPick = {
        let e = cx.entity();
        std::rc::Rc::new(move |k, _, cx| {
            with_sec(|s| {
                s.hours = PAUSE_HOURS[k.min(PAUSE_HOURS.len() - 1)];
                s.open = false;
            });
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let sel = PAUSE_HOURS.iter().position(|h| *h == s.hours).unwrap_or(1);
    let locked = s.locked;
    let body = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(super::settings_form::dropdown("sec-hours", PAUSE_HOURS.iter().map(|h| hours_label(*h)).collect(), sel, s.open, toggle, pick, cx))
        .child(pause_card("sec-invites", crate::t!("admin.pause_invites"), crate::t!("admin.pause_invites_desc"), s.invites, locked).on_click(cx.listener(move |_, _, _, cx| {
            if !locked {
                with_sec(|s| s.invites = !s.invites);
            }
            cx.notify();
        })))
        .child(pause_card("sec-dms", crate::t!("admin.pause_dms"), crate::t!("admin.pause_dms_desc"), s.dms, false).on_click(cx.listener(|_, _, _, cx| {
            with_sec(|s| s.dms = !s.dms);
            cx.notify();
        })));
    let dirty = s.dirty();
    let c2 = close.clone();
    let c_save = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("sec-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| c2(w, cx)))
        .child(button("sec-save", crate::t!("admin.security_save"), BtnKind::Primary).flex_1().when(!dirty, |b| b.opacity(0.5)).on_click(cx.listener(move |a, _, w, cx| {
            if !dirty {
                return;
            }
            let mut call = None;
            with_sec(|s| call = Some(s.call(now_unix())));
            c_save(w, cx);
            if let Some(call) = call {
                a.admin_toast(call, Some(crate::t!("admin.security_saved")), cx);
            }
            cx.notify();
        })));
    if framed {
        return super::card_with(480., crate::t!("menu.security_actions"), None, body.into_any_element(), Some(footer.into_any_element()), close);
    }
    let body = div().px(z(24.)).pt(z(8.)).child(body).into_any_element();
    super::channel_perms::sub_frame("cs-security", 480., crate::t!("menu.security_actions"), body, footer.into_any_element(), close)
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
    fn security_call_matches_the_client() {
        let mut s = Security::new(Id(1));
        assert!(!s.dirty());
        s.invites = true;
        s.hours = 1;
        assert!(s.dirty());
        assert_eq!(s.call(0), AdminCall::IncidentActions { guild: sf(Id(1)), invites_until: Some(3600), dms_until: None, lockdown_hours: Some(1) });
        let off = Security::with_state(Id(1), false, false, Some(7), false);
        assert_eq!(off.hours, 2);
        assert_eq!(off.call(0), AdminCall::IncidentActions { guild: sf(Id(1)), invites_until: None, dms_until: None, lockdown_hours: None });
    }
}
