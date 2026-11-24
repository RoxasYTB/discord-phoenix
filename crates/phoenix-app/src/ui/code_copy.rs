use std::time::Duration;

use gpui::{App, ClipboardItem, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, prelude::*, svg};

use crate::theme::z;

pub const GROUP: &str = "phx-code-block";

#[derive(IntoElement)]
pub struct CodeCopy {
    text: SharedString,
}

impl CodeCopy {
    pub fn new(text: SharedString) -> Self {
        Self { text }
    }
}

impl RenderOnce for CodeCopy {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        let key = ("code-copy", h.finish() as usize);
        let copied = window.use_keyed_state(key, cx, |_, _| false);
        let icon = if *copied.read(cx) { "check" } else { "chat/copy" };
        let text = self.text;
        div()
            .id(key)
            .absolute()
            .top(z(8.))
            .right(z(4.))
            .invisible()
            .group_hover(GROUP, |s| s.visible())
            .cursor_pointer()
            .child(svg().path(crate::icons::path(icon)).size(z(16.)).text_color(crate::ui::chat::tk(crate::theme::generated::idx::TEXT_CODE)))
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
                copied.update(cx, |c, cx| {
                    *c = true;
                    cx.notify();
                });
                let copied = copied.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                    cx.update(|cx| {
                        copied.update(cx, |c, cx| {
                            *c = false;
                            cx.notify();
                        })
                    });
                })
                .detach();
            })
    }
}
