//! Centered modal dialog over a dimmed backdrop (Discord's confirm dialogs:
//! 776412 `container_dvN-MP` layout: header, body, action bar of Mana buttons).

use crate::theme::z;
use std::rc::Rc;

use gpui::{
    AnyElement, App, FontWeight, IntoElement, MouseButton, ParentElement, SharedString, Styled,
    Window, deferred, div, prelude::*, px,
};

use std::time::Duration;

use gpui::{Animation, AnimationExt, ease_out_quint};

use crate::theme::{c, generated::idx, tok};

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct Confirm {
    pub title: SharedString,
    pub body: Vec<AnyElement>,
    pub confirm_label: SharedString,
    pub danger: bool,
    pub on_confirm: Action,
    pub on_cancel: Action,
}

/// Generic frame: backdrop + 440px card. Clicking the backdrop cancels.
pub fn frame(id: &'static str, on_cancel: Action, card: impl IntoElement) -> AnyElement {
    deferred(
        div()
            .id(id)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(c(tok(idx::BACKGROUND_SCRIM))) // --background-scrim
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, move |_, w, cx| on_cancel(w, cx))
            .child(
                crate::ui::focus_ring::trap_modal(
                    div()
                        .id("modal-card")
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(card)
                // Discord: card fades in and grows from ~0.9 over ~200ms.
                .with_animation(
                    id,
                    Animation::new(crate::ui::settings::effects::motion(200)).with_easing(ease_out_quint()),
                    |d, t| d.opacity(t).mt(px(24. * (1. - t))),
                ),
            )
            .with_animation(
                "modal-backdrop-fade",
                Animation::new(Duration::from_millis(150)),
                |d, t| d.bg(c(tok(idx::BACKGROUND_SCRIM)).opacity(t)),
            ),
    )
    .with_priority(20)
    .into_any_element()
}

pub fn confirm(m: Confirm) -> AnyElement {
    use crate::ui::controls::{BtnKind, button};
    let cancel = m.on_cancel.clone();
    let cancel2 = m.on_cancel.clone();
    let ok = m.on_confirm.clone();
    // Live dumps (delete-message confirm, leave-server alertdialog, 2026-11-23):
    // the first body element is the header subtitle (headerSubtitleWrapper_,
    // body-md/normal 16/20 --text-subtle, 8px under the title); anything after
    // it goes into body_ between bodySpacerTop (16) and bodySpacerBottom (8).
    let mut body = m.body.into_iter();
    let subtitle = body.next();
    let rest: Vec<AnyElement> = body.collect();
    let has_body = !rest.is_empty();
    // container_dvN-MP: --background-surface-high, 1px --border-subtle, radius 12,
    // --shadow-high; padding 8 0 16. size-sm 400 (no body) / size-md 480.
    let card = div()
        .relative()
        .w(z(if has_body { 480. } else { 400. }))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        // --shadow-high: 0 12px 24px 0 rgb(0 0 0 / .24) (vanilla-confirm-leave #2068)
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        // header.section_: padding 16 24 8; stack gap 8. headerMain leaves room
        // for headerTrailing (12 + 40 - 16 = 36).
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(
                    div()
                        .pr(z(36.))
                        .text_size(z(20.)) // heading-lg/semibold
                        .line_height(z(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(m.title),
                )
                .children(subtitle.map(|s| {
                    div()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(c(tok(idx::TEXT_SUBTLE)))
                        .child(s)
                })),
        )
        // bodySpacerTop 16 / bodyInner padding 0 8 0 24 / bodySpacerBottom 8
        .when(has_body, |d| {
            d.child(
                div()
                    .pt(z(16.))
                    .pb(z(8.))
                    .pl(z(24.))
                    .pr(z(24.))
                    .flex()
                    .flex_col()
                    .gap(z(20.)) // spacingTop_ margin-top 20
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .children(rest),
            )
        })
        // headerTrailing: margin-top -16 / margin-right -16 against the header's
        // 16/24 padding -> the 40px md icon-only close button sits 8px from the
        // card's top and right edges. Icon 20px --control-icon-only-icon-default.
        .child(
            crate::ui::focus_ring::ring(
                div()
                    .id("modal-close")
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .size(z(40.))
                    .rounded(z(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                    .on_click({
                        let cancel = m.on_cancel.clone();
                        move |_, w, cx| cancel(w, cx)
                    })
                    .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
                crate::ui::states::key("modal-close"),
                8.,
            ),
        )
        // footer.actionBar_: padding 16 24 8; actionBarTrailing stack gap 8 with
        // both md buttons stretched to equal widths.
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .gap(z(8.))
                .child(
                    crate::ui::focus_ring::ring(
                        button("modal-cancel", crate::t!("common.cancel"), BtnKind::Secondary)
                            .relative()
                            .flex_1()
                            .on_click(move |_, w, cx| cancel2(w, cx)),
                        crate::ui::states::key("modal-cancel"),
                        8.,
                    ),
                )
                .child(
                    crate::ui::focus_ring::ring(
                        button("modal-confirm", m.confirm_label, if m.danger { BtnKind::Critical } else { BtnKind::Primary })
                            .relative()
                            .flex_1()
                            .on_click(move |_, w, cx| ok(w, cx)),
                        crate::ui::states::key("modal-confirm"),
                        8.,
                    ),
                ),
        );
    frame("modal-backdrop", cancel, card)
}
