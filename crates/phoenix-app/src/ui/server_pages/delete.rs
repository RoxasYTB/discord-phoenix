use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::ServerCall;

use super::{page, sf, t, text, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button},
};

pub fn confirmed(typed: &str, name: &str) -> bool {
    !name.is_empty() && typed.trim() == name.trim()
}

pub fn render(_app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if super::with(|s| s.modal.is_none()) && !super::flag(guild, "delete-shown").unwrap_or(false) {
        super::set_flag(guild, "delete-shown", true);
        super::open_modal(guild, "delete");
    }
    page()
        .child(super::title(crate::t!("gs2.delete_server")))
        .child(div().mt(z(24.)).flex().child(button("gs2-del-open", crate::t!("gs2.delete_server"), BtnKind::Critical).on_click(cx.listener(
            move |_, _, _, cx| {
                super::open_modal(guild, "delete");
                cx.notify();
            },
        ))))
        .into_any_element()
}

pub fn modal(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.to_string()).unwrap_or_default();
    let error = super::flag(guild, "delete-error").unwrap_or(false);
    let e = cx.entity();
    let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
        super::close_modal();
        super::forget_input(guild, "delete-name");
        super::with(|s| {
            s.flags.remove(&(guild, "delete-error".to_owned()));
        });
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
        .child(div().mt(z(20.)).child(text(
            12.,
            16.,
            FontWeight::BOLD,
            if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE },
            crate::t!("gs2.delete_enter_name").to_uppercase(),
        )))
        .children(error.then(|| text(12., 16., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, crate::t!("gs2.delete_wrong"))))
        .child(super::field(guild, "delete-name", "", cx).mt(z(8.)).when(error, |d| d.border_color(t(idx::BORDER_FEEDBACK_CRITICAL))));
    let close2 = close.clone();
    let n2 = name.clone();
    let footer =
        div().flex().gap(z(8.)).child(button("gs2-del-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(move |_, w, cx| close2(w, cx))).child(
            button("gs2-del-ok", crate::t!("gs2.delete_server"), BtnKind::Critical).on_click(cx.listener(move |a, _, _, cx| {
                let typed = super::value(guild, "delete-name", cx).unwrap_or_default();
                if confirmed(&typed, &n2) {
                    super::close_modal();
                    super::forget_input(guild, "delete-name");
                    write(a, ServerCall::DeleteGuild { guild: sf(guild) }, None, cx);
                } else {
                    super::set_flag(guild, "delete-error", true);
                }
                cx.notify();
            })),
        );
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
