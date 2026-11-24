//! Converts parsed markdown into cached GPUI text runs. Built once per
//! message (or on spoiler reveal), never in the render path.

use std::ops::Range;

pub mod highlight;

use gpui::{
    Font, FontStyle, FontWeight, Hsla, SharedString, StrikethroughStyle, TextRun, UnderlineStyle,
    font, px,
};

use crate::{
    markdown::{self, Block, Inline, Mention, MentionResolver, Style},
    model::Id,
    theme::{FONT_MONO, FONT_UI, Palette, c, ca, generated::idx, tok},
};

/// Pseudo emoji token: draw the text-channel icon inside a channel mention pill.
pub const CHANNEL_ICON_TOKEN: &str = "\u{1}channel-icon";

#[derive(Clone, Debug)]
pub enum ClickTarget {
    Link(SharedString),
    Spoiler(usize),
    Mention(Mention),
}

#[derive(Clone)]
pub struct RichText {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
    pub click_ranges: Vec<Range<usize>>,
    pub click_targets: Vec<ClickTarget>,
    /// Inline emoji to draw as images: byte range in `text` + token
    /// (`<:name:id>` or the Unicode sequence).
    pub emoji: Vec<(Range<usize>, SharedString)>,
}

#[derive(Clone)]
pub enum RBlock {
    Text(RichText),
    Quote(RichText),
    Code { lang: Option<SharedString>, code: SharedString, runs: Vec<TextRun> },
    Heading(u8, RichText),
    Subtext(RichText),
    ListItem { ordered: Option<u32>, depth: u8, text: RichText },
}

impl RBlock {
    pub fn text(&self) -> Option<&RichText> {
        match self {
            RBlock::Text(t) | RBlock::Quote(t) | RBlock::Heading(_, t) | RBlock::Subtext(t) => Some(t),
            RBlock::ListItem { text, .. } => Some(text),
            RBlock::Code { .. } => None,
        }
    }
}

/// Syntax-coloured monospace runs for a code block.
pub fn code_runs(code: &str, lang: Option<&str>) -> Vec<TextRun> {
    highlight::highlight(code, lang)
        .into_iter()
        .map(|(r, t)| TextRun {
            len: r.len(),
            font: font(FONT_MONO()),
            color: c(t.color()),
            background_color: None,
            underline: None,
            strikethrough: None,
        })
        .collect()
}

pub fn ui_font() -> Font {
    font(FONT_UI())
}

fn run_for(len: usize, style: Style, revealed: bool, base: Hsla) -> TextRun {
    let mut f = if style.has(Style::CODE) { font(FONT_MONO()) } else { ui_font() };
    if style.has(Style::BOLD) {
        f.weight = FontWeight::BOLD;
    }
    if style.has(Style::ITALIC) {
        f.style = FontStyle::Italic;
    }
    let mut color = base;
    let mut background_color = None;
    if style.has(Style::CODE) {
        // markup_ code: --background-code (the 1px --border-normal and 85% size
        // can't be expressed on a text run).
        background_color = Some(c(tok(idx::BACKGROUND_CODE)));
    }
    if style.has(Style::MENTION) {
        color = c(Palette::MENTION_FG);
        background_color = Some(ca(Palette::MENTION_BG));
        f.weight = FontWeight::MEDIUM;
    }
    if style.has(Style::SOUNDBOARD) {
        // Provisional inline chip treatment; the dedicated Soundboard mention CSS
        // is not present in the local CSS cache.
        color = c(Palette::MENTION_FG);
        background_color = Some(ca(Palette::MENTION_BG));
        f.weight = FontWeight::MEDIUM;
    }
    if style.has(Style::TIMESTAMP) {
        // markup_ .timestamp_: --background-mod-normal.
        background_color = Some(c(tok(idx::BACKGROUND_MOD_NORMAL)));
    }
    if style.has(Style::LINK) {
        // anchor_: --text-link
        color = c(tok(idx::TEXT_LINK));
    }
    // Accessibility "Always underline links" (anchorUnderlineOnHover_ off).
    let link_ul = style.has(Style::LINK) && crate::ui::settings::effects::underline_links();
    let underline = (style.has(Style::UNDERLINE) || link_ul).then(|| UnderlineStyle {
        thickness: px(1.),
        color: Some(color),
        wavy: false,
    });
    if style.has(Style::SPOILER) {
        if revealed {
            // spoilerContent_: --__spoiler-background-color--revealed = --spoiler-revealed-background
            background_color = Some(c(tok(idx::SPOILER_REVEALED_BACKGROUND)));
        } else {
            // .hidden_: text and background --spoiler-hidden-background
            color = c(tok(idx::SPOILER_HIDDEN_BACKGROUND));
            background_color = Some(c(tok(idx::SPOILER_HIDDEN_BACKGROUND)));
        }
    }
    TextRun {
        len,
        font: f,
        color,
        background_color,
        underline,
        strikethrough: style.has(Style::STRIKE).then(|| StrikethroughStyle {
            thickness: px(1.),
            color: Some(color),
        }),
    }
}

pub fn rich_inline(inline: &Inline, revealed: &dyn Fn(usize) -> bool, base: Hsla) -> RichText {
    let mut runs = Vec::with_capacity(inline.spans.len());
    let mut click_ranges = Vec::new();
    let mut click_targets = Vec::new();
    for s in &inline.spans {
        let rev = s.spoiler.is_some_and(|i| crate::ui::settings::effects::spoilers_always() || revealed(i));
        runs.push(run_for(s.range.len(), s.style, rev, base));
        if let Some(sp) = s.spoiler
            && !rev
        {
            click_ranges.push(s.range.clone());
            click_targets.push(ClickTarget::Spoiler(sp));
        } else if let Some(l) = &s.link {
            click_ranges.push(s.range.clone());
            click_targets.push(ClickTarget::Link(l.clone().into()));
        } else if let Some(m) = &s.mention {
            click_ranges.push(s.range.clone());
            click_targets.push(ClickTarget::Mention(m.clone()));
        }
    }
    // Code spans keep emoji as text.
    let in_code = |r: &Range<usize>| {
        inline.spans.iter().any(|s| s.style.has(Style::CODE) && s.range.start <= r.start && r.end <= s.range.end)
    };
    let mut emoji: Vec<(Range<usize>, SharedString)> =
        emoji_ranges(&inline.text).into_iter().filter(|(r, _)| !in_code(r)).collect();
    // Channel mentions draw the channel icon (icon__RYHZ5, 1em) in place of the "#".
    for s in &inline.spans {
        if matches!(s.mention, Some(Mention::Channel(_))) && inline.text[s.range.clone()].starts_with('#') {
            emoji.push((s.range.start..s.range.start + 1, CHANNEL_ICON_TOKEN.into()));
        }
    }
    emoji.sort_by_key(|(r, _)| r.start);
    RichText {
        text: inline.text.clone().into(),
        runs,
        click_ranges,
        click_targets,
        emoji,
    }
}

pub fn render_blocks(
    blocks: &[Block],
    revealed: &dyn Fn(usize) -> bool,
    base: Hsla,
) -> Vec<RBlock> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(i) => RBlock::Text(rich_inline(i, revealed, base)),
            // markup_ blockquote color --text-subtle.
            Block::Quote(i) => RBlock::Quote(rich_inline(i, revealed, c(tok(idx::TEXT_SUBTLE)))),
            Block::Code { lang, code } => RBlock::Code {
                lang: lang.clone().map(Into::into),
                code: code.clone().into(),
                runs: code_runs(code, lang.as_deref()),
            },
            Block::Heading(l, i) => {
                let mut t = rich_inline(i, revealed, c(Palette::TEXT_HEADER));
                for r in &mut t.runs {
                    r.font.weight = FontWeight::BOLD;
                }
                RBlock::Heading(*l, t)
            }
            // markup_ small:not(.inlineFormat_) color --text-subtle.
            Block::Subtext(i) => RBlock::Subtext(rich_inline(i, revealed, c(tok(idx::TEXT_SUBTLE)))),
            Block::ListItem { ordered, depth, inline } => RBlock::ListItem {
                ordered: *ordered,
                depth: *depth,
                text: rich_inline(inline, revealed, base),
            },
        })
        .collect()
}

/// Everything needed to draw one message row without further work.
#[derive(Clone)]
pub struct RenderedMessage {
    pub group_start: bool,
    pub day_divider: Option<SharedString>,
    pub time_full: SharedString,
    pub time_short: SharedString,
    pub blocks: Vec<RBlock>,
    pub mentions_me: bool,
    /// One-line preview used when other messages reply to this one.
    pub preview: SharedString,
    pub parsed: Vec<Block>,
}

pub fn build(
    content: &str,
    resolver: &dyn MentionResolver,
    me: Id,
    revealed: &dyn Fn(usize) -> bool,
) -> (Vec<Block>, Vec<RBlock>, bool, SharedString) {
    let parsed = markdown::parse(content, resolver);
    let mentions_me = parsed.iter().filter_map(Block::inline).any(|i| {
        i.spans.iter().any(|s| {
            matches!(s.mention, Some(Mention::User(u)) if u == me.0)
                || matches!(s.mention, Some(Mention::Everyone | Mention::Here))
                || matches!(s.mention, Some(Mention::Role(r)) if resolver.has_role(r))
        })
    });
    let (preview, _) = preview_with_mentions(&parsed);
    let mut blocks = render_blocks(&parsed, revealed, c(Palette::TEXT_NORMAL));
    color_role_mentions(&parsed, &mut blocks, resolver);
    (parsed, blocks, mentions_me, preview.into())
}

/// One-line reply preview of `parsed` (blocks joined by spaces, newlines folded, at
/// most 120 chars) and the byte ranges of its mentions, which the client's
/// `repliedTextContent_` still draws with the mention colours.
pub fn preview_with_mentions(parsed: &[Block]) -> (String, Vec<Range<usize>>) {
    let mut text = String::new();
    let mut mentions = Vec::new();
    for (n, b) in parsed.iter().enumerate() {
        if n > 0 {
            text.push(' ');
        }
        let at = text.len();
        match b {
            Block::Code { code, .. } => text.push_str(code),
            b => {
                if let Some(i) = b.inline() {
                    text.push_str(&i.text);
                    mentions.extend(i.spans.iter().filter(|s| s.mention.is_some()).map(|s| s.range.start + at..s.range.end + at));
                }
            }
        }
    }
    let text = text.replace('\n', " ");
    let cut = text.char_indices().nth(120).map_or(text.len(), |(i, _)| i);
    let mentions = mentions.into_iter().filter(|r| r.start < cut).map(|r| r.start..r.end.min(cut)).collect();
    (text[..cut].to_string(), mentions)
}

/// Role mentions take the role color: text in the color, background at 10% alpha
/// (live client: inline `color` + `background-color` on `.roleMention_`). `rich_inline`
/// emits one run per span, so span `i` is run `i`.
fn color_role_mentions(parsed: &[Block], blocks: &mut [RBlock], r: &dyn MentionResolver) {
    for (b, rb) in parsed.iter().zip(blocks.iter_mut()) {
        let (Some(inl), Some(t)) = (b.inline(), rich_text_mut(rb)) else { continue };
        for (s, run) in inl.spans.iter().zip(t.runs.iter_mut()) {
            if let Some(Mention::Role(id)) = s.mention
                && let Some(rgb) = r.role_color(id).filter(|&v| v != 0)
            {
                let col: Hsla = gpui::rgb(rgb).into();
                run.color = col;
                run.background_color = Some(Hsla { a: 0.1, ..col });
            }
        }
    }
}

fn rich_text_mut(b: &mut RBlock) -> Option<&mut RichText> {
    match b {
        RBlock::Text(t) | RBlock::Quote(t) | RBlock::Heading(_, t) | RBlock::Subtext(t) => Some(t),
        RBlock::ListItem { text, .. } => Some(text),
        RBlock::Code { .. } => None,
    }
}

fn is_emoji_modifier(c: char) -> bool {
    matches!(c as u32, 0xFE0F | 0x200D | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F)
}

/// Custom emoji tokens and Unicode emoji sequences in `text`, in order.
pub fn emoji_ranges(text: &str) -> Vec<(Range<usize>, SharedString)> {
    let custom = crate::media::emoji::find_custom(text);
    let mut out: Vec<(Range<usize>, SharedString)> = Vec::new();
    let mut ci = custom.iter().peekable();
    let mut it = text.char_indices().peekable();
    while let Some((i, ch)) = it.next() {
        if let Some((r, _)) = ci.peek()
            && r.start == i
        {
            let r = r.clone();
            out.push((r.clone(), text[r.clone()].to_owned().into()));
            ci.next();
            while it.peek().is_some_and(|(j, _)| *j < r.end) {
                it.next();
            }
            continue;
        }
        let cp = ch as u32;
        let start_ok = cp >= 0x1F000 || (0x2190..0x2BFF).contains(&cp) || (0x2600..0x27C0).contains(&cp);
        let keycap = ch.is_ascii_digit() || ch == '#' || ch == '*';
        // Any other symbol with VS16 (U+FE0F) asks for emoji presentation ("ℹ️", "‼️").
        let vs16 = !ch.is_ascii() && text[i + ch.len_utf8()..].starts_with('\u{FE0F}');
        if !start_ok && !keycap && !vs16 {
            continue;
        }
        let mut end = i + ch.len_utf8();
        let mut regional = (0x1F1E6..=0x1F1FF).contains(&cp);
        while let Some(&(j, c)) = it.peek() {
            let joined = text[..j].ends_with('\u{200D}');
            if is_emoji_modifier(c) || joined || (regional && (0x1F1E6..=0x1F1FF).contains(&(c as u32))) {
                if (0x1F1E6..=0x1F1FF).contains(&(c as u32)) {
                    regional = false;
                }
                end = j + c.len_utf8();
                it.next();
            } else {
                break;
            }
        }
        let seq = &text[i..end];
        if keycap && !seq.contains('\u{20E3}') {
            continue;
        }
        if crate::media::emoji::twemoji_code(seq).is_some() {
            out.push((i..end, seq.to_owned().into()));
        }
    }
    out
}

/// Emoji-only message (whitespace aside, at most 30 emoji): Discord draws
/// them "jumbo" (48px). Client markup rule: only text, emoji and custom emoji
/// top-level nodes, non-blank text disqualifies, and more than 30 emoji keeps
/// them inline; a heading is another node type, so it never jumbos.
pub fn is_jumbo(blocks: &[RBlock]) -> bool {
    let mut n = 0;
    for b in blocks {
        let RBlock::Text(t) = b else { return false };
        let mut last = 0;
        for (r, _) in &t.emoji {
            if !t.text[last..r.start].trim().is_empty() {
                return false;
            }
            last = r.end;
            n += 1;
        }
        if !t.text[last..].trim().is_empty() {
            return false;
        }
    }
    n > 0 && n <= 30
}

#[cfg(test)]
mod emoji_tests {
    use super::*;
    #[test]
    fn jumbo_up_to_thirty_emoji() {
        let block = |s: String| RBlock::Text(RichText { emoji: emoji_ranges(&s), text: s.into(), runs: vec![], click_ranges: vec![], click_targets: vec![] });
        assert!(is_jumbo(&[block("👍 <a:d:9>".into())]));
        assert!(is_jumbo(&[block("👍".repeat(30))]));
        assert!(!is_jumbo(&[block("👍".repeat(31))]));
        assert!(!is_jumbo(&[block("hi 👍".into())]));
    }

    #[test]
    fn finds_unicode_and_custom() {
        let t = "hi 👍🏽 <:blob:12> 👨\u{200D}👩\u{200D}👧 🇫🇷 ok 1";
        let e: Vec<String> = emoji_ranges(t).into_iter().map(|(_, s)| s.to_string()).collect();
        assert_eq!(e, vec!["👍🏽", "<:blob:12>", "👨\u{200D}👩\u{200D}👧", "🇫🇷"]);
    }
}
