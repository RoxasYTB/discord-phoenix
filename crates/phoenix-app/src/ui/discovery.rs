use std::cell::{Cell, RefCell};

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

pub const HEADER_H: f32 = 49.;
pub const HERO_H: f32 = 348.;
pub const CARD_MIN_W: f32 = 280.;
pub const CARD_H: f32 = 322.;
pub const GRID_GAP: f32 = 20.;
pub const GRID_PAD_X: f32 = 24.;
pub const BANNER_H: f32 = 146.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Apps,
    Servers,
}

pub const TABS: [Tab; 2] = [Tab::Apps, Tab::Servers];

#[derive(Clone, Debug, PartialEq)]
pub struct DiscoverableGuild {
    pub id: u64,
    pub name: SharedString,
    pub description: SharedString,
    pub icon: Option<SharedString>,
    pub banner: Option<SharedString>,
    pub online: u64,
    pub members: u64,
    pub verified: bool,
    pub partnered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Home,
    Id(u32),
    Hubs,
}

pub const CATEGORIES: [Category; 7] = [Category::Home, Category::Id(1), Category::Id(2), Category::Id(3), Category::Id(5), Category::Id(6), Category::Hubs];

pub struct CategoryText {
    pub tab: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
    pub section: Option<&'static str>,
}

pub fn category_text(c: Category) -> CategoryText {
    let (tab, title, desc, section) = match c {
        Category::Home => ("discovery.cat.home", "discovery.hero_title", "discovery.hero_desc", Some("discovery.popular")),
        Category::Id(1) => ("discovery.cat.gaming", "discovery.cat.gaming", "discovery.cat_desc.gaming", Some("discovery.cat_popular.gaming")),
        Category::Id(2) => ("discovery.cat.music", "discovery.cat.music", "discovery.cat_desc.music", Some("discovery.cat_popular.music")),
        Category::Id(3) => ("discovery.cat.entertainment", "discovery.cat.entertainment", "discovery.cat_desc.entertainment", Some("discovery.cat_popular.entertainment")),
        Category::Id(5) => ("discovery.cat.science", "discovery.cat.science", "discovery.cat_desc.science", Some("discovery.cat_popular.science")),
        Category::Id(6) => ("discovery.cat.education", "discovery.cat.education", "discovery.cat_desc.education", Some("discovery.cat_popular.education")),
        _ => ("discovery.cat.hubs", "discovery.cat.hubs", "discovery.cat_desc.hubs", None),
    };
    CategoryText { tab, title, desc, section }
}

pub const FEATURED_LIMIT: u32 = 30;
pub const SEARCH_LIMIT: u32 = 12;

pub fn featured_path(category: Option<u32>, offset: u32) -> String {
    match category {
        Some(c) => format!("/discoverable-guilds?categories={c}"),
        None => format!("/discoverable-guilds?offset={offset}&limit={FEATURED_LIMIT}"),
    }
}

pub fn encode(q: &str) -> String {
    let mut out = String::new();
    for b in q.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn search_path(query: &str, offset: u32, category: Option<i64>, language: Option<&str>, with_counts: bool) -> String {
    let mut p = format!("/discoverable-guilds/search?query={}", encode(query));
    if let Some(c) = category.filter(|c| *c >= 0) {
        p.push_str(&format!("&category_id={c}"));
    }
    p.push_str(&format!("&offset={offset}&limit={SEARCH_LIMIT}"));
    if let Some(l) = language {
        p.push_str(&format!("&language_code={}", encode(l)));
    }
    p.push_str(&format!("&with_counts={with_counts}"));
    p
}

pub const EXCLUDED_LANGUAGES: &[&str] = &["en-GB"];

pub fn search_languages() -> Vec<&'static crate::i18n::languages::Language> {
    crate::i18n::languages::LANGUAGES.iter().filter(|l| !EXCLUDED_LANGUAGES.contains(&l.code)).collect()
}

pub fn default_search_language(locale: &str) -> &'static str {
    let list = search_languages();
    list.iter().find(|l| l.code == locale).or(list.first()).map_or("en-US", |l| l.code)
}

fn language_name(code: &str) -> SharedString {
    if code == "en-US" {
        return crate::t!("discovery.lang_en_us");
    }
    crate::i18n::languages::LANGUAGES.iter().find(|l| l.code == code).map_or_else(|| SharedString::from(code.to_owned()), |l| l.native.into())
}

pub fn lurk_path(guild: u64, session_id: &str) -> String {
    let mut p = format!("/guilds/{guild}/members/@me?lurker=true");
    if !session_id.is_empty() {
        p.push_str(&format!("&session_id={}", encode(session_id)));
    }
    p
}

pub fn directory_path_allowed(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    let seg: Vec<&str> = p.trim_start_matches('/').split('/').collect();
    let id = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match seg.as_slice() {
        ["discoverable-guilds"] | ["discoverable-guilds", "search"] | ["discovery", "categories"] => true,
        ["application-directory-static", "collections"] | ["application-directory-static", "categories"] => true,
        ["application-directory-static", "applications", a] | ["application-directory-static", "applications", a, "similar"] => id(a),
        ["applications", a, "public"] | ["applications", a, "application-command-index"] => id(a),
        ["quests", "@me", "claimed"] | ["oauth2", "authorize"] => true,
        _ => false,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchResults {
    pub guilds: Vec<DiscoverableGuild>,
    pub total: u64,
    pub counts: Vec<(i64, u64)>,
}

pub fn parse_search(v: &Value) -> SearchResults {
    let counts = v["categories"]
        .as_array()
        .map(|a| a.iter().filter_map(|c| Some((c["id"].as_str()?.parse().ok()?, c["count"].as_u64().unwrap_or(0)))).collect())
        .unwrap_or_default();
    SearchResults { guilds: parse_guilds(v), total: v["total_count"].as_u64().unwrap_or(0), counts }
}

pub const SEARCH_SIDEBAR_ROWS: usize = 8;

pub fn parse_category_names(v: &Value, locale: &str) -> std::collections::HashMap<i64, SharedString> {
    let list = v.as_array().or_else(|| v.get("categories").and_then(Value::as_array)).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|c| {
            let id = c["id"].as_i64().or_else(|| c["id"].as_str()?.parse().ok())?;
            let n = &c["name"];
            let name = n
                .as_str()
                .or_else(|| n["localizations"][locale].as_str())
                .or_else(|| n["localizations"][locale.split('-').next().unwrap_or(locale)].as_str())
                .or_else(|| n["default"].as_str())?;
            Some((id, SharedString::from(name.to_owned())))
        })
        .collect()
}

#[derive(Default)]
struct Search {
    query: String,
    category: Option<i64>,
    results: SearchResults,
    loading: bool,
    done: bool,
    generation: u64,
}

#[derive(Default)]
struct State {
    category: Option<Category>,
    lists: std::collections::HashMap<Category, Option<Vec<DiscoverableGuild>>>,
    names: Option<std::collections::HashMap<i64, SharedString>>,
    names_requested: bool,
    search: Option<Search>,
    language: Option<&'static str>,
    language_open: bool,
}

thread_local! {
    static OPEN: Cell<Option<Tab>> = const { Cell::new(None) };
    static STATE: RefCell<State> = RefCell::new(State::default());
    static SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
    static INPUT: RefCell<Option<gpui::Entity<gpui_component::input::InputState>>> = const { RefCell::new(None) };
}

pub fn open(tab: Tab) {
    OPEN.with(|o| o.set(Some(tab)));
}

pub fn close() {
    OPEN.with(|o| o.set(None));
}

pub fn current() -> Option<Tab> {
    OPEN.with(|o| o.get())
}

pub fn category() -> Category {
    STATE.with(|s| s.borrow().category.unwrap_or(Category::Home))
}

pub fn set_category(c: Category) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.category = Some(c);
        s.search = None;
    });
    SCROLL.with(|h| h.set_offset(gpui::point(gpui::px(0.), gpui::px(0.))));
}

pub fn search_query() -> Option<String> {
    STATE.with(|s| s.borrow().search.as_ref().map(|x| x.query.clone()))
}

fn cdn(path: String) -> SharedString {
    format!("https://cdn.discordapp.com/{path}").into()
}

pub fn parse_guilds(v: &Value) -> Vec<DiscoverableGuild> {
    let list = v.get("guilds").and_then(Value::as_array).or_else(|| v.as_array()).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|g| {
            let id = g["id"].as_str()?.parse().ok()?;
            let s = |k: &str| g[k].as_str().filter(|x| !x.is_empty()).map(str::to_owned);
            let features: Vec<&str> = g["features"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let ext = |h: &str| if h.starts_with("a_") { "gif" } else { "webp" };
            Some(DiscoverableGuild {
                id,
                name: s("name").unwrap_or_default().into(),
                description: s("description").unwrap_or_default().into(),
                icon: s("icon").map(|h| cdn(format!("icons/{id}/{h}.{}?size=128", ext(&h)))),
                banner: s("discovery_splash").or_else(|| s("splash")).map(|h| cdn(format!("discovery-splashes/{id}/{h}.jpg?size=600"))),
                online: g["approximate_presence_count"].as_u64().unwrap_or(0),
                members: g["approximate_member_count"].as_u64().unwrap_or(0),
                verified: features.contains(&"VERIFIED"),
                partnered: features.contains(&"PARTNERED"),
            })
        })
        .collect()
}

pub fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(ch);
    }
    out
}

pub fn columns(width: f32) -> usize {
    (((width - 2. * GRID_PAD_X + GRID_GAP) / (CARD_MIN_W + GRID_GAP)).floor() as usize).max(1)
}

fn ensure_loaded(app: &PhoenixApp, cat: Category, cx: &mut Context<PhoenixApp>) {
    let go = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.lists.contains_key(&cat) {
            return false;
        }
        s.lists.insert(cat, None);
        true
    });
    if go && cat != Category::Hubs {
        let fut = app.backend.discoverable_guilds(match cat {
            Category::Id(c) => Some(c),
            _ => None,
        }, 0);
        cx.spawn(async move |this, cx| {
            let v = fut.await.unwrap_or(Value::Null);
            STATE.with(|s| s.borrow_mut().lists.insert(cat, Some(parse_guilds(&v))));
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    let names = STATE.with(|s| !std::mem::replace(&mut s.borrow_mut().names_requested, true));
    if names {
        let fut = app.backend.discovery_categories(crate::i18n::locale().to_string());
        cx.spawn(async move |this, cx| {
            let v = fut.await.unwrap_or(Value::Null);
            let map = parse_category_names(&v, crate::i18n::locale());
            STATE.with(|s| s.borrow_mut().names = Some(map));
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
}

pub fn start_search(app: &PhoenixApp, query: String, category: Option<i64>, with_counts: bool, cx: &mut Context<PhoenixApp>) {
    let query = query.trim().to_owned();
    if query.is_empty() {
        STATE.with(|s| s.borrow_mut().search = None);
        cx.notify();
        return;
    }
    let generation = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let generation = s.search.as_ref().map_or(0, |x| x.generation) + 1;
        let counts = s.search.as_ref().filter(|x| x.query == query).map(|x| x.results.counts.clone()).unwrap_or_default();
        let total = s.search.as_ref().filter(|x| x.query == query).map_or(0, |x| x.results.total);
        s.search = Some(Search { query: query.clone(), category, results: SearchResults { guilds: Vec::new(), total, counts }, loading: true, done: false, generation });
        generation
    });
    SCROLL.with(|h| h.set_offset(gpui::point(gpui::px(0.), gpui::px(0.))));
    fetch_search_page(app, query, category, 0, generation, with_counts, cx);
}

fn fetch_search_page(app: &PhoenixApp, query: String, category: Option<i64>, offset: u32, generation: u64, with_counts: bool, cx: &mut Context<PhoenixApp>) {
    let language = STATE.with(|s| s.borrow().language);
    let fut = app.backend.directory_get(search_path(&query, offset, category, language, with_counts));
    cx.spawn(async move |this, cx| {
        let v = fut.await.unwrap_or(Value::Null);
        let page = parse_search(&v);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let Some(st) = s.search.as_mut().filter(|x| x.generation == generation) else { return };
            st.loading = false;
            st.done = page.guilds.len() < SEARCH_LIMIT as usize;
            st.results.guilds.extend(page.guilds);
            if offset == 0 {
                st.results.total = page.total;
                if with_counts {
                    st.results.counts = page.counts;
                }
            }
        });
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn maybe_load_more(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let near = SCROLL.with(|h| h.max_offset().y + h.offset().y < gpui::px(600.));
    if !near {
        return;
    }
    let next = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.search.as_mut()?;
        if st.loading || st.done {
            return None;
        }
        st.loading = true;
        Some((st.query.clone(), st.category, st.results.guilds.len() as u32, st.generation))
    });
    if let Some((q, c, off, g)) = next {
        fetch_search_page(app, q, c, off, g, false, cx);
    }
}

pub fn lurk(app: &mut PhoenixApp, id: u64, cx: &mut Context<PhoenixApp>) {
    let guild = crate::model::Id(id);
    if app.guilds.iter().any(|g| g.id == guild) {
        close();
        app.select_guild(Some(guild), cx);
        cx.notify();
        return;
    }
    let fut = app.backend.lurk_guild(guild);
    cx.spawn(async move |this, cx| {
        if fut.await.is_err() {
            return;
        }
        for _ in 0..40 {
            let found = this.update(cx, |a, cx| {
                if a.guilds.iter().any(|g| g.id == guild) {
                    close();
                    crate::ui::join_flow::mark_lurking(guild);
                    a.select_guild(Some(guild), cx);
                    cx.notify();
                    true
                } else {
                    false
                }
            });
            if !matches!(found, Ok(false)) {
                break;
            }
            cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
        }
    })
    .detach();
}

fn tk(i: usize) -> Hsla {
    c(tok(i))
}

fn text(size: f32, line: f32, weight: FontWeight, color: Hsla, s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(color).child(s.into())
}

fn tab_label(t: Tab) -> SharedString {
    match t {
        Tab::Apps => crate::t!("discovery.apps"),
        Tab::Servers => crate::t!("discovery.servers"),
    }
}

fn tab_icon(t: Tab) -> &'static str {
    match t {
        Tab::Apps => "apps",
        Tab::Servers => "server/server",
    }
}

pub fn sidebar(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let sel = current();
    let mut nav = div().p(z(8.)).flex_1().flex().flex_col().gap(z(2.));
    for t in TABS {
        let on = sel == Some(t);
        let col = if on { tk(idx::INTERACTIVE_TEXT_ACTIVE) } else { tk(idx::INTERACTIVE_TEXT_DEFAULT) };
        nav = nav.child(
            div()
                .id(("discovery-tab", t as usize))
                .h(z(56.))
                .px(z(10.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(12.))
                .cursor_pointer()
                .text_color(col)
                .when(on, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                .when(!on, |d| d.hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(tk(idx::INTERACTIVE_TEXT_HOVER))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    open(t);
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path(tab_icon(t))).flex_none().size(z(24.)).text_color(col))
                .child(text(16., 20., FontWeight::MEDIUM, col, tab_label(t))),
        );
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .child(
            div()
                .flex_none()
                .h(z(HEADER_H))
                .p(z(16.))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .child(text(18., 22., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!("discovery.title"))),
        )
        .child(nav)
        .into_any_element()
}

pub fn verified_check_badge() -> AnyElement {
    verified_badge(false)
}

fn verified_badge(partner: bool) -> AnyElement {
    div()
        .size(z(16.))
        .flex_none()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .child(
            gpui::svg()
                .path(crate::icons::path("guild/flower-star"))
                .absolute()
                .inset_0()
                .size(z(16.))
                .text_color(tk(if partner { idx::BRAND_500 } else { idx::GREEN_360 })),
        )
        .child(gpui::svg().path(crate::icons::path(if partner { "guild/partner" } else { "check-small" })).size(z(10.)).text_color(gpui::white()))
        .into_any_element()
}

fn card(g: &DiscoverableGuild, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let group: SharedString = format!("discovery-card-{}", g.id).into();
    let banner = div().absolute().top_0().left_0().w_full().h(z(BANNER_H)).overflow_hidden().bg(tk(idx::BACKGROUND_SURFACE_HIGH)).children(g.banner.clone().map(|b| img(b).size_full().object_fit(gpui::ObjectFit::Cover)));
    let maximize = div()
        .absolute()
        .top(z(8.))
        .right(z(8.))
        .size(z(32.))
        .rounded(z(4.))
        .bg(gpui::hsla(0., 0., 0., 0.72))
        .flex()
        .items_center()
        .justify_center()
        .opacity(0.)
        .group_hover(group.clone(), |s| s.opacity(1.))
        .child(gpui::svg().path(crate::icons::path("maximize")).size(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)));
    let icon = div()
        .absolute()
        .left(z(12.))
        .top(z(BANNER_H - 32.))
        .size(z(56.))
        .p(z(4.))
        .child(crate::ui::squircle::shape().absolute().top_0().left_0().size_full().text_color(tk(idx::BACKGROUND_BASE_LOW)).group_hover(group.clone(), |s| s.text_color(tk(idx::BACKGROUND_SURFACE_HIGH))))
        .child(match g.icon.clone() {
            Some(u) => crate::ui::squircle::image(u, 48.).size_full().into_any_element(),
            None => div()
                .relative()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(crate::ui::squircle::shape().absolute().top_0().left_0().size_full().text_color(tk(idx::BACKGROUND_SURFACE_HIGH)))
                .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), crate::model::acronym(&g.name)))
                .into_any_element(),
        });
    let dot = |col: Hsla| div().size(z(8.)).rounded_full().flex_none().bg(col);
    let count = |col: Hsla, label: SharedString| div().min_w_0().flex().items_center().gap(z(4.)).child(dot(col)).child(text(12., 16., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), label).overflow_hidden().text_ellipsis().whitespace_nowrap());
    let details = div()
        .flex_1()
        .min_h_0()
        .mt(z(BANNER_H))
        .pt(z(28.))
        .px(z(16.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .when(g.verified || g.partnered, |d| d.child(div().ml(z(-2.)).child(verified_badge(g.partnered && !g.verified))))
                .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), g.name.clone()).min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap()),
        )
        .child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), g.description.clone()).mt(z(4.)).mb(z(16.)).max_h(z(54.)).overflow_hidden())
        .child(
            div()
                .flex_1()
                .flex()
                .items_end()
                .gap(z(16.))
                .child(count(tk(idx::GREEN_360), crate::t!("discovery.online", n = group_digits(g.online))))
                .child(count(tk(idx::PRIMARY_300), crate::t!("discovery.members", n = group_digits(g.members)))),
        );
    let id = g.id;
    div()
        .id(("discovery-card", g.id))
        .group(group)
        .relative()
        .h(z(CARD_H))
        .min_w_0()
        .flex()
        .flex_col()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .cursor_pointer()
        .hover(|s| s.bg(tk(idx::BACKGROUND_SURFACE_HIGH)).border_color(tk(idx::BORDER_STRONG)).shadow_sm())
        .on_click(cx.listener(move |a, _, _, cx| lurk(a, id, cx)))
        .child(banner)
        .child(maximize)
        .child(details)
        .child(icon)
        .into_any_element()
}

pub const STATIC_SPLASH_DARK: &str = "https://discord.com/assets/b00e3f0a7090b61f.svg";
pub const STATIC_SPLASH_LIGHT: &str = "https://discord.com/assets/90bd7fcad4aab8ed.svg";
pub const STATIC_CARD_W: f32 = 200.;
pub const STATIC_CARD_H: f32 = 258.;
const ACRONYM_SIZES: [f32; 7] = [16., 16., 14., 14., 12., 10., 8.];

pub struct StaticCard {
    pub guild: DiscoverableGuild,
    pub online: Option<u64>,
    pub members: Option<u64>,
    pub local_splash: Option<std::path::PathBuf>,
}

pub fn acronym_size(acronym: &str) -> f32 {
    ACRONYM_SIZES.get(acronym.chars().count()).copied().unwrap_or(ACRONYM_SIZES[ACRONYM_SIZES.len() - 1])
}

pub fn static_card(card: &StaticCard, w: f32, h: f32) -> gpui::Div {
    let g = &card.guild;
    let light = crate::theme::is_light();
    let splash = match (&card.local_splash, &g.banner) {
        (Some(p), _) => img(p.clone()),
        (None, Some(u)) => img(u.clone()),
        _ => img(SharedString::new_static(if light { STATIC_SPLASH_LIGHT } else { STATIC_SPLASH_DARK })),
    };
    let acronym = crate::model::acronym(&g.name);
    let inner = match g.icon.clone() {
        Some(u) => crate::ui::squircle::image(u, 40.).size_full().into_any_element(),
        None => div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(crate::ui::squircle::shape().absolute().top_0().left_0().size_full().text_color(tk(idx::BRAND_500)))
            .child(text(acronym_size(&acronym), 18., FontWeight::NORMAL, gpui::white(), acronym))
            .into_any_element(),
    };
    let icon = div()
        .absolute()
        .left(z(12.))
        .bottom(z(-21.))
        .size(z(48.))
        .p(z(4.))
        .child(crate::ui::squircle::shape().absolute().top_0().left_0().size_full().text_color(tk(if light { idx::BACKGROUND_SURFACE_HIGH } else { idx::BACKGROUND_MOD_NORMAL })))
        .child(div().relative().size_full().child(inner));
    let count = |dot: Hsla, label: SharedString, last: bool| div().flex().items_center().when(!last, |d| d.mr(z(16.))).child(div().size(z(8.)).mr(z(4.)).flex_none().rounded_full().bg(dot)).child(text(12., 16., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), label));
    let mut counts = div().flex().flex_none();
    if let Some(n) = card.online {
        counts = counts.child(count(tk(idx::GREEN_360), crate::t!("discovery.card_online", membersOnline = group_digits(n)), card.members.is_none()));
    }
    if let Some(n) = card.members {
        counts = counts.child(count(tk(idx::TEXT_MUTED), crate::t!("discovery.card_members", count = group_digits(n)), true));
    }
    div()
        .relative()
        .w(z(w.max(STATIC_CARD_W)))
        .h(z(h.max(STATIC_CARD_H)))
        .flex_none()
        .flex()
        .flex_col()
        .rounded(z(crate::theme::generated::global::RADIUS_SM))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .shadow(crate::ui::inbox::popout_shadow())
        .child(div().relative().flex_none().h(z(112.)).mb(z(28.)).child(div().absolute().top_0().left_0().size_full().overflow_hidden().child(splash.size_full().object_fit(gpui::ObjectFit::Cover))).child(icon))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .px(z(12.))
                .pb(z(12.))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .when(g.verified || g.partnered, |d| d.child(div().ml(z(-2.)).mr(z(4.)).child(verified_badge(g.partnered && !g.verified))))
                        .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), g.name.clone()).min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap()),
                )
                .child(div().flex_1().min_h_0().child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), g.description.clone()).my(z(8.)).line_clamp(3)))
                .child(counts),
        )
}

fn tab_strip(items: Vec<TabItem>, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let mut tabs = div().h_full().flex().items_center().gap(z(30.));
    for (id, label, on, target) in items {
        let mut t = div()
            .id(id)
            .h_full()
            .relative()
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if on { tk(idx::TEXT_STRONG) } else { tk(idx::TEXT_SUBTLE) })
            .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
            .child(label)
            .when(on, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(z(2.)).bg(tk(idx::BRAND_500))));
        if let Some(c) = target {
            t = t.on_click(cx.listener(move |_, _, _, cx| {
                set_category(c);
                cx.notify();
            }));
        }
        tabs = tabs.child(t);
    }
    tabs
}

fn static_search() -> gpui::Div {
    search_shell().child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_MUTED), crate::t!("discovery.search")))
}

fn search_shell() -> gpui::Div {
    div()
        .flex_none()
        .w(z(240.))
        .h(z(32.))
        .px(z(4.))
        .rounded(z(8.))
        .bg(gpui::hsla(0., 0., 0., 0.12))
        .border_1()
        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(div().flex_none().size(z(22.)).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("search")).size(z(16.)).text_color(tk(idx::TEXT_DEFAULT))))
}

fn search_field(cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let Some(input) = INPUT.with(|i| i.borrow().clone()) else { return static_search() };
    let filled = !input.read(cx).value().is_empty();
    search_shell()
        .when(filled, |d| d.border_color(tk(idx::INPUT_BORDER_ACTIVE)))
        .child(div().flex_1().min_w_0().text_size(z(14.)).child(gpui_component::input::Input::new(&input).appearance(false).bordered(false)))
        .when(filled, |d| {
            d.child(
                div()
                    .id("discovery-search-clear")
                    .flex_none()
                    .size(z(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, w, cx| {
                        clear_search(w, cx);
                    }))
                    .child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))),
            )
        })
}

fn clear_search(window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(i) = INPUT.with(|i| i.borrow().clone()) {
        i.update(cx, |s, cx| s.set_value("", window, cx));
    }
    STATE.with(|s| s.borrow_mut().search = None);
    cx.notify();
}

fn bar(left: gpui::AnyElement, right: Option<gpui::Div>, opaque: bool) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(z(HEADER_H))
        .px(z(16.))
        .flex()
        .items_center()
        .gap(z(20.))
        .when(opaque, |d| d.bg(tk(idx::BACKGROUND_BASE_LOWEST)))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(div().flex_1().min_w_0().h_full().flex().items_center().gap(z(20.)).child(left))
        .children(right)
        .into_any_element()
}

fn header_bar(icon: &'static str, cats: &[&'static str], vw: f32, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let items = cats.iter().enumerate().map(|(n, k)| (gpui::ElementId::from(("discovery-cat", n)), crate::t!(*k), n == 0, None)).collect();
    tab_header(Tab::Apps, icon, items, vw, window, cx)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchBar {
    #[default]
    Default,
    Collapsed,
    Floating,
}

pub const TAB_MEASURE_GAP: f32 = 20.;
pub const SEARCH_BAR_W: f32 = 240.;
pub const SEARCH_ICON_W: f32 = 24.;

pub fn next_search_bar(state: SearchBar, available: f32, empty: bool) -> SearchBar {
    let i = available.round();
    match state {
        SearchBar::Default if i < 20. => {
            if empty {
                SearchBar::Collapsed
            } else {
                SearchBar::Floating
            }
        }
        SearchBar::Collapsed if i > 236. => SearchBar::Default,
        SearchBar::Floating if i > 284. => SearchBar::Default,
        s => s,
    }
}

pub fn tabs_container_width(bar_w: f32, state: SearchBar) -> f32 {
    let search = match state {
        SearchBar::Default => SEARCH_BAR_W + 20.,
        SearchBar::Collapsed => SEARCH_ICON_W + 20.,
        SearchBar::Floating => 0.,
    };
    bar_w - 32. - 24. - 20. - search
}

pub fn measured_sum(widths: &[f32]) -> f32 {
    widths.iter().enumerate().map(|(n, w)| w + if n == 0 { 0. } else { TAB_MEASURE_GAP }).sum()
}

pub fn settle_search_bar(state: SearchBar, bar_w: f32, widths: &[f32], empty: bool) -> SearchBar {
    let sum = measured_sum(widths);
    let mut s = state;
    for _ in 0..3 {
        let n = next_search_bar(s, tabs_container_width(bar_w, s) - sum, empty);
        if n == s {
            break;
        }
        s = n;
    }
    s
}

pub fn last_visible(widths: &[f32], container: f32, overflow: f32) -> usize {
    let (mut used, mut last) = (0., 0);
    for (e, d) in widths.iter().copied().enumerate() {
        if d.is_nan() || d > container {
            continue;
        }
        let tail = if e + 1 == widths.len() { 0. } else { overflow + TAB_MEASURE_GAP };
        if (used + d + tail).round() >= container.round() {
            break;
        }
        last = e;
        used += d + TAB_MEASURE_GAP;
    }
    last
}

thread_local! {
    static SEARCH_BARS: Cell<[SearchBar; 2]> = const { Cell::new([SearchBar::Default; 2]) };
    static MORE_OPEN: Cell<Option<Tab>> = const { Cell::new(None) };
    static MORE_BOUNDS: Cell<Option<gpui::Bounds<gpui::Pixels>>> = const { Cell::new(None) };
}

fn bar_slot(t: Tab) -> usize {
    match t {
        Tab::Apps => 0,
        Tab::Servers => 1,
    }
}

pub fn search_bar(t: Tab) -> SearchBar {
    SEARCH_BARS.with(|b| b.get()[bar_slot(t)])
}

pub fn set_search_bar(t: Tab, s: SearchBar) {
    SEARCH_BARS.with(|b| {
        let mut v = b.get();
        v[bar_slot(t)] = s;
        b.set(v);
    });
}

fn label_w(window: &Window, text: &str, size: f32, weight: FontWeight) -> f32 {
    let mut font = crate::richtext::ui_font();
    font.weight = weight;
    let run = gpui::TextRun { len: text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().layout_line(text, gpui::px(size), &[run], None).width)
}

fn search_empty(cx: &Context<PhoenixApp>) -> bool {
    INPUT.with(|i| i.borrow().as_ref().is_none_or(|i| i.read(cx).value().trim().is_empty()))
}

type TabItem = (gpui::ElementId, SharedString, bool, Option<Category>);

fn tab_header(kind: Tab, icon: &'static str, mut items: Vec<TabItem>, vw: f32, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let widths: Vec<f32> = items.iter().map(|i| label_w(window, &i.1, 16., FontWeight::MEDIUM)).collect();
    let more_label = crate::t!("discovery.more");
    let overflow_w = label_w(window, &more_label, 14., FontWeight::SEMIBOLD) + 4. + 16.;
    let live = kind == Tab::Servers && INPUT.with(|i| i.borrow().is_some());
    let empty = !live || search_empty(cx);
    let state = settle_search_bar(search_bar(kind), vw, &widths, empty);
    set_search_bar(kind, state);
    let last = last_visible(&widths, tabs_container_width(vw, state), overflow_w);
    let hidden: Vec<TabItem> = if last + 1 < items.len() { items.split_off(last + 1) } else { Vec::new() };
    let hidden_selected = hidden.iter().any(|i| i.2);
    let mut tabs = tab_strip(items, cx);
    if !hidden.is_empty() {
        let menu: Vec<(SharedString, bool, Option<Category>)> = hidden.iter().map(|i| (i.1.clone(), i.2, i.3)).collect();
        tabs = tabs.child(more_tab(kind, more_label, hidden_selected, menu, cx));
    }
    let left = div()
        .w_full()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .gap(z(20.))
        .child(gpui::svg().path(crate::icons::path(icon)).flex_none().size(z(24.)).text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)))
        .child(div().flex_1().min_w_0().h_full().overflow_hidden().child(tabs));
    let field = (state != SearchBar::Collapsed).then(|| if live { search_field(cx) } else { static_search() });
    let el = match state {
        SearchBar::Default => bar(left.into_any_element(), field, false),
        SearchBar::Collapsed => bar(left.into_any_element(), Some(collapsed_search(kind, cx)), false),
        SearchBar::Floating => {
            let floating = div().absolute().top_0().bottom_0().right(z(16.)).flex().items_center().children(field);
            div().absolute().top_0().left_0().right_0().h(z(HEADER_H)).child(bar(left.into_any_element(), None, false)).child(floating).into_any_element()
        }
    };
    let menu = (MORE_OPEN.with(|m| m.get()) == Some(kind)).then(more_menu);
    div().absolute().top_0().left_0().right_0().h(z(HEADER_H)).child(el).children(menu.flatten()).into_any_element()
}

fn collapsed_search(kind: Tab, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    div().flex_none().w(z(SEARCH_ICON_W)).flex().justify_end().child(
        div()
            .id("discovery-search-collapsed")
            .size(z(24.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, w, cx| {
                set_search_bar(kind, SearchBar::Floating);
                if kind == Tab::Servers
                    && let Some(i) = INPUT.with(|i| i.borrow().clone())
                {
                    i.update(cx, |s, cx| s.focus(w, cx));
                }
                cx.notify();
            }))
            .child(gpui::svg().path(crate::icons::path("search")).size(z(18.)).text_color(tk(idx::ICON_STRONG))),
    )
}

thread_local! {
    static MORE_ITEMS: RefCell<Vec<(SharedString, bool, Option<Category>)>> = const { RefCell::new(Vec::new()) };
}

fn more_tab(kind: Tab, label: SharedString, selected: bool, menu: Vec<(SharedString, bool, Option<Category>)>, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let open = MORE_OPEN.with(|m| m.get()) == Some(kind);
    MORE_ITEMS.with(|m| *m.borrow_mut() = menu);
    let color = |hover: bool, icon: bool| {
        if hover {
            tk(idx::TEXT_STRONG)
        } else if selected {
            tk(idx::TEXT_BRAND)
        } else if icon {
            tk(idx::INTERACTIVE_TEXT_DEFAULT)
        } else {
            tk(idx::TEXT_DEFAULT)
        }
    };
    let chevron = if open { "chevron-up" } else { "chevron" };
    div()
        .id("discovery-cat-more")
        .group("discovery-cat-more")
        .h_full()
        .relative()
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .child(gpui::canvas(|b, _, _| MORE_BOUNDS.with(|m| m.set(Some(b))), |_, _, _, _| {}).absolute().size_full())
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color(false, false))
                .group_hover("discovery-cat-more", |s| s.text_color(color(true, false)))
                .child(label),
        )
        .child(
            gpui::svg()
                .path(crate::icons::path(chevron))
                .size(z(16.))
                .text_color(color(false, true))
                .group_hover("discovery-cat-more", |s| s.text_color(color(true, true))),
        )
        .on_click(cx.listener(move |_, _, _, cx| {
            MORE_OPEN.with(|m| m.set(if m.get() == Some(kind) { None } else { Some(kind) }));
            cx.notify();
        }))
}

fn more_menu() -> Option<AnyElement> {
    use crate::ui::context_menu::{Dismiss, Handler, MenuItem};
    let b = MORE_BOUNDS.with(|m| m.get())?;
    let items: Vec<MenuItem> = MORE_ITEMS.with(|m| m.borrow().clone())
        .into_iter()
        .map(|(label, on, target)| {
            let on_click: Handler = std::rc::Rc::new(move |_: &gpui::ClickEvent, w: &mut Window, _: &mut gpui::App| {
                if let Some(c) = target {
                    set_category(c);
                }
                MORE_OPEN.with(|m| m.set(None));
                w.refresh();
            });
            MenuItem::Action { label, icon: on.then_some(SharedString::new_static("circle-check")), subtext: None, danger: false, on_click }
        })
        .collect();
    let dismiss: Dismiss = std::rc::Rc::new(|w: &mut Window, _: &mut gpui::App| {
        MORE_OPEN.with(|m| m.set(None));
        w.refresh();
    });
    Some(crate::ui::context_menu::render_anchored(gpui::point(b.left(), b.bottom() + z(8.)), gpui::Anchor::TopLeft, items, dismiss))
}

pub const HERO_STOPS: [(f32, [f32; 4]); 3] = [(0., [47., 114., 193., 1.]), (0.3573, [37., 34., 154., 0.8]), (1., [25., 23., 92., 0.])];
pub const HERO_OPACITY: f32 = 0.7;

pub fn hero_pixel(x: f32, y: f32) -> [u8; 3] {
    let dx = (x + 0.0395) / 1.3066;
    let dy = y / 3.2498;
    let t = (dx * dx + dy * dy).sqrt().clamp(0., 1.);
    let (mut a, mut b) = (HERO_STOPS[0], HERO_STOPS[1]);
    for w in HERO_STOPS.windows(2) {
        if t >= w[0].0 && t <= w[1].0 {
            (a, b) = (w[0], w[1]);
        }
    }
    let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 0. };
    let ch = |i: usize| a.1[i] + (b.1[i] - a.1[i]) * f;
    let alpha = ch(3) * HERO_OPACITY;
    let px = |i: usize| (ch(i) * alpha).round().clamp(0., 255.) as u8;
    [px(0), px(1), px(2)]
}

pub const HERO_LIGHT_STOPS: [(f32, [f32; 3]); 3] = [(0., [239., 248., 255.]), (0.3028, [215., 220., 255.]), (1., [253., 253., 255.])];
pub const HERO_LIGHT_OPACITY: f32 = 0.8;

pub fn hero_pixel_light(x: f32, y: f32, under: [f32; 3]) -> [u8; 3] {
    let dx = (x + 0.0404) / 1.2223;
    let dy = y / 3.5184;
    let t = (dx * dx + dy * dy).sqrt().clamp(0., 1.);
    let (mut a, mut b) = (HERO_LIGHT_STOPS[0], HERO_LIGHT_STOPS[1]);
    for w in HERO_LIGHT_STOPS.windows(2) {
        if t >= w[0].0 && t <= w[1].0 {
            (a, b) = (w[0], w[1]);
        }
    }
    let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 0. };
    let px = |i: usize| ((a.1[i] + (b.1[i] - a.1[i]) * f) * HERO_LIGHT_OPACITY + under[i] * (1. - HERO_LIGHT_OPACITY)).round().clamp(0., 255.) as u8;
    [px(0), px(1), px(2)]
}

thread_local! {
    static HERO_IMG: RefCell<Option<(Option<u32>, std::sync::Arc<gpui::RenderImage>)>> = const { RefCell::new(None) };
}

fn hero_image(light_under: Option<[f32; 3]>) -> std::sync::Arc<gpui::RenderImage> {
    let key = light_under.map(|u| ((u[0] as u32) << 16) | ((u[1] as u32) << 8) | (u[2] as u32));
    if let Some(i) = HERO_IMG.with(|h| h.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, i)| i.clone())) {
        return i;
    }
    let (w, h) = (640u32, 200u32);
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, p) in out.enumerate_pixels_mut() {
        let (fx, fy) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
        let [r, g, b] = match light_under {
            Some(u) => hero_pixel_light(fx, fy, u),
            None => hero_pixel(fx, fy),
        };
        *p = image::Rgba([b, g, r, 255]);
    }
    let arc = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(out)]));
    HERO_IMG.with(|h| *h.borrow_mut() = Some((key, arc.clone())));
    arc
}

fn hero(lines: &[SharedString], desc: SharedString) -> AnyElement {
    let mut title = div().max_w(z(740.)).flex().flex_col();
    for l in lines {
        title = title.child(text(44., 44., FontWeight::BOLD, tk(idx::TEXT_STRONG), l.to_uppercase()).font_family(crate::fonts::display_family(crate::fonts::NITRO)));
    }
    let light = crate::theme::is_light();
    let under = light.then(|| {
        let c = gpui::Rgba::from(tk(idx::BACKGROUND_BASE_LOWEST));
        [c.r * 255., c.g * 255., c.b * 255.]
    });
    div()
        .relative()
        .w_full()
        .h(z(HERO_H))
        .overflow_hidden()
        .when(!light, |d| d.bg(gpui::black()))
        .child(img(gpui::ImageSource::Render(hero_image(under))).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Fill))
        .child(
            div()
                .absolute()
                .top(z(60.))
                .bottom_0()
                .left_0()
                .pl(z(32.))
                .flex()
                .flex_col()
                .justify_center()
                .child(title)
                .child(text(16., 20., FontWeight::MEDIUM, tk(if light { idx::TEXT_STRONG } else { idx::TEXT_MUTED }), desc).mt(z(16.)).max_w(z(600.))),
        )
        .into_any_element()
}

fn ensure_input(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if INPUT.with(|i| i.borrow().is_some()) {
        return;
    }
    let _ = app;
    let q = search_query().unwrap_or_default();
    let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(crate::t!("discovery.search")).default_value(q));
    cx.subscribe(&input, |a: &mut PhoenixApp, i, ev: &gpui_component::input::InputEvent, cx| match ev {
        gpui_component::input::InputEvent::PressEnter { .. } => {
            let q = i.read(cx).value().to_string();
            let cat = STATE.with(|s| s.borrow().search.as_ref().and_then(|x| x.category));
            start_search(a, q, cat, true, cx);
        }
        gpui_component::input::InputEvent::Change => cx.notify(),
        gpui_component::input::InputEvent::Blur if search_bar(Tab::Servers) == SearchBar::Floating && i.read(cx).value().trim().is_empty() => {
            set_search_bar(Tab::Servers, SearchBar::Collapsed);
            cx.notify();
        }
        _ => {}
    })
    .detach();
    INPUT.with(|i| *i.borrow_mut() = Some(input));
}

fn guild_grid(list: Option<&Vec<DiscoverableGuild>>, cols: usize, pad: (f32, f32, f32), gap: f32, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let mut grid = div().pl(z(pad.0)).pr(z(pad.1)).py(z(pad.2)).grid().grid_cols(cols as u16).gap(z(gap));
    match list {
        Some(list) => {
            for g in list.iter() {
                grid = grid.child(card(g, cx));
            }
        }
        None => {
            for _ in 0..6 {
                grid = grid.child(div().h(z(CARD_H)).rounded(z(8.)).bg(tk(idx::BACKGROUND_BASE_LOWEST)));
            }
        }
    }
    grid
}

pub fn page(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if current() == Some(Tab::Apps) {
        if let Some(id) = crate::ui::app_profile::current() {
            return crate::ui::app_profile::page(app, id, window, cx);
        }
        return apps_page(app, window, cx);
    }
    ensure_input(app, window, cx);
    if let Some(i) = INPUT.with(|i| i.borrow().clone()) {
        crate::ui::locale_inputs::keep_placeholder(&i, "discovery.search", window, cx);
    }
    let cat = category();
    ensure_loaded(app, cat, cx);
    let vw = f32::from(window.viewport_size().width) / crate::theme::zoom() - crate::ui::sidebar_resize::width() - 72.;
    if STATE.with(|s| s.borrow().search.is_some()) {
        maybe_load_more(app, cx);
        return search_page(vw, cx);
    }
    let guilds = STATE.with(|s| s.borrow().lists.get(&cat).cloned().flatten());
    let cols = columns(vw);
    let texts = category_text(cat);
    let mut body = div().id("discovery-scroll").size_full().overflow_y_scroll().pr(z(8.));
    body = SCROLL.with(|s| body.track_scroll(s));
    body = body.child(hero(&[crate::t!(texts.title)], crate::t!(texts.desc)));
    if let Some(sec) = texts.section {
        body = body
            .child(div().pt(z(24.)).pl(z(32.)).h(z(48.)).child(text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!(sec))))
            .child(guild_grid(guilds.as_ref(), cols, (GRID_PAD_X, GRID_PAD_X, 20.), GRID_GAP, cx));
    }
    let items = CATEGORIES.iter().enumerate().map(|(n, c)| (gpui::ElementId::from(("discovery-cat", n)), crate::t!(category_text(*c).tab), *c == cat, Some(*c))).collect();
    let header = tab_header(Tab::Servers, "server/server", items, vw, window, cx);
    div().size_full().relative().bg(tk(idx::BACKGROUND_BASE_LOWEST)).child(body).child(header).into_any_element()
}

pub fn search_columns(width: f32) -> usize {
    let inner = width - 8. - SEARCH_SIDEBAR_W - 32. - 16.;
    (((inner + 16.) / (SEARCH_CARD_MIN_W + 16.)).floor() as usize).max(1)
}

pub const SEARCH_SIDEBAR_W: f32 = 178.;

fn language_select(query: String, category: Option<i64>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (chosen, open) = STATE.with(|s| {
        let s = s.borrow();
        (s.language, s.language_open)
    });
    let current = chosen.unwrap_or_else(|| default_search_language(crate::i18n::locale()));
    let list = search_languages();
    let selected = list.iter().position(|l| l.code == current).unwrap_or(0);
    let options: Vec<SharedString> = list.iter().map(|l| language_name(l.code)).collect();
    let toggle: std::rc::Rc<dyn Fn(&mut gpui::App)> = std::rc::Rc::new(|_| STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.language_open = !s.language_open;
    }));
    let e = cx.entity();
    let pick: crate::ui::modals::settings_form::OnPick = std::rc::Rc::new(move |k, _, cx| {
        let Some(code) = search_languages().get(k).map(|l| l.code) else { return };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.language = Some(code);
            s.language_open = false;
        });
        let q = query.clone();
        e.update(cx, |a, cx| start_search(a, q, category, false, cx));
    });
    crate::ui::modals::settings_form::dropdown("discovery-search-lang", options, selected, open, toggle, pick, cx)
}
pub const SEARCH_CARD_MIN_W: f32 = 260.;

fn search_page(vw: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (query, cat, res, loading) = STATE.with(|s| {
        let s = s.borrow();
        let st = s.search.as_ref().expect("search");
        (st.query.clone(), st.category, st.results.clone(), st.loading)
    });
    let names = STATE.with(|s| s.borrow().names.clone()).unwrap_or_default();
    let cols = search_columns(vw);
    let grid = guild_grid(if res.guilds.is_empty() && loading { None } else { Some(&res.guilds) }, cols, (32., 16., 32.), 16., cx).pr(z(SEARCH_SIDEBAR_W + 16.));
    let mut cats = div().flex().flex_col().gap(z(2.));
    for (n, (id, count)) in res.counts.iter().take(SEARCH_SIDEBAR_ROWS).enumerate() {
        let id = *id;
        let on = cat.unwrap_or(-1) == id;
        let label: SharedString = if id < 0 { crate::t!("discovery.search_all") } else { names.get(&id).cloned().unwrap_or_default() };
        let q = query.clone();
        cats = cats.child(
            div()
                .id(("discovery-search-cat", n))
                .p(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .when(on, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                .when(!on, |d| d.hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(cx.listener(move |a, _, _, cx| start_search(a, q.clone(), (id >= 0).then_some(id), false, cx)))
                .child(text(14., 18., FontWeight::MEDIUM, tk(idx::TEXT_STRONG), label).flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap())
                .child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_MUTED), group_digits(*count)).ml(z(8.))),
        );
    }
    let lang = div()
        .h(z(50.))
        .mb(z(18.))
        .flex()
        .items_center()
        .justify_end()
        .child(div().w_full().child(language_select(query.clone(), cat, cx)));
    let sidebar = div().absolute().top_0().right_0().w(z(SEARCH_SIDEBAR_W)).pr(z(8.)).child(lang).child(cats);
    let mut body = div().id("discovery-scroll").size_full().overflow_y_scroll().pt(z(HEADER_H));
    body = SCROLL.with(|s| body.track_scroll(s));
    body = body.child(div().relative().w_full().child(grid).child(sidebar));
    let back = div()
        .id("discovery-search-back")
        .flex_none()
        .size(z(32.))
        .rounded(z(4.))
        .bg(gpui::hsla(240. / 360., 0.04, 0.61, 0.12))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(cx.listener(|_, _, w, cx| clear_search(w, cx)))
        .child(gpui::svg().path(crate::icons::path("nav/back")).size(z(18.)).text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)));
    let left = div()
        .h_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap(z(20.))
        .child(back)
        .child(text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!("discovery.search_results", query = query)).min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap());
    div().size_full().relative().bg(tk(idx::BACKGROUND_BASE_LOW)).child(body).child(bar(left.into_any_element(), Some(search_field(cx)), true)).into_any_element()
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectoryApp {
    pub id: u64,
    pub name: SharedString,
    pub description: SharedString,
    pub icon: Option<SharedString>,
    pub banner: Option<SharedString>,
    pub category: Option<SharedString>,
    pub promoted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    pub title: SharedString,
    pub apps: Vec<DirectoryApp>,
}

pub const PROMOTED_FLAG: u64 = 1 << 29;

pub fn parse_collections(v: &Value) -> Vec<Collection> {
    let mut list: Vec<&Value> = v.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    list.sort_by_key(|c| c["position"].as_i64().unwrap_or(i64::MAX));
    list.into_iter()
        .filter_map(|c| {
            let mut items: Vec<&Value> = c["application_directory_collection_items"].as_array()?.iter().filter(|i| i["type"].as_u64() == Some(1)).collect();
            items.sort_by_key(|i| i["position"].as_i64().unwrap_or(i64::MAX));
            let apps: Vec<DirectoryApp> = items
                .into_iter()
                .filter_map(|i| {
                    let a = &i["application"];
                    let id: u64 = a["id"].as_str()?.parse().ok()?;
                    let s = |v: &Value| v.as_str().filter(|x| !x.is_empty()).map(str::to_owned);
                    let bot = &a["bot"];
                    let banner = match (s(&bot["id"]), s(&bot["banner"])) {
                        (Some(b), Some(h)) => Some(cdn(format!("banners/{b}/{h}.png?size=600"))),
                        _ => s(&a["cover_image"]).map(|h| cdn(format!("app-icons/{id}/{h}.png?size=600"))),
                    };
                    Some(DirectoryApp {
                        id,
                        name: s(&a["name"]).unwrap_or_default().into(),
                        description: s(&a["description"]).unwrap_or_default().into(),
                        icon: s(&a["icon"]).map(|h| cdn(format!("app-icons/{id}/{h}.png?size=64"))),
                        banner,
                        category: a["categories"].get(0).and_then(|c| s(&c["name"])).map(Into::into),
                        promoted: a["flags"].as_u64().unwrap_or(0) & PROMOTED_FLAG != 0,
                    })
                })
                .collect();
            (!apps.is_empty()).then(|| Collection { title: s_of(&c["title"]), apps })
        })
        .collect()
}

fn s_of(v: &Value) -> SharedString {
    v.as_str().unwrap_or_default().to_owned().into()
}

pub fn app_columns(window_w: f32) -> usize {
    if window_w >= 1200. {
        4
    } else if window_w >= 800. {
        2
    } else {
        1
    }
}

thread_local! {
    static APPS: RefCell<(bool, Option<Vec<Collection>>)> = const { RefCell::new((false, None)) };
}

fn ensure_apps(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if APPS.with(|a| std::mem::replace(&mut a.borrow_mut().0, true)) {
        return;
    }
    let fut = app.backend.app_directory_collections(crate::i18n::locale().to_string());
    cx.spawn(async move |this, cx| {
        let v = fut.await.unwrap_or(Value::Null);
        APPS.with(|a| a.borrow_mut().1 = Some(parse_collections(&v)));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn app_card(a: &DirectoryApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let banner = div()
        .relative()
        .w_full()
        .h(z(140.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .children(a.banner.clone().map(|b| img(b).size_full().object_fit(gpui::ObjectFit::Cover)));
    let icon = div()
        .absolute()
        .left(z(12.))
        .top(z(140. - 28.))
        .size(z(56.))
        .p(z(4.))
        .rounded(z(12.))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .child(match a.icon.clone() {
            Some(u) => img(u).size_full().rounded(z(8.)).into_any_element(),
            None => div().size_full().rounded(z(8.)).bg(tk(idx::BACKGROUND_BASE_LOWEST)).into_any_element(),
        });
    let mut info = div().mt(z(4.)).flex().gap(z(4.));
    let has_info = a.category.is_some() || a.promoted;
    if let Some(c) = a.category.clone() {
        info = info.child(text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_DEFAULT), c));
    }
    if a.category.is_some() && a.promoted {
        info = info.child(text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_SUBTLE), "\u{2022}"));
    }
    if a.promoted {
        info = info.child(text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_SUBTLE), crate::t!("discovery.promoted")));
    }
    let id = a.id;
    div()
        .id(("discovery-app", a.id))
        .on_click(cx.listener(move |_, _, _, cx| {
            crate::ui::app_profile::open(id);
            cx.notify();
        }))
        .relative()
        .min_w_0()
        .h(z(275.))
        .flex()
        .flex_col()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .cursor_pointer()
        .hover(|s| s.bg(tk(idx::BACKGROUND_SURFACE_HIGH)).border_color(tk(idx::BORDER_STRONG)).shadow_sm())
        .child(banner)
        .child(div().h(z(28.)))
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .pt(z(4.))
                .px(z(16.))
                .pb(z(16.))
                .flex()
                .flex_col()
                .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), a.name.clone()).overflow_hidden().text_ellipsis().whitespace_nowrap())
                .when(has_info, |d| d.child(info))
                .child(text(14., 18., FontWeight::MEDIUM, tk(idx::TEXT_DEFAULT), a.description.clone()).mt(z(8.)).max_h(z(36.)).overflow_hidden()),
        )
        .into_any_element()
}

fn apps_page(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure_apps(app, cx);
    let cols = app_columns(f32::from(window.viewport_size().width) / crate::theme::zoom());
    let vw = f32::from(window.viewport_size().width) / crate::theme::zoom() - crate::ui::sidebar_resize::width() - 72.;
    let colls = APPS.with(|a| a.borrow().1.clone());
    let mut body = div().flex().flex_col().pb(z(16.));
    for (n, c) in colls.unwrap_or_default().iter().enumerate() {
        body = body.child(div().pt(z(if n == 0 { 24. } else { 16. })).pl(z(32.)).child(text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), c.title.clone())));
        let mut grid = div().px(z(32.)).py(z(16.)).grid().grid_cols(cols as u16).gap(z(16.));
        for a in &c.apps {
            grid = grid.child(app_card(a, cx));
        }
        body = body.child(grid);
    }
    let scroll = APP_SCROLL.with(|s| {
        div()
            .id("discovery-apps-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(s)
            .pr(z(8.))
            .child(hero(&[crate::t!("discovery.apps_title1"), crate::t!("discovery.apps_title2")], crate::t!("discovery.apps_desc")))
            .child(body)
    });
    let cats = ["discovery.apps_cat.featured", "discovery.apps_cat.games", "discovery.apps_cat.entertainment", "discovery.apps_cat.moderation", "discovery.apps_cat.social", "discovery.apps_cat.utilities"];
    div().size_full().relative().bg(tk(idx::BACKGROUND_BASE_LOWEST)).child(scroll).child(header_bar("apps", &cats, vw, window, cx)).into_any_element()
}

thread_local! {
    static APP_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_discoverable_guilds() {
        let v = json!({"total": 2, "guilds": [
            {"id": "11", "name": "A", "description": "d", "icon": "ab", "discovery_splash": "sp", "splash": "x", "features": ["VERIFIED"], "approximate_presence_count": 941116, "approximate_member_count": 4625125},
            {"id": "12", "name": "B", "description": null, "icon": null, "splash": "s2", "features": ["PARTNERED"]}
        ]});
        let g = parse_guilds(&v);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].icon.as_deref(), Some("https://cdn.discordapp.com/icons/11/ab.webp?size=128"));
        assert_eq!(g[0].banner.as_deref(), Some("https://cdn.discordapp.com/discovery-splashes/11/sp.jpg?size=600"));
        assert!(g[0].verified && !g[0].partnered);
        assert_eq!(g[1].banner.as_deref(), Some("https://cdn.discordapp.com/discovery-splashes/12/s2.jpg?size=600"));
        assert!(g[1].partnered && g[1].icon.is_none());
        assert_eq!(group_digits(941116), "941\u{202f}116");
        assert_eq!(group_digits(18149845), "18\u{202f}149\u{202f}845");
        assert_eq!(group_digits(12), "12");
    }

    #[test]
    fn parses_app_directory_collections() {
        let v = json!([
            {"title": "Puzzle Games", "position": 2, "application_directory_collection_items": [
                {"type": 1, "position": 1, "application": {"id": "7", "name": "Wordle", "icon": "ic", "description": "d", "flags": 11665410, "categories": [{"id": 6, "name": "Jeux"}], "cover_image": "cv", "bot": {"id": "7", "banner": null}}}
            ]},
            {"title": "Promoted Games", "position": 1, "application_directory_collection_items": [
                {"type": 1, "position": 2, "application": {"id": "9", "name": "Werwulf", "flags": 537002498, "categories": [], "bot": {"id": "9", "banner": "bn"}}},
                {"type": 2, "position": 1, "application": {"id": "8", "name": "skip"}},
                {"type": 1, "position": 1, "application": {"id": "5", "name": "EXOPLANET", "flags": 537002498, "categories": [{"name": "Jeux"}]}}
            ]},
            {"title": "Empty", "position": 3, "application_directory_collection_items": []}
        ]);
        let c = parse_collections(&v);
        assert_eq!(c.iter().map(|x| x.title.as_ref()).collect::<Vec<_>>(), ["Promoted Games", "Puzzle Games"]);
        assert_eq!(c[0].apps.iter().map(|a| a.name.as_ref()).collect::<Vec<_>>(), ["EXOPLANET", "Werwulf"]);
        assert!(c[0].apps[1].promoted && c[0].apps[1].category.is_none());
        assert_eq!(c[0].apps[1].banner.as_deref(), Some("https://cdn.discordapp.com/banners/9/bn.png?size=600"));
        let w = &c[1].apps[0];
        assert!(!w.promoted);
        assert_eq!(w.category.as_deref(), Some("Jeux"));
        assert_eq!(w.banner.as_deref(), Some("https://cdn.discordapp.com/app-icons/7/cv.png?size=600"));
        assert_eq!(w.icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/7/ic.png?size=64"));
        assert_eq!((app_columns(1536.), app_columns(1000.), app_columns(700.)), (4, 2, 1));
    }

    #[test]
    fn requests_match_the_client() {
        assert_eq!(featured_path(None, 0), "/discoverable-guilds?offset=0&limit=30");
        assert_eq!(featured_path(Some(1), 0), "/discoverable-guilds?categories=1");
        assert_eq!(search_path("minecraft", 0, None, None, true), "/discoverable-guilds/search?query=minecraft&offset=0&limit=12&with_counts=true");
        assert_eq!(search_path("a b", 12, Some(1), Some("fr"), false), "/discoverable-guilds/search?query=a%20b&category_id=1&offset=12&limit=12&language_code=fr&with_counts=false");
        assert_eq!(search_path("x", 0, Some(-1), None, true), "/discoverable-guilds/search?query=x&offset=0&limit=12&with_counts=true");
        assert_eq!(default_search_language("fr"), "fr");
        assert_eq!(default_search_language("en-GB"), "da");
        assert!(!search_languages().iter().any(|l| l.code == "en-GB"));
        assert_eq!(lurk_path(42, "abc"), "/guilds/42/members/@me?lurker=true&session_id=abc");
        assert_eq!(lurk_path(42, ""), "/guilds/42/members/@me?lurker=true");
        assert!(directory_path_allowed("/discoverable-guilds/search?query=x"));
        assert!(directory_path_allowed("/application-directory-static/applications/123/similar?locale=fr"));
        assert!(directory_path_allowed("/quests/@me/claimed"));
        assert!(!directory_path_allowed("/application-directory-static/applications/abc"));
        assert!(!directory_path_allowed("/users/@me/settings-proto/1"));
        assert!(!directory_path_allowed("/guilds/1/members/@me"));
        assert_eq!(CATEGORIES.len(), 7);
        assert_eq!(category_text(Category::Id(1)).section, Some("discovery.cat_popular.gaming"));
        assert!(category_text(Category::Hubs).section.is_none());
    }

    #[test]
    fn category_names_accept_plain_and_localized_forms() {
        let a = parse_category_names(&json!([{"id": 1, "name": "Gaming"}, {"id": "14", "name": {"default": "General Chatting", "localizations": {"fr": "Discussion générale"}}}]), "fr");
        assert_eq!(a.get(&1).map(|x| x.as_ref()), Some("Gaming"));
        assert_eq!(a.get(&14).map(|x| x.as_ref()), Some("Discussion générale"));
    }

    #[test]
    fn parses_search_counts() {
        let v = json!({"guilds": [{"id": "1", "name": "MINECRAFT"}], "total_count": 7796, "categories": [{"id": "-1", "count": 7796}, {"id": "1", "count": 7480}]});
        let r = parse_search(&v);
        assert_eq!((r.guilds.len(), r.total), (1, 7796));
        assert_eq!(r.counts, vec![(-1, 7796), (1, 7480)]);
    }

    #[test]
    fn hero_gradient_follows_the_css_stops() {
        let a = hero_pixel(0., 0.);
        assert!(a[2] > a[0] && a[2] > 120 && a[2] < 140);
        let far = hero_pixel(1., 1.);
        assert!(far.iter().all(|c| *c < 30));
        let mid = hero_pixel(0.45, 0.);
        assert!(mid[2] > mid[1] && mid[0] < 40);
    }

    #[test]
    fn light_hero_gradient_follows_the_light_theme_css() {
        let white = [255., 255., 255.];
        let a = hero_pixel_light(0., 0., white);
        assert!(a[2] == 255 && a[0] > 235);
        let far = hero_pixel_light(1., 1., white);
        assert!(far.iter().all(|c| *c > 245));
        let ring = hero_pixel_light(0.33, 0., white);
        assert!(ring[0] < 230 && ring[2] == 255);
    }

    #[test]
    fn three_columns_in_the_measured_width() {
        assert_eq!(columns(1111.), 3);
        assert_eq!(columns(760.), 2);
        assert_eq!(columns(300.), 1);
    }

    #[test]
    fn static_card_acronym_shrinks_like_the_client() {
        assert_eq!(acronym_size(""), 16.);
        assert_eq!(acronym_size("AS"), 14.);
        assert_eq!(acronym_size("ABCDE"), 10.);
        assert_eq!(acronym_size("ABCDEFGHIJ"), 8.);
    }

    #[test]
    fn search_bar_follows_the_tabs_available_width() {
        use SearchBar::*;
        assert_eq!(next_search_bar(Default, 19.4, true), Collapsed);
        assert_eq!(next_search_bar(Default, 19.4, false), Floating);
        assert_eq!(next_search_bar(Default, 20., true), Default);
        assert_eq!(next_search_bar(Collapsed, 236., true), Collapsed);
        assert_eq!(next_search_bar(Collapsed, 237., true), Default);
        assert_eq!(next_search_bar(Floating, 284., false), Floating);
        assert_eq!(next_search_bar(Floating, 285., false), Default);
        let w = [51., 53., 59., 104., 156., 70., 107.];
        assert_eq!(settle_search_bar(Default, 1600., &w, true), Default);
        assert_eq!(settle_search_bar(Default, 628., &w, true), Collapsed);
        assert_eq!(settle_search_bar(Collapsed, 1000., &w, true), Collapsed);
        assert_eq!(settle_search_bar(Collapsed, 1300., &w, true), Default);
    }

    #[test]
    fn overflowing_tabs_move_into_more() {
        let w = [51., 53., 59., 104., 156., 70., 107.];
        let all = measured_sum(&w);
        assert_eq!(last_visible(&w, all + 1., 47.), 6);
        assert_eq!(last_visible(&w, all, 47.), 5);
        assert_eq!(last_visible(&w, 400., 47.), 3);
        assert_eq!(last_visible(&w, 10., 47.), 0);
    }
}
