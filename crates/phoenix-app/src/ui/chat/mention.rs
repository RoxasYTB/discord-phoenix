use std::cell::Cell;
use std::ops::Range;

use gpui::{App, Entity, TextRun, Window};

use crate::{
    app::PhoenixApp,
    markdown::Mention,
    model::Id,
    richtext::ClickTarget,
    theme::{c, generated::idx, tok},
};

thread_local! {
    static HOVER: Cell<Option<(u64, usize, u64, usize)>> = const { Cell::new(None) };
    static HOVER_LINK: std::cell::RefCell<Option<(u64, usize, u64, gpui::SharedString)>> = const { std::cell::RefCell::new(None) };
    static HOVER_PILL: Cell<Option<(u64, usize, usize, u64)>> = const { Cell::new(None) };
    static ANCHOR: std::cell::RefCell<Option<(u64, usize, Range<usize>)>> = const { std::cell::RefCell::new(None) };
}

pub fn anchor_at(ranges: &[Range<usize>], targets: &[ClickTarget], ix: usize) -> Option<(Range<usize>, gpui::SharedString)> {
    let k = ranges.iter().position(|r| r.contains(&ix))?;
    let ClickTarget::Link(url) = &targets[k] else { return None };
    let same = |j: usize| matches!(&targets[j], ClickTarget::Link(u) if u == url);
    let (mut a, mut b) = (k, k);
    while a > 0 && same(a - 1) && ranges[a - 1].end == ranges[a].start {
        a -= 1;
    }
    while b + 1 < ranges.len() && same(b + 1) && ranges[b + 1].start == ranges[b].end {
        b += 1;
    }
    Some((ranges[a].start..ranges[b].end, url.clone()))
}

pub fn set_anchor(msg: Id, bi: usize, scope: Range<usize>, r: Option<Range<usize>>, window: &mut Window) {
    let changed = ANCHOR.with(|a| {
        let mut a = a.borrow_mut();
        let mine = a.as_ref().is_some_and(|(m, b, cur)| *m == msg.0 && *b == bi && cur.start < scope.end && scope.start < cur.end);
        let new = r.map(|r| (msg.0, bi, r));
        if *a != new && (new.is_some() || mine) {
            *a = new;
            true
        } else {
            false
        }
    });
    if changed {
        window.refresh();
    }
}

pub fn anchor_runs(msg: Id, bi: usize, offset: usize, runs: &[TextRun]) -> Option<Vec<TextRun>> {
    let r = ANCHOR.with(|a| a.borrow().as_ref().filter(|(m, b, _)| *m == msg.0 && *b == bi).map(|(_, _, r)| r.clone()))?;
    let mut out = runs.to_vec();
    let mut at = offset;
    for run in &mut out {
        let span = at..at + run.len;
        if span.start < r.end && r.start < span.end {
            run.underline = Some(gpui::UnderlineStyle { thickness: gpui::px(1.), color: Some(run.color), wavy: false });
        }
        at = span.end;
    }
    Some(out)
}

pub fn link_title(label: &str, url: &str) -> gpui::SharedString {
    if label == url {
        return label.to_owned().into();
    }
    let mut suffix = format!("\n\n({url})");
    let mut title = label.to_owned();
    if title.chars().count() + suffix.chars().count() > 1024 {
        suffix = format!("...{suffix}");
        let keep = 1024usize.saturating_sub(suffix.chars().count());
        title = title.chars().take(keep).collect::<String>().trim_end().to_owned();
    }
    format!("{title}{suffix}").into()
}

pub fn hovered_pill(msg: Id, bi: usize) -> Option<usize> {
    HOVER_PILL.with(Cell::get).filter(|(m, b, _, _)| *m == msg.0 && *b == bi).map(|(_, _, s, _)| s)
}

pub fn hover_pill(msg: Id, bi: usize, start: usize, part: u64, hovered: bool, window: &mut Window) {
    let cur = HOVER_PILL.with(Cell::get);
    let new = if hovered {
        Some((msg.0, bi, start, part))
    } else if cur == Some((msg.0, bi, start, part)) {
        None
    } else {
        cur
    };
    if new != cur {
        HOVER_PILL.with(|h| h.set(new));
        window.refresh();
    }
}

pub fn pill_runs(runs: &[TextRun], r: &Range<usize>, role: bool) -> Vec<TextRun> {
    let mut out = runs.to_vec();
    let mut at = 0;
    for run in &mut out {
        let span = at..at + run.len;
        if span.start < r.end && r.start < span.end {
            match run.background_color {
                Some(bg) if role && is_role_tint(bg, run.color) => run.background_color = Some(gpui::Hsla { a: 0.3, ..bg }),
                _ => {
                    run.background_color = Some(c(tok(idx::BRAND_500)));
                    run.color = c(tok(idx::WHITE));
                }
            }
        }
        at = span.end;
    }
    out
}

pub fn hovered_link(msg: Id, bi: usize, seed: u64) -> Option<gpui::SharedString> {
    HOVER_LINK.with(|h| h.borrow().as_ref().filter(|(m, b, s, _)| *m == msg.0 && *b == bi && *s == seed).map(|(_, _, _, u)| u.clone()))
}

pub fn on_hover(
    msg: Id,
    bi: usize,
    seed: u64,
    ranges: &[Range<usize>],
    targets: &[ClickTarget],
    ix: Option<usize>,
    window: &mut Window,
) {
    let at = ix.and_then(|i| ranges.iter().position(|r| r.contains(&i)));
    HOVER_LINK.with(|h| {
        let mut h = h.borrow_mut();
        match at.map(|k| &targets[k]) {
            Some(ClickTarget::Link(url)) => *h = Some((msg.0, bi, seed, url.clone())),
            _ if h.as_ref().is_some_and(|(m, b, s, _)| *m == msg.0 && *b == bi && *s == seed) => *h = None,
            _ => {}
        }
    });
    let hit = at.filter(|&k| matches!(targets[k], ClickTarget::Mention(_)));
    let new = hit.map(|k| (msg.0, bi, seed, k));
    let cur = HOVER.with(Cell::get);
    let mine = cur.is_some_and(|(m, b, s, _)| m == msg.0 && b == bi && s == seed);
    if new != cur && (new.is_some() || mine) {
        HOVER.with(|h| h.set(new));
        window.refresh();
    }
}

pub fn hovered_runs(
    msg: Id,
    bi: usize,
    seed: u64,
    ranges: &[Range<usize>],
    targets: &[ClickTarget],
    runs: &[TextRun],
) -> Option<Vec<TextRun>> {
    let (m, b, s, k) = HOVER.with(Cell::get)?;
    if m != msg.0 || b != bi || s != seed {
        return None;
    }
    let r = ranges.get(k)?.clone();
    let role = matches!(targets.get(k), Some(ClickTarget::Mention(Mention::Role(_))));
    Some(pill_runs(runs, &r, role))
}

fn is_role_tint(bg: gpui::Hsla, fg: gpui::Hsla) -> bool {
    (bg.a - 0.1).abs() < 0.01 && (bg.h - fg.h).abs() < 1e-3 && (bg.s - fg.s).abs() < 1e-3 && (bg.l - fg.l).abs() < 1e-3
}

pub fn click(entity: &Entity<PhoenixApp>, m: &Mention, window: &mut Window, cx: &mut App) {
    match *m {
        Mention::User(u) => {
            let at = window.mouse_position();
            entity.update(cx, |a, cx| {
                let g = a.selected_guild;
                a.open_profile(Id(u), g, at, window, cx);
            });
        }
        Mention::Channel(ch) => entity.update(cx, |a, cx| {
            if a.channels.contains_key(&Id(ch)) {
                a.open_channel_chat(Id(ch), cx);
            }
        }),
        Mention::Role(_) | Mention::Everyone | Mention::Here => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_spans_the_whole_masked_link() {
        let l = |u: &str| ClickTarget::Link(u.to_owned().into());
        let ranges = vec![0..4, 4..9, 10..15, 15..20];
        let targets = vec![l("https://a.b"), l("https://a.b"), l("https://a.b"), l("https://c.d")];
        assert_eq!(anchor_at(&ranges, &targets, 6).map(|a| a.0), Some(0..9));
        assert_eq!(anchor_at(&ranges, &targets, 12).map(|a| a.0), Some(10..15));
        assert_eq!(anchor_at(&ranges, &targets, 9), None);
        assert_eq!(anchor_at(&[0..3], &[ClickTarget::Spoiler(0)], 1), None);
    }

    #[test]
    fn link_title_follows_the_client() {
        assert_eq!(link_title("https://a.b", "https://a.b").as_ref(), "https://a.b");
        assert_eq!(link_title("docs", "https://a.b").as_ref(), "docs\n\n(https://a.b)");
        let long = "x".repeat(2000);
        let t = link_title(&long, "https://a.b");
        assert_eq!(t.chars().count(), 1024);
        assert!(t.ends_with("...\n\n(https://a.b)"));
    }

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            font: gpui::font("x"),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn hover_paints_only_the_mention_run() {
        let ranges = std::slice::from_ref(&(3..8)).to_vec();
        let targets = vec![ClickTarget::Mention(Mention::User(1))];
        HOVER.with(|h| h.set(Some((7, 0, 0, 0))));
        let out = hovered_runs(Id(7), 0, 0, &ranges, &targets, &[run(3), run(5), run(2)]).unwrap();
        assert!(out[0].background_color.is_none());
        assert!(out[1].background_color.is_some());
        assert!(out[2].background_color.is_none());
        assert!(hovered_runs(Id(8), 0, 0, &ranges, &targets, &[run(3)]).is_none());
        HOVER.with(|h| h.set(None));
    }

    #[test]
    fn coloured_role_hover_raises_the_tint() {
        let ranges = vec![0..4, 4..8];
        let targets = vec![ClickTarget::Mention(Mention::Role(1)), ClickTarget::Mention(Mention::Role(2))];
        let orange: gpui::Hsla = gpui::rgb(0xe67e22).into();
        let mut tinted = run(4);
        tinted.color = orange;
        tinted.background_color = Some(gpui::Hsla { a: 0.1, ..orange });
        let runs = [tinted, run(4)];
        HOVER.with(|h| h.set(Some((7, 0, 0, 0))));
        let out = hovered_runs(Id(7), 0, 0, &ranges, &targets, &runs).unwrap();
        assert_eq!(out[0].color, orange);
        assert!((out[0].background_color.unwrap().a - 0.3).abs() < 1e-6);
        HOVER.with(|h| h.set(Some((7, 0, 0, 1))));
        let out = hovered_runs(Id(7), 0, 0, &ranges, &targets, &runs).unwrap();
        assert_eq!(out[1].color, c(tok(idx::WHITE)));
        HOVER.with(|h| h.set(None));
    }
}
