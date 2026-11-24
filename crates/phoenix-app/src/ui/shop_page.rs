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
    pub base: Option<(i64, u32, String)>,
    pub nitro: Option<(i64, u32, String)>,
}

thread_local! {
    static CARDS: RefCell<Option<Vec<ShopCard>>> = const { RefCell::new(None) };
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static TAB: Cell<usize> = const { Cell::new(0) };
    static ORBS: Cell<Option<u64>> = const { Cell::new(None) };
    static SEARCH: RefCell<Option<gpui::Entity<gpui_component::input::InputState>>> = const { RefCell::new(None) };
}

fn price(p: &Value, tier: &str) -> Option<(i64, u32, String)> {
    let x = p.get("prices")?.get(tier)?.get("country_prices")?.get("prices")?.as_array()?.first()?;
    Some((x.get("amount")?.as_i64()?, x.get("exponent").and_then(Value::as_u64).unwrap_or(2) as u32, x.get("currency")?.as_str()?.to_owned()))
}

pub fn parse(v: &Value) -> Vec<ShopCard> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in phoenix_api::rest::collectibles::products(v) {
        let Some(sku) = p.get("sku_id").and_then(Value::as_str).and_then(|s| s.parse().ok()) else { continue };
        if !seen.insert(sku) {
            continue;
        }
        let bundle = p.get("type").and_then(Value::as_u64) == Some(1000);
        out.push(ShopCard {
            sku,
            name: p.get("name").and_then(Value::as_str).unwrap_or_default().to_owned().into(),
            items: if bundle { p.get("items").and_then(Value::as_array).map_or(0, Vec::len) } else { 0 },
            preview: crate::ui::settings::collectibles::product_preview(p),
            base: price(p, "0"),
            nitro: price(p, "2"),
        });
    }
    out
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
    cx.spawn(async move |this, cx| {
        let v = fut.await.unwrap_or(Value::Null);
        let cards = parse(&v);
        CARDS.with(|c| *c.borrow_mut() = Some(cards));
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

fn search_input(window: &mut Window, cx: &mut Context<PhoenixApp>) -> gpui::Entity<gpui_component::input::InputState> {
    if let Some(i) = SEARCH.with(|s| s.borrow().clone()) {
        return i;
    }
    let i = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(crate::t!("shop.search")));
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

fn card(app: &PhoenixApp, k: &ShopCard, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let premium = app.backend.has_premium();
    let mine = if premium { k.nitro.clone().or(k.base.clone()) } else { k.base.clone() };
    let off = match (&k.base, &mine) {
        (Some(b), Some(m)) => discount(b.0, m.0),
        _ => None,
    };
    let fmt = |p: &(i64, u32, String)| format_price(p.0, p.1, &p.2);
    let group: SharedString = format!("shop-card-{}", k.sku).into();
    let name = k.name.clone();
    let stub = crate::t!("shop.preview_only");
    let stub2 = stub.clone();
    div()
        .id(("shop-card", k.sku))
        .group(group.clone())
        .size(z(246.))
        .flex_none()
        .relative()
        .overflow_hidden()
        .rounded(z(8.))
        .border_2()
        .border_color(tk(idx::BORDER_MUTED))
        .hover(|s| s.border_color(tk(idx::BORDER_SUBTLE)))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .cursor_pointer()
        .children(k.preview.clone().map(|u| {
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
        .child(
            div().absolute().left_0().right_0().bottom_0().h(z(146.)).bg(gpui::linear_gradient(
                0.,
                gpui::linear_color_stop(tk(idx::BACKGROUND_BASE_LOWEST), 0.),
                gpui::linear_color_stop(tk(idx::BACKGROUND_BASE_LOWEST).opacity(0.), 1.),
            )),
        )
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
                    .child(crate::t!("shop.discount", n = p)),
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
                        .when(k.items > 0, |d| {
                            d.child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(crate::t!("shop.items", n = k.items)))
                        })
                        .child(div().min_w_0().overflow_hidden().text_ellipsis().child(name)),
                )
                .child(
                    div()
                        .relative()
                        .h(z(46.))
                        .child(
                            div()
                                .absolute()
                                .top(z(4.))
                                .flex()
                                .items_center()
                                .gap(z(8.))
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .group_hover(group.clone(), |s| s.opacity(0.))
                                .when(off.is_some(), |d| {
                                    d.children(k.base.as_ref().map(|b| div().font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).line_through().child(fmt(b))))
                                })
                                .children(mine.as_ref().map(|m| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(z(4.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(tk(idx::TEXT_STRONG))
                                        .when(premium && off.is_some(), |d| d.child(glyph("nitro", 18., tk(idx::TEXT_STRONG))))
                                        .child(fmt(m))
                                })),
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
                                    crate::ui::controls::button(
                                        ("shop-buy", k.sku),
                                        match &mine {
                                            Some(m) => crate::t!("shop.buy_for", price = fmt(m)),
                                            None => crate::t!("shop.buy"),
                                        },
                                        crate::ui::controls::BtnKind::Primary,
                                    )
                                    .flex_1()
                                    .h(z(40.))
                                    .on_click(cx.listener(move |a, _, _, cx| a.stub(&stub, cx))),
                                )
                                .child(
                                    crate::ui::controls::button(("shop-gift", k.sku), SharedString::default(), crate::ui::controls::BtnKind::Primary)
                                        .size(z(40.))
                                        .child(glyph("gift", 20., gpui::white()))
                                        .on_click(cx.listener(move |a, _, _, cx| a.stub(&stub2, cx))),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

pub fn render(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure_loaded(app, cx);
    let head = header(window, cx).into_any_element();
    let q = SEARCH.with(|s| s.borrow().clone()).map(|i| i.read(cx).value().to_lowercase()).unwrap_or_default();
    let cards = CARDS.with(|c| c.borrow().clone());
    let body: AnyElement = match cards {
        None => div().p(z(32.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("common.loading")).into_any_element(),
        Some(v) => {
            let mut grid = div().p(z(32.)).flex().flex_wrap().gap(z(30.));
            for k in v.iter().filter(|k| q.is_empty() || k.name.to_lowercase().contains(q.trim())) {
                grid = grid.child(card(app, k, cx));
            }
            grid.into_any_element()
        }
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .child(head)
        .child(div().id("shop-scroll").flex_1().min_h_0().overflow_y_scroll().child(body))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{discount, parse};

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
