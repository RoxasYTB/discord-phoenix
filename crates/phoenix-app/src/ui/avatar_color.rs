use std::cell::RefCell;
use std::collections::HashMap;

use futures::AsyncReadExt;
use gpui::App;

thread_local! {
    static CACHE: RefCell<HashMap<String, Option<u32>>> = RefCell::new(HashMap::new());
}

fn sample_url(avatar: &str) -> Option<String> {
    if !avatar.contains("/avatars/") && !avatar.contains("/guilds/") {
        return None;
    }
    let (base, _) = avatar.split_once('?').unwrap_or((avatar, ""));
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    Some(format!("{stem}.png?size=128"))
}

pub fn get(avatar: &str) -> Option<u32> {
    let key = sample_url(avatar)?;
    CACHE.with(|c| c.borrow().get(&key).copied().flatten())
}

pub fn ensure(avatar: &str, cx: &mut App) {
    let Some(url) = sample_url(avatar) else { return };
    let fresh = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.contains_key(&url) {
            false
        } else {
            c.insert(url.clone(), None);
            true
        }
    });
    if !fresh {
        return;
    }
    let http = cx.http_client();
    cx.spawn(async move |cx| {
        let Ok(mut resp) = http.get(&url, Default::default(), true).await else { return };
        let mut bytes = Vec::new();
        if resp.body_mut().read_to_end(&mut bytes).await.is_err() {
            return;
        }
        let Some(rgb) = dominant_png(&bytes) else { return };
        cx.update(|cx| {
            CACHE.with(|c| c.borrow_mut().insert(url, Some(rgb)));
            cx.refresh_windows();
        });
    })
    .detach();
}

pub fn dominant_png(bytes: &[u8]) -> Option<u32> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let px: Vec<[u8; 3]> = img
        .pixels()
        .step_by(10)
        .filter(|p| p[3] >= 125 && !(p[0] > 250 && p[1] > 250 && p[2] > 250))
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    let [r, g, b] = palette(&px, 5).first().copied().unwrap_or([0, 0, 0]);
    Some((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
}

const SIG: u32 = 5;
const SHIFT: u32 = 8 - SIG;

fn ix(r: i32, g: i32, b: i32) -> usize {
    ((r << (2 * SIG)) + (g << SIG) + b) as usize
}

#[derive(Clone, Copy)]
struct VBox {
    r1: i32,
    r2: i32,
    g1: i32,
    g2: i32,
    b1: i32,
    b2: i32,
}

impl VBox {
    fn volume(&self) -> i64 {
        i64::from(self.r2 - self.r1 + 1) * i64::from(self.g2 - self.g1 + 1) * i64::from(self.b2 - self.b1 + 1)
    }
    fn count(&self, h: &[u32]) -> i64 {
        let mut n = 0i64;
        for r in self.r1..=self.r2 {
            for g in self.g1..=self.g2 {
                for b in self.b1..=self.b2 {
                    n += i64::from(h[ix(r, g, b)]);
                }
            }
        }
        n
    }
    fn avg(&self, h: &[u32]) -> [u8; 3] {
        let mult = f64::from(1u32 << SHIFT);
        let (mut tot, mut rs, mut gs, mut bs) = (0f64, 0f64, 0f64, 0f64);
        for r in self.r1..=self.r2 {
            for g in self.g1..=self.g2 {
                for b in self.b1..=self.b2 {
                    let n = f64::from(h[ix(r, g, b)]);
                    tot += n;
                    rs += n * (f64::from(r) + 0.5) * mult;
                    gs += n * (f64::from(g) + 0.5) * mult;
                    bs += n * (f64::from(b) + 0.5) * mult;
                }
            }
        }
        let c = |v: f64| v.trunc().clamp(0., 255.) as u8;
        if tot > 0. {
            [c(rs / tot), c(gs / tot), c(bs / tot)]
        } else {
            let m = |a: i32, b: i32| c(mult * f64::from(a + b + 1) / 2.);
            [m(self.r1, self.r2), m(self.g1, self.g2), m(self.b1, self.b2)]
        }
    }
}

fn median_cut(h: &[u32], v: &VBox) -> Option<Vec<VBox>> {
    let count = v.count(h);
    if count == 0 {
        return None;
    }
    let (rw, gw, bw) = (v.r2 - v.r1 + 1, v.g2 - v.g1 + 1, v.b2 - v.b1 + 1);
    let maxw = rw.max(gw).max(bw);
    if count == 1 {
        return Some(vec![*v]);
    }
    let axis = if maxw == rw {
        0
    } else if maxw == gw {
        1
    } else {
        2
    };
    let (lo, hi) = match axis {
        0 => (v.r1, v.r2),
        1 => (v.g1, v.g2),
        _ => (v.b1, v.b2),
    };
    let mut partial: [Option<i64>; 34] = [None; 34];
    let mut total = 0i64;
    for i in lo..=hi {
        let mut sum = 0i64;
        let (a1, a2, b1, b2) = match axis {
            0 => (v.g1, v.g2, v.b1, v.b2),
            1 => (v.r1, v.r2, v.b1, v.b2),
            _ => (v.r1, v.r2, v.g1, v.g2),
        };
        for a in a1..=a2 {
            for b in b1..=b2 {
                sum += i64::from(match axis {
                    0 => h[ix(i, a, b)],
                    1 => h[ix(a, i, b)],
                    _ => h[ix(a, b, i)],
                });
            }
        }
        total += sum;
        partial[i as usize] = Some(total);
    }
    let p = |k: i32| if (0..34).contains(&k) { partial[k as usize].unwrap_or(0) } else { 0 };
    let look = |k: i32| total - p(k);
    for i in lo..=hi {
        if (p(i) as f64) > total as f64 / 2. {
            let (left, right) = (i - lo, hi - i);
            let mut d = if left <= right {
                (hi - 1).min((f64::from(i) + f64::from(right) / 2.).trunc() as i32)
            } else {
                lo.max((f64::from(i) - 1. - f64::from(left) / 2.).trunc() as i32)
            };
            while p(d) == 0 {
                d += 1;
            }
            let mut c2 = look(d);
            while c2 == 0 && p(d - 1) != 0 {
                d -= 1;
                c2 = look(d);
            }
            let (mut v1, mut v2) = (*v, *v);
            match axis {
                0 => {
                    v1.r2 = d;
                    v2.r1 = d + 1;
                }
                1 => {
                    v1.g2 = d;
                    v2.g1 = d + 1;
                }
                _ => {
                    v1.b2 = d;
                    v2.b1 = d + 1;
                }
            }
            return Some(vec![v1, v2]);
        }
    }
    None
}

fn iterate(q: &mut Vec<VBox>, h: &[u32], target: f64, key: &dyn Fn(&VBox) -> i64) {
    let mut ncolors = 1;
    let mut niters = 0;
    while niters < 1000 {
        q.sort_by_key(|v| key(v));
        let Some(v) = q.pop() else { return };
        if v.count(h) == 0 {
            q.push(v);
            niters += 1;
            continue;
        }
        let Some(cut) = median_cut(h, &v) else { return };
        q.push(cut[0]);
        if let Some(v2) = cut.get(1) {
            q.push(*v2);
            ncolors += 1;
        }
        if f64::from(ncolors) >= target {
            return;
        }
        let over = niters > 1000;
        niters += 1;
        if over {
            return;
        }
    }
}

pub fn palette(px: &[[u8; 3]], max: usize) -> Vec<[u8; 3]> {
    if px.is_empty() || !(2..=256).contains(&max) {
        return vec![[0, 0, 0]];
    }
    let mut h = vec![0u32; 1 << (3 * SIG)];
    for p in px {
        h[ix(i32::from(p[0] >> SHIFT), i32::from(p[1] >> SHIFT), i32::from(p[2] >> SHIFT))] += 1;
    }
    let mut v = VBox { r1: 1_000_000, r2: 0, g1: 1_000_000, g2: 0, b1: 1_000_000, b2: 0 };
    for p in px {
        let (r, g, b) = (i32::from(p[0] >> SHIFT), i32::from(p[1] >> SHIFT), i32::from(p[2] >> SHIFT));
        if r < v.r1 {
            v.r1 = r
        } else if r > v.r2 {
            v.r2 = r
        }
        if g < v.g1 {
            v.g1 = g
        } else if g > v.g2 {
            v.g2 = g
        }
        if b < v.b1 {
            v.b1 = b
        } else if b > v.b2 {
            v.b2 = b
        }
    }
    let hh = h.as_slice();
    let mut q = vec![v];
    iterate(&mut q, hh, 0.75 * max as f64, &|b| b.count(hh));
    let cv = |b: &VBox| b.count(hh) * b.volume();
    let mut q2 = q;
    q2.sort_by_key(cv);
    let rest = max as f64 - q2.len() as f64;
    iterate(&mut q2, hh, rest, &cv);
    q2.sort_by_key(cv);
    q2.iter().rev().map(|b| b.avg(hh)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_colour_image() {
        let mut px = vec![[120u8, 20, 30]; 70];
        px.extend(std::iter::repeat_n([20u8, 40, 200], 30));
        let p = palette(&px, 5);
        assert_eq!(p[0], [124, 20, 28]);
        assert!(p.contains(&[20, 44, 204]));
    }

    #[test]
    fn empty_is_black() {
        assert_eq!(palette(&[], 5), vec![[0, 0, 0]]);
    }

    #[test]
    fn sample_url_is_static_png() {
        assert_eq!(
            sample_url("https://cdn.discordapp.com/avatars/1/a_abc.gif?size=80").as_deref(),
            Some("https://cdn.discordapp.com/avatars/1/a_abc.png?size=128")
        );
        assert!(sample_url("https://example.com/x.png").is_none());
    }
}
