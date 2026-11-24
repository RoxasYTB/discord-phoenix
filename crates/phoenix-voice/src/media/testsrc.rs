use std::sync::Arc;

use super::VideoFrame;

const BARS: [[u8; 3]; 7] = [[191, 191, 191], [191, 191, 0], [0, 191, 191], [0, 191, 0], [191, 0, 191], [191, 0, 0], [0, 0, 191]];

pub struct TestPattern {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    n: u64,
}

impl TestPattern {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self { width, height, fps: fps.max(1), n: 0 }
    }

    pub fn pixel(&self, x: u32, y: u32, n: u64) -> [u8; 4] {
        let (w, h) = (self.width.max(1), self.height.max(1));
        let side = (h / 4).max(4);
        let travel = w.saturating_sub(side).max(1) as u64;
        let pos = (n * 6) % (travel * 2);
        let bx = (if pos < travel { pos } else { travel * 2 - pos }) as u32;
        let by = h / 2 - side / 2;
        if x >= bx && x < bx + side && y >= by && y < by + side {
            return [255, 255, 255, 255];
        }
        if y >= h - h / 8 {
            let bit = (x * 16 / w) as u64;
            let on = (n >> (15 - bit)) & 1 == 1;
            return if on { [235, 235, 235, 255] } else { [16, 16, 16, 255] };
        }
        let c = BARS[((x * 7 / w) as usize).min(6)];
        [c[0], c[1], c[2], 255]
    }

    pub fn frame(&mut self) -> VideoFrame {
        let (w, h) = (self.width as usize, self.height as usize);
        let n = self.n;
        let bars: Vec<u8> = (0..self.width)
            .flat_map(|x| {
                let c = BARS[((x * 7 / self.width.max(1)) as usize).min(6)];
                [c[0], c[1], c[2], 255]
            })
            .collect();
        let counter: Vec<u8> = (0..self.width).flat_map(|x| self.pixel(x, self.height - 1, n)).collect();
        let side = (self.height / 4).max(4) as usize;
        let travel = w.saturating_sub(side).max(1) as u64;
        let pos = (n * 6) % (travel * 2);
        let bx = (if pos < travel { pos } else { travel * 2 - pos }) as usize;
        let by = h / 2 - side / 2;
        let bottom = h - h / 8;
        let mut data = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            if y >= bottom {
                data.extend_from_slice(&counter);
                continue;
            }
            let start = data.len();
            data.extend_from_slice(&bars);
            if y >= by && y < by + side {
                let a = start + bx * 4;
                let b = start + (bx + side).min(w) * 4;
                data[a..b].fill(255);
            }
        }
        let pts_us = n * 1_000_000 / self.fps as u64;
        self.n += 1;
        VideoFrame { width: self.width, height: self.height, pts_us, data: Arc::from(data) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_have_rgba_size_and_move() {
        let mut t = TestPattern::new(64, 36, 30);
        let a = t.frame();
        let b = t.frame();
        assert_eq!(a.data.len(), 64 * 36 * 4);
        assert_ne!(a.data, b.data);
        assert_eq!(b.pts_us, 33_333);
        let p = TestPattern::new(64, 36, 30);
        for y in [0u32, 20, 35] {
            for x in [0u32, 10, 63] {
                let i = ((y * 64 + x) * 4) as usize;
                assert_eq!(a.data[i..i + 4], p.pixel(x, y, 0), "{x},{y}");
            }
        }
    }
}
