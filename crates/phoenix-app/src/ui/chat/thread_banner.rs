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
        let stub_label = label.clone();
        button_sm("thread-banner-action", label, BtnKind::Secondary).on_click(cx.listener(move |app, _, _, cx| {
            app.stub(&stub_label, cx);
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
