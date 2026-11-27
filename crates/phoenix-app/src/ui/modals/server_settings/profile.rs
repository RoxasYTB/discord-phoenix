use gpui::{AnyElement, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use serde_json::Value;

use super::{
    State,
    kit::{self, Cx, divider, field, labels, page_head, text},
};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::BtnKind,
};

pub const TRAIT_KEYS: [&str; 5] = ["trait0", "trait1", "trait2", "trait3", "trait4"];

pub const BANNERS: [(&str, u32, u32); 10] = [
    ("gs.profile.banner_icon", 0xFFEDCB, 0xDF9677),
    ("Soul", 0xFF8FE6, 0xFF1C90),
    ("Volcano", 0xFF876A, 0xE81D1E),
    ("Marsh", 0xFFC471, 0xE86E1D),
    ("Thunder", 0xFFFF8A, 0xE8C02F),
    ("Rising", 0xC786E0, 0x71368A),
    ("Zephyr", 0x97F6FF, 0x029FFC),
    ("Cascade", 0xB3FFFF, 0x4FE2CA),
    ("Earth", 0x93B858, 0x406601),
    ("Boulder", 0x707070, 0x272727),
];

pub fn traits(s: &State) -> Vec<(Option<String>, String)> {
    let mut out: Vec<(i64, Option<String>, String)> = s
        .get("profile")
        .and_then(|p| p.get("traits"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|t| {
            let emoji = t.get("emoji_name").and_then(Value::as_str).map(str::to_owned);
            (t.get("position").and_then(Value::as_i64).unwrap_or(0), emoji, kit::s(t, "label"))
        })
        .collect();
    out.sort_by_key(|t| t.0);
    out.into_iter().map(|t| (t.1, t.2)).collect()
}

fn emoji(name: &str, size: f32) -> AnyElement {
    let glyph = crate::ui::pickers::emoji::by_shortcode(name).map(crate::ui::pickers::emoji::glyph).unwrap_or(name);
    crate::media::emoji::emoji_element(glyph, size)
}

fn gradient(inner: u32, outer: u32) -> gpui::Background {
    gpui::linear_gradient(0., gpui::linear_color_stop(c(inner), 0.2), gpui::linear_color_stop(c(outer), 0.85))
}

pub fn banner_url(guild: u64, p: &Value) -> Option<SharedString> {
    let h = p.get("custom_banner_hash").and_then(Value::as_str)?;
    Some(format!("https://cdn.discordapp.com/discovery-splashes/{guild}/{h}.png?size=600").into())
}

pub fn icon_url(guild: u64, g: &Value, size: u32) -> Option<SharedString> {
    let h = g.get("icon").or_else(|| g.get("icon_hash")).and_then(Value::as_str)?;
    let ext = if h.starts_with("a_") { "gif" } else { "webp" };
    Some(format!("https://cdn.discordapp.com/icons/{guild}/{h}.{ext}?size={size}").into())
}

fn game_cover(app: &Value, size: u32) -> Option<SharedString> {
    let id = kit::s(app, "id");
    let h = app.get("cover_image").or_else(|| app.get("icon")).and_then(Value::as_str)?;
    Some(format!("https://cdn.discordapp.com/app-icons/{id}/{h}.png?size={size}").into())
}

fn game_icon(app: &Value, size: u32) -> Option<SharedString> {
    let id = kit::s(app, "id");
    let h = app.get("icon").and_then(Value::as_str)?;
    Some(format!("https://cdn.discordapp.com/app-icons/{id}/{h}.png?size={size}").into())
}

pub fn top_game_ids(top: &Value, selected: &[String]) -> Vec<String> {
    let Some(map) = top.as_object() else { return Vec::new() };
    let mut ids: Vec<(&String, f64)> = map.iter().filter(|(k, _)| !selected.contains(k)).map(|(k, v)| (k, v.get("score").and_then(Value::as_f64).unwrap_or(0.))).collect();
    ids.sort_by(|a, b| b.1.total_cmp(&a.1));
    ids.into_iter().map(|(k, _)| k.clone()).collect()
}

fn members_play(app: &PhoenixApp, s: &State, cx: &mut Cx) -> Option<Vec<Value>> {
    use phoenix_api::rest::guild_server::ServerCall;
    let top = crate::ui::server_pages::fetch(app, s.guild, ServerCall::TopGames { guild: crate::actions::admin::sf(s.guild) }, cx)?;
    let selected: Vec<String> = games(s).iter().map(|g| kit::s(g, "id").to_owned()).collect();
    let ids = top_game_ids(&top, &selected);
    if ids.is_empty() {
        return Some(Vec::new());
    }
    let snow: Vec<phoenix_api::models::Snowflake> = ids.iter().filter_map(|i| i.parse().ok()).map(phoenix_api::models::Snowflake).collect();
    let apps = crate::ui::server_pages::fetch(app, s.guild, ServerCall::PublicApps { ids: snow }, cx)?;
    let list = apps.as_array().cloned().unwrap_or_default();
    Some(ids.iter().filter_map(|id| list.iter().find(|a| kit::s(a, "id") == *id).cloned()).collect())
}

pub fn games(s: &State) -> Vec<Value> {
    let order: Vec<String> =
        s.get("profile").and_then(|p| p.get("game_application_ids")).and_then(Value::as_array).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    let apps = s.list("games");
    order.iter().filter_map(|id| apps.iter().find(|a| kit::s(a, "id") == *id).cloned()).collect()
}

pub fn since_label(guild: u64) -> SharedString {
    let secs = kit::snowflake_unix(guild);
    let d = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
    let month = crate::t!(&format!("gs.month.{}", chrono::Datelike::month(&d)));
    crate::t!("gs.profile.since", date = format!("{month} {}", chrono::Datelike::year(&d)))
}

fn tile(id: (&'static str, usize), bg: gpui::Background, selected: bool) -> gpui::Stateful<gpui::Div> {
    let inner = div().flex_1().h(z(64.)).rounded(z(8.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).bg(bg).overflow_hidden();
    div()
        .id(id)
        .flex_1()
        .min_w(z(100.))
        .cursor_pointer()
        .when(selected, |d| d.m(z(-4.)).p(z(3.)).rounded(z(10.)).border_1().border_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))))
        .flex()
        .child(inner)
}

fn banners(s: &State, guild: u64, cx: &mut Cx) -> AnyElement {
    let p = s.get("profile").cloned().unwrap_or(Value::Null);
    let uploaded = banner_url(guild, &p);
    let local = s.uploads.get("banner").cloned();
    let current = s.edits.banner_preset.or(if uploaded.is_some() || local.is_some() { Some(BANNERS.len()) } else { None });
    let has_image = uploaded.is_some() || local.is_some();
    let mut cells: Vec<AnyElement> = BANNERS
        .iter()
        .enumerate()
        .map(|(i, (_, a, b))| {
            tile(("ss-banner", i), gradient(*a, *b), current == Some(i))
                .on_click(cx.listener(move |_, _, _, cx| {
                    super::with_state(|s| s.edits.banner_preset = Some(i));
                    cx.notify();
                }))
                .into_any_element()
        })
        .collect();
    let up = div()
        .id(("ss-banner", BANNERS.len()))
        .flex_1()
        .min_w(z(100.))
        .group("ss-banner-up")
        .cursor_pointer()
        .on_click(cx.listener(|_, _, _, cx| super::pick_upload("banner", &crate::ui::file_pick::IMAGES, cx)))
        .when(current == Some(BANNERS.len()), |d| d.m(z(-4.)).p(z(3.)).rounded(z(10.)).border_1().border_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))))
        .flex()
        .child(
            div()
                .relative()
                .flex_1()
                .h(z(64.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .children(match (&local, &uploaded) {
                    (Some(p), _) => Some(gpui::img(p.clone()).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)),
                    (None, Some(u)) => Some(gpui::img(u.clone()).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)),
                    _ => None,
                })
                .child(div().absolute().inset_0().bg(c(tok(idx::BACKGROUND_SCRIM))).opacity(if has_image { 0. } else { 1. }).group_hover("ss-banner-up", |s| s.opacity(1.)))
                .child(gpui::svg().path(crate::icons::path("image-plus")).size(z(24.)).text_color(gpui::white()).opacity(if has_image { 0. } else { 1. }).group_hover("ss-banner-up", |s| s.opacity(1.))),
        )
        .into_any_element();
    cells.push(up);
    let mut grid = div().flex().flex_col().gap(z(8.));
    let mut it = cells.into_iter().peekable();
    while it.peek().is_some() {
        let mut row = div().flex().gap(z(8.));
        for _ in 0..3 {
            row = row.child(it.next().unwrap_or_else(|| div().flex_1().min_w(z(100.)).into_any_element()));
        }
        grid = grid.child(row);
    }
    grid.into_any_element()
}

fn trait_inputs(s: &State) -> AnyElement {
    let t = traits(s);
    let cell = |i: usize| {
        let em = t.get(i).and_then(|x| x.0.clone());
        div()
            .flex_1()
            .min_w_0()
            .h(z(40.))
            .px(z(4.))
            .gap(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
            .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
            .flex()
            .items_center()
            .child(
                div().size(z(30.)).flex_none().flex().items_center().justify_center().child(match em {
                    Some(e) => emoji(&e, 20.),
                    None => gpui::svg().path(crate::icons::path("emoji")).size(z(20.)).text_color(c(tok(idx::ICON_SUBTLE))).into_any_element(),
                }),
            )
            .child(div().flex_1().min_w_0().text_size(z(16.)).children(s.inputs.get(TRAIT_KEYS[i]).map(|i| gpui_component::input::Input::new(i).appearance(false).bordered(false))))
            .child(div().size(z(30.)).flex_none().flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("close-small")).size(z(16.)).text_color(c(tok(idx::ICON_SUBTLE)))))
    };
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(div().flex().gap(z(8.)).child(cell(0)).child(cell(1)))
        .child(div().flex().gap(z(8.)).child(cell(2)).child(cell(3)))
        .child(div().flex().gap(z(8.)).child(cell(4)).child(div().flex_1()))
        .into_any_element()
}

fn games_section(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let list = games(s);
    let mut grid = div().flex().flex_col().gap(z(12.));
    for chunk in list.chunks(3) {
        let mut row = div().flex().gap(z(8.));
        for g in chunk {
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .relative()
                            .h(z(181.5))
                            .mb(z(8.))
                            .rounded(z(4.))
                            .overflow_hidden()
                            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                            .children(game_cover(g, 300).map(|u| gpui::img(u).size_full().object_fit(gpui::ObjectFit::Cover)))
                            .child(
                                div()
                                    .absolute()
                                    .top(z(4.))
                                    .right(z(4.))
                                    .size(z(20.))
                                    .rounded(z(4.))
                                    .bg(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(gpui::svg().path(crate::icons::path("check")).size(z(14.)).text_color(gpui::white())),
                            ),
                    )
                    .child(text(kit::s(g, "name"), 12., 16., FontWeight::MEDIUM, idx::TEXT_STRONG).truncate()),
            );
        }
        for _ in chunk.len()..3 {
            row = row.child(div().flex_1());
        }
        grid = grid.child(row);
    }
    let mut parts = vec![
        kit::select_box("ss-games", text(crate::t!("gs.profile.games_search"), 16., 20., FontWeight::NORMAL, idx::INPUT_PLACEHOLDER_TEXT_DEFAULT).into_any_element(), false).into_any_element(),
        grid.into_any_element(),
    ];
    if let Some(members) = members_play(app, s, cx).filter(|l| !l.is_empty()) {
        let mut tiles = div().flex().flex_wrap().gap_x(z(8.)).gap_y(z(12.));
        for g in members {
            tiles = tiles.child(
                div()
                    .id(SharedString::from(format!("ss-top-game-{}", kit::s(&g, "id"))))
                    .w(z(133.))
                    .flex()
                    .flex_col()
                    .opacity(0.5)
                    .hover(|d| d.opacity(1.))
                    .cursor_pointer()
                    .child(
                        div()
                            .h(z(181.5))
                            .mb(z(8.))
                            .rounded(z(4.))
                            .overflow_hidden()
                            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                            .children(game_cover(&g, 256).map(|u| gpui::img(u).size_full().object_fit(gpui::ObjectFit::Cover))),
                    )
                    .child(text(kit::s(&g, "name"), 12., 16., FontWeight::MEDIUM, idx::TEXT_STRONG).truncate()),
            );
        }
        parts.push(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).into_any_element());
        parts.push(div().flex().flex_col().gap(z(8.)).child(text(crate::t!("gs.profile.members_play"), 12., 16., FontWeight::SEMIBOLD, idx::TEXT_MUTED)).child(tiles).into_any_element());
    }
    kit::fieldset(crate::t!("gs.profile.games"), Some(crate::t!("gs.profile.games_desc")), parts).into_any_element()
}

fn private_section(s: &State, cx: &mut Cx) -> AnyElement {
    let private = s.get("profile").and_then(|p| p.get("visibility")).and_then(Value::as_u64) == Some(2);
    let left = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .flex()
                .gap(z(24.))
                .child(labels(crate::t!("gs.profile.private"), Some(crate::t!("gs.profile.private_desc"))).flex_1())
                .child(crate::ui::controls::switch("ss-private", private)),
        )
        .child(text(crate::t!("gs.profile.private_note"), 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED));
    let _ = cx;
    let card = div()
        .flex_1()
        .min_w_0()
        .opacity(if private { 1. } else { 0.5 })
        .child(
            div()
                .rounded(z(16.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(div().h(z(120.)).bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .child(div().px(z(16.)).mt(z(-34.)).child(div().size(z(70.)).rounded(z(20.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).border_4().border_color(c(tok(idx::BACKGROUND_SURFACE_HIGH)))))
                .child(div().px(z(16.)).my(z(8.)).child(text(crate::t!("gs.profile.private_title"), 20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG)))
                .child(div().px(z(16.)).mb(z(16.)).child(text(crate::t!("gs.profile.private_body"), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE))),
        );
    div().flex().gap(z(32.)).child(left).child(card).into_any_element()
}

fn dot(color: Hsla) -> gpui::Div {
    div().size(z(8.)).flex_none().rounded_full().bg(color)
}

pub fn preview(s: &State, guild: u64) -> AnyElement {
    card(s, guild, None)
}

pub fn preview_with_cta(s: &State, guild: u64, cta: AnyElement) -> AnyElement {
    card(s, guild, Some(cta))
}

fn card(s: &State, guild: u64, cta: Option<AnyElement>) -> AnyElement {
    let wide = cta.is_some();
    let p = s.get("profile").cloned().unwrap_or(Value::Null);
    let g = s.guild_json();
    let name = {
        let n = kit::s(&p, "name");
        if n.is_empty() { kit::s(g, "name") } else { n }
    };
    let banner: AnyElement = match (s.edits.banner_preset, banner_url(guild, &p)) {
        (Some(i), _) if i < BANNERS.len() => div().h(z(120.)).bg(gradient(BANNERS[i].1, BANNERS[i].2)).into_any_element(),
        _ if s.uploads.contains_key("banner") => gpui::img(s.uploads["banner"].clone()).w_full().h(z(120.)).object_fit(gpui::ObjectFit::Cover).into_any_element(),
        (_, Some(u)) => gpui::img(u).w_full().h(z(120.)).object_fit(gpui::ObjectFit::Cover).into_any_element(),
        _ => div().h(z(120.)).bg(gradient(BANNERS[0].1, BANNERS[0].2)).into_any_element(),
    };
    let icon = s
        .uploads
        .get("icon")
        .map(|p| gpui::img(p.clone()).size(z(64.)).rounded(z(19.)).object_fit(gpui::ObjectFit::Cover).into_any_element())
        .or_else(|| icon_url(guild, g, 128).map(|u| gpui::img(u).size(z(64.)).rounded(z(19.)).into_any_element()))
        .unwrap_or_else(|| div().size(z(64.)).rounded(z(19.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).into_any_element());
    let tag = kit::s(&p, "tag");
    let online = kit::n(&p, "online_count");
    let members = kit::n(&p, "member_count");
    let header = div()
        .px(z(16.))
        .my(z(8.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(text(name, 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).truncate())
                .when(kit::n(&p, "visibility") == 1 || g.get("features").and_then(Value::as_array).is_some_and(|f| f.iter().any(|x| x == "DISCOVERABLE")), |d| {
                    d.child(gpui::svg().path(crate::icons::path("discovery-badge")).size(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))))
                })
                .when(!tag.is_empty(), |d| {
                    d.child(
                        div()
                            .px(z(4.))
                            .rounded(z(4.))
                            .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                            .flex()
                            .items_center()
                            .gap(z(2.))
                            .child(text(tag.clone(), 12., 16., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT)),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(z(8.))
                .child(div().flex().items_center().gap(z(4.)).child(dot(c(tok(idx::TEXT_STATUS_ONLINE)))).child(text(crate::t!("gs.profile.online", n = kit::fmt_int(online)), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE)))
                .child(div().flex().items_center().gap(z(4.)).child(dot(c(tok(idx::TEXT_STATUS_OFFLINE)))).child(text(crate::t!("gs.profile.members", n = kit::fmt_int(members)), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE))),
        )
        .child(text(since_label(guild), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE));
    let games = games(s);
    let body = div()
        .px(z(16.))
        .mb(z(16.))
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(text(kit::s(&p, "description"), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE))
        .when(!games.is_empty(), |d| {
            d.child(div().flex().gap(z(4.)).children(games.iter().filter_map(|a| game_icon(a, 64)).map(|u| {
                gpui::img(u).size(z(26.)).rounded(z(4.)).border_1().border_color(c(tok(idx::BORDER_MUTED)))
            })))
        })
        .child(div().flex().flex_wrap().gap(z(4.)).children(traits(s).into_iter().map(|(e, l)| {
            div()
                .py(z(4.))
                .px(z(8.))
                .gap(z(4.))
                .rounded(z(16.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .items_center()
                .children(e.map(|e| emoji(&e, 16.)))
                .child(text(l, 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT))
        })));
    div()
        .when(wide, |d| d.w_full())
        .when(!wide, |d| d.w(z(300.)))
        .flex_none()
        .rounded(z(16.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(banner)
        .child(div().px(z(16.)).mt(z(-34.)).child(div().size(z(70.)).rounded(z(21.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).flex().items_center().justify_center().child(icon)))
        .child(header)
        .child(body)
        .children(cta.map(|b| div().px(z(16.)).pb(z(16.)).child(b)))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let guild = s.guild.0;
    let icon_buttons = div()
        .flex()
        .gap(z(8.))
        .child(kit::btn_sm("ss-icon-change", crate::t!("gs.profile.icon_change"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| super::pick_upload("icon", &crate::ui::file_pick::IMAGES, cx))))
        .child(kit::critical_secondary("ss-icon-remove", crate::t!("gs.profile.icon_remove")));
    let left = div()
        .flex_1()
        .min_w_0()
        .max_w(z(560.))
        .flex()
        .flex_col()
        .gap(z(40.))
        .child(page_head(crate::t!("menu.gs.profile"), Some(crate::t!("gs.profile.desc"))))
        .child(field(crate::t!("gs.profile.name"), None, s.inputs.get("name").map(|i| super::super::input_box(i).into_any_element()).unwrap_or_else(|| div().h(z(40.)).into_any_element())))
        .child(divider())
        .child(field(crate::t!("gs.profile.icon"), Some(crate::t!("gs.profile.icon_desc")), icon_buttons))
        .child(divider())
        .child(field(crate::t!("gs.profile.banner"), None, banners(s, guild, cx)))
        .child(divider())
        .child(field(crate::t!("gs.profile.traits"), Some(crate::t!("gs.profile.traits_desc")), trait_inputs(s)))
        .child(divider())
        .child(field(
            crate::t!("gs.profile.description"),
            Some(crate::t!("gs.profile.description_desc")),
            s.areas.get("description").map(|a| super::super::area_box(a, 3).into_any_element()).unwrap_or_else(|| div().h(z(88.)).into_any_element()),
        ))
        .child(divider())
        .child(games_section(app, s, cx))
        .child(divider())
        .child(private_section(s, cx));
    let right = div().w(z(300.)).flex_none().flex().flex_col().gap(z(24.)).child(preview(s, guild));
    div().w_full().pt(z(60.)).px(z(40.)).pb(z(100.)).flex().gap(z(40.)).child(left).child(right).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn top_games_skip_selected_and_sort_by_score() {
        let top = json!({ "1": { "score": 0.2 }, "2": { "score": 0.9 }, "3": { "score": 0.5 } });
        assert_eq!(top_game_ids(&top, &["3".to_owned()]), vec!["2", "1"]);
        assert!(top_game_ids(&json!(null), &[]).is_empty());
    }

    #[test]
    fn traits_sorted_and_banner_urls() {
        let mut s = State::new(crate::model::Id(5), 0);
        s.data.insert(
            "profile",
            json!({ "traits": [
                { "emoji_name": "robot", "label": "BOT", "position": 1 },
                { "emoji_name": null, "label": "AI", "position": 0 }
            ], "custom_banner_hash": "abc", "game_application_ids": ["2", "1"] }),
        );
        s.data.insert("games", json!([{ "id": "1", "name": "One" }, { "id": "2", "name": "Two" }]));
        assert_eq!(traits(&s), vec![(None, "AI".to_owned()), (Some("robot".to_owned()), "BOT".to_owned())]);
        assert_eq!(banner_url(5, s.get("profile").unwrap()).as_deref(), Some("https://cdn.discordapp.com/discovery-splashes/5/abc.png?size=600"));
        assert_eq!(games(&s).iter().map(|g| kit::s(g, "name")).collect::<Vec<_>>(), ["Two", "One"]);
        assert_eq!(icon_url(5, &json!({ "icon": "a_x" }), 64).as_deref(), Some("https://cdn.discordapp.com/icons/5/a_x.gif?size=64"));
    }
}
