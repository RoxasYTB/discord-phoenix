//! Avatar decoration overlay for the generic avatar component.
//! `avatarDecorationContainer_XpIUHy`: absolute, inset-inline-start/top
//! --custom-avatar-avatar-decoration-border-position
//! (`(1 - --decoration-to-avatar-ratio) / 2 * 100%`), size avatar ×
//! --decoration-to-avatar-ratio (1.2), pointer-events none, drawn above the
//! avatar but masked by the status cut-out (the status stays on top).

use gpui::{img, px, SharedString, Styled};

use crate::model::User;

/// --decoration-to-avatar-ratio
pub const RATIO: f32 = 1.2;

/// Decoration box for an avatar of `size`: (offset, side).
pub fn geometry(size: f32) -> (f32, f32) {
    let side = size * RATIO;
    ((size - side) / 2., side)
}

/// The decoration image `src` for an avatar of `size`, positioned in the
/// avatar's own (relative) box. Avatars with a status take it through
/// [`crate::ui::avatar_with_status_deco`] so the status cut-out and glyph
/// are painted above it.
pub fn layer(src: SharedString, size: f32) -> gpui::Img {
    let (off, side) = geometry(size);
    img(src).absolute().left(px(off)).top(px(off)).size(px(side))
}

/// The user's decoration source, animated while row `key` is hovered.
pub fn source(user: Option<&User>, key: Option<u64>) -> Option<SharedString> {
    let deco = user?.decoration.clone()?;
    Some(key.map(|k| src_for(&deco, k)).unwrap_or(deco))
}

#[cfg(test)]
mod tests {
    #[test]
    fn border_position_is_minus_ten_percent() {
        let (off, side) = super::geometry(32.);
        assert!((off + 3.2).abs() < 1e-4 && (side - 38.4).abs() < 1e-4);
    }

    /// Live `svg-mask-avatar-decoration-status-round-<n>` holes (cx = cy, r, in
    /// fractions of the decoration box) land on the avatar's own status cut,
    /// which is what lets the cut be painted over the decoration.
    #[test]
    fn decoration_hole_is_the_avatar_cut() {
        for (size, c, r) in [(32., 0.786_458_3, 0.208_333_3), (80., 0.791_666_7, 0.145_833_3), (96., 0.777_777_8, 0.156_25), (120., 0.777_777_8, 0.138_888_9)] {
            let (off, side) = super::geometry(size);
            let (cx, cr, _) = crate::ui::status_svg::geometry(size);
            assert!((off + c * side - cx).abs() < 0.05, "{size}: centre {} vs {cx}", off + c * side);
            assert!((r * side - cr).abs() < 0.05, "{size}: radius {} vs {cr}", r * side);
        }
    }
}

thread_local! {
    /// Row whose decoration currently plays (`hover_key`), like the client's
    /// static → animated swap on row hover.
    static HOVER: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Stable key of a hoverable row (`kind` separates member rows from messages).
pub fn hover_key(kind: u8, id: u64) -> u64 {
    ((kind as u64) << 56) ^ id
}

/// Records the hover state of row `key`; true when it changed (re-render).
pub fn set_hover(key: u64, hovered: bool) -> bool {
    HOVER.with(|h| {
        let next = if hovered { Some(key) } else if h.get() == Some(key) { None } else { h.get() };
        h.replace(next) != next
    })
}

/// Static decoration URL from `avatar_decoration_data` (`{asset, sku_id}`):
/// the 2026 client loads `media/v1/collectibles-shop/{sku_id}/static`
/// (`getCollectiblesItemAssetUrl`); the asset rides along as a query
/// parameter (ignored by the CDN) for [`animated`]. Without a SKU, the preset.
pub fn url(d: &serde_json::Value) -> Option<SharedString> {
    let asset = d.get("asset")?.as_str().filter(|a| !a.is_empty())?;
    Some(match d.get("sku_id").and_then(|s| s.as_str()).filter(|s| !s.is_empty()) {
        Some(sku) => format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku}/static?asset={asset}").into(),
        None => format!("https://cdn.discordapp.com/avatar-decoration-presets/{asset}.png?size=96&passthrough=false").into(),
    })
}

/// The animated variant of a static decoration URL. The live client swaps
/// `…/static` for `…/animated` (an APNG, which GPUI cannot play); the preset
/// CDN serves the same artwork as an animated WebP, which it decodes.
pub fn animated(url: &str) -> Option<SharedString> {
    let asset = match url.split_once("?asset=") {
        Some((_, a)) => a,
        None => url.split_once(".png?")?.0.rsplit('/').next()?,
    };
    asset.starts_with("a_").then(|| format!("https://cdn.discordapp.com/avatar-decoration-presets/{asset}.webp?size=96&animated=true").into())
}

/// Decoration source: animated when `play`, else static.
pub fn src(deco: &SharedString, play: bool) -> SharedString {
    play.then(|| animated(deco)).flatten().unwrap_or_else(|| deco.clone())
}

/// [`src`] for row `key`: animated while it is hovered.
pub fn src_for(deco: &SharedString, key: u64) -> SharedString {
    src(deco, HOVER.with(|h| h.get()) == Some(key))
}

#[cfg(test)]
mod anim_tests {
    #[test]
    fn animated_url_only_for_animated_assets() {
        let s = "https://cdn.discordapp.com/avatar-decoration-presets/a_12ab.png?size=96&passthrough=false";
        assert_eq!(super::animated(s).as_deref(), Some("https://cdn.discordapp.com/avatar-decoration-presets/a_12ab.webp?size=96&animated=true"));
        assert!(super::animated("https://cdn.discordapp.com/avatar-decoration-presets/12ab.png?size=96&passthrough=false").is_none());
        let u = super::url(&serde_json::json!({"asset": "a_12ab", "sku_id": "77"})).unwrap();
        assert_eq!(u.as_ref(), "https://cdn.discordapp.com/media/v1/collectibles-shop/77/static?asset=a_12ab");
        assert_eq!(super::animated(&u).as_deref(), Some("https://cdn.discordapp.com/avatar-decoration-presets/a_12ab.webp?size=96&animated=true"));
        assert!(super::set_hover(super::hover_key(1, 5), true));
        assert!(!super::set_hover(super::hover_key(1, 5), true));
        assert!(super::set_hover(super::hover_key(1, 5), false));
    }
}
