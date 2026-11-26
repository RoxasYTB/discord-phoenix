use gpui::{Context, IntoElement, ParentElement, Styled, div, prelude::*};

use crate::app::PhoenixApp;
use crate::theme::{generated::idx, z};
use crate::ui::controls::{BtnKind, button_sm};

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    let ch = app.selected_channel?;
    let x = app.nav.extras.get(&ch)?;
    if !x.thread_locked && !x.thread_archived {
        return None;
    }
    let text = if x.thread_locked && x.thread_archived {
        crate::t!("chat.thread_closed_locked")
    } else if x.thread_locked {
        crate::t!("chat.thread_locked")
    } else {
        crate::t!("chat.thread_archived")
    };
    let action = x.can_manage_threads.then(|| {
        let label = if x.thread_locked { crate::t!("chat.thread_unlock") } else { crate::t!("chat.thread_open") };
        let channel = ch.0;
        let body = banner_patch(x.thread_locked);
        button_sm("thread-banner-action", label, BtnKind::Secondary).on_click(cx.listener(move |app, _, _, cx| {
            app.blocked_write(crate::actions::guard::GuardedWrite::edit_channel(channel, body.clone()), cx);
        }))
    });
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(z(16.))
            .py(z(8.))
            .px(z(16.))
            .bg(super::tk(idx::BACKGROUND_MOD_NORMAL))
            .child(
                div()
                    .flex_1()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(gpui::FontWeight::NORMAL)
                    .text_color(super::tk(idx::TEXT_DEFAULT))
                    .child(text),
            )
            .children(action)
            .into_any_element(),
    )
}

pub fn banner_patch(locked: bool) -> serde_json::Value {
    if locked { serde_json::json!({ "locked": false, "archived": false }) } else { serde_json::json!({ "archived": false }) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn banner_cta_matches_unarchive_and_unlock() {
        assert_eq!(super::banner_patch(false), serde_json::json!({ "archived": false }));
        assert_eq!(super::banner_patch(true), serde_json::json!({ "locked": false, "archived": false }));
    }
}
