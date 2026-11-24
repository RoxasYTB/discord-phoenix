use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    canvas, div, img, prelude::*, AnyElement, App, Context, Global, ImageSource, RenderImage, SharedString,
};

use crate::{app::PhoenixApp, ui::settings::collectibles::{Anchor, Collectible, FrameSpec}};

const MAX_W: u32 = 450;
const MAX_ANIMS: usize = 4;
const INTRO_MS: u64 = 500;

#[derive(Default)]
struct Store {
    products: HashMap<u64, Option<Collectible>>,
    anims: HashMap<SharedString, (Option<Arc<RenderImage>>, Instant)>,
    clocks: HashMap<u64, (Instant, Instant)>,
    sizes: HashMap<SharedString, Option<(f32, f32)>>,
}

impl Global for Store {}

pub fn product(app: &PhoenixApp, sku: u64, cx: &mut Context<PhoenixApp>) -> Option<Collectible> {
    let s = cx.default_global::<Store>();
    if let Some(p) = s.products.get(&sku) {
        return p.clone();
    }
    s.products.insert(sku, None);
    let fut = app.backend.collectible_product(sku);
    cx.spawn(async move |this, cx| {
        let p = fut.await.ok().flatten();
        let _ = this.update(cx, |_, cx| {
            cx.default_global::<Store>().products.insert(sku, p);
            cx.notify();
        });
    })
    .detach();
    None
}

fn decode(bytes: &[u8], still: bool) -> anyhow::Result<RenderImage> {
    use image::{AnimationDecoder, codecs::png::PngDecoder};
    let dec = PngDecoder::new(std::io::Cursor::new(bytes))?;
    let mut frames: Vec<image::Frame> = if dec.is_apng()? {
        dec.apng()?.into_frames().collect_frames()?
    } else {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.into_rgba8();
        vec![image::Frame::new(img)]
    };
    if still && frames.len() > 1 {
        let ink = |f: &image::Frame| f.buffer().pixels().map(|p| p.0[3] as u64).sum::<u64>();
        let best = (0..frames.len()).max_by_key(|&i| ink(&frames[i])).unwrap_or(0);
        frames = vec![frames.swap_remove(best)];
    }
    let frames = frames
        .into_iter()
        .map(|f| {
            let delay = f.delay();
            let mut buf = f.into_buffer();
            if buf.width() > MAX_W {
                let h = (buf.height() as u64 * MAX_W as u64 / buf.width() as u64) as u32;
                buf = image::imageops::resize(&buf, MAX_W, h.max(1), image::imageops::FilterType::Triangle);
            }
            for p in buf.chunks_exact_mut(4) {
                p.swap(0, 2);
            }
            image::Frame::from_parts(buf, 0, 0, delay)
        })
        .collect::<Vec<_>>();
    Ok(RenderImage::new(frames))
}

fn anim(src: &SharedString, still: bool, cx: &mut Context<PhoenixApp>) -> Option<Arc<RenderImage>> {
    let url = &SharedString::from(if still { format!("still:{src}") } else { src.to_string() });
    let s = cx.default_global::<Store>();
    if let Some((a, used)) = s.anims.get_mut(url) {
        *used = Instant::now();
        return a.clone();
    }
    if s.anims.len() >= MAX_ANIMS
        && let Some(old) = s.anims.iter().min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone())
    {
        s.anims.remove(&old);
    }
    s.anims.insert(url.clone(), (None, Instant::now()));
    let http = cx.http_client();
    let u = url.clone();
    let fetch_url = src.clone();
    cx.spawn(async move |this, cx| {
        use futures::AsyncReadExt;
        let mut bytes = Vec::new();
        let ok = match http.get(&fetch_url, Default::default(), true).await {
            Ok(mut r) => r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let img = if ok { cx.background_spawn(async move { decode(&bytes, still).ok() }).await.map(Arc::new) } else { None };
        let _ = this.update(cx, |_, cx| {
            if let Some(e) = cx.default_global::<Store>().anims.get_mut(&u) {
                e.0 = img;
            }
            cx.notify();
        });
    })
    .detach();
    None
}

fn art_size(url: &SharedString, cx: &mut Context<PhoenixApp>) -> Option<(f32, f32)> {
    let s = cx.default_global::<Store>();
    if let Some(v) = s.sizes.get(url) {
        return *v;
    }
    s.sizes.insert(url.clone(), None);
    let http = cx.http_client();
    let (u, fetch) = (url.clone(), url.to_string());
    cx.spawn(async move |this, cx| {
        use futures::AsyncReadExt;
        let mut bytes = Vec::new();
        let ok = match http.get(&fetch, Default::default(), true).await {
            Ok(mut r) => r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let size = ok
            .then(|| image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format().ok()?.into_dimensions().ok())
            .flatten()
            .map(|(w, h)| (w as f32, h as f32));
        let _ = this.update(cx, |_, cx| {
            cx.default_global::<Store>().sizes.insert(u, size);
            cx.notify();
        });
    })
    .detach();
    None
}

fn clock(sku: u64, cx: &mut App) -> Duration {
    let now = Instant::now();
    let s = cx.default_global::<Store>();
    let e = s.clocks.entry(sku).or_insert((now, now));
    if now.duration_since(e.1) > Duration::from_secs(1) {
        e.0 = now;
    }
    e.1 = now;
    now.duration_since(e.0)
}

pub static DISABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn effect(c: &Collectible, w: f32, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if DISABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    effect_adjusted(c, w, 0., cx)
}

pub fn effect_adjusted(c: &Collectible, w: f32, banner_adj: f32, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let sku = c.sku;
    let z = crate::theme::z;
    let layer = |a: Arc<RenderImage>, id: Option<SharedString>, left: f32, top: f32| {
        let sz = a.size(0);
        let h = if sz.width.0 > 0 { w * sz.height.0 as f32 / sz.width.0 as f32 } else { w };
        let el = img(ImageSource::Render(a)).absolute().top(z(top)).left(z(left)).w(z(w)).h(z(h));
        match id {
            Some(id) => el.id(id).into_any_element(),
            None => el.into_any_element(),
        }
    };
    let mut layers: Vec<AnyElement> = Vec::new();
    if c.effects.is_empty() || cx.reduce_motion() {
        let rest = c.persistent.then(|| c.static_frame.clone().zip(c.effects.first())).flatten();
        if let Some((src, l0)) = rest {
            if let Some(a) = anim(&src, true, cx) {
                layers.push(layer(a, None, l0.x, if l0.y != 0. { l0.y } else { -banner_adj }));
            }
        } else if let Some(src) = c.full.clone().or(c.preview.clone())
            && let Some(a) = anim(&src, true, cx)
        {
            layers.push(layer(a, None, 0., -banner_adj));
        }
    } else {
        let el = clock(sku, cx).as_millis() as u64;
        let t = el.saturating_sub(INTRO_MS);
        let loop_end = c.effects.iter().map(|l| l.start + l.duration).max().unwrap_or(0);
        let stopped = !c.effects.iter().any(|l| l.looping) && t >= loop_end;
        for (i, l) in c.effects.iter().enumerate() {
            if i == 0
                && c.persistent
                && (el < INTRO_MS || stopped)
                && let Some(src) = &c.static_frame
            {
                if let Some(a) = anim(src, true, cx) {
                    layers.push(layer(a, None, l.x, if l.y != 0. { l.y } else { -banner_adj }));
                }
                continue;
            }
            let Some(a) = anim(&l.src, false, cx) else { continue };
            if el < INTRO_MS || t < l.start || l.duration == 0 {
                continue;
            }
            let since = t - l.start;
            let (cycle, active) = if l.looping {
                let period = l.duration + l.loop_delay;
                (since / period, since % period < l.duration)
            } else {
                (0, since < l.duration)
            };
            if !active {
                continue;
            }
            layers.push(layer(a, Some(SharedString::from(format!("pfx-{sku}-{i}-{cycle}"))), l.x, l.y - banner_adj));
        }
    }
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden()
            .children(layers)
            .child(canvas(|_, _, _| {}, |_, _, window, _| window.request_animation_frame()).absolute().size_0())
            .into_any_element(),
    )
}

pub fn frame(f: &FrameSpec, w: f32, front: bool, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let k = w / f.inner_width;
    let z = crate::theme::z;
    let layers: Vec<_> = f.layers.iter().enumerate().filter(|(_, l)| l.2 == front).collect();
    if layers.is_empty() {
        return None;
    }
    let box_w = w + 2. * f.overflow_horizontal * k;
    let mut d = div().absolute().top_0().bottom_0().left(z(-f.overflow_horizontal * k)).w(z(box_w));
    for (i, (url, anchor, _)) in layers {
        let Some((iw, ih)) = art_size(url, cx) else { continue };
        let art = img(url.clone()).w_full().h(z(box_w * ih / iw.max(1.))).flex_none();
        if f.staple.get(i).copied().unwrap_or(false) {
            let l = div().absolute().left_0().right_0();
            let l = match anchor {
                Anchor::Bottom => l.bottom(z(-f.overflow_bottom * k)),
                _ => l.top(z(-f.overflow_top * k)),
            };
            d = d.child(l.child(art));
        } else {
            let col = div().absolute().top_0().left_0().size_full().overflow_hidden().flex().flex_col();
            let col = match anchor {
                Anchor::Top => col.justify_start(),
                Anchor::Center => col.justify_center(),
                Anchor::Bottom => col.justify_end(),
            };
            d = d.child(col.child(art));
        }
    }
    Some(d.into_any_element())
}

#[cfg(test)]
mod tests {
    fn apng() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, 4, 4);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.set_animated(3, 0).unwrap();
            let mut w = e.write_header().unwrap();
            w.set_dispose_op(png::DisposeOp::None).unwrap();
            w.set_blend_op(png::BlendOp::Over).unwrap();
            w.write_image_data(&[0; 64]).unwrap();
            w.set_frame_dimension(2, 2).unwrap();
            w.write_image_data(&[255, 0, 0, 255].repeat(4)).unwrap();
            w.set_frame_dimension(1, 1).unwrap();
            w.set_frame_position(3, 3).unwrap();
            w.write_image_data(&[0, 0, 255, 255]).unwrap();
            w.finish().unwrap();
        }
        out
    }

    #[test]
    fn decodes_composed_frames_and_skips_empty_intro_for_still() {
        let b = apng();
        let anim = super::decode(&b, false).unwrap();
        assert_eq!(anim.frame_count(), 3);
        let f = anim.as_bytes(2).unwrap();
        assert_eq!(&f[0..4], &[0, 0, 255, 255]);
        assert_eq!(&f[60..64], &[255, 0, 0, 255]);
        let still = super::decode(&b, true).unwrap();
        assert_eq!(still.frame_count(), 1);
        assert_eq!(still.as_bytes(0).unwrap(), f);
    }
}
