use std::{cell::RefCell, ops::Range};

use gpui::{AnyElement, App, Context, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement, StrikethroughStyle, Styled, UnderlineStyle, Window, div, px};
use gpui::TextBox;
use gpui_base::input::{LineStyle, RichSpan};
use gpui_component::input::{InlineToken, InlineTokenContext, InputContent, TextDecoration};

use crate::{
    app::PhoenixApp,
    markdown::MentionResolver,
    theme::{Palette, c},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    User(u64),
    Role(u64),
    Channel(u64),
    Emoji { name: String, animated: bool },
}

pub fn legacy(app: &PhoenixApp) -> bool {
    app.prefs.synced.flag("legacy-input")
}

pub fn enabled(app: &PhoenixApp) -> bool {
    app.prefs.synced.flag("md-preview") && !legacy(app)
}

fn id_of(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn kind(inner: &str) -> Option<Kind> {
    if let Some(r) = inner.strip_prefix("@&") {
        return id_of(r).map(Kind::Role);
    }
    if let Some(u) = inner.strip_prefix('@') {
        return id_of(u.strip_prefix('!').unwrap_or(u)).map(Kind::User);
    }
    if let Some(ch) = inner.strip_prefix('#') {
        return id_of(ch).map(Kind::Channel);
    }
    let (animated, rest) = match inner.strip_prefix("a:") {
        Some(r) => (true, r),
        None => (false, inner.strip_prefix(':')?),
    };
    let (name, id) = rest.split_once(':')?;
    id_of(id)?;
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then(|| Kind::Emoji { name: name.to_owned(), animated })
}

pub fn spans(text: &str) -> Vec<(Range<usize>, Kind)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find('<') {
        let start = from + i;
        let Some(len) = text[start + 1..].find(['>', '<']) else { break };
        let end = start + 1 + len;
        if text.as_bytes()[end] == b'>'
            && let Some(k) = kind(&text[start + 1..end])
        {
            out.push((start..end + 1, k));
            from = end + 1;
        } else {
            from = end;
        }
    }
    out
}

pub fn label(k: &Kind, r: &dyn MentionResolver) -> String {
    match k {
        Kind::User(id) => format!("@{}", r.user(*id).unwrap_or_else(|| crate::t!("tts.unknown_user").to_string())),
        Kind::Role(id) => format!("@{}", r.role(*id).unwrap_or_else(|| crate::t!("tts.unknown_role").to_string())),
        Kind::Channel(id) => format!("#{}", r.channel(*id).unwrap_or_else(|| crate::t!("tts.unknown_channel").to_string())),
        Kind::Emoji { name, .. } => format!(":{name}:"),
    }
}

pub fn content(text: &str, r: &dyn MentionResolver) -> InputContent {
    let mut content = InputContent::new(text.to_owned());
    for (range, k) in spans(text) {
        let raw = text[range.clone()].to_owned();
        let token = InlineToken::new(raw.clone(), raw).with_label(label(&k, r));
        if let Ok(next) = content.clone().with_token(range, token) {
            content = next;
        }
    }
    content
}

type Area = gpui::Entity<gpui_component::input::TextareaState>;

fn wanted(app: &PhoenixApp, area: &Area, cx: &App) -> InputContent {
    let text = area.read(cx).value().to_string();
    if !enabled(app) {
        return InputContent::new(text);
    }
    match app.selected_channel {
        Some(ch) => content(&text, &app.resolver_with_sounds(&[], ch)),
        None => content(&text, &app.resolver()),
    }
}

pub fn style(a: u16) -> HighlightStyle {
    use crate::markdown::input as md;
    use crate::theme::{generated::idx, tok};
    let has = |f: u16| a & f != 0;
    let mut s = HighlightStyle::default();
    if has(md::SYNTAX) {
        s.color = Some(c(tok(idx::TEXTBOX_MARKDOWN_SYNTAX)));
        s.font_weight = Some(if has(md::LIGHT) { FontWeight::NORMAL } else { FontWeight::SEMIBOLD });
    } else {
        if has(md::BOLD) {
            s.font_weight = Some(FontWeight::BOLD);
        }
        if has(md::ITALIC) {
            s.font_style = Some(FontStyle::Italic);
        }
        if has(md::STRIKE) {
            s.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: None });
        }
    }
    if has(md::UNDERLINE) {
        s.underline = Some(UnderlineStyle { thickness: px(1.), color: None, wavy: false });
    }
    if has(md::LINK) {
        s.color = Some(c(tok(idx::TEXT_LINK)));
    }
    if has(md::SUBTEXT) || has(md::SUBTEXT_MARK) {
        s.color = Some(c(tok(idx::TEXT_MUTED)));
    }
    if has(md::SUBTEXT_MARK) {
        s.font_weight = Some(FontWeight::BOLD);
    }
    if has(md::FENCE) {
        s.color = Some(c(tok(idx::TEXTBOX_MARKDOWN_SYNTAX)));
    }
    if has(md::LANG) {
        s.color = Some(c(tok(idx::TEXT_FEEDBACK_POSITIVE)));
    }
    s
}

pub struct Rich {
    pub decorations: Vec<TextDecoration>,
    pub spans: Vec<RichSpan>,
    pub lines: Vec<LineStyle>,
}

const CODE_SCALE: f32 = 0.85;
const BASE_SIZE: f32 = 16.;
const SUBTEXT_SIZE: f32 = 13.;
const SPOILER_PADDING_EM: f32 = 0.2;
const QUOTE_BAR: f32 = 4.;
const QUOTE_GAP: f32 = 12.;
const QUOTE_END: f32 = 8.;

fn merged(spans: &[(Range<usize>, u16)], flag: u16) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    for (r, a) in spans {
        if a & flag == 0 {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end == r.start => last.end = r.end,
            _ => out.push(r.clone()),
        }
    }
    out
}

pub fn rich_for(text: &str) -> Rich {
    use crate::markdown::input as md;
    use crate::theme::{generated::global::RADIUS_XS, generated::idx, tok};
    let spans = md::spans(text);
    let blocks = md::blocks(text);
    let mono = crate::theme::FONT_MONO();
    let mut decorations: Vec<TextDecoration> = spans.iter().map(|(r, a)| TextDecoration::new(r.clone(), style(*a))).collect();
    let mut rich: Vec<RichSpan> = Vec::new();
    let small = |a: u16| a & (md::SUBTEXT | md::SUBTEXT_MARK) != 0;
    for (r, a) in &spans {
        let size = if small(*a) { SUBTEXT_SIZE } else { BASE_SIZE };
        if a & md::CODE != 0 {
            rich.push(RichSpan::new(r.clone()).font_family(mono).font_size(px(size * CODE_SCALE)));
        } else if small(*a) {
            rich.push(RichSpan::new(r.clone()).font_size(px(size)));
        }
    }
    for r in merged(&spans, md::CODE) {
        let b = TextBox::new(r.clone())
            .background(c(tok(idx::BACKGROUND_CODE)))
            .border(px(1.), c(tok(idx::BORDER_MUTED)))
            .corner_radius(px(RADIUS_XS));
        rich.push(RichSpan::new(r).text_box(b));
    }
    for (r, a) in spans.iter().filter(|(_, a)| a & md::SPOILER != 0 && a & md::SYNTAX == 0) {
        let size = if small(*a) { SUBTEXT_SIZE } else { BASE_SIZE };
        let b = TextBox::new(r.clone())
            .background(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .corner_radius(px(RADIUS_XS))
            .padding(px(size * SPOILER_PADDING_EM))
            .clone_edges(true);
        rich.push(RichSpan::new(r.clone()).text_box(b));
    }
    for (r, start, end) in md::marker_gaps(text) {
        let mut b = TextBox::new(r.clone()).padding(px(if start != 0. { start } else { end }));
        b.open_start = start == 0.;
        b.open_end = end == 0.;
        rich.push(RichSpan::new(r).text_box(b));
    }
    let mut lines: Vec<LineStyle> = Vec::new();
    for q in &blocks.quotes {
        lines.push(
            LineStyle::new(q.clone())
                .bar(c(tok(idx::SPINE_DEFAULT)), px(QUOTE_BAR), px(QUOTE_BAR))
                .indent(px(QUOTE_BAR + QUOTE_GAP))
                .end_inset(px(QUOTE_END)),
        );
        rich.push(RichSpan::new(q.start..q.start + 2).hidden());
    }
    for (l, _) in &blocks.code_lines {
        lines.push(LineStyle::new(l.clone()).font_family(mono).font_size(px(BASE_SIZE * CODE_SCALE)));
    }
    for (range, lang) in &blocks.code {
        let code = &text[range.clone()];
        let toks = match lang {
            Some(lang) => crate::richtext::highlight::highlight(code, Some(lang)),
            None => vec![(0..code.len(), crate::richtext::highlight::Tok::Plain)],
        };
        for (r, t) in toks {
            let color = match t {
                crate::richtext::highlight::Tok::Plain => tok(idx::TEXT_DEFAULT),
                crate::richtext::highlight::Tok::Comment => tok(idx::TEXT_MUTED),
                t => t.color(),
            };
            let s = HighlightStyle { color: Some(c(color)), ..Default::default() };
            decorations.push(TextDecoration::new(range.start + r.start..range.start + r.end, s));
        }
    }
    decorations.sort_by_key(|d| (d.range.start, d.range.end));
    rich.sort_by_key(|s| (s.range.start, s.range.end));
    lines.sort_by_key(|l| l.range.start);
    Rich { decorations, spans: rich, lines }
}

pub fn snap(quotes: &[Range<usize>], prev: usize, cur: usize) -> Option<usize> {
    let q = quotes.iter().find(|q| q.len() >= 2 && (cur == q.start || cur == q.start + 1))?;
    let s = q.start;
    if cur == s + 1 && prev == s + 2 && s > 0 {
        return Some(s - 1);
    }
    Some(s + 2)
}

thread_local! {
    static LAST_CARET: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn snap_quote_caret(area: &Area, cx: &mut App) {
    let (text, range) = {
        let st = area.read(cx);
        (st.value().to_string(), st.selected_range())
    };
    if !range.is_empty() {
        LAST_CARET.with(|c| c.set(range.end));
        return;
    }
    let cur = range.start;
    let prev = LAST_CARET.with(|c| c.replace(cur));
    let quotes = crate::markdown::input::blocks(&text).quotes;
    if let Some(to) = snap(&quotes, prev, cur).filter(|t| *t != cur && *t <= text.len()) {
        LAST_CARET.with(|c| c.set(to));
        area.update(cx, |s, cx| s.set_selected_range(to..to, cx));
    }
}

fn fingerprint() -> [u32; 4] {
    use crate::theme::{generated::idx, tok};
    let h = |i: usize| {
        let x = c(tok(i));
        x.h.to_bits() ^ x.s.to_bits().rotate_left(8) ^ x.l.to_bits().rotate_left(16) ^ x.a.to_bits().rotate_left(24)
    };
    [h(idx::TEXT_DEFAULT), h(idx::TEXTBOX_MARKDOWN_SYNTAX), h(idx::BACKGROUND_CODE), h(idx::SPINE_DEFAULT)]
}

type Cached = (String, [u32; 4], std::rc::Rc<Rich>);

thread_local! {
    static WANTED: RefCell<Option<Cached>> = const { RefCell::new(None) };
}

fn wanted_rich(app: &PhoenixApp, area: &Area, cx: &App) -> std::rc::Rc<Rich> {
    let base = markdown_rich(app, area, cx);
    let spell = spell_decorations(app, area, cx);
    if spell.is_empty() {
        return base;
    }
    let mut decorations = base.decorations.clone();
    decorations.extend(spell.iter().cloned());
    decorations.sort_by_key(|d| (d.range.start, d.range.end));
    std::rc::Rc::new(Rich { decorations, spans: base.spans.clone(), lines: base.lines.clone() })
}

pub const SPELL_RED: u32 = 0xff0000;
pub const SPELL_THICKNESS: f32 = 1.;

pub fn spell_style() -> HighlightStyle {
    HighlightStyle {
        underline: Some(UnderlineStyle { thickness: px(SPELL_THICKNESS), color: Some(gpui::rgb(SPELL_RED).into()), wavy: true }),
        ..Default::default()
    }
}

type SpellCached = (String, Option<usize>, u64, std::rc::Rc<Vec<TextDecoration>>);

thread_local! {
    static SPELL: RefCell<Vec<SpellCached>> = const { RefCell::new(Vec::new()) };
}

const SPELL_CACHE: usize = 4;

fn spell_decorations(app: &PhoenixApp, area: &Area, cx: &App) -> std::rc::Rc<Vec<TextDecoration>> {
    if !crate::ui::extra_menus::spellcheck_on(app) || !crate::spell::has_dictionaries() {
        return std::rc::Rc::new(Vec::new());
    }
    let (text, caret) = {
        let st = area.read(cx);
        let sel = st.selected_range();
        (st.value().to_string(), sel.is_empty().then_some(sel.start))
    };
    let generation = crate::spell::generation();
    if let Some(hit) = SPELL.with_borrow(|c| c.iter().find(|(t, k, g, _)| *t == text && *k == caret && *g == generation).map(|e| e.3.clone())) {
        return hit;
    }
    let ranges = crate::spell::misspelled(&text, caret, cx);
    let style = spell_style();
    let list = std::rc::Rc::new(ranges.into_iter().map(|r| TextDecoration::new(r, style)).collect::<Vec<_>>());
    let generation = crate::spell::generation();
    SPELL.with_borrow_mut(|c| {
        c.retain(|(t, k, _, _)| !(*t == text && *k == caret));
        c.push((text, caret, generation, list.clone()));
        if c.len() > SPELL_CACHE {
            c.remove(0);
        }
    });
    list
}

fn markdown_rich(app: &PhoenixApp, area: &Area, cx: &App) -> std::rc::Rc<Rich> {
    if !enabled(app) {
        return std::rc::Rc::new(Rich { decorations: Vec::new(), spans: Vec::new(), lines: Vec::new() });
    }
    let text = area.read(cx).value().to_string();
    let print = fingerprint();
    WANTED.with_borrow_mut(|w| {
        if let Some((t, p, r)) = w.as_ref()
            && *t == text
            && *p == print
        {
            return r.clone();
        }
        let r = std::rc::Rc::new(rich_for(&text));
        *w = Some((text, print, r.clone()));
        r
    })
}

pub fn set_value_at_end(s: &mut gpui_component::input::TextareaState, value: String, window: &mut gpui::Window, cx: &mut Context<gpui_component::input::TextareaState>) {
    let end = value.len();
    s.set_value(value, window, cx);
    s.set_selected_range(end..end, cx);
}

pub fn sync(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let composer = app.composer.clone();
    sync_area(app, &composer, cx);
    if let Some(edit) = app.chat.edit.as_ref().map(|e| e.input.clone()) {
        sync_area(app, &edit, cx);
    }
}

pub fn sync_area(app: &mut PhoenixApp, area: &Area, cx: &mut Context<PhoenixApp>) {
    let want = wanted(app, area, cx);
    let rich = wanted_rich(app, area, cx);
    area.update(cx, |s, cx| {
        let _ = s.set_tokens(want, cx);
        s.set_decorations(rich.decorations.clone(), cx);
        s.set_rich_decorations(rich.spans.clone(), rich.lines.clone(), cx);
    });
}

pub fn needs_sync(app: &PhoenixApp, cx: &App) -> bool {
    area_needs_sync(app, &app.composer, cx) || app.chat.edit.as_ref().is_some_and(|e| area_needs_sync(app, &e.input, cx))
}

fn area_needs_sync(app: &PhoenixApp, area: &Area, cx: &App) -> bool {
    let want = wanted(app, area, cx);
    let rich = wanted_rich(app, area, cx);
    let state = area.read(cx);
    let have = state.tokens();
    have.len() != want.tokens().len()
        || have.iter().zip(want.tokens()).any(|(a, b)| a.range() != b.range() || a.token().label() != b.token().label())
        || state.decorations() != rich.decorations.as_slice()
        || state.rich_decorations().spans() != rich.spans.as_slice()
        || state.rich_decorations().lines() != rich.lines.as_slice()
}

pub fn render_token(ctx: &InlineTokenContext, _: &mut Window, _: &mut App) -> AnyElement {
    let text = ctx.token().text().to_string();
    let size = f32::from(ctx.line_height());
    if text.starts_with("<:") || text.starts_with("<a:") {
        return div().h(ctx.line_height()).flex().items_center().child(crate::media::emoji::emoji_element(&text, size)).into_any_element();
    }
    div()
        .h(ctx.line_height())
        .flex()
        .items_center()
        .px(gpui::px(2.))
        .rounded(gpui::px(3.))
        .bg(c(Palette::MENTION_BG))
        .text_color(c(Palette::MENTION_FG))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(ctx.token().label().clone())
        .into_any_element()
}

thread_local! {
    static PLAIN: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn plain_text(k: &Kind, app: &PhoenixApp) -> Option<String> {
    Some(match k {
        Kind::User(id) => format!("@{}", app.users.get(&crate::model::Id(*id))?.name),
        Kind::Role(id) => format!("@{}", app.roles.values().flatten().find(|r| r.id.0 == *id)?.name),
        Kind::Channel(id) => format!("#{}", app.channels.get(&crate::model::Id(*id))?.name),
        Kind::Emoji { name, .. } => format!(":{name}:"),
    })
}

pub fn insertion(app: &PhoenixApp, raw: String) -> String {
    if enabled(app) {
        return raw;
    }
    let Some(k) = raw.strip_prefix('<').and_then(|r| r.strip_suffix('>')).and_then(kind) else { return raw };
    let Some(shown) = plain_text(&k, app) else { return raw };
    PLAIN.with_borrow_mut(|p| {
        p.retain(|(s, _)| *s != shown);
        p.push((shown.clone(), raw));
    });
    shown
}

fn boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
}

pub fn resolve(text: &str, map: &[(String, String)]) -> String {
    let mut pairs: Vec<&(String, String)> = map.iter().collect();
    pairs.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < text.len() {
        for (shown, raw) in &pairs {
            if text[i..].starts_with(shown.as_str()) && boundary(text[i + shown.len()..].chars().next()) {
                out.push_str(raw);
                i += shown.len();
                continue 'outer;
            }
        }
        let ch = text[i..].chars().next().unwrap_or_default();
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

pub fn peek_plain(text: &str) -> String {
    let map = PLAIN.with_borrow(Clone::clone);
    if map.is_empty() { text.to_owned() } else { resolve(text, &map) }
}

pub fn take_plain(text: &str) -> String {
    let map = PLAIN.with_borrow_mut(std::mem::take);
    if map.is_empty() { text.to_owned() } else { resolve(text, &map) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_marker_is_a_single_caret_stop() {
        let text = "hi\n> quote";
        let q = crate::markdown::input::blocks(text).quotes;
        assert_eq!(q, vec![3..10]);
        assert_eq!(snap(&q, 5, 4), Some(2));
        assert_eq!(snap(&q, 2, 3), Some(5));
        assert_eq!(snap(&q, 9, 3), Some(5));
        assert_eq!(snap(&q, 5, 5), None);
        let first = crate::markdown::input::blocks("> a").quotes;
        assert_eq!(snap(&first, 2, 1), Some(2));
        assert_eq!(snap(&first, 3, 0), Some(2));
    }

    #[test]
    fn strike_underline_and_italic_markers_get_the_client_margins() {
        let gaps = crate::markdown::input::marker_gaps("~~a~~ __b__ *c* **d**");
        assert_eq!(gaps, vec![(0..2, 0., 1.), (3..5, 1., 0.), (6..8, 0., 1.), (9..11, 1., 0.), (12..13, 0., -1.), (14..15, 1., 0.)]);
        let rich = rich_for("~~a~~");
        let boxes: Vec<_> = rich.spans.iter().filter_map(|s| s.text_box.clone()).collect();
        assert_eq!(boxes.len(), 2);
        assert!(boxes[0].open_start && !boxes[0].open_end && boxes[0].padding == px(1.));
        assert!(!boxes[1].open_start && boxes[1].open_end);
        assert!(boxes.iter().all(|b| b.background.is_none() && b.border_color.is_none()));
    }

    struct R;
    impl MentionResolver for R {
        fn user(&self, id: u64) -> Option<String> {
            (id == 1).then(|| "Alice".into())
        }
        fn channel(&self, id: u64) -> Option<String> {
            (id == 5).then(|| "général".into())
        }
        fn role(&self, id: u64) -> Option<String> {
            (id == 9).then(|| "Modo".into())
        }
    }

    #[test]
    fn finds_the_discord_tokens() {
        let t = "hi <@1> <@!2> <@&9> <#5> <:pog:12> <a:dance:3> <nope> <@x> <t:1:R>";
        let kinds: Vec<Kind> = spans(t).into_iter().map(|(_, k)| k).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::User(1),
                Kind::User(2),
                Kind::Role(9),
                Kind::Channel(5),
                Kind::Emoji { name: "pog".into(), animated: false },
                Kind::Emoji { name: "dance".into(), animated: true },
            ]
        );
        assert_eq!(spans("<<@1>"), vec![(1..5, Kind::User(1))]);
    }

    #[test]
    fn builds_labelled_tokens() {
        let c = content("yo <@1> in <#5> <:pog:12>", &R);
        let labels: Vec<String> = c.tokens().iter().map(|t| t.token().label().to_string()).collect();
        assert_eq!(labels, vec!["@Alice", "#général", ":pog:"]);
        assert_eq!(c.tokens()[0].range(), 3..7);
        assert_eq!(c.text().as_ref(), "yo <@1> in <#5> <:pog:12>");
    }

    #[test]
    fn rich_styles_follow_the_client_css() {
        let t = "a `co` ||sp||\n-# small\n> quote\n```rust\nlet x = 1; // c\n```";
        let r = rich_for(t);
        let code = t.find('`').unwrap();
        let boxed = r.spans.iter().find(|s| s.text_box.is_some() && s.range.start == code).expect("code box");
        assert_eq!(boxed.range, code..code + 4);
        let b = boxed.text_box.as_ref().unwrap();
        assert_eq!(b.border_width, px(1.));
        assert_eq!(b.corner_radius, px(4.));
        assert!(r.spans.iter().any(|s| s.range.start == code + 1 && s.font_size == Some(px(13.6)) && s.font_family.is_some()));
        let sp = t.find("sp|").unwrap();
        let spoiler = r.spans.iter().find(|s| s.range.start == sp && s.text_box.is_some()).expect("spoiler box");
        let b = spoiler.text_box.as_ref().unwrap();
        assert!((f32::from(b.padding) - 3.2).abs() < 1e-4 && b.clone_edges && b.border_width == px(0.));
        let small = t.find("small").unwrap();
        assert!(r.spans.iter().any(|s| s.range.start <= small && small < s.range.end && s.font_size == Some(px(13.))));
        let q = t.find("> quote").unwrap();
        assert!(r.spans.iter().any(|s| s.range == (q..q + 2) && s.hidden));
        let quote = r.lines.iter().find(|l| l.range.start == q).expect("quote line");
        assert_eq!(quote.indent, px(16.));
        assert_eq!(quote.end_inset, px(8.));
        assert_eq!(quote.bar.map(|b| b.width), Some(px(4.)));
        let l = t.find("let").unwrap();
        let line = r.lines.iter().find(|x| x.range.start == l).expect("code line");
        assert_eq!(line.font_size, Some(px(13.6)));
        assert!(line.font_family.is_some());
        let comment = t.find("// c").unwrap();
        assert!(r.decorations.iter().any(|d| d.range.contains(&comment) && d.style.color.is_some()));
        assert!(r.decorations.iter().all(|d| d.style.background_color.is_none()));
    }

    #[test]
    fn plain_names_go_back_to_ids_on_send() {
        let map = vec![("@bob".to_owned(), "<@7>".to_owned()), ("@bobby".to_owned(), "<@8>".to_owned()), (":pog:".to_owned(), "<:pog:12>".to_owned())];
        assert_eq!(resolve("hey @bobby and @bob :pog: @bobx", &map), "hey <@8> and <@7> <:pog:12> @bobx");
        assert_eq!(resolve("café @bob.", &map), "café <@7>.");
    }
}
