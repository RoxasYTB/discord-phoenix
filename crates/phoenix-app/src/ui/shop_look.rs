use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ObjectFit, ParentElement, SharedString, Styled, div, img, prelude::*};
use phoenix_api::rest::collectibles::LookSku;

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{context_menu::MenuItem, settings::collectibles::Kind},
};

pub const BUTTON_EXPERIMENT: &str = "2026-11-shop-this-look-full-profile-button";
const PLACEHOLDER_AVATAR: &str = "https://cdn.discordapp.com/embed/avatars/0.png";

thread_local! {
    static LOOKS: RefCell<HashMap<Vec<u64>, Option<Vec<LookSku>>>> = RefCell::new(HashMap::new());
    static BUTTON_EXP: Cell<Option<bool>> = const { Cell::new(None) };
    static EXP_FETCHING: Cell<bool> = const { Cell::new(false) };
}

#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    Hidden,
    Loading(usize),
    Ready(Vec<LookSku>),
}

pub fn skus(app: &PhoenixApp, user: Id) -> Vec<u64> {
    if user == app.me.id {
        return Vec::new();
    }
    app.people.profiles.get(&user).map(|p| p.look_skus.clone()).unwrap_or_default()
}

pub fn state(skus: &[u64]) -> Look {
    if skus.is_empty() {
        return Look::Hidden;
    }
    match LOOKS.with(|l| l.borrow().get(skus).cloned()) {
        Some(Some(v)) => Look::Ready(v),
        _ => Look::Loading(skus.len()),
    }
}

pub fn set_loaded(skus: Vec<u64>, items: Vec<LookSku>) {
    LOOKS.with(|l| l.borrow_mut().insert(skus, Some(items)));
}

pub fn prefetch(app: &PhoenixApp, user: Id, cx: &mut Context<PhoenixApp>) {
    let skus = skus(app, user);
    if skus.is_empty() || LOOKS.with(|l| l.borrow().contains_key(&skus)) {
        return;
    }
    LOOKS.with(|l| l.borrow_mut().insert(skus.clone(), None));
    let fut = app.backend.storefront_skus(skus.clone());
    cx.spawn(async move |this, cx| {
        let items = fut.await.unwrap_or_default();
        let _ = this.update(cx, |_, cx| {
            set_loaded(skus, items);
            cx.notify();
        });
    })
    .detach();
}

pub fn renderable(look: &Look) -> bool {
    match look {
        Look::Hidden => false,
        Look::Loading(_) => true,
        Look::Ready(v) => !v.is_empty(),
    }
}

pub fn set_button_experiment(list: &[(u32, i64)]) {
    BUTTON_EXP.with(|e| e.set(Some(phoenix_api::rest::experiments::bucket(list, BUTTON_EXPERIMENT) == 1)));
}

pub fn button_experiment(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    if let Some(on) = BUTTON_EXP.with(Cell::get) {
        return on;
    }
    if !EXP_FETCHING.with(|f| f.replace(true)) {
        let fut = app.backend.user_experiments();
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |_, cx| {
                match res {
                    Ok(list) => set_button_experiment(&list),
                    Err(_) => BUTTON_EXP.with(|e| e.set(Some(false))),
                }
                cx.notify();
            });
        })
        .detach();
    }
    false
}

pub fn button_experiment_cached() -> bool {
    BUTTON_EXP.with(Cell::get).unwrap_or(false)
}

fn kind(k: u64) -> Option<Kind> {
    Some(match k {
        0 => Kind::Decoration,
        1 => Kind::Effect,
        2 => Kind::Nameplate,
        3 => Kind::Frame,
        _ => return None,
    })
}

pub fn type_label(k: u64) -> Option<SharedString> {
    Some(match kind(k)? {
        Kind::Decoration => crate::t!("shop_look.decoration"),
        Kind::Effect => crate::t!("shop_look.effect"),
        Kind::Nameplate => crate::t!("shop_look.nameplate"),
        Kind::Frame => crate::t!("shop_look.frame"),
    })
}

fn preview(l: &LookSku) -> AnyElement {
    let url = kind(l.kind).and_then(|k| crate::ui::settings::collectibles::item_preview(k, &l.item));
    let r = z(4.);
    match kind(l.kind) {
        Some(Kind::Decoration) => div()
            .relative()
            .size(z(42.))
            .child(img(PLACEHOLDER_AVATAR).absolute().left(z(3.5)).top(z(3.5)).size(z(35.)).rounded_full())
            .children(url.map(|u| img(u).absolute().left_0().top_0().size(z(42.))))
            .into_any_element(),
        Some(Kind::Nameplate) => div()
            .size(z(42.))
            .rounded(r)
            .overflow_hidden()
            .bg(c(tok(idx::MODAL_BACKGROUND)))
            .children(url.map(|u| img(u).size_full().object_fit(ObjectFit::Cover)))
            .into_any_element(),
        Some(Kind::Effect | Kind::Frame) => div()
            .w(z(42.))
            .h(z(52.))
            .rounded(r)
            .overflow_hidden()
            .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
            .children(url.map(|u| img(u).size_full().rounded(r).object_fit(ObjectFit::Cover)))
            .into_any_element(),
        None => div().into_any_element(),
    }
}

fn skeleton(ix: usize) -> AnyElement {
    div()
        .id(("shop-look-skeleton", ix))
        .flex()
        .items_center()
        .gap(z(8.))
        .p(z(8.))
        .child(div().size(z(45.)).flex_none().rounded(z(4.)).bg(c(tok(idx::BACKGROUND_MOD_MUTED))))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(div().h(z(14.)).w_full().rounded(z(4.)).bg(c(tok(idx::BACKGROUND_MOD_MUTED))))
                .child(div().h(z(14.)).w(gpui::relative(0.7)).rounded(z(4.)).bg(c(tok(idx::BACKGROUND_MOD_MUTED)))),
        )
        .into_any_element()
}

pub fn row(e: &gpui::Entity<PhoenixApp>, l: &LookSku, ix: usize) -> AnyElement {
    let shoppable = l.shoppable;
    let e = e.clone();
    div()
        .id(("shop-look-row", ix))
        .flex()
        .items_center()
        .gap(z(8.))
        .p(z(8.))
        .rounded(z(2.))
        .when(shoppable, |d| {
            d.cursor_pointer().hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))).on_click(move |_, _, cx: &mut App| {
                e.update(cx, |a, cx| a.open_shop_this_look(cx))
            })
        })
        .when(!shoppable, |d| d.opacity(0.5).cursor_not_allowed())
        .child(div().flex_none().flex().items_center().justify_center().child(preview(l)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(SharedString::from(l.name.clone())),
                )
                .children(type_label(l.kind).map(|t| {
                    div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::NORMAL).text_color(c(tok(idx::TEXT_SUBTLE))).child(t)
                })),
        )
        .into_any_element()
}

pub fn contents(e: &gpui::Entity<PhoenixApp>, look: &Look) -> Vec<AnyElement> {
    match look {
        Look::Hidden => Vec::new(),
        Look::Loading(n) => (0..*n).map(skeleton).collect(),
        Look::Ready(v) => v.iter().enumerate().map(|(i, l)| row(e, l, i)).collect(),
    }
}

pub fn menu_item(app: &PhoenixApp, e: &gpui::Entity<PhoenixApp>, user: Id) -> Option<MenuItem> {
    let skus = skus(app, user);
    if skus.is_empty() {
        return None;
    }
    if button_experiment_cached() && app.people.modal == Some(user) {
        return None;
    }
    let look = state(&skus);
    let items = contents(e, &look).into_iter().map(MenuItem::Custom).collect();
    Some(MenuItem::submenu(crate::t!("shop_look.title"), items))
}

pub fn button_shown(experiment: bool, look: &Look) -> bool {
    experiment && renderable(look)
}

pub fn button_items(app: &PhoenixApp, e: &gpui::Entity<PhoenixApp>, user: Id) -> Vec<MenuItem> {
    let look = state(&skus(app, user));
    if !renderable(&look) {
        return Vec::new();
    }
    contents(e, &look).into_iter().map(MenuItem::Custom).collect()
}

impl PhoenixApp {
    pub fn open_shop_this_look(&mut self, cx: &mut Context<Self>) {
        crate::ui::extra_menus::close();
        self.people.modal = None;
        self.people.popout = None;
        self.open_home_view(crate::ui::home::data::HomeView::Shop, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sku(id: u64, kind: u64, shoppable: bool) -> LookSku {
        LookSku { sku: id, kind, name: format!("n{id}"), shoppable, item: serde_json::json!({"asset": "a"}) }
    }

    #[test]
    fn look_state_follows_the_fetch() {
        assert_eq!(state(&[]), Look::Hidden);
        assert_eq!(state(&[901, 902]), Look::Loading(2));
        set_loaded(vec![901, 902], vec![sku(901, 0, true)]);
        assert_eq!(state(&[901, 902]), Look::Ready(vec![sku(901, 0, true)]));
        assert!(renderable(&state(&[901, 902])));
        set_loaded(vec![903], Vec::new());
        assert!(!renderable(&state(&[903])));
    }

    #[test]
    fn nothing_to_buy_draws_no_button_or_menu() {
        assert!(!button_shown(true, &Look::Hidden));
        set_loaded(vec![904], Vec::new());
        assert!(!button_shown(true, &state(&[904])));
        assert!(!button_shown(false, &Look::Ready(vec![sku(905, 0, true)])));
        assert!(button_shown(true, &Look::Ready(vec![sku(905, 0, true)])));
        assert!(button_shown(true, &Look::Loading(1)));
    }

    #[test]
    fn rows_are_labelled_by_collectible_type() {
        assert!(type_label(0).is_some() && type_label(1).is_some() && type_label(2).is_some() && type_label(3).is_some());
        assert!(type_label(9).is_none());
    }

    #[test]
    fn button_experiment_reads_bucket_one() {
        let h = phoenix_api::rest::experiments::murmur3(BUTTON_EXPERIMENT);
        set_button_experiment(&[(h, 1)]);
        assert!(button_experiment_cached());
        set_button_experiment(&[(h, 0)]);
        assert!(!button_experiment_cached());
    }
}
