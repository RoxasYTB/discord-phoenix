#![allow(missing_docs)]

use crate::{DevicePixels, Size, size};

// PHOENIX PATCH (text glow): CSS `filter: blur(σ)` of a monochrome glyph mask, a separable
// gaussian of standard deviation σ device pixels cut at 3σ, padded by that radius.
pub fn blur_padding(sigma: f32) -> i32 {
    if sigma > 0.0 {
        (sigma * 3.0).ceil() as i32
    } else {
        0
    }
}

fn kernel(sigma: f32, radius: usize) -> Vec<f32> {
    let weights: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let x = i as f32 - radius as f32;
            (-(x * x) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    weights.into_iter().map(|w| w / sum).collect()
}

pub fn blur_mask(
    mask_size: Size<DevicePixels>,
    mask: &[u8],
    sigma: f32,
) -> (Size<DevicePixels>, Vec<u8>) {
    let pad = blur_padding(sigma) as usize;
    let (w, h) = (
        mask_size.width.0.max(0) as usize,
        mask_size.height.0.max(0) as usize,
    );
    let (ow, oh) = (w + 2 * pad, h + 2 * pad);
    let out_size = size(DevicePixels(ow as i32), DevicePixels(oh as i32));
    if mask.len() < w * h {
        return (out_size, vec![0; ow * oh]);
    }
    if pad == 0 {
        return (out_size, mask[..w * h].to_vec());
    }
    let k = kernel(sigma, pad);
    let mut rows = vec![0f32; ow * h];
    for y in 0..h {
        let src = &mask[y * w..(y + 1) * w];
        let dst = &mut rows[y * ow..(y + 1) * ow];
        for (sx, &a) in src.iter().enumerate() {
            if a == 0 {
                continue;
            }
            let a = a as f32;
            for (i, weight) in k.iter().enumerate() {
                dst[sx + i] += a * weight;
            }
        }
    }
    let mut out = vec![0f32; ow * oh];
    for y in 0..h {
        for (i, weight) in k.iter().enumerate() {
            let dst = &mut out[(y + i) * ow..(y + i + 1) * ow];
            for (d, s) in dst.iter_mut().zip(&rows[y * ow..(y + 1) * ow]) {
                *d += s * weight;
            }
        }
    }
    (
        out_size,
        out.into_iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dp(w: i32, h: i32) -> Size<DevicePixels> {
        size(DevicePixels(w), DevicePixels(h))
    }

    #[test]
    fn padding_is_three_standard_deviations() {
        assert_eq!(blur_padding(6.0), 18);
        assert_eq!(blur_padding(4.0 * 1.25), 15);
        assert_eq!(blur_padding(0.0), 0);
    }

    #[test]
    fn blur_keeps_the_coverage_and_spreads_it_like_a_gaussian() {
        let mut mask = vec![0u8; 9 * 9];
        mask[4 * 9 + 4] = 255;
        let (s, out) = blur_mask(dp(9, 9), &mask, 2.0);
        assert_eq!(s, dp(9 + 12, 9 + 12));
        let total: u32 = out.iter().map(|&v| v as u32).sum();
        assert!((total as i32 - 255).abs() <= 40, "{total}");
        let c = 10 * 21 + 10;
        assert!(out[c] > out[c + 2] && out[c + 2] > out[c + 4]);
        assert_eq!(out[c - 2], out[c + 2]);
        assert_eq!(out[c - 2 * 21], out[c + 2]);
        let expected = 255.0 / (2.0 * std::f32::consts::PI * 4.0);
        assert!((out[c] as f32 - expected).abs() <= 1.5, "{} {expected}", out[c]);
    }

    #[test]
    fn a_full_mask_stays_opaque_in_its_middle_and_fades_at_its_edges() {
        let mask = vec![255u8; 40 * 40];
        let (s, out) = blur_mask(dp(40, 40), &mask, 4.0);
        let w = s.width.0 as usize;
        assert_eq!(out[(12 + 20) * w + 12 + 20], 255);
        let row = (12 + 20) * w;
        let edge = (out[row + 11] as i32 + out[row + 12] as i32) as f32 / 2.0;
        assert!((edge - 127.5).abs() <= 1.0, "{edge}");
        assert_eq!(out[0], 0);
    }

    #[test]
    fn no_blur_is_the_glyph_itself() {
        let (s, out) = blur_mask(dp(2, 1), &[10, 20], 0.0);
        assert_eq!((s, out), (dp(2, 1), vec![10, 20]));
    }
}
