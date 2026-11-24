use std::cell::RefCell;

use gpui::{AnyElement, Context, Entity, Focusable, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

pub const NAME_ROW_GROUP: &str = "profile-name-row";

struct Editor {
    user: Id,
    input: Entity<InputState>,
    _sub: gpui::Subscription,
}

thread_local! {
    static EDIT: RefCell<Option<Editor>> = const { RefCell::new(None) };
}

fn editing(uid: Id) -> Option<Entity<InputState>> {
    EDIT.with(|e| e.borrow().as_ref().filter(|e| e.user == uid).map(|e| e.input.clone()))
}

pub fn close() {
    EDIT.with(|e| e.borrow_mut().take());
}

impl PhoenixApp {
    pub fn start_note_edit(&mut self, uid: Id, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.people.profiles.get(&uid).map(|p| p.note.to_string()).unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::t!("profile.note_placeholder")).default_value(current)
        });
        let sub = cx.subscribe_in(&input, window, move |this, input, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Blur | InputEvent::PressEnter { shift: false, .. }) {
                let text = input.read(cx).value().trim().to_string();
                this.save_note(uid, text, cx);
                close();
                cx.notify();
            }
        });
        let fh = input.read(cx).focus_handle(cx);
        fh.focus(window, cx);
        EDIT.with(|e| *e.borrow_mut() = Some(Editor { user: uid, input, _sub: sub }));
        cx.notify();
    }

    pub fn save_note(&mut self, uid: Id, text: String, cx: &mut Context<Self>) {
        let Some(p) = self.people.profiles.get_mut(&uid) else { return };
        if p.note.as_ref() == text {
            return;
        }
        p.note = text.clone().into();
        cx.background_spawn(self.backend.set_note(uid, text)).detach();
    }
}

pub fn note_button(uid: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let shown = editing(uid).is_some();
    div()
        .id(("profile-note-btn", uid.0))
        .size(z(24.))
        .p(z(4.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(if shown { 1. } else { 0. })
        .group_hover(NAME_ROW_GROUP, |s| s.opacity(1.))
        .text_color(c(tok(idx::ICON_SUBTLE)))
        .hover(|s| s.text_color(c(tok(idx::ICON_STRONG))))
        .on_click(cx.listener(move |a, _, window, cx| {
            cx.stop_propagation();
            a.start_note_edit(uid, window, cx)
        }))
        .child(gpui::svg().path(crate::icons::path("note")).size(z(16.)).text_color(c(tok(idx::ICON_SUBTLE))))
        .into_any_element()
}

pub fn note_section(note: Option<SharedString>, uid: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if let Some(input) = editing(uid) {
        return Some(
            div()
                .m(z(-4.))
                .p(z(4.))
                .rounded(z(3.))
                .bg(c(tok(idx::USER_PROFILE_NOTE_BACKGROUND_FOCUS)))
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(Input::new(&input).appearance(false).bordered(false))
                .into_any_element(),
        );
    }
    let note = note.filter(|n| !n.is_empty())?;
    Some(
        div()
            .id(("profile-note", uid.0))
            .m(z(-4.))
            .p(z(4.))
            .rounded(z(3.))
            .cursor_text()
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(c(tok(idx::TEXT_DEFAULT)))
            .on_click(cx.listener(move |a, _, window, cx| a.start_note_edit(uid, window, cx)))
            .child(note)
            .into_any_element(),
    )
}
