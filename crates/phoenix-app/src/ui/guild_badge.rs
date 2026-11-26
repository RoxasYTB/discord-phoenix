use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

use crate::theme::{c, generated::idx, tok, z};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Partnered,
    Verified,
    VerifiedAndPartnered,
    Community,
    Discoverable,
    Staff,
    None,
}

pub fn kind(features: &[String]) -> Kind {
    let has = |f: &str| features.iter().any(|x| x == f);
    if has("INTERNAL_EMPLOYEE_ONLY") {
        Kind::Staff
    } else if has("VERIFIED") && has("PARTNERED") {
        Kind::VerifiedAndPartnered
    } else if has("VERIFIED") {
        Kind::Verified
    } else if has("PARTNERED") {
        Kind::Partnered
    } else if has("COMMUNITY") && has("DISCOVERABLE") {
        Kind::Discoverable
    } else if has("COMMUNITY") {
        Kind::Community
    } else {
        Kind::None
    }
}

pub fn premium(boosts: u32, tier: u8) -> bool {
    boosts > 0 || tier > 0
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Flat(usize),
    Gradient(usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub glyph: &'static str,
    pub fg: usize,
    pub bg: Fill,
    pub glyph_size: f32,
}

pub fn look(kind: Kind, premium: bool, dark: bool, size: f32) -> Option<Look> {
    let base = (0.75 * size).floor();
    let (glyph, fg, bg, adjust) = match kind {
        Kind::Staff => ("guild/staff", idx::WHITE, idx::GREEN_360, 0.),
        Kind::Verified | Kind::VerifiedAndPartnered => ("check-small", idx::WHITE, idx::GREEN_360, 0.),
        Kind::Partnered => ("guild/partner", idx::WHITE, idx::BRAND_500, 0.),
        Kind::Community => {
            let (fg, bg) = if dark { (idx::PRIMARY_500, idx::WHITE) } else { (idx::WHITE, idx::PRIMARY_500) };
            ("guild/home", fg, bg, 2.)
        }
        _ => return None,
    };
    let (fg, bg) = if premium && kind == Kind::Community { (idx::WHITE, Fill::Gradient(idx::PREMIUM_TIER_2_PURPLE, idx::PREMIUM_TIER_2_PINK)) } else { (fg, Fill::Flat(bg)) };
    Some(Look { glyph, fg, bg, glyph_size: base - adjust })
}

const FLOWER_STAR: &str = "M5.52995 0.431867C6.27995 0.181867 7.2 1.23199 8 1.23199C8.8 1.23199 9.75007 0.231867 10.4701 0.431867C11.19 0.631965 11.38 2.07173 12 2.52171C12.62 2.9717 14.0003 2.70184 14.4603 3.32184C14.9199 3.94191 14.2303 5.16178 14.4603 5.91169C14.6903 6.66159 15.9998 7.21166 16 8.00153C16 8.79146 14.72 9.38146 14.4798 10.0914C14.2398 10.8014 14.9198 12.0919 14.4798 12.6819C14.0397 13.2716 12.6401 13.0321 12.0202 13.482C11.4002 13.932 11.2298 15.3219 10.4798 15.5719C9.72987 15.8216 8.80967 14.7717 8.00973 14.7717C7.2098 14.7719 6.25964 15.7718 5.53971 15.5719C4.81995 15.3716 4.61982 13.9321 4 13.482C3.38 13.032 1.99971 13.3019 1.53971 12.6819C1.07975 12.0619 1.76971 10.8414 1.53971 10.0914C1.30939 9.34159 0 8.79139 0 8.00153C0.000177681 7.21159 1.2802 6.62163 1.52018 5.91169C1.76012 5.20167 1.08021 3.91183 1.52018 3.32184C1.96018 2.73184 3.36999 3.0017 4 2.52171C4.62997 2.04173 4.78003 0.681954 5.52995 0.431867Z";

fn hex(i: usize) -> String {
    let rgba = gpui::Rgba::from(c(tok(i)));
    let v = |x: f32| (x * 255.).round() as u32;
    format!("#{:02X}{:02X}{:02X}{:02X}", v(rgba.r), v(rgba.g), v(rgba.b), v(rgba.a))
}

pub fn paths_of(svg: &str) -> Vec<String> {
    svg.split(" d=\"").skip(1).filter_map(|s| s.split('"').next().map(str::to_owned)).collect()
}

pub fn view_box_width(svg: &str) -> f32 {
    svg.split("viewBox=\"").nth(1).and_then(|v| v.split('"').next()).and_then(|v| v.split_whitespace().nth(2)).and_then(|w| w.parse().ok()).unwrap_or(24.)
}

pub fn badge_svg(look: &Look, glyph_paths: &[String], glyph_box: f32, size: f32) -> String {
    let scale = size / 16.;
    let (defs, fill) = match &look.bg {
        Fill::Flat(i) => (String::new(), hex(*i)),
        Fill::Gradient(a, b) => (
            format!(r#"<defs><linearGradient id="g" gradientTransform="rotate(45)"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs>"#, hex(*a), hex(*b)),
            "url(#g)".to_owned(),
        ),
    };
    let g = look.glyph_size;
    let off = (size - g) / 2.;
    let glyph: String = glyph_paths.iter().map(|d| format!(r#"<path fill="{}" d="{d}"/>"#, hex(look.fg))).collect();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">{defs}<path transform="scale({scale})" fill="{fill}" fill-rule="evenodd" clip-rule="evenodd" d="{FLOWER_STAR}"/><g transform="translate({off} {off}) scale({})">{glyph}</g></svg>"#,
        g / glyph_box
    )
}

thread_local! {
    static CACHE: RefCell<HashMap<(Kind, bool, bool), Arc<gpui::Image>>> = RefCell::new(HashMap::new());
}

pub fn element(kind: Kind, premium: bool, size: f32) -> Option<AnyElement> {
    let dark = !crate::theme::is_light();
    let look = look(kind, premium, dark, size)?;
    let image = CACHE.with(|m| {
        m.borrow_mut()
            .entry((kind, premium, dark))
            .or_insert_with(|| {
                let svg = crate::icons::lookup(&crate::icons::path(look.glyph)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
                Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, badge_svg(&look, &paths_of(&svg), view_box_width(&svg), size).into_bytes()))
            })
            .clone()
    });
    Some(div().size(z(size)).flex_none().child(gpui::img(image).size(z(size))).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn badge_kind_follows_the_client_order() {
        assert_eq!(kind(&f(&["INTERNAL_EMPLOYEE_ONLY", "VERIFIED"])), Kind::Staff);
        assert_eq!(kind(&f(&["VERIFIED", "PARTNERED"])), Kind::VerifiedAndPartnered);
        assert_eq!(kind(&f(&["PARTNERED", "COMMUNITY"])), Kind::Partnered);
        assert_eq!(kind(&f(&["COMMUNITY", "DISCOVERABLE"])), Kind::Discoverable);
        assert_eq!(kind(&f(&["COMMUNITY"])), Kind::Community);
        assert_eq!(kind(&f(&["DISCOVERABLE"])), Kind::None);
    }

    #[test]
    fn community_badge_colors_and_glyph_size() {
        let l = look(Kind::Community, false, true, 18.).unwrap();
        assert_eq!((l.glyph, l.fg, l.bg.clone(), l.glyph_size), ("guild/home", idx::PRIMARY_500, Fill::Flat(idx::WHITE), 11.));
        let p = look(Kind::Community, true, true, 18.).unwrap();
        assert_eq!((p.fg, p.bg), (idx::WHITE, Fill::Gradient(idx::PREMIUM_TIER_2_PURPLE, idx::PREMIUM_TIER_2_PINK)));
        assert_eq!(look(Kind::Verified, false, true, 18.).unwrap().glyph_size, 13.);
        assert!(look(Kind::Discoverable, false, true, 18.).is_none());
        let p = look(Kind::Partnered, false, true, 18.).unwrap();
        assert_eq!((p.glyph, p.fg, p.bg, p.glyph_size), ("guild/partner", idx::WHITE, Fill::Flat(idx::BRAND_500), 13.));
        assert_eq!(look(Kind::Staff, false, true, 18.).unwrap().glyph, "guild/staff");
        assert_eq!(view_box_width(r#"<svg viewBox="0 0 16 16"><path d="M1"/></svg>"#), 16.);
        assert_eq!(view_box_width("<svg><path/></svg>"), 24.);
        assert_eq!(paths_of(r#"<svg><path d="M1 2"/><path fill="x" d="M3 4Z"/></svg>"#), vec!["M1 2".to_owned(), "M3 4Z".to_owned()]);
    }
}
