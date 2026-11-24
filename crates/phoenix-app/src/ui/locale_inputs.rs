use std::{cell::RefCell, collections::HashMap};

use gpui::{App, Entity, EntityId, Window};
use gpui_component::input::InputState;

thread_local! {
    static APPLIED: RefCell<HashMap<EntityId, &'static str>> = RefCell::default();
}

pub fn keep_placeholder(input: &Entity<InputState>, key: &str, window: &mut Window, cx: &mut App) {
    let locale = crate::i18n::locale();
    let stale = APPLIED.with_borrow_mut(|m| m.insert(input.entity_id(), locale) != Some(locale));
    if stale {
        let text = crate::i18n::t(key, &[]);
        input.update(cx, |s, cx| s.set_placeholder(text, window, cx));
    }
}
