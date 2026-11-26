use std::cell::{Cell, RefCell};

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

#[derive(Clone, Debug, PartialEq)]
pub struct ShopCard {
    pub sku: u64,
    pub name: SharedString,
    pub items: usize,
    pub preview: Option<SharedString>,
    pub bundle_art: Option<(SharedString, SharedString)>,
    pub base: Option<(i64, u32, String)>,
    pub nitro: Option<(i64, u32, String)>,
    pub sale: Option<(i64, u32, String)>,
    pub orbs: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub asset: SharedString,
    pub category: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Hero { banner: Option<SharedString>, logo: Option<SharedString>, backdrop: Option<(u32, u32)>, skus: Vec<u64> },
    Marketing(Vec<Tile>),
    Shelf { name: SharedString, skus: Vec<u64>, button: Option<SharedString> },
    Picks(Vec<u64>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Home {
    pub blocks: Vec<Block>,
    pub cards: std::collections::HashMap<u64, ShopCard>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    Fiat { price: (i64, u32, String), former: Option<(i64, u32, String)>, nitro: bool },
    Orbs(u64),
}

thread_local! {
    static CARDS: RefCell<Option<Vec<ShopCard>>> = const { RefCell::new(None) };
    static HOME: RefCell<Option<std::rc::Rc<Home>>> = const { RefCell::new(None) };
    static CAROUSEL: RefCell<std::collections::HashMap<usize, usize>> = RefCell::new(std::collections::HashMap::new());
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static TAB: Cell<usize> = const { Cell::new(0) };
    static ORBS: Cell<Option<u64>> = const { Cell::new(None) };
    static SEARCH: RefCell<Option<gpui::Entity<gpui_component::input::InputState>>> = const { RefCell::new(None) };
}

pub const TIER_DEFAULT: &str = "0";
pub const TIER_SALE: &str = "2";
pub const TIER_PREMIUM_2: &str = "4";
pub const ORB_CURRENCY: &str = "discord_orb";

fn tier_prices<'a>(p: &'a Value, tier: &str) -> impl Iterator<Item = &'a Value> {
    p.get("prices").and_then(|x| x.get(tier)).and_then(|x| x.get("country_prices")).and_then(|x| x.get("prices")).and_then(Value::as_array).into_iter().flatten()
}

fn price(p: &Value, tier: &str) -> Option<(i64, u32, String)> {
    let x = tier_prices(p, tier).find(|x| x.get("currency").and_then(Value::as_str) != Some(ORB_CURRENCY))?;
    Some((x.get("amount")?.as_i64()?, x.get("exponent").and_then(Value::as_u64).unwrap_or(2) as u32, x.get("currency")?.as_str()?.to_owned()))
}

fn orb_price(p: &Value) -> Option<u64> {
    tier_prices(p, TIER_DEFAULT).find(|x| x.get("currency").and_then(Value::as_str) == Some(ORB_CURRENCY))?.get("amount")?.as_u64()
}

pub fn card_of(p: &Value) -> Option<ShopCard> {
    let sku = p.get("sku_id").and_then(Value::as_str).and_then(|s| s.parse().ok())?;
    let bundle = p.get("type").and_then(Value::as_u64) == Some(1000);
    let art = p.get("preview_assets").and_then(|a| Some((SharedString::from(a.get("bg_static")?.as_str()?.to_owned()), SharedString::from(a.get("fg_static")?.as_str()?.to_owned()))));
    Some(ShopCard {
        sku,
        name: p.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(),
        items: if bundle { p.get("items").and_then(Value::as_array).map_or(0, Vec::len) } else { 0 },
        preview: crate::ui::settings::collectibles::product_preview(p),
        bundle_art: art.filter(|_| bundle),
        base: price(p, TIER_DEFAULT),
        nitro: price(p, TIER_PREMIUM_2),
        sale: price(p, TIER_SALE),
        orbs: orb_price(p),
    })
}

pub fn parse(v: &Value) -> Vec<ShopCard> {
    let mut seen = std::collections::HashSet::new();
    phoenix_api::rest::collectibles::products(v).filter_map(card_of).filter(|k| seen.insert(k.sku)).collect()
}

fn skus(v: &Value, key: &str) -> Vec<u64> {
    v.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.as_str()?.parse().ok()).collect()
}

fn url(v: &Value, key: &str) -> Option<SharedString> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| SharedString::from(s.to_owned()))
}

pub fn backdrop(style: &str) -> Option<(u32, u32)> {
    let hex: Vec<u32> = style.split('#').skip(1).filter_map(|h| u32::from_str_radix(h.get(..6)?, 16).ok()).collect();
    match hex.as_slice() {
        [a, .., b] => Some((*a, *b)),
        _ => None,
    }
}

pub fn parse_home(home: &Value, catalog: &Value) -> Home {
    let mut cards = std::collections::HashMap::new();
    for p in phoenix_api::rest::collectibles::products(catalog).chain(phoenix_api::rest::collectibles::products(home)) {
        if let Some(k) = card_of(p) {
            cards.insert(k.sku, k);
        }
    }
    let mut blocks = Vec::new();
    for b in home.get("shop_blocks").and_then(Value::as_array).into_iter().flatten() {
        match b.get("type").and_then(Value::as_u64) {
            Some(0) => blocks.push(Block::Hero {
                banner: url(b, "hero_banner_url"),
                logo: url(b, "hero_logo_url"),
                backdrop: b.get("banner_display_config").and_then(|c| c.get("background_style")).and_then(Value::as_str).and_then(backdrop),
                skus: skus(b, "ranked_sku_ids"),
            }),
            Some(1) => blocks.push(Block::Marketing(
                b.get("subblocks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|t| Some(Tile { asset: url(t, "asset_url")?, category: t.get("category_sku_id")?.as_str()?.parse().ok()? }))
                    .collect(),
            )),
            Some(4) => blocks.push(Block::Shelf {
                name: b.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(),
                skus: skus(b, "ranked_sku_ids"),
                button: b.get("show_button").and_then(Value::as_bool).unwrap_or(false).then(|| url(b, "button_text")).flatten(),
            }),
            Some(2) => {
                let sorted = b.get("sorted_sku_ids").map(|s| skus(s, "recommended")).filter(|v| !v.is_empty());
                blocks.push(Block::Picks(sorted.unwrap_or_else(|| skus(b, "ranked_sku_ids"))));
            }
            _ => {}
        }
    }
    for blk in &mut blocks {
        match blk {
            Block::Hero { skus, .. } | Block::Shelf { skus, .. } | Block::Picks(skus) => skus.retain(|s| cards.contains_key(s)),
            Block::Marketing(_) => {}
        }
    }
    Home { blocks, cards }
}

pub fn shown_price(k: &ShopCard, premium: bool, balance: Option<u64>) -> Option<Shown> {
    let fiat = if premium { k.nitro.clone().or_else(|| k.base.clone()) } else { k.base.clone() };
    if let Some(o) = k.orbs
        && (fiat.is_none() || balance.is_some_and(|b| b >= o))
    {
        return Some(Shown::Orbs(o));
    }
    let fiat = fiat?;
    let nitro = premium && matches!((&k.nitro, &k.base), (Some(n), Some(b)) if n.0 < b.0);
    Some(match k.sale.clone() {
        Some(s) if s.0 < fiat.0 => Shown::Fiat { price: s, former: Some(fiat), nitro: false },
        _ => Shown::Fiat { price: fiat, former: None, nitro },
    })
}

pub fn format_price(amount: i64, exponent: u32, currency: &str) -> String {
    let div = 10i64.pow(exponent);
    let (whole, frac) = (amount / div, amount % div);
    let sym = match currency {
        "eur" => "€",
        "usd" => "$",
        "gbp" => "£",
        other => other,
    };
    if crate::i18n::locale() == "fr" {
        format!("{whole},{frac:0w$}\u{a0}{sym}", w = exponent as usize)
    } else {
        format!("{sym}{whole}.{frac:0w$}", w = exponent as usize)
    }
}

pub fn discount(base: i64, mine: i64) -> Option<u32> {
    (base > 0 && mine < base).then(|| ((base - mine) * 100 + base / 2) as u32 / base as u32).filter(|p| *p > 0)
}

fn ensure_loaded(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if REQUESTED.with(|r| r.replace(true)) {
        return;
    }
    let orbs = app.backend.orbs_balance();
    cx.spawn(async move |this, cx| {
        if let Ok(b) = orbs.await {
            ORBS.with(|o| o.set(b));
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
    let fut = app.backend.shop_storefront();
    let home = app.backend.shop_home();
    cx.spawn(async move |this, cx| {
        let v = fut.await.unwrap_or(Value::Null);
        let h = home.await.unwrap_or(Value::Null);
        let built = cx.background_executor().spawn(async move { (parse(&v), parse_home(&h, &v)) }).await;
        CARDS.with(|c| *c.borrow_mut() = Some(built.0));
        HOME.with(|c| *c.borrow_mut() = Some(std::rc::Rc::new(built.1)));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn glyph(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).size(z(size)).flex_none().text_color(color)
}

pub const SEARCH_PLACEHOLDER: &str = "shop.search";

fn search_input(window: &mut Window, cx: &mut Context<PhoenixApp>) -> gpui::Entity<gpui_component::input::InputState> {
    if let Some(i) = SEARCH.with(|s| s.borrow().clone()) {
        crate::ui::locale_inputs::keep_placeholder(&i, SEARCH_PLACEHOLDER, window, cx);
        return i;
    }
    let i = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(crate::t!(SEARCH_PLACEHOLDER)));
    crate::ui::locale_inputs::keep_placeholder(&i, SEARCH_PLACEHOLDER, window, cx);
    cx.subscribe_in(&i, window, |_: &mut PhoenixApp, _, ev: &gpui_component::input::InputEvent, _, cx| {
        if let gpui_component::input::InputEvent::Change = ev {
            cx.notify();
        }
    })
    .detach();
    SEARCH.with(|s| *s.borrow_mut() = Some(i.clone()));
    i
}

fn header(window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let search = search_input(window, cx);
    let tabs = [crate::t!("shop.tab_now"), crate::t!("shop.tab_browse"), crate::t!("shop.tab_orbs"), crate::t!("shop.tab_more")];
    let carets = [false, true, false, true];
    let sel = TAB.with(Cell::get);
    let mut nav = div().flex().items_center().gap(z(32.)).ml(z(44.));
    for (i, (label, caret)) in tabs.into_iter().zip(carets).enumerate() {
        nav = nav.child(
            div()
                .id(("shop-tab", i))
                .h(z(48.))
                .pt(z(2.))
                .border_b_2()
                .border_color(if i == sel { tk(idx::TEXT_STRONG) } else { gpui::transparent_black() })
                .flex()
                .items_center()
                .gap(z(2.))
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_DEFAULT))
                .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
                .on_click(cx.listener(move |_, _, _, cx| {
                    TAB.with(|t| t.set(i));
                    cx.notify();
                }))
                .child(label)
                .when(caret, |d| d.child(glyph("chevron", 18., tk(idx::TEXT_DEFAULT)))),
        );
    }
    let pill = |id: &'static str| {
        div()
            .id(id)
            .h(z(32.))
            .rounded(z(8.))
            .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
            .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
    };
    div()
        .flex_none()
        .h(z(49.))
        .pl(z(20.))
        .pr(z(14.))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .flex()
        .items_center()
        .child(glyph("shop", 24., tk(idx::TEXT_DEFAULT)))
        .child(nav)
        .child(div().flex_1())
        .child(
            div()
                .pl(z(10.))
                .flex()
                .items_center()
                .gap(z(10.))
                .child(
                    div()
                        .w(z(256.))
                        .h(z(32.))
                        .pl(z(4.))
                        .rounded(z(8.))
                        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                        .border_1()
                        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .text_size(z(14.))
                        .text_color(tk(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT))
                        .child(div().size(z(22.)).flex().items_center().justify_center().child(glyph("search", 16., tk(idx::ICON_MUTED))))
                        .child(
                            div().flex_1().min_w_0().child(
                                gpui_component::input::Input::new(&search).appearance(false).bordered(false).pl(z(0.)).text_size(z(14.)),
                            ),
                        ),
                )
                .child(pill("shop-wishlist").w(z(32.)).child(glyph("heart", 20., tk(idx::INTERACTIVE_TEXT_DEFAULT))))
                .child(
                    pill("shop-orbs")
                        .px(z(12.))
                        .gap(z(2.))
                        .child(glyph("orbs", 20., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                        .children(ORBS.with(Cell::get).map(|b| {
                            div()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(b.to_string())
                        })),
                ),
        )
}

fn price_label(shown: &Shown) -> AnyElement {
    let fmt = |p: &(i64, u32, String)| format_price(p.0, p.1, &p.2);
    match shown {
        Shown::Orbs(o) => div()
            .flex()
            .items_center()
            .gap(z(4.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .child(glyph("orbs", 18., tk(idx::TEXT_STRONG)))
            .child(o.to_string())
            .into_any_element(),
        Shown::Fiat { price, former, nitro } => div()
            .flex()
            .items_center()
            .gap(z(8.))
            .children(former.as_ref().map(|b| div().font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).line_through().child(fmt(b))))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tk(idx::TEXT_STRONG))
                    .when(*nitro, |d| d.child(glyph("nitro", 18., tk(idx::TEXT_STRONG))))
                    .child(fmt(price)),
            )
            .into_any_element(),
    }
}

fn card(app: &PhoenixApp, k: &ShopCard, width: f32, scope: usize, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let premium = app.backend.has_premium();
    let shown = shown_price(k, premium, ORBS.with(Cell::get));
    let off = match &shown {
        Some(Shown::Fiat { price, former: Some(f), .. }) => discount(f.0, price.0),
        _ => None,
    };
    let group: SharedString = format!("shop-card-{scope}-{}", k.sku).into();
    let name = k.name.clone();
    let stub = crate::t!("shop.preview_only");
    let stub2 = stub.clone();
    let buy = match &shown {
        Some(Shown::Fiat { price, .. }) => crate::t!("shop.buy_for", price = format_price(price.0, price.1, &price.2)),
        _ => crate::t!("shop.buy"),
    };
    let buy_id: SharedString = format!("shop-buy-{scope}-{}", k.sku).into();
    let gift_id: SharedString = format!("shop-gift-{scope}-{}", k.sku).into();
    div()
        .id(group.clone())
        .group(group.clone())
        .w(z(width))
        .h(z(CARD_SIZE))
        .flex_none()
        .relative()
        .overflow_hidden()
        .rounded(z(8.))
        .border_2()
        .border_color(tk(idx::BORDER_MUTED))
        .hover(|s| s.border_color(tk(idx::BORDER_SUBTLE)))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .cursor_pointer()
        .when_some(k.bundle_art.clone(), |d, (bg, fg)| {
            d.bg(gpui::black())
                .child(img(bg.to_string()).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
                .child(img(fg.to_string()).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
                .child(div().absolute().left_0().right_0().bottom_0().h(z(100.)).bg(gpui::linear_gradient(
                    0.,
                    gpui::linear_color_stop(gpui::black().opacity(0.92), 0.),
                    gpui::linear_color_stop(gpui::black().opacity(0.), 0.74),
                )))
        })
        .when(k.bundle_art.is_none(), |d| {
            d.children(k.preview.clone().map(|u| {
                div()
                    .absolute()
                    .inset_0()
                    .group_hover(group.clone(), |s| s.top(z(-6.)).bottom(z(6.)))
                    .px(z(8.))
                    .pb(z(56.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(img(u.to_string()).size(z(150.)).object_fit(gpui::ObjectFit::Contain))
            }))
            .child(div().absolute().left_0().right_0().bottom_0().h(z(146.)).bg(gpui::linear_gradient(
                0.,
                gpui::linear_color_stop(tk(idx::BACKGROUND_BASE_LOWEST), 0.),
                gpui::linear_color_stop(tk(idx::BACKGROUND_BASE_LOWEST).opacity(0.), 1.),
            )))
        })
        .children(off.map(|p| {
            div().absolute().top(z(12.)).left(z(12.)).child(
                div()
                    .h(z(16.))
                    .px(z(8.))
                    .rounded_full()
                    .bg(gpui::white())
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(gpui::black())
                    .child(glyph("tag", 12., gpui::black()))
                    .child(crate::t!("shop.discount", n = p).to_uppercase()),
            )
        }))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .p(z(12.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(z(20.))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(div().min_w_0().overflow_hidden().text_ellipsis().child(bundle_title(k.items, &name))),
                )
                .child(
                    div()
                        .relative()
                        .h(z(46.))
                        .child(
                            div()
                                .absolute()
                                .top(z(4.))
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .group_hover(group.clone(), |s| s.opacity(0.))
                                .children(shown.as_ref().map(price_label)),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(z(6.))
                                .left_0()
                                .right_0()
                                .flex()
                                .gap(z(8.))
                                .opacity(0.)
                                .group_hover(group, |s| s.opacity(1.))
                                .child(
                                    crate::ui::controls::button(buy_id, buy, crate::ui::controls::BtnKind::Primary)
                                        .flex_1()
                                        .h(z(40.))
                                        .on_click(cx.listener(move |a, _, _, cx| a.stub(&stub, cx))),
                                )
                                .child(
                                    crate::ui::controls::button(gift_id, SharedString::default(), crate::ui::controls::BtnKind::Primary)
                                        .size(z(40.))
                                        .child(glyph("gift", 20., gpui::white()))
                                        .on_click(cx.listener(move |a, _, _, cx| a.stub(&stub2, cx))),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

pub fn bundle_label(items: usize, name: &str) -> (String, usize) {
    if items == 0 {
        return (name.to_owned(), 0);
    }
    let count = crate::t!("shop.items", n = items).to_string();
    let bold = count.len();
    (format!("{count} {name}"), bold)
}

fn bundle_title(items: usize, name: &str) -> gpui::StyledText {
    let (text, bold) = bundle_label(items, name);
    let highlights = if bold > 0 { vec![(0..bold, gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })] } else { Vec::new() };
    gpui::StyledText::new(text).with_highlights(highlights)
}

pub const CARD_SIZE: f32 = 246.;
pub const CARD_GAP: f32 = 30.;
pub const GRID_PAD: f32 = 32.;
pub const ROW_H: f32 = CARD_SIZE + CARD_GAP;

pub fn grid_columns(width: f32) -> usize {
    (((width - 2. * GRID_PAD + CARD_GAP) / ROW_H).floor() as usize).max(1)
}

pub fn rows_in_view(rows: usize, top: f32, offset: f32, view_h: f32) -> (usize, usize) {
    let first = (((offset - top) / ROW_H).floor().max(0.) as usize).saturating_sub(1).min(rows);
    let last = ((((offset + view_h - top) / ROW_H).ceil().max(0.) as usize) + 1).min(rows);
    (first, last.max(first))
}

pub fn visible_rows(rows: usize, offset: f32, view_h: f32) -> (usize, usize) {
    rows_in_view(rows, GRID_PAD, offset, view_h)
}

pub const FEED_PAD_X: f32 = 32.;
pub const FEED_GUTTER: f32 = 8.;
pub const BLOCK_PAD: f32 = 30.;
pub const HERO_HEADER: f32 = 258.;
pub const HERO_PAD_BOTTOM: f32 = 24.;
pub const HERO_H: f32 = HERO_HEADER + CARD_SIZE + HERO_PAD_BOTTOM;
pub const BANNER_H: f32 = 350.;
pub const MARKETING_H: f32 = 236.;
pub const SECTION_HEAD: f32 = 40. + 24.;

pub fn fit(inner: f32) -> usize {
    (((inner + CARD_GAP) / ROW_H).floor() as usize).max(1)
}

pub fn pick_width(inner: f32, cols: usize) -> f32 {
    (inner - CARD_GAP * (cols as f32 - 1.)) / cols as f32
}

pub fn block_height(b: &Block, inner: f32) -> f32 {
    match b {
        Block::Hero { .. } => HERO_H,
        Block::Marketing(_) => MARKETING_H + 2. * BLOCK_PAD,
        Block::Shelf { .. } => 2. * BLOCK_PAD + SECTION_HEAD + CARD_SIZE,
        Block::Picks(skus) => 2. * BLOCK_PAD + SECTION_HEAD + (skus.len().div_ceil(fit(inner)) as f32 * ROW_H - CARD_GAP).max(0.),
    }
}

pub fn carousel_start(first: usize, len: usize, fit: usize, step: isize) -> usize {
    let max = len.saturating_sub(fit);
    (first as isize + step * fit as isize).clamp(0, max as isize) as usize
}

fn arrow(id: SharedString, icon: &'static str, left: bool, block: usize, len: usize, fit: usize, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let step = if left { -1 } else { 1 };
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .when(left, |d| d.left(z(12.)))
        .when(!left, |d| d.right(z(12.)))
        .flex()
        .items_center()
        .child(
            div()
                .id(id)
                .size(z(40.))
                .rounded(z(8.))
                .border_1()
                .border_color(gpui::rgba(0x97979f29))
                .bg(gpui::black().opacity(0.56))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(glyph(icon, 20., gpui::white()))
                .on_click(cx.listener(move |_, _, _, cx| {
                    CAROUSEL.with(|c| {
                        let mut c = c.borrow_mut();
                        let cur = c.get(&block).copied().unwrap_or(0);
                        c.insert(block, carousel_start(cur, len, fit, step));
                    });
                    cx.notify();
                })),
        )
}

fn carousel(app: &PhoenixApp, home: &Home, block: usize, skus: &[u64], inner: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let n = fit(inner);
    let len = skus.len();
    let first = CAROUSEL.with(|c| c.borrow().get(&block).copied().unwrap_or(0)).min(len.saturating_sub(n));
    let mut row = div().absolute().top_0().left_0().flex().gap(z(CARD_GAP));
    for sku in skus.iter().skip(first).take(n + 1) {
        if let Some(k) = home.cards.get(sku) {
            row = row.child(card(app, k, CARD_SIZE, block + 1, cx));
        }
    }
    div()
        .relative()
        .w_full()
        .h(z(CARD_SIZE))
        .overflow_hidden()
        .child(row)
        .when(first > 0, |d| d.child(arrow(format!("shop-prev-{block}").into(), "chevron-small-left", true, block, len, n, cx)))
        .when(first + n < len, |d| d.child(arrow(format!("shop-next-{block}").into(), "chevron-right", false, block, len, n, cx)))
        .into_any_element()
}

fn heading(text: SharedString) -> impl IntoElement {
    div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(text)
}

fn section_head(left: AnyElement, right: Option<AnyElement>) -> impl IntoElement {
    div().h(z(40.)).mb(z(24.)).flex().items_center().justify_between().gap(z(12.)).child(left).children(right)
}

fn secondary(id: SharedString, label: SharedString) -> gpui::Stateful<gpui::Div> {
    crate::ui::controls::button(id, label, crate::ui::controls::BtnKind::Secondary).h(z(40.)).px(z(16.))
}

fn hero(app: &PhoenixApp, home: &Home, block: usize, b: &Block, inner: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Block::Hero { banner, logo, backdrop, skus } = b else { return div().into_any_element() };
    let (top, bottom) = backdrop.map_or((gpui::black(), gpui::black()), |(a, b)| (gpui::rgb(a).into(), gpui::rgb(b).into()));
    div()
        .relative()
        .w_full()
        .h(z(HERO_H))
        .px(z(FEED_PAD_X))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(z(BANNER_H))
                .overflow_hidden()
                .bg(gpui::linear_gradient(180., gpui::linear_color_stop(top, 0.), gpui::linear_color_stop(bottom, 1.)))
                .children(banner.clone().map(|u| img(u.to_string()).size_full().object_fit(gpui::ObjectFit::Cover))),
        )
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(z(HERO_HEADER))
                        .pt(z(19.))
                        .pb(z(16.))
                        .flex()
                        .items_center()
                        .children(logo.clone().map(|u| img(u.to_string()).w(z(200.)).h(z(90.)).mb(z(12.)).object_fit(gpui::ObjectFit::Contain))),
                )
                .child(carousel(app, home, block, skus, inner, cx)),
        )
        .into_any_element()
}

fn marketing(block: usize, tiles: &[Tile]) -> AnyElement {
    let mut row = div().flex().gap(z(24.));
    for (i, t) in tiles.iter().enumerate() {
        row = row.child(
            div()
                .id(SharedString::from(format!("shop-tile-{block}-{i}-{}", t.category)))
                .flex_1()
                .min_w_0()
                .h(z(MARKETING_H))
                .relative()
                .rounded(z(16.))
                .overflow_hidden()
                .child(img(t.asset.to_string()).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
                .child(
                    div()
                        .relative()
                        .size_full()
                        .pt(z(32.))
                        .px(z(24.))
                        .pb(z(24.))
                        .flex()
                        .flex_col()
                        .items_start()
                        .child(div().h(z(16.)))
                        .child(div().h(z(110.4)))
                        .child(
                            div()
                                .id(SharedString::from(format!("shop-tile-go-{block}-{i}")))
                                .h(z(40.))
                                .px(z(16.))
                                .rounded(z(8.))
                                .border_1()
                                .border_color(gpui::rgba(0x97979f33))
                                .bg(gpui::white())
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(gpui::black())
                                .child(crate::t!("shop.take_me_there")),
                        ),
                ),
        );
    }
    div().w_full().px(z(FEED_PAD_X)).py(z(BLOCK_PAD)).child(row).into_any_element()
}

fn shelf(app: &PhoenixApp, home: &Home, block: usize, b: &Block, inner: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Block::Shelf { name, skus, button } = b else { return div().into_any_element() };
    div()
        .w_full()
        .px(z(FEED_PAD_X))
        .py(z(BLOCK_PAD))
        .child(section_head(
            heading(name.clone()).into_any_element(),
            button.clone().map(|t| secondary(format!("shop-shelf-all-{block}").into(), t).into_any_element()),
        ))
        .child(carousel(app, home, block, skus, inner, cx))
        .into_any_element()
}

fn picks_head(block: usize) -> impl IntoElement {
    let left = div().flex().items_center().gap(z(4.)).child(heading(crate::t!("shop.find_style"))).child(glyph("info", 18., tk(idx::ICON_MUTED)));
    let right = div()
        .flex()
        .items_center()
        .gap(z(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(12.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("shop.sort_by")))
                .child(
                    div()
                        .w(z(280.))
                        .h(z(40.))
                        .px(z(12.))
                        .rounded(z(8.))
                        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                        .border_1()
                        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(crate::t!("shop.for_you"))
                        .child(glyph("chevron", 18., tk(idx::ICON_MUTED))),
                ),
        )
        .child(secondary(format!("shop-shuffle-{block}").into(), crate::t!("shop.shuffle")));
    section_head(left.into_any_element(), Some(right.into_any_element()))
}

#[derive(Clone, Copy)]
struct Frame {
    inner: f32,
    offset: f32,
    view_h: f32,
}

fn picks(app: &PhoenixApp, home: &Home, block: usize, skus: &[u64], top: f32, f: Frame, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let cols = fit(f.inner);
    let w = pick_width(f.inner, cols);
    let rows = skus.len().div_ceil(cols);
    let (first, last) = rows_in_view(rows, top + BLOCK_PAD + SECTION_HEAD, f.offset, f.view_h);
    let mut grid = div().flex().flex_col().gap(z(CARD_GAP));
    if first > 0 {
        grid = grid.child(div().flex_none().h(z(first as f32 * ROW_H - CARD_GAP)));
    }
    for r in first..last {
        let mut row = div().flex().gap(z(CARD_GAP));
        for sku in skus.iter().skip(r * cols).take(cols) {
            if let Some(k) = home.cards.get(sku) {
                row = row.child(card(app, k, w, block + 1, cx));
            }
        }
        grid = grid.child(row);
    }
    if last < rows {
        grid = grid.child(div().flex_none().h(z((rows - last) as f32 * ROW_H - CARD_GAP)));
    }
    div().w_full().px(z(FEED_PAD_X)).py(z(BLOCK_PAD)).child(picks_head(block)).child(grid).into_any_element()
}

fn feed(app: &PhoenixApp, home: &Home, f: Frame, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Frame { inner, offset, view_h } = f;
    let mut col = div().w_full().min_w_0().overflow_hidden().pr(z(FEED_GUTTER)).flex().flex_col();
    let mut top = 0.;
    for (i, b) in home.blocks.iter().enumerate() {
        let h = block_height(b, inner);
        let el = if top + h < offset - view_h || top > offset + 2. * view_h {
            div().flex_none().w_full().h(z(h)).into_any_element()
        } else {
            match b {
                Block::Hero { .. } => hero(app, home, i, b, inner, cx),
                Block::Marketing(t) => marketing(i, t),
                Block::Shelf { .. } => shelf(app, home, i, b, inner, cx),
                Block::Picks(s) => picks(app, home, i, s, top, f, cx),
            }
        };
        col = col.child(el);
        top += h;
    }
    col.into_any_element()
}

pub fn render(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure_loaded(app, cx);
    let head = header(window, cx).into_any_element();
    let q = SEARCH.with(|s| s.borrow().clone()).map(|i| i.read(cx).value().to_lowercase()).unwrap_or_default();
    let cards = CARDS.with(|c| c.borrow().clone());
    let home = HOME.with(|h| h.borrow().clone()).filter(|h| !h.blocks.is_empty() && TAB.with(Cell::get) == 0 && q.trim().is_empty());
    let zoom = crate::theme::zoom();
    let width = f32::from(window.viewport_size().width) / zoom - crate::ui::sidebar_resize::width() - 72.;
    let handle = crate::ui::chat::scrollbar::handle("shop-scroll");
    let offset = (-f32::from(handle.offset().y) / zoom).max(0.);
    let view_h = Some(f32::from(handle.bounds().size.height) / zoom).filter(|h| *h > 0.).unwrap_or(f32::from(window.viewport_size().height) / zoom);
    let body: AnyElement = match (home, cards) {
        (Some(h), _) => feed(app, &h, Frame { inner: width - FEED_GUTTER - 2. * FEED_PAD_X, offset, view_h }, cx),
        (None, None) => div().p(z(32.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("common.loading")).into_any_element(),
        (None, Some(v)) => {
            let shown: Vec<&ShopCard> = v.iter().filter(|k| q.is_empty() || k.name.to_lowercase().contains(q.trim())).collect();
            let cols = grid_columns(width);
            let rows = shown.len().div_ceil(cols);
            let (first, last) = visible_rows(rows, offset, view_h);
            let mut grid = div().p(z(GRID_PAD)).flex().flex_col().gap(z(CARD_GAP));
            if first > 0 {
                grid = grid.child(div().flex_none().h(z(first as f32 * ROW_H - CARD_GAP)));
            }
            for r in first..last {
                let mut row = div().flex().gap(z(CARD_GAP));
                for k in shown.iter().skip(r * cols).take(cols) {
                    row = row.child(card(app, k, CARD_SIZE, 0, cx));
                }
                grid = grid.child(row);
            }
            if last < rows {
                grid = grid.child(div().flex_none().h(z((rows - last) as f32 * ROW_H - CARD_GAP)));
            }
            grid.into_any_element()
        }
    };
    div()
        .size_full()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .child(head)
        .child(
            crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin)
                .flex_1()
                .min_h_0()
                .child(div().id("shop-scroll").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("shop-scroll")).child(body))
                .children(crate::ui::chat::scrollbar::bar_for("shop-scroll", crate::ui::chat::scrollbar::Bar::Thin)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{discount, parse};

    #[test]
    fn bundle_names_lead_with_the_bold_count() {
        let (t, b) = super::bundle_label(0, "Lune");
        assert_eq!((t.as_str(), b), ("Lune", 0));
        let (t, b) = super::bundle_label(4, "Pack Lune");
        assert!(t.ends_with(" Pack Lune") && t[..b].contains('4') && !t[..b].ends_with(' '));
    }

    #[test]
    fn only_the_visible_rows_are_built() {
        use super::{grid_columns, visible_rows};
        assert_eq!(grid_columns(1138.), 4);
        assert_eq!(grid_columns(1137.), 3);
        assert_eq!(grid_columns(100.), 1);
        assert_eq!(visible_rows(200, 0., 800.), (0, 4));
        assert_eq!(visible_rows(200, 2760., 800.), (8, 14));
        assert_eq!(visible_rows(3, 0., 800.), (0, 3));
        assert_eq!(visible_rows(0, 500., 800.), (0, 0));
    }

    #[test]
    fn nitro_price_is_tier_four_and_orbs_win_when_affordable() {
        let p = serde_json::json!({"sku_id": "7", "name": "Pack Lune", "type": 1000, "items": [{}, {}, {}],
            "preview_assets": {"bg_static": "https://cdn/bg", "fg_static": "https://cdn/fg"},
            "prices": {
                "0": {"country_prices": {"prices": [{"amount": 1599, "currency": "eur", "exponent": 2}, {"amount": 11000, "currency": "discord_orb", "exponent": 0}]}},
                "4": {"country_prices": {"prices": [{"amount": 1299, "currency": "eur", "exponent": 2}]}}
            }});
        let k = super::card_of(&p).expect("card");
        assert_eq!(k.base, Some((1599, 2, "eur".into())));
        assert_eq!(k.nitro, Some((1299, 2, "eur".into())));
        assert_eq!(k.orbs, Some(11000));
        assert_eq!(k.items, 3);
        assert_eq!(k.bundle_art.as_ref().map(|a| a.0.as_ref()), Some("https://cdn/bg"));
        use super::{Shown, shown_price};
        assert_eq!(shown_price(&k, true, Some(15440)), Some(Shown::Orbs(11000)));
        assert_eq!(shown_price(&k, true, Some(100)), Some(Shown::Fiat { price: (1299, 2, "eur".into()), former: None, nitro: true }));
        assert_eq!(shown_price(&k, false, None), Some(Shown::Fiat { price: (1599, 2, "eur".into()), former: None, nitro: false }));
        let mut sale = k.clone();
        sale.orbs = None;
        sale.sale = Some((974, 2, "eur".into()));
        assert_eq!(shown_price(&sale, true, None), Some(Shown::Fiat { price: (974, 2, "eur".into()), former: Some((1299, 2, "eur".into())), nitro: false }));
        assert_eq!(discount(1299, 974), Some(25));
    }

    #[test]
    fn shop_home_blocks_follow_the_client_feed() {
        use super::{Block, parse_home};
        let prod = |id: &str| serde_json::json!({"sku_id": id, "name": id, "prices": {"0": {"country_prices": {"prices": [{"amount": 599, "currency": "eur", "exponent": 2}]}}}});
        let home = serde_json::json!({"shop_blocks": [
            {"type": 0, "ranked_sku_ids": ["1", "9", "2"], "hero_banner_url": "b", "hero_logo_url": "l",
             "banner_display_config": {"background_style": "linear-gradient(180deg, #000000 0%, #616161 100%)"}},
            {"type": 1, "subblocks": [{"category_sku_id": "50", "asset_url": "a"}, {"category_sku_id": "51", "asset_url": "c"}]},
            {"type": 4, "name": "Pale Reverie", "ranked_sku_ids": ["3"], "show_button": true, "button_text": "Tout voir"},
            {"type": 2, "ranked_sku_ids": ["2"], "sorted_sku_ids": {"recommended": ["3", "1"]}},
            {"type": 99}
        ], "categories": [{"products": [prod("1"), prod("2")]}]});
        let catalog = serde_json::json!({"categories": [{"products": [prod("3")]}]});
        let h = parse_home(&home, &catalog);
        assert_eq!(h.blocks.len(), 4);
        assert_eq!(h.blocks[0], Block::Hero { banner: Some("b".into()), logo: Some("l".into()), backdrop: Some((0, 0x616161)), skus: vec![1, 2] });
        assert!(matches!(&h.blocks[1], Block::Marketing(t) if t.len() == 2 && t[1].category == 51));
        assert!(matches!(&h.blocks[2], Block::Shelf { skus, button: Some(b), .. } if skus == &vec![3] && b.as_ref() == "Tout voir"));
        assert_eq!(h.blocks[3], Block::Picks(vec![3, 1]));
    }

    #[test]
    fn feed_geometry_matches_the_dump() {
        use super::{Block, HERO_H, block_height, carousel_start, fit, pick_width};
        assert_eq!(HERO_H, 528.);
        assert_eq!(fit(1047.4), 3);
        assert_eq!(fit(1074.), 4);
        assert!((pick_width(1047.4, 3) - 329.13).abs() < 0.01);
        assert_eq!(block_height(&Block::Marketing(Vec::new()), 1047.4), 296.);
        assert_eq!(block_height(&Block::Shelf { name: "".into(), skus: vec![], button: None }, 1047.4), 370.);
        assert_eq!(block_height(&Block::Picks((0..12).collect()), 1047.4), 2. * 30. + 64. + 1074.);
        assert_eq!(carousel_start(0, 11, 3, 1), 3);
        assert_eq!(carousel_start(6, 11, 3, 1), 8);
        assert_eq!(carousel_start(8, 11, 3, -1), 5);
        assert_eq!(carousel_start(2, 11, 3, -1), 0);
        assert_eq!(super::rows_in_view(20, 900., 0., 700.), (0, 1));
        assert_eq!(super::rows_in_view(20, 900., 900., 700.), (0, 4));
    }

    #[test]
    fn search_placeholder_follows_the_locale() {
        use crate::i18n::t_in;
        assert_eq!(t_in("fr", super::SEARCH_PLACEHOLDER, &[]).as_ref(), "Rechercher dans la Boutique");
        assert_eq!(t_in("en-US", super::SEARCH_PLACEHOLDER, &[]).as_ref(), "Search the Shop");
        assert_eq!(crate::i18n::messages::mappings()[super::SEARCH_PLACEHOLDER].hash, "arz34K");
    }

    #[test]
    fn discount_rounds() {
        assert_eq!(discount(1299, 974), Some(25));
        assert_eq!(discount(499, 499), None);
        assert_eq!(discount(0, 0), None);
    }

    #[test]
    fn parses_products_with_prices() {
        let v = serde_json::json!({"categories": [{"products": [
            {"sku_id": "1", "name": "A", "type": 1000, "items": [{}, {}],
             "prices": {"0": {"country_prices": {"prices": [{"amount": 1299, "currency": "eur", "exponent": 2}]}}}},
            {"sku_id": "1", "name": "dup"}
        ]}]});
        let c = parse(&v);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].items, 2);
        assert_eq!(c[0].base, Some((1299, 2, "eur".into())));
    }
}
