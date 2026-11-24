//! Locked thread banner over the message list (client ChannelChatNotice):
//! archived + locked → "closed and locked" (rEeodK), locked → "locked" (V/JF2N).
//! The Open / Unlock buttons need MANAGE_THREADS and write actions: not shown.

use gpui::{IntoElement, ParentElement, Styled, div};

use crate::app::PhoenixApp;
use crate::theme::{generated::idx, z};

pub fn render(app: &PhoenixApp) -> Option<gpui::AnyElement> {
    let ch = app.selected_channel?;
    let x = app.nav.extras.get(&ch)?;
    if !x.thread_locked {
        return None;
    }
    let text = if x.thread_archived { crate::t!("chat.thread_closed_locked") } else { crate::t!("chat.thread_locked") };
    // chatHeaderBar_By_-nl: flex, center, space-between, gap 16px, padding 8px 16px,
    // --background-mod-normal; chatHeaderBarText_ flex 1, body-sm/normal (14px / 18px, 400), text-default.
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
            .into_any_element(),
    )
}
