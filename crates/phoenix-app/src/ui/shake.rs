use std::{cell::Cell, time::Instant};

thread_local! {
    static SHAKE: Cell<Option<(Instant, u32, f32)>> = const { Cell::new(None) };
}

pub fn start(duration_ms: u32, intensity: f32) {
    SHAKE.with(|s| s.set(Some((Instant::now(), duration_ms, intensity))));
}

fn frame_offset(frame: u32, intensity: f32) -> (f32, f32) {
    let step = frame / 2;
    let dir = if step.is_multiple_of(2) { -1. } else { 1. };
    let mut h = step.wrapping_mul(2_654_435_761) ^ 0x9e37_79b9;
    h ^= h << 13;
    h ^= h >> 17;
    h ^= h << 5;
    let r = (h % 2001) as f32 / 1000. - 1.;
    (dir * intensity, r * intensity)
}

pub fn offset(window: &mut gpui::Window, reduced_motion: bool) -> Option<(f32, f32)> {
    let (t, dur, i) = SHAKE.with(Cell::get)?;
    let ms = t.elapsed().as_millis() as u32;
    if ms > dur || reduced_motion || !window.is_window_active() {
        SHAKE.with(|s| s.set(None));
        return None;
    }
    window.request_animation_frame();
    Some(frame_offset(ms * 60 / 1000, i))
}

#[cfg(test)]
mod tests {
    use super::frame_offset;

    #[test]
    fn x_alternates_and_y_stays_in_range() {
        let a = frame_offset(0, 2.);
        let b = frame_offset(2, 2.);
        assert_eq!(a.0, -2.);
        assert_eq!(b.0, 2.);
        for f in 0..40 {
            let (_, y) = frame_offset(f, 2.);
            assert!((-2. ..=2.).contains(&y));
        }
    }
}
