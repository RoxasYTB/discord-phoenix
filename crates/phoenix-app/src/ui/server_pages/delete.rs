use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};
use phoenix_api::rest::{guild_manage::AdminCall, guild_server::ServerCall};

use super::{page, sf, t, text};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button},
};

pub fn confirmed(typed: &str, name: &str) -> bool {
    !name.is_empty() && typed.trim() == name.trim()
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if !owns_server(app, guild) { return div().into_any_element(); }
    if super::with(|s| s.modal.is_none()) && !super::flag(guild, "delete-shown").unwrap_or(false) {
        super::set_flag(guild, "delete-shown", true);
        super::open_modal(guild, "delete");
    }
    page()
        .child(super::title(crate::t!("gs2.delete_server")))
        .child(div().mt(z(24.)).flex().child(button("gs2-del-open", crate::t!("gs2.delete_server"), BtnKind::Critical).on_click(cx.listener(move |_, _, _, cx| {
            super::open_modal(guild, "delete");
            cx.notify();
        }))))
        .into_any_element()
}

fn owns_server(app: &PhoenixApp, guild: Id) -> bool {
    app.guilds.iter().any(|g| g.id == guild && g.owner == Some(app.me.id))
}

pub fn modal(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if !owns_server(app, guild) { return div().into_any_element(); }
    let name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.to_string()).unwrap_or_default();
    let error = super::flag(guild, "delete-error").unwrap_or(false);
    let pending = super::flag(guild, "delete-pending").unwrap_or(false);
    let mfa_required = super::flag(guild, "delete-mfa-required").unwrap_or(false);
    let mfa_error = super::flag(guild, "delete-mfa-error").unwrap_or(false);
    let e = cx.entity();
    let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
        super::close_modal();
        super::forget_input(guild, "delete-name");
        super::with(|s| {
            s.flags.remove(&(guild, "delete-error".to_owned()));
            s.flags.remove(&(guild, "delete-mfa-required".to_owned()));
            s.flags.remove(&(guild, "delete-mfa-error".to_owned()));
        });
        super::forget_input(guild, "delete-mfa-code");
        e.update(cx, |_, cx| cx.notify());
    });
    let body = div()
        .flex()
        .flex_col()
        .child(
            div()
                .p(z(16.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_FEEDBACK_WARNING))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(t(idx::TEXT_DEFAULT))
                .child(crate::t!("gs2.delete_confirm", name = name)),
        )
        .when(mfa_required, |d| {
            d.child(div().mt(z(12.)).child(text(14., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.delete_mfa_required"))))
                .child(div().mt(z(12.)).child(text(12., 16., FontWeight::BOLD, if mfa_error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE }, crate::t!("gs2.delete_mfa_code").to_uppercase())))
                .children(mfa_error.then(|| text(12., 16., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, crate::t!("gs2.delete_mfa_wrong"))))
                .child(super::field(guild, "delete-mfa-code", "", cx).mt(z(8.)).when(mfa_error, |d| d.border_color(t(idx::BORDER_FEEDBACK_CRITICAL))))
        })
        .child(div().mt(z(20.)).child(text(12., 16., FontWeight::BOLD, if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE }, crate::t!("gs2.delete_enter_name").to_uppercase())))
        .children(error.then(|| text(12., 16., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, crate::t!("gs2.delete_wrong"))))
        .child(super::field(guild, "delete-name", "", cx).mt(z(8.)).when(error, |d| d.border_color(t(idx::BORDER_FEEDBACK_CRITICAL))));
    let close2 = close.clone();
    let n2 = name.clone();
    let footer = div()
        .flex()
        .gap(z(8.))
        .child(button("gs2-del-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(move |_, w, cx| close2(w, cx)))
        .child(button("gs2-del-ok", crate::t!("gs2.delete_server"), BtnKind::Critical).when(pending, |button| button.opacity(0.5)).on_click(cx.listener(move |a, _, _, cx| {
            if !owns_server(a, guild) || super::flag(guild, "delete-pending").unwrap_or(false) { return; }
            let typed = super::value(guild, "delete-name", cx).unwrap_or_default();
            let mfa_now = super::flag(guild, "delete-mfa-required").unwrap_or(false);
            let code: String = super::value(guild, "delete-mfa-code", cx).unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
            if mfa_now && code.is_empty() {
                super::set_flag(guild, "delete-mfa-error", true);
                cx.notify();
                return;
            }
            if confirmed(&typed, &n2) {
                super::set_flag(guild, "delete-error", false);
                super::set_flag(guild, "delete-mfa-error", false);
                super::set_flag(guild, "delete-pending", true);
                let call = AdminCall::Server(ServerCall::DeleteGuild { guild: sf(guild) });
                let done = move |a: &mut PhoenixApp, result: Result<serde_json::Value, String>, cx: &mut Context<PhoenixApp>| {
                    super::set_flag(guild, "delete-pending", false);
                    match result {
                        Ok(_) => {
                            if super::with(|s| s.modal == Some((guild, "delete"))) {
                                super::close_modal();
                                super::forget_input(guild, "delete-name");
                            }
                            if crate::ui::modals::server_settings::with_state(|s| s.guild == guild) == Some(true) {
                                crate::actions::close_modal();
                            }
                            if a.guilds.iter().any(|g| g.id == guild) {
                                a.handle_event(crate::backend::AppEvent::GuildDelete(guild), cx);
                            }
                        }
                        Err(error) if error.ends_with("(code 60003)") => super::set_flag(guild, "delete-mfa-required", true),
                        Err(error) if error.ends_with("(code 60008)") || error.ends_with("(code 60011)") || error.contains("(code 50035)") => {
                            super::set_flag(guild, "delete-mfa-required", true);
                            super::set_flag(guild, "delete-mfa-error", true);
                        }
                        Err(error) => a.parity_toast(error, cx),
                    }
                    cx.notify();
                };
                if mfa_now {
                    a.admin_mfa(call, code, cx, done);
                } else {
                    a.admin(call, cx, done);
                }
            } else {
                super::set_flag(guild, "delete-error", true);
            }
            cx.notify();
        })));
    crate::ui::modals::card_with(440., crate::t!("gs2.delete_title", name = name), None, body.into_any_element(), Some(footer.into_any_element()), close)
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_must_match() {
        assert!(super::confirmed(" Aperture Sciences ", "Aperture Sciences"));
        assert!(!super::confirmed("aperture sciences", "Aperture Sciences"));
        assert!(!super::confirmed("", ""));
    }
}
