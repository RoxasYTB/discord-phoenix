use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

fn push(out: &mut Vec<Range<usize>>, r: Range<usize>) {
    if !r.is_empty() {
        out.push(r);
    }
}

fn token_end(text: &str, from: usize) -> usize {
    text[from..].find(char::is_whitespace).map_or(text.len(), |i| from + i)
}

fn urls(text: &str, out: &mut Vec<Range<usize>>) {
    for pat in ["://", "www."] {
        let mut from = 0;
        while let Some(i) = text[from..].find(pat) {
            let at = from + i;
            let start = text[..at].rfind(|c: char| c.is_whitespace() || c == '<' || c == '(').map_or(0, |s| s + 1);
            let end = token_end(text, at);
            push(out, start..end);
            from = end.max(at + pat.len());
        }
    }
}

fn emoji_shortcodes(text: &str, out: &mut Vec<Range<usize>>) {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b':' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'-' || b[j] == b'~') {
            j += 1;
        }
        if j > i + 1 {
            let end = if j < b.len() && b[j] == b':' { j + 1 } else { j };
            let open_ended = end == j;
            if !open_ended || j == b.len() {
                push(out, i..end);
                i = end;
                continue;
            }
        }
        i += 1;
    }
}

fn sigils(text: &str, out: &mut Vec<Range<usize>>) {
    for (i, ch) in text.char_indices() {
        if matches!(ch, '@' | '#') && text[..i].chars().next_back().is_none_or(|p| p.is_whitespace() || p == '(') {
            push(out, i..token_end(text, i));
        }
    }
}

fn slash_command(text: &str, out: &mut Vec<Range<usize>>) {
    let start = text.len() - text.trim_start().len();
    if text[start..].starts_with('/') {
        push(out, start..token_end(text, start));
    }
}

pub fn excluded(text: &str) -> Vec<Range<usize>> {
    use crate::markdown::input as md;
    let mut out: Vec<Range<usize>> = Vec::new();
    for (r, a) in md::spans(text) {
        if a & (md::CODE | md::LINK | md::FENCE | md::LANG) != 0 {
            push(&mut out, r);
        }
    }
    let blocks = md::blocks(text);
    for (r, _) in blocks.code.iter().chain(blocks.code_lines.iter()) {
        push(&mut out, r.clone());
    }
    for (r, _) in crate::ui::chat::rich_input::spans(text) {
        push(&mut out, r);
    }
    urls(text, &mut out);
    emoji_shortcodes(text, &mut out);
    sigils(text, &mut out);
    slash_command(text, &mut out);
    out.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for r in out {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    merged
}

fn overlaps(skip: &[Range<usize>], r: &Range<usize>) -> bool {
    let i = skip.partition_point(|s| s.end <= r.start);
    skip.get(i).is_some_and(|s| s.start < r.end)
}

fn checkable(word: &str) -> bool {
    word.chars().any(char::is_alphabetic) && !word.chars().any(|c| c.is_numeric() || c == '_')
}

pub fn words(text: &str, caret: Option<usize>) -> Vec<Range<usize>> {
    let skip = excluded(text);
    text.unicode_word_indices()
        .map(|(i, w)| {
            let w = w.trim_end_matches(['\'', '’']);
            i..i + w.len()
        })
        .filter(|r| !r.is_empty() && checkable(&text[r.clone()]) && !overlaps(&skip, r))
        .filter(|r| caret != Some(r.end))
        .collect()
}

pub fn word_at(text: &str, offset: usize) -> Option<Range<usize>> {
    words(text, None).into_iter().find(|r| r.start <= offset && offset <= r.end)
}

pub fn contains_word(text: &str, word: &str) -> bool {
    let lower = text.to_lowercase();
    let word = word.to_lowercase();
    lower.unicode_word_indices().any(|(_, w)| w.trim_end_matches(['\'', '’']) == word)
}

pub fn nearest(text: &str, word: &str, near: usize) -> Option<Range<usize>> {
    words(text, None)
        .into_iter()
        .filter(|r| &text[r.clone()] == word)
        .min_by_key(|r| if near < r.start { r.start - near } else { near.saturating_sub(r.end) })
}
