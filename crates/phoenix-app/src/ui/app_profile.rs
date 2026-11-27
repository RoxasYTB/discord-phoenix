use std::cell::RefCell;

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*, rgb};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

pub const BANNER_H: f32 = 246.;
pub const AVATAR: f32 = 104.;
pub const SIDE_W: f32 = 240.;
pub const COL_GAP: f32 = 48.;
pub const PAD_X: f32 = 32.;
pub const OVERVIEW_COLLAPSED_H: f32 = 380.;
pub const EMBEDDED_FLAG: u64 = 1 << 17;
pub const MESSAGE_CONTENT_FLAGS: u64 = (1 << 18) | (1 << 19);
pub const GUILD_MEMBERS_FLAGS: u64 = (1 << 14) | (1 << 15);

pub const PERMISSION_ORDER: [u32; 54] = [
    3, 5, 28, 4, 1, 2, 0, 27, 26, 30, 43, 29, 7, 10, 33, 44, 40, 19, 41, 50, 11, 38, 35, 36, 12, 13, 34, 14, 15, 16, 17, 6, 18, 37, 31, 46, 49, 51, 52, 53, 20, 21, 22, 23, 24, 25, 8, 32, 9, 39, 42, 45, 48, 47,
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Command {
    pub name: SharedString,
    pub description: SharedString,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Similar {
    pub id: u64,
    pub name: SharedString,
    pub icon: Option<SharedString>,
    pub category: Option<SharedString>,
    pub description: SharedString,
    pub guild_count: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub id: u64,
    pub name: SharedString,
    pub icon: Option<SharedString>,
    pub banner: Option<SharedString>,
    pub overview: String,
    pub markdown: bool,
    pub categories: Vec<SharedString>,
    pub guild_count: Option<u64>,
    pub locales: Vec<String>,
    pub links: Vec<(SharedString, SharedString, &'static str)>,
    pub monetized: bool,
    pub launchable: bool,
    pub store: bool,
    pub images: Vec<SharedString>,
    pub commands: Vec<Command>,
    pub permissions: Vec<u32>,
    pub reads_messages: bool,
    pub reads_members: bool,
    pub privacy: Option<SharedString>,
}

fn cdn(path: String) -> SharedString {
    format!("https://cdn.discordapp.com/{path}").into()
}

fn s(v: &Value) -> Option<String> {
    v.as_str().filter(|x| !x.is_empty()).map(str::to_owned)
}

fn flags(a: &Value) -> u64 {
    a["flags"].as_u64().or_else(|| a["flags_new"].as_str().and_then(|f| f.parse().ok())).unwrap_or(0)
}

pub fn permission_bits(perms: &str) -> Vec<u32> {
    let Ok(n) = perms.parse::<u128>() else { return Vec::new() };
    PERMISSION_ORDER.iter().copied().filter(|b| n & (1u128 << b) != 0).collect()
}

pub fn parse_profile(a: &Value) -> Option<Profile> {
    let id: u64 = a["id"].as_str()?.parse().ok()?;
    let de = &a["directory_entry"];
    let bot = &a["bot"];
    let banner = match (s(&bot["id"]), s(&bot["banner"])) {
        (Some(b), Some(h)) => Some(cdn(format!("banners/{b}/{h}.{}?size=1024", if h.starts_with("a_") { "gif" } else { "png" }))),
        _ => s(&a["cover_image"]).map(|h| cdn(format!("app-icons/{id}/{h}.png?size=1024"))),
    };
    let (overview, markdown) = match s(&de["detailed_description"]) {
        Some(d) => (d, true),
        None => (s(&de["short_description"]).or_else(|| s(&a["description"])).unwrap_or_default(), false),
    };
    let mut links: Vec<(SharedString, SharedString, &'static str)> = de["external_urls"]
        .as_array()
        .map(|l| l.iter().filter_map(|u| Some((SharedString::from(s(&u["name"])?), SharedString::from(s(&u["url"])?), "link"))).collect())
        .unwrap_or_default();
    if let Some(t) = s(&a["terms_of_service_url"]) {
        links.push((crate::t!("app_profile.terms"), t.into(), "file"));
    }
    if let Some(p) = s(&a["privacy_policy_url"]) {
        links.push((crate::t!("app_profile.privacy"), p.into(), "lock"));
    }
    let f = flags(a);
    Some(Profile {
        id,
        name: s(&a["name"]).unwrap_or_default().into(),
        icon: s(&a["icon"]).map(|h| cdn(format!("app-icons/{id}/{h}.png?size=256"))),
        banner,
        overview,
        markdown,
        categories: a["categories"].as_array().map(|c| c.iter().filter_map(|x| s(&x["name"]).map(Into::into)).collect()).unwrap_or_default(),
        guild_count: de["guild_count"].as_u64(),
        locales: de["supported_locales"].as_array().map(|l| l.iter().filter_map(s).collect()).unwrap_or_default(),
        links,
        monetized: a["is_monetized"].as_bool().unwrap_or(false),
        launchable: f & EMBEDDED_FLAG != 0 || !a["embedded_activity_config"].is_null(),
        store: a["storefront_available"].as_bool().unwrap_or(false),
        images: de["carousel_items"].as_array().map(|l| l.iter().filter_map(|i| s(&i["proxy_url"]).or_else(|| s(&i["url"])).map(Into::into)).collect()).unwrap_or_default(),
        commands: de["popular_application_command_ids"]
            .as_array()
            .map(|ids| {
                let all = de["popular_application_commands"].as_array().cloned().unwrap_or_default();
                ids.iter()
                    .filter_map(|i| all.iter().find(|c| c["id"] == *i))
                    .map(|c| Command { name: s(&c["name"]).unwrap_or_default().into(), description: s(&c["description"]).unwrap_or_default().into() })
                    .collect()
            })
            .unwrap_or_default(),
        permissions: a["install_params"]["permissions"].as_str().map(permission_bits).unwrap_or_default(),
        reads_messages: f & MESSAGE_CONTENT_FLAGS != 0,
        reads_members: f & GUILD_MEMBERS_FLAGS != 0,
        privacy: s(&a["privacy_policy_url"]).map(Into::into),
    })
}

pub fn parse_similar(v: &Value) -> Vec<Similar> {
    v["applications"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|a| {
                    let id: u64 = a["id"].as_str()?.parse().ok()?;
                    let de = &a["directory_entry"];
                    Some(Similar {
                        id,
                        name: s(&a["name"]).unwrap_or_default().into(),
                        icon: s(&a["icon"]).map(|h| cdn(format!("app-icons/{id}/{h}.png?size=96"))),
                        category: a["categories"].get(0).and_then(|c| s(&c["name"])).map(Into::into),
                        description: s(&de["short_description"]).or_else(|| s(&a["description"])).unwrap_or_default().into(),
                        guild_count: de["guild_count"].as_u64(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn compact(n: u64) -> String {
    let fr = crate::i18n::locale().starts_with("fr");
    let one = |v: f64, unit: &str| {
        let r = (v * 10.).round() / 10.;
        let t = if r >= 10. || r.fract() == 0. { format!("{}", r.round() as u64) } else { format!("{r:.1}") };
        let t = if fr { t.replace('.', ",") } else { t };
        if fr { format!("{t}\u{a0}{unit}") } else { format!("{t}{unit}") }
    };
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => one(n as f64 / 1e3, if fr { "k" } else { "K" }),
        1_000_000..=999_999_999 => one(n as f64 / 1e6, "M"),
        _ => one(n as f64 / 1e9, if fr { "Md" } else { "B" }),
    }
}

pub fn permission_label(bit: u32) -> SharedString {
    match bit {
        11 => crate::t!("perm.send_messages"),
        b => crate::t!(&format!("gs.perm.p{b}")),
    }
}

#[derive(Default)]
struct State {
    open: Option<u64>,
    profiles: std::collections::HashMap<u64, Option<(Profile, Vec<Similar>)>>,
    tab: usize,
    image: usize,
    expanded: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

pub fn open(id: u64) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.open = Some(id);
        s.tab = 0;
        s.image = 0;
        s.expanded = false;
    });
    SCROLL.with(|h| h.set_offset(gpui::point(gpui::px(0.), gpui::px(0.))));
}

pub fn close() {
    STATE.with(|s| s.borrow_mut().open = None);
}

pub fn current() -> Option<u64> {
    STATE.with(|s| s.borrow().open)
}

fn ensure(app: &PhoenixApp, id: u64, cx: &mut Context<PhoenixApp>) {
    let go = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.profiles.contains_key(&id) {
            return false;
        }
        s.profiles.insert(id, None);
        true
    });
    if !go {
        return;
    }
    let loc = crate::i18n::locale();
    let main = app.backend.directory_get(format!("/application-directory-static/applications/{id}?locale={loc}"));
    let similar = app.backend.directory_get(format!("/application-directory-static/applications/{id}/similar?locale={loc}"));
    cx.spawn(async move |this, cx| {
        let v = main.await.unwrap_or(Value::Null);
        let sim = similar.await.unwrap_or(Value::Null);
        if let Some(p) = parse_profile(&v) {
            STATE.with(|s| s.borrow_mut().profiles.insert(id, Some((p, parse_similar(&sim)))));
        }
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn tk(i: usize) -> Hsla {
    c(tok(i))
}

fn text(size: f32, line: f32, weight: FontWeight, color: Hsla, t: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(color).child(t.into())
}

fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(path)).flex_none().size(z(size)).text_color(color)
}

fn md_button(id: impl Into<gpui::ElementId>, label: SharedString, primary: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let kind = if primary { crate::ui::controls::BtnKind::Primary } else { crate::ui::controls::BtnKind::Secondary };
    let l = label.clone();
    crate::ui::controls::button(id, label, kind).on_click(cx.listener(move |a, _, _, cx| a.stub(&l, cx)))
}

fn icon_btn(id: &'static str, path: &'static str, label: SharedString, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(40.))
        .flex_none()
        .rounded(z(8.))
        .bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .border_1()
        .border_color(tk(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
        .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| a.stub(&label, cx)))
        .child(icon(path, 20., tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
}

fn section_title(t: SharedString) -> gpui::Div {
    text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), t).mb(z(8.))
}

fn boxed() -> gpui::Div {
    div().p(z(16.)).rounded(z(8.)).border_1().border_color(tk(idx::BORDER_SUBTLE)).flex().flex_col()
}

fn side_title(t: SharedString) -> gpui::Div {
    text(14., 18., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), t).mb(z(12.))
}

fn carousel(p: &Profile, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let n = p.images.len();
    let url = p.images.get(STATE.with(|s| s.borrow().image).min(n.checked_sub(1)?))?.clone();
    let sel = STATE.with(|s| s.borrow().image).min(n - 1);
    let mut arrows = Vec::new();
    for (id, path, delta) in [("app-profile-prev", "nav/back", -1isize), ("app-profile-next", "nav/forward", 1)] {
        arrows.push(
            div()
                .id(id)
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .on_click(cx.listener(move |_, _, _, cx| {
                    STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        s.image = (s.image as isize + delta).rem_euclid(n as isize) as usize;
                    });
                    cx.notify();
                }))
                .child(icon(path, 20., rgb(0x96979e).into())),
        );
    }
    let next = arrows.pop();
    let prev = arrows.pop();
    let mut dots = div().flex().items_center().gap(z(8.));
    for i in 0..n {
        dots = dots.child(
            div()
                .id(("app-profile-dot", i))
                .size(z(8.))
                .rounded_full()
                .bg(gpui::white())
                .opacity(if i == sel { 1. } else { 0.3 })
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    STATE.with(|s| s.borrow_mut().image = i);
                    cx.notify();
                })),
        );
    }
    Some(
        div()
            .flex()
            .flex_col()
            .child(div().w_full().aspect_ratio(16. / 9.).rounded(z(5.)).overflow_hidden().bg(tk(idx::BACKGROUND_BASE_LOWER)).child(img(url).size_full().object_fit(gpui::ObjectFit::Cover)))
            .when(n > 1, |d| d.child(div().mt(z(12.)).px(z(12.)).flex().items_center().justify_between().children(prev).child(dots).children(next)))
            .into_any_element(),
    )
}

fn overview(p: &Profile, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let expanded = STATE.with(|s| s.borrow().expanded);
    let long = p.overview.len() > 600 || p.overview.matches('\n').count() > 12;
    let style = gpui_component::text::TextViewStyle::default().paragraph_gap(gpui::rems(0.5)).heading_font_size(|level, _| match level {
        1 => z(16.),
        2 => z(16.),
        _ => z(16.),
    });
    let body: AnyElement = if p.markdown {
        gpui_component::text::TextView::markdown(("app-profile-overview", p.id as usize), p.overview.clone()).style(style).into_any_element()
    } else {
        div().child(SharedString::from(p.overview.clone())).into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .child(section_title(crate::t!("app_profile.overview")))
        .child(
            div()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(tk(idx::TEXT_DEFAULT))
                .when(long && !expanded, |d| d.max_h(z(OVERVIEW_COLLAPSED_H)).overflow_hidden())
                .child(body),
        )
        .when(long, |d| {
            d.child(
                div()
                    .id("app-profile-more")
                    .mt(z(12.))
                    .h(z(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(z(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| {
                        STATE.with(|s| {
                            let mut s = s.borrow_mut();
                            s.expanded = !s.expanded;
                        });
                        cx.notify();
                    }))
                    .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::INTERACTIVE_TEXT_DEFAULT), if expanded { crate::t!("app_profile.show_less") } else { crate::t!("app_profile.show_more") }))
                    .child(icon("chevron", 18., tk(idx::TEXT_DEFAULT))),
            )
        })
        .into_any_element()
}

fn commands(p: &Profile) -> Option<AnyElement> {
    if p.commands.is_empty() {
        return None;
    }
    let mut b = boxed().gap(z(16.));
    for c in &p.commands {
        b = b.child(
            div()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(
                    div()
                        .flex_none()
                        .px(z(4.))
                        .py(z(2.))
                        .rounded(z(4.))
                        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                        .border_1()
                        .border_color(tk(idx::BORDER_SUBTLE))
                        .font_family(crate::fonts::mono_family())
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(format!("/ {}", c.name)),
                )
                .child(text(16., 20., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), c.description.clone()).min_w_0()),
        );
    }
    Some(div().flex().flex_col().child(section_title(crate::t!("app_profile.commands"))).child(b).into_any_element())
}

fn permissions(p: &Profile) -> Option<AnyElement> {
    if p.permissions.is_empty() {
        return None;
    }
    let mut grid = div().grid().grid_cols(3).gap_x(z(16.)).gap_y(z(10.));
    for b in &p.permissions {
        grid = grid.child(
            div()
                .flex()
                .items_start()
                .gap(z(8.))
                .child(icon("check", 16., rgb(0x3d9e60).into()))
                .child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), permission_label(*b)).min_w_0()),
        );
    }
    Some(
        div()
            .flex()
            .flex_col()
            .child(section_title(crate::t!("app_profile.permissions")))
            .child(boxed().child(text(14., 18., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!("app_profile.required_permissions")).mb(z(12.))).child(grid))
            .into_any_element(),
    )
}

fn data_access(p: &Profile) -> AnyElement {
    let mut b = boxed().gap(z(8.));
    if p.reads_messages || p.reads_members {
        let mut rows = div().flex().flex_col().gap(z(16.)).mb(z(16.));
        let row = |path: &'static str, title: SharedString, desc: SharedString| {
            div()
                .flex()
                .items_start()
                .gap(z(16.))
                .child(icon(path, 24., tk(idx::TEXT_DEFAULT)))
                .child(div().flex().flex_col().child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), title)).child(text(16., 20., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), desc).mt(z(4.))))
        };
        if p.reads_messages {
            rows = rows.child(row("chat", crate::t!("app_profile.read_messages"), crate::t!("app_profile.read_messages_desc")));
        }
        if p.reads_members {
            rows = rows.child(row("members", crate::t!("app_profile.members"), crate::t!("app_profile.members_desc")));
        }
        b = b.child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), crate::t!("app_profile.data_intro")).mb(z(16.))).child(rows);
    }
    let link = |t: SharedString| div().text_color(tk(idx::TEXT_LINK)).child(t);
    b = b.child(
        div()
            .flex()
            .flex_wrap()
            .text_size(z(16.))
            .line_height(z(20.))
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(crate::t!("app_profile.baseline_lead"))
            .child(link(crate::t!("app_profile.help_center")))
            .child(crate::t!("app_profile.baseline_tail")),
    );
    if p.privacy.is_some() {
        b = b.child(
            div()
                .flex()
                .flex_wrap()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(crate::t!("app_profile.privacy_lead"))
                .child(link(crate::t!("app_profile.privacy_link")))
                .child(crate::t!("app_profile.privacy_tail")),
        );
    }
    div().flex().flex_col().child(section_title(crate::t!("app_profile.data_access"))).child(b).into_any_element()
}

fn similar_card(a: &Similar, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let id = a.id;
    div()
        .id(("app-profile-similar", a.id))
        .min_w_0()
        .p(z(16.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .hover(|s| s.bg(tk(idx::BACKGROUND_SURFACE_HIGH)).border_color(tk(idx::BORDER_STRONG)))
        .cursor_pointer()
        .flex()
        .flex_col()
        .on_click(cx.listener(move |_, _, _, cx| {
            open(id);
            cx.notify();
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(match a.icon.clone() {
                    Some(u) => img(u).size(z(48.)).flex_none().rounded(z(12.)).into_any_element(),
                    None => div().size(z(48.)).flex_none().rounded(z(12.)).bg(tk(idx::BACKGROUND_SURFACE_HIGH)).into_any_element(),
                })
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), a.name.clone()).overflow_hidden().text_ellipsis().whitespace_nowrap())
                        .children(a.category.clone().map(|c| text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), c).mt(z(4.)))),
                ),
        )
        .child(text(14., 18., FontWeight::MEDIUM, tk(idx::TEXT_DEFAULT), a.description.clone()).mt(z(16.)).max_h(z(36.)).overflow_hidden())
        .children(a.guild_count.map(|n| {
            div()
                .mt(z(16.))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(icon("server/server", 16., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                .child(text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_DEFAULT), crate::t!("app_profile.in_servers", n = compact(n))))
        }))
        .into_any_element()
}

fn sidebar(p: &Profile) -> AnyElement {
    let mut col = div().w(z(SIDE_W)).flex_none().flex().flex_col().gap(z(32.));
    if let Some(n) = p.guild_count {
        col = col.child(
            div()
                .flex()
                .flex_col()
                .child(side_title(crate::t!("app_profile.server_count")))
                .child(div().flex().items_center().gap(z(8.)).child(icon("server/server", 16., tk(idx::INTERACTIVE_TEXT_DEFAULT))).child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), crate::t!("app_profile.in_servers", n = compact(n))))),
        );
    }
    if !p.categories.is_empty() {
        let mut chips = div().flex().flex_wrap().gap(z(8.));
        for c in &p.categories {
            chips = chips.child(div().px(z(12.)).py(z(6.)).rounded(z(4.)).border_1().border_color(tk(idx::BORDER_STRONG)).child(text(14., 18., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), c.clone())));
        }
        col = col.child(div().flex().flex_col().child(side_title(crate::t!("app_profile.categories"))).child(chips));
    }
    if !p.locales.is_empty() {
        let names: Vec<String> = p
            .locales
            .iter()
            .map(|l| {
                let key = format!("app_profile.locale.{l}");
                let t = crate::t!(&key).to_string();
                if t == key { crate::ui::server_pages::community::locale_name(l) } else { t }
            })
            .collect();
        col = col.child(div().flex().flex_col().child(side_title(crate::t!("app_profile.languages"))).child(text(14., 18., FontWeight::NORMAL, tk(idx::TEXT_DEFAULT), names.join(", "))));
    }
    if !p.links.is_empty() {
        let mut list = div().flex().flex_col().gap(z(12.));
        for (n, (label, url, path)) in p.links.iter().enumerate() {
            let u = url.clone();
            list = list.child(
                div()
                    .id(("app-profile-link", n))
                    .h(z(20.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&u))
                    .child(icon(path, 20., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .child(text(14., 18., FontWeight::MEDIUM, tk(idx::TEXT_DEFAULT), label.clone()).overflow_hidden().text_ellipsis().whitespace_nowrap()),
            );
        }
        col = col.child(div().flex().flex_col().child(side_title(crate::t!("app_profile.links"))).child(list));
    }
    col.into_any_element()
}

fn header(p: Option<&Profile>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(z(49.))
        .px(z(16.))
        .flex()
        .items_center()
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(
            div()
                .id("app-profile-back")
                .flex_none()
                .size(z(32.))
                .rounded(z(4.))
                .bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(|_, _, _, cx| {
                    close();
                    cx.notify();
                }))
                .child(icon("nav/back", 18., tk(idx::INTERACTIVE_TEXT_ACTIVE))),
        )
        .child(div().flex_1().min_w_0().flex().justify_center().children(p.map(|p| text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), p.name.clone()).overflow_hidden().text_ellipsis().whitespace_nowrap())))
        .child(div().size(z(32.)).flex_none())
        .into_any_element()
}

pub fn page(app: &PhoenixApp, id: u64, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure(app, id, cx);
    let data = STATE.with(|s| s.borrow().profiles.get(&id).cloned().flatten());
    let Some((p, similar)) = data else {
        return div().size_full().relative().bg(tk(idx::BACKGROUND_BASE_LOWEST)).child(header(None, cx)).into_any_element();
    };
    let tab = STATE.with(|s| s.borrow().tab);
    let banner = div().w_full().h(z(BANNER_H)).overflow_hidden().bg(tk(idx::BACKGROUND_BASE_LOWER)).children(p.banner.clone().map(|b| {
        img(b.clone()).id(crate::media::anim_id(&b)).size_full().object_fit(gpui::ObjectFit::Cover)
    }));
    let avatar = div()
        .absolute()
        .top(z(BANNER_H - 52.))
        .left(z(PAD_X - 4.))
        .size(z(AVATAR))
        .rounded(z(24.))
        .border_4()
        .border_color(tk(idx::BACKGROUND_BASE_LOWEST))
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .overflow_hidden()
        .children(p.icon.clone().map(|u| img(u).size_full().rounded(z(20.))));
    let mut actions = div().flex().items_center().gap(z(8.));
    if p.launchable {
        actions = actions.child(md_button("app-profile-launch", crate::t!("app_profile.launch"), true, cx));
    }
    actions = actions
        .child(md_button("app-profile-add", crate::t!("app_profile.add"), false, cx))
        .child(icon_btn("app-profile-copy", "link", crate::t!("app_profile.copy_link"), cx))
        .child(icon_btn("app-profile-more-menu", "more-horizontal", crate::t!("app_profile.more"), cx));
    let title = div()
        .flex()
        .items_start()
        .justify_between()
        .gap(z(16.))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .child(text(32., 40., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), p.name.clone()).overflow_hidden().text_ellipsis().whitespace_nowrap())
                .when(p.monetized, |d| d.child(text(12., 16., FontWeight::NORMAL, tk(idx::TEXT_MUTED), crate::t!("app_profile.in_app_purchases")).mt(z(4.)))),
        )
        .child(actions);
    let tabs = p.store.then(|| {
        let mut t = div().flex().items_center().gap(z(16.));
        for (n, key) in ["app_profile.about", "app_profile.store"].iter().enumerate() {
            let on = tab == n;
            t = t.child(
                div()
                    .id(("app-profile-tab", n))
                    .h(z(40.))
                    .px(z(12.))
                    .rounded(z(8.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .when(on, |d| d.bg(gpui::hsla(240. / 360., 0.04, 0.61, 0.24)))
                    .when(!on, |d| d.hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER))))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        STATE.with(|s| s.borrow_mut().tab = n);
                        cx.notify();
                    }))
                    .child(text(16., 20., FontWeight::NORMAL, if on { tk(idx::TEXT_STRONG) } else { tk(idx::INTERACTIVE_TEXT_DEFAULT) }, crate::t!(key))),
            );
        }
        t
    });
    let mut main = div().flex_1().min_w_0().flex().flex_col().gap(z(32.));
    if tab == 0 {
        main = main.children(carousel(&p, cx)).child(overview(&p, cx)).children(commands(&p)).children(permissions(&p)).child(data_access(&p));
        if !similar.is_empty() {
            let mut grid = div().grid().grid_cols(3).gap(z(16.));
            for a in similar.iter().take(3) {
                grid = grid.child(similar_card(a, cx));
            }
            main = main
                .child(div().h(z(1.)).bg(tk(idx::BORDER_SUBTLE)))
                .child(div().flex().flex_col().child(text(14., 18., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!("app_profile.similar")).mb(z(16.))).child(grid));
        }
    } else {
        main = main.child(text(16., 20., FontWeight::NORMAL, tk(idx::TEXT_MUTED), crate::t!("app_profile.store_empty")));
    }
    let content = div()
        .px(z(PAD_X))
        .pt(z(AVATAR - 52. + 20.))
        .pb(z(48.))
        .flex()
        .flex_col()
        .child(title)
        .children(tabs.map(|t| t.mt(z(16.))))
        .child(div().mt(z(16.)).flex().items_start().gap(z(COL_GAP)).child(main).child(sidebar(&p)));
    let mut scroll = div().id("app-profile-scroll").size_full().overflow_y_scroll().pt(z(49.)).pr(z(8.));
    scroll = SCROLL.with(|h| scroll.track_scroll(h));
    div()
        .size_full()
        .relative()
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .child(scroll.child(div().relative().child(banner).child(avatar).child(content)))
        .child(header(Some(&p), cx))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_directory_profile_like_the_client() {
        let v = json!({"id": "270904126974590976", "name": "Dank Memer", "icon": "ic", "is_monetized": true, "storefront_available": true, "flags": 10928128,
            "terms_of_service_url": "https://t", "privacy_policy_url": "https://p",
            "bot": {"id": "270904126974590976", "banner": "bn"},
            "install_params": {"permissions": "2147871808"},
            "categories": [{"id": 6, "name": "Jeux"}, {"id": 4, "name": "Divertissements"}],
            "directory_entry": {"guild_count": 8790000, "detailed_description": "# Hi", "supported_locales": ["en-US"],
                "carousel_items": [{"url": "https://a", "proxy_url": "https://pa"}, {"url": "https://b"}],
                "external_urls": [{"name": "Website", "url": "https://w"}],
                "popular_application_command_ids": ["2", "1"],
                "popular_application_commands": [{"id": "1", "name": "fish", "description": "f"}, {"id": "2", "name": "beg", "description": "b"}]}});
        let p = parse_profile(&v).unwrap();
        assert_eq!(p.banner.as_deref(), Some("https://cdn.discordapp.com/banners/270904126974590976/bn.png?size=1024"));
        assert_eq!(p.permissions, vec![10, 11, 13, 14, 15, 16, 6, 18, 31]);
        assert_eq!(p.commands.iter().map(|c| c.name.as_ref()).collect::<Vec<_>>(), ["beg", "fish"]);
        assert_eq!(p.images.iter().map(|c| c.as_ref()).collect::<Vec<_>>(), ["https://pa", "https://b"]);
        assert!(p.monetized && p.store && p.markdown && p.reads_messages && p.reads_members);
        assert_eq!(p.links.len(), 3);
        assert_eq!(p.links[0].0.as_ref(), "Website");
        let e = parse_profile(&json!({"id": "1", "name": "E", "flags": 537002498, "embedded_activity_config": {}, "directory_entry": {"short_description": "s"}})).unwrap();
        assert!(e.launchable && !e.markdown && !e.reads_messages && e.overview == "s" && e.permissions.is_empty());
    }

    #[test]
    fn similar_apps_and_compact_counts() {
        let s = parse_similar(&json!({"applications": [{"id": "5", "name": "Tupperbox", "icon": "i", "categories": [{"name": "Jeux"}], "directory_entry": {"guild_count": 5440000, "short_description": "d"}}]}));
        assert_eq!(s[0].icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/5/i.png?size=96"));
        assert_eq!((s[0].category.as_deref(), s[0].guild_count), (Some("Jeux"), Some(5440000)));
        assert_eq!(compact(353), "353");
        assert!(compact(8_790_000).starts_with("8"));
        assert!(compact(8_790_000).ends_with('M'));
        assert!(compact(1_700_000).contains('7'));
    }
}
