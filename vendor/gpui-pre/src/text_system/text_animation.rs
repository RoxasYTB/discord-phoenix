#![allow(missing_docs)]

use crate::{GlyphGradient, MonochromeSprite, Pixels, Point, Rgba, TransformationMatrix};
use scheduler::Instant;
use std::time::Duration;

// PHOENIX PATCH (text keyframes): a CSS `@keyframes` animation of the glyphs painted inside
// `Window::with_text_animation`, applied by the renderer at present time like the gradient
// scroll of patch 17: `transform` (translate, scale around `origin`), `background-position`
// of the text gradient (`shift`), the fill colour (`tint`) and `opacity`. The timing function
// applies to every interval between two keyframes of a property, as in CSS. Before its delay
// the animation has no effect (no backwards fill); after its last iteration it keeps its last
// keyframe (`animation-fill-mode: forwards`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier(pub f32, pub f32, pub f32, pub f32);

impl Default for CubicBezier {
    fn default() -> Self {
        Self::LINEAR
    }
}

impl CubicBezier {
    pub const LINEAR: Self = Self(0.0, 0.0, 1.0, 1.0);
    pub const EASE_IN_OUT: Self = Self(0.42, 0.0, 0.58, 1.0);

    fn curve(a: f32, b: f32, t: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    }

    fn slope(a: f32, b: f32, t: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    }

    pub fn at(&self, x: f32) -> f32 {
        if *self == Self::LINEAR {
            return x;
        }
        let x = x.clamp(0.0, 1.0);
        let mut t = x;
        for _ in 0..8 {
            let err = Self::curve(self.0, self.2, t) - x;
            if err.abs() < 1e-6 {
                return Self::curve(self.1, self.3, t);
            }
            let d = Self::slope(self.0, self.2, t);
            if d.abs() < 1e-6 {
                break;
            }
            t = (t - err / d).clamp(0.0, 1.0);
        }
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        t = x;
        for _ in 0..40 {
            let v = Self::curve(self.0, self.2, t);
            if (v - x).abs() < 1e-6 {
                break;
            }
            if v < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) * 0.5;
        }
        Self::curve(self.1, self.3, t)
    }
}

pub trait KeyframeValue: Copy {
    fn lerp(a: Self, b: Self, t: f32) -> Self;
}

impl KeyframeValue for f32 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl<const N: usize> KeyframeValue for [f32; N] {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        let mut out = a;
        for (o, (x, y)) in out.iter_mut().zip(a.iter().zip(b)) {
            *o = x + (y - x) * t;
        }
        out
    }
}

pub fn sample_keyframes<T: KeyframeValue>(
    frames: &[(f32, T)],
    progress: f32,
    easing: CubicBezier,
) -> Option<T> {
    let first = frames.first()?;
    if progress <= first.0 {
        return Some(first.1);
    }
    for pair in frames.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if progress <= b.0 {
            let span = b.0 - a.0;
            let u = if span > 0.0 { (progress - a.0) / span } else { 1.0 };
            return Some(T::lerp(a.1, b.1, easing.at(u)));
        }
    }
    frames.last().map(|f| f.1)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextAnimation {
    pub start: Instant,
    pub delay: Duration,
    pub duration: Duration,
    pub looped: bool,
    pub easing: CubicBezier,
    pub origin: Point<Pixels>,
    pub translate: Vec<(f32, Point<Pixels>)>,
    pub scale: Vec<(f32, [f32; 2])>,
    pub shift: Vec<(f32, Point<Pixels>)>,
    pub tint: Vec<(f32, Rgba)>,
    pub opacity: Vec<(f32, f32)>,
}

impl TextAnimation {
    pub fn new(start: Instant, duration: Duration) -> Self {
        Self {
            start,
            delay: Duration::ZERO,
            duration,
            looped: false,
            easing: CubicBezier::LINEAR,
            origin: Point::default(),
            translate: Vec::new(),
            scale: Vec::new(),
            shift: Vec::new(),
            tint: Vec::new(),
            opacity: Vec::new(),
        }
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn looped(mut self, looped: bool) -> Self {
        self.looped = looped;
        self
    }

    pub fn easing(mut self, easing: CubicBezier) -> Self {
        self.easing = easing;
        self
    }

    pub fn origin(mut self, origin: Point<Pixels>) -> Self {
        self.origin = origin;
        self
    }

    pub fn translate(mut self, frames: Vec<(f32, Point<Pixels>)>) -> Self {
        self.translate = frames;
        self
    }

    pub fn scale(mut self, frames: Vec<(f32, [f32; 2])>) -> Self {
        self.scale = frames;
        self
    }

    pub fn shift(mut self, frames: Vec<(f32, Point<Pixels>)>) -> Self {
        self.shift = frames;
        self
    }

    pub fn tint(mut self, frames: Vec<(f32, Rgba)>) -> Self {
        self.tint = frames;
        self
    }

    pub fn opacity(mut self, frames: Vec<(f32, f32)>) -> Self {
        self.opacity = frames;
        self
    }

    pub fn to_glyph_animation(&self, scale_factor: f32) -> GlyphAnimation {
        let point = |p: Point<Pixels>| [f32::from(p.x) * scale_factor, f32::from(p.y) * scale_factor];
        GlyphAnimation {
            start: crate::gradient_clock(self.start) + self.delay.as_secs_f64(),
            duration: self.duration.as_secs_f32(),
            looped: self.looped,
            easing: self.easing,
            origin: point(self.origin),
            translate: self.translate.iter().map(|&(k, p)| (k, point(p))).collect(),
            scale: self.scale.clone(),
            shift: self.shift.iter().map(|&(k, p)| (k, point(p))).collect(),
            tint: self
                .tint
                .iter()
                .map(|&(k, c)| (k, [c.r, c.g, c.b, c.a]))
                .collect(),
            opacity: self.opacity.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlyphAnimation {
    pub start: f64,
    pub duration: f32,
    pub looped: bool,
    pub easing: CubicBezier,
    pub origin: [f32; 2],
    pub translate: Vec<(f32, [f32; 2])>,
    pub scale: Vec<(f32, [f32; 2])>,
    pub shift: Vec<(f32, [f32; 2])>,
    pub tint: Vec<(f32, [f32; 4])>,
    pub opacity: Vec<(f32, f32)>,
}

impl GlyphAnimation {
    pub fn progress(&self, now: f64) -> Option<f32> {
        let elapsed = now - self.start;
        if elapsed < 0.0 {
            return None;
        }
        if self.duration <= 0.0 {
            return Some(1.0);
        }
        let cycles = elapsed / self.duration as f64;
        Some(if self.looped {
            cycles.fract() as f32
        } else {
            cycles.min(1.0) as f32
        })
    }

    pub fn running(&self, now: f64) -> bool {
        self.looped || now < self.start + self.duration as f64
    }

    pub fn apply(&self, sprite: &mut MonochromeSprite, gradient: &mut GlyphGradient, now: f64) {
        let Some(p) = self.progress(now) else {
            return;
        };
        let e = self.easing;
        let t = sample_keyframes(&self.translate, p, e).unwrap_or([0.0, 0.0]);
        let s = sample_keyframes(&self.scale, p, e).unwrap_or([1.0, 1.0]);
        if t != [0.0, 0.0] || s != [1.0, 1.0] {
            let o = self.origin;
            let motion = TransformationMatrix {
                rotation_scale: [[s[0], 0.0], [0.0, s[1]]],
                translation: [o[0] - s[0] * o[0] + t[0], o[1] - s[1] * o[1] + t[1]],
            };
            sprite.transformation = motion.compose(sprite.transformation);
        }
        let shift = sample_keyframes(&self.shift, p, e).unwrap_or([0.0, 0.0]);
        gradient.origin[0] += shift[0] + t[0];
        gradient.origin[1] += shift[1] + t[1];
        if let Some(c) = sample_keyframes(&self.tint, p, e) {
            for stop in gradient.stops.iter_mut() {
                stop.color = c;
            }
        }
        if let Some(o) = sample_keyframes(&self.opacity, p, e) {
            gradient.opacity *= o.clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn cubic_bezier_matches_the_css_curves() {
        assert!(close(CubicBezier::LINEAR.at(0.3), 0.3));
        let ease = CubicBezier(0.25, 0.1, 0.25, 1.0);
        assert!(close(ease.at(0.5), 0.8024));
        assert!(close(CubicBezier::EASE_IN_OUT.at(0.5), 0.5));
        assert!(close(CubicBezier::EASE_IN_OUT.at(0.25), 0.1291));
        let c = CubicBezier(0.44, 0.29, 0.48, 1.0);
        assert!(close(c.at(0.0), 0.0) && close(c.at(1.0), 1.0));
        assert!(c.at(0.5) > 0.5);
    }

    #[test]
    fn keyframes_ease_every_interval_and_hold_the_ends() {
        let frames = [(0.0, 0.0f32), (0.5, 10.0), (1.0, 10.0)];
        assert!(close(sample_keyframes(&frames, 0.25, CubicBezier::LINEAR).unwrap(), 5.0));
        let eased = sample_keyframes(&frames, 0.125, CubicBezier::EASE_IN_OUT).unwrap();
        assert!(close(eased, 1.291), "{eased}");
        assert!(close(sample_keyframes(&frames, 0.75, CubicBezier::EASE_IN_OUT).unwrap(), 10.0));
        assert!(close(sample_keyframes(&frames, 2.0, CubicBezier::LINEAR).unwrap(), 10.0));
        assert!(sample_keyframes::<f32>(&[], 0.5, CubicBezier::LINEAR).is_none());
        let same = [(0.15, 1.0f32), (0.16, 0.0), (0.18, 1.0)];
        assert!(close(sample_keyframes(&same, 0.16, CubicBezier::LINEAR).unwrap(), 0.0));
        assert!(close(sample_keyframes(&same, 0.0, CubicBezier::LINEAR).unwrap(), 1.0));
    }

    fn sprite() -> MonochromeSprite {
        MonochromeSprite {
            order: 0,
            pad: 1,
            tile: crate::AtlasTile {
                texture_id: crate::AtlasTextureId {
                    index: 0,
                    kind: crate::AtlasTextureKind::Monochrome,
                },
                tile_id: crate::TileId(0),
                padding: 0,
                bounds: Default::default(),
            },
            bounds: Default::default(),
            content_mask: Default::default(),
            color: Default::default(),
            transformation: TransformationMatrix::unit(),
        }
    }

    fn animation(looped: bool) -> GlyphAnimation {
        let start = Instant::now();
        TextAnimation::new(start, Duration::from_secs(4))
            .delay(Duration::from_millis(100))
            .looped(looped)
            .origin(point(px(10.0), px(10.0)))
            .scale(vec![(0.0, [1.0, 1.0]), (0.5, [2.0, 0.5]), (1.0, [1.0, 1.0])])
            .translate(vec![(0.0, point(px(0.0), px(0.0))), (1.0, point(px(0.0), px(4.0)))])
            .to_glyph_animation(1.5)
    }

    #[test]
    fn progress_waits_for_the_delay_then_loops_or_stops_forwards() {
        let a = animation(false);
        assert_eq!(a.progress(a.start - 0.05), None);
        assert!(a.running(a.start - 0.05));
        assert!(close(a.progress(a.start + 1.0).unwrap(), 0.25));
        assert!(close(a.progress(a.start + 9.0).unwrap(), 1.0));
        assert!(!a.running(a.start + 4.01));
        let l = animation(true);
        assert!(close(l.progress(l.start + 5.0).unwrap(), 0.25));
        assert!(l.running(l.start + 100.0));
        assert_eq!(a.origin, [15.0, 15.0]);
        assert_eq!(a.translate[1].1, [0.0, 6.0]);
    }

    #[test]
    fn apply_scales_around_the_origin_and_moves_the_gradient_with_the_glyph() {
        let a = animation(false);
        let mut sprite = sprite();
        let mut gradient = GlyphGradient::default();
        a.apply(&mut sprite, &mut gradient, a.start + 2.0);
        let m = sprite.transformation;
        assert_eq!(m.rotation_scale, [[2.0, 0.0], [0.0, 0.5]]);
        let x = m.rotation_scale[0][0] * 15.0 + m.translation[0];
        let y = m.rotation_scale[1][1] * 15.0 + m.translation[1];
        assert!(close(x, 15.0) && close(y, 15.0 + 3.0));
        assert!(close(gradient.origin[1], 3.0));
        let mut before = GlyphGradient::default();
        let mut untouched = sprite;
        untouched.transformation = TransformationMatrix::unit();
        a.apply(&mut untouched, &mut before, a.start - 0.01);
        assert_eq!(untouched.transformation, TransformationMatrix::unit());
    }

    #[test]
    fn tint_replaces_the_fill_and_opacity_fades_it() {
        let start = Instant::now();
        let a = TextAnimation::new(start, Duration::from_millis(100))
            .tint(vec![(0.0, Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }), (1.0, Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 })])
            .opacity(vec![(0.0, 0.0), (1.0, 0.7)])
            .easing(CubicBezier::EASE_IN_OUT)
            .to_glyph_animation(1.0);
        let mut sprite = sprite();
        let mut g = GlyphGradient { stop_count: 1, opacity: 1.0, ..Default::default() };
        a.apply(&mut sprite, &mut g, a.start + 0.05);
        assert!(close(g.stops[0].color[1], 0.5));
        assert!(close(g.opacity, 0.35));
        assert_eq!(sprite.transformation, TransformationMatrix::unit());
    }
}
