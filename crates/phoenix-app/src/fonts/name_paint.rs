use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels,
    SharedString, Style, TextAlign, Window, point, px, relative,
};

pub struct NameText {
    text: SharedString,
    spacing: Pixels,
    colors: Vec<Hsla>,
    period: Option<Pixels>,
}

impl NameText {
    pub fn new(text: SharedString, spacing: Pixels, colors: Vec<Hsla>, period: Option<Pixels>) -> Self {
        Self { text, spacing, colors, period }
    }
}

pub fn gradient_at(colors: &[Hsla], t: f32) -> Option<Hsla> {
    match colors {
        [] => None,
        [c] => Some(*c),
        _ => {
            let t = t.clamp(0., 1.) * (colors.len() - 1) as f32;
            let i = (t.floor() as usize).min(colors.len() - 2);
            let f = t - i as f32;
            let (a, b) = (gpui::Rgba::from(colors[i]), gpui::Rgba::from(colors[i + 1]));
            let l = |x: f32, y: f32| x + (y - x) * f;
            Some(gpui::Rgba { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: l(a.a, b.a) }.into())
        }
    }
}

pub struct Layout {
    chars: Vec<(String, Pixels)>,
    ellipsis: Pixels,
    line_height: Pixels,
    underline: Option<(Pixels, Pixels, Pixels, gpui::UnderlineStyle)>,
}

impl NameText {
    fn shape(&self, s: &str, window: &mut Window, color: Hsla) -> gpui::ShapedLine {
        let style = window.text_style();
        let mut run = style.to_run(s.len());
        run.color = color;
        run.underline = None;
        let size = style.font_size.to_pixels(window.rem_size());
        window.text_system().shape_line(SharedString::from(s.to_owned()), size, &[run], None)
    }
}

impl Element for NameText {
    type RequestLayoutState = Layout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Layout) {
        let color = window.text_style().color;
        let line_height = window.text_style().line_height_in_pixels(window.rem_size());
        let full = self.shape(&self.text, window, color);
        let text = self.text.clone();
        let starts: Vec<(usize, &str)> = unicode_segmentation::UnicodeSegmentation::grapheme_indices(text.as_ref(), true).collect();
        let per: Vec<(String, Pixels)> = starts
            .iter()
            .enumerate()
            .map(|(i, (b, g))| {
                let x0 = full.x_for_index(*b);
                let x1 = starts.get(i + 1).map_or(full.width, |(nb, _)| full.x_for_index(*nb));
                ((*g).to_owned(), (x1 - x0).max(px(0.)))
            })
            .collect();
        let em = window.text_style().font_size.to_pixels(window.rem_size()) * 1.25;
        let per = per.into_iter().map(|(g, a)| if is_pictograph(&g) { (g, if a > em { em } else { a }) } else { (g, a) }).collect();
        let chars = merge_ligatures(per);
        let ellipsis = self.shape("\u{2026}", window, color).width;
        let underline = window.text_style().underline.map(|u| {
            let primary = self.shape(" ", window, color);
            let (ascent, descent) = primary.decoration_metrics();
            (primary.font_size, ascent, descent, u)
        });
        let natural = chars.iter().fold(px(0.), |w, (_, a)| w + *a + self.spacing);
        let mut style = Style::default();
        style.size.width = natural.into();
        style.size.height = line_height.into();
        style.min_size.width = px(0.).into();
        style.max_size.width = relative(1.).into();
        style.flex_shrink = 1.;
        let id = window.request_layout(style, [], cx);
        (id, Layout { chars, ellipsis, line_height, underline })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _layout: &mut Layout,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Layout,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let base = window.text_style().color;
        let natural = layout.chars.iter().fold(px(0.), |w, (_, a)| w + *a + self.spacing);
        let fits = natural <= bounds.size.width + px(0.5);
        let limit = if fits { natural } else { bounds.size.width - layout.ellipsis };
        let total = if fits { natural } else { bounds.size.width };
        let period = self.period.unwrap_or(total);
        let color_at = |x: Pixels| {
            let t = if period > px(0.) { (x / period).fract() } else { 0. };
            gradient_at(&self.colors, t).unwrap_or(base)
        };
        let mut x = px(0.);
        for (s, adv) in &layout.chars {
            if !fits && x + *adv > limit {
                break;
            }
            let line = self.shape(s, window, color_at(x + *adv / 2.));
            let _ = line.paint(point(bounds.origin.x + x, bounds.origin.y), layout.line_height, TextAlign::Left, None, window, cx);
            x += *adv + self.spacing;
        }
        if !fits {
            let line = self.shape("\u{2026}", window, color_at(x + layout.ellipsis / 2.));
            let _ = line.paint(point(bounds.origin.x + x, bounds.origin.y), layout.line_height, TextAlign::Left, None, window, cx);
            x += layout.ellipsis + self.spacing;
        }
        if let Some((size, ascent, descent, mut u)) = layout.underline {
            let (y, thickness) =
                gpui::underline_geometry(bounds.origin.y, layout.line_height, size, ascent, descent, window.scale_factor());
            u.color = u.color.or_else(|| self.colors.first().copied()).or(Some(base));
            if !u.wavy {
                u.thickness = thickness;
            }
            let width = (x - self.spacing).max(px(0.));
            window.paint_underline(point(bounds.origin.x, y), width, &u);
        }
    }
}

fn is_pictograph(g: &str) -> bool {
    g.contains(['\u{FE0F}', '\u{200D}']) || g.chars().next().is_some_and(|c| c as u32 >= 0x1F000)
}

fn merge_ligatures(per: Vec<(String, Pixels)>) -> Vec<(String, Pixels)> {
    let mut out: Vec<(String, Pixels)> = Vec::with_capacity(per.len());
    for (g, adv) in per {
        match out.last_mut() {
            Some(last) if adv <= px(0.) || last.1 <= px(0.) => {
                last.0.push_str(&g);
                last.1 += adv;
            }
            _ => out.push((g, adv)),
        }
    }
    out
}

impl IntoElement for NameText {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligatures_paint_as_one_cluster() {
        let v = |s: &str, a: f32| (s.to_owned(), px(a));
        assert_eq!(merge_ligatures(vec![v("f", 9.), v("l", 0.), v("a", 7.)]), vec![v("fl", 9.), v("a", 7.)]);
        assert_eq!(merge_ligatures(vec![v("f", 0.), v("l", 9.), v("a", 7.)]), vec![v("fl", 9.), v("a", 7.)]);
        assert_eq!(merge_ligatures(vec![v("a", 7.), v("b", 7.)]), vec![v("a", 7.), v("b", 7.)]);
    }

    #[test]
    fn pictographs() {
        assert!(is_pictograph("🌸"));
        assert!(is_pictograph("❤\u{FE0F}"));
        assert!(!is_pictograph("a"));
        assert!(!is_pictograph("☲"));
    }

    #[test]
    fn gradient_stops() {
        let a: Hsla = gpui::rgb(0x000000).into();
        let b: Hsla = gpui::rgb(0xffffff).into();
        let mid = gpui::Rgba::from(gradient_at(&[a, b], 0.5).unwrap());
        assert!((mid.r - 0.5).abs() < 0.01);
        assert_eq!(gradient_at(&[a], 0.7), Some(a));
        assert_eq!(gradient_at(&[], 0.7), None);
    }
}
