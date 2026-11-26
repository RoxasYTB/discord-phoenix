use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use futures::AsyncReadExt;
use gpui::{App, ImageCacheError, ImageSource, RenderImage, SharedString, Window};

pub const TO_LEFT: [(f32, f32); 17] = [
    (0., 0.), (0.047, 0.013), (0.092, 0.049), (0.136, 0.104), (0.18, 0.175), (0.226, 0.259),
    (0.274, 0.352), (0.326, 0.45), (0.383, 0.55), (0.445, 0.648), (0.514, 0.741), (0.591, 0.825),
    (0.677, 0.896), (0.773, 0.951), (0.88, 0.987), (0.94, 1.), (1., 1.),
];

const MAX_SIDE: u32 = 512;

type Cache = HashMap<String, Option<Arc<RenderImage>>>;

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(HashMap::new());
}

pub fn alpha_at(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    for w in TO_LEFT.windows(2) {
        let ((x0, a0), (x1, a1)) = (w[0], w[1]);
        if x <= x1 {
            let t = if x1 > x0 { (x - x0) / (x1 - x0) } else { 1. };
            return a0 + (a1 - a0) * t;
        }
    }
    1.
}

pub fn source(url: SharedString) -> ImageSource {
    ImageSource::from(move |_: &mut Window, cx: &mut App| -> Option<Result<Arc<RenderImage>, ImageCacheError>> { get(&url, cx).map(Ok) })
}

fn get(url: &str, cx: &mut App) -> Option<Arc<RenderImage>> {
    if let Some(hit) = CACHE.with(|c| c.borrow().get(url).cloned()) {
        return hit;
    }
    CACHE.with(|c| c.borrow_mut().insert(url.to_owned(), None));
    let http = cx.http_client();
    let key = url.to_owned();
    cx.spawn(async move |cx| {
        let Ok(mut resp) = http.get(&key, Default::default(), true).await else { return };
        let mut bytes = Vec::new();
        if resp.body_mut().read_to_end(&mut bytes).await.is_err() {
            return;
        }
        let baked = cx.background_executor().spawn(async move { bake(&bytes) }).await;
        let Some(img) = baked else { return };
        cx.update(|cx| {
            CACHE.with(|c| c.borrow_mut().insert(key, Some(Arc::new(img))));
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

fn bake(bytes: &[u8]) -> Option<RenderImage> {
    let mut src = image::load_from_memory(bytes).ok()?.into_rgba8();
    if src.width() == 0 || src.height() == 0 {
        return None;
    }
    if src.width().max(src.height()) > MAX_SIDE {
        let k = MAX_SIDE as f32 / src.width().max(src.height()) as f32;
        let (w, h) = (((src.width() as f32 * k).round() as u32).max(1), ((src.height() as f32 * k).round() as u32).max(1));
        src = image::imageops::resize(&src, w, h, image::imageops::FilterType::Triangle);
    }
    let w = src.width() as f32;
    let ramp: Vec<f32> = (0..src.width()).map(|x| alpha_at((x as f32 + 0.5) / w)).collect();
    for (x, _, p) in src.enumerate_pixels_mut() {
        let [r, g, b, a] = p.0;
        *p = image::Rgba([b, g, r, (a as f32 * ramp[x as usize]).round() as u8]);
    }
    Some(RenderImage::new(vec![image::Frame::new(src)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_follows_the_css_mask() {
        assert_eq!(alpha_at(0.), 0.);
        assert_eq!(alpha_at(1.), 1.);
        assert!((alpha_at(0.383) - 0.55).abs() < 1e-6);
        assert!((alpha_at(0.4) - (0.55 + (0.648 - 0.55) * (0.017 / 0.062))).abs() < 1e-4);
        assert_eq!(alpha_at(0.97), 1.);
    }

    #[test]
    fn bake_fades_the_left_edge_and_swaps_to_bgra() {
        let mut png = Vec::new();
        let px = image::RgbaImage::from_pixel(64, 8, image::Rgba([200, 100, 50, 255]));
        image::DynamicImage::ImageRgba8(px).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let img = bake(&png).unwrap();
        let buf = img.as_bytes(0).unwrap();
        assert!(buf[3] < 8);
        let last = (63 * 4) as usize;
        assert_eq!(&buf[last..last + 4], &[50, 100, 200, 255]);
    }
}
