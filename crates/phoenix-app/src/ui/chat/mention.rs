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
    Some(out)
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
