use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{generated::idx, z},
    ui::shop_page::{Block, Home, card_sized, hero, secondary, tk},
};

pub const PRODUCT_EFFECT: u64 = 1;
pub const PRODUCT_BUNDLE: u64 = 1000;
pub const PRODUCT_VARIANTS: u64 = 2000;
pub const SECTION_MAX_W: f32 = 1124.;
pub const SECTION_PAD: f32 = 20.;
pub const GRID_GAP: f32 = 24.;
pub const RELATED_MAX: usize = 8;
pub const CAROUSEL_SCOPE: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Square,
    Tall,
    Wide,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub sku: u64,
    pub name: SharedString,
    pub hero_banner: Option<SharedString>,
    pub hero_logo: Option<SharedString>,
    pub backdrop: Option<(u32, u32)>,
    pub hero_ranking: Vec<u64>,
    pub catalog_banner: Option<SharedString>,
    pub featured_block: Option<SharedString>,
    pub products: Vec<(u64, Shape)>,
    pub unpublished: bool,
}

fn url(v: &Value, key: &str) -> Option<SharedString> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| SharedString::from(s.to_owned()))
}

pub fn shape(p: &Value) -> Shape {
    let ty = p.get("type").and_then(Value::as_u64);
    let variants: Vec<Option<u64>> = p.get("variants").and_then(Value::as_array).into_iter().flatten().map(|v| v.get("type").and_then(Value::as_u64)).collect();
    match ty {
        Some(PRODUCT_EFFECT) => Shape::Tall,
        Some(PRODUCT_BUNDLE) => Shape::Wide,
        Some(PRODUCT_VARIANTS) if !variants.is_empty() && variants.iter().all(|t| *t == Some(PRODUCT_EFFECT)) => Shape::Tall,
        _ => Shape::Square,
    }
}

pub fn parse_categories(v: &Value) -> Vec<Category> {
    v.get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let ids = |k: &str| -> Vec<u64> { c.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.as_str()?.parse().ok()).collect() };
            Some(Category {
                sku: c.get("sku_id")?.as_str()?.parse().ok()?,
                name: c.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(),
                hero_banner: url(c, "hero_banner_url"),
                hero_logo: url(c, "hero_logo_url"),
                backdrop: c
                    .get("hero_banner_display_config")
                    .and_then(|d| d.get("background_style"))
                    .and_then(Value::as_str)
                    .and_then(crate::ui::shop_page::backdrop),
                hero_ranking: ids("hero_ranking"),
                catalog_banner: url(c, "catalog_banner_url"),
                featured_block: url(c, "featured_block_url"),
                products: c
                    .get("products")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|p| Some((p.get("sku_id")?.as_str()?.parse().ok()?, shape(p))))
                    .collect(),
                unpublished: c.get("unpublished_at").is_some_and(|u| !u.is_null()),
            })
        })
        .collect()
}

pub fn columns(container: f32) -> usize {
    match container {
        w if w <= 480. => 1,
        w if w <= 768. => 2,
        w if w <= 896. => 3,
        _ => 4,
    }
}

pub fn pack(shapes: &[Shape], cols: usize) -> (Vec<(usize, usize)>, usize) {
    let cols = cols.max(1);
    let mut taken: Vec<Vec<bool>> = Vec::new();
    let mut out = Vec::with_capacity(shapes.len());
    for s in shapes {
        let (w, h) = match s {
            Shape::Square => (1, 1),
            Shape::Tall => (1, 2),
            Shape::Wide => (2.min(cols), 1),
        };
        let mut r = 0;
        let spot = loop {
            while taken.len() < r + h {
                taken.push(vec![false; cols]);
            }
            if let Some(c) = (0..=cols - w).find(|&c| (r..r + h).all(|rr| (c..c + w).all(|cc| !taken[rr][cc]))) {
                break (r, c);
            }
            r += 1;
        };
        for row in taken.iter_mut().skip(spot.0).take(h) {
            for cell in row.iter_mut().skip(spot.1).take(w) {
                *cell = true;
            }
        }
        out.push(spot);
    }
    let rows = taken.iter().rposition(|r| r.iter().any(|c| *c)).map_or(0, |i| i + 1);
    (out, rows)
}

pub fn related(all: &[Category], current: u64) -> Vec<Category> {
    let start = all.iter().position(|c| c.sku == current).map_or(0, |i| i + 1);
    all.iter().skip(start).chain(all.iter().take(start)).filter(|c| c.sku != current && c.featured_block.is_some() && !c.unpublished).take(RELATED_MAX).cloned().collect()
}

fn breadcrumb(name: SharedString, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    div()
        .absolute()
        .left(z(20.))
        .top(z(10.))
        .flex()
        .gap(z(10.))
        .py(z(6.))
        .px(z(8.))
        .rounded(z(4.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH).opacity(0.65))
        .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.16), offset: gpui::point(z(0.), z(4.)), blur_radius: z(8.), spread_radius: z(0.), inset: false }])
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .child(
            div()
                .id("shop-crumb-all")
                .flex()
                .items_center()
                .gap(z(10.))
                .cursor_pointer()
                .text_color(tk(idx::TEXT_MUTED))
                .hover(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
                .on_click(cx.listener(|_, _, _, cx| {
                    crate::ui::shop_page::select_tab(1);
                    cx.notify();
                }))
                .child(crate::t!("shop.shop_all"))
                .child(div().text_color(tk(idx::TEXT_MUTED)).child("/")),
        )
        .child(div().text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)).child(name))
}

fn grid(app: &PhoenixApp, home: &Home, cat: &Category, inner: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let items: Vec<(u64, Shape)> = cat.products.iter().filter(|(s, _)| home.cards.contains_key(s)).copied().collect();
    let cols = columns(inner);
    let cell = (inner - GRID_GAP * (cols as f32 - 1.)) / cols as f32;
    let shapes: Vec<Shape> = items.iter().map(|(_, s)| *s).collect();
    let (spots, rows) = pack(&shapes, cols);
    let mut g = div().relative().w_full().h(z((rows as f32 * (cell + GRID_GAP) - GRID_GAP).max(0.)));
    for ((sku, shape), (r, c)) in items.iter().zip(spots) {
        let Some(k) = home.cards.get(sku) else { continue };
        let (w, h) = match shape {
            Shape::Square => (cell, cell),
            Shape::Tall => (cell, 2. * cell + GRID_GAP),
            Shape::Wide if cols > 1 => (2. * cell + GRID_GAP, cell),
            Shape::Wide => (cell, cell),
        };
        g = g.child(
            div()
                .absolute()
                .left(z(c as f32 * (cell + GRID_GAP)))
                .top(z(r as f32 * (cell + GRID_GAP)))
                .child(card_sized(app, k, w, h, CAROUSEL_SCOPE + 1, cx)),
        );
    }
    g.into_any_element()
}

fn section(child: impl IntoElement) -> gpui::Div {
    div().w_full().flex().justify_center().child(div().w_full().max_w(z(SECTION_MAX_W)).px(z(SECTION_PAD)).child(child))
}

fn related_row(cats: &[Category], inner: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let per = if inner >= 700. { 2 } else { 1 };
    let w = (inner - GRID_GAP * (per as f32 - 1.)) / per as f32;
    let mut row = div().flex().gap(z(GRID_GAP)).overflow_hidden();
    for (i, c) in cats.iter().take(per).enumerate() {
        let sku = c.sku;
        row = row.child(
            div()
                .id(("shop-related", i))
                .w(z(w))
                .flex_none()
                .h(z(crate::ui::shop_page::MARKETING_H))
                .relative()
                .rounded(z(16.))
                .overflow_hidden()
                .cursor_pointer()
                .bg(tk(idx::BACKGROUND_BASE_LOW))
                .on_click(cx.listener(move |_, _, _, cx| {
                    crate::ui::shop_page::open_collection(sku);
                    cx.notify();
                }))
                .children(c.featured_block.clone().map(|u| img(u.to_string()).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))),
        );
    }
    row.into_any_element()
}

fn footer(cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let tabs = [crate::t!("shop.cat_decorations"), crate::t!("shop.cat_effects"), crate::t!("shop.cat_nameplates"), crate::t!("shop.cat_frames"), crate::t!("shop.cat_bundles")];
    let mut row = div().flex().gap(z(10.)).justify_center().max_w_full();
    for (i, t) in tabs.into_iter().enumerate() {
        row = row.child(secondary(SharedString::from(format!("shop-footer-{i}")), t).on_click(cx.listener(|_, _, _, cx| {
            crate::ui::shop_page::select_tab(1);
            cx.notify();
        })));
    }
    div()
        .mt(z(64.))
        .px(z(20.))
        .flex()
        .flex_col()
        .items_center()
        .gap(z(24.))
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("shop.so_much_more")))
        .child(row)
}

pub fn render(app: &PhoenixApp, sku: u64, width: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let cats = crate::ui::shop_page::categories_loaded();
    let (Some(home), Some(cat)) = (crate::ui::shop_page::home_loaded(), cats.iter().find(|c| c.sku == sku).cloned()) else {
        return div().p(z(32.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("common.loading")).into_any_element();
    };
    let inner = width.min(SECTION_MAX_W) - 2. * SECTION_PAD;
    let top: AnyElement = if cat.hero_banner.is_some() {
        let b = Block::Hero { banner: cat.hero_banner.clone(), logo: cat.hero_logo.clone(), backdrop: cat.backdrop, skus: cat.hero_ranking.iter().copied().filter(|s| home.cards.contains_key(s)).collect(), sale: None };
        div()
            .relative()
            .child(hero(app, &home, CAROUSEL_SCOPE, &b, width - 2. * crate::ui::shop_page::FEED_PAD_X, cx))
            .child(breadcrumb(cat.name.clone(), cx))
            .into_any_element()
    } else {
        div()
            .relative()
            .pt(z(48.))
            .child(breadcrumb(cat.name.clone(), cx))
            .child(section(div().my(z(24.)).w_full().h(z(220.)).rounded(z(16.)).overflow_hidden().children(cat.catalog_banner.clone().map(|u| img(u.to_string()).size_full().object_fit(gpui::ObjectFit::Cover)))))
            .into_any_element()
    };
    let rel = related(&cats, cat.sku);
    div()
        .w_full()
        .pb(z(48.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .flex()
        .flex_col()
        .child(top)
        .child(section(grid(app, &home, &cat, inner, cx)))
        .when(!rel.is_empty(), |d| {
            d.child(
                section(
                    div()
                        .pt(z(64.))
                        .child(div().mb(z(24.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("shop.fresh_drops")))
                        .child(related_row(&rel, inner, cx)),
                ),
            )
        })
        .child(footer(cx))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grid_columns_follow_the_container_queries() {
        assert_eq!(columns(1084.), 4);
        assert_eq!(columns(896.), 3);
        assert_eq!(columns(768.), 2);
        assert_eq!(columns(480.), 1);
    }

    #[test]
    fn dense_packing_backfills_holes_like_css_grid() {
        use Shape::*;
        let (spots, rows) = pack(&[Square, Square, Square, Wide, Tall, Square], 4);
        assert_eq!(spots, vec![(0, 0), (0, 1), (0, 2), (1, 0), (0, 3), (1, 2)]);
        assert_eq!(rows, 2);
        let (spots, rows) = pack(&[Wide, Square], 1);
        assert_eq!((spots, rows), (vec![(0, 0), (1, 0)], 2));
        assert_eq!(pack(&[], 3), (vec![], 0));
    }

    #[test]
    fn product_shapes_match_the_client_aspect_ratios() {
        assert_eq!(shape(&json!({"type": 1})), Shape::Tall);
        assert_eq!(shape(&json!({"type": 1000})), Shape::Wide);
        assert_eq!(shape(&json!({"type": 2000, "variants": [{"type": 1}, {"type": 1}]})), Shape::Tall);
        assert_eq!(shape(&json!({"type": 2000, "variants": [{"type": 0}]})), Shape::Square);
        assert_eq!(shape(&json!({"type": 0})), Shape::Square);
    }

    #[test]
    fn categories_and_related_drops_skip_the_current_and_unpublished_ones() {
        let v = json!({"categories": [
            {"sku_id": "1", "name": "A", "featured_block_url": "f1", "hero_ranking": ["10"], "products": [{"sku_id": "10", "type": 1}],
             "hero_banner_display_config": {"background_style": "linear-gradient(180deg, #000000 0%, #616161 100%)"}},
            {"sku_id": "2", "name": "B", "featured_block_url": "f2", "unpublished_at": "2026-01-01"},
            {"sku_id": "3", "name": "C"},
            {"sku_id": "4", "name": "D", "featured_block_url": "f4"}
        ]});
        let c = parse_categories(&v);
        assert_eq!(c.len(), 4);
        assert_eq!(c[0].products, vec![(10, Shape::Tall)]);
        assert_eq!(c[0].backdrop, Some((0, 0x616161)));
        assert!(c[1].unpublished);
        assert_eq!(related(&c, 1).iter().map(|c| c.sku).collect::<Vec<_>>(), vec![4]);
        assert_eq!(related(&c, 4).iter().map(|c| c.sku).collect::<Vec<_>>(), vec![1]);
    }
}
