use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use futures::AsyncReadExt;
use gpui::{div, img, prelude::*, AnyElement, App, Global, Hsla, ImageCacheError, RenderImage, Rgba, SharedString};

use crate::model::User;
use crate::theme::z;

pub fn palette_dark(name: &str) -> Option<u32> {
    Some(match name {
        "crimson" => 0x900007,
        "berry" => 0x893A99,
        "sky" => 0x0080B7,
        "teal" => 0x086460,
        "forest" => 0x2D5401,
        "bubble_gum" => 0xDC3E97,
        "violet" => 0x730BC8,
        "cobalt" => 0x0131C2,
        "clover" => 0x047B20,
        "lemon" => 0xF6CD12,
        "white" => 0xFFFFFF,
        "black" => 0x000000,
        _ => return None,
    })
}

fn palette_for_theme(dark: u32) -> u32 {
    if !crate::theme::is_light() {
        return dark;
    }
    match dark {
        0x900007 => 0xE7040F,
        0x893A99 => 0xB11FCF,
        0x0080B7 => 0x56CCFF,
        0x086460 => 0x7DEED7,
        0x2D5401 => 0x6AA624,
        0xDC3E97 => 0xF957B3,
        0x730BC8 => 0x972FED,
        0x0131C2 => 0x4278FF,
        0x047B20 => 0x63CD5A,
        0xF6CD12 => 0xFED400,
        other => other,
    }
}

fn rgba(c: u32, a: f32) -> Hsla {
    let mut r: Rgba = gpui::rgb(c);
    r.a = a;
    r.into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Member,
    Channel,
    Account,
}

pub fn layer(user: &User, h: f32, place: Place, hovered: bool, selected: bool) -> Option<AnyElement> {
    layer_with_fade(user, h, place, hovered, selected, Fade::Fixed(220.))
}

#[derive(Default)]
struct Widths(HashMap<(u8, u64), f32>);

impl Global for Widths {}

fn place_key(place: Place) -> u8 {
    match place {
        Place::Member => 0,
        Place::Channel => 1,
        Place::Account => 2,
    }
}

pub fn width_probe(place: Place, id: u64) -> impl IntoElement {
    gpui::canvas(
        move |b, window, cx| {
            let w = f32::from(b.size.width) / crate::theme::zoom();
            let old = cx.default_global::<Widths>().0.insert((place_key(place), id), w);
            if old.is_none_or(|o| (o - w).abs() > 0.25) {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[derive(Clone, Copy)]
pub enum Fade {
    Fixed(f32),
    Measured { place: Place, id: u64, offset: f32, fallback: f32 },
}

impl Fade {
    fn resolve(self, cx: &App) -> f32 {
        match self {
            Fade::Fixed(v) => v,
            Fade::Measured { place, id, offset, fallback } => cx
                .try_global::<Widths>()
                .and_then(|w| w.0.get(&(place_key(place), id)).copied())
                .map_or(fallback, |w| w + offset),
        }
    }
}

const MAX_CACHED: usize = 128;

const MAX_RATIO: f32 = 60.;

#[derive(Clone, Hash, PartialEq, Eq)]
struct MaskKey {
    url: SharedString,
    start_q: u16,
    end_q: u16,
}

enum State {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

#[derive(Default)]
struct Store {
    images: HashMap<MaskKey, (State, Instant)>,
}

impl Global for Store {}

fn mask_alpha(u: f32, start: f32, end: f32) -> f32 {
    if u <= start {
        0.3
    } else if u >= end || end <= start {
        1.0
    } else {
        0.3 + 0.7 * ((u - start) / (end - start))
    }
}

fn bake_masked(bytes: &[u8], start: f32, end: f32) -> Option<RenderImage> {
    let src = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let col_alpha: Vec<f32> = (0..w).map(|x| mask_alpha((x as f32 + 0.5) / w as f32, start, end)).collect();
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, p) in out.enumerate_pixels_mut() {
        let [r, g, b, a] = src.get_pixel(x, y).0;
        let out_a = ((a as f32) * col_alpha[x as usize]).round().clamp(0.0, 255.0) as u8;
        *p = image::Rgba([b, g, r, out_a]);
    }
    Some(RenderImage::new(vec![image::Frame::new(out)]))
}

fn load_masked(
    url: &SharedString,
    start_ratio: f32,
    end_ratio: f32,
    cx: &mut App,
) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
    let start_q = (start_ratio.clamp(0.0, MAX_RATIO) * 1000.0).round() as u16;
    let end_q = (end_ratio.clamp(0.0, MAX_RATIO) * 1000.0).round() as u16;
    let key = MaskKey { url: url.clone(), start_q, end_q };

    let store = cx.default_global::<Store>();
    let same_url = |store: &Store| {
        store
            .images
            .iter()
            .filter(|(k, _)| k.url == *url)
            .find_map(|(_, (st, _))| if let State::Ready(img) = st { Some(Ok(img.clone())) } else { None })
    };
    let cached = store.images.get_mut(&key).map(|(st, used)| {
        *used = Instant::now();
        match st {
            State::Loading => None,
            State::Ready(img) => Some(Ok(img.clone())),
            State::Failed => Some(Err(ImageCacheError::Other(Arc::new(anyhow::anyhow!("nameplate decode failed"))))),
        }
    });
    if let Some(res) = cached {
        return res.or_else(|| same_url(store));
    }

    let evicted = if store.images.len() >= MAX_CACHED
        && let Some(old) = store
            .images
            .iter()
            .filter(|(_, (st, _))| !matches!(st, State::Loading))
            .min_by_key(|(_, (_, t))| *t)
            .map(|(k, _)| k.clone())
    {
        match store.images.remove(&old) {
            Some((State::Ready(img), _)) => Some(img),
            _ => None,
        }
    } else {
        None
    };
    if let Some(img) = evicted {
        cx.drop_image(img, None);
    }
    let store = cx.default_global::<Store>();

    if url.starts_with('/') {
        let state = std::fs::read(url.as_ref())
            .ok()
            .and_then(|bytes| bake_masked(&bytes, start_q as f32 / 1000.0, end_q as f32 / 1000.0))
            .map(|img| State::Ready(Arc::new(img)))
            .unwrap_or(State::Failed);
        let res = match &state {
            State::Ready(img) => Some(Ok(img.clone())),
            _ => Some(Err(ImageCacheError::Other(Arc::new(anyhow::anyhow!("nameplate local decode failed"))))),
        };
        store.images.insert(key, (state, Instant::now()));
        return res;
    }

    let fallback = same_url(store);
    store.images.insert(key.clone(), (State::Loading, Instant::now()));
    let http = cx.http_client();
    let fetch = url.clone();
    let s = start_q as f32 / 1000.0;
    let e = end_q as f32 / 1000.0;
    cx.spawn(async move |cx| {
        let mut bytes = Vec::new();
        let ok = match http.get(&fetch, Default::default(), true).await {
            Ok(mut r) => r.status().is_success() && r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let img = if ok { cx.background_spawn(async move { bake_masked(&bytes, s, e) }).await } else { None };
        cx.update(|cx| {
            if let Some(entry) = cx.default_global::<Store>().images.get_mut(&key) {
                entry.0 = match img {
                    Some(i) => State::Ready(Arc::new(i)),
                    None => State::Failed,
                };
            }
            cx.refresh_windows();
        });
    })
    .detach();
    fallback
}

pub fn layer_with_fade(user: &User, h: f32, place: Place, hovered: bool, selected: bool, fade: Fade) -> Option<AnyElement> {
    let (url, pal) = user.nameplate.clone()?;
    let base_opacity = match (place, selected, hovered) {
        (Place::Account, _, _) => 0.4,
        (_, true, _) => 0.9,
        (Place::Channel, false, true) => 0.4,
        _ => 0.6,
    };

    let mut d = if place == Place::Account {
        div().absolute().top(z(-1.)).left_0().bottom(z(-1.)).right(z(-1.)).rounded(z(8.)).overflow_hidden()
    } else {
        div().absolute().top_0().left_0().size_full().rounded(z(8.)).overflow_hidden()
    };

    if let Some(dark_rgb) = pal {
        let rgb = palette_for_theme(dark_rgb);
        let seg = |l: f32, r: f32, a0: f32, a1: f32| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(gpui::relative(l))
                .w(gpui::relative(r - l))
                .bg(gpui::linear_gradient(
                    90.,
                    gpui::linear_color_stop(rgba(rgb, a0), 0.),
                    gpui::linear_color_stop(rgba(rgb, a1), 1.),
                ))
        };
        if place == Place::Account {
            let end_a = if crate::theme::is_light() { 0x4D as f32 / 255. } else { 0x66 as f32 / 255. };
            d = d.child(seg(0., 1., 0x1A as f32 / 255., end_a).rounded(z(8.)));
        } else {
            let end_a = if selected {
                0x80 as f32 / 255.
            } else if hovered && place == Place::Member {
                0x4D as f32 / 255.
            } else {
                0x33 as f32 / 255.
            };
            let mid_a = 0x14 as f32 / 255.;
            d = d
                .child(seg(0., 0.2, 0., mid_a).rounded_l(z(8.)))
                .child(seg(0.2, 0.5, mid_a, mid_a))
                .child(seg(0.5, 1., mid_a, end_a).rounded_r(z(8.)));
        }
    }

    let img_w = h * (16. / 3.);
    if place == Place::Account {
        let src = if url.starts_with('/') {
            gpui::ImageSource::from(std::path::PathBuf::from(url.as_ref()))
        } else {
            gpui::ImageSource::from(url)
        };
        d = d.child(
            img(src)
                .h_full()
                .w(z(img_w))
                .object_fit(gpui::ObjectFit::Fill)
                .opacity(base_opacity)
                .absolute()
                .top_0()
                .right_0()
                .rounded_r(z(8.)),
        );
    } else {
        d = d.child(
            img(move |_: &mut gpui::Window, cx: &mut App| {
                let f = fade.resolve(cx);
                load_masked(&url, (f / img_w).max(0.), ((f + 50.) / img_w).max(0.), cx)
            })
            .h_full()
            .w(z(img_w))
            .object_fit(gpui::ObjectFit::Fill)
            .opacity(base_opacity)
            .absolute()
            .top_0()
            .right_0()
            .rounded_r(z(8.)),
        );
    }

    Some(d.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_alpha_ramps_continuously() {
        assert!((mask_alpha(0.0, 0.5, 0.75) - 0.3).abs() < 1e-6);
        assert!((mask_alpha(0.5, 0.5, 0.75) - 0.3).abs() < 1e-6);
        assert!((mask_alpha(0.625, 0.5, 0.75) - 0.65).abs() < 1e-6);
        assert!((mask_alpha(0.75, 0.5, 0.75) - 1.0).abs() < 1e-6);
        assert!((mask_alpha(1.0, 0.5, 0.75) - 1.0).abs() < 1e-6);
        assert!((mask_alpha(0.999, 1.0, 1.0) - 0.3).abs() < 1e-6);
        assert!((mask_alpha(1.0, 220. / 224., 270. / 224.) - 0.356).abs() < 1e-3);
    }

    #[test]
    fn bake_masked_applies_alpha_and_bgra() {
        let mut src = image::RgbaImage::new(4, 1);
        for x in 0..4 {
            src.put_pixel(x, 0, image::Rgba([200, 100, 50, 255]));
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        src.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let rendered = bake_masked(buf.get_ref(), 0.25, 0.75).unwrap();
        let raw = rendered.as_bytes(0).unwrap();
        assert_eq!(&raw[0..4], &[50, 100, 200, 77]);
        assert_eq!(&raw[12..16], &[50, 100, 200, 255]);
    }
}
