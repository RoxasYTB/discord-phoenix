use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

pub fn label(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, false, false)
}

pub fn label_with_custom(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, true, false)
}

pub fn status_label(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, true, true)
}

pub fn status_wrapped(text: SharedString, _font_px: f32) -> AnyElement {
    let ranges = crate::richtext::emoji_ranges(&text);
    if ranges.is_empty() {
        return text.into_any_element();
    }
    let zoom = crate::theme::zoom();
    let emoji_only = ranges.len() == 1
        && text[..ranges[0].0.start].trim().is_empty()
        && text[ranges[0].0.end..].trim().is_empty();
    if emoji_only {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .child(crate::media::emoji::emoji_element(&ranges[0].1, 32. * zoom))
            .into_any_element();
    }
    let mut row = div().flex().flex_wrap().items_center();
    let mut at = 0;
    let words = |row: gpui::Div, r: std::ops::Range<usize>| {
        crate::ui::chat::message_ext::words(&text, r)
            .into_iter()
            .fold(row, |row, w| row.child(div().flex_none().whitespace_nowrap().child(SharedString::from(text[w].to_string()))))
    };
    for (r, tok) in &ranges {
        row = words(row, at..r.start);
        row = row.child(div().flex_none().mr(gpui::px(4. * zoom)).child(crate::media::emoji::emoji_element(tok, 16. * zoom)));
        at = r.end;
    }
    words(row, at..text.len()).into_any_element()
}

fn label_impl(text: SharedString, font_px: f32, custom: bool, images: bool) -> AnyElement {
    let text: SharedString = if text.contains(['\n', '\r']) { text.replace(['\r', '\n'], " ").into() } else { text };
    let ranges: Vec<_> = crate::richtext::emoji_ranges(&text)
        .into_iter()
        .filter(|(r, _)| custom || !text[r.clone()].starts_with('<'))
        .filter(|(r, _)| images || text[r.clone()].starts_with('<') || emoji_presentation(&text[r.clone()]))
        .map(|(r, _)| r)
        .collect();
    if ranges.is_empty() || std::env::var_os("PHX_NO_EMOJI_FONT").is_some() {
        return text.into_any_element();
    }
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut at = 0;
    for r in ranges {
        if r.start > at {
            row = row.child(div().flex_none().child(SharedString::from(text[at..r.start].to_string())));
        }
        if images || text[r.clone()].starts_with('<') {
            row = row.child(div().flex_none().mx(gpui::px(1.)).child(crate::media::emoji::emoji_element(&text[r.clone()], font_px * 1.375)));
            at = r.end;
            continue;
        }
        let mut e = div().flex_none();
        e.text_style().font_fallbacks =
            Some(gpui::FontFallbacks::from_fonts(vec![crate::fonts::fallback::EMOJI_FAMILY.to_string()]));
        row = row.child(e.child(SharedString::from(text[r.clone()].to_string())));
        at = r.end;
    }
    if at < text.len() {
        row = row.child(div().min_w_0().text_ellipsis().child(SharedString::from(text[at..].to_string())));
    }
    row.into_any_element()
}

fn emoji_presentation(seq: &str) -> bool {
    let mut cs = seq.chars();
    let Some(first) = cs.next() else { return false };
    if seq.contains('\u{FE0F}') || cs.next().is_some() {
        return true;
    }
    let c = first as u32;
    c >= 0x1F000
        || matches!(
            c,
            0x231A..=0x231B | 0x23E9..=0x23EC | 0x23F0 | 0x23F3 | 0x25FD..=0x25FE | 0x2614..=0x2615
                | 0x2648..=0x2653 | 0x267F | 0x2693 | 0x26A1 | 0x26AA..=0x26AB | 0x26BD..=0x26BE
                | 0x26C4..=0x26C5 | 0x26CE | 0x26D4 | 0x26EA | 0x26F2..=0x26F3 | 0x26F5 | 0x26FA
                | 0x26FD | 0x2705 | 0x270A..=0x270B | 0x2728 | 0x274C | 0x274E | 0x2753..=0x2755
                | 0x2757 | 0x2795..=0x2797 | 0x27B0 | 0x27BF | 0x2B1B..=0x2B1C | 0x2B50 | 0x2B55
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_emoji_in_channel_names() {
        let t = "〢❓・aide-membres";
        let r = crate::richtext::emoji_ranges(t);
        assert_eq!(r.len(), 1);
        assert_eq!(&t[r[0].0.clone()], "❓");
        assert!(crate::richtext::emoji_ranges("〢・general").is_empty());
        assert_eq!(crate::richtext::emoji_ranges("ℹ\u{FE0F} Informations").len(), 1);
        assert!(super::emoji_presentation("❓") && super::emoji_presentation("ℹ\u{FE0F}") && super::emoji_presentation("🌸"));
        assert!(!super::emoji_presentation("☲") && !super::emoji_presentation("♡"));
    }
}
