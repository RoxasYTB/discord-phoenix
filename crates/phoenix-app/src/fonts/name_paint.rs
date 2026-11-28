use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, CubicBezier, Element, ElementId, GlobalElementId, GradientScroll, Hsla, InspectorElementId, IntoElement,
    LayoutId, Pixels, Rgba, SharedString, Style, TextAlign, TextAnimation, TextGradient, Window, point, px, relative, size,
};

pub const ROLE_SCROLL: Duration = Duration::from_millis(1500);
pub const PRISM_SCROLL: Duration = Duration::from_millis(2000);
pub const GLOW_BLUR: f32 = 4.;
pub const GLOW_OPACITY: f32 = 0.7;
pub const GLOW_FADE: Duration = Duration::from_millis(100);
pub const EFFECT_ANIMATION: Duration = Duration::from_millis(4000);
pub const NEON_EASE: CubicBezier = CubicBezier(0.24, 0.31, 0.36, 0.93);
pub const EFFECT_EASE: CubicBezier = CubicBezier(0.44, 0.29, 0.48, 1.);
pub const GUMMY_LETTER_DELAY: Duration = Duration::from_millis(50);
pub const WHITE: u32 = 0xffffff;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    Still,
    Once,
    Loop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionEnv {
    pub reduced: bool,
    pub focused: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    pub main: u32,
    pub light1: u32,
    pub light2: u32,
    pub dark1: u32,
    pub dark2: u32,
    pub toon_stroke: u32,
    pub neon_stroke: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NameFill {
    Solid(u32),
    Role { stops: Vec<(u32, f32)>, period: Pixels },
    Diagonal(Vec<(u32, f32)>),
    Prism { stops: Vec<(u32, f32)>, chars: usize, min: Pixels },
    Neon { palette: Palette, flicker: u32 },
    Toon(Palette),
    Pop(Palette),
    Gummy(Vec<u32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub color: u32,
    pub gradient: Option<TextGradient>,
    pub stroke: Option<Pixels>,
    pub blur: Option<Pixels>,
    pub dy: Pixels,
    pub animation: Option<TextAnimation>,
}

impl Layer {
    fn solid(color: u32) -> Self {
        Self { color, gradient: None, stroke: None, blur: None, dy: px(0.), animation: None }
    }

    fn stroked(mut self, width: Pixels) -> Self {
        self.stroke = Some(width);
        self
    }
}

pub struct EffectBox {
    pub origin: gpui::Point<Pixels>,
    pub width: Pixels,
    pub line_height: Pixels,
    pub em: Pixels,
    pub css_px: Pixels,
}

fn rgba(c: u32) -> Rgba {
    gpui::rgb(c)
}

fn down(y: Pixels) -> gpui::Point<Pixels> {
    point(px(0.), y)
}

pub fn neon_frames(flicker: u32) -> Vec<(f32, Rgba)> {
    let (w, f) = (rgba(WHITE), rgba(flicker));
    [(0., w), (0.15, w), (0.16, f), (0.18, w), (0.2, w), (0.22, f), (0.23, w), (0.25, w), (0.28, f), (0.5, w), (0.51, w), (1., w)].to_vec()
}

pub fn gummy_frames() -> Vec<(f32, [f32; 2])> {
    vec![(0., [1., 1.]), (0.07, [1.28, 0.72]), (0.14, [0.82, 1.18]), (0.21, [1.1, 0.9]), (0.28, [0.96, 1.04]), (0.35, [1., 1.]), (1., [1., 1.])]
}

pub fn gummy_letter(start: Instant, looped: bool, index: usize, origin: gpui::Point<Pixels>) -> TextAnimation {
    TextAnimation::new(start, EFFECT_ANIMATION)
        .delay(GUMMY_LETTER_DELAY * index as u32)
        .looped(looped)
        .easing(EFFECT_EASE)
        .origin(origin)
        .scale(gummy_frames())
}

impl NameFill {
    pub fn first(&self) -> u32 {
        match self {
            NameFill::Solid(c) => *c,
            NameFill::Role { stops, .. } | NameFill::Diagonal(stops) | NameFill::Prism { stops, .. } => {
                stops.first().map_or(0, |s| s.0)
            }
            NameFill::Neon { palette, .. } | NameFill::Toon(palette) | NameFill::Pop(palette) => palette.main,
            NameFill::Gummy(colors) => colors.first().copied().unwrap_or(0),
        }
    }

    pub fn gradient(&self, origin: gpui::Point<Pixels>, width: Pixels, line_height: Pixels, ch: Pixels) -> Option<TextGradient> {
        let tile = |w: Pixels| Bounds::new(origin, size(w, line_height));
        let (base, stops) = match self {
            NameFill::Role { stops, period } => (TextGradient::new(90., tile(*period)).repeat_x(true), stops),
            NameFill::Diagonal(stops) => (TextGradient::to_corner(true, true, tile(width)), stops),
            NameFill::Prism { stops, chars, min } => {
                let cycle = (ch * *chars as f32).max(*min);
                (TextGradient::new(90., tile(cycle)).repeat_x(true), stops)
            }
            _ => return None,
        };
        if stops.len() < 2 {
            return None;
        }
        Some(stops.iter().fold(base, |g, &(c, p)| g.stop(gpui::rgb(c), p)))
    }

    pub fn scroll(&self, motion: Motion, env: MotionEnv, start: Instant, tile: Pixels) -> Option<GradientScroll> {
        if motion == Motion::Still || env.reduced || tile <= px(0.) {
            return None;
        }
        let tile = f32::from(tile);
        match self {
            NameFill::Role { .. } if env.focused => Some(GradientScroll::new(tile / ROLE_SCROLL.as_secs_f32(), start, None)),
            NameFill::Prism { .. } => {
                let duration = (motion == Motion::Once).then_some(PRISM_SCROLL);
                Some(GradientScroll::new(-tile / PRISM_SCROLL.as_secs_f32(), start, duration))
            }
            _ => None,
        }
    }

    pub fn glows(&self, motion: Motion) -> bool {
        motion != Motion::Still && matches!(self, NameFill::Role { .. })
    }

    pub fn keeps_state(&self, motion: Motion) -> bool {
        motion != Motion::Still || matches!(self, NameFill::Role { .. })
    }

    pub fn animates(motion: Motion, env: MotionEnv) -> Option<bool> {
        (motion != Motion::Still && !env.reduced).then_some(motion == Motion::Loop)
    }

    pub fn layers(&self, b: &EffectBox, animated: Option<(Instant, bool)>) -> Option<Vec<Layer>> {
        let em = b.em;
        let (x0, y0) = (b.origin.x, b.origin.y);
        let anim = |ease: CubicBezier| animated.map(|(start, looped)| TextAnimation::new(start, EFFECT_ANIMATION).looped(looped).easing(ease));
        match self {
            NameFill::Neon { palette, flicker } => {
                let w = b.css_px + em * 0.04;
                let r = b.css_px * 4. + em * 0.12;
                let shadow = Layer { blur: Some(r * 0.5), ..Layer::solid(palette.main).stroked(w) };
                let fill = Layer { animation: anim(NEON_EASE).map(|a| a.tint(neon_frames(*flicker))), ..Layer::solid(WHITE) };
                Some(vec![shadow, Layer::solid(palette.neon_stroke).stroked(w), fill])
            }
            NameFill::Toon(p) => {
                let w = b.css_px * 1.6 + em * 0.04;
                let h = b.line_height + w;
                let tile = Bounds::new(point(x0 - w, y0), size(b.width + w * 2., h * 4.));
                let stops = [
                    (WHITE, 0.),
                    (p.light2, 0.08),
                    (p.light1, 0.15),
                    (p.main, 0.25),
                    (p.light2, 0.45),
                    (p.main, 0.55),
                    (WHITE, 0.75),
                    (p.light2, 0.83),
                    (p.light1, 0.9),
                    (p.main, 1.),
                ];
                let gradient = stops.iter().fold(TextGradient::new(180., tile), |g, &(c, at)| g.stop(gpui::rgb(c), at));
                let scrolled = down(-h * 3.);
                let animation = anim(EFFECT_EASE).map(|a| a.shift(vec![(0., down(px(0.))), (0.05, down(px(0.))), (0.55, scrolled), (1., scrolled)]));
                Some(vec![
                    Layer::solid(p.toon_stroke).stroked(w),
                    Layer { gradient: Some(gradient), animation, ..Layer::solid(p.main) },
                ])
            }
            NameFill::Pop(p) => {
                let w = b.css_px * 1.2 + em * 0.04;
                let d = em * 0.08;
                let (bw, bh) = (b.width + w, b.line_height + w * 2. + d * 2.);
                let tile = Bounds::new(point(x0 - w - bw, y0), size(bw * 2., bh * 2.));
                let stops = [(p.light1, 0.), (p.light1, 0.06), (p.main, 0.2), (p.main, 0.5), (p.light1, 0.56), (p.main, 0.7), (p.main, 1.)];
                let gradient = stops.iter().fold(TextGradient::to_corner(false, true, tile), |g, &(c, at)| g.stop(gpui::rgb(c), at));
                let moved = point(bw, -bh);
                let shadow_motion = anim(EFFECT_EASE).map(|a| {
                    a.translate(vec![(0., down(px(0.))), (0.18, down(em * 0.05)), (0.35, down(-d)), (0.5, down(px(0.))), (1., down(px(0.)))])
                        .shift(vec![(0., down(px(0.))), (0.5, moved), (1., moved)])
                });
                let main_motion = anim(EFFECT_EASE)
                    .map(|a| a.translate(vec![(0., down(px(0.))), (0.18, down(em * -0.05)), (0.35, down(d)), (0.5, down(px(0.))), (1., down(px(0.)))]));
                Some(vec![
                    Layer { gradient: Some(gradient), dy: d, animation: shadow_motion, ..Layer::solid(p.main).stroked(w) },
                    Layer { animation: main_motion.clone(), ..Layer::solid(p.dark2).stroked(w) },
                    Layer { animation: main_motion, ..Layer::solid(WHITE) },
                ])
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowFade {
    pub on: bool,
    pub at: Option<Instant>,
    pub from: f32,
    pub duration: Duration,
}

impl Default for GlowFade {
    fn default() -> Self {
        Self { on: false, at: None, from: 0., duration: GLOW_FADE }
    }
}

impl GlowFade {
    fn target(&self) -> f32 {
        if self.on { 1. } else { 0. }
    }

    pub fn value(&self, now: Instant) -> f32 {
        let Some(at) = self.at else { return self.target() };
        let t = now.saturating_duration_since(at).as_secs_f32() / self.duration.as_secs_f32().max(f32::EPSILON);
        if t >= 1. { self.target() } else { self.from + (self.target() - self.from) * CubicBezier::EASE_IN_OUT.at(t) }
    }

    pub fn set(self, on: bool, now: Instant) -> Self {
        if on == self.on {
            return self;
        }
        let from = self.value(now);
        let target = if on { 1. } else { 0. };
        Self { on, at: Some(now), from, duration: GLOW_FADE.mul_f32((target - from).abs()) }
    }

    pub fn visible(&self, now: Instant) -> bool {
        self.on || self.at.is_some_and(|at| now < at + self.duration)
    }

    pub fn animation(&self, now: Instant) -> Option<TextAnimation> {
        let at = self.at.filter(|&at| now < at + self.duration)?;
        Some(
            TextAnimation::new(at, self.duration)
                .easing(CubicBezier::EASE_IN_OUT)
                .opacity(vec![(0., self.from), (1., self.target())]),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionState {
    pub start: Instant,
    pub moving: bool,
    pub glow: GlowFade,
}

impl MotionState {
    pub fn new(now: Instant) -> Self {
        Self { start: now, moving: false, glow: GlowFade::default() }
    }

    pub fn advance(self, now: Instant, moving: bool, glow: bool) -> Self {
        let start = if moving && !self.moving { now } else { self.start };
        Self { start, moving, glow: self.glow.set(glow, now) }
    }
}

pub struct NameText {
    text: SharedString,
    spacing: Pixels,
    fill: Option<NameFill>,
    motion: Motion,
}

impl NameText {
    pub fn new(text: SharedString, spacing: Pixels, fill: Option<NameFill>) -> Self {
        Self { text, spacing, fill, motion: Motion::Still }
    }

    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }
}

pub struct Layout {
    chars: Vec<(String, Pixels)>,
    ellipsis: Pixels,
    line_height: Pixels,
    ch: Pixels,
    em: Pixels,
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
        self.fill.as_ref().filter(|f| f.keeps_state(self.motion)).map(|_| ElementId::Name("name-motion".into()))
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
        let gummy = matches!(self.fill, Some(NameFill::Gummy(_)));
        let per: Vec<(String, Pixels)> = starts
            .iter()
            .enumerate()
            .map(|(i, (b, g))| {
                if gummy {
                    return ((*g).to_owned(), self.shape(g, window, color).width);
                }
                let x0 = full.x_for_index(*b);
                let x1 = starts.get(i + 1).map_or(full.width, |(nb, _)| full.x_for_index(*nb));
                ((*g).to_owned(), (x1 - x0).max(px(0.)))
            })
            .collect();
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let em = font_size * 1.25;
        let per = per.into_iter().map(|(g, a)| if is_pictograph(&g) { (g, if a > em { em } else { a }) } else { (g, a) }).collect();
        let chars = if gummy { per } else { merge_ligatures(per) };
        let ellipsis = self.shape("\u{2026}", window, color).width;
        let ch = if matches!(self.fill, Some(NameFill::Prism { .. })) { self.shape("0", window, color).width } else { px(0.) };
        let underline = window.text_style().underline.map(|u| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let ts = window.text_system();
            let font = ts.resolve_font(&style.font());
            (size, ts.ascent(font, size), ts.descent(font, size), u)
        });
        let natural = chars.iter().fold(px(0.), |w, (_, a)| w + *a + self.spacing);
        let mut style = Style::default();
        style.size.width = natural.into();
        style.size.height = line_height.into();
        style.min_size.width = px(0.).into();
        style.max_size.width = relative(1.).into();
        style.flex_shrink = 1.;
        let id = window.request_layout(style, [], cx);
        (id, Layout { chars, ellipsis, line_height, ch, em: font_size, underline })
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
        id: Option<&GlobalElementId>,
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
        let first: Option<Hsla> = self.fill.as_ref().map(|f| gpui::rgb(f.first()).into());
        let color = first.unwrap_or(base);
        let mut gradient = self.fill.as_ref().and_then(|f| f.gradient(bounds.origin, total, layout.line_height, layout.ch));
        let now = Instant::now();
        let moving = self.motion != Motion::Still;
        let wants_glow = self.fill.as_ref().is_some_and(|f| f.glows(self.motion));
        let state = id.map(|id| {
            window.with_element_state::<MotionState, _>(id, |s, _| {
                let s = s.unwrap_or_else(|| MotionState::new(now)).advance(now, moving, wants_glow);
                (s, s)
            })
        });
        let env = MotionEnv { reduced: crate::ui::settings::effects::reduced_motion(), focused: window.is_window_active() };
        let mut glow = None;
        if let (Some(fill), Some(g), Some(state)) = (self.fill.as_ref(), gradient.as_mut(), state) {
            g.scroll = fill.scroll(self.motion, env, state.start, g.tile.size.width);
            if state.glow.visible(now) {
                glow = Some(((crate::theme::z(GLOW_BLUR), GLOW_OPACITY), state.glow.animation(now)));
            }
        }
        let animated = NameFill::animates(self.motion, env).map(|looped| (state.map_or(now, |s| s.start), looped));
        let paint = |window: &mut Window, cx: &mut App, glyphs_only: bool, color: Hsla, dy: Pixels| {
            let mut x = px(0.);
            for (s, adv) in &layout.chars {
                if !fits && x + *adv > limit {
                    break;
                }
                if !(glyphs_only && is_pictograph(s)) {
                    let line = self.shape(s, window, color);
                    let _ = line.paint(point(bounds.origin.x + x, bounds.origin.y + dy), layout.line_height, TextAlign::Left, None, window, cx);
                }
                x += *adv + self.spacing;
            }
            if !fits {
                let line = self.shape("\u{2026}", window, color);
                let _ = line.paint(point(bounds.origin.x + x, bounds.origin.y + dy), layout.line_height, TextAlign::Left, None, window, cx);
                x += layout.ellipsis + self.spacing;
            }
            x
        };
        let effect = EffectBox {
            origin: bounds.origin,
            width: total,
            line_height: layout.line_height,
            em: layout.em,
            css_px: crate::theme::z(1.),
        };
        let x = if let Some(layers) = self.fill.as_ref().and_then(|f| f.layers(&effect, animated)) {
            let mut x = px(0.);
            let last = layers.len().saturating_sub(1);
            for (i, l) in layers.iter().enumerate() {
                let c: Hsla = gpui::rgb(l.color).into();
                x = window.with_text_gradient(l.gradient.as_ref(), |window| {
                    window.with_text_stroke(l.stroke, |window| {
                        window.with_text_glow(l.blur.map(|b| (b, 1.)), |window| {
                            window.with_text_animation(l.animation.as_ref(), |window| paint(window, cx, i < last, c, l.dy))
                        })
                    })
                });
            }
            x
        } else if let Some(NameFill::Gummy(colors)) = self.fill.as_ref() {
            let mut x = px(0.);
            let mut letter = 0usize;
            for (s, adv) in &layout.chars {
                if !fits && x + *adv > limit {
                    break;
                }
                let plain = s.trim().is_empty() || is_pictograph(s);
                let c: Hsla = if plain || colors.is_empty() { base } else { gpui::rgb(colors[letter % colors.len()]).into() };
                let origin = point(bounds.origin.x + x + *adv * 0.5, bounds.origin.y + layout.line_height * 0.5);
                let anim = animated.filter(|_| !plain).map(|(start, looped)| gummy_letter(start, looped, letter, origin));
                let line = self.shape(s, window, c);
                window.with_text_animation(anim.as_ref(), |window| {
                    let _ = line.paint(point(bounds.origin.x + x, bounds.origin.y), layout.line_height, TextAlign::Left, None, window, cx);
                });
                if !plain {
                    letter += 1;
                }
                x += *adv + self.spacing;
            }
            if !fits {
                let line = self.shape("\u{2026}", window, base);
                let _ = line.paint(point(bounds.origin.x + x, bounds.origin.y), layout.line_height, TextAlign::Left, None, window, cx);
                x += layout.ellipsis + self.spacing;
            }
            x
        } else {
            window.with_text_gradient(gradient.as_ref(), |window| {
                if let Some((glow, fade)) = glow.as_ref() {
                    window.with_text_glow(Some(*glow), |window| {
                        window.with_text_animation(fade.as_ref(), |window| paint(window, cx, true, color, px(0.)))
                    });
                }
                paint(window, cx, false, color, px(0.))
            })
        };
        if let Some((size, ascent, descent, mut u)) = layout.underline {
            let (y, thickness) =
                gpui::underline_geometry(bounds.origin.y, layout.line_height, size, ascent, descent, window.scale_factor());
            u.color = u.color.or(first).or(Some(base));
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

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn close_mod(a: f32, b: f32, period: f32) -> bool {
        let d = (a - b).rem_euclid(period);
        d < 1e-3 || (period - d) < 1e-3
    }

    #[test]
    fn role_gradient_is_horizontal_and_repeats_every_period() {
        let fill = NameFill::Role { stops: vec![(0x9e6bff, 0.), (0x9fc1ff, 0.5), (0x9e6bff, 1.)], period: px(100.) };
        let g = fill.gradient(point(px(10.), px(4.)), px(250.), px(20.), px(0.)).unwrap();
        assert_eq!(g.angle, 90.);
        assert!(g.repeat_x);
        assert_eq!(g.tile, Bounds::new(point(px(10.), px(4.)), size(px(100.), px(20.))));
        assert!(close(g.position_at(point(px(60.), px(10.))), 0.5));
        assert!(close(g.position_at(point(px(160.), px(10.))), 0.5));
        let mid = g.color_at(point(px(60.), px(10.)));
        assert!(close(mid.r, 0x9f as f32 / 255.) && close(mid.g, 0xc1 as f32 / 255.));
    }

    #[test]
    fn display_name_gradient_runs_to_the_bottom_right_corner_of_the_box() {
        let fill = NameFill::Diagonal(vec![(0x666666, 0.1), (0xfffffe, 0.9)]);
        let g = fill.gradient(point(px(0.), px(0.)), px(35.), px(20.), px(0.)).unwrap();
        assert!(!g.repeat_x);
        assert!(close(g.angle, 150.2551));
        assert!(close(g.position_at(point(px(35.), px(0.))), 0.5));
        assert!(close(g.position_at(point(px(0.), px(20.))), 0.5));
        assert!(close(g.color_at(point(px(0.), px(0.))).r, 0x66 as f32 / 255.));
        assert!(close(g.color_at(point(px(35.), px(20.))).r, 1.));
    }

    #[test]
    fn prism_cycle_is_the_name_length_in_ch_with_a_38px_floor() {
        let fill = NameFill::Prism { stops: vec![(0xff0000, 0.), (0x0000ff, 0.5), (0xff0000, 1.)], chars: 4, min: px(38.) };
        let short = fill.gradient(point(px(0.), px(0.)), px(30.), px(20.), px(8.)).unwrap();
        assert_eq!(short.tile.size.width, px(38.));
        let long = NameFill::Prism { stops: vec![(0xff0000, 0.), (0x0000ff, 0.5), (0xff0000, 1.)], chars: 12, min: px(38.) };
        let g = long.gradient(point(px(0.), px(0.)), px(90.), px(20.), px(8.)).unwrap();
        assert_eq!(g.tile.size.width, px(96.));
        assert_eq!(g.angle, 90.);
        assert!(g.repeat_x);
    }

    const MOVING: MotionEnv = MotionEnv { reduced: false, focused: true };

    fn role() -> NameFill {
        NameFill::Role { stops: vec![(0x9e6bff, 0.), (0x9fc1ff, 0.5), (0x9e6bff, 1.)], period: px(100.) }
    }

    fn prism() -> NameFill {
        NameFill::Prism { stops: vec![(0xff0000, 0.), (0x0000ff, 0.5), (0xff0000, 1.)], chars: 12, min: px(38.) }
    }

    #[test]
    fn role_gradient_scrolls_100px_to_the_right_every_1_5s_like_the_client_keyframes() {
        let start = Instant::now();
        let scroll = role().scroll(Motion::Loop, MOVING, start, px(100.)).unwrap();
        assert_eq!(scroll.duration, None);
        let s = scroll.to_glyph_scroll(1.5);
        let t0 = gpui::gradient_clock(start);
        let period = 150.;
        assert!(close_mod(s.displacement(t0, period), 0., period));
        assert!(close(s.displacement(t0 + 0.375, period), 37.5));
        assert!(close(s.displacement(t0 + 0.75, period), 75.));
        assert!(close_mod(s.displacement(t0 + 1.5, period), 0., period));
        assert!(close(s.displacement(t0 + 2.25, period), 75.));
        assert!(s.running(t0 + 60.));
        let g = role().gradient(point(px(0.), px(0.)), px(250.), px(20.), px(0.)).unwrap().scroll(Some(scroll));
        let d = g.to_glyph_gradient(1.5, 1.);
        let moved = s.apply(d, t0 + 0.375);
        assert!(close(moved.position_at(37.5, 0.), 0.));
        assert!(close(moved.position_at(0., 0.), 0.75));
    }

    #[test]
    fn prism_scrolls_its_cycle_to_the_left_in_2s_once_or_forever() {
        let start = Instant::now();
        let t0 = gpui::gradient_clock(start);
        let once = prism().scroll(Motion::Once, MOVING, start, px(96.)).unwrap().to_glyph_scroll(1.);
        assert!(close(once.displacement(t0 + 0.5, 96.), 72.));
        assert!(close(once.displacement(t0 + 1., 96.), 48.));
        assert!(once.running(t0 + 1.9));
        assert!(!once.running(t0 + 2.01));
        assert!(close(once.displacement(t0 + 30., 96.), 0.));
        let looped = prism().scroll(Motion::Loop, MOVING, start, px(96.)).unwrap().to_glyph_scroll(1.);
        assert!(looped.running(t0 + 30.));
        assert!(close(looped.displacement(t0 + 2.5, 96.), 72.));
    }

    #[test]
    fn reduced_motion_window_focus_and_still_names_do_not_scroll() {
        let start = Instant::now();
        let reduced = MotionEnv { reduced: true, focused: true };
        let blurred = MotionEnv { reduced: false, focused: false };
        assert!(role().scroll(Motion::Still, MOVING, start, px(100.)).is_none());
        assert!(role().scroll(Motion::Loop, reduced, start, px(100.)).is_none());
        assert!(prism().scroll(Motion::Loop, reduced, start, px(96.)).is_none());
        assert!(role().scroll(Motion::Loop, blurred, start, px(100.)).is_none());
        assert!(prism().scroll(Motion::Loop, blurred, start, px(96.)).is_some());
        assert!(NameFill::Diagonal(vec![(0, 0.1), (0xffffff, 0.9)]).scroll(Motion::Loop, MOVING, start, px(40.)).is_none());
    }

    #[test]
    fn only_moving_role_gradients_glow() {
        assert!(role().glows(Motion::Loop));
        assert!(!role().glows(Motion::Still));
        assert!(!prism().glows(Motion::Loop));
        assert_eq!((GLOW_BLUR, GLOW_OPACITY), (4., 0.7));
    }

    #[test]
    fn solid_and_single_stop_fills_paint_without_a_gradient() {
        assert!(NameFill::Solid(0x123456).gradient(point(px(0.), px(0.)), px(30.), px(20.), px(8.)).is_none());
        assert!(NameFill::Diagonal(vec![(0x123456, 0.1)]).gradient(point(px(0.), px(0.)), px(30.), px(20.), px(8.)).is_none());
        assert_eq!(NameFill::Diagonal(vec![(0x123456, 0.1), (0, 0.9)]).first(), 0x123456);
    }

    fn effect_box() -> EffectBox {
        EffectBox { origin: point(px(10.), px(20.)), width: px(60.), line_height: px(22.), em: px(16.), css_px: px(1.) }
    }

    fn palette() -> Palette {
        Palette { main: 0x691d6d, light1: 0x7e2383, light2: 0xa82eaf, dark1: 0x3f1141, dark2: 0x150616, toon_stroke: 0x2a0b2c, neon_stroke: 0x8d2593 }
    }

    #[test]
    fn neon_is_a_white_fill_over_a_stroke_and_a_main_colour_shadow() {
        let fill = NameFill::Neon { palette: palette(), flicker: 0xf4b8f7 };
        let still = fill.layers(&effect_box(), None).unwrap();
        assert_eq!(still.len(), 3);
        let w = px(1. + 0.04 * 16.);
        assert_eq!((still[0].color, still[0].stroke), (0x691d6d, Some(w)));
        assert!(close(f32::from(still[0].blur.unwrap()), (4. + 0.12 * 16.) / 2.));
        assert_eq!((still[1].color, still[1].stroke, still[1].blur), (0x8d2593, Some(w), None));
        assert_eq!((still[2].color, still[2].stroke), (WHITE, None));
        assert!(still.iter().all(|l| l.animation.is_none()));
        let start = Instant::now();
        let moving = fill.layers(&effect_box(), Some((start, true))).unwrap();
        let a = moving[2].animation.as_ref().unwrap();
        assert_eq!((a.duration, a.looped, a.easing), (EFFECT_ANIMATION, true, NEON_EASE));
        let flicker: Vec<f32> = a.tint.iter().filter(|(_, c)| *c == gpui::rgb(0xf4b8f7)).map(|(k, _)| *k).collect();
        assert_eq!(flicker, vec![0.16, 0.22, 0.28]);
        assert_eq!(a.tint.last().unwrap(), &(1., gpui::rgb(WHITE)));
        assert!(moving[0].animation.is_none() && moving[1].animation.is_none());
    }

    #[test]
    fn toon_fills_a_stroked_name_with_a_four_times_taller_gradient_that_scrolls_up() {
        let fill = NameFill::Toon(palette());
        let w = 1.6 + 0.04 * 16.;
        let layers = fill.layers(&effect_box(), Some((Instant::now(), false))).unwrap();
        assert_eq!((layers[0].color, layers[0].stroke), (0x2a0b2c, Some(px(w))));
        let g = layers[1].gradient.as_ref().unwrap();
        assert_eq!(g.angle, 180.);
        assert_eq!(g.stops.len(), 10);
        let h = 22. + w;
        assert!(close(f32::from(g.tile.size.height), 4. * h));
        assert!(close(f32::from(g.tile.origin.y), 20.));
        let top = g.color_at(point(px(30.), px(20.)));
        assert!(close(top.r, 1.) && close(top.g, 1.) && close(top.b, 1.));
        let a = layers[1].animation.as_ref().unwrap();
        assert_eq!(a.easing, EFFECT_EASE);
        assert_eq!(a.shift[1], (0.05, point(px(0.), px(0.))));
        assert!(close(f32::from(a.shift[2].1.y), -3. * h) && a.shift[2].0 == 0.55);
        assert!(layers[0].animation.is_none());
    }

    #[test]
    fn pop_drops_a_gradient_shadow_08em_below_a_white_name_with_a_dark_stroke() {
        let fill = NameFill::Pop(palette());
        let w = 1.2 + 0.04 * 16.;
        let d = 0.08 * 16.;
        let still = fill.layers(&effect_box(), None).unwrap();
        assert_eq!(still.len(), 3);
        let shadow = &still[0];
        assert!(close(f32::from(shadow.dy), d));
        assert_eq!(shadow.stroke, Some(px(w)));
        let g = shadow.gradient.as_ref().unwrap();
        let (bw, bh) = (60. + w, 22. + 2. * w + 2. * d);
        assert!(close(f32::from(g.tile.size.width), 2. * bw) && close(f32::from(g.tile.size.height), 2. * bh));
        assert!(close(f32::from(g.tile.origin.x), 10. - w - bw));
        assert_eq!(g.stops.len(), 7);
        assert_eq!((still[1].color, still[1].stroke), (0x150616, Some(px(w))));
        assert_eq!((still[2].color, still[2].stroke), (WHITE, None));
        let moving = fill.layers(&effect_box(), Some((Instant::now(), true))).unwrap();
        let s = moving[0].animation.as_ref().unwrap();
        assert!(close(f32::from(s.translate[1].1.y), 0.05 * 16.) && close(f32::from(s.translate[2].1.y), -d));
        assert_eq!(s.shift[1], (0.5, point(px(bw), px(-bh))));
        let m = moving[2].animation.as_ref().unwrap();
        assert!(close(f32::from(m.translate[1].1.y), -0.05 * 16.) && close(f32::from(m.translate[2].1.y), d));
        assert_eq!(moving[1].animation, moving[2].animation);
    }

    #[test]
    fn gummy_letters_squish_one_after_the_other_around_their_centre() {
        let start = Instant::now();
        let a = gummy_letter(start, false, 3, point(px(15.), px(11.)));
        assert_eq!(a.delay, Duration::from_millis(150));
        assert_eq!((a.duration, a.easing, a.looped), (EFFECT_ANIMATION, EFFECT_EASE, false));
        assert_eq!(a.origin, point(px(15.), px(11.)));
        assert_eq!(a.scale[1], (0.07, [1.28, 0.72]));
        assert_eq!(a.scale.last(), Some(&(1., [1., 1.])));
        assert!(NameFill::Gummy(vec![1, 2]).layers(&effect_box(), None).is_none());
        assert_eq!(NameFill::Gummy(vec![0x123456, 2]).first(), 0x123456);
    }

    #[test]
    fn effects_animate_only_when_moving_without_reduced_motion_even_unfocused() {
        let blurred = MotionEnv { reduced: false, focused: false };
        assert_eq!(NameFill::animates(Motion::Loop, blurred), Some(true));
        assert_eq!(NameFill::animates(Motion::Once, MOVING), Some(false));
        assert_eq!(NameFill::animates(Motion::Still, MOVING), None);
        assert_eq!(NameFill::animates(Motion::Loop, MotionEnv { reduced: true, focused: true }), None);
    }

    #[test]
    fn the_glow_fades_in_and_out_in_100ms_ease_in_out() {
        let t0 = Instant::now();
        let fade = GlowFade::default().set(true, t0);
        assert!(close(fade.value(t0), 0.));
        assert!(close(fade.value(t0 + Duration::from_millis(25)), 0.1291));
        assert!(close(fade.value(t0 + Duration::from_millis(50)), 0.5));
        assert!(close(fade.value(t0 + Duration::from_millis(100)), 1.));
        let a = fade.animation(t0 + Duration::from_millis(10)).unwrap();
        assert_eq!((a.duration, a.easing), (GLOW_FADE, CubicBezier::EASE_IN_OUT));
        assert_eq!(a.opacity, vec![(0., 0.), (1., 1.)]);
        assert!(fade.animation(t0 + Duration::from_millis(100)).is_none());
        let out = fade.set(false, t0 + Duration::from_millis(200));
        assert!(out.visible(t0 + Duration::from_millis(250)));
        assert!(!out.visible(t0 + Duration::from_millis(300)));
        assert!(close(out.value(t0 + Duration::from_millis(250)), 0.5));
        let back = fade.set(false, t0 + Duration::from_millis(50));
        assert!(close(back.duration.as_secs_f32(), 0.05));
        assert!(close(back.from, 0.5));
    }

    #[test]
    fn a_new_hover_restarts_the_animation_clock() {
        let t0 = Instant::now();
        let s = MotionState::new(t0).advance(t0, false, false);
        let t1 = t0 + Duration::from_secs(3);
        let hovered = s.advance(t1, true, true);
        assert_eq!(hovered.start, t1);
        assert!(hovered.glow.on);
        assert_eq!(hovered.advance(t1 + Duration::from_secs(1), true, true).start, t1);
        let t2 = t1 + Duration::from_secs(5);
        assert_eq!(hovered.advance(t2, false, false).advance(t2, true, true).start, t2);
        assert!(role().keeps_state(Motion::Still));
        assert!(!prism().keeps_state(Motion::Still));
        assert!(prism().keeps_state(Motion::Once));
    }
}
