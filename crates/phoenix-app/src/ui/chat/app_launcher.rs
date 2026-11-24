//! App launcher popout, opened by the composer's Applications button.
//!
//! Measured on the live client (2026-11-24, 1536x788, Onyx theme):
//! `contentWrapper_` 500x680, right edge on the button's right edge, bottom
//! 8px above its top, --radius-sm, --shadow-border + --shadow-high;
//! `searchBarContainer_` padding 16 around the same 40px search field as the
//! expression picker; `scrollableContent_` padding 0 16 16 with 32px between
//! sections. Section header 24px (padding-top 4, body-md/medium
//! --text-strong), 8px above its content. Two-column sections (`type` 5,
//! "Promoted"): 224x180 cards, gap 12, --card-background-default, radius 12,
//! 126px banner then 8px details (38px icon radius 12, 12px gap, name
//! heading-sm/semibold --text-strong, one line of body-sm --text-subtle).
//! Row sections (`type` 4): 64px rows, padding 12, 40px icon, 1px
//! --background-mod-subtle divider, 12px radius on the outer corners, four
//! rows then "Voir plus" (body-md/medium --text-brand) in the header.
//!
//! Data: `GET /application-directory-static/collections?surface=2&...`
//! (public, read-only; what the client renders as Promoted / Puzzle Games /
//! ...). Opening the launcher in the official client also PATCHes
//! `settings-proto/1` and posts telemetry; Phoenix does neither. Clicking an
//! app only shows a notice: nothing is launched, authorised or installed.

use std::cell::{Cell, RefCell};

use gpui::{AnyElement, Bounds, Context, Entity, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Styled, Window, div, img, prelude::*, px};
use gpui_component::input::{InputEvent, InputState};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok},
};

pub const W: f32 = 500.;
pub const H: f32 = 680.;
/// Rows a row section shows before "Voir plus".
const ROWS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct LauncherApp {
    pub id: u64,
    pub name: SharedString,
    pub description: SharedString,
    pub icon: Option<SharedString>,
    pub cover: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    pub title: SharedString,
    /// Two-column banner cards (`type` 5) instead of rows (`type` 4).
    pub cards: bool,
    pub apps: Vec<LauncherApp>,
}

/// Collections in `position` order, each with its items in `position` order
/// (stable for ties, as the client lists "Daily Word Wheel" before "Daily Word
/// Griddle", both at 1).
pub fn parse(v: &Value) -> Vec<Collection> {
    let mut cols: Vec<(i64, Collection)> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let mut items: Vec<(i64, LauncherApp)> = c
                .get("application_directory_collection_items")?
                .as_array()?
                .iter()
                .filter_map(|i| {
                    let a = i.get("application")?;
                    let id: u64 = a.get("id")?.as_str()?.parse().ok()?;
                    let s = |k: &str| a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
                    let asset = |h: &str, size: u32| SharedString::from(format!("https://cdn.discordapp.com/app-icons/{id}/{h}.png?size={size}"));
                    Some((
                        i.get("position").and_then(Value::as_i64).unwrap_or(0),
                        LauncherApp {
                            id,
                            name: s("name").unwrap_or_default().to_owned().into(),
                            description: s("description").unwrap_or_default().lines().next().unwrap_or_default().to_owned().into(),
                            icon: s("icon").map(|h| asset(h, 160)),
                            cover: s("cover_image").map(|h| asset(h, 512)),
                        },
                    ))
                })
                .collect();
            items.sort_by_key(|(p, _)| *p);
            let title = c.get("title").and_then(Value::as_str)?.to_owned();
            Some((
                c.get("position").and_then(Value::as_i64).unwrap_or(0),
                Collection { title: title.into(), cards: c.get("type").and_then(Value::as_u64) == Some(5), apps: items.into_iter().map(|(_, a)| a).collect() },
            ))
        })
        .filter(|(_, c)| !c.apps.is_empty())
        .collect();
    cols.sort_by_key(|(p, _)| *p);
    cols.into_iter().map(|(_, c)| c).collect()
}

/// Apps whose name contains `q` (case-insensitive), once each, catalogue order.
pub fn search(cols: &[Collection], q: &str) -> Vec<LauncherApp> {
    let q = q.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    cols.iter().flat_map(|c| &c.apps).filter(|a| a.name.to_lowercase().contains(&q) && seen.insert(a.id)).cloned().collect()
}

struct Open {
    input: Entity<InputState>,
    _sub: gpui::Subscription,
}

thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
    static DATA: RefCell<Option<Vec<Collection>>> = const { RefCell::new(None) };
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    /// Row sections expanded by "Voir plus" (by title).
    static EXPANDED: RefCell<Vec<SharedString>> = const { RefCell::new(Vec::new()) };
    /// Window bounds of the composer's Applications button, from the last paint.
    static ANCHOR: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.borrow().is_some())
}

/// Records the Applications button's bounds (painted inside the button).
pub fn anchor_probe() -> impl IntoElement {
    gpui::canvas(|b, _, _| ANCHOR.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
}

pub fn toggle(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if is_open() {
        close(app, window, cx);
        return;
    }
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("launcher.search")));
    let sub = cx.subscribe_in(&input, window, |_, _, ev: &InputEvent, _, cx| {
        if matches!(ev, InputEvent::Change) {
            cx.notify();
        }
    });
    input.update(cx, |s, cx| s.focus(window, cx));
    OPEN.with(|o| *o.borrow_mut() = Some(Open { input, _sub: sub }));
    EXPANDED.with(|e| e.borrow_mut().clear());
    if !REQUESTED.with(|r| r.replace(true)) {
        let fut = app.backend.app_directory_collections(crate::i18n::locale().to_owned());
        cx.spawn(async move |this, cx| {
            let v = fut.await.unwrap_or(Value::Null);
            let cols = parse(&v);
            if cols.is_empty() {
                // Let a later open retry (offline, rate limited).
                REQUESTED.set(false);
            }
            DATA.with(|d| *d.borrow_mut() = Some(cols));
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    cx.notify();
}

/// Closes the popout; true when it was open (Escape handling).
pub fn close(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    let was = OPEN.with(|o| o.borrow_mut().take()).is_some();
    if was {
        app.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }
    was
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

/// App icon: the app's own icon, else its initial on a neutral tile.
fn app_icon(a: &LauncherApp, size: f32) -> AnyElement {
    let frame = div().flex_none().size(px(size)).rounded(px(12.)).overflow_hidden().border_1().border_color(tk(idx::BORDER_SUBTLE));
    match &a.icon {
        Some(u) => frame.child(img(u.clone()).size_full()).into_any_element(),
        None => frame
            .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(size * 0.4))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .child(a.name.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_default())
            .into_any_element(),
    }
}

/// Name (heading-sm/semibold --text-strong) over one line of description
/// (body-sm/normal --text-subtle), both clamped to one line.
fn details(a: &LauncherApp) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(20.))
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_STRONG))
                .truncate()
                .child(a.name.clone()),
        )
        .child(div().h(px(18.)).text_size(px(14.)).line_height(px(18.)).text_color(tk(idx::TEXT_SUBTLE)).truncate().child(a.description.clone()))
}

fn pick(a: &LauncherApp, cx: &mut Context<PhoenixApp>) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    let name = a.name.clone();
    cx.listener(move |app, _, _, cx| app.stub(&name, cx))
}

fn card(a: &LauncherApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let banner = div().w_full().h(px(126.)).flex_none().bg(tk(idx::BACKGROUND_BASE_LOW)).when_some(a.cover.clone(), |d, u| d.child(img(u).size_full().object_fit(gpui::ObjectFit::Cover)));
    div()
        .id(SharedString::from(format!("launcher-card-{}", a.id)))
        .w(px(224.))
        .h(px(180.))
        .rounded(px(12.))
        .overflow_hidden()
        .bg(tk(idx::CARD_BACKGROUND_DEFAULT))
        .cursor_pointer()
        .flex()
        .flex_col()
        .child(banner)
        .child(div().p(px(8.)).flex().items_center().gap(px(12.)).child(app_icon(a, 38.)).child(details(a)))
        .on_click(pick(a, cx))
        .into_any_element()
}

fn rows(apps: &[LauncherApp], cx: &mut Context<PhoenixApp>) -> AnyElement {
    let n = apps.len();
    div()
        .flex()
        .flex_col()
        .children(apps.iter().enumerate().map(|(i, a)| {
            let (first, last) = (i == 0, i + 1 == n);
            div()
                .id(SharedString::from(format!("launcher-row-{}-{i}", a.id)))
                .h(px(64.))
                .p(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .bg(tk(idx::CARD_BACKGROUND_DEFAULT))
                .when(first, |d| d.rounded_tl(px(12.)).rounded_tr(px(12.)))
                .when(last, |d| d.rounded_bl(px(12.)).rounded_br(px(12.)))
                .when(!last, |d| d.border_b_1().border_color(tk(idx::BACKGROUND_MOD_SUBTLE)))
                .cursor_pointer()
                .child(app_icon(a, 40.))
                .child(details(a))
                .on_click(pick(a, cx))
        }))
        .into_any_element()
}

fn header(title: SharedString, more: Option<AnyElement>) -> gpui::Div {
    div()
        .h(px(24.))
        .pt(px(4.))
        .flex()
        .justify_between()
        .text_size(px(16.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .child(div().text_color(tk(idx::TEXT_STRONG)).child(title))
        .children(more)
}

fn section(col: &Collection, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let expanded = EXPANDED.with(|e| e.borrow().contains(&col.title));
    let more = (!col.cards && col.apps.len() > ROWS && !expanded).then(|| {
        let title = col.title.clone();
        div()
            .id(SharedString::from(format!("launcher-more-{}", col.title)))
            .cursor_pointer()
            .text_color(tk(idx::TEXT_BRAND))
            .child(crate::t!("launcher.view_more"))
            .on_click(cx.listener(move |_, _, _, cx| {
                EXPANDED.with(|e| e.borrow_mut().push(title.clone()));
                cx.notify();
            }))
            .into_any_element()
    });
    let body = if col.cards {
        div().flex().flex_wrap().gap(px(12.)).children(col.apps.iter().map(|a| card(a, cx))).into_any_element()
    } else {
        let shown = if expanded { col.apps.len() } else { col.apps.len().min(ROWS) };
        rows(&col.apps[..shown], cx)
    };
    div().flex().flex_col().gap(px(8.)).child(header(col.title.clone(), more)).child(body).into_any_element()
}

pub fn render(_app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let input = OPEN.with(|o| o.borrow().as_ref().map(|o| o.input.clone()))?;
    let vp = window.viewport_size();
    let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
    let (right, bottom) = ANCHOR.get().map_or((vw - 16., vh - 75.), |b| (f32::from(b.right()), f32::from(b.top()) - 8.));
    // Not measured below 788px: keep 8px from the window top and shrink.
    let h = if bottom - 8. < H { (bottom - 8.).max(200.) } else { H };
    let left = (right - W).max(8.);
    let q = input.read(cx).value().to_string();
    let content: Vec<AnyElement> = DATA.with(|d| match d.borrow().as_ref() {
        None => vec![],
        Some(cols) if !q.trim().is_empty() => {
            let hits = search(cols, &q);
            if hits.is_empty() {
                vec![div().pt(px(24.)).text_center().text_size(px(16.)).text_color(tk(idx::TEXT_SUBTLE)).child(crate::t!("launcher.no_results")).into_any_element()]
            } else {
                vec![rows(&hits, cx)]
            }
        }
        Some(cols) => cols.iter().map(|c| section(c, cx)).collect(),
    });
    Some(
        div()
            .id("launcher-layer")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|a, _, w, cx| {
                    close(a, w, cx);
                }),
            )
            .child(
                div()
                    .id("launcher")
                    .absolute()
                    .left(px(left))
                    .top(px(bottom - h))
                    .w(px(W))
                    .h(px(h))
                    .rounded(px(8.))
                    .bg(tk(idx::BACKGROUND_BASE_LOWER))
                    .shadow(crate::ui::inbox::popout_shadow())
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(div().flex_none().p(px(16.)).flex().child(crate::ui::pickers::search_box(&input)))
                    .child(
                        div()
                            .id("launcher-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px(px(16.))
                            .pb(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(32.))
                            .children(content),
                    ),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str, pos: i64) -> Value {
        serde_json::json!({"position": pos, "application": {"id": id, "name": name, "description": format!("{name} desc\nmore"), "icon": "ab", "cover_image": "cd"}})
    }

    #[test]
    fn orders_collections_and_items_by_position() {
        let v = serde_json::json!([
            {"title": "Puzzle Games", "type": 4, "position": 2, "application_directory_collection_items": [item("3", "Hex", 3), item("4", "Wheel", 1), item("5", "Griddle", 1)]},
            {"title": "Promoted", "type": 5, "position": 1, "application_directory_collection_items": [item("1", "Farm", 3), item("2", "Wordle", 1)]},
            {"title": "Empty", "type": 4, "position": 0, "application_directory_collection_items": []},
        ]);
        let cols = parse(&v);
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].title.as_ref(), "Promoted");
        assert!(cols[0].cards && !cols[1].cards);
        let names: Vec<&str> = cols[1].apps.iter().map(|a| a.name.as_ref()).collect();
        assert_eq!(names, ["Wheel", "Griddle", "Hex"]);
        assert_eq!(cols[0].apps[0].description.as_ref(), "Wordle desc");
        assert_eq!(cols[0].apps[0].icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/2/ab.png?size=160"));
    }

    #[test]
    fn search_dedups_across_collections() {
        let a = |id: u64, n: &str| LauncherApp { id, name: n.to_owned().into(), description: "".into(), icon: None, cover: None };
        let cols = vec![
            Collection { title: "A".into(), cards: true, apps: vec![a(1, "Farm Merge Valley"), a(2, "Wordle")] },
            Collection { title: "B".into(), cards: false, apps: vec![a(1, "Farm Merge Valley"), a(3, "Mergest Kingdom")] },
        ];
        let hits: Vec<u64> = search(&cols, " MERGE ").iter().map(|a| a.id).collect();
        assert_eq!(hits, [1, 3]);
    }
}
