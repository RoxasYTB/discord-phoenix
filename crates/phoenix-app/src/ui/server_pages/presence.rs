use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::ServerCall;
use serde_json::Value;

use super::{description, divider, fetch, icon, label, link, page, sf, str_of, t, text, title};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button_sm, switch},
};

fn field(v: impl Into<SharedString>, h: f32) -> gpui::Div {
    div()
        .min_h(z(h))
        .px(z(12.))
        .py(z(9.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::INPUT_BORDER_DEFAULT))
        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
        .flex()
        .items_start()
        .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, v))
}

fn chip(s: String) -> gpui::Div {
    div().px(z(8.)).py(z(4.)).rounded(z(4.)).bg(t(idx::BACKGROUND_MOD_SUBTLE)).border_1().border_color(t(idx::BORDER_MUTED)).flex().items_center().gap(z(4.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, s)).child(icon("close-small", 16., idx::ICON_SUBTLE))
}

pub fn category_name(cats: &Value, id: u64) -> String {
    cats.as_array().into_iter().flatten().find(|c| c.get("id").and_then(Value::as_u64) == Some(id)).map(|c| str_of(c, "name").to_owned()).unwrap_or_default()
}

pub fn channel_summary(app: &PhoenixApp, list: &[Id]) -> String {
    let names: Vec<String> = list.iter().take(2).map(|c| app.channels.get(c).map(|c| c.name.to_string()).unwrap_or_default()).collect();
    let more = list.len().saturating_sub(2);
    if more > 0 { format!("{}, +\u{a0}{more} en plus", names.join(", ")) } else { names.join(", ") }
}

fn content_row(app: &PhoenixApp, head: SharedString, desc: SharedString, on: bool, ids: &[Id], key: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .justify_between()
                .gap(z(24.))
                .child(div().w(z(453.)).flex().flex_col().gap(z(4.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, head)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, desc)).child(link(crate::t!("gs2.learn_more"))))
                .child(switch(key, on)),
        )
        .child(
            div()
                .mt(z(12.))
                .px(z(12.))
                .py(z(15.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .flex()
                .gap(z(12.))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("gs2.selected_channels")))
                .child(div().flex_1().child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, channel_summary(app, ids))))
                .child(icon("chevron-right", 16., idx::ICON_DEFAULT)),
        )
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let meta = fetch(app, guild, ServerCall::DiscoveryMetadata { guild: sf(guild) }, cx);
    let cats = fetch(app, guild, ServerCall::DiscoveryCategories { locale: "fr".into() }, cx);
    let content = fetch(app, guild, ServerCall::DiscoverableContent { guild: sf(guild) }, cx);
    let index = fetch(app, guild, ServerCall::ServerIndex { guild: sf(guild) }, cx);
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx);
    let mut p = page().child(title(crate::t!("menu.gs.discovery"))).child(div().mt(z(8.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.presence_lead"))));
    let (Some(meta), Some(cats), Some(content), Some(g)) = (meta, cats, content, g) else { return p.child(super::loading()).into_any_element() };
    let url = index.as_ref().and_then(|i| i.get("seo")).map(|s| str_of(s, "canonical_url").to_owned()).unwrap_or_default();
    p = p
        .child(div().mt(z(24.)).child(label(crate::t!("gs2.page_url"))))
        .child(
            div()
                .mt(z(8.))
                .h(z(40.))
                .pl(z(12.))
                .pr(z(4.))
                .rounded(z(8.))
                .border_1()
                .border_color(t(idx::INPUT_BORDER_DEFAULT))
                .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
                .flex()
                .items_center()
                .child(div().flex_1().min_w_0().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, url.clone()).truncate()))
                .child(button_sm("gs2-copy-url", crate::t!("gs2.copy"), BtnKind::Secondary).on_click({
                    let url = url.clone();
                    move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(url.clone()))
                })),
        )
        .child(div().mt(z(16.)).flex().child(button_sm("gs2-open-page", crate::t!("gs2.view_page"), BtnKind::Primary).on_click({
            let url = url.clone();
            move |_, _, cx| cx.open_url(&url)
        })))
        .child(divider(40.));
    let primary = meta.get("primary_category_id").and_then(Value::as_u64).unwrap_or(0);
    let mut subs = div().flex().flex_wrap().gap(z(4.));
    for c in meta.get("category_ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64) {
        subs = subs.child(chip(category_name(&cats, c)));
    }
    p = p
        .child(label(crate::t!("gs2.category")))
        .child(div().mt(z(4.)).child(description(crate::t!("gs2.category_desc"))))
        .child(div().mt(z(8.)).child(crate::ui::controls::select_field("gs2-category", category_name(&cats, primary))))
        .child(div().mt(z(40.)).child(label(crate::t!("gs2.subcategories"))))
        .child(div().mt(z(8.)).min_h(z(40.)).p(z(6.)).rounded(z(8.)).border_1().border_color(t(idx::INPUT_BORDER_DEFAULT)).bg(t(idx::INPUT_BACKGROUND_DEFAULT)).child(subs))
        .child(divider(40.));
    let picked = splash_draft(guild);
    let splash: Option<gpui::ImageSource> = match &picked {
        Some(p) if p.is_empty() => None,
        Some(p) => Some(std::path::PathBuf::from(p).into()),
        None => g.get("discovery_splash").and_then(Value::as_str).map(|h| SharedString::from(format!("https://cdn.discordapp.com/discovery-splashes/{}/{h}.jpg?size=512", guild.0)).into()),
    };
    let has_splash = splash.is_some();
    p = p.child(
        div()
            .flex()
            .gap(z(24.))
            .child(div().w(z(324.)).flex().flex_col().gap(z(4.)).child(label(crate::t!("gs2.discovery_cover"))).child(description(crate::t!("gs2.discovery_cover_desc"))))
            .child(
                div()
                    .w(z(279.))
                    .flex()
                    .flex_col()
                    .gap(z(12.))
                    .child(div().h(z(157.)).rounded(z(8.)).overflow_hidden().bg(t(idx::BACKGROUND_MOD_STRONG)).children(splash.map(|u| gpui::img(u).size_full().object_fit(gpui::ObjectFit::Cover))))
                    .when(has_splash, |d| {
                        d.child(div().flex().justify_center().child(
                            div().id("gs2-remove-cover").cursor_pointer().hover(|s| s.underline()).child(text(14., 14., FontWeight::NORMAL, idx::TEXT_LINK, crate::t!("gs2.remove"))).on_click(cx.listener(move |_, _, _, cx| {
                                set_splash_draft(guild, String::new());
                                cx.notify();
                            })),
                        ))
                    })
                    .child(button_sm("gs2-upload-cover", crate::t!("gs2.upload_bg"), BtnKind::Secondary).w_full().on_click(cx.listener(move |_, _, _, cx| {
                        crate::ui::file_pick::pick(&["png", "jpg", "jpeg", "webp", "gif"], cx, move |_, path, cx| {
                            set_splash_draft(guild, path.to_string_lossy().into_owned());
                            cx.notify();
                        });
                    }))),
            ),
    );
    let sect = |k: &str| content.get(k).cloned().unwrap_or(Value::Null);
    let ids = |v: &Value| -> Vec<Id> {
        v.get("channels").and_then(Value::as_array).into_iter().flatten().filter(|c| c.get("included").and_then(Value::as_bool).unwrap_or(false)).filter_map(|c| super::id_of(c, "channel_id")).collect()
    };
    let sums = sect("channel_summaries");
    let msgs = sect("channel_messages");
    p = p
        .child(divider(40.))
        .child(div().flex().items_center().gap(z(16.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("gs2.web_content"))).child(div().px(z(4.)).rounded(z(4.)).bg(t(idx::BACKGROUND_BRAND)).child(text(12., 16., FontWeight::SEMIBOLD, idx::WHITE, crate::t!("gs2.new")))))
        .child(div().mt(z(4.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.web_content_desc"))))
        .child(div().mt(z(32.)).child(content_row(app, crate::t!("gs2.channel_summaries"), crate::t!("gs2.channel_summaries_desc"), sums.get("enabled").and_then(Value::as_bool).unwrap_or(false), &ids(&sums), "gs2-sum-sw")))
        .child(div().mt(z(32.)).child(content_row(app, crate::t!("gs2.channel_messages"), crate::t!("gs2.channel_messages_desc"), msgs.get("enabled").and_then(Value::as_bool).unwrap_or(false), &ids(&msgs), "gs2-msg-sw")))
        .child(divider(40.));
    let desc0 = str_of(&g, "description").to_owned();
    p = p
        .child(label(crate::t!("gs2.server_description")))
        .child(div().mt(z(4.)).child(description(crate::t!("gs2.presence_description_desc"))))
        .child(div().mt(z(8.)).child(super::area_field(guild, "pr-desc", &desc0, 3, 88., cx)))
        .child(divider(40.))
        .child(label(crate::t!("gs2.reasons")))
        .child(div().mt(z(4.)).child(description(crate::t!("gs2.reasons_desc"))));
    let reasons: Vec<Value> = meta.get("reasons_to_join").and_then(Value::as_array).cloned().unwrap_or_default();
    for i in 0..4 {
        let r = reasons.get(i);
        p = p.child(
            div()
                .mt(z(if i == 0 { 8. } else { 16. }))
                .h(z(40.))
                .px(z(10.))
                .rounded(z(8.))
                .border_1()
                .border_color(t(idx::INPUT_BORDER_DEFAULT))
                .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
                .flex()
                .items_center()
                .gap(z(10.))
                .child(div().w(z(19.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, r.map(|r| str_of(r, "emoji_name").to_owned()).unwrap_or_default())))
                .child(div().flex_1().min_w_0().child(match super::input(guild, &format!("pr-reason-{i}"), r.map(|r| str_of(r, "reason")).unwrap_or_default(), cx) {
                    Some(e) => gpui_component::input::Input::new(&e).appearance(false).bordered(false).into_any_element(),
                    None => text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, r.map(|r| str_of(r, "reason").to_owned()).unwrap_or_default()).into_any_element(),
                }))
                .child(icon("close-small", 16., idx::ICON_SUBTLE)),
        );
    }
    let about = str_of(&meta, "about").to_owned();
    let n = super::area_value(guild, "pr-about", cx).unwrap_or_else(|| about.clone()).chars().count();
    p = p
        .child(divider(40.))
        .child(label(crate::t!("gs2.about")))
        .child(div().mt(z(4.)).child(description(crate::t!("gs2.about_desc"))))
        .child(div().mt(z(8.)).child(super::area_field(guild, "pr-about", &about, 10, 232., cx)))
        .child(div().mt(z(4.)).flex().justify_end().child(text(12., 16., FontWeight::NORMAL, idx::TEXT_MUTED, format!("{n} / 2400"))))
        .child(divider(40.))
        .child(label(crate::t!("gs2.social")))
        .child(div().mt(z(4.)).child(description(crate::t!("gs2.social_desc"))));
    let links = social_rows(&meta, social_extra(guild));
    for (i, l) in links.iter().map(String::as_str).enumerate() {
        let (plat, prefix) = social_platform(l);
        p = p.child(
            div()
                .id(("gs2-social", i))
                .mt(z(8.))
                .flex()
                .gap(z(12.))
                .child(div().w(z(186.)).child(crate::ui::controls::select_field(("gs2-social-plat", i), plat)))
                .child(match super::input(guild, &format!("pr-social-{i}"), &social_display(l, prefix), cx) {
                    Some(e) => crate::ui::modals::input_box(&e).flex_1().into_any_element(),
                    None => field(social_display(l, prefix), 40.).flex_1().into_any_element(),
                }),
        );
    }
    p = p
        .child(div().mt(z(20.)).flex().child({
            let full = links.len() >= MAX_SOCIAL;
            button_sm("gs2-social-add", crate::t!("gs2.add_another"), BtnKind::Secondary).when(full, |b| b.opacity(0.5).cursor_not_allowed()).on_click(cx.listener(move |_, _, _, cx| {
                if !full {
                    super::with(|s| {
                        let n = s.text.get(&(guild, "pr-social-extra".to_owned())).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                        s.text.insert((guild, "pr-social-extra".to_owned()), (n + 1).to_string());
                    });
                    cx.notify();
                }
            }))
        }))
        .child(div().mt(z(80.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.presence_remove_hint"))));
    let draft = Draft::read(guild, &meta, cx);
    let orig = Draft::original(&g, &meta);
    if let Some(d) = draft.filter(|d| *d != orig || picked.is_some()) {
        let reset: crate::ui::modals::settings_layout::Act = std::rc::Rc::new(move |_, _, _| forget_draft(guild));
        let reasons0: Vec<Value> = meta.get("reasons_to_join").and_then(Value::as_array).cloned().unwrap_or_default();
        let save: crate::ui::modals::settings_layout::Act = std::rc::Rc::new(move |a, _, cx| {
            let (meta_patch, desc) = d.patch(&orig, &reasons0);
            if let Some(patch) = meta_patch {
                super::write(a, ServerCall::EditDiscoveryMetadata { guild: sf(guild), patch }, None, cx);
            }
            if let Some(description) = desc {
                a.admin_toast(
                    phoenix_api::rest::guild_manage::AdminCall::EditGuild { guild: sf(guild), edit: phoenix_api::rest::guild_manage::GuildEdit { description: Some(description), ..Default::default() } },
                    None,
                    cx,
                );
            }
            if let Some(w) = splash_draft(guild).map(|p| splash_write(guild, &p)) {
                a.blocked_write(w, cx);
            }
            forget_draft(guild);
        });
        p = p.child(div().mt(z(16.)).child(crate::ui::modals::settings_layout::unsaved_bar(reset, save, cx)));
    }
    p.into_any_element()
}

pub const MAX_SOCIAL: usize = 9;

pub fn social_rows(meta: &Value, extra: usize) -> Vec<String> {
    let mut v: Vec<String> = meta.get("social_links").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
    v.extend(std::iter::repeat_n(String::new(), extra));
    v.truncate(MAX_SOCIAL);
    v
}

fn social_extra(guild: Id) -> usize {
    super::with(|s| s.text.get(&(guild, "pr-social-extra".to_owned())).and_then(|v| v.parse().ok()).unwrap_or(0))
}

fn splash_draft(guild: Id) -> Option<String> {
    super::with(|s| s.text.get(&(guild, "pr-splash".to_owned())).cloned())
}

fn set_splash_draft(guild: Id, path: String) {
    super::with(|s| {
        s.text.insert((guild, "pr-splash".to_owned()), path);
    });
}

fn forget_draft(guild: Id) {
    super::forget_prefix(guild, "pr-");
    super::with(|s| s.text.retain(|(g, k), _| !(*g == guild && k.starts_with("pr-"))));
}

pub fn splash_write(guild: Id, path: &str) -> crate::actions::guard::GuardedWrite {
    let data = (!path.is_empty()).then(|| crate::ui::file_pick::data_uri(std::path::Path::new(path))).flatten();
    crate::actions::guard::GuardedWrite::edit_guild(guild.0, serde_json::json!({ "discovery_splash": data }))
}

fn social_display(l: &str, prefix: &str) -> String {
    format!("{prefix}{}", l.trim_start_matches("https://").trim_start_matches("http://"))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub description: String,
    pub about: String,
    pub reasons: Vec<String>,
    pub social: Vec<String>,
}

impl Draft {
    pub fn original(g: &Value, meta: &Value) -> Self {
        let reasons = meta.get("reasons_to_join").and_then(Value::as_array).cloned().unwrap_or_default();
        Self {
            description: str_of(g, "description").to_owned(),
            about: str_of(meta, "about").to_owned(),
            reasons: (0..4).map(|i| reasons.get(i).map(|r| str_of(r, "reason").to_owned()).unwrap_or_default()).collect(),
            social: meta.get("social_links").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(|l| social_display(l, "")).collect(),
        }
    }

    fn read(guild: Id, meta: &Value, cx: &gpui::App) -> Option<Self> {
        let social_n = social_rows(meta, social_extra(guild)).len();
        let desc = super::area_value(guild, "pr-desc", cx)?;
        Some(Self {
            description: desc,
            about: super::area_value(guild, "pr-about", cx)?,
            reasons: (0..4).map(|i| super::value(guild, &format!("pr-reason-{i}"), cx).unwrap_or_default()).collect(),
            social: (0..social_n).map(|i| super::value(guild, &format!("pr-social-{i}"), cx).unwrap_or_default()).collect(),
        })
    }

    pub fn patch(&self, orig: &Self, reasons0: &[Value]) -> (Option<Value>, Option<String>) {
        let mut m = serde_json::Map::new();
        if self.about != orig.about {
            m.insert("about".into(), Value::from(self.about.trim()));
        }
        if self.reasons != orig.reasons {
            let list: Vec<Value> = self
                .reasons
                .iter()
                .enumerate()
                .filter(|(_, r)| !r.trim().is_empty())
                .map(|(i, r)| {
                    let mut o = reasons0.get(i).cloned().filter(Value::is_object).unwrap_or_else(|| serde_json::json!({ "emoji_id": null, "emoji_name": null }));
                    o["reason"] = Value::from(r.trim());
                    o
                })
                .collect();
            m.insert("reasons_to_join".into(), Value::from(list));
        }
        if self.social != orig.social {
            let links: Vec<Value> = self.social.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| Value::from(format!("https://{s}"))).collect();
            m.insert("social_links".into(), Value::from(links));
        }
        let desc = (self.description != orig.description).then(|| self.description.trim().to_owned());
        ((!m.is_empty()).then_some(Value::Object(m)), desc)
    }
}

pub fn social_platform(url: &str) -> (&'static str, &'static str) {
    let host = url.trim_start_matches("https://").trim_start_matches("www.");
    for (h, name) in [("instagram.com", "Instagram"), ("x.com", "X"), ("twitter.com", "X"), ("youtube.com", "YouTube"), ("tiktok.com", "TikTok"), ("twitch.tv", "Twitch"), ("facebook.com", "Facebook"), ("reddit.com", "Reddit"), ("github.com", "GitHub")] {
        if host.starts_with(h) {
            return (name, "");
        }
    }
    ("Site web", "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn social_rows_grow_up_to_nine() {
        let meta = json!({ "social_links": ["https://x.com/a"] });
        assert_eq!(social_rows(&meta, 2), vec!["https://x.com/a".to_owned(), String::new(), String::new()]);
        assert_eq!(social_rows(&meta, 20).len(), MAX_SOCIAL);
        let w = splash_write(Id(5), "");
        assert_eq!((w.path.as_str(), w.body), ("/guilds/5", Some(json!({ "discovery_splash": null }))));
        assert!(w.refusal().is_write_blocked());
    }

    #[test]
    fn presence_helpers() {
        let cats = json!([{ "id": 49, "name": "Bots" }, { "id": 14, "name": "Discussion générale" }]);
        assert_eq!(category_name(&cats, 49), "Bots");
        assert_eq!(social_platform("https://instagram.com/gladosofficiel").0, "Instagram");
    }

    #[test]
    fn presence_saves_only_changed_fields() {
        let g = json!({ "description": "Desc" });
        let meta = json!({ "about": "A", "reasons_to_join": [{ "reason": "r1", "emoji_name": "🔥" }], "social_links": ["https://instagram.com/x"] });
        let orig = Draft::original(&g, &meta);
        assert_eq!(orig.social, vec!["instagram.com/x"]);
        let r0 = meta["reasons_to_join"].as_array().unwrap().clone();
        assert_eq!(orig.patch(&orig, &r0), (None, None));
        let mut d = orig.clone();
        d.about = " Nouveau ".into();
        d.social[0] = "instagram.com/y".into();
        let (m, desc) = d.patch(&orig, &r0);
        assert_eq!(m, Some(json!({ "about": "Nouveau", "social_links": ["https://instagram.com/y"] })));
        assert_eq!(desc, None);
        d.description = "Autre".into();
        d.reasons[0] = "r2".into();
        d.reasons[1] = "neuf".into();
        let (m, desc) = d.patch(&orig, &r0);
        assert_eq!(desc.as_deref(), Some("Autre"));
        assert_eq!(m.unwrap()["reasons_to_join"], json!([{ "reason": "r2", "emoji_name": "🔥" }, { "reason": "neuf", "emoji_id": null, "emoji_name": null }]));
    }
}
