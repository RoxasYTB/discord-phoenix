use std::{cell::RefCell, time::Instant};

use gpui::{App, EntityId, ListState, Window, px};

const STEP: f64 = 1. / 240.;
const TENSION: f64 = 200.;
const FRICTION: f64 = 35.;
const MASS: f64 = 2.;
const THRESHOLD: f64 = 0.001;
const MAX_LAG: f64 = 2.;
pub const PAGE_FRACTION: f32 = 0.9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub from: f64,
    pub target: f64,
    vel: f64,
    acc: f64,
    last: Option<Instant>,
}

impl Spring {
    pub fn new(from: f64, target: f64) -> Self {
        Self { from, target, vel: 0., acc: 0., last: None }
    }

    pub fn retarget(&mut self, from: f64, target: f64) {
        self.from = from;
        self.target = target;
    }

    fn integrate(&self, vel: f64, from: f64) -> (f64, f64, f64) {
        let accel = (-TENSION * (from - self.target) - FRICTION * vel) / MASS;
        let vel = vel + accel * STEP;
        (from + vel * STEP, vel, accel)
    }

    pub fn advance(&mut self, now: Instant) -> (f64, bool) {
        let Some(last) = self.last.replace(now) else {
            return (self.from, false);
        };
        self.acc = (self.acc + now.saturating_duration_since(last).as_secs_f64()).min(MAX_LAG);
        while self.acc > STEP {
            self.acc -= STEP;
            let (next, vel, accel) = self.integrate(self.vel, self.from);
            self.vel = vel;
            let crossed = next == self.target
                || (next < self.target && self.from > self.target)
                || (next > self.target && self.from < self.target);
            if crossed || (accel * STEP).abs() < THRESHOLD {
                self.from = self.target;
                return (self.target, true);
            }
            self.from = next;
        }
        let mut shown = self.from;
        if self.acc > 0. {
            let (next, ..) = self.integrate(self.vel, self.from);
            shown += (next - self.from) * (self.acc / STEP);
        }
        (shown, false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Goal {
    By,
    Top,
}

struct Run {
    list: ListState,
    view: EntityId,
    goal: Goal,
    spring: Spring,
    shown: f64,
}

thread_local! {
    static RUN: RefCell<Option<Run>> = const { RefCell::new(None) };
}

fn top_of(list: &ListState) -> f64 {
    -f64::from(f32::from(list.scroll_px_offset_for_scrollbar().y))
}

fn below_of(list: &ListState) -> f64 {
    (f64::from(f32::from(list.max_offset_for_scrollbar().y)) - top_of(list)).max(0.)
}

pub fn page(list: &ListState, up: bool, view: EntityId, window: &mut Window, reduced: bool) {
    let h = f32::from(list.viewport_bounds().size.height);
    let d = f64::from(if h > 0. { h * PAGE_FRACTION } else { 400. });
    if list.max_offset_for_scrollbar().y <= px(0.) {
        return;
    }
    let by = if up {
        let top = list.logical_scroll_top();
        if top.item_ix == 0 && top.offset_in_item < px(0.5) {
            return;
        }
        -d
    } else {
        if list.is_scrolled_to_end() == Some(true) {
            return;
        }
        let below = below_of(list);
        if below > 1. { d.min(below) } else { d }
    };
    start(list, Goal::By, by, view, window, reduced);
}

pub const JUMP_BUFFER: f32 = 32.;

pub fn middle_offset(rel: f32, node_h: f32, view_h: f32, zoom: f32) -> f32 {
    (rel - 0.5 * view_h + 0.5 * node_h - 8. * zoom).min(rel - JUMP_BUFFER * zoom)
}

pub fn animate_by(list: &ListState, by: f64, view: EntityId, window: &mut Window, reduced: bool) {
    let by = by.clamp(-top_of(list), below_of(list));
    if by.abs() < 0.5 {
        return;
    }
    start(list, Goal::By, by, view, window, reduced);
}

pub fn to_top(list: &ListState, view: EntityId, window: &mut Window, reduced: bool) {
    start(list, Goal::Top, -top_of(list), view, window, reduced);
}

fn start(list: &ListState, goal: Goal, by: f64, view: EntityId, window: &mut Window, reduced: bool) {
    if reduced {
        stop();
        let list = list.clone();
        window.on_next_frame(move |window, cx| {
            list.scroll_by_notify(px(by as f32), view, window, cx);
            if goal == Goal::Top {
                list.scroll_to(gpui::ListOffset { item_ix: 0, offset_in_item: px(0.) });
            }
        });
        return;
    }
    let idle = RUN.with(|r| {
        let mut r = r.borrow_mut();
        let idle = r.is_none();
        match r.as_mut().filter(|run| run.view == view) {
            Some(run) => {
                run.goal = goal;
                run.spring.retarget(run.shown, run.shown + by);
            }
            None => *r = Some(Run { list: list.clone(), view, goal, spring: Spring::new(0., by), shown: 0. }),
        }
        idle
    });
    if idle {
        window.on_next_frame(step);
    }
}

pub fn stop() {
    RUN.with(|r| r.borrow_mut().take());
}

fn step(window: &mut Window, cx: &mut App) {
    let now = Instant::now();
    let Some((list, view, delta, done, goal)) = RUN.with(|r| {
        let mut r = r.borrow_mut();
        let run = r.as_mut()?;
        if run.goal == Goal::Top {
            run.spring.target = run.shown - top_of(&run.list);
        }
        let (pos, done) = run.spring.advance(now);
        let delta = pos - run.shown;
        run.shown = pos;
        let out = (run.list.clone(), run.view, delta, done, run.goal);
        if done {
            *r = None;
        }
        Some(out)
    }) else {
        return;
    };
    list.scroll_by_notify(px(delta as f32), view, window, cx);
    if !done {
        window.on_next_frame(step);
    } else if goal == Goal::Top && top_of(&list) > 0.5 {
        list.scroll_to(gpui::ListOffset { item_ix: 0, offset_in_item: px(0.) });
        cx.notify(view);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn run(mut s: Spring, frame_ms: u64) -> Vec<(u64, f64)> {
        let t0 = Instant::now();
        let mut out = vec![(0, s.advance(t0).0)];
        for i in 1..400 {
            let (pos, done) = s.advance(t0 + Duration::from_millis(i * frame_ms));
            out.push((i * frame_ms, pos));
            if done {
                break;
            }
        }
        out
    }

    #[test]
    fn unread_jump_centres_the_message_like_the_client_middle_orientation() {
        assert_eq!(middle_offset(900., 40., 600., 1.), 612.);
        assert_eq!(middle_offset(900., 600., 600., 1.), 868.);
        assert_eq!(middle_offset(-300., 40., 600., 1.5), -592.);
    }

    #[test]
    fn a_page_settles_on_its_target_without_overshoot_like_the_client_spring() {
        let frames = run(Spring::new(1000., 460.), 16);
        let (end_ms, end) = *frames.last().unwrap();
        assert_eq!(end, 460.);
        assert!((350..=900).contains(&end_ms), "{end_ms}");
        assert!(frames.windows(2).all(|w| w[1].1 <= w[0].1), "{frames:?}");
        assert!(frames.iter().all(|(_, p)| *p >= 460.));
        let half = frames.iter().find(|(_, p)| *p <= 730.).unwrap().0;
        assert!((80..=260).contains(&half), "{half}");
    }

    #[test]
    fn the_first_frame_only_starts_the_clock() {
        let mut s = Spring::new(0., 100.);
        let t0 = Instant::now();
        assert_eq!(s.advance(t0), (0., false));
        let (p, done) = s.advance(t0 + Duration::from_millis(16));
        assert!(!done && p > 0. && p < 10., "{p}");
    }

    #[test]
    fn a_second_page_keeps_the_velocity_and_moves_the_target() {
        let mut s = Spring::new(0., 540.);
        let t0 = Instant::now();
        s.advance(t0);
        let (p, _) = s.advance(t0 + Duration::from_millis(100));
        s.retarget(p, p + 540.);
        let (q, _) = s.advance(t0 + Duration::from_millis(116));
        let fresh = {
            let mut f = Spring::new(p, p + 540.);
            f.advance(t0 + Duration::from_millis(100));
            f.advance(t0 + Duration::from_millis(116)).0
        };
        assert!(q - p > fresh - p, "{q} {fresh} {p}");
    }
}
