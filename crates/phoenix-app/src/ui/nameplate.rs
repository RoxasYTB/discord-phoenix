//! User nameplates (collectibles) behind DM list / member list / user panel rows.
//!
//! Structure from live Discord 1.0.171 (modules `214881`, `351952`, `431939`, `641886`):
//! - `container_AFi54G`: absolute, fills the row, `border-radius: inherit` (8px),
//!   `overflow: hidden`, background gradient:
//!   - `Member` / `Channel`: `linear-gradient(90deg, transparent 0%, P/.08 20%, P/.08 50%, P/<end> 100%)`
//!     where `<end>` is `.50` (`0x80`) when selected, `.30` (`0x4D`) on member hover, `.20` (`0x33`) idle.
//!   - `Account` (user panel): `linear-gradient(90deg, P/.10 0%, P/<.30 light | .40 dark> 100%)`,
//!     with `fitInAccount_AFi54G` (`top: -1px; height: calc(100% + 2px); width: calc(100% + 1px)`).
//! - `img_AFi54G`: `height: 100%`, `inset-inline-end: 0`, `object-position: right center`,
//!   opacity `.6` (`.selected` `.9`, `.channel.hover` `.4`, `.account` `.4`).
//!   It is a child of `container_AFi54G`, so the palette gradient (the container's
//!   background) is painted UNDER the image, never over it.
//! - Live member list (2026-11-24, 1536x840 @125 %): the row `childContainer_J1Adu_
//!   nameplated_J1Adu_` is 252px wide (`member_Iy0c7A` margin-left 8, scroller
//!   `content_tqapBM` 260 of 268: `scrollbarGutterStable thin` keeps an 8px gutter),
//!   padding `0 32px 0 10px`; `img_AFi54G` 223.925x41.99 (`left: 28.075px; right: 0`,
//!   natural 448x84, `object-fit: fill`) ends on the row's right edge; one-line rows
//!   are 40px tall with a 213x40 image, DM rows 48px with 256x48.
//! - Mask (module `351952`): a `ResizeObserver` on the row's `layout_` element sets
//!   `fade = contentRect.width + 10` (member list) or `- 5` (DM channel), then
//!   `mask-image: linear-gradient(to right, rgba(0,0,0,.3) <fade>px, #000 <fade+50>px)`
//!   in the image's own box. The ramp is NOT clamped to the image: with `fade` 220 on a
//!   224px image the right edge only reaches alpha .356.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use futures::AsyncReadExt;
use gpui::{div, img, prelude::*, AnyElement, App, Global, Hsla, ImageCacheError, RenderImage, Rgba, SharedString};

use crate::model::User;
use crate::theme::z;

/// Nameplate palette name -> `darkBackground` (client module `641886`).
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

/// Map a dark palette RGB from [`palette_dark`] to its `lightBackground`
/// counterpart (client module `641886`).
fn palette_for_theme(dark: u32) -> u32 {
    if !crate::theme::is_light() {
        return dark;
    }
    match dark {
        0x900007 => 0xE7040F, // crimson
        0x893A99 => 0xB11FCF, // berry
        0x0080B7 => 0x56CCFF, // sky
        0x086460 => 0x7DEED7, // teal
        0x2D5401 => 0x6AA624, // forest
        0xDC3E97 => 0xF957B3, // bubble_gum
        0x730BC8 => 0x972FED, // violet
        0x0131C2 => 0x4278FF, // cobalt
        0x047B20 => 0x63CD5A, // clover
        0xF6CD12 => 0xFED400, // lemon
        other => other,       // white, black, or custom
    }
}

fn rgba(c: u32, a: f32) -> Hsla {
    let mut r: Rgba = gpui::rgb(c);
    r.a = a;
    r.into()
}

/// Where the nameplate is drawn: opacity and gradient stops differ.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Member list (`.img` .6, `.selected` .9).
    Member,
    /// DM list (`.channel`: .6, hover .4, selected .9).
    Channel,
    /// Bottom-left user panel (`.account`: .4, `fitInAccount` bleed).
    Account,
}

/// Absolute layer to put first inside a `relative` row with rounded corners.
/// `h` is the row height in px (image width follows the asset's 16:3 ratio,
/// 448x84 natural / 224x42 at h=42).
pub fn layer(user: &User, h: f32, place: Place, hovered: bool, selected: bool) -> Option<AnyElement> {
    layer_with_fade(user, h, place, hovered, selected, Fade::Fixed(220.))
}

/// Measured `layout_` widths (CSS px) per (place, row id): the client's `ResizeObserver`.
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

/// Absolute, full-size probe to put inside the row's `layout_` equivalent: it records
/// that element's laid-out width, which [`Fade::Measured`] reads on the next frame
/// (the client applies its `ResizeObserver` result one frame later as well).
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

/// Where the mask fade starts, in px from the image's left edge.
#[derive(Clone, Copy)]
pub enum Fade {
    /// A fixed value.
    Fixed(f32),
    /// `width_probe(place, id)` width + `offset` (`+10` member list, `-5` DM),
    /// `fallback` until the probe has run.
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

/// Maximum number of masked nameplate `RenderImage`s kept in memory.
const MAX_CACHED: usize = 128;

/// Upper bound for the quantized mask ratios (u16 per mille).
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

/// Continuous horizontal alpha mask matching CSS:
/// `mask-image: linear-gradient(to right, rgba(0, 0, 0, 0.3) <start>, rgb(0, 0, 0) <end>)`.
fn mask_alpha(u: f32, start: f32, end: f32) -> f32 {
    if u <= start {
        0.3
    } else if u >= end || end <= start {
        1.0
    } else {
        0.3 + 0.7 * ((u - start) / (end - start))
    }
}

/// Decode `bytes` and apply the horizontal alpha mask pixel by pixel into a BGRA
/// `RenderImage`, eliminating all div-slicing seams and banding at any DPR.
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
        // GPUI textures are BGRA.
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
    // Ratios may exceed 1: the CSS ramp runs past the image's right edge.
    let start_q = (start_ratio.clamp(0.0, MAX_RATIO) * 1000.0).round() as u16;
    let end_q = (end_ratio.clamp(0.0, MAX_RATIO) * 1000.0).round() as u16;
    let key = MaskKey { url: url.clone(), start_q, end_q };

    let store = cx.default_global::<Store>();
    // While a new mask bakes (the measured width changed), keep showing a mask of the
    // same image instead of nothing.
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

    if store.images.len() >= MAX_CACHED
        && let Some(old) = store
            .images
            .iter()
            .filter(|(_, (st, _))| !matches!(st, State::Loading))
            .min_by_key(|(_, (_, t))| *t)
            .map(|(k, _)| k.clone())
    {
        store.images.remove(&old);
    }

    // Local disk paths (offscreen mock renders) decode synchronously.
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

/// [`layer`] with an explicit horizontal mask fade start (`fade`, px from the left
/// edge of `img_AFi54G`).
///
/// Live client (2026-11-24, `img_AFi54G` + module `351952`):
/// `mask-image: linear-gradient(to right, rgba(0, 0, 0, 0.3) <fade>px, rgb(0, 0, 0) <fade+50>px)`.
///
/// The alpha mask is baked pixel-by-pixel into a cached `RenderImage` keyed by
/// `(url, fade_start_ratio, fade_end_ratio)` so there are zero seams, bands, or
/// tearing at any DPR.
pub fn layer_with_fade(user: &User, h: f32, place: Place, hovered: bool, selected: bool, fade: Fade) -> Option<AnyElement> {
    let (url, pal) = user.nameplate.clone()?;
    let base_opacity = match (place, selected, hovered) {
        (Place::Account, _, _) => 0.4,
        (_, true, _) => 0.9,
        (Place::Channel, false, true) => 0.4,
        _ => 0.6,
    };

    let mut d = if place == Place::Account {
        // `.fitInAccount_AFi54G`: top -1px, height calc(100% + 2px), width calc(100% + 1px)
        div().absolute().top(z(-1.)).left_0().bottom(z(-1.)).right(z(-1.)).rounded(z(8.)).overflow_hidden()
    } else {
        div().absolute().top_0().left_0().size_full().rounded(z(8.)).overflow_hidden()
    };

    // 1. Palette gradient: `container_AFi54G`'s own background (module `351952`), so
    //    it sits under the image. Painting it over the image washed the art out
    //    towards the right edge (live 1536x840: client (36,10,38) vs (29,12,44)).
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
            // GPUI clips children to a plain rectangle, so the pieces touching the
            // rounded corners carry the 8px radius themselves.
            d = d
                .child(seg(0., 0.2, 0., mid_a).rounded_l(z(8.)))
                .child(seg(0.2, 0.5, mid_a, mid_a))
                .child(seg(0.5, 1., mid_a, end_a).rounded_r(z(8.)));
        }
    }

    // 2. `img_AFi54G`: h x h*16/3 (448x84 natural, `object-fit: fill`), right-aligned,
    //    with the continuous per-pixel alpha mask baked in.
    let img_w = h * (16. / 3.);
    if place == Place::Account {
        // `.account`: no mask (the hook has no `content` ref there).
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
                // Not clamped to 1: past the right edge the ramp simply never ends.
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
        // Full-width fade (fade_start_px >= img_w) stays at 0.3 everywhere.
        assert!((mask_alpha(0.999, 1.0, 1.0) - 0.3).abs() < 1e-6);
        // Live member row: fade 220px on a 224px image, ramp to 270px. The right
        // edge only reaches .3 + .7 * 4/50 = .356, never 1.
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
        // Pixel 0 (u=0.125 <= 0.25): alpha = round(255 * 0.3) = 77, BGRA = [50, 100, 200, 77]
        assert_eq!(&raw[0..4], &[50, 100, 200, 77]);
        // Pixel 3 (u=0.875 >= 0.75): alpha = 255, BGRA = [50, 100, 200, 255]
        assert_eq!(&raw[12..16], &[50, 100, 200, 255]);
    }
}
