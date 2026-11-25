use std::cell::RefCell;

use gpui::{
    Anchor, AnchoredPositionMode, AnyElement, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, anchored, deferred, div, point, prelude::FluentBuilder,
};

use crate::{
    model::Attachment,
    theme::{c, ca, generated::idx, tok, z},
};

thread_local! {
    static OPEN: RefCell<Option<SharedString>> = const { RefCell::new(None) };
}

pub fn description(a: &Attachment, hidden_spoiler: bool) -> Option<SharedString> {
    if hidden_spoiler || !crate::ui::settings::effects::alt_text() {
        return None;
    }
    a.description.clone().filter(|d| !d.is_empty())
}

pub fn is_open(key: &str) -> bool {
    OPEN.with_borrow(|o| o.as_deref() == Some(key))
}

pub fn close() -> bool {
    OPEN.with_borrow_mut(|o| o.take().is_some())
}

pub fn badge(key: SharedString, text: SharedString) -> AnyElement {
    let open = is_open(&key);
    div()
        .absolute()
        .bottom(z(4.))
        .left(z(4.5))
        .child(button(key.clone()))
        .when(open, |d| d.child(div().absolute().top_0().left(gpui::relative(1.)).child(popout(key, text))))
        .into_any_element()
}

pub fn video_badge(key: SharedString, text: SharedString, controls_group: SharedString) -> AnyElement {
    let open = is_open(&key);
    div()
        .absolute()
        .top(z(4.))
        .left(z(4.5))
        .child(
            div()
                .absolute()
                .top_0()
                .left(z(4.5))
                .when(!open, |d| d.opacity(0.).group_hover(controls_group, |s| s.opacity(1.)))
                .child(button(key.clone()))
                .when(open, |d| d.child(div().absolute().top_0().left(gpui::relative(1.)).child(popout(key, text)))),
        )
        .into_any_element()
}

fn button(key: SharedString) -> gpui::Stateful<gpui::Div> {
    let k = key.clone();
    div()
        .id(ElementId::Name(format!("alt-{key}").into()))
        .occlude()
        .rounded(z(4.))
        .px(z(8.))
        .py(z(7.))
        .bg(ca(0x00000099))
        .hover(|s| s.bg(ca(0x000000CC)))
        .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .text_color(gpui::white())
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .child(crate::t!("chat.alt_badge"))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            OPEN.with_borrow_mut(|o| *o = if o.as_ref() == Some(&k) { None } else { Some(k.clone()) });
            window.refresh();
        })
}

fn popout(key: SharedString, text: SharedString) -> AnyElement {
    let card = div()
        .id(ElementId::Name(format!("alt-pop-{key}").into()))
        .occlude()
        .max_w(z(360.))
        .flex()
        .flex_col()
        .px(z(12.))
        .py(z(13.5))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .shadow(crate::ui::inbox::popout_shadow())
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(div().mb(z(9.5)).text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).child(crate::t!("chat.alt_title")))
        .child(div().text_size(z(16.)).line_height(z(20.)).child(text))
        .on_mouse_down_out(move |_, window, _| {
            if close() {
                window.refresh();
            }
        });
    deferred(
        anchored()
            .position_mode(AnchoredPositionMode::Local)
            .anchor(Anchor::TopLeft)
            .position(point(z(8.), z(0.)))
            .snap_to_window_with_margin(z(8.))
            .child(card),
    )
    .with_priority(2)
    .into_any_element()
}
