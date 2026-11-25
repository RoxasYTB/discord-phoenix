use std::cell::{Cell, RefCell};

use chrono::{DateTime, Utc};
use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*, rgba};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

pub const CDN: &str = "https://cdn.discordapp.com/";
pub const TILE_MIN_W: f32 = 336.;
pub const GAP: f32 = 24.;
pub const HERO_ART: &str = "https://cdn.discordapp.com/assets/content/fff9144c814ffc3126fdcaf5ec5bc68eff6d09e23ac8af5a3fbe61b05970cda0.png";
pub const REWARD_ART_SMALL: &str = "https://cdn.discordapp.com/assets/content/d4fc1c99b3e3044530f08ede0156d1d43a13bf78f0f539e240b63f2d4dad3554.webm?format=webp";
pub const REWARD_ART_LARGE: &str = "https://cdn.discordapp.com/assets/content/864e5841fd9af8da3c50ad6d6220bee54e8c2da2d36d3c85d0a5ae95cdbf095e.webm?format=webp";
pub const APP_FEATURE: u64 = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Video { seconds: u64 },
    Play { minutes: u64 },
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quest {
    pub id: u64,
    pub name: SharedString,
    pub game: SharedString,
    pub publisher: SharedString,
    pub hero: Option<SharedString>,
    pub logotype: Option<SharedString>,
    pub expires: Option<DateTime<Utc>>,
    pub reward: SharedString,
    pub orbs: Option<u64>,
    pub premium_orbs: Option<u64>,
    pub task: Task,
    pub app_quest: bool,
    pub cta: Option<SharedString>,
    pub enrolled: bool,
    pub completed: bool,
    pub claimed: bool,
    pub progress: f32,
}

thread_local! {
    static QUESTS: RefCell<Option<Vec<Quest>>> = const { RefCell::new(None) };
    static BALANCE: Cell<Option<u64>> = const { Cell::new(None) };
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
}

fn asset(v: &Value) -> Option<SharedString> {
    let p = v.as_str()?;
    if p.is_empty() || p == "PLACEHOLDER" {
        return None;
    }
    Some(if p.starts_with("http") { p.to_owned().into() } else { format!("{CDN}{p}").into() })
}

fn date(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
}

fn task_of(tasks: &serde_json::Map<String, Value>) -> Task {
    for k in ["WATCH_VIDEO", "WATCH_VIDEO_ON_MOBILE"] {
        if let Some(t) = tasks.get(k) {
            return Task::Video { seconds: t["target"].as_u64().unwrap_or(0) };
        }
    }
    for k in ["PLAY_ON_DESKTOP", "PLAY_ACTIVITY", "STREAM_ON_DESKTOP", "PLAY_ON_XBOX", "PLAY_ON_PLAYSTATION"] {
        if let Some(t) = tasks.get(k) {
            return Task::Play { minutes: t["target"].as_u64().unwrap_or(0) / 60 };
        }
    }
    Task::Other
}

pub fn parse(v: &Value, now: DateTime<Utc>) -> Vec<Quest> {
    let list = v.get("quests").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for q in &list {
        let cfg = &q["config"];
        let expires = date(&cfg["expires_at"]);
        if expires.is_some_and(|e| e <= now) {
            continue;
        }
        let msgs = &cfg["messages"];
        let reward = &cfg["rewards_config"]["rewards"][0];
        let us = &q["user_status"];
        let tasks = cfg["task_config_v2"]["tasks"].as_object().cloned().unwrap_or_default();
        let progress = tasks
            .iter()
            .filter_map(|(k, t)| {
                let target = t["target"].as_f64().filter(|t| *t > 0.)?;
                let value = us["progress"][k]["value"].as_f64().unwrap_or(0.);
                Some((value / target).clamp(0., 1.) as f32)
            })
            .fold(0f32, f32::max);
        let features: Vec<u64> = cfg["features"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        out.push(Quest {
            id: q["id"].as_str().and_then(|s| s.parse().ok()).unwrap_or_default(),
            name: msgs["quest_name"].as_str().unwrap_or_default().to_owned().into(),
            game: msgs["game_title"].as_str().unwrap_or_default().to_owned().into(),
            publisher: msgs["game_publisher"].as_str().unwrap_or_default().to_owned().into(),
            hero: asset(&cfg["assets"]["hero"]),
            logotype: asset(&cfg["assets"]["logotype_dark"]).or_else(|| asset(&cfg["assets"]["logotype"])),
            expires,
            reward: reward["messages"]["name"].as_str().unwrap_or_default().to_owned().into(),
            orbs: reward["orb_quantity"].as_u64(),
            premium_orbs: reward["premium_orb_quantity"].as_u64(),
            task: task_of(&tasks),
            app_quest: features.contains(&APP_FEATURE),
            cta: cfg["cta_config"]["button_label"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_owned().into()),
            enrolled: !us["enrolled_at"].is_null(),
            completed: !us["completed_at"].is_null(),
            claimed: !us["claimed_at"].is_null(),
            progress,
        });
    }
    out.retain(|q| !q.claimed);
    out
}

pub fn reward_amount(q: &Quest, premium: bool) -> Option<u64> {
    if premium { q.premium_orbs.or(q.orbs) } else { q.orbs }
}

pub fn multiplier(q: &Quest) -> Option<String> {
    let (base, boosted) = (q.orbs?, q.premium_orbs?);
    if base == 0 || boosted <= base {
        return None;
    }
    let m = boosted as f64 / base as f64;
    let s = format!("{:.1}", m);
    Some(format!("x{}", if crate::i18n::locale().starts_with("fr") { s.replace('.', ",") } else { s }))
}

pub fn duration_short(seconds: u64) -> String {
    if seconds < 60 { crate::t!("quests.seconds_short", n = seconds).to_string() } else { crate::t!("quests.minutes_short", n = seconds.div_ceil(60)).to_string() }
}

pub fn description(q: &Quest, reward: &str) -> SharedString {
    match q.task {
        Task::Video { .. } => crate::t!("quests.desc_video", reward = reward),
        Task::Play { minutes } if q.app_quest => crate::t!("quests.desc_use", game = q.game.clone(), n = minutes, reward = reward),
        Task::Play { minutes } => crate::t!("quests.desc_play", game = q.game.clone(), n = minutes, reward = reward),
        Task::Other => SharedString::default(),
    }
}

fn ensure_loaded(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if REQUESTED.with(|r| r.replace(true)) {
        return;
    }
    let fut = app.backend.quests();
    let bal = app.backend.orbs_balance();
    cx.spawn(async move |this, cx| {
        let v = fut.await.unwrap_or(Value::Null);
        QUESTS.with(|q| *q.borrow_mut() = Some(parse(&v, Utc::now())));
        let b = bal.await.ok().flatten();
        BALANCE.with(|c| c.set(b));
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

fn date_dm(e: DateTime<Utc>) -> String {
    e.with_timezone(&chrono::Local).format("%d/%m").to_string()
}

fn orb_icon(size: f32) -> gpui::Svg {
    gpui::svg().path(crate::icons::path("orbs")).size(z(size)).flex_none().text_color(tk(idx::TEXT_STRONG))
}

fn button(id: impl Into<gpui::ElementId>, label: SharedString, primary: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let kind = if primary { crate::ui::controls::BtnKind::Primary } else { crate::ui::controls::BtnKind::Secondary };
    let l = label.clone();
    crate::ui::controls::button(id, label, kind).flex_1().min_w_0().on_click(cx.listener(move |a, _, _, cx| a.stub(&l, cx)))
}

fn util_button(icon: &'static str) -> gpui::Div {
    div()
        .size(z(24.))
        .rounded_full()
        .bg(rgba(0x1e1f227a))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)))
}

fn tile(q: &Quest, premium: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let surface = tk(idx::BACKGROUND_SURFACE_HIGH);
    let banner_h = 95.;
    let mut top = div().relative().w_full().h(z(146.)).flex_none().rounded_t(z(8.)).overflow_hidden().bg(surface);
    if let Some(h) = q.hero.clone() {
        top = top.child(div().absolute().top_0().left_0().right_0().h(z(banner_h)).overflow_hidden().bg(tk(idx::BACKGROUND_BASE_LOW)).child(img(h).size_full().object_fit(gpui::ObjectFit::Cover)));
    }
    top = top
        .child(div().absolute().top_0().left_0().right_0().h(z(banner_h)).bg(gpui::linear_gradient(0., gpui::linear_color_stop(surface, 0.), gpui::linear_color_stop(surface.opacity(0.), 1.))))
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(12.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(z(16.))
                        .when(matches!(q.task, Task::Video { .. }), |d| d.child(util_button("play")))
                        .child(util_button("more-horizontal")),
                )
                .child(div().flex_1())
                .children(q.logotype.clone().map(|l| div().mb(z(4.)).h(z(34.)).w(z(121.)).flex().items_end().child(img(l).max_h(z(34.)).max_w(z(121.)).object_fit(gpui::ObjectFit::Contain))))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_start()
                        .gap(z(4.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(z(3.))
                                .child(div().text_color(tk(idx::TEXT_MUTED)).child(crate::t!("quests.promoted_by")))
                                .child(crate::ui::discovery::verified_check_badge())
                                .child(div().min_w_0().text_color(gpui::white()).child(q.publisher.clone())),
                        )
                        .children(q.expires.map(|e| div().flex_none().text_color(tk(idx::TEXT_MUTED)).child(crate::t!("quests.ends_on", date = date_dm(e))))),
                ),
        );
    let amount = reward_amount(q, premium);
    let reward_label: SharedString = match amount {
        Some(n) => crate::t!("quests.orbs", n = n),
        None => q.reward.clone(),
    };
    let art = if amount.unwrap_or(0) >= 700 { REWARD_ART_LARGE } else { REWARD_ART_SMALL };
    let mut header = div().flex().items_center().flex_wrap().gap(z(8.)).mb(z(2.)).child(
        div()
            .flex()
            .items_center()
            .gap(z(3.))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .child(crate::t!("quests.claim"))
            .when(amount.is_some(), |d| d.child(orb_icon(14.)))
            .child(reward_label.clone()),
    );
    if premium && let Some(m) = multiplier(q) {
        header = header.child(
            div()
                .h(z(19.))
                .pl(z(4.))
                .pr(z(8.))
                .rounded_full()
                .bg(gpui::linear_gradient(90., gpui::linear_color_stop(rgba(0x8c138180), 0.), gpui::linear_color_stop(rgba(0x612caf80), 1.)))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(gpui::svg().path(crate::icons::path("nitro")).size(z(16.)).text_color(gpui::white()))
                .child(text(12., 16., FontWeight::SEMIBOLD, gpui::white(), m)),
        );
    }
    let mid = div()
        .w_full()
        .h(z(104.))
        .flex_none()
        .bg(surface)
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .h(z(104.))
                .flex()
                .child(div().pt(z(12.)).pl(z(12.)).pb(z(12.)).child(img(art).size(z(80.)).rounded(z(9.4)).object_fit(gpui::ObjectFit::Cover)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .p(z(12.))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .overflow_hidden()
                        .child(text(12., 16., FontWeight::BOLD, tk(idx::TEXT_BRAND), crate::t!("quests.quest_name", name = q.name.clone()).to_uppercase()).mb(z(4.)))
                        .child(header)
                        .child(text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), description(q, &reward_label))),
                ),
        );
    let primary: SharedString = match q.task {
        Task::Video { seconds } if !q.enrolled => crate::t!("quests.watch", d = duration_short(seconds)),
        _ if !q.enrolled => crate::t!("quests.accept"),
        _ => crate::t!("quests.view"),
    };
    let cta = div()
        .w_full()
        .min_h(z(62.))
        .p(z(12.))
        .border_t_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .gap(z(8.))
        .children(q.cta.clone().map(|l| button(("quest-cta2", q.id), l, false, cx)))
        .child(button(("quest-cta1", q.id), primary, true, cx));
    div()
        .id(("quest-tile", q.id))
        .w_full()
        .min_w_0()
        .max_h(z(316.))
        .flex()
        .flex_col()
        .rounded(z(10.))
        .overflow_hidden()
        .bg(surface)
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .shadow_sm()
        .child(top)
        .child(mid)
        .child(cta)
        .into_any_element()
}

pub fn columns(width: f32) -> usize {
    (((width + GAP) / (TILE_MIN_W + GAP)).floor() as usize).max(1)
}

fn header_bar(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let balance = BALANCE.with(|b| b.get());
    let tab = |label: SharedString, on: bool| {
        div()
            .h_full()
            .flex()
            .items_center()
            .cursor_pointer()
            .border_b_2()
            .border_color(if on { tk(idx::ICON_STRONG) } else { gpui::transparent_black() })
            .hover(|s| s.border_color(tk(idx::BRAND_500)))
            .child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_STRONG), label))
    };
    div()
        .flex_none()
        .h(z(49.))
        .pl(z(20.))
        .pr(z(14.))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .flex()
        .items_center()
        .child(gpui::svg().path(crate::icons::path("quests")).size(z(24.)).flex_none().text_color(tk(idx::ICON_STRONG)))
        .child(div().ml(z(44.)).flex_1().h_full().flex().gap(z(24.)).child(tab(crate::t!("quests.all"), true)).child(tab(crate::t!("quests.claimed_tab"), false)))
        .child(
            div()
                .id("quests-balance")
                .pl(z(10.))
                .cursor_pointer()
                .on_click(cx.listener(|a, _, _, cx| a.stub(&crate::t!("quests.explore_shop"), cx)))
                .child(
                    div()
                        .h(z(32.))
                        .px(z(12.))
                        .rounded(z(8.))
                        .bg(rgba(0x97979f1f))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(orb_icon(20.))
                        .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), balance.map(|b| b.to_string()).unwrap_or_default())),
                ),
        )
        .into_any_element()
}

fn hero(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let shop = crate::t!("quests.explore_shop");
    let terms = crate::t!("quests.terms");
    let overlay_btn = |id: &'static str, label: SharedString, light: bool| {
        let l = label.clone();
        div()
            .id(id)
            .h(z(40.))
            .px(z(16.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .cursor_pointer()
            .bg(if light { gpui::white() } else { gpui::Hsla::from(rgba(0x0000008f)) })
            .text_color(if light { gpui::black() } else { gpui::white() })
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .child(label)
            .on_click(cx.listener(move |a, _, _, cx| a.stub(&l, cx)))
    };
    div()
        .relative()
        .w_full()
        .min_h(z(500.))
        .rounded(z(16.))
        .overflow_hidden()
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(gpui::black())
        .child(img(HERO_ART).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
        .child(div().absolute().inset_0().opacity(0.25).bg(gpui::linear_gradient(0., gpui::linear_color_stop(tk(idx::BRAND_500), 0.), gpui::linear_color_stop(tk(idx::BRAND_500).opacity(0.), 0.33))))
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(32.))
                .flex()
                .flex_col()
                .justify_end()
                .child(
                    div()
                        .max_w(z(540.))
                        .flex()
                        .flex_col()
                        .gap(z(16.))
                        .child(text(44., 44., FontWeight::BOLD, tk(idx::TEXT_STRONG), crate::t!("quests.hero_title").to_uppercase()))
                        .child(text(16., 20., FontWeight::NORMAL, tk(idx::TEXT_STRONG), crate::t!("quests.hero_subtitle")).opacity(0.8))
                        .child(div().flex().flex_col().items_start().gap(z(7.)).child(overlay_btn("quests-hero-shop", shop, true)).child(overlay_btn("quests-hero-terms", terms, false))),
                ),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure_loaded(app, cx);
    let quests = QUESTS.with(|q| q.borrow().clone());
    let premium = app.backend.has_premium();
    let page_w = f32::from(window.viewport_size().width) / crate::theme::zoom() - crate::ui::sidebar_resize::width() - 72.;
    let inner = page_w.min(1350.) - 40. - 8.;
    let cols = columns(inner);
    let small_btn = |id: &'static str, label: SharedString, icon: &'static str| {
        div()
            .id(id)
            .h(z(32.))
            .px(z(12.))
            .rounded(z(8.))
            .bg(rgba(0x97979f1f))
            .flex()
            .items_center()
            .gap(z(4.))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
            .child(label)
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
    };
    let heading = div()
        .h(z(32.))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(16.))
        .child(text(20., 24., FontWeight::MEDIUM, tk(idx::TEXT_STRONG), crate::t!("quests.available_title")))
        .child(div().flex().gap(z(8.)).child(small_btn("quests-sort", crate::t!("quests.suggested"), "chevron")).child(small_btn("quests-filter", crate::t!("quests.filter"), "inbox/filter")));
    let mut content = div().w_full().max_w(z(1350.)).mx_auto().px(z(20.)).py(z(24.)).flex().flex_col().child(hero(cx)).child(div().h(z(64.))).child(heading);
    match quests {
        None => content = content.child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), crate::t!("common.loading")).mt(z(24.))),
        Some(list) if list.is_empty() => {
            content = content.child(div().py(z(80.)).flex().justify_center().child(text(24., 32., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!("quests.none_title"))))
        }
        Some(list) => {
            let mut grid = div().mt(z(GAP)).grid().grid_cols(cols as u16).gap(z(GAP));
            for q in &list {
                grid = grid.child(tile(q, premium, cx));
            }
            content = content.child(grid);
        }
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .child(header_bar(cx))
        .child(div().id("quests-scroll").flex_1().min_h_0().overflow_y_scroll().pr(z(8.)).child(content))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({"quests": [
            {"id": "2", "config": {"expires_at": "2030-01-01T00:00:00+00:00", "features": [26, 15], "messages": {"quest_name": "A", "game_title": "G", "game_publisher": "P"},
              "assets": {"hero": "quests/2/h.jpg", "logotype_dark": "PLACEHOLDER", "logotype": "quests/2/l.png"},
              "rewards_config": {"rewards": [{"type": 4, "messages": {"name": "700 Orbs"}, "orb_quantity": 700, "premium_orb_quantity": 840}]},
              "task_config_v2": {"tasks": {"PLAY_ON_DESKTOP": {"target": 900}}}, "cta_config": {"button_label": "Télécharger"}},
             "user_status": {"enrolled_at": "x", "completed_at": null, "claimed_at": null, "progress": {"PLAY_ON_DESKTOP": {"value": 450}}}},
            {"id": "3", "config": {"expires_at": "2030-01-01T00:00:00+00:00", "messages": {"quest_name": "V"},
              "rewards_config": {"rewards": [{"type": 4, "orb_quantity": 200, "premium_orb_quantity": 240}]},
              "task_config_v2": {"tasks": {"WATCH_VIDEO": {"target": 35}}}}, "user_status": null},
            {"id": "4", "config": {"expires_at": "2030-01-01T00:00:00+00:00", "messages": {"quest_name": "C"}}, "user_status": {"claimed_at": "x", "enrolled_at": "x", "completed_at": "x"}},
            {"id": "1", "config": {"expires_at": "2020-01-01T00:00:00+00:00"}, "user_status": null}
        ]})
    }

    #[test]
    fn parses_quests_like_the_client_tiles() {
        let q = parse(&sample(), Utc::now());
        assert_eq!(q.iter().map(|x| x.id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(q[0].hero.as_deref(), Some("https://cdn.discordapp.com/quests/2/h.jpg"));
        assert_eq!(q[0].logotype.as_deref(), Some("https://cdn.discordapp.com/quests/2/l.png"));
        assert_eq!((q[0].orbs, q[0].premium_orbs), (Some(700), Some(840)));
        assert_eq!(q[0].task, Task::Play { minutes: 15 });
        assert!(q[0].app_quest && q[0].enrolled && !q[0].completed);
        assert_eq!(q[0].cta.as_deref(), Some("Télécharger"));
        assert!((q[0].progress - 0.5).abs() < 1e-6);
        assert_eq!(q[1].task, Task::Video { seconds: 35 });
        assert!(!q[1].app_quest && q[1].cta.is_none());
        assert_eq!(reward_amount(&q[1], true), Some(240));
        assert_eq!(reward_amount(&q[1], false), Some(200));
        assert!(multiplier(&q[1]).is_some_and(|m| m.starts_with("x1") && m.ends_with('2')));
        assert_eq!(columns(1071.), 3);
        assert_eq!(columns(300.), 1);
    }
}
