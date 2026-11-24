use std::collections::HashMap;

use gpui::{div, img, prelude::*, Context, Div, FontWeight, Global, Hsla, SharedString};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::z,
    ui::{
        profile_games::{GameWidget, Kind},
        settings::{profile_activity::RecentGame, profile_widgets::AppWidget},
    },
};

#[derive(Clone, Default)]
pub struct Board {
    pub widgets: Vec<AppWidget>,
    pub games: Vec<GameWidget>,
    pub recent: Vec<RecentGame>,
    pub wishlist: Vec<crate::ui::profile_wishlist::WishItem>,
}

impl Board {
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty() && self.games.iter().all(|g| g.games.is_empty())
    }
}

#[derive(Default)]
struct Store(HashMap<Id, Board>);
impl Global for Store {}

pub fn get(app: &PhoenixApp, user: Id, cx: &mut Context<PhoenixApp>) -> Board {
    let s = cx.default_global::<Store>();
    if let Some(b) = s.0.get(&user) {
        return b.clone();
    }
    s.0.insert(user, Board::default());
    let (w, g, r, l) = (app.backend.profile_widgets(user), app.backend.game_widgets(user), app.backend.recent_activity(user), app.backend.wishlist(user));
    cx.spawn(async move |this, cx| {
        let (w, g, r, l) = futures::join!(w, g, r, l);
        let _ = this.update(cx, |_, cx| {
            let b = cx.default_global::<Store>().0.entry(user).or_default();
            b.widgets = w.unwrap_or_default();
            b.games = g.unwrap_or_default();
            b.recent = r.unwrap_or_default();
            b.wishlist = l.unwrap_or_default();
            cx.notify();
        });
    })
    .detach();
    Board::default()
}

pub struct Colors {
    pub strong: Hsla,
    pub card: Hsla,
    pub cover: Hsla,
}

fn title(k: Kind) -> SharedString {
    match k {
        Kind::Favorite => crate::t!("board.favorite_game"),
        Kind::Current => crate::t!("board.current_games"),
        Kind::WantToPlay => crate::t!("board.want_to_play"),
        Kind::Played => crate::t!("board.played_games"),
    }
}

fn cover(g: &crate::ui::profile_games::Game, col: &Colors) -> Div {
    div()
        .w(z(88.))
        .h(z(117.))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(col.cover)
        .children(g.cover.clone().map(|u| img(u).size_full()))
}

thread_local! {
    static MENU: std::cell::Cell<Option<gpui::Point<gpui::Pixels>>> = const { std::cell::Cell::new(None) };
}

pub fn menu() -> Option<gpui::AnyElement> {
    let pos = MENU.with(|m| m.get())?;
    let close: crate::ui::context_menu::Dismiss = std::rc::Rc::new(|w: &mut gpui::Window, _: &mut gpui::App| {
        MENU.with(|m| m.set(None));
        w.refresh();
    });
    let c2 = close.clone();
    let items = vec![crate::ui::context_menu::MenuItem::danger(crate::t!("board.report_widget"), Some("flag".into()), move |_, w, cx| c2(w, cx))];
    Some(crate::ui::context_menu::render(pos, items, close))
}

pub fn more_button(color: Hsla) -> gpui::Stateful<Div> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    div()
        .id(("widget-more", id))
        .on_click(|ev: &gpui::ClickEvent, window, _| {
            MENU.with(|m| m.set(Some(ev.position())));
            window.refresh();
        })
        .size(z(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(20.)).text_color(color))
}

pub fn card(w: &GameWidget, col: &Colors) -> Option<Div> {
    if w.games.is_empty() {
        return None;
    }
    let header = div()
        .h(z(24.))
        .flex()
        .items_center()
        .justify_between()
        .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(title(w.kind)))
        .child(more_button(col.strong));
    let body = if w.kind == Kind::Favorite {
        let g = &w.games[0];
        div().flex().gap(z(16.)).child(cover(g, col)).child(
            div().flex_1().min_w_0().pl(z(7.)).flex().flex_col().justify_center().gap(z(4.)).child(
                div().truncate().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(g.name.clone()),
            ),
        )
    } else {
        div().flex().flex_wrap().gap(z(16.)).children(w.games.iter().map(|g| cover(g, col)))
    };
    Some(div().flex().flex_col().gap(z(8.)).pt(z(12.)).px(z(16.)).pb(z(16.)).rounded(z(16.)).bg(col.card).child(header).child(body))
}
