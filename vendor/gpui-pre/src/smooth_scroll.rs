// PHOENIX PATCH (smooth wheel): animated mouse wheel scrolling, after Chromium's
// `cc::ScrollOffsetAnimationCurve` with `DurationBehavior::kInverseDelta`.

use crate::{App, Pixels, Point, Window, point, px};
use scheduler::Instant;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

const DURATION_DIVISOR: f64 = 60.0;
const RAMP_START_PX: f64 = 120.0;
const RAMP_END_PX: f64 = 480.0;
const MIN_FRAMES: f64 = 6.0;
const MAX_FRAMES: f64 = 12.0;
const EPSILON: f64 = 0.01;

/// Turns animated wheel scrolling on or off for every scroll container of the app.
/// Off by default. Only wheel notches (`ScrollDelta::Lines`) are animated; precise
/// deltas from touchpads and touch pans keep moving the content directly.
pub fn set_smooth_wheel_scroll(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether animated wheel scrolling is on, see [`set_smooth_wheel_scroll`].
pub fn smooth_wheel_scroll_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn max_dimension(v: (f64, f64)) -> f64 {
    if v.0.abs() > v.1.abs() { v.0 } else { v.1 }
}

/// Duration in seconds of a wheel scroll segment of `delta` pixels: 12 frames at 60 Hz up
/// to 120 px, 6 frames from 480 px, linear in between.
pub fn wheel_segment_duration(delta: f64) -> f64 {
    let slope = (MIN_FRAMES - MAX_FRAMES) / (RAMP_END_PX - RAMP_START_PX);
    let offset = MAX_FRAMES - RAMP_START_PX * slope;
    (offset + delta.abs() * slope).clamp(MIN_FRAMES, MAX_FRAMES) / DURATION_DIVISOR
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EaseInOut {
    x1: f64,
    y1: f64,
}

impl EaseInOut {
    const X2: f64 = 0.58;
    const Y2: f64 = 1.0;

    fn with_initial_slope(slope: f64) -> Self {
        let slope = slope.clamp(-1000.0, 1000.0);
        Self {
            x1: 0.42,
            y1: slope * 0.42,
        }
    }

    fn curve(a: f64, b: f64, t: f64) -> f64 {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    }

    fn derivative(a: f64, b: f64, t: f64) -> f64 {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    }

    fn solve(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        let mut t = x;
        for _ in 0..8 {
            let err = Self::curve(self.x1, Self::X2, t) - x;
            if err.abs() < 1e-7 {
                return t;
            }
            let d = Self::derivative(self.x1, Self::X2, t);
            if d.abs() < 1e-7 {
                break;
            }
            t = (t - err / d).clamp(0.0, 1.0);
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        t = x;
        for _ in 0..50 {
            let v = Self::curve(self.x1, Self::X2, t);
            if (v - x).abs() < 1e-7 {
                break;
            }
            if v < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.0;
        }
        t
    }

    fn value(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        Self::curve(self.y1, Self::Y2, self.solve(x))
    }

    fn velocity(&self, x: f64) -> f64 {
        let t = self.solve(x);
        let dx = Self::derivative(self.x1, Self::X2, t);
        if dx.abs() < 1e-9 {
            return 0.0;
        }
        Self::derivative(self.y1, Self::Y2, t) / dx
    }
}

/// The offset of one wheel scroll animation over time, in pixels from where it started.
/// New notches retarget it and keep its current velocity, like Chromium's `UpdateTarget`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelCurve {
    origin: Instant,
    initial: (f64, f64),
    target: (f64, f64),
    last_retarget: f64,
    total: f64,
    ease: EaseInOut,
}

impl WheelCurve {
    /// Starts an animation of `delta` pixels at `now`.
    pub fn new(delta: Point<Pixels>, now: Instant) -> Self {
        let delta = (f64::from(f32::from(delta.x)), f64::from(f32::from(delta.y)));
        Self {
            origin: now,
            initial: (0.0, 0.0),
            target: delta,
            last_retarget: 0.0,
            total: wheel_segment_duration(max_dimension(delta)),
            ease: EaseInOut::with_initial_slope(0.0),
        }
    }

    fn time(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.origin).as_secs_f64()
    }

    fn value_at(&self, t: f64) -> (f64, f64) {
        let duration = self.total - self.last_retarget;
        if t >= self.total || duration <= 0.0 {
            return self.target;
        }
        let p = self.ease.value(((t - self.last_retarget) / duration).clamp(0.0, 1.0));
        (
            self.initial.0 + (self.target.0 - self.initial.0) * p,
            self.initial.1 + (self.target.1 - self.initial.1) * p,
        )
    }

    fn velocity_at(&self, t: f64) -> f64 {
        let duration = self.total - self.last_retarget;
        if duration <= 0.0 || t >= self.total {
            return 0.0;
        }
        let delta = (self.target.0 - self.initial.0, self.target.1 - self.initial.1);
        self.ease
            .velocity(((t - self.last_retarget) / duration).clamp(0.0, 1.0))
            * max_dimension(delta)
            / duration
    }

    /// The offset reached at `now`.
    pub fn value(&self, now: Instant) -> Point<Pixels> {
        let (x, y) = self.value_at(self.time(now));
        point(px(x as f32), px(y as f32))
    }

    /// The offset the animation ends at.
    pub fn target(&self) -> Point<Pixels> {
        point(px(self.target.0 as f32), px(self.target.1 as f32))
    }

    /// Whether the animation has reached its target at `now`.
    pub fn done(&self, now: Instant) -> bool {
        self.time(now) >= self.total
    }

    /// Adds `delta` pixels to the target at `now`.
    pub fn add(&mut self, delta: Point<Pixels>, now: Instant) {
        let delta = (f64::from(f32::from(delta.x)), f64::from(f32::from(delta.y)));
        let t = self.time(now);
        let new_target = (self.target.0 + delta.0, self.target.1 + delta.1);
        if t >= self.total {
            self.initial = self.target;
            self.target = new_target;
            self.last_retarget = t;
            self.total = t + wheel_segment_duration(max_dimension(delta));
            self.ease = EaseInOut::with_initial_slope(0.0);
            return;
        }
        let current = self.value_at(t);
        let new_delta = (new_target.0 - current.0, new_target.1 - current.1);
        let new_max = max_dimension(new_delta);
        if new_max.abs() < EPSILON {
            self.last_retarget = t;
            self.total = t;
            self.target = new_target;
            return;
        }
        let velocity = self.velocity_at(t);
        let bound = if velocity.abs() < EPSILON {
            f64::INFINITY
        } else {
            let b = new_max / velocity * 2.5;
            if b < 0.0 { f64::INFINITY } else { b }
        };
        let new_duration = wheel_segment_duration(new_max).min(bound);
        if new_duration < EPSILON {
            self.target = new_target;
            self.total = t;
            return;
        }
        self.ease = EaseInOut::with_initial_slope(velocity * new_duration / new_max);
        self.initial = current;
        self.target = new_target;
        self.total = t + new_duration;
        self.last_retarget = t;
    }
}

pub(crate) struct SmoothScroll {
    key: usize,
    curve: WheelCurve,
    applied: Point<Pixels>,
    apply: Box<dyn FnMut(Point<Pixels>, &mut Window, &mut App)>,
}

impl Window {
    /// Scrolls the container identified by `key` by `delta` pixels over the next frames,
    /// calling `apply` on each frame with the part of the delta for that frame. Further
    /// calls with the same key while it runs retarget the same animation.
    pub fn smooth_wheel_scroll(
        &mut self,
        key: usize,
        delta: Point<Pixels>,
        apply: impl FnMut(Point<Pixels>, &mut Window, &mut App) + 'static,
    ) {
        let now = Instant::now();
        let mut scrolls = self.smooth_scrolls.borrow_mut();
        match scrolls.iter_mut().find(|s| s.key == key) {
            Some(scroll) => {
                scroll.curve.add(delta, now);
                scroll.apply = Box::new(apply);
            }
            None => scrolls.push(SmoothScroll {
                key,
                curve: WheelCurve::new(delta, now),
                applied: Point::default(),
                apply: Box::new(apply),
            }),
        }
        drop(scrolls);
        if !self.smooth_scroll_ticking.replace(true) {
            self.on_next_frame(|window, cx| window.step_smooth_scrolls(cx));
        }
    }

    /// Whether a wheel scroll animation is running in this window.
    pub fn smooth_scrolling(&self) -> bool {
        !self.smooth_scrolls.borrow().is_empty()
    }

    pub(crate) fn step_smooth_scrolls(&mut self, cx: &mut App) {
        self.smooth_scroll_ticking.set(false);
        let now = Instant::now();
        let mut scrolls = std::mem::take(&mut *self.smooth_scrolls.borrow_mut());
        for scroll in &mut scrolls {
            let value = scroll.curve.value(now);
            let step = value - scroll.applied;
            scroll.applied = value;
            if step != Point::default() {
                (scroll.apply)(step, self, cx);
            }
        }
        scrolls.retain(|s| !s.curve.done(now));
        let mut current = self.smooth_scrolls.borrow_mut();
        for added in current.drain(..) {
            match scrolls.iter_mut().find(|s| s.key == added.key) {
                Some(scroll) => {
                    scroll.curve.add(added.curve.target() - added.applied, now);
                    scroll.apply = added.apply;
                }
                None => scrolls.push(added),
            }
        }
        *current = scrolls;
        let running = !current.is_empty();
        drop(current);
        if running && !self.smooth_scroll_ticking.replace(true) {
            self.on_next_frame(|window, cx| window.step_smooth_scrolls(cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    #[test]
    fn segment_duration_follows_the_inverse_delta_ramp() {
        assert!((wheel_segment_duration(60.0) - 0.2).abs() < 1e-9);
        assert!((wheel_segment_duration(120.0) - 0.2).abs() < 1e-9);
        assert!((wheel_segment_duration(300.0) - 0.15).abs() < 1e-9);
        assert!((wheel_segment_duration(-480.0) - 0.1).abs() < 1e-9);
        assert!((wheel_segment_duration(2000.0) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn one_notch_eases_in_and_out_over_two_hundred_ms() {
        let t0 = Instant::now();
        let c = WheelCurve::new(point(px(0.), px(60.)), t0);
        assert_eq!(c.value(t0).y, px(0.));
        let quarter = f32::from(c.value(at(t0, 50)).y);
        let half = f32::from(c.value(at(t0, 100)).y);
        assert!(quarter > 3.0 && quarter < 15.0, "{quarter}");
        assert!((half - 30.0).abs() < 1.0, "{half}");
        assert!(!c.done(at(t0, 199)));
        assert!(c.done(at(t0, 200)));
        assert_eq!(c.value(at(t0, 250)).y, px(60.));
    }

    #[test]
    fn a_second_notch_keeps_the_velocity_and_extends_the_target() {
        let t0 = Instant::now();
        let mut c = WheelCurve::new(point(px(0.), px(60.)), t0);
        let before = f32::from(c.value(at(t0, 80)).y);
        let v_before = f32::from(c.value(at(t0, 84)).y) - f32::from(c.value(at(t0, 76)).y);
        c.add(point(px(0.), px(60.)), at(t0, 80));
        let after = f32::from(c.value(at(t0, 80)).y);
        assert!((before - after).abs() < 1e-3);
        let v_after = f32::from(c.value(at(t0, 84)).y) - f32::from(c.value(at(t0, 80)).y);
        assert!(v_after > 0.0 && (v_after * 2.0 - v_before).abs() < v_before * 0.5, "{v_before} {v_after}");
        assert!(!c.done(at(t0, 200)));
        assert_eq!(c.value(at(t0, 1000)).y, px(120.));
    }

    #[test]
    fn reversing_direction_restarts_from_the_current_offset() {
        let t0 = Instant::now();
        let mut c = WheelCurve::new(point(px(0.), px(60.)), t0);
        let mid = c.value(at(t0, 100)).y;
        c.add(point(px(0.), px(-60.)), at(t0, 100));
        assert!((f32::from(c.value(at(t0, 100)).y) - f32::from(mid)).abs() < 1e-3);
        assert_eq!(c.value(at(t0, 1000)).y, px(0.));
    }

    #[test]
    fn a_notch_after_the_end_starts_a_fresh_segment() {
        let t0 = Instant::now();
        let mut c = WheelCurve::new(point(px(0.), px(-60.)), t0);
        assert!(c.done(at(t0, 300)));
        c.add(point(px(0.), px(-60.)), at(t0, 300));
        assert_eq!(c.value(at(t0, 300)).y, px(-60.));
        assert!((f32::from(c.value(at(t0, 400)).y) + 90.0).abs() < 1.0);
        assert_eq!(c.value(at(t0, 600)).y, px(-120.));
    }
}
