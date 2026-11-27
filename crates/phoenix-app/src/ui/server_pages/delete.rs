use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};
use phoenix_api::rest::{guild_manage::{AdminCall, AdminMfaMethod}, guild_server::ServerCall};

use super::{page, sf, t, text};
use crate::{
    actions::{ActionReply, ActionRequest},
    app::PhoenixApp,
    discord_backend::store::Secret,
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

pub(super) fn cancel(guild: Id) {
    if super::flag(guild, "delete-pending").unwrap_or(false) {
        return;
    }
    super::close_modal();
    super::forget_input(guild, "delete-name");
    super::forget_input(guild, "delete-mfa-code");
    super::with(|s| s.flags.retain(|(id, key), _| *id != guild || !key.starts_with("delete-") || key == "delete-shown"));
}

pub fn modal(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if !owns_server(app, guild) { return div().into_any_element(); }
    let name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.to_string()).unwrap_or_default();
    let error = super::flag(guild, "delete-error").unwrap_or(false);
    let pending = super::flag(guild, "delete-pending").unwrap_or(false);
    let writes_allowed = crate::actions::guard::writes_enabled();
    let mfa_required = super::flag(guild, "delete-mfa-required").unwrap_or(false);
    let mfa_error = super::flag(guild, "delete-mfa-error").unwrap_or(false);
    let authenticator = super::flag(guild, "delete-mfa-authenticator").unwrap_or(false);
    let backup = super::flag(guild, "delete-mfa-backup").unwrap_or(false);
    let use_backup = super::flag(guild, "delete-mfa-use-backup").unwrap_or(false);
    let method = match (authenticator, backup, use_backup) {
        (_, true, true) | (false, true, _) => Some(AdminMfaMethod::Backup),
        (true, _, _) => Some(AdminMfaMethod::Authenticator),
        _ => None,
    };
    let e = cx.entity();
    let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
        cancel(guild);
        e.update(cx, |_, cx| cx.notify());
    });
    let error_style = if mfa_error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE };
    let code_label = if method == Some(AdminMfaMethod::Backup) { crate::t!("gs2.delete_mfa_backup") } else { crate::t!("gs2.delete_mfa_code") };
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
        .when(!writes_allowed, |d| d.child(div().mt(z(12.)).child(super::description(crate::t!("gs2.delete_protected")))))
        .when(mfa_required, |d| d.child(div().mt(z(12.)).child(text(14., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.delete_mfa_required")))))
        .when(mfa_required && method.is_none(), |d| d.child(div().mt(z(8.)).child(super::description(crate::t!("gs2.delete_mfa_unsupported")))))
        .when(method.is_some(), |d| {
            d.child(div().mt(z(12.)).child(text(12., 16., FontWeight::BOLD, error_style, code_label.to_uppercase())))
                .children(mfa_error.then(|| text(12., 16., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, crate::t!("gs2.delete_mfa_wrong"))))
                .child(super::field(guild, "delete-mfa-code", "", cx).mt(z(8.)).when(mfa_error, |d| d.border_color(t(idx::BORDER_FEEDBACK_CRITICAL))))
                .when(authenticator && backup && !pending, |d| {
                    let label = if use_backup { crate::t!("gs2.delete_mfa_use_app") } else { crate::t!("gs2.delete_mfa_use_backup") };
                    d.child(div().id("delete-mfa-switch").mt(z(8.)).child(super::link(label)).on_click(cx.listener(move |_, _, _, cx| {
                        super::set_flag(guild, "delete-mfa-use-backup", !use_backup);
                        super::set_flag(guild, "delete-mfa-error", false);
                        super::forget_input(guild, "delete-mfa-code");
                        cx.notify();
                    })))
                })
        })
        .child(div().mt(z(20.)).child(text(12., 16., FontWeight::BOLD, if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE }, crate::t!("gs2.delete_enter_name").to_uppercase())))
        .children(error.then(|| text(12., 16., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, crate::t!("gs2.delete_wrong"))))
        .child(super::field(guild, "delete-name", "", cx).mt(z(8.)).when(error, |d| d.border_color(t(idx::BORDER_FEEDBACK_CRITICAL))));
    let close2 = close.clone();
    let disabled = pending || !writes_allowed || (mfa_required && method.is_none());
    let footer = div()
        .flex()
        .gap(z(8.))
        .child(button("gs2-del-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(move |_, w, cx| close2(w, cx)))
        .child(button("gs2-del-ok", crate::t!("gs2.delete_server"), BtnKind::Critical).when(disabled, |button| button.opacity(0.5)).on_click(cx.listener(move |a, _, window, cx| {
            if disabled || !owns_server(a, guild) || super::flag(guild, "delete-pending").unwrap_or(false) { return; }
            let typed = super::value(guild, "delete-name", cx).unwrap_or_default();
            let current_name = a.guilds.iter().find(|g| g.id == guild).map(|g| g.name.as_ref()).unwrap_or_default();
            if !confirmed(&typed, current_name) {
                super::set_flag(guild, "delete-error", true);
                cx.notify();
                return;
            }
            super::set_flag(guild, "delete-error", false);
            let call = AdminCall::Server(ServerCall::DeleteGuild { guild: sf(guild) });
            let request = if let Some(method) = method {
                let code = super::value(guild, "delete-mfa-code", cx).unwrap_or_default();
                let Some(code) = method.normalize(&code) else {
                    super::set_flag(guild, "delete-mfa-error", true);
                    cx.notify();
                    return;
                };
                if let Some(input) = super::input(guild, "delete-mfa-code", "", cx) {
                    input.update(cx, |state, cx| state.set_value("", window, cx));
                }
                ActionRequest::AdminMfa(call, method, Secret::new(code))
            } else {
                ActionRequest::Admin(call)
            };
            super::set_flag(guild, "delete-mfa-error", false);
            super::set_flag(guild, "delete-pending", true);
            a.action(request, cx, move |a, result, cx| {
                    super::set_flag(guild, "delete-pending", false);
                    match result {
                        Ok(ActionReply::Done | ActionReply::Json(_)) => {
                            if super::with(|s| s.modal == Some((guild, "delete"))) {
                                cancel(guild);
                            }
                            if crate::ui::modals::server_settings::with_state(|s| s.guild == guild) == Some(true) {
                                crate::actions::close_modal();
                            }
                            if a.guilds.iter().any(|g| g.id == guild) {
                                a.handle_event(crate::backend::AppEvent::GuildDelete(guild), cx);
                            }
                        }
                        Ok(ActionReply::AdminMfaRequired(methods)) => {
                            super::set_flag(guild, "delete-mfa-required", true);
                            super::set_flag(guild, "delete-mfa-authenticator", methods.contains(&AdminMfaMethod::Authenticator));
                            super::set_flag(guild, "delete-mfa-backup", methods.contains(&AdminMfaMethod::Backup));
                        }
                        Ok(ActionReply::AdminMfaRejected) => super::set_flag(guild, "delete-mfa-error", true),
                        Ok(_) => a.parity_toast(crate::t!("gs2.delete_failed"), cx),
                        Err(error) => a.parity_toast(error.0, cx),
                    }
                    cx.notify();
            });
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
