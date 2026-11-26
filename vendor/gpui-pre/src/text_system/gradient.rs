#![allow(missing_docs)]

use crate::{
    Bounds, GlyphGradient, GlyphGradientScroll, GlyphGradientStop, MAX_GLYPH_GRADIENT_STOPS,
    Pixels, Point, Rgba, Size,
};
use scheduler::Instant;
use std::{sync::OnceLock, time::Duration};

// PHOENIX PATCH (text gradient): a CSS `linear-gradient` painted through the glyphs of a
// text run (`background-clip: text`), sampled per device pixel by the glyph shader.
#[derive(Clone, Debug, PartialEq)]
pub struct TextGradient {
    pub angle: f32,
    pub stops: Vec<(Rgba, f32)>,
    pub tile: Bounds<Pixels>,
    pub repeat_x: bool,
    pub scroll: Option<GradientScroll>,
}

// PHOENIX PATCH (gradient scroll): a CSS animation of `background-position-x`, `speed`
// logical px per second from `start`, for `duration` (forever when `None`). The renderer
// applies it at present time, so the animation needs no new layout or paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientScroll {
    pub speed: f32,
    pub start: Instant,
    pub duration: Option<Duration>,
}

impl GradientScroll {
    pub fn new(speed: f32, start: Instant, duration: Option<Duration>) -> Self {
        Self {
            speed,
            start,
            duration,
        }
    }

    pub fn to_glyph_scroll(&self, scale_factor: f32) -> GlyphGradientScroll {
        GlyphGradientScroll {
            speed: self.speed * scale_factor,
            start: gradient_clock(self.start),
            duration: self.duration.map_or(f32::INFINITY, |d| d.as_secs_f32()),
        }
    }
}

pub fn gradient_clock(at: Instant) -> f64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = *EPOCH.get_or_init(Instant::now);
    match at.checked_duration_since(epoch) {
        Some(after) => after.as_secs_f64(),
        None => -epoch.duration_since(at).as_secs_f64(),
    }
}

impl TextGradient {
    pub fn new(angle: f32, tile: Bounds<Pixels>) -> Self {
        Self {
            angle,
            stops: Vec::new(),
            tile,
            repeat_x: false,
            scroll: None,
        }
    }

    pub fn scroll(mut self, scroll: Option<GradientScroll>) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn to_corner(right: bool, bottom: bool, tile: Bounds<Pixels>) -> Self {
        Self::new(corner_angle(tile.size, right, bottom), tile)
    }

    pub fn stop(mut self, color: impl Into<Rgba>, position: f32) -> Self {
        self.stops.push((color.into(), position));
        self
    }

    pub fn repeat_x(mut self, repeat: bool) -> Self {
        self.repeat_x = repeat;
        self
    }

    pub fn first_color(&self) -> Option<Rgba> {
        self.stops.first().map(|(c, _)| *c)
    }

    pub fn to_glyph_gradient(&self, scale_factor: f32, opacity: f32) -> GlyphGradient {
        let w = f32::from(self.tile.size.width);
        let h = f32::from(self.tile.size.height);
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let dir = [sin, -cos];
        let length = (w * sin).abs() + (h * cos).abs();
        let inv = if length > 0.0 { 1.0 / length } else { 0.0 };
        let offset = 0.5 - (w * 0.5 * dir[0] + h * 0.5 * dir[1]) * inv;
        let mut stops = [GlyphGradientStop::default(); MAX_GLYPH_GRADIENT_STOPS];
        let mut last = f32::NEG_INFINITY;
        let count = self.stops.len().min(MAX_GLYPH_GRADIENT_STOPS);
        for (slot, (color, position)) in stops.iter_mut().zip(&self.stops) {
            last = position.max(last);
            *slot = GlyphGradientStop {
                color: [color.r, color.g, color.b, color.a],
                position: last,
            };
        }
        GlyphGradient {
            origin: [
                f32::from(self.tile.origin.x) * scale_factor,
                f32::from(self.tile.origin.y) * scale_factor,
            ],
            step: [
                dir[0] * inv / scale_factor,
                dir[1] * inv / scale_factor,
            ],
            offset,
            period: if self.repeat_x { w * scale_factor } else { 0.0 },
            stop_count: count.max(1) as u32,
            opacity,
            stops,
        }
    }

    pub fn position_at(&self, point: Point<Pixels>) -> f32 {
        self.to_glyph_gradient(1.0, 1.0)
            .position_at(f32::from(point.x), f32::from(point.y))
    }

    pub fn color_at(&self, point: Point<Pixels>) -> Rgba {
        let g = self.to_glyph_gradient(1.0, 1.0);
        let [r, gr, b, a] = g.color_at_position(g.position_at(f32::from(point.x), f32::from(point.y)));
        Rgba { r, g: gr, b, a }
    }
}

pub fn corner_angle(size: Size<Pixels>, right: bool, bottom: bool) -> f32 {
    let w = f32::from(size.width);
    let h = f32::from(size.height);
    let sx = if right { 1.0 } else { -1.0 };
    let sy = if bottom { 1.0 } else { -1.0 };
    (sx * h).atan2(-sy * w).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px, rgb, size};

    fn tile(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn horizontal_gradient_runs_from_the_left_to_the_right_edge() {
        let g = TextGradient::new(90.0, tile(10.0, 5.0, 100.0, 20.0));
        assert!(close(g.position_at(point(px(10.0), px(5.0))), 0.0));
        assert!(close(g.position_at(point(px(60.0), px(25.0))), 0.5));
        assert!(close(g.position_at(point(px(110.0), px(9.0))), 1.0));
    }

    #[test]
    fn repeated_tiles_restart_every_period() {
        let g = TextGradient::new(90.0, tile(0.0, 0.0, 100.0, 20.0)).repeat_x(true);
        assert!(close(g.position_at(point(px(125.0), px(0.0))), 0.25));
        assert!(close(g.position_at(point(px(-25.0), px(0.0))), 0.75));
        let d = g.to_glyph_gradient(1.5, 1.0);
        assert!(close(d.period, 150.0));
        assert!(close(d.position_at(187.5, 3.0), 0.25));
    }

    #[test]
    fn to_bottom_right_puts_the_other_corners_on_the_middle_line() {
        let t = tile(0.0, 0.0, 35.0, 20.0);
        let g = TextGradient::to_corner(true, true, t);
        assert!(close(g.angle, 150.255_12));
        assert!(close(g.position_at(point(px(0.0), px(0.0))), 0.0));
        assert!(close(g.position_at(point(px(35.0), px(20.0))), 1.0));
        assert!(close(g.position_at(point(px(35.0), px(0.0))), 0.5));
        assert!(close(g.position_at(point(px(0.0), px(20.0))), 0.5));
        assert!(close(corner_angle(size(px(10.0), px(10.0)), true, true), 135.0));
        assert!(close(corner_angle(size(px(10.0), px(10.0)), true, false), 45.0));
    }

    #[test]
    fn device_pixels_map_back_to_the_logical_gradient() {
        let g = TextGradient::to_corner(true, true, tile(20.0, 40.0, 35.0, 20.0));
        let d = g.to_glyph_gradient(1.5, 1.0);
        for (x, y) in [(20.0, 40.0), (37.5, 50.0), (55.0, 60.0), (30.0, 58.0)] {
            assert!(close(d.position_at(x * 1.5, y * 1.5), g.position_at(point(px(x), px(y)))));
        }
    }

    #[test]
    fn stops_clamp_outside_and_interpolate_between() {
        let g = TextGradient::new(90.0, tile(0.0, 0.0, 100.0, 10.0))
            .stop(rgb(0x000000), 0.1)
            .stop(rgb(0xffffff), 0.9);
        assert_eq!(g.color_at(point(px(5.0), px(0.0))).r, 0.0);
        assert!(close(g.color_at(point(px(50.0), px(0.0))).r, 0.5));
        assert_eq!(g.color_at(point(px(95.0), px(0.0))).r, 1.0);
        let d = g.to_glyph_gradient(1.0, 0.5);
        assert!(close(d.color_at_position(0.5)[3], 0.5));
    }

    #[test]
    fn scroll_moves_the_tile_right_and_wraps_by_the_period() {
        let start = Instant::now();
        let g = TextGradient::new(90.0, tile(0.0, 0.0, 100.0, 20.0))
            .repeat_x(true)
            .scroll(Some(GradientScroll::new(100.0 / 1.5, start, None)));
        let scroll = g.scroll.unwrap().to_glyph_scroll(1.5);
        let d = g.to_glyph_gradient(1.5, 1.0);
        let t0 = gradient_clock(start);
        assert!(close(scroll.displacement(t0, d.period), 0.0));
        assert!(close(scroll.displacement(t0 + 0.75, d.period), 75.0));
        assert!(close(scroll.displacement(t0 + 1.5, d.period), 0.0));
        assert!(close(scroll.displacement(t0 + 3600.375, d.period), 37.5));
        assert!(close(scroll.displacement(t0 - 1.0, d.period), 0.0));
        let moved = scroll.apply(d, t0 + 0.375);
        assert!(close(moved.position_at(37.5, 0.0), 0.0));
        assert!(close(moved.position_at(112.5, 0.0), 0.5));
        assert!(scroll.running(t0 + 1000.0));
    }

    #[test]
    fn a_finite_scroll_stops_on_its_last_frame_like_fill_mode_forwards() {
        let start = Instant::now();
        let s = GradientScroll::new(-48.0, start, Some(Duration::from_secs(2))).to_glyph_scroll(1.0);
        let t0 = gradient_clock(start);
        assert!(close(s.displacement(t0 + 1.0, 96.0), 48.0));
        assert!(close(s.displacement(t0 + 0.5, 0.0), -24.0));
        assert!(close(s.displacement(t0 + 5.0, 0.0), -96.0));
        assert!(s.running(t0 + 1.9));
        assert!(!s.running(t0 + 2.1));
        assert!(!GlyphGradientScroll::default().running(t0));
    }

    #[test]
    fn the_clock_orders_instants_on_both_sides_of_its_epoch() {
        let now = Instant::now();
        let before = now - Duration::from_millis(1500);
        let a = gradient_clock(before);
        let b = gradient_clock(now);
        assert!(((b - a) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn stops_out_of_order_take_the_previous_position_like_css() {
        let g = TextGradient::new(90.0, tile(0.0, 0.0, 100.0, 10.0))
            .stop(rgb(0xff0000), 0.6)
            .stop(rgb(0x0000ff), 0.2);
        let d = g.to_glyph_gradient(1.0, 1.0);
        assert!(close(d.stops[1].position, 0.6));
        assert_eq!(d.color_at_position(0.59)[0], 1.0);
        assert_eq!(d.color_at_position(0.61)[2], 1.0);
    }
}
