//! Unicode emoji inside plain UI labels (channel / category / thread names).
//!
//! Live client (vanilla, 2026-11-24): channel names are plain text in
//! `"gg sans","Noto Sans",...`, and Chromium draws emoji-presentation characters
//! with the system color emoji font (`fc-match emoji` = Noto Color Emoji), e.g. the
//! red "❓" in "〢❓・aide-membres". GPUI's per-glyph fallback only checks coverage,
//! so "❓" came from a monochrome symbol font and some emoji (📞) were missing.
//! Here the emoji sequences found by `richtext::emoji_ranges` get the system color
//! emoji font (`fonts::fallback::EMOJI_FAMILY`) with no UI fallback chain;
//! everything else stays inherited text.

use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

/// `text` (drawn at `font_px`) with its Unicode emoji as images.
pub fn label(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, false, false)
}

/// [`label`] that also draws custom `<:name:id>` emoji as images (channel topics).
pub fn label_with_custom(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, true, false)
}

/// Custom statuses and activity lines: every emoji, custom or Unicode, is an
/// image (live client 2026-11-24: `customStatusContent_` and the member
/// activity text hold `img.emoji` 14px Twemoji, not font glyphs, while names
/// and channel names stay text).
pub fn status_label(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, true, true)
}

/// Wrapping version of [`status_label`] for the profile status bubble (two
/// lines): words and emoji images flow in a wrapping row.
///
/// Official CSS (`rmCN9_`):
/// - `.statusEmoji_rmCN9_:not(:has(~.statusText_rmCN9_))` (emoji-only): 32×32 px
/// - `.statusEmoji_rmCN9_:has(~.statusText_rmCN9_)` (emoji + text): 16×16 px, `margin-inline-end: 4px`
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
    // HTML collapses newlines in these one-line labels; a bot's multi-line custom status
    // otherwise stacked three lines over the member name.
    let text: SharedString = if text.contains(['\n', '\r']) { text.replace(['\r', '\n'], " ").into() } else { text };
    let ranges: Vec<_> = crate::richtext::emoji_ranges(&text)
        .into_iter()
        // custom `<:name:id>` tokens stay text unless asked for
        .filter(|(r, _)| custom || !text[r.clone()].starts_with('<'))
        // Chromium only switches to the emoji font for emoji presentation: "☲" (U+2632)
        // in role names stays a text glyph, "❓" or "ℹ️" do not.
        .filter(|(r, _)| images || text[r.clone()].starts_with('<') || emoji_presentation(&text[r.clone()]))
        .map(|(r, _)| r)
        .collect();
    // PHX_NO_EMOJI_FONT=1: old behaviour, for before/after renders.
    if ranges.is_empty() || std::env::var_os("PHX_NO_EMOJI_FONT").is_some() {
        return text.into_any_element();
    }
    // Labels are one line: without nowrap the pieces before an emoji wrapped inside the
    // row and spilled over the next row (member custom status "/help · site ✨ · docs").
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut at = 0;
    for r in ranges {
        if r.start > at {
            row = row.child(div().flex_none().child(SharedString::from(text[at..r.start].to_string())));
        }
        if images || text[r.clone()].starts_with('<') {
            // `.emoji` 1.375em
            row = row.child(div().flex_none().mx(gpui::px(1.)).child(crate::media::emoji::emoji_element(&text[r.clone()], font_px * 1.375)));
            at = r.end;
            continue;
        }
        // Keep the UI font (gg sans has no emoji) but make the color emoji font the
        // only fallback, so "❓" no longer resolves to DejaVu Sans' monochrome glyph.
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

/// Unicode `Emoji_Presentation=Yes` for a sequence: VS16, multi-codepoint
/// sequences (ZWJ, modifiers, flags, keycaps), supplementary-plane pictographs, and
/// the BMP characters that default to emoji style.
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
        // VS16 asks for emoji presentation even outside the usual blocks.
        assert_eq!(crate::richtext::emoji_ranges("ℹ\u{FE0F} Informations").len(), 1);
        assert!(super::emoji_presentation("❓") && super::emoji_presentation("ℹ\u{FE0F}") && super::emoji_presentation("🌸"));
        assert!(!super::emoji_presentation("☲") && !super::emoji_presentation("♡"));
    }
}
