#![allow(missing_docs)]

use crate::{
    Bounds, BorderStyle, Corners, Edges, FontId, Hsla, LineLayout, Pixels, Point, ShapedGlyph,
    ShapedRun, Window, point, px, quad, size,
};
use smallvec::SmallVec;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TextBox {
    pub range: Range<usize>,
    pub background: Option<Hsla>,
    pub border_color: Option<Hsla>,
    pub border_width: Pixels,
    pub corner_radius: Pixels,
    pub padding: Pixels,
    pub clone_edges: bool,
    pub open_start: bool,
    pub open_end: bool,
}

impl TextBox {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            ..Default::default()
        }
    }

    pub fn background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }

    pub fn border(mut self, width: Pixels, color: Hsla) -> Self {
        self.border_width = width;
        self.border_color = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: Pixels) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = padding;
        self
    }

    pub fn clone_edges(mut self, clone: bool) -> Self {
        self.clone_edges = clone;
        self
    }

    fn start_space(&self) -> Pixels {
        if self.open_start {
            px(0.)
        } else {
            self.padding + self.border_width
        }
    }

    fn end_space(&self) -> Pixels {
        if self.open_end {
            px(0.)
        } else {
            self.padding + self.border_width
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RichText {
    pub sizes: Vec<(Range<usize>, Pixels)>,
    pub boxes: Vec<TextBox>,
    pub hidden: Vec<Range<usize>>,
}

impl RichText {
    pub fn is_empty(&self) -> bool {
        self.sizes.is_empty() && self.boxes.is_empty() && self.hidden.is_empty()
    }

    pub fn size_at(&self, index: usize) -> Option<Pixels> {
        self.sizes
            .iter()
            .find(|(range, _)| range.start <= index && index < range.end)
            .map(|(_, size)| *size)
    }

    pub fn slice(&self, range: Range<usize>) -> RichText {
        let clip = |r: &Range<usize>| -> Option<Range<usize>> {
            let start = r.start.max(range.start);
            let end = r.end.min(range.end);
            (start < end).then(|| start - range.start..end - range.start)
        };
        RichText {
            sizes: self
                .sizes
                .iter()
                .filter_map(|(r, s)| clip(r).map(|r| (r, *s)))
                .collect(),
            boxes: self
                .boxes
                .iter()
                .filter_map(|b| {
                    let r = clip(&b.range)?;
                    let cut_start = b.range.start < range.start;
                    let cut_end = b.range.end > range.end;
                    Some(TextBox {
                        range: r,
                        open_start: b.open_start || (cut_start && !b.clone_edges),
                        open_end: b.open_end || (cut_end && !b.clone_edges),
                        ..b.clone()
                    })
                })
                .collect(),
            hidden: self.hidden.iter().filter_map(clip).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaintBox {
    pub(crate) left: Pixels,
    pub(crate) right: Pixels,
    pub(crate) ascent: Pixels,
    pub(crate) descent: Pixels,
    pub(crate) style: TextBox,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct RichPaint {
    pub(crate) run_sizes: Vec<Pixels>,
    pub(crate) boxes: SmallVec<[PaintBox; 2]>,
}

pub(crate) struct Segment {
    pub(crate) range: Range<usize>,
    pub(crate) font_size: Pixels,
}

pub(crate) fn segments(len: usize, base: Pixels, rich: &RichText) -> Vec<Segment> {
    let mut cuts: Vec<usize> = vec![0, len];
    for (r, _) in &rich.sizes {
        cuts.push(r.start.min(len));
        cuts.push(r.end.min(len));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<Segment> = Vec::new();
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        if start >= end {
            continue;
        }
        let font_size = rich.size_at(start).unwrap_or(base);
        match out.last_mut() {
            Some(last) if last.font_size == font_size && last.range.end == start => {
                last.range.end = end;
            }
            _ => out.push(Segment {
                range: start..end,
                font_size,
            }),
        }
    }
    out
}

pub(crate) fn append_segment(
    merged: &mut LineLayout,
    run_sizes: &mut Vec<Pixels>,
    piece: &LineLayout,
    start: usize,
    font_size: Pixels,
) {
    let pen = merged.width;
    for run in &piece.runs {
        merged.runs.push(ShapedRun {
            font_id: run.font_id,
            glyphs: run
                .glyphs
                .iter()
                .map(|g| ShapedGlyph {
                    id: g.id,
                    position: point(g.position.x + pen, g.position.y),
                    index: g.index + start,
                    is_emoji: g.is_emoji,
                })
                .collect(),
        });
        run_sizes.push(font_size);
    }
    merged.width += piece.width;
    merged.ascent = merged.ascent.max(piece.ascent);
    merged.descent = merged.descent.max(piece.descent);
    merged.len = start + piece.len;
}

pub(crate) fn apply_spacing(
    layout: &mut LineLayout,
    run_sizes: &mut Vec<Pixels>,
    rich: &RichText,
    metrics: impl Fn(FontId, Pixels) -> (Pixels, Pixels),
) -> SmallVec<[PaintBox; 2]> {
    let mut events: Vec<(usize, u8, Pixels)> = Vec::new();
    for b in &rich.boxes {
        events.push((b.range.end, 0, b.end_space()));
        events.push((b.range.start, 1, b.start_space()));
    }
    for h in &rich.hidden {
        let width = layout.x_for_index(h.end) - layout.x_for_index(h.start);
        events.push((h.end, 0, -width));
    }
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let shift_through = |index: usize| -> Pixels {
        events
            .iter()
            .filter(|(at, _, _)| *at <= index)
            .map(|(_, _, w)| *w)
            .fold(px(0.), |a, b| a + b)
    };
    let shift_until = |index: usize| -> Pixels {
        events
            .iter()
            .filter(|(at, kind, _)| *at < index || (*at == index && *kind == 0))
            .map(|(_, _, w)| *w)
            .fold(px(0.), |a, b| a + b)
    };
    let shift_strict = |index: usize| -> Pixels {
        events
            .iter()
            .filter(|(at, _, _)| *at < index)
            .map(|(_, _, w)| *w)
            .fold(px(0.), |a, b| a + b)
    };
    let mut boxes: SmallVec<[PaintBox; 2]> = SmallVec::new();
    for b in &rich.boxes {
        let left = layout.x_for_index(b.range.start) + shift_until(b.range.start);
        let right = layout.x_for_index(b.range.end) + shift_strict(b.range.end) + b.end_space();
        let (run_ix, font_id) = layout
            .runs
            .iter()
            .enumerate()
            .find(|(_, run)| run.glyphs.iter().any(|g| g.index >= b.range.start))
            .map(|(ix, run)| (ix, run.font_id))
            .unwrap_or((0, layout.runs.first().map_or(FontId(0), |r| r.font_id)));
        let font_size = run_sizes.get(run_ix).copied().unwrap_or(layout.font_size);
        let (ascent, descent) = metrics(font_id, font_size);
        boxes.push(PaintBox {
            left,
            right,
            ascent,
            descent,
            style: b.clone(),
        });
    }
    let total = events.iter().map(|(_, _, w)| *w).fold(px(0.), |a, b| a + b);
    let hidden = &rich.hidden;
    let mut kept_sizes = Vec::with_capacity(run_sizes.len());
    let mut runs = Vec::with_capacity(layout.runs.len());
    for (ix, mut run) in std::mem::take(&mut layout.runs).into_iter().enumerate() {
        run.glyphs
            .retain(|g| !hidden.iter().any(|h| h.start <= g.index && g.index < h.end));
        for g in &mut run.glyphs {
            g.position.x += shift_through(g.index);
        }
        if !run.glyphs.is_empty() {
            runs.push(run);
            kept_sizes.push(run_sizes.get(ix).copied().unwrap_or(layout.font_size));
        }
    }
    layout.runs = runs;
    *run_sizes = kept_sizes;
    layout.width += total;
    boxes
}

pub(crate) fn paint_boxes(
    origin: Point<Pixels>,
    baseline: Pixels,
    boxes: &[PaintBox],
    window: &mut Window,
) {
    for b in boxes {
        let s = &b.style;
        let top = origin.y + baseline - b.ascent - s.border_width;
        let bottom = origin.y + baseline + b.descent + s.border_width;
        let bounds = Bounds::new(
            point(origin.x + b.left, top),
            size(b.right - b.left, bottom - top),
        );
        let r = s.corner_radius;
        let start_r = if s.open_start { px(0.) } else { r };
        let end_r = if s.open_end { px(0.) } else { r };
        let corners = Corners {
            top_left: start_r,
            bottom_left: start_r,
            top_right: end_r,
            bottom_right: end_r,
        };
        let bw = s.border_width;
        let edges = Edges {
            top: bw,
            bottom: bw,
            left: if s.open_start { px(0.) } else { bw },
            right: if s.open_end { px(0.) } else { bw },
        };
        let has_border = s.border_color.is_some() && bw > px(0.);
        window.paint_quad(quad(
            bounds,
            corners,
            s.background.unwrap_or_default(),
            if has_border { edges } else { Edges::default() },
            s.border_color.unwrap_or_default(),
            BorderStyle::Solid,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppContext as _, Context, IntoElement, Render, SharedString, Styled, TestAppContext, TextRun,
        WindowTextSystem, black, canvas, font, hsla,
    };

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            font: font(".ZedMono"),
            color: black(),
            ..TextRun::default()
        }
    }

    fn system(cx: &mut TestAppContext) -> WindowTextSystem {
        cx.update(|cx| WindowTextSystem::new(cx.text_system().clone()))
    }

    fn close(a: Pixels, b: f32) {
        assert!((f32::from(a) - b).abs() < 0.01, "{a:?} != {b}");
    }

    #[gpui::test]
    fn sizes_lay_out_each_segment_at_its_own_size(cx: &mut TestAppContext) {
        let ts = system(cx);
        let head = f32::from(ts.layout_line("ab ", px(16.), &[run(3)], None).width);
        let c = f32::from(ts.layout_line("c", px(8.), &[run(1)], None).width);
        let tail = f32::from(ts.layout_line("cd", px(8.), &[run(2)], None).width);
        let rich = RichText {
            sizes: vec![(3..5, px(8.))],
            ..Default::default()
        };
        let line = ts.shape_line_rich("ab cd".into(), px(16.), &[run(5)], &rich, None);
        close(line.x_for_index(3), head);
        close(line.x_for_index(4), head + c);
        close(line.width(), head + tail);
        let paint = line.rich.as_ref().unwrap();
        assert_eq!(paint.run_sizes, vec![px(16.), px(8.)]);
        assert_eq!(line.len(), 5);
        let measured = ts.layout_line_rich("ab cd", px(16.), &[run(5)], &rich);
        close(measured.width, head + tail);
    }

    #[gpui::test]
    fn boxes_add_their_padding_and_border_to_the_advance(cx: &mut TestAppContext) {
        let ts = system(cx);
        let base = ts.layout_line("abcd", px(16.), &[run(4)], None);
        let x = |i: usize| f32::from(base.x_for_index(i));
        let rich = RichText {
            boxes: vec![TextBox::new(1..3).padding(px(2.)).border(px(1.), black())],
            ..Default::default()
        };
        let line = ts.shape_line_rich("abcd".into(), px(16.), &[run(4)], &rich, None);
        close(line.x_for_index(1), x(1) + 3.);
        close(line.x_for_index(3), x(3) + 6.);
        close(line.width(), f32::from(base.width) + 6.);
        let b = &line.rich.as_ref().unwrap().boxes[0];
        close(b.left, x(1));
        close(b.right, x(3) + 6.);
        assert!(b.ascent > px(0.) && b.descent >= px(0.));
    }

    #[gpui::test]
    fn adjacent_boxes_do_not_overlap(cx: &mut TestAppContext) {
        let ts = system(cx);
        let rich = RichText {
            boxes: vec![
                TextBox::new(0..2).padding(px(2.)),
                TextBox::new(2..4).padding(px(3.)),
            ],
            ..Default::default()
        };
        let line = ts.shape_line_rich("abcd".into(), px(16.), &[run(4)], &rich, None);
        let boxes = &line.rich.as_ref().unwrap().boxes;
        close(boxes[0].right, f32::from(boxes[1].left));
        close(line.x_for_index(2), f32::from(boxes[1].left) + 3.);
    }

    #[test]
    fn slicing_opens_cut_edges_unless_cloned() {
        let rich = RichText {
            sizes: vec![(0..6, px(13.))],
            boxes: vec![
                TextBox::new(1..5).padding(px(2.)),
                TextBox::new(1..5).padding(px(2.)).clone_edges(true),
            ],
            hidden: vec![0..2],
        };
        let part = rich.slice(3..6);
        assert_eq!(part.sizes, vec![(0..3, px(13.))]);
        assert_eq!(part.boxes[0].range, 0..2);
        assert!(part.boxes[0].open_start && !part.boxes[0].open_end);
        assert!(!part.boxes[1].open_start && !part.boxes[1].open_end);
        assert!(part.hidden.is_empty());
        let head = rich.slice(0..3);
        assert!(!head.boxes[0].open_start && head.boxes[0].open_end);
        assert_eq!(head.hidden, vec![0..2]);
    }

    #[gpui::test]
    fn hidden_text_takes_no_space(cx: &mut TestAppContext) {
        let ts = system(cx);
        let base = ts.layout_line("> quote", px(16.), &[run(7)], None);
        let x = |i: usize| f32::from(base.x_for_index(i));
        let rich = RichText {
            hidden: vec![0..2],
            ..Default::default()
        };
        let line = ts.shape_line_rich("> quote".into(), px(16.), &[run(7)], &rich, None);
        close(line.x_for_index(0), 0.);
        close(line.x_for_index(2), 0.);
        close(line.x_for_index(3), x(3) - x(2));
        close(line.width(), f32::from(base.width) - x(2));
        assert_eq!(line.index_for_x(px(0.5)), Some(2));
        let glyphs: usize = line.runs.iter().map(|r| r.glyphs.len()).sum();
        assert_eq!(glyphs, 5);
    }

    #[gpui::test]
    fn empty_rich_text_is_the_plain_shape(cx: &mut TestAppContext) {
        let ts = system(cx);
        let line = ts.shape_line_rich("abc".into(), px(16.), &[run(3)], &RichText::default(), None);
        assert!(line.rich.is_none());
        let plain = ts.shape_line(SharedString::new("abc"), px(16.), &[run(3)], None);
        assert_eq!(line.width(), plain.width());
    }

    struct BoxView;

    impl Render for BoxView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            canvas(
                |_, _, _| {},
                |_, _, window, cx| {
                    let rich = RichText {
                        sizes: vec![(1..3, px(13.6))],
                        boxes: vec![
                            TextBox::new(1..3)
                                .background(hsla(0.5, 0.5, 0.5, 1.))
                                .border(px(1.), black())
                                .corner_radius(px(4.)),
                        ],
                        ..Default::default()
                    };
                    let line = window.text_system().shape_line_rich(
                        "a`b`c".into(),
                        px(16.),
                        &[run(5)],
                        &rich,
                        None,
                    );
                    line.paint_background(
                        point(px(10.), px(10.)),
                        px(22.),
                        crate::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .unwrap();
                },
            )
            .size_full()
        }
    }

    #[gpui::test]
    fn boxes_paint_one_rounded_bordered_quad(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| BoxView);
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            let quads = &window.rendered_frame.scene.quads;
            let quad = quads
                .iter()
                .find(|q| q.border_widths.left.0 > 0.)
                .expect("box quad");
            assert!(quad.corner_radii.top_left.0 > 0.);
            assert!(quad.bounds.size.height.0 > 0.);
        })
        .unwrap();
    }
}
