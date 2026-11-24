//! Owned collectibles for the profile editor pickers (avatar decoration,
//! profile effect, nameplate, profile frame), from the endpoint the client
//! calls: `GET /users/@me/collectibles-purchases?variants_return_style=2`.
//! Each purchase has `items[]` typed 0 avatar decoration (`asset` →
//! `cdn/avatar-decoration-presets/{asset}.png`), 1 profile effect
//! (`thumbnailPreviewSrc` / `staticFrameSrc`), 2 nameplate (`asset` path →
//! `cdn/assets/collectibles/{asset}static.png`), 3 profile frame.

use gpui::SharedString;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Decoration,
    Effect,
    Nameplate,
    Frame,
}

impl Kind {
    fn from_type(t: u64) -> Option<Self> {
        Some(match t {
            0 => Self::Decoration,
            1 => Self::Effect,
            2 => Self::Nameplate,
            3 => Self::Frame,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collectible {
    pub sku: u64,
    pub kind: Kind,
    pub name: SharedString,
    pub preview: Option<SharedString>,
    /// Profile effects: the full-card still (`reducedMotionSrc`) drawn over
    /// the picker's preview card.
    pub full: Option<SharedString>,
    /// Profile effects: the animated layers (`effects[]`).
    pub effects: Vec<EffectLayer>,
    /// Profile frames: layers and geometry.
    pub frame: Option<FrameSpec>,
    /// Profile effects: `animationType == ANIMATION_TYPE_PERSISTENT` (1).
    pub persistent: bool,
    /// Profile effects: `staticFrameSrc`, shown in place of the first layer of
    /// a persistent effect while it is not ticking (before the intro, after
    /// the play-through).
    pub static_frame: Option<SharedString>,
}

/// One `effects[]` layer of a profile effect: an APNG drawn over the whole
/// card from `start` ms, for `duration` ms, then (when `looping`) again after
/// `loop_delay` ms. Higher `z` on top.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectLayer {
    pub src: SharedString,
    pub looping: bool,
    pub start: u64,
    pub duration: u64,
    pub loop_delay: u64,
    pub z: i64,
    /// `position {x, y}` in CSS px: the layer's `left` / `top`.
    pub x: f32,
    pub y: f32,
}

/// Where a frame layer sits on the card (`layers[].type` / `anchor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Top,
    Center,
    Bottom,
}

/// A profile frame: images laid around the card. `inner_width` image px map
/// to the card width; the art overflows by `overflow_*` image px.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSpec {
    /// (static image URL, anchor, drawn in front of the card content).
    pub layers: Vec<(SharedString, Anchor, bool)>,
    pub inner_width: f32,
    pub overflow_top: f32,
    pub overflow_bottom: f32,
    pub overflow_horizontal: f32,
    /// Per layer: a `staple` (natural-height art sticking out by `overflow_top`
    /// / `overflow_bottom`); otherwise a rail/border filling the card height.
    pub staple: Vec<bool>,
}

fn effects(kind: Kind, item: &Value) -> Vec<EffectLayer> {
    if kind != Kind::Effect {
        return Vec::new();
    }
    let n = |l: &Value, k: &str| l.get(k).and_then(Value::as_u64).unwrap_or(0);
    let pos = |l: &Value, k: &str| l.get("position").and_then(|p| p.get(k)).and_then(Value::as_f64).unwrap_or(0.) as f32;
    let mut v: Vec<EffectLayer> = item
        .get("effects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| {
            Some(EffectLayer {
                src: l.get("src")?.as_str()?.to_owned().into(),
                looping: l.get("loop").and_then(Value::as_bool).unwrap_or(false),
                start: n(l, "start"),
                duration: n(l, "duration"),
                loop_delay: n(l, "loopDelay"),
                z: l.get("zIndex").and_then(Value::as_i64).unwrap_or(0),
                x: pos(l, "x"),
                y: pos(l, "y"),
            })
        })
        .collect();
    v.sort_by_key(|l| l.z);
    v
}

fn frame(kind: Kind, item: &Value) -> Option<FrameSpec> {
    if kind != Kind::Frame {
        return None;
    }
    let sku = item.get("sku_id")?.as_str()?;
    // The API spells the geometry in snake_case or camelCase (live store:
    // innerWidth 1200, overflowTop 304, overflowHorizontal 56).
    let f = |k: &str| {
        let camel: String = k.split('_').enumerate().map(|(i, w)| if i == 0 { w.to_owned() } else { w[..1].to_uppercase() + &w[1..] }).collect();
        item.get(k).or_else(|| item.get(&camel)).and_then(Value::as_f64).unwrap_or(0.) as f32
    };
    let raw = item.get("layers")?.as_array()?;
    let staple = raw
        .iter()
        .filter(|l| l.get("id").and_then(Value::as_str).is_some())
        .map(|l| l.get("type").and_then(Value::as_str) == Some("staple"))
        .collect();
    let layers = raw
        .iter()
        .filter_map(|l| {
            let s = |k: &str| l.get(k).and_then(Value::as_str);
            let anchor = match s("anchor") {
                Some("top") => Anchor::Top,
                Some("bottom") => Anchor::Bottom,
                _ => Anchor::Center,
            };
            let url = format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku}/{}/static", s("id")?);
            Some((url.into(), anchor, s("order") != Some("back")))
        })
        .collect();
    Some(FrameSpec {
        layers,
        inner_width: f("inner_width").max(1.),
        overflow_top: f("overflow_top"),
        overflow_bottom: f("overflow_bottom"),
        overflow_horizontal: f("overflow_horizontal"),
        staple,
    })
}

fn item(kind: Kind, sku: u64, name: SharedString, it: &Value) -> Collectible {
    Collectible { sku, kind, name, preview: preview(kind, it), full: full(kind, it), effects: effects(kind, it), frame: frame(kind, it),
        persistent: kind == Kind::Effect && it.get("animationType").and_then(Value::as_u64) == Some(1),
        static_frame: (kind == Kind::Effect).then(|| it.get("staticFrameSrc").and_then(Value::as_str).map(|s| s.to_owned().into())).flatten() }
}

/// A `GET /collectibles-products/{sku}` reply: its first item.
pub fn parse_product(v: &Value) -> Option<Collectible> {
    let it = v.get("items")?.as_array()?.first()?;
    let kind = Kind::from_type(it.get("type")?.as_u64()?)?;
    let sku = it.get("sku_id")?.as_str()?.parse().ok()?;
    Some(item(kind, sku, v.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(), it))
}

/// Applied effect / frame SKUs of a profile (`user_profile.collectibles`,
/// type 1 effect, 3 frame).
pub fn applied_skus(c: &[Value]) -> (Option<u64>, Option<u64>) {
    let find = |t: u64| c.iter().find(|x| x.get("type").and_then(Value::as_u64) == Some(t)).and_then(|x| x.get("sku_id")?.as_str()?.parse().ok());
    (find(1), find(3))
}

/// Shop page card art: the preview of a product's first item (any kind).
pub fn product_preview(p: &Value) -> Option<SharedString> {
    let it = p.get("items")?.as_array()?.first()?;
    preview(Kind::from_type(it.get("type")?.as_u64()?)?, it)
}

fn full(kind: Kind, item: &Value) -> Option<SharedString> {
    (kind == Kind::Effect).then(|| item.get("reducedMotionSrc").and_then(Value::as_str).map(|s| s.to_owned().into())).flatten()
}

fn preview(kind: Kind, item: &Value) -> Option<SharedString> {
    let s = |k: &str| item.get(k).and_then(Value::as_str);
    let url = match kind {
        Kind::Decoration => format!("https://cdn.discordapp.com/avatar-decoration-presets/{}.png?size=96&passthrough=false", s("asset")?),
        // Tiles load the thumbnail at `?width=100&height=195` (client `Rc`).
        Kind::Effect => match s("thumbnailPreviewSrc") {
            Some(t) => format!("{t}?width=100&height=195"),
            None => s("staticFrameSrc")?.to_owned(),
        },
        Kind::Nameplate => format!("https://cdn.discordapp.com/assets/collectibles/{}static.png", s("asset")?),
        Kind::Frame => s("asset")
            .map(|a| format!("https://cdn.discordapp.com/avatar-decoration-presets/{a}.png?size=96&passthrough=false"))
            .or_else(|| s("staticFrameSrc").map(str::to_owned))
            .or_else(|| frame_layer(item))?,
    };
    Some(url.into())
}

/// Profile frames carry `layers[]`; the client loads each one from
/// `media/v1/collectibles-shop/{sku}/{layer_id}/static`
/// (`getCollectiblesItemAssetUrl`). The tile shows the first front staple.
fn frame_layer(item: &Value) -> Option<String> {
    let sku = item.get("sku_id")?.as_str()?;
    let layers = item.get("layers")?.as_array()?;
    let is = |l: &&Value, k: &str, v: &str| l.get(k).and_then(Value::as_str) == Some(v);
    let l = layers.iter().find(|l| is(l, "type", "staple") && is(l, "order", "front")).or(layers.first())?;
    Some(format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku}/{}/static", l.get("id")?.as_str()?))
}

/// Picker sections fed by the shop catalogue
/// (`GET /collectibles-categories/v2`, see `phoenix_api::rest::collectibles`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShopCatalog {
    /// "Nitro exclusive": products with `premium_type` 2.
    pub nitro: Vec<Collectible>,
    /// "Check out the Shop": every other single-item product.
    pub shop: Vec<Collectible>,
}

pub fn parse_shop(v: &Value) -> ShopCatalog {
    let mut out = ShopCatalog::default();
    let mut seen = std::collections::HashSet::new();
    for p in phoenix_api::rest::collectibles::products(v) {
        // Bundles (1000) and variant groups (2000) are not picker tiles.
        let Some(kind) = p.get("type").and_then(Value::as_u64).and_then(Kind::from_type) else { continue };
        let Some(it) = p.get("items").and_then(Value::as_array).and_then(|i| i.first()) else { continue };
        let Some(sku) = it.get("sku_id").and_then(Value::as_str).and_then(|s| s.parse().ok()) else { continue };
        if !seen.insert(sku) {
            continue;
        }
        let c = item(kind, sku, p.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(), it);
        if p.get("premium_type").and_then(Value::as_u64) == Some(phoenix_api::rest::collectibles::PREMIUM_TYPE_NITRO) {
            out.nitro.push(c);
        } else {
            out.shop.push(c);
        }
    }
    out
}

/// Moves the items whose SKU is in `rec` to the front, in `rec` order; the
/// rest keep their catalogue order.
pub fn promote(items: &mut Vec<Collectible>, rec: &[u64]) {
    let mut front = Vec::new();
    for sku in rec {
        if let Some(i) = items.iter().position(|c| c.sku == *sku) {
            front.push(items.remove(i));
        }
    }
    front.append(items);
    *items = front;
}

pub fn parse_purchases(v: &Value) -> Vec<Collectible> {
    v.as_array()
        .into_iter()
        .flatten()
        .flat_map(|p| {
            let name = p.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
            p.get("items").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter_map(move |it| {
                let kind = Kind::from_type(it.get("type")?.as_u64()?)?;
                Some(item(kind, it.get("sku_id")?.as_str()?.parse().ok()?, name.clone().into(), &it))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_kinds_and_previews() {
        let v = json!([
            {"name": "Deco", "items": [{"type": 0, "sku_id": "1", "asset": "a_x"}]},
            {"name": "Fx", "items": [{"type": 1, "sku_id": "2", "thumbnailPreviewSrc": "https://t"}]},
            {"name": "Badge", "items": []},
            {"name": "Plate", "items": [{"type": 2, "sku_id": "3", "asset": "nameplates/x/"}]}
        ]);
        let c = parse_purchases(&v);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].kind, Kind::Decoration);
        assert!(c[0].preview.as_deref().unwrap().contains("avatar-decoration-presets/a_x.png"));
        assert_eq!(c[1].preview.as_deref(), Some("https://t?width=100&height=195"));
        assert_eq!(c[2].preview.as_deref(), Some("https://cdn.discordapp.com/assets/collectibles/nameplates/x/static.png"));
    }

    #[test]
    fn splits_shop_sections() {
        let v = json!({"categories": [{"products": [
            {"type": 0, "premium_type": 2, "name": "N", "items": [{"type": 0, "sku_id": "1", "asset": "a"}]},
            {"type": 1, "premium_type": 0, "name": "E", "items": [{"type": 1, "sku_id": "2", "thumbnailPreviewSrc": "https://t"}]},
            {"type": 3, "premium_type": 0, "name": "F", "items": [{"type": 3, "sku_id": "3", "layers": [
                {"id": "9", "type": "border", "order": "front"}, {"id": "8", "type": "staple", "order": "front"}], "innerWidth": 1200, "overflowTop": 304}]},
            {"type": 1000, "premium_type": 0, "name": "B", "items": [{"type": 0, "sku_id": "4"}]},
            {"type": 1, "premium_type": 0, "name": "dup", "items": [{"type": 1, "sku_id": "2"}]}
        ]}]});
        let s = parse_shop(&v);
        assert_eq!(s.nitro.len(), 1);
        assert_eq!(s.nitro[0].kind, Kind::Decoration);
        assert_eq!(s.shop.len(), 2);
        let mut v = s.shop.clone();
        promote(&mut v, &[3, 99]);
        assert_eq!(v.iter().map(|c| c.sku).collect::<Vec<_>>(), vec![3, 2]);
        assert_eq!(s.shop[1].preview.as_deref(), Some("https://cdn.discordapp.com/media/v1/collectibles-shop/3/8/static"));
        let f = s.shop[1].frame.as_ref().unwrap();
        assert_eq!(f.layers.len(), 2);
        assert_eq!(f.layers[1].1, Anchor::Center);
        assert_eq!(f.staple, vec![false, true]);
        assert_eq!((f.inner_width, f.overflow_top), (1200., 304.));
    }

    #[test]
    fn parses_product_effects_and_applied() {
        let v = json!({"name": "Fx", "type": 1, "items": [{"type": 1, "sku_id": "7", "effects": [
            {"src": "https://b", "loop": true, "duration": 4000, "start": 7500, "loopDelay": 4500, "zIndex": 101, "position": {"x": 12, "y": -8}},
            {"src": "https://a", "loop": false, "duration": 5810, "start": 0, "zIndex": 100}], "animationType": 1, "staticFrameSrc": "https://s"}]});
        let c = parse_product(&v).unwrap();
        assert_eq!(c.effects.len(), 2);
        assert_eq!(c.effects[0].src.as_ref(), "https://a");
        assert!(c.effects[1].looping && c.effects[1].loop_delay == 4500);
        assert_eq!((c.effects[1].x, c.effects[1].y), (12., -8.));
        assert!(c.persistent && c.static_frame.as_deref() == Some("https://s"));
        let a = applied_skus(&[json!({"type": 3, "sku_id": "5"}), json!({"type": 1, "sku_id": "7"})]);
        assert_eq!(a, (Some(7), Some(5)));
    }
}
