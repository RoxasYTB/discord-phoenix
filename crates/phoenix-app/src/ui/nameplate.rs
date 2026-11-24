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

use gpui::{div, img, prelude::*, AnyElement, Hsla, Rgba};

use crate::model::User;
use crate::theme::{Palette, c, generated::idx, tok, z};

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
    layer_with_fade(user, h, place, hovered, selected, 220.)
}

/// [`layer`] with an explicit horizontal mask fade start (`fade_start_px` from
/// the left edge of `img_AFi54G`).
///
/// Live client (2026-11-24, `img_AFi54G` + module `351952`):
/// `mask-image: linear-gradient(to right, rgba(0, 0, 0, 0.3) <fade>px, rgb(0, 0, 0) <fade+50>px)`.
///
/// GPUI has no CSS `mask-image`; instead of slicing the `<img>` into multiple
/// clipped divs (which produces visible subpixel vertical seams at fractional
/// DPR such as 1.25/1.5), we render a single `<img>` at `base_opacity` and
/// attenuate its left region with a smooth surface-colored overlay (alpha 0.7 -> 0.0)
/// snapped to 4px boundaries (exact physical pixels at 1.0, 1.25, 1.5, and 2.0 DPR),
/// then draw the palette gradient on top so the tint is never dimmed.
pub fn layer_with_fade(
    user: &User,
    h: f32,
    place: Place,
    hovered: bool,
    selected: bool,
    fade_start_px: f32,
) -> Option<AnyElement> {
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

    let img_w = h * (16. / 3.);
    let make_img = |op: f32| {
        // Absolute paths load from disk (offscreen mock renders); URLs as usual.
        img(if url.starts_with('/') {
            gpui::ImageSource::from(std::path::PathBuf::from(url.as_ref()))
        } else {
            gpui::ImageSource::from(url.clone())
        })
        .h_full()
        .w(z(img_w))
        .object_fit(gpui::ObjectFit::Cover)
        .opacity(op)
    };

    // 1. Single seam-free image layer.
    if place == Place::Account {
        d = d.child(make_img(base_opacity).absolute().top_0().right_0().rounded_r(z(8.)));
    } else if fade_start_px >= img_w {
        d = d.child(make_img(base_opacity * 0.3).absolute().top_0().right_0().rounded_r(z(8.)));
    } else {
        // Snap to 4px logical grid -> exact integer physical pixels at 1.0, 1.25, 1.5, 2.0 DPR.
        let fade_start = ((fade_start_px / 4.).round() * 4.).clamp(0., img_w);
        let ramp_w = 48.0_f32.min((img_w - fade_start).max(0.));
        let mut surface = match place {
            Place::Member => c(Palette::BG_SECONDARY),
            Place::Channel | Place::Account => c(tok(idx::BACKGROUND_BASE_LOW)),
        };
        let mut art = div()
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .w(z(img_w))
            .rounded_r(z(8.))
            .overflow_hidden()
            .child(make_img(base_opacity).absolute().top_0().left_0().rounded_r(z(8.)));
        if fade_start > 0. {
            surface.a = 0.7;
            art = art.child(div().absolute().top_0().bottom_0().left_0().w(z(fade_start)).bg(surface));
        }
        if ramp_w > 0. {
            let mut s0 = surface;
            s0.a = 0.7;
            let mut s1 = surface;
            s1.a = 0.0;
            art = art.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(z(fade_start))
                    .w(z(ramp_w))
                    .bg(gpui::linear_gradient(
                        90.,
                        gpui::linear_color_stop(s0, 0.),
                        gpui::linear_color_stop(s1, 1.),
                    )),
            );
        }
        d = d.child(art);
    }

    // 2. Palette background gradient (module `351952`).
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

    Some(d.into_any_element())
}
