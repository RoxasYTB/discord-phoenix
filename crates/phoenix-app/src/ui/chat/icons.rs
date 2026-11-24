use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatIcon {
    Edit,
    Trash,
    More,
    AddReaction,
    Copy,
    IdBadge,
    Unread,
    Link,
    Thread,
    Eye,
    Upload,
    Join,
    Boost,
    ArrowRight,
    Retry,
}

impl ChatIcon {
    pub fn path(self) -> SharedString {
        SharedString::new_static(match self {
            ChatIcon::Edit => "icons/chat/edit.svg",
            ChatIcon::Trash => "icons/chat/trash.svg",
            ChatIcon::More => "icons/chat/more.svg",
            ChatIcon::AddReaction => "icons/chat/add-reaction.svg",
            ChatIcon::Copy => "icons/chat/copy.svg",
            ChatIcon::IdBadge => "icons/chat/id.svg",
            ChatIcon::Unread => "icons/chat/unread.svg",
            ChatIcon::Link => "icons/chat/link.svg",
            ChatIcon::Thread => "icons/chat/thread.svg",
            ChatIcon::Eye => "icons/chat/eye.svg",
            ChatIcon::Upload => "icons/chat/upload.svg",
            ChatIcon::Join => "icons/chat/join.svg",
            ChatIcon::Boost => "icons/chat/boost.svg",
            ChatIcon::ArrowRight => "icons/chat/arrow-right.svg",
            ChatIcon::Retry => "icons/chat/retry.svg",
        })
    }
}

macro_rules! s {
    ($body:expr) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            $body,
            "</svg>"
        )
    };
}
macro_rules! f {
    ($body:expr) => {
        concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#, $body, "</svg>")
    };
}

pub fn svg(path: &str) -> Option<&'static str> {
    Some(match path {
        "icons/chat/edit.svg" => f!(r#"<path d="M16.6 2.6a2 2 0 0 1 2.8 0l2 2a2 2 0 0 1 0 2.8L9 19.8 3 21l1.2-6zM15 6.4 5.9 15.5l-.6 3.2 3.2-.6L17.6 9z"/>"#),
        "icons/chat/trash.svg" => f!(r#"<path d="M9 2h6a1 1 0 0 1 1 1v1h4a1 1 0 1 1 0 2H4a1 1 0 0 1 0-2h4V3a1 1 0 0 1 1-1zM5 8h14l-1 13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1zm4 3v8h2v-8zm4 0v8h2v-8z"/>"#),
        "icons/chat/more.svg" => f!(r#"<circle cx="5" cy="12" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="19" cy="12" r="2"/>"#),
        "icons/chat/add-reaction.svg" => f!(r#"<path fill-rule="evenodd" d="M11 2.05V4a8 8 0 1 0 9 9h2A10 10 0 1 1 11 2.05zM8.5 9a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm7 0a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM7 14h10a5 5 0 0 1-10 0zM19 1h2v3h3v2h-3v3h-2V6h-3V4h3z"/>"#),
        "icons/chat/copy.svg" => s!(r#"<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 4H5a2 2 0 0 0-2 2v11"/>"#),
        "icons/chat/id.svg" => s!(r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 9v6M11 9v6h2a3 3 0 0 0 0-6z"/>"#),
        "icons/chat/unread.svg" => f!(r#"<path d="M4 4h12v2H4v12h12v-4h2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"/><circle cx="19" cy="6" r="4"/>"#),
        "icons/chat/link.svg" => s!(r#"<path d="M10 14a4 4 0 0 0 5.7 0l3.5-3.5a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3.5 3.5a4 4 0 0 0 5.7 5.7l1-1"/>"#),
        "icons/chat/thread.svg" => s!(r#"<path d="M4 5h12v8H9l-3 2.5V13H4zM19 9h1v8h-2v2.5L15 17h-4"/>"#),
        "icons/chat/eye.svg" => s!(r#"<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#),
        "icons/chat/upload.svg" => s!(r#"<path d="M12 16V4m-5 5 5-5 5 5M4 20h16"/>"#),
        "icons/chat/join.svg" => s!(r#"<path d="M4 12h14m-5-5 5 5-5 5"/>"#),
        "icons/chat/boost.svg" => f!(r#"<path d="M12 2 19 8v8l-7 6-7-6V8zm0 4L8 9.5v5L12 18l4-3.5v-5z"/>"#),
        "icons/chat/arrow-right.svg" => s!(r#"<path d="m9 6 6 6-6 6"/>"#),
        "icons/chat/retry.svg" => s!(r#"<path d="M20 12a8 8 0 1 1-2.6-5.9"/><path d="M20 4v5h-5"/>"#),
        _ => return None,
    })
}

pub fn icon(i: ChatIcon, size: f32, color: gpui::Hsla) -> gpui::Svg {
    use gpui::Styled;
    gpui::svg().path(i.path()).size(gpui::px(size)).flex_none().text_color(color)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_chat_icon_has_svg() {
        use ChatIcon::*;
        for i in [Edit, Trash, More, AddReaction, Copy, IdBadge, Unread, Link, Thread, Eye, Upload, Join, Boost, ArrowRight, Retry] {
            assert!(svg(&i.path()).is_some(), "{i:?}");
        }
    }
}
