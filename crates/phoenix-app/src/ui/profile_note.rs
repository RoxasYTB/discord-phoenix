use std::cell::RefCell;

use gpui::{AnyElement, Context, Entity, Focusable, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};

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

struct ModalEditor {
    user: Id,
    input: Entity<TextareaState>,
    _sub: gpui::Subscription,
}

thread_local! {
    static EDIT: RefCell<Option<Editor>> = const { RefCell::new(None) };
    static MODAL_EDIT: RefCell<Option<ModalEditor>> = const { RefCell::new(None) };
}

pub const MODAL_NOTE_ROWS: usize = 10;

fn modal_editing(uid: Id) -> Option<Entity<TextareaState>> {
    MODAL_EDIT.with(|e| e.borrow().as_ref().filter(|e| e.user == uid).map(|e| e.input.clone()))
}

pub fn close_modal() {
    MODAL_EDIT.with(|e| e.borrow_mut().take());
}

pub fn editing(uid: Id) -> Option<Entity<InputState>> {
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

    pub fn start_modal_note_edit(&mut self, uid: Id, window: &mut Window, cx: &mut Context<Self>) {
        if modal_editing(uid).is_some() {
            return;
        }
        let current = self.people.profiles.get(&uid).map(|p| p.note.to_string()).unwrap_or_default();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, MODAL_NOTE_ROWS)
                .submit_on_enter(true)
                .placeholder(crate::t!("profile.note_placeholder"))
                .default_value(current)
        });
        let sub = cx.subscribe_in(&input, window, move |this, input, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Blur | InputEvent::PressEnter { shift: false, .. }) {
                let text = input.read(cx).value().trim().to_string();
                this.save_note(uid, text, cx);
                close_modal();
                cx.notify();
            }
        });
        input.update(cx, |s, cx| s.focus(window, cx));
        MODAL_EDIT.with(|e| *e.borrow_mut() = Some(ModalEditor { user: uid, input, _sub: sub }));
        cx.notify();
    }

    pub fn save_note(&mut self, uid: Id, text: String, cx: &mut Context<Self>) {
        let Some(p) = self.people.profiles.get_mut(&uid) else { return };
        if p.note.as_ref() == text {
            return;
        }
        let previous = std::mem::replace(&mut p.note, text.clone().into());
        let fut = self.backend.set_note(uid, text);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |a, cx| {
                if res.is_err() {
                    if let Some(p) = a.people.profiles.get_mut(&uid) {
                        p.note = previous;
                    }
                    a.parity_toast(crate::t!("acct.blocked"), cx);
                    cx.notify();
                }
            });
        })
        .detach();
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Popout,
    Sidebar,
    FullProfile,
}

pub fn shown(surface: Surface, own: bool, editing: bool, streamer_hides: bool) -> bool {
    if streamer_hides {
        return false;
    }
    match surface {
        Surface::FullProfile => true,
        Surface::Popout | Surface::Sidebar => !own && editing,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NoteLook {
    Modal,
    Bot,
}

pub fn modal_note(note: SharedString, uid: Id, look: NoteLook, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let editing = modal_editing(uid);
    let focused = editing.is_some();
    let base = div()
        .id(("profile-modal-note", uid.0))
        .w_full()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .cursor_text();
    let field = match look {
        NoteLook::Modal => base
            .rounded(z(8.))
            .when(!focused, |d| {
                d.py(z(4.))
                    .px(z(6.))
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .hover(|s| s.bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT))).border_color(c(tok(idx::BORDER_STRONG))))
            })
            .when(focused, |d| {
                d.py(z(3.)).px(z(5.)).border_2().border_color(c(tok(idx::INPUT_BORDER_ACTIVE))).bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
            }),
        NoteLook::Bot => base
            .p(z(4.))
            .min_h(z(46.))
            .rounded(z(3.))
            .when(focused, |d| d.bg(c(tok(idx::USER_PROFILE_NOTE_BACKGROUND_FOCUS)))),
    };
    let field = match editing {
        Some(input) => field.child(Textarea::new(&input).appearance(false).bordered(false)),
        None => field
            .on_click(cx.listener(move |a, _, window, cx| {
                cx.stop_propagation();
                a.start_modal_note_edit(uid, window, cx)
            }))
            .map(|d| {
                if note.is_empty() {
                    d.text_color(c(tok(idx::TEXT_MUTED))).italic().child(crate::t!("profile.note_placeholder"))
                } else {
                    d.child(note)
                }
            }),
    };
    match look {
        NoteLook::Modal => div().mt(z(-4.)).mx(z(-7.)).child(field).into_any_element(),
        NoteLook::Bot => div().mx(z(-4.)).child(field).into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_a_note_on_your_own_popout_or_sidebar() {
        for s in [Surface::Popout, Surface::Sidebar] {
            assert!(!shown(s, true, false, false));
            assert!(!shown(s, true, true, false));
            assert!(!shown(s, false, false, false));
            assert!(shown(s, false, true, false));
            assert!(!shown(s, false, true, true));
        }
    }

    #[test]
    fn full_profile_note_follows_the_client_and_streamer_mode() {
        assert!(shown(Surface::FullProfile, false, false, false));
        assert!(shown(Surface::FullProfile, true, false, false));
        assert!(!shown(Surface::FullProfile, false, false, true));
        assert!(!shown(Surface::FullProfile, true, false, true));
    }

    #[test]
    fn note_writes_go_through_the_guard() {
        let w = crate::actions::guard::GuardedWrite::set_note(42, "hello");
        assert_eq!(w.method, reqwest::Method::PUT);
        assert_eq!(w.path, "/users/@me/notes/42");
        assert_eq!(w.body, Some(serde_json::json!({ "note": "hello" })));
        assert!(w.refusal().is_write_blocked());
    }

    #[test]
    fn modal_note_is_not_the_popout_editor() {
        assert!(editing(Id(7)).is_none());
        assert!(modal_editing(Id(7)).is_none());
    }
}
