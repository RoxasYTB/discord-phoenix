use gpui::{Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*};
use serde_json::Value;

use crate::theme::{c, generated::idx, tok, z};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WidgetItem {
    pub name: SharedString,
    pub value: SharedString,
    pub image: Option<SharedString>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppWidget {
    pub app_id: u64,
    pub name: SharedString,
    pub icon: Option<SharedString>,
    pub title: SharedString,
    pub subtitle: SharedString,
    pub hero: Option<SharedString>,
    pub items: Vec<WidgetItem>,
    pub contained: bool,
    pub progress: Option<(f32, WidgetItem)>,
    pub mini_stat: SharedString,
    pub mini_hero: Option<SharedString>,
    pub order: usize,
}

pub fn app_ids(profile_widgets: &[Value]) -> Vec<(usize, u64)> {
    profile_widgets
        .iter()
        .enumerate()
        .filter_map(|(order, w)| {
            let d = w.get("data")?;
            (d.get("type")?.as_str()? == "application").then_some(())?;
            Some((order, d.get("application_id")?.as_str()?.parse().ok()?))
        })
        .collect()
}

pub fn identity_data(identities: &Value, app: u64) -> Vec<(String, String)> {
    let id = app.to_string();
    identities
        .get("identities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|i| i.get("application_id").and_then(Value::as_str) == Some(id.as_str()))
        .filter_map(|i| i.pointer("/profile/data/dynamic")?.as_array().cloned())
        .flatten()
        .filter_map(|d| {
            let v = match d.get("value")? {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            Some((d.get("name")?.as_str()?.to_owned(), v))
        })
        .collect()
}

fn asset_url(app: u64, cfg: &Value, key: &str, size: u32) -> Option<SharedString> {
    let a = cfg.get("resolved_assets")?.as_array()?.iter().find(|a| a.get("key").and_then(Value::as_str) == Some(key))?;
    let id = a.get("asset_id")?.as_str()?;
    Some(format!("https://cdn.discordapp.com/app-assets/{app}/{id}.png?size={size}").into())
}

fn resolve(app: u64, cfg: &Value, data: &[(String, String)], f: Option<&Value>, size: u32) -> Option<SharedString> {
    let f = f?;
    let v = f.get("value").and_then(Value::as_str);
    let out = match f.get("value_type").and_then(Value::as_str) {
        Some("data") => v.and_then(|k| data.iter().find(|(n, _)| n == k)).map(|(_, v)| SharedString::from(v.clone())),
        Some("custom_string") => v.map(|s| SharedString::from(s.to_owned())),
        Some("application_asset") => v.and_then(|k| asset_url(app, cfg, k, size)),
        _ => None,
    };
    out.or_else(|| resolve(app, cfg, data, f.get("fallback"), size))
}

pub fn build(app: u64, configs: &Value, data: &[(String, String)]) -> Option<AppWidget> {
    let cfgs = configs.as_array()?;
    let cfg = cfgs.iter().find(|c| c.get("status").and_then(Value::as_str) == Some("published")).or(cfgs.first())?;
    let field = |surface: &str, comp: &str, name: &str, size: u32| {
        resolve(app, cfg, data, cfg.pointer(&format!("/surfaces/{surface}/components/{comp}/fields/{name}")), size)
    };
    let mut items: Vec<(u32, WidgetItem)> = cfg
        .pointer("/surfaces/widget_bottom/components")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, _)| {
            let n: u32 = k.strip_prefix("item_")?.parse().ok()?;
            Some((
                n,
                WidgetItem {
                    name: field("widget_bottom", k, "name", 128).unwrap_or_default(),
                    value: field("widget_bottom", k, "description", 128).unwrap_or_default(),
                    image: field("widget_bottom", k, "image", 128),
                },
            ))
        })
        .collect();
    items.sort_by_key(|(n, _)| *n);
    let appl = cfg.get("application");
    let name = appl
        .and_then(|a| a.get("name"))
        .or(cfg.get("display_name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let icon = appl
        .and_then(|a| a.get("icon")?.as_str())
        .map(|h| SharedString::from(format!("https://cdn.discordapp.com/app-icons/{app}/{h}.png?size=32")));
    Some(AppWidget {
        app_id: app,
        name: name.into(),
        icon,
        title: field("widget_top", "title", "text", 0).unwrap_or_default(),
        subtitle: field("widget_top", "subtitle_1", "text", 0).unwrap_or_default(),
        hero: field("widget_top", "hero_image", "image", 512).or_else(|| field("widget_top", "contained_image", "image", 512)),
        contained: cfg.pointer("/surfaces/widget_top/components/contained_image").is_some(),
        progress: cfg.pointer("/surfaces/widget_bottom/components/progress").map(|_| {
            let v = field("widget_bottom", "progress", "current", 0).and_then(|v| v.trim_end_matches('%').trim().parse::<f32>().ok()).unwrap_or(0.);
            let pct = if v <= 1. { v * 100. } else { v };
            (
                pct.clamp(0., 100.),
                WidgetItem {
                    name: field("widget_bottom", "objective", "name", 128).unwrap_or_default(),
                    value: field("widget_bottom", "objective", "description", 128).unwrap_or_default(),
                    image: field("widget_bottom", "objective", "image", 128),
                },
            )
        }),
        mini_stat: field("mini_profile", "stat", "text", 0).unwrap_or_default(),
        mini_hero: field("mini_profile", "hero_image", "image", 256),
        items: items.into_iter().map(|(_, i)| i).collect(),
        order: 0,
    })
}

pub struct WidgetColors {
    pub strong: Hsla,
    pub muted: Hsla,
    pub card: Hsla,
    pub more: bool,
}

pub fn render(w: &AppWidget, col: &WidgetColors) -> Div {
    let header = div()
        .h(z(24.))
        .flex()
        .items_center()
        .gap(z(4.))
        .children(w.icon.clone().map(|u| img(u).size(z(16.)).rounded(z(2.))))
        .child(div().flex_1().min_w_0().truncate().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(w.name.clone()))
        .children(col.more.then(|| crate::ui::profile_board::more_button(col.strong)));
    let content = div()
        .flex()
        .flex_col()
        .justify_center()
        .gap(z(2.))
        .py(z(32.))
        .w(z(188.))
        .child(div().mb(z(4.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(w.title.clone()))
        .when(!w.subtitle.is_empty(), |d| d.child(div().text_size(z(14.)).line_height(z(18.)).text_color(col.muted).child(w.subtitle.clone())));
    let contained = w.contained.then(|| w.hero.clone()).flatten().map(|u| {
        div()
            .absolute()
            .top(z(48.))
            .right(z(16.))
            .size(z(152.))
            .rounded(z(12.))
            .overflow_hidden()
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .child(img(u).size_full().object_fit(gpui::ObjectFit::Contain))
    });
    let hero = (!w.contained).then(|| w.hero.clone()).flatten().map(|u| {
        div()
            .absolute()
            .top_0()
            .right_0()
            .w(z(204.))
            .h(z(148.))
            .rounded_tr(z(16.))
            .overflow_hidden()
            .child(img(crate::ui::fade_image::source(u)).w(z(204.)).h(z(151.)).object_fit(gpui::ObjectFit::Cover))
    });
    let content = if w.contained { content.min_h(z(180.)) } else { content };
    let top = div().relative().pt(z(12.)).px(z(16.)).children(hero).children(contained).child(header).child(content);
    let item = |i: &WidgetItem| {
        div()
            .w(z(182.))
            .flex()
            .gap(z(12.))
            .child(match &i.image {
                Some(u) => img(u.clone()).size(z(48.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).object_fit(gpui::ObjectFit::Contain).into_any_element(),
                None => div().size(z(48.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).into_any_element(),
            })
            .child(
                div()
                    .min_w_0()
                    .my_auto()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(i.name.clone()))
                    .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(col.muted).child(i.value.clone())),
            )
    };
    let grid = div()
        .pt(z(24.))
        .px(z(16.))
        .pb(z(16.))
        .flex()
        .flex_wrap()
        .gap(z(16.))
        .children(w.items.iter().map(item));
    div()
        .relative()
        .rounded(z(16.))
        .border_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .bg(col.card)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(top)
        .when(!w.items.is_empty(), |d| d.child(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))).child(grid))
        .when_some(w.progress.clone(), |d, p| d.child(div().h(z(2.)).bg(c(tok(idx::BORDER_SUBTLE)))).child(progress_row(&p, col)))
}

fn progress_row((pct, it): &(f32, WidgetItem), col: &WidgetColors) -> Div {
    div()
        .pt(z(24.))
        .px(z(16.))
        .pb(z(16.))
        .flex()
        .items_center()
        .gap(z(12.))
        .child(match &it.image {
            Some(u) => img(u.clone()).size(z(50.)).flex_none().rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).object_fit(gpui::ObjectFit::Contain).into_any_element(),
            None => div().size(z(50.)).flex_none().rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).into_any_element(),
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(
                    div().h(z(6.)).w_full().rounded(z(12.)).bg(gpui::hsla(240. / 360., 0.04, 0.608, 0.32)).child(
                        div().h_full().w(gpui::relative(pct / 100.)).rounded(z(12.)).bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT))),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .gap(z(2.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(it.name.clone()))
                                .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_SUBTLE))).child(it.value.clone())),
                        )
                        .child(div().flex_none().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(format!("{}%", pct.round()))),
                ),
        )
}

pub fn render_compact(w: &AppWidget, col: &WidgetColors) -> Div {
    let header = div()
        .h(z(16.))
        .flex()
        .items_center()
        .gap(z(4.))
        .children(w.icon.clone().map(|u| img(u).size(z(16.)).rounded(z(2.))))
        .child(div().truncate().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(col.strong).child(w.name.clone()));
    let content = div()
        .py(z(2.))
        .flex()
        .flex_col()
        .gap(z(2.))
        .child(div().truncate().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(col.strong).child(if w.mini_stat.is_empty() { w.title.clone() } else { w.mini_stat.clone() }))
        .child(
            div()
                .id("widget-view-all-stats")
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::NORMAL)
                .text_color(col.strong)
                .cursor_pointer()
                .hover(|s| s.underline())
                .child("View All Stats"),
        );
    let hero = w.mini_hero.clone().or_else(|| w.hero.clone()).map(|u| {
        let s = 79.;
        div()
            .relative()
            .size(z(s))
            .flex_none()
            .child(img(crate::ui::fade_image::source(u)).size(z(s)).object_fit(gpui::ObjectFit::Cover))
    });
    div()
        .flex()
        .flex_none()
        .gap(z(4.))
        .rounded(z(8.))
        .overflow_hidden()
        .border_1()
        .border_color(c(tok(idx::USER_PROFILE_BORDER)))
        .bg(col.card)
        .child(div().flex_1().min_w_0().p(z(8.)).flex().flex_col().gap(z(8.)).child(header).child(content))
        .children(hero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_card_from_config_and_identity() {
        let widgets = vec![json!({"data": {"type": "application", "application_id": "42"}}), json!({"data": {"type": "favorite_games"}})];
        assert_eq!(app_ids(&widgets), vec![(0, 42)]);
        let ident = json!({"identities": [{"application_id": "42", "profile": {"data": {"dynamic": [
            {"type": 1, "name": "title", "value": "Hello"}, {"type": 1, "name": "users", "value": "12"}]}}}]});
        let data = identity_data(&ident, 42);
        let cfg = json!([{"status": "published", "application": {"name": "Bot", "icon": "abc"},
            "resolved_assets": [{"key": "img", "asset_id": "7"}],
            "surfaces": {
                "widget_top": {"components": {"title": {"fields": {"text": {"value_type": "data", "value": "title"}}},
                    "subtitle_1": {"fields": {"text": {"value_type": "custom_string", "value": "Sub"}}},
                    "hero_image": {"fields": {"image": {"value_type": "application_asset", "value": "img"}}}}},
                "widget_bottom": {"components": {
                    "item_2": {"fields": {"name": {"value_type": "custom_string", "value": "B"}, "description": {"value_type": "data", "value": "nope", "fallback": {"value_type": "custom_string", "value": "-"}}}},
                    "item_1": {"fields": {"name": {"value_type": "custom_string", "value": "A"}, "description": {"value_type": "data", "value": "users"}}}}}}}]);
        let w = build(42, &cfg, &data).unwrap();
        assert_eq!(w.title, "Hello");
        assert_eq!(w.subtitle, "Sub");
        assert_eq!(w.name, "Bot");
        assert_eq!(w.hero.as_deref(), Some("https://cdn.discordapp.com/app-assets/42/7.png?size=512"));
        assert_eq!(w.items.iter().map(|i| (i.name.to_string(), i.value.to_string())).collect::<Vec<_>>(), vec![("A".into(), "12".into()), ("B".into(), "-".into())]);
    }
}
