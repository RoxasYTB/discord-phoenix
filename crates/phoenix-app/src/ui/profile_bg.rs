use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use futures::AsyncReadExt;
use gpui::{App, RenderImage};

const BAKE_W: u32 = 128;
const MASK_H: f32 = 280.;

type Cache = HashMap<(String, u32), Option<Arc<RenderImage>>>;

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(HashMap::new());
}

fn still_url(banner: &str) -> String {
    let (base, _) = banner.split_once('?').unwrap_or((banner, ""));
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    format!("{stem}.png?size=480")
}

pub fn get(banner: &str, w: f32, cx: &mut App) -> Option<(Arc<RenderImage>, f32)> {
    let url = still_url(banner);
    let key = (url.clone(), w.round() as u32);
    let hit = CACHE.with(|c| c.borrow().get(&key).cloned());
    match hit {
        Some(Some(img)) => {
            let sz = img.size(0);
            let h = if sz.width.0 > 0 { w * sz.height.0 as f32 / sz.width.0 as f32 } else { 0. };
            return Some((img, h));
        }
        Some(None) => return None,
        None => {}
    }
    CACHE.with(|c| c.borrow_mut().insert(key.clone(), None));
    let http = cx.http_client();
    cx.spawn(async move |cx| {
        let Ok(mut resp) = http.get(&url, Default::default(), true).await else { return };
        let mut bytes = Vec::new();
        if resp.body_mut().read_to_end(&mut bytes).await.is_err() {
            return;
        }
        let Some(img) = bake(&bytes, key.1 as f32) else { return };
        cx.update(|cx| {
            CACHE.with(|c| c.borrow_mut().insert(key, Some(Arc::new(img))));
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

fn saturate([r, g, b]: [f32; 3], s: f32) -> [f32; 3] {
    [
        (0.213 + 0.787 * s) * r + (0.715 - 0.715 * s) * g + (0.072 - 0.072 * s) * b,
        (0.213 - 0.213 * s) * r + (0.715 + 0.285 * s) * g + (0.072 - 0.072 * s) * b,
        (0.213 - 0.213 * s) * r + (0.715 - 0.715 * s) * g + (0.072 + 0.928 * s) * b,
    ]
}

fn alpha_at(y: f32) -> f32 {
    0.3 * (1. - y / MASK_H).clamp(0., 1.)
}

fn bake(bytes: &[u8], w_css: f32) -> Option<RenderImage> {
    let src = image::load_from_memory(bytes).ok()?.into_rgba8();
    if src.width() == 0 || src.height() == 0 || w_css <= 0. {
        return None;
    }
    let h = ((src.height() as u64 * BAKE_W as u64) / src.width() as u64).max(1) as u32;
    let mut buf = image::imageops::resize(&src, BAKE_W, h, image::imageops::FilterType::Triangle);
    let sigma = 15. * BAKE_W as f32 / w_css;
    buf = image::imageops::blur(&buf, sigma.max(0.5));
    let css_per_px = w_css / BAKE_W as f32;
    let rows = ((MASK_H / css_per_px).ceil() as u32 + 1).min(buf.height());
    let mut out = image::RgbaImage::new(BAKE_W, rows);
    for (x, y, p) in out.enumerate_pixels_mut() {
        let q = buf.get_pixel(x, y).0;
        let [r, g, b] = saturate([q[0] as f32, q[1] as f32, q[2] as f32], 0.5);
        let a = alpha_at((y as f32 + 0.5) * css_per_px) * q[3] as f32 / 255.;
        *p = image::Rgba([b.clamp(0., 255.) as u8, g.clamp(0., 255.) as u8, r.clamp(0., 255.) as u8, (a * 255.).round() as u8]);
    }
    Some(RenderImage::new(vec![image::Frame::new(out)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn still_png_url() {
        assert_eq!(still_url("https://cdn.discordapp.com/banners/1/a_ab.gif?size=600"), "https://cdn.discordapp.com/banners/1/a_ab.png?size=480");
    }

    #[test]
    fn mask_and_saturate() {
        assert!((alpha_at(0.) - 0.3).abs() < 1e-6);
        assert_eq!(alpha_at(280.), 0.);
        let g = saturate([255., 0., 0.], 0.5);
        assert!(g[0] < 255. && g[1] > 0.);
        assert_eq!(saturate([10., 10., 10.], 0.5).map(|v| v.round()), [10., 10., 10.]);
    }
}
