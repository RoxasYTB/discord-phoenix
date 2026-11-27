use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*};
use serde_json::{Value, json};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Interests,
    GameStats,
}

pub const INTERESTS: [(&str, &str); 4] = [
    ("favorite_games", "pe.widget_favorite"),
    ("played_games", "pe.widget_played"),
    ("current_games", "pe.widget_current"),
    ("want_to_play_games", "pe.widget_want"),
];

#[derive(Clone, Copy)]
enum Placeholder {
    Details(u64),
    Grid([u64; 4]),
}

const LEAGUE_OF_LEGENDS: u64 = 1402418696126992445;
const VALORANT: u64 = 700136079562375258;
const PEAK: u64 = 1384276457596911676;
const REPO: u64 = 1344368447928401961;
const BALDURS_GATE_3: u64 = 1137125502985961543;
const MARVEL_RIVALS: u64 = 1314395942253756416;
const WORLD_OF_WARCRAFT: u64 = 356875762940379136;
const RUST: u64 = 1402418594532298837;
const BATTLEFIELD_6: u64 = 1402692356343599254;
const SILKSONG: u64 = 1413176957381771337;

fn placeholder(kind: &str) -> Placeholder {
    match kind {
        "favorite_games" => Placeholder::Details(LEAGUE_OF_LEGENDS),
        "current_games" => Placeholder::Details(VALORANT),
        "played_games" => Placeholder::Grid([PEAK, BATTLEFIELD_6, REPO, BALDURS_GATE_3]),
        _ => Placeholder::Grid([MARVEL_RIVALS, WORLD_OF_WARCRAFT, RUST, SILKSONG]),
    }
}

fn placeholder_ids() -> Vec<u64> {
    let mut out = Vec::new();
    for (kind, _) in INTERESTS {
        match placeholder(kind) {
            Placeholder::Details(id) => out.push(id),
            Placeholder::Grid(ids) => out.extend(ids),
        }
    }
    out
}

thread_local! {
    static OPEN: Cell<Option<Category>> = const { Cell::new(None) };
    static COVERS: RefCell<Option<HashMap<u64, SharedString>>> = const { RefCell::new(None) };
}

pub fn open(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    OPEN.with(|o| o.set(Some(Category::Interests)));
    if COVERS.with_borrow(Option::is_some) {
        return;
    }
    COVERS.with_borrow_mut(|c| *c = Some(HashMap::new()));
    let fut = app.backend.games(placeholder_ids());
    cx.spawn(async move |this, cx| {
        let Ok(games) = fut.await else { return };
        let covers: HashMap<u64, SharedString> = games.into_iter().filter_map(|g| g.cover.map(|u| (g.id, u))).collect();
        COVERS.with_borrow_mut(|c| *c = Some(covers));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

pub fn close() -> bool {
    OPEN.with(|o| o.take().is_some())
}

pub fn is_open() -> Option<Category> {
    OPEN.with(Cell::get)
}

pub fn submission(existing: &[u64], kind: &str) -> Value {
    let mut out: Vec<Value> = existing.iter().map(|id| json!({ "data": { "type": "application", "application_id": id.to_string() } })).collect();
    out.push(json!({ "data": { "type": kind, "games": [] } }));
    Value::Array(out)
}

fn nav_item(id: usize, label: SharedString, on: bool, cat: Category, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .id(("widget-add-nav", id))
        .flex()
        .items_center()
        .gap(z(12.))
        .mb(z(4.))
        .pt(z(8.))
        .pb(z(8.))
        .pl(z(8.))
        .pr(z(12.))
        .rounded(z(8.))
        .text_size(z(16.))
        .line_height(z(24.))
        .text_color(c(tok(if on { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
        .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))).hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG)))))
        .when(!on, |d| d.cursor_pointer().hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))).text_color(c(tok(idx::TEXT_STRONG)))))
        .on_click(cx.listener(move |_, _, _, cx| {
            OPEN.with(|o| o.set(Some(cat)));
            cx.notify();
        }))
        .child(label)
        .into_any_element()
}

fn cover(id: u64) -> gpui::Div {
    let url = COVERS.with_borrow(|c| c.as_ref().and_then(|m| m.get(&id).cloned()));
    div()
        .w_full()
        .min_w_0()
        .aspect_ratio(3. / 4.)
        .opacity(0.2)
        .rounded(z(8.))
        .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.16), offset: gpui::point(z(0.), z(3.)), blur_radius: z(12.), spread_radius: z(0.), inset: false }])
        .when(url.is_none(), |d| d.bg(c(tok(idx::BACKGROUND_MOD_MUTED))))
        .children(url.map(|u| img(u).size_full().rounded(z(8.)).object_fit(gpui::ObjectFit::Fill)))
}

fn placeholder_art(kind: &str) -> gpui::Div {
    let grid = div().w_full().grid().grid_cols(4).gap(z(16.)).items_center();
    match placeholder(kind) {
        Placeholder::Details(id) => {
            let bar = |frac: f32| div().w(gpui::relative(frac)).h(z(20.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)));
            grid.child(cover(id)).child(div().col_span(3).min_w_0().flex().flex_col().justify_center().gap(z(9.)).child(bar(0.6)).child(bar(0.95)))
        }
        Placeholder::Grid(ids) => grid.children(ids.into_iter().map(cover)),
    }
}

fn interest_card(ix: usize, kind: &'static str, key: &'static str, existing: Vec<u64>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .id(("widget-add-interest", ix))
        .relative()
        .w_full()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .on_click(cx.listener(move |a, _, _, cx| {
            close();
            a.blocked_write(crate::actions::guard::GuardedWrite::put_profile_widgets(submission(&existing, kind)), cx);
            cx.notify();
        }))
        .child(div().p(z(16.)).child(placeholder_art(kind)))
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(z(8.))
                .child(gpui::svg().path(crate::icons::path("plus-circle")).size(z(24.)).text_color(c(tok(idx::TEXT_DEFAULT))))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!(key))),
        )
        .into_any_element()
}

pub fn modal(existing: Vec<u64>, viewport_h: f32, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let cat = is_open()?;
    let e = cx.entity();
    let close_action: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        close();
        e.update(cx, |_, cx| cx.notify());
    });
    let close_btn = close_action.clone();
    let pane: AnyElement = match cat {
        Category::Interests => div()
            .py(z(24.))
            .pl(z(24.))
            .pr(z(24.))
            .flex()
            .flex_col()
            .gap(z(12.))
            .children(INTERESTS.iter().enumerate().map(|(i, (kind, key))| interest_card(i, kind, key, existing.clone(), cx)))
            .into_any_element(),
        Category::GameStats => div()
            .pt(z(64.))
            .px(z(48.))
            .text_center()
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(tok(idx::TEXT_SUBTLE)))
            .child(crate::t!("pe.widget_all_added", tabName = crate::t!("pe.widget_cat_stats")))
            .into_any_element(),
    };
    let card = div()
        .id("widget-add-modal")
        .relative()
        .w(z(680.))
        .max_h(gpui::px((viewport_h - f32::from(z(112.))).max(0.)))
        .pt(z(8.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .overflow_hidden()
        .occlude()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .pr(z(76.))
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(crate::t!("pe.widget_add_title")),
        )
        .child(
            div()
                .id("widget-add-close")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click(move |_, w, cx| close_btn(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        )
        .child(
            div()
                .mt(z(4.))
                .h(z(540.))
                .flex_shrink()
                .min_h_0()
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .child(
                    div()
                        .w(z(212.))
                        .flex_none()
                        .py(z(24.))
                        .px(z(16.))
                        .border_r_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .flex()
                        .flex_col()
                        .child(nav_item(0, crate::t!("pe.widget_cat_interests"), cat == Category::Interests, Category::Interests, cx))
                        .child(nav_item(1, crate::t!("pe.widget_cat_stats"), cat == Category::GameStats, Category::GameStats, cx)),
                )
                .child(div().id("widget-add-pane").flex_1().min_w_0().min_h_0().overflow_y_scroll().child(pane)),
        );
    Some(crate::ui::modal::frame("widget-add-backdrop", close_action, card))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_widgets_follow_the_client_order() {
        assert_eq!(INTERESTS.map(|(k, _)| k), ["favorite_games", "played_games", "current_games", "want_to_play_games"]);
        let body = submission(&[42], "favorite_games");
        assert_eq!(body, json!([{ "data": { "type": "application", "application_id": "42" } }, { "data": { "type": "favorite_games", "games": [] } }]));
        let w = crate::actions::guard::GuardedWrite::put_profile_widgets(body);
        assert!(!crate::actions::guard::read_only_write(&w));
    }
}
