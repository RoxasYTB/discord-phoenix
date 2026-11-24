use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{AnimationExt, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

const AVATAR: f32 = 80.;
const PULSE_MS: u64 = 1352;

fn ringing_avatar(app: &PhoenixApp, user: crate::model::Id) -> AnyElement {
    let size = AVATAR;
    div()
        .relative()
        .size(z(size))
        .flex_none()
        .child(crate::ui::avatar(app.users.get(&user), size * crate::theme::zoom()))
        .child(
            div()
                .id(("dm-call-pulse", user.0))
                .absolute()
                .top(z(-3.))
                .left(z(-3.))
                .size(z(size + 6.))
                .rounded_full()
                .border(z(3.))
                .border_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
                .with_animation(
                    ("dm-call-pulse-anim", user.0),
                    gpui::Animation::new(Duration::from_millis(PULSE_MS)).repeat(),
                    move |d, t| {
                        let grow = (size + 6.) * 0.1 * t;
                        d.opacity(1. - t)
                            .top(z(-3. - grow / 2.))
                            .left(z(-3. - grow / 2.))
                            .size(z(size + 6. + grow))
                    },
                ),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let ch = app.selected_channel?;
    if app.parity.incoming_call != Some(ch) {
        return None;
    }
    let channel = app.channels.get(&ch)?;
    if channel.guild.is_some() {
        return None;
    }
    let users: Vec<crate::model::Id> = channel.voice_users.iter().copied().take(6).collect();
    let button = |id: &'static str, icon: &'static str, bg: usize, hover: usize, tip: gpui::SharedString| {
        div()
            .id(id)
            .size(z(56.))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(c(tok(bg)))
            .hover(move |s| s.bg(c(tok(hover))))
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tip(tip))
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).text_color(c(tok(idx::WHITE))))
    };
    Some(
        div()
            .id("dm-call-header")
            .w_full()
            .flex_none()
            .h(z(240.))
            .bg(c(tok(idx::BLACK)))
            .when(crate::theme::frame_borders(), |d| d.border_t_1().border_color(c(tok(idx::APP_FRAME_BORDER))))
            .overflow_hidden()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(z(24.))
            .child(div().flex().gap(z(16.)).children(users.into_iter().map(|u| ringing_avatar(app, u))))
            .child(
                div()
                    .flex()
                    .gap(z(16.))
                    .child(
                        button(
                            "dm-call-join",
                            "phone-call",
                            idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT,
                            idx::CONTROL_CONNECTED_BACKGROUND_HOVER,
                            crate::t!("call.join_voice"),
                        )
                        .on_click(cx.listener(|a, _, _, cx| crate::ui::incoming_call::answer(a, cx))),
                    )
                    .child(
                        button(
                            "dm-call-decline",
                            "close",
                            idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT,
                            idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER,
                            crate::t!("call.decline"),
                        )
                        .on_click(cx.listener(|a, _, _, cx| crate::ui::incoming_call::decline(a, cx))),
                    ),
            )
            .into_any_element(),
    )
}
