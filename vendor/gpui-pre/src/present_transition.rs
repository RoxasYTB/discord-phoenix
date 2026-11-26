#![allow(missing_docs)]

use crate::{
    Background, Bounds, ContentMask, Corners, CubicBezier, Edges, Hsla, MonochromeSprite, Path,
    Pixels, Point, PolychromeSprite, Quad, Rgba, ScaledPixels, Shadow, SpringConfig,
    SubpixelSprite, TransformationMatrix, Underline, point,
};
use scheduler::Instant;
use std::time::Duration;

// PHOENIX PATCH (present transitions): a CSS transition of everything painted inside
// `Window::with_present_transition`, applied by the renderer at present time like the
// gradient scroll of patch 17. It moves (`translate`), scales around `origin` (`scale`), fades
// (`opacity`) and recolours the solid quad backgrounds (`background`) from their first value
// to their second. The painted primitives are the layout state; the transition only changes
// how they are drawn, so no view is notified, laid out or painted while it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransitionTiming {
    Ease { duration: Duration, easing: CubicBezier },
    Spring(SpringConfig),
}

impl Default for TransitionTiming {
    fn default() -> Self {
        Self::Ease {
            duration: Duration::ZERO,
            easing: CubicBezier::LINEAR,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub start: Option<Instant>,
    pub delay: Duration,
    pub timing: TransitionTiming,
    pub origin: Point<f32>,
    pub translate: (Point<Pixels>, Point<Pixels>),
    pub scale: (f32, f32),
    pub opacity: (f32, f32),
    pub background: Option<(Hsla, Hsla)>,
    pub height: Option<(Pixels, Pixels)>,
    pub move_masks: bool,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            start: None,
            delay: Duration::ZERO,
            timing: TransitionTiming::default(),
            origin: point(0.5, 0.5),
            translate: (Point::default(), Point::default()),
            scale: (1.0, 1.0),
            opacity: (1.0, 1.0),
            background: None,
            height: None,
            move_masks: true,
        }
    }
}

impl Transition {
    pub fn ease(duration: Duration, easing: CubicBezier) -> Self {
        Self {
            timing: TransitionTiming::Ease { duration, easing },
            ..Default::default()
        }
    }

    pub fn spring(config: SpringConfig) -> Self {
        Self {
            timing: TransitionTiming::Spring(config),
            ..Default::default()
        }
    }

    pub fn start(mut self, start: Instant) -> Self {
        self.start = Some(start);
        self
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn origin(mut self, x: f32, y: f32) -> Self {
        self.origin = point(x, y);
        self
    }

    pub fn translate(mut self, from: Point<Pixels>, to: Point<Pixels>) -> Self {
        self.translate = (from, to);
        self
    }

    pub fn scale(mut self, from: f32, to: f32) -> Self {
        self.scale = (from, to);
        self
    }

    pub fn opacity(mut self, from: f32, to: f32) -> Self {
        self.opacity = (from, to);
        self
    }

    pub fn background(mut self, from: Hsla, to: Hsla) -> Self {
        self.background = Some((from, to));
        self
    }

    pub fn height(mut self, from: Pixels, to: Pixels) -> Self {
        self.height = Some((from, to));
        self
    }

    pub fn keep_masks(mut self) -> Self {
        self.move_masks = false;
        self
    }

    pub fn to_scene(&self, bounds: Bounds<Pixels>, start: Instant, scale_factor: f32) -> SceneTransition {
        let device = |p: Point<Pixels>| [p.x.0 * scale_factor, p.y.0 * scale_factor];
        let (duration, easing, spring) = match self.timing {
            TransitionTiming::Ease { duration, easing } => (duration.as_secs_f32(), easing, None),
            TransitionTiming::Spring(config) => (spring_settle_time(config), CubicBezier::LINEAR, Some(config)),
        };
        SceneTransition {
            start: crate::gradient_clock(self.start.unwrap_or(start)) + self.delay.as_secs_f64(),
            duration,
            easing,
            spring,
            origin: [
                (bounds.origin.x.0 + bounds.size.width.0 * self.origin.x) * scale_factor,
                (bounds.origin.y.0 + bounds.size.height.0 * self.origin.y) * scale_factor,
            ],
            translate: (device(self.translate.0), device(self.translate.1)),
            scale: self.scale,
            opacity: self.opacity,
            background: self.background,
            height: self.height.map(|(a, b)| (a.0 * scale_factor, b.0 * scale_factor)),
            move_masks: self.move_masks,
            parent: 0,
        }
    }
}

pub fn spring_progress(config: SpringConfig, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    let (w0, zeta) = config.canonical();
    if !w0.is_finite() || w0 <= 0.0 {
        return 1.0;
    }
    let x = if zeta < 1.0 - 1e-4 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        -(-zeta * w0 * t).exp() * ((wd * t).cos() + zeta * w0 / wd * (wd * t).sin())
    } else if zeta <= 1.0 + 1e-4 {
        -(-w0 * t).exp() * (1.0 + w0 * t)
    } else {
        let root = (zeta * zeta - 1.0).sqrt();
        let (r1, r2) = (-w0 * (zeta - root), -w0 * (zeta + root));
        let c1 = r2 / (r1 - r2);
        let c2 = -1.0 - c1;
        c1 * (r1 * t).exp() + c2 * (r2 * t).exp()
    };
    1.0 + x
}

pub fn spring_settle_time(config: SpringConfig) -> f32 {
    let (w0, zeta) = config.canonical();
    if !w0.is_finite() || w0 <= 0.0 {
        return 0.0;
    }
    if zeta < 1.0 {
        let rate = zeta * w0;
        if rate <= 0.0 {
            return 0.0;
        }
        let gain = 1.0 + zeta / (1.0 - zeta * zeta).sqrt();
        return ((gain / 0.001).ln() / rate).max(0.0);
    }
    let mut t = 0.0f32;
    while t < 5.0 {
        t += 0.001;
        if 1.0 - spring_progress(config, t) < 0.001 {
            break;
        }
    }
    t
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneTransition {
    pub start: f64,
    pub duration: f32,
    pub easing: CubicBezier,
    pub spring: Option<SpringConfig>,
    pub origin: [f32; 2],
    pub translate: ([f32; 2], [f32; 2]),
    pub scale: (f32, f32),
    pub opacity: (f32, f32),
    pub background: Option<(Hsla, Hsla)>,
    pub height: Option<(f32, f32)>,
    pub move_masks: bool,
    pub parent: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionState {
    pub offset: [f32; 2],
    pub scale: f32,
    pub origin: [f32; 2],
    pub opacity: f32,
    pub background: Option<Hsla>,
    pub height: Option<f32>,
    pub move_masks: bool,
}

fn lerp(a: f32, b: f32, p: f32) -> f32 {
    a + (b - a) * p
}

pub fn mix_premultiplied(a: Hsla, b: Hsla, p: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let alpha = lerp(a.a, b.a, p).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return Hsla::from(Rgba { r: b.r, g: b.g, b: b.b, a: 0.0 });
    }
    let channel = |x: f32, y: f32| (lerp(x * a.a, y * b.a, p) / alpha).clamp(0.0, 1.0);
    Hsla::from(Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    })
}

impl SceneTransition {
    pub fn progress(&self, now: f64) -> f32 {
        let elapsed = (now - self.start) as f32;
        if elapsed <= 0.0 || elapsed.is_nan() {
            return 0.0;
        }
        if elapsed >= self.duration {
            return 1.0;
        }
        match self.spring {
            Some(config) => spring_progress(config, elapsed),
            None => self.easing.at(elapsed / self.duration.max(f32::EPSILON)),
        }
    }

    pub fn running(&self, now: f64) -> bool {
        now < self.start + self.duration as f64
    }

    pub fn state(&self, now: f64) -> TransitionState {
        let p = self.progress(now);
        let ((fx, fy), (tx, ty)) = (
            (self.translate.0[0], self.translate.0[1]),
            (self.translate.1[0], self.translate.1[1]),
        );
        TransitionState {
            offset: [lerp(fx, tx, p), lerp(fy, ty, p)],
            scale: lerp(self.scale.0, self.scale.1, p).max(0.0),
            origin: self.origin,
            opacity: lerp(self.opacity.0, self.opacity.1, p).clamp(0.0, 1.0),
            background: self
                .background
                .map(|(from, to)| mix_premultiplied(from, to, p.clamp(0.0, 1.0))),
            height: self.height.map(|(from, to)| lerp(from, to, p).max(0.0)),
            move_masks: self.move_masks,
        }
    }
}

impl TransitionState {
    pub fn is_identity(&self) -> bool {
        self.offset == [0.0, 0.0]
            && self.scale == 1.0
            && self.opacity >= 1.0
            && self.background.is_none()
            && self.height.is_none()
    }

    pub fn moves(&self) -> bool {
        self.offset != [0.0, 0.0] || self.scale != 1.0
    }

    pub fn point(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + (p[0] - self.origin[0]) * self.scale + self.offset[0],
            self.origin[1] + (p[1] - self.origin[1]) * self.scale + self.offset[1],
        ]
    }

    pub fn matrix(&self) -> TransformationMatrix {
        TransformationMatrix {
            rotation_scale: [[self.scale, 0.0], [0.0, self.scale]],
            translation: [
                self.origin[0] - self.scale * self.origin[0] + self.offset[0],
                self.origin[1] - self.scale * self.origin[1] + self.offset[1],
            ],
        }
    }

    pub fn bounds(&self, b: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
        if !self.moves() {
            return b;
        }
        let o = self.point([b.origin.x.0, b.origin.y.0]);
        Bounds {
            origin: point(ScaledPixels(o[0]), ScaledPixels(o[1])),
            size: crate::size(
                ScaledPixels(b.size.width.0 * self.scale),
                ScaledPixels(b.size.height.0 * self.scale),
            ),
        }
    }

    pub fn mask(&self, m: ContentMask<ScaledPixels>) -> ContentMask<ScaledPixels> {
        if !self.move_masks {
            return m;
        }
        ContentMask {
            bounds: self.bounds(m.bounds),
        }
    }

    fn length(&self, v: ScaledPixels) -> ScaledPixels {
        ScaledPixels(v.0 * self.scale)
    }

    fn corners(&self, c: Corners<ScaledPixels>) -> Corners<ScaledPixels> {
        Corners {
            top_left: self.length(c.top_left),
            top_right: self.length(c.top_right),
            bottom_right: self.length(c.bottom_right),
            bottom_left: self.length(c.bottom_left),
        }
    }

    fn edges(&self, e: Edges<ScaledPixels>) -> Edges<ScaledPixels> {
        Edges {
            top: self.length(e.top),
            right: self.length(e.right),
            bottom: self.length(e.bottom),
            left: self.length(e.left),
        }
    }

    fn fade_background(&self, background: Background) -> Background {
        if self.opacity >= 1.0 {
            background
        } else {
            background.opacity(self.opacity)
        }
    }

    pub fn quad(&self, mut q: Quad) -> Quad {
        if let Some(color) = self.background
            && q.background.as_solid().is_some()
        {
            q.background.solid = color;
        }
        if self.moves() {
            q.bounds = self.bounds(q.bounds);
            q.content_mask = self.mask(q.content_mask);
            q.corner_radii = self.corners(q.corner_radii);
            q.border_widths = self.edges(q.border_widths);
        }
        if let Some(h) = self.height {
            let h = h * self.scale;
            q.bounds.origin.y.0 += (q.bounds.size.height.0 - h) / 2.0;
            q.bounds.size.height = ScaledPixels(h);
        }
        q.background = self.fade_background(q.background);
        q.border_color = q.border_color.opacity(self.opacity);
        q
    }

    pub fn shadow(&self, mut s: Shadow) -> Shadow {
        if self.moves() {
            s.bounds = self.bounds(s.bounds);
            s.content_mask = self.mask(s.content_mask);
            s.corner_radii = self.corners(s.corner_radii);
            s.element_bounds = self.bounds(s.element_bounds);
            s.element_corner_radii = self.corners(s.element_corner_radii);
            s.blur_radius = self.length(s.blur_radius);
        }
        s.color = s.color.opacity(self.opacity);
        s
    }

    pub fn underline(&self, mut u: Underline) -> Underline {
        if self.moves() {
            u.bounds = self.bounds(u.bounds);
            u.content_mask = self.mask(u.content_mask);
            u.thickness = self.length(u.thickness);
        }
        u.color = u.color.opacity(self.opacity);
        u
    }

    fn sprite_geometry(
        &self,
        bounds: &mut Bounds<ScaledPixels>,
        mask: &mut ContentMask<ScaledPixels>,
        transformation: &mut TransformationMatrix,
    ) {
        if !self.moves() {
            return;
        }
        *mask = self.mask(*mask);
        if *transformation == TransformationMatrix::unit() {
            *bounds = self.bounds(*bounds);
        } else {
            *transformation = self.matrix().compose(*transformation);
        }
    }

    pub fn monochrome(&self, mut s: MonochromeSprite) -> MonochromeSprite {
        self.sprite_geometry(&mut s.bounds, &mut s.content_mask, &mut s.transformation);
        s.color = s.color.opacity(self.opacity);
        s
    }

    pub fn gradient(&self, gradient: &mut crate::GlyphGradient) {
        if self.moves() {
            let o = self.point(gradient.origin);
            gradient.origin = o;
        }
        gradient.opacity *= self.opacity;
    }

    pub fn subpixel(&self, mut s: SubpixelSprite) -> SubpixelSprite {
        self.sprite_geometry(&mut s.bounds, &mut s.content_mask, &mut s.transformation);
        s.color = s.color.opacity(self.opacity);
        s
    }

    pub fn polychrome(&self, mut s: PolychromeSprite) -> PolychromeSprite {
        if self.moves() {
            s.bounds = self.bounds(s.bounds);
            s.content_mask = self.mask(s.content_mask);
            s.corner_radii = self.corners(s.corner_radii);
        }
        s.opacity *= self.opacity;
        s
    }

    pub fn path(&self, path: &Path<ScaledPixels>) -> Path<ScaledPixels> {
        let mut path = path.clone();
        if self.moves() {
            path.bounds = self.bounds(path.bounds);
            path.content_mask = self.mask(path.content_mask);
            for v in &mut path.vertices {
                let p = self.point([v.xy_position.x.0, v.xy_position.y.0]);
                v.xy_position = point(ScaledPixels(p[0]), ScaledPixels(p[1]));
                v.content_mask = self.mask(v.content_mask);
            }
        }
        path.color = self.fade_background(path.color);
        path
    }

    pub fn then(&self, outer: &TransitionState) -> TransitionState {
        let inner_origin_moved = outer.point(self.point(self.origin));
        let scale = self.scale * outer.scale;
        let origin = self.origin;
        let offset = [
            inner_origin_moved[0] - origin[0],
            inner_origin_moved[1] - origin[1],
        ];
        TransitionState {
            offset,
            scale,
            origin,
            opacity: self.opacity * outer.opacity,
            background: self.background.or(outer.background),
            height: self.height.or(outer.height),
            move_masks: if self.moves() { self.move_masks } else { outer.move_masks },
        }
    }
}

pub struct PresentTransitionElement<E> {
    id: crate::ElementId,
    element: Option<E>,
    enter: Option<Transition>,
    exit: Option<(u64, Transition)>,
    start: Option<Instant>,
}

impl<E> PresentTransitionElement<E> {
    pub fn on_exit(mut self, key: u64, exit: Transition) -> Self {
        self.exit = Some((key, exit));
        self
    }
}

pub trait PresentTransitionExt: crate::IntoElement + Sized {
    fn present_transition(self, id: impl Into<crate::ElementId>, enter: Option<Transition>) -> PresentTransitionElement<Self> {
        PresentTransitionElement {
            id: id.into(),
            element: Some(self),
            enter,
            exit: None,
            start: None,
        }
    }
}

impl<E: crate::IntoElement> PresentTransitionExt for E {}

impl<E: crate::IntoElement + 'static> crate::IntoElement for PresentTransitionElement<E> {
    type Element = PresentTransitionElement<E>;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: crate::ParentElement> crate::ParentElement for PresentTransitionElement<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = crate::AnyElement>) {
        if let Some(element) = &mut self.element {
            element.extend(elements);
        }
    }
}

impl<E: crate::IntoElement + 'static> crate::Element for PresentTransitionElement<E> {
    type RequestLayoutState = crate::AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        if let Some(global_id) = global_id {
            let start = window.with_element_state(global_id, |state: Option<Instant>, _| {
                let start = state.unwrap_or_else(Instant::now);
                (start, start)
            });
            self.start = Some(start);
        }
        let mut element = self
            .element
            .take()
            .expect("should only be called once")
            .into_any_element();
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> Self::PrepaintState {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let start = self.start.unwrap_or_else(Instant::now);
        let enter = self.enter;
        let paint = move |window: &mut crate::Window| {
            window.with_present_transition(enter.as_ref(), bounds, start, |window| element.paint(window, cx))
        };
        match self.exit {
            Some((key, exit)) => window.with_exit_transition(key, &exit, bounds, paint),
            None => paint(window),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{px, size};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn ease_out_slide_reaches_its_place_after_its_delay_and_duration() {
        let start = Instant::now();
        let t = Transition::ease(Duration::from_millis(200), CubicBezier(0.0, 0.0, 0.58, 1.0))
            .start(start)
            .delay(Duration::from_millis(10))
            .translate(point(px(0.0), px(10.0)), Point::default())
            .to_scene(Bounds::default(), start, 1.5);
        let t0 = crate::gradient_clock(start);
        assert_eq!(t.state(t0).offset, [0.0, 15.0]);
        assert_eq!(t.state(t0 + 0.005).offset, [0.0, 15.0]);
        let mid = t.state(t0 + 0.11).offset[1];
        assert!(mid > 0.0 && mid < 7.5, "{mid}");
        assert!(t.running(t0 + 0.2));
        assert!(!t.running(t0 + 0.211));
        assert!(t.state(t0 + 0.3).is_identity());
    }

    #[test]
    fn springs_start_at_rest_overshoot_when_underdamped_and_settle() {
        let under = SpringConfig::new(447.4, 31.0, 1.0);
        assert!(close(spring_progress(under, 0.0), 0.0));
        let peak = (1..100).map(|i| spring_progress(under, i as f32 * 0.005)).fold(0.0f32, f32::max);
        assert!(peak > 1.02 && peak < 1.05, "{peak}");
        let over = SpringConfig::new(1200.0, 80.0, 1.0);
        let max = (1..100).map(|i| spring_progress(over, i as f32 * 0.005)).fold(0.0f32, f32::max);
        assert!(max <= 1.0 + 1e-4, "{max}");
        let critical = SpringConfig::new(100.0, 20.0, 1.0);
        assert!(spring_progress(critical, 0.05) > 0.0);
        for config in [under, over, critical] {
            let end = spring_settle_time(config);
            assert!(end > 0.05 && end < 2.0, "{end}");
            assert!((spring_progress(config, end) - 1.0).abs() < 0.002);
        }
    }

    #[test]
    fn scale_keeps_the_origin_still_and_fades_every_colour() {
        let bounds = Bounds::new(point(px(100.0), px(100.0)), size(px(200.0), px(100.0)));
        let start = Instant::now();
        let t = Transition::ease(Duration::from_millis(100), CubicBezier::LINEAR)
            .start(start)
            .origin(0.5, 1.0)
            .scale(0.5, 1.0)
            .opacity(0.0, 1.0)
            .to_scene(bounds, start, 1.0);
        let s = t.state(crate::gradient_clock(start));
        assert_eq!(s.origin, [200.0, 200.0]);
        assert_eq!(s.point([200.0, 200.0]), [200.0, 200.0]);
        assert_eq!(s.point([100.0, 100.0]), [150.0, 150.0]);
        assert_eq!(s.opacity, 0.0);
        let b = s.bounds(Bounds::new(point(ScaledPixels(100.0), ScaledPixels(100.0)), size(ScaledPixels(200.0), ScaledPixels(100.0))));
        assert_eq!((b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0), (150.0, 150.0, 100.0, 50.0));
    }

    #[test]
    fn background_mixes_premultiplied_like_css() {
        let clear = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 };
        let white = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.5 };
        let half = Rgba::from(mix_premultiplied(clear, white, 0.5));
        assert!(close(half.a, 0.25) && close(half.r, 1.0), "{half:?}");
    }

    #[test]
    fn nested_states_compose_inner_then_outer() {
        let inner = TransitionState { offset: [10.0, 0.0], scale: 1.0, origin: [0.0, 0.0], opacity: 0.5, background: None, height: None, move_masks: false };
        let outer = TransitionState { offset: [0.0, 0.0], scale: 2.0, origin: [0.0, 0.0], opacity: 0.5, background: None, height: None, move_masks: true };
        let both = inner.then(&outer);
        assert_eq!(both.point([5.0, 5.0]), outer.point(inner.point([5.0, 5.0])));
        assert!(close(both.opacity, 0.25));
        assert!(!both.move_masks);
        let mask = ContentMask { bounds: Bounds::new(point(ScaledPixels(0.0), ScaledPixels(0.0)), size(ScaledPixels(10.0), ScaledPixels(10.0))) };
        assert_eq!(inner.mask(mask), mask);
    }

    #[test]
    fn height_resizes_quads_around_their_middle() {
        let start = Instant::now();
        let t = Transition::ease(Duration::from_millis(100), CubicBezier::LINEAR)
            .start(start)
            .height(px(8.0), px(40.0))
            .to_scene(Bounds::default(), start, 1.0);
        let s = t.state(crate::gradient_clock(start) + 0.05);
        assert!(close(s.height.unwrap(), 24.0));
        assert!(!s.is_identity());
    }
}
