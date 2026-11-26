use gpui::{AnyElement, Context, FontWeight, IntoElement, ObjectFit, ParentElement, SharedString, Styled, div, img, prelude::*};
use phoenix_api::rest::collectibles::LookSku;
use serde_json::json;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::settings::collectibles::Kind,
};

pub const MODAL_W: f32 = 960.;
pub const MODAL_H: f32 = 650.;
pub const RAIL_W: f32 = 330.;
pub const PREVIEW_AVATAR: f32 = 152.;

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn kind(k: u64) -> Option<Kind> {
    match k {
        0 => Some(Kind::Decoration),
        1 => Some(Kind::Effect),
        2 => Some(Kind::Nameplate),
        3 => Some(Kind::Frame),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Price {
    pub shown: (i64, u32, String),
    pub former: Option<(i64, u32, String)>,
    pub nitro: bool,
}

pub fn price(l: &LookSku, premium: bool) -> Option<Price> {
    let card = crate::ui::shop_page::card_of(&json!({ "sku_id": l.sku.to_string(), "name": l.name, "prices": l.prices }))?;
    let base = card.base.clone();
    match (premium, card.nitro.clone(), base) {
        (true, Some(n), Some(b)) if n.0 < b.0 => Some(Price { shown: n, former: Some(b), nitro: true }),
        (true, Some(n), None) => Some(Price { shown: n, former: None, nitro: true }),
        (_, _, Some(b)) => Some(Price { shown: b, former: None, nitro: false }),
        _ => None,
    }
}

pub fn modal_width(viewport_w: f32) -> f32 {
    if viewport_w > 0. { MODAL_W.min(viewport_w - 32.) } else { MODAL_W }
}

fn fmt(p: &(i64, u32, String)) -> String {
    crate::ui::shop_page::format_price(p.0, p.1, &p.2)
}

fn avatar_with_deco(app: &PhoenixApp, l: &LookSku, size: f32) -> AnyElement {
    let deco = crate::ui::avatar_deco::url(&l.item).or_else(|| crate::ui::settings::collectibles::item_preview(Kind::Decoration, &l.item));
    let d = size * 1.2;
    div()
        .relative()
        .size(z(d))
        .flex_none()
        .child(div().absolute().left(z((d - size) / 2.)).top(z((d - size) / 2.)).child(crate::ui::avatar(Some(&app.me), size)))
        .children(deco.map(|u| img(u).absolute().left_0().top_0().size(z(d))))
        .into_any_element()
}

fn left_preview(app: &PhoenixApp, l: &LookSku) -> AnyElement {
    let url = kind(l.kind).and_then(|k| crate::ui::settings::collectibles::item_preview(k, &l.item));
    match kind(l.kind) {
        Some(Kind::Decoration) => div().flex().items_center().justify_center().child(avatar_with_deco(app, l, PREVIEW_AVATAR)).into_any_element(),
        Some(Kind::Effect) => div()
            .w(z(127.))
            .h(z(246.))
            .rounded(z(12.))
            .overflow_hidden()
            .relative()
            .bg(tk(idx::BACKGROUND_MOD_STRONG))
            .children(url.map(|u| img(u).size_full().object_fit(ObjectFit::Cover)))
            .into_any_element(),
        Some(Kind::Frame) => div().w(z(132.)).h(z(246.)).relative().children(url.map(|u| img(u).size_full().object_fit(ObjectFit::Contain))).into_any_element(),
        Some(Kind::Nameplate) => div()
            .w(z(260.))
            .h(z(56.))
            .rounded(z(8.))
            .overflow_hidden()
            .bg(tk(idx::MODAL_BACKGROUND))
            .children(url.map(|u| img(u).size_full().object_fit(ObjectFit::Cover)))
            .into_any_element(),
        None => div().into_any_element(),
    }
}

fn price_tag(p: &Price) -> AnyElement {
    div()
        .flex()
        .items_end()
        .children(p.former.as_ref().map(|f| {
            div().mr(z(8.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_MUTED)).line_through().child(fmt(f))
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .text_color(tk(idx::TEXT_DEFAULT))
                .when(p.nitro, |d| d.child(gpui::svg().path(crate::icons::path("nitro")).size(z(16.)).flex_none().text_color(tk(idx::TEXT_DEFAULT))))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).child(fmt(&p.shown))),
        )
        .into_any_element()
}

fn overlay_icon_button(id: &'static str, icon: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(32.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(tk(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
        .border_1()
        .border_color(tk(idx::CONTROL_OVERLAY_SECONDARY_BORDER_DEFAULT))
        .hover(|s| s.bg(tk(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)).border_color(tk(idx::CONTROL_OVERLAY_SECONDARY_BORDER_HOVER)))
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(18.)).text_color(tk(idx::CONTROL_OVERLAY_SECONDARY_ICON_DEFAULT)))
}

pub fn render(app: &PhoenixApp, l: &LookSku, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let premium = app.backend.has_premium();
    let p = price(l, premium);
    let stub = crate::t!("shop.preview_only");
    let stub2 = stub.clone();
    let buy_label = match &p {
        Some(p) => crate::t!("shop.buy_for", price = fmt(&p.shown)),
        None => crate::t!("shop.buy"),
    };
    let info = div()
        .flex_none()
        .pb(z(24.))
        .flex()
        .flex_col()
        .child(div().mb(z(4.)).text_size(z(24.)).line_height(z(32.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(SharedString::from(l.name.clone())))
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).child(SharedString::from(l.summary.clone())));
    let pricing = div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(z(13.))
        .children(p.as_ref().map(price_tag))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(
                    crate::ui::controls::button("pdp-buy", buy_label, crate::ui::controls::BtnKind::Primary)
                        .flex_1()
                        .when(!l.shoppable, |d| d.opacity(0.5).cursor_not_allowed())
                        .on_click(cx.listener(move |a, _, _, cx| a.stub(&stub, cx))),
                )
                .child(
                    crate::ui::controls::button("pdp-gift", SharedString::default(), crate::ui::controls::BtnKind::Primary)
                        .flex_none()
                        .w(z(40.))
                        .px(z(0.))
                        .child(gpui::svg().path(crate::icons::path("gift")).size(z(20.)).text_color(gpui::white()))
                        .on_click(cx.listener(move |a, _, _, cx| a.stub(&stub2, cx))),
                ),
        );
    let rail = div()
        .w(z(RAIL_W))
        .flex_none()
        .h_full()
        .p(z(24.))
        .relative()
        .flex()
        .flex_col()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .child(div().flex_1().min_h(z(207.)).overflow_hidden().flex().items_center().justify_center().child(left_preview(app, l)))
        .child(info)
        .child(pricing);
    let close = overlay_icon_button("pdp-close", "close").on_click(cx.listener(|_, _, _, cx| {
        crate::actions::close_modal();
        cx.notify();
    }));
    let big = match kind(l.kind) {
        Some(Kind::Decoration) => avatar_with_deco(app, l, 200.),
        _ => left_preview(app, l),
    };
    let right = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .relative()
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .children(l.background.clone().map(|u| img(SharedString::from(u)).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
        .child(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(big))
        .child(div().absolute().top(z(16.)).right(z(16.)).flex().items_start().gap(z(10.)).child(close));
    let w = modal_width(crate::ui::title_bar::viewport_w_css());
    div()
        .id("shop-pdp")
        .w(z(w))
        .h(z(MODAL_H))
        .rounded(z(12.))
        .overflow_hidden()
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.24), offset: gpui::point(z(0.), z(12.)), blur_radius: z(24.), spread_radius: z(0.), inset: false }])
        .flex()
        .child(rail)
        .child(right)
        .into_any_element()
}

impl PhoenixApp {
    pub fn open_shop_product(&mut self, l: LookSku, cx: &mut Context<Self>) {
        crate::ui::extra_menus::close();
        self.people.modal = None;
        self.people.popout = None;
        crate::actions::open_modal(crate::ui::modals::Modal::ShopProduct(Box::new(l)), None);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(prices: serde_json::Value) -> LookSku {
        LookSku { sku: 5, kind: 0, name: "Bunny".into(), shoppable: true, item: json!({"asset": "a"}), summary: String::new(), background: None, prices }
    }

    fn tier(amount: i64) -> serde_json::Value {
        json!({ "country_prices": { "prices": [{ "amount": amount, "currency": "eur", "exponent": 2 }] } })
    }

    #[test]
    fn nitro_members_see_the_member_price_with_the_full_price_struck() {
        let l = look(json!({ "0": tier(599), "4": tier(499) }));
        let p = price(&l, true).unwrap();
        assert_eq!(p.shown.0, 499);
        assert_eq!(p.former.map(|f| f.0), Some(599));
        assert!(p.nitro);
        let q = price(&l, false).unwrap();
        assert_eq!((q.shown.0, q.former, q.nitro), (599, None, false));
        assert!(price(&look(serde_json::Value::Null), true).is_none());
    }

    #[test]
    fn modal_keeps_the_client_width_when_it_fits() {
        assert_eq!(modal_width(1280.), 960.);
        assert_eq!(modal_width(800.), 768.);
        assert_eq!(modal_width(0.), 960.);
    }
}
