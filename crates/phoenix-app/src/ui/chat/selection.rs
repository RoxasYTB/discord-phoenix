use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Range;

use gpui::{
    App, Bounds, DispatchPhase, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString,
    Styled, TextLayout, TextRun, Window, canvas,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub msg: u64,
    pub block: usize,
    pub ix: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct Sel {
    anchor: Option<Pos>,
    focus: Option<Pos>,
    drag: bool,
    channel: Option<u64>,
}

#[derive(Clone)]
struct Cell {
    msg: u64,
    block: usize,
    start: usize,
    len: usize,
    bounds: Bounds<Pixels>,
    layout: TextLayout,
}

#[derive(Clone, Default)]
struct Block {
    text: SharedString,
    swaps: Vec<(Range<usize>, SharedString)>,
}

thread_local! {
    static SEL: RefCell<Sel> = RefCell::new(Sel::default());
    static CELLS: RefCell<Vec<Cell>> = const { RefCell::new(Vec::new()) };
    static TEXTS: RefCell<BTreeMap<(u64, usize), Block>> = const { RefCell::new(BTreeMap::new()) };
    static AREA: RefCell<Option<Bounds<Pixels>>> = const { RefCell::new(None) };
    static POINTER: RefCell<Option<Point<Pixels>>> = const { RefCell::new(None) };
}

pub fn autoscroll_step(area: Bounds<Pixels>, p: Point<Pixels>) -> Option<Pixels> {
    let over = if p.y < area.top() {
        p.y - area.top()
    } else if p.y > area.bottom() {
        p.y - area.bottom()
    } else {
        return None;
    };
    let max = gpui::px(40.);
    Some((over / 2.).clamp(-max, max))
}

fn set_focus(p: Pos) -> bool {
    SEL.with(|s| {
        let mut s = s.borrow_mut();
        let moved = s.focus != Some(p);
        s.focus = Some(p);
        moved
    })
}

pub fn selection_bg() -> Hsla {
    gpui::Hsla::from(gpui::rgba(SELECTION_RGBA))
}

pub const SELECTION_RGBA: u32 = 0x0041c6cc;
pub const AUTHOR_BLOCK: usize = 0;
pub const BODY_BASE: usize = 1;
pub const EMBED_BASE: usize = 10_000;
pub const EMBED_STRIDE: usize = 1_000;

pub fn begin_frame(channel: Option<u64>) {
    CELLS.with(|c| c.borrow_mut().clear());
    let changed = SEL.with(|s| {
        let mut s = s.borrow_mut();
        if s.channel != channel {
            *s = Sel { channel, ..Sel::default() };
            true
        } else {
            false
        }
    });
    if changed {
        TEXTS.with(|t| t.borrow_mut().clear());
    }
}

pub fn note(msg: u64, block: usize, text: SharedString, emoji: &[(Range<usize>, SharedString)]) {
    let swaps = emoji
        .iter()
        .map(|(r, token)| {
            let shown = crate::media::emoji::parse_custom(token).map_or_else(|| token.clone(), |e| SharedString::from(format!(":{}:", e.name)));
            (r.clone(), shown)
        })
        .collect();
    TEXTS.with(|t| t.borrow_mut().insert((msg, block), Block { text, swaps }));
}

fn ordered() -> Option<(Pos, Pos)> {
    SEL.with(|s| {
        let s = s.borrow();
        let (a, f) = (s.anchor?, s.focus?);
        (a != f).then(|| if a <= f { (a, f) } else { (f, a) })
    })
}

pub fn has_selection() -> bool {
    ordered().is_some()
}

pub fn clear() -> bool {
    SEL.with(|s| {
        let mut s = s.borrow_mut();
        let had = s.anchor.is_some();
        s.anchor = None;
        s.focus = None;
        s.drag = false;
        had
    })
}

pub fn range_in(msg: u64, block: usize, start: usize, len: usize) -> Option<Range<usize>> {
    let (a, b) = ordered()?;
    let key = (msg, block);
    if key < (a.msg, a.block) || key > (b.msg, b.block) {
        return None;
    }
    let from = if key == (a.msg, a.block) { a.ix } else { 0 };
    let to = if key == (b.msg, b.block) { b.ix } else { usize::MAX };
    let lo = from.max(start);
    let hi = to.min(start + len);
    (lo < hi).then(|| lo - start..hi - start)
}

pub fn covers(msg: u64, block: usize, r: &Range<usize>) -> bool {
    range_in(msg, block, r.start, r.len()).is_some_and(|s| s.start == 0 && s.end == r.len())
}

pub fn paint_runs(runs: &[TextRun], range: Range<usize>, bg: Hsla) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        let cuts = [at, range.start.clamp(at, end), range.end.clamp(at, end), end];
        for w in cuts.windows(2) {
            if w[1] > w[0] {
                let mut piece = run.clone();
                piece.len = w[1] - w[0];
                if w[0] >= range.start && w[1] <= range.end {
                    piece.background_color = Some(bg);
                }
                out.push(piece);
            }
        }
        at = end;
    }
    out
}

pub fn highlight(msg: u64, block: usize, start: usize, len: usize, runs: Vec<TextRun>) -> Vec<TextRun> {
    match range_in(msg, block, start, len) {
        Some(r) => paint_runs(&runs, r, selection_bg()),
        None => runs,
    }
}

pub fn probe(layout: TextLayout, msg: u64, block: usize, start: usize, len: usize) -> impl IntoElement {
    canvas(
        move |_, _, _| {
            let bounds = layout.bounds();
            CELLS.with(|c| c.borrow_mut().push(Cell { msg, block, start, len, bounds, layout: layout.clone() }));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_0()
}

fn cell_pos(c: &Cell, p: Point<Pixels>) -> Pos {
    let ix = match c.layout.index_for_position(p) {
        Ok(i) | Err(i) => i,
    };
    Pos { msg: c.msg, block: c.block, ix: c.start + ix.min(c.len) }
}

fn exact(p: Point<Pixels>) -> Option<(Cell, Pos)> {
    CELLS.with(|c| c.borrow().iter().find(|c| c.bounds.contains(&p)).map(|c| (c.clone(), cell_pos(c, p))))
}

pub fn nearest_in(cells: &[(Pos, Bounds<Pixels>, Pos)], p: Point<Pixels>) -> Option<Pos> {
    let mut best: Option<Pos> = None;
    let mut first: Option<Pos> = None;
    for (start, b, end) in cells {
        if first.is_none_or(|f| *start < f) {
            first = Some(*start);
        }
        let before = b.bottom() <= p.y || (b.top() <= p.y && b.left() <= p.x);
        if before && best.is_none_or(|x| *end > x) {
            best = Some(*end);
        }
    }
    best.or(first)
}

fn nearest(p: Point<Pixels>) -> Option<Pos> {
    if let Some((_, pos)) = exact(p) {
        return Some(pos);
    }
    let cells: Vec<(Pos, Bounds<Pixels>, Pos)> = CELLS.with(|c| {
        c.borrow()
            .iter()
            .map(|c| (Pos { msg: c.msg, block: c.block, ix: c.start }, c.bounds, Pos { msg: c.msg, block: c.block, ix: c.start + c.len }))
            .collect()
    });
    nearest_in(&cells, p)
}

pub fn word_at(text: &str, ix: usize) -> Range<usize> {
    let ix = ix.min(text.len());
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..ix].char_indices().rev().take_while(|(_, c)| is_word(*c)).last().map_or(ix, |(i, _)| i);
    let end = text[ix..].char_indices().find(|(_, c)| !is_word(*c)).map_or(text.len(), |(i, _)| ix + i);
    if start == end {
        let next = text[ix..].chars().next().map_or(0, char::len_utf8);
        return ix..ix + next;
    }
    start..end
}

pub fn line_at(text: &str, ix: usize) -> Range<usize> {
    let ix = ix.min(text.len());
    let start = text[..ix].rfind('\n').map_or(0, |i| i + 1);
    let end = text[ix..].find('\n').map_or(text.len(), |i| ix + i);
    start..end
}

fn block_text(msg: u64, block: usize) -> Option<SharedString> {
    TEXTS.with(|t| t.borrow().get(&(msg, block)).map(|b| b.text.clone()))
}

pub fn on_down(ev: &MouseDownEvent, w: &mut Window, _: &mut App) {
    let hit = exact(ev.position);
    let Some((_, pos)) = hit else {
        if !ev.modifiers.shift && clear() {
            w.refresh();
        }
        return;
    };
    SEL.with(|s| {
        let mut s = s.borrow_mut();
        if ev.modifiers.shift && s.anchor.is_some() {
            s.focus = Some(pos);
            s.drag = true;
            return;
        }
        let span = match ev.click_count {
            2 => block_text(pos.msg, pos.block).map(|t| word_at(&t, pos.ix)),
            n if n >= 3 => block_text(pos.msg, pos.block).map(|t| line_at(&t, pos.ix)),
            _ => None,
        };
        match span {
            Some(r) => {
                s.anchor = Some(Pos { ix: r.start, ..pos });
                s.focus = Some(Pos { ix: r.end, ..pos });
                s.drag = false;
            }
            None => {
                s.anchor = Some(pos);
                s.focus = Some(pos);
                s.drag = true;
            }
        }
    });
    w.refresh();
}

pub fn listener(list: gpui::ListState) -> impl IntoElement {
    canvas(
        |bounds, _, _| {
            AREA.with(|a| *a.borrow_mut() = Some(bounds));
        },
        move |_, _, w, _| {
            let dragging = SEL.with(|s| s.borrow().drag);
            let pointer = POINTER.with(|p| *p.borrow());
            let area = AREA.with(|a| *a.borrow());
            if let (true, Some(p), Some(area)) = (dragging, pointer, area)
                && let Some(step) = autoscroll_step(area, p)
            {
                list.scroll_by(step);
                let inside = gpui::point(p.x, p.y.clamp(area.top() + gpui::px(1.), area.bottom() - gpui::px(1.)));
                if let Some(pos) = nearest(inside) {
                    set_focus(pos);
                }
                w.request_animation_frame();
            }
            w.on_mouse_event(|ev: &MouseMoveEvent, phase, w, _| {
                if phase != DispatchPhase::Bubble || ev.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                if !SEL.with(|s| s.borrow().drag) {
                    return;
                }
                POINTER.with(|p| *p.borrow_mut() = Some(ev.position));
                let scrolling = AREA.with(|a| a.borrow().and_then(|b| autoscroll_step(b, ev.position)).is_some());
                if scrolling {
                    w.refresh();
                    return;
                }
                if let Some(p) = nearest(ev.position)
                    && set_focus(p)
                {
                    w.refresh();
                }
            });
            w.on_mouse_event(|ev: &MouseUpEvent, phase, _, _| {
                if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left {
                    SEL.with(|s| s.borrow_mut().drag = false);
                    POINTER.with(|p| *p.borrow_mut() = None);
                }
            });
            w.on_mouse_event(|ev: &MouseDownEvent, phase, w, _| {
                if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                    return;
                }
                let inside = AREA.with(|a| a.borrow().is_some_and(|b| b.contains(&ev.position)));
                if !inside && clear() {
                    w.refresh();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn slice(b: &Block, from: usize, to: usize) -> String {
    let to = to.min(b.text.len());
    let from = from.min(to);
    let mut out = String::new();
    let mut at = from;
    for (r, shown) in b.swaps.iter().filter(|(r, _)| r.start >= from && r.end <= to) {
        if r.start < at {
            continue;
        }
        out.push_str(b.text.get(at..r.start).unwrap_or_default());
        out.push_str(shown);
        at = r.end;
    }
    out.push_str(b.text.get(at..to).unwrap_or_default());
    out
}

pub fn selected_text() -> Option<String> {
    let (a, b) = ordered()?;
    let parts: Vec<(u64, String)> = TEXTS.with(|t| {
        t.borrow()
            .range((a.msg, a.block)..=(b.msg, b.block))
            .map(|(&(msg, block), blk)| {
                let from = if (msg, block) == (a.msg, a.block) { a.ix } else { 0 };
                let to = if (msg, block) == (b.msg, b.block) { b.ix } else { usize::MAX };
                (msg, slice(blk, from, to))
            })
            .collect()
    });
    let mut out = String::new();
    for (i, (_, s)) in parts.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(s);
    }
    (!out.is_empty()).then_some(out)
}

pub fn copy(cx: &mut App) -> bool {
    match selected_text() {
        Some(s) => {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(s));
            true
        }
        None => false,
    }
}

pub fn google_url(text: &str) -> String {
    let mut q = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => q.push(b as char),
            _ => q.push_str(&format!("%{b:02X}")),
        }
    }
    format!("https://www.google.com/search?q={q}")
}

#[cfg(test)]
pub fn set_for_test(channel: Option<u64>, anchor: Pos, focus: Pos) {
    SEL.with(|s| *s.borrow_mut() = Sel { anchor: Some(anchor), focus: Some(focus), drag: false, channel });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(msg: u64, block: usize, ix: usize) -> Pos {
        Pos { msg, block, ix }
    }

    #[test]
    fn copies_across_messages_in_order_and_names_custom_emoji() {
        begin_frame(Some(1));
        note(10, 0, "hello world".into(), &[]);
        note(11, 0, "a <:wave:42> b".into(), &[(2..12, "<:wave:42>".into())]);
        note(12, 0, "last".into(), &[]);
        set_for_test(Some(1), p(12, 0, 2), p(10, 0, 6));
        assert_eq!(selected_text().as_deref(), Some("world\na :wave: b\nla"));
        assert_eq!(range_in(11, 0, 2, 10), Some(0..10));
        assert_eq!(range_in(10, 0, 0, 5), None);
        assert_eq!(range_in(10, 0, 6, 5), Some(0..5));
        begin_frame(Some(2));
        assert!(!has_selection());
        assert!(selected_text().is_none());
    }

    #[test]
    fn autoscroll_only_past_the_list_edges_and_capped() {
        let area = Bounds { origin: gpui::point(gpui::px(0.), gpui::px(100.)), size: gpui::size(gpui::px(500.), gpui::px(400.)) };
        assert_eq!(autoscroll_step(area, gpui::point(gpui::px(10.), gpui::px(300.))), None);
        assert_eq!(autoscroll_step(area, gpui::point(gpui::px(10.), gpui::px(90.))), Some(gpui::px(-5.)));
        assert_eq!(autoscroll_step(area, gpui::point(gpui::px(10.), gpui::px(700.))), Some(gpui::px(40.)));
    }

    #[test]
    fn words_lines_and_google_query() {
        assert_eq!(word_at("salut les amis", 7), 6..9);
        assert_eq!(word_at("salut les amis", 5), 0..5);
        assert_eq!(line_at("un\ndeux trois\nquatre", 5), 3..13);
        assert_eq!(google_url("é a&b"), "https://www.google.com/search?q=%C3%A9%20a%26b");
    }

    #[test]
    fn highlight_splits_runs_at_the_selection_edges() {
        let run = |len: usize| TextRun { len, font: gpui::font("x"), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
        let out = paint_runs(&[run(4), run(4)], 2..6, gpui::white());
        let lens: Vec<usize> = out.iter().map(|r| r.len).collect();
        assert_eq!(lens, vec![2, 2, 2, 2]);
        let painted: Vec<bool> = out.iter().map(|r| r.background_color.is_some()).collect();
        assert_eq!(painted, vec![false, true, true, false]);
    }

    #[test]
    fn dragging_past_the_text_lands_on_the_cell_before_the_pointer() {
        let b = |x: f32, y: f32| Bounds { origin: gpui::point(gpui::px(x), gpui::px(y)), size: gpui::size(gpui::px(40.), gpui::px(20.)) };
        let cells = vec![(p(1, 0, 0), b(0., 0.), p(1, 0, 5)), (p(1, 0, 6), b(50., 0.), p(1, 0, 11)), (p(2, 0, 0), b(0., 30.), p(2, 0, 4))];
        assert_eq!(nearest_in(&cells, gpui::point(gpui::px(200.), gpui::px(10.))), Some(p(1, 0, 11)));
        assert_eq!(nearest_in(&cells, gpui::point(gpui::px(200.), gpui::px(60.))), Some(p(2, 0, 4)));
        assert_eq!(nearest_in(&cells, gpui::point(gpui::px(-5.), gpui::px(-5.))), Some(p(1, 0, 0)));
    }

    #[test]
    fn copies_author_body_and_embed_in_visual_order() {
        begin_frame(Some(7));
        note(100, AUTHOR_BLOCK, "Alice".into(), &[]);
        note(100, BODY_BASE, "hello".into(), &[]);
        note(100, EMBED_BASE + 1, "Embed Title".into(), &[]);
        note(100, EMBED_BASE + 10, "Embed Desc".into(), &[]);
        set_for_test(Some(7), p(100, AUTHOR_BLOCK, 0), p(100, EMBED_BASE + 10, 10));
        assert_eq!(selected_text().as_deref(), Some("Alice\nhello\nEmbed Title\nEmbed Desc"));
    }
}
