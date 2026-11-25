use std::ops::Range;

use gpui::{FontStyle, FontWeight, Hsla, Pixels, RichText, SharedString, TextBox, TextRun, px};

use crate::input::RopeExt;
use ropey::Rope;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RichSpan {
    pub range: Range<usize>,
    pub font_family: Option<SharedString>,
    pub font_size: Option<Pixels>,
    pub text_box: Option<TextBox>,
    pub hidden: bool,
}

impl RichSpan {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            ..Default::default()
        }
    }

    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn font_size(mut self, size: Pixels) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn text_box(mut self, text_box: TextBox) -> Self {
        self.text_box = Some(text_box);
        self
    }

    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineBar {
    pub color: Hsla,
    pub width: Pixels,
    pub radius: Pixels,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct LineStyle {
    pub range: Range<usize>,
    pub bar: Option<LineBar>,
    pub indent: Pixels,
    pub end_inset: Pixels,
    pub font_family: Option<SharedString>,
    pub font_size: Option<Pixels>,
}

impl LineStyle {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            ..Default::default()
        }
    }

    pub fn bar(mut self, color: Hsla, width: Pixels, radius: Pixels) -> Self {
        self.bar = Some(LineBar {
            color,
            width,
            radius,
        });
        self
    }

    pub fn indent(mut self, indent: Pixels) -> Self {
        self.indent = indent;
        self
    }

    pub fn end_inset(mut self, inset: Pixels) -> Self {
        self.end_inset = inset;
        self
    }

    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn font_size(mut self, size: Pixels) -> Self {
        self.font_size = Some(size);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RichDecorations {
    pub(crate) spans: Vec<RichSpan>,
    pub(crate) lines: Vec<LineStyle>,
    pub(crate) faces: Vec<(Range<usize>, Option<FontWeight>, Option<FontStyle>)>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct LineView {
    pub(crate) style: Option<LineStyle>,
    pub(crate) rich: RichText,
    pub(crate) families: Vec<(Range<usize>, SharedString)>,
    pub(crate) faces: Vec<(Range<usize>, Option<FontWeight>, Option<FontStyle>)>,
}

impl LineView {
    pub(crate) fn is_plain(&self) -> bool {
        self.style.is_none() && self.rich.is_empty() && self.families.is_empty()
    }

    pub(crate) fn font_size(&self, base: Pixels) -> Pixels {
        self.style
            .as_ref()
            .and_then(|s| s.font_size)
            .unwrap_or(base)
    }

    pub(crate) fn indent(&self) -> Pixels {
        self.style.as_ref().map_or(px(0.), |s| s.indent)
    }

    pub(crate) fn end_inset(&self) -> Pixels {
        self.style.as_ref().map_or(px(0.), |s| s.end_inset)
    }

    pub(crate) fn runs(&self, runs: &[TextRun], range: &Range<usize>, faces: bool) -> Vec<TextRun> {
        let family = self.style.as_ref().and_then(|s| s.font_family.clone());
        let mut cuts: Vec<usize> = vec![range.start, range.end];
        let mut at = range.start;
        for run in runs {
            at += run.len;
            cuts.push(at.min(range.end));
        }
        let mut add = |r: &Range<usize>| {
            cuts.push(r.start.clamp(range.start, range.end));
            cuts.push(r.end.clamp(range.start, range.end));
        };
        for (r, _) in &self.families {
            add(r);
        }
        if faces {
            for (r, _, _) in &self.faces {
                add(r);
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        let mut out: Vec<TextRun> = Vec::with_capacity(cuts.len());
        let mut run_ix = 0;
        let mut run_end = range.start + runs.first().map_or(0, |r| r.len);
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            if start >= end {
                continue;
            }
            while run_end <= start && run_ix + 1 < runs.len() {
                run_ix += 1;
                run_end += runs[run_ix].len;
            }
            let Some(base) = runs.get(run_ix) else {
                break;
            };
            let mut run = TextRun {
                len: end - start,
                ..base.clone()
            };
            if let Some(family) = &family {
                run.font.family = family.clone();
            }
            if let Some((_, f)) = self
                .families
                .iter()
                .find(|(r, _)| r.start <= start && start < r.end)
            {
                run.font.family = f.clone();
            }
            if faces {
                for (r, weight, style) in &self.faces {
                    if r.start <= start && start < r.end {
                        if let Some(w) = weight {
                            run.font.weight = *w;
                        }
                        if let Some(s) = style {
                            run.font.style = *s;
                        }
                    }
                }
            }
            if let Some(last) = out.last_mut()
                && last.font == run.font
                && last.color == run.color
                && last.background_color == run.background_color
                && last.underline == run.underline
                && last.strikethrough == run.strikethrough
            {
                last.len += run.len;
            } else {
                out.push(run);
            }
        }
        out
    }
}

fn local(range: &Range<usize>, line: &Range<usize>) -> Option<Range<usize>> {
    let start = range.start.max(line.start);
    let end = range.end.min(line.end);
    (start < end).then(|| start - line.start..end - line.start)
}

impl RichDecorations {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty() && self.lines.is_empty()
    }

    pub fn spans(&self) -> &[RichSpan] {
        &self.spans
    }

    pub fn lines(&self) -> &[LineStyle] {
        &self.lines
    }

    pub(crate) fn new(
        text: &Rope,
        mut spans: Vec<RichSpan>,
        mut lines: Vec<LineStyle>,
        faces: Vec<(Range<usize>, Option<FontWeight>, Option<FontStyle>)>,
    ) -> Self {
        let len = text.len();
        let fix = |r: &mut Range<usize>| {
            r.start = text.clip_offset(r.start.min(len), sum_tree::Bias::Left);
            r.end = text.clip_offset(r.end.min(len), sum_tree::Bias::Right);
        };
        spans.iter_mut().for_each(|s| fix(&mut s.range));
        spans.retain(|s| !s.range.is_empty());
        spans.sort_by_key(|s| (s.range.start, s.range.end));
        lines.iter_mut().for_each(|l| {
            l.range.start = l.range.start.min(len);
            l.range.end = l.range.end.min(len).max(l.range.start);
        });
        lines.sort_by_key(|l| l.range.start);
        let mut faces = faces;
        faces.retain(|(r, w, s)| r.end <= len && !r.is_empty() && (w.is_some() || s.is_some()));
        Self {
            spans,
            lines,
            faces,
        }
    }

    pub(crate) fn adjust_for_edit(&mut self, edited: &Range<usize>, inserted: usize) {
        let adjust = |r: &Range<usize>| -> Range<usize> {
            let shift = |p: usize| -> usize {
                if p <= edited.start {
                    p
                } else if p >= edited.end {
                    p - edited.len() + inserted
                } else {
                    edited.start + inserted
                }
            };
            shift(r.start)..shift(r.end)
        };
        self.spans.retain_mut(|s| {
            s.range = adjust(&s.range);
            !s.range.is_empty()
        });
        for l in &mut self.lines {
            l.range = adjust(&l.range);
        }
        self.faces.retain_mut(|(r, _, _)| {
            *r = adjust(&*r);
            !r.is_empty()
        });
    }

    pub(crate) fn line_view(&self, line: Range<usize>) -> LineView {
        let style = self
            .lines
            .iter()
            .find(|l| l.range.start <= line.start && line.start <= l.range.end)
            .map(|l| LineStyle {
                range: 0..0,
                ..l.clone()
            });
        let first = self.spans.partition_point(|s| s.range.end <= line.start);
        let mut rich = RichText::default();
        let mut families = Vec::new();
        for span in self.spans[first..]
            .iter()
            .take_while(|s| s.range.start < line.end.max(line.start + 1))
        {
            let Some(r) = local(&span.range, &line) else {
                continue;
            };
            if let Some(size) = span.font_size {
                rich.sizes.push((r.clone(), size));
            }
            if let Some(family) = &span.font_family {
                families.push((r.clone(), family.clone()));
            }
            if let Some(b) = &span.text_box {
                let mut b = b.clone();
                b.open_start |= span.range.start < line.start;
                b.open_end |= span.range.end > line.end;
                b.range = r.clone();
                rich.boxes.push(b);
            }
            if span.hidden {
                rich.hidden.push(r);
            }
        }
        let faces = if rich.is_empty() && style.is_none() && families.is_empty() {
            Vec::new()
        } else {
            self.faces
                .iter()
                .filter_map(|(r, w, s)| local(r, &line).map(|r| (r, *w, *s)))
                .collect()
        };
        LineView {
            style,
            rich,
            families,
            faces,
        }
    }

    pub(crate) fn touched_rows(&self, text: &Rope) -> Vec<usize> {
        let mut rows: Vec<usize> = Vec::new();
        let len = text.len();
        let mut push = |r: &Range<usize>| {
            let a = text.offset_to_point(r.start.min(len)).row;
            let b = text.offset_to_point(r.end.min(len)).row;
            rows.extend(a..=b);
        };
        self.spans.iter().for_each(|s| push(&s.range));
        self.lines.iter().for_each(|l| push(&l.range));
        rows.sort_unstable();
        rows.dedup();
        rows
    }
}
