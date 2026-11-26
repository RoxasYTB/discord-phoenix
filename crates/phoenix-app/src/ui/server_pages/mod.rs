pub mod analytics;
pub mod automod;
pub mod boost;
pub mod community;
pub mod delete;
pub mod expr_upload;
pub mod guide_editors;
pub mod integrations;
pub mod mock;
pub mod onboarding;
pub mod presence;
pub mod safety;
pub mod tag;
pub mod tag_colors;
pub mod template;

use std::{cell::RefCell, collections::HashMap};

use gpui::{AnyElement, App, Context, Entity, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{InputState, TextareaState};
use phoenix_api::{
    models::Snowflake,
    rest::{guild_manage::AdminCall, guild_server::ServerCall},
};
use serde_json::Value;

use crate::{
    actions::{ActionReply, ActionRequest},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Sub {
    #[default]
    Root,
    RaidCaptcha,
    DmSpam,
    Automod,
    Permissions,
    Webhooks,
    App(String),
}

#[derive(Clone, Debug)]
pub struct AutomodMenu {
    pub rule: phoenix_api::rest::guild_server::AutomodRule,
    pub pos: gpui::Point<gpui::Pixels>,
}

#[derive(Clone, Debug)]
pub enum Load {
    Pending,
    Ready(Value),
    Failed(String),
}

#[derive(Default)]
pub struct Store {
    pub data: HashMap<(Id, String), Load>,
    pub sub: HashMap<(Id, &'static str), Sub>,
    pub open: HashMap<Id, String>,
    pub flags: HashMap<(Id, String), bool>,
    pub text: HashMap<(Id, String), String>,
    pub modal: Option<(Id, &'static str)>,
    pub rule_draft: Option<(Id, phoenix_api::rest::guild_server::AutomodRule)>,
    pub menu: Option<(Id, AutomodMenu)>,
    pub inputs: HashMap<(Id, String), Entity<InputState>>,
    pub want: Vec<(Id, String, String)>,
    pub areas: HashMap<(Id, String), Entity<TextareaState>>,
    pub want_area: Vec<(Id, String, String, usize)>,
    pub subs: Vec<gpui::Subscription>,
}

thread_local!(static STORE: RefCell<Store> = RefCell::new(Store::default()));

pub fn with<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    STORE.with(|s| f(&mut s.borrow_mut()))
}

pub fn sf(id: Id) -> Snowflake {
    Snowflake(id.0)
}

pub fn sub(guild: Id, section: &'static str) -> Sub {
    with(|s| s.sub.get(&(guild, section)).cloned().unwrap_or_default())
}

pub fn set_sub(guild: Id, section: &'static str, v: Sub) {
    with(|s| {
        s.sub.insert((guild, section), v);
    });
}

pub fn input(guild: Id, key: &str, initial: &str, cx: &mut Context<PhoenixApp>) -> Option<Entity<InputState>> {
    let k = (guild, key.to_owned());
    if let Some(e) = with(|s| s.inputs.get(&k).cloned()) {
        return Some(e);
    }
    with(|s| {
        if !s.want.iter().any(|(g, kk, _)| *g == guild && kk == key) {
            s.want.push((guild, key.to_owned(), initial.to_owned()));
        }
    });
    cx.notify();
    None
}

pub fn area(guild: Id, key: &str, initial: &str, rows: usize, cx: &mut Context<PhoenixApp>) -> Option<Entity<TextareaState>> {
    let k = (guild, key.to_owned());
    if let Some(e) = with(|s| s.areas.get(&k).cloned()) {
        return Some(e);
    }
    with(|s| {
        if !s.want_area.iter().any(|(g, kk, _, _)| *g == guild && kk == key) {
            s.want_area.push((guild, key.to_owned(), initial.to_owned(), rows));
        }
    });
    cx.notify();
    None
}

pub fn area_value(guild: Id, key: &str, cx: &App) -> Option<String> {
    with(|s| s.areas.get(&(guild, key.to_owned())).cloned()).map(|e| e.read(cx).value().to_string())
}

pub fn forget_prefix(guild: Id, prefix: &str) {
    with(|s| {
        s.inputs.retain(|(g, k), _| !(*g == guild && k.starts_with(prefix)));
        s.areas.retain(|(g, k), _| !(*g == guild && k.starts_with(prefix)));
    });
}

pub fn area_field(guild: Id, key: &str, initial: &str, rows: usize, h: f32, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let shell = div()
        .relative()
        .min_h(z(h))
        .px(z(10.))
        .py(z(10.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::INPUT_BORDER_DEFAULT))
        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
        .text_size(z(16.))
        .line_height(z(20.));
    match area(guild, key, initial, rows, cx) {
        Some(e) => shell.child(gpui_component::input::Textarea::new(&e).appearance(false).bordered(false)),
        None => shell.child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, initial.to_owned())),
    }
}

pub fn split_list(v: &str, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in v.split([',', '\n']).map(str::trim).filter(|w| !w.is_empty()) {
        if out.len() >= max {
            break;
        }
        if !out.iter().any(|x| x == w) {
            out.push(w.to_owned());
        }
    }
    out
}

pub fn value(guild: Id, key: &str, cx: &App) -> Option<String> {
    with(|s| s.inputs.get(&(guild, key.to_owned())).cloned()).map(|e| e.read(cx).value().to_string())
}

pub fn forget_input(guild: Id, key: &str) {
    with(|s| {
        s.inputs.remove(&(guild, key.to_owned()));
    });
}

pub fn field(guild: Id, key: &str, initial: &str, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    match input(guild, key, initial, cx) {
        Some(e) => crate::ui::modals::input_box(&e),
        None => div()
            .h(z(40.))
            .px(z(12.))
            .rounded(z(8.))
            .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
            .border_1()
            .border_color(t(idx::INPUT_BORDER_DEFAULT))
            .flex()
            .items_center()
            .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, initial.to_owned())),
    }
}

pub fn prepare(window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let areas = with(|s| std::mem::take(&mut s.want_area));
    for (g, k, init, rows) in areas {
        let e = cx.new(|cx| TextareaState::new(window, cx).auto_grow(rows.max(1), 12).default_value(init));
        let sub = cx.observe(&e, |_, _, cx| cx.notify());
        with(|s| {
            s.areas.insert((g, k), e);
            s.subs.push(sub);
        });
    }
    let want = with(|s| std::mem::take(&mut s.want));
    if want.is_empty() {
        return;
    }
    for (g, k, init) in want {
        let e = cx.new(|cx| InputState::new(window, cx));
        if !init.is_empty() {
            e.update(cx, |s, cx| s.set_value(init, window, cx));
        }
        let enter_key = (g, k.clone());
        let sub = cx.subscribe_in(&e, window, move |_, st, ev: &gpui_component::input::InputEvent, w, cx| match ev {
            gpui_component::input::InputEvent::Change => cx.notify(),
            gpui_component::input::InputEvent::PressEnter { .. } if enter_key.1.starts_with("am-rx:") => {
                let v = st.read(cx).value().trim().to_owned();
                if !v.is_empty() {
                    automod::add_regex(&v);
                    st.update(cx, |s, cx| s.set_value("", w, cx));
                }
                cx.notify();
            }
            _ => {}
        });
        with(|s| {
            s.inputs.insert((g, k), e);
            s.subs.push(sub);
        });
    }
    cx.notify();
}

pub fn parse_sub(s: &str) -> Option<(&'static str, Sub)> {
    let (k, arg) = s.split_once(':').unwrap_or((s, ""));
    Some(match k {
        "raid" => (safety::SECTION, Sub::RaidCaptcha),
        "dm" => (safety::SECTION, Sub::DmSpam),
        "automod" => (safety::SECTION, Sub::Automod),
        "perms" => (safety::SECTION, Sub::Permissions),
        "webhooks" => (integrations::SECTION, Sub::Webhooks),
        "app" => (integrations::SECTION, Sub::App(arg.to_owned())),
        _ => return None,
    })
}

pub fn open_sub(guild: Id, s: &str) {
    match s {
        "verification" => {
            set_sub(guild, safety::SECTION, Sub::DmSpam);
            open_modal(guild, "verification");
        }
        "wizard" => set_flag(guild, "onboarding-wizard", true),
        w if w.starts_with("upload-sticker") || w.starts_with("upload-sound") => {
            let (kind, path) = w.split_once('=').unwrap_or((w, ""));
            if kind == "upload-sound" {
                expr_upload::open_sound(guild);
            } else {
                expr_upload::open_sticker(guild, kind == "upload-sticker-partnered");
            }
            let (path, play) = path.split_once("#play=").map_or((path, None), |(p, ms)| (p, ms.parse::<f64>().ok()));
            if !path.is_empty() {
                expr_upload::load_path(std::path::Path::new(path));
            }
            if let Some(ms) = play {
                expr_upload::play_silent_from(ms);
            }
        }
        w if w.starts_with("wizard:") => {
            set_flag(guild, "onboarding-wizard", true);
            let mut parts = w.split(':').skip(1);
            onboarding::set_step(guild, parts.next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1));
            match parts.next() {
                Some("tip") => {}
                Some(spec) => {
                    onboarding::dismiss_tips(guild);
                    guide_editors::request(guild, spec);
                }
                None => onboarding::dismiss_tips(guild),
            }
        }
        _ => {
            if let Some((section, sub)) = parse_sub(s) {
                set_sub(guild, section, sub);
            }
        }
    }
}

pub fn flag(guild: Id, key: &str) -> Option<bool> {
    with(|s| s.flags.get(&(guild, key.to_owned())).copied())
}

pub fn set_flag(guild: Id, key: &str, v: bool) {
    with(|s| {
        s.flags.insert((guild, key.to_owned()), v);
    });
}

pub fn fetch(app: &PhoenixApp, guild: Id, call: ServerCall, cx: &mut Context<PhoenixApp>) -> Option<Value> {
    debug_assert!(call.is_read());
    let key = phoenix_api::rest::guild_server::build(&call).path;
    let k = (guild, key);
    let cur = with(|s| s.data.get(&k).cloned());
    match cur {
        Some(Load::Ready(v)) => return Some(v),
        Some(_) => return None,
        None => {}
    }
    with(|s| {
        s.data.insert(k.clone(), Load::Pending);
    });
    let fut = app.backend.action(ActionRequest::Admin(AdminCall::Server(call)));
    cx.spawn(async move |this, cx| {
        let r = fut.await;
        with(|s| {
            s.data.insert(
                k,
                match r {
                    Ok(ActionReply::Json(v)) => Load::Ready(v),
                    Ok(_) => Load::Ready(Value::Null),
                    Err(e) => Load::Failed(e.0),
                },
            );
        });
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
    None
}

pub fn failed(guild: Id, call: &ServerCall) -> Option<String> {
    let key = phoenix_api::rest::guild_server::build(call).path;
    with(|s| match s.data.get(&(guild, key)) {
        Some(Load::Failed(e)) => Some(e.clone()),
        _ => None,
    })
}

pub fn write(app: &mut PhoenixApp, call: ServerCall, ok: Option<SharedString>, cx: &mut Context<PhoenixApp>) {
    app.admin_toast(AdminCall::Server(call), ok, cx);
}

pub fn content(app: &PhoenixApp, guild: Id, key: &str, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let page = match key {
        "menu.gs.tag" => tag::render(app, guild, cx),
        "menu.gs.boost_perks" => boost::render(app, guild, cx),
        "menu.gs.integrations" => integrations::render(app, guild, cx),
        "menu.gs.safety" => safety::render(app, guild, cx),
        "menu.gs.community" => community::render(app, guild, cx),
        "menu.gs.onboarding" => community::onboarding(app, guild, cx),
        "menu.gs.analytics" => analytics::render(app, guild, cx),
        "menu.gs.discovery" => presence::render(app, guild, cx),
        "menu.gs.templates" => template::render(app, guild, cx),
        "menu.gs.delete" => delete::render(app, guild, cx),
        _ => return None,
    };
    Some(page)
}

pub fn notice(app: &PhoenixApp, guild: Id, key: &str, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    match key {
        "menu.gs.onboarding" => onboarding::notice(app, guild, cx),
        _ => None,
    }
}

pub fn modal(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if let Some(m) = expr_upload::render(cx) {
        return Some(m);
    }
    if let Some((guild, m)) = with(|s| s.menu.clone()) {
        return Some(automod::rule_menu(guild, m, cx));
    }
    let (guild, which) = with(|s| s.modal)?;
    match which {
        "delete" => Some(delete::modal(app, guild, cx)),
        "verification" => Some(safety::verification_modal(app, guild, cx)),
        "obw-editor" => guide_editors::modal(app, guild, cx),
        _ => None,
    }
}

pub fn open_modal(guild: Id, which: &'static str) {
    with(|s| s.modal = Some((guild, which)));
}

pub fn close_modal() {
    with(|s| s.modal = None);
}

pub fn t(i: usize) -> Hsla {
    c(tok(i))
}

pub fn text(size: f32, lh: f32, w: FontWeight, col: usize, s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(size)).line_height(z(lh)).font_weight(w).text_color(t(col)).child(s.into())
}

pub fn title(s: impl Into<SharedString>) -> gpui::Div {
    text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, s)
}

pub fn lead(s: impl Into<SharedString>) -> gpui::Div {
    text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, s)
}

pub fn label(s: impl Into<SharedString>) -> gpui::Div {
    text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, s)
}

pub fn description(s: impl Into<SharedString>) -> gpui::Div {
    text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, s)
}

pub fn link(s: impl Into<SharedString>) -> gpui::Div {
    text(14., 18., FontWeight::MEDIUM, idx::TEXT_LINK, s).cursor_pointer().hover(|s| s.underline())
}

pub fn page() -> gpui::Div {
    div().w_full().flex().flex_col()
}

pub fn divider(my: f32) -> gpui::Div {
    div().my(z(my)).h(z(1.)).flex_none().bg(t(idx::BORDER_SUBTLE))
}

pub fn card() -> gpui::Div {
    div().bg(t(idx::CARD_BACKGROUND_DEFAULT)).border_1().border_color(t(idx::BORDER_SUBTLE)).rounded(z(8.))
}

pub fn icon(name: &str, size: f32, col: usize) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(t(col))
}

pub fn pill(s: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(z(4.))
        .px(z(8.))
        .py(z(4.))
        .rounded(z(16.))
        .bg(t(idx::BACKGROUND_MOD_NORMAL))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_SUBTLE, s))
}

pub fn back(guild: Id, section: &'static str, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("gs-back-{section}")))
        .h(z(24.))
        .mb(z(32.))
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .on_click(cx.listener(move |_, _, _, cx| {
            set_sub(guild, section, Sub::Root);
            cx.notify();
        }))
        .child(icon("chevron-right", 24., idx::ICON_SUBTLE).with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))))
        .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_SUBTLE, crate::t!("gs2.back")))
}

pub fn loading() -> AnyElement {
    let err = with(|s| {
        s.data.values().find_map(|l| match l {
            Load::Failed(e) => Some(e.clone()),
            _ => None,
        })
    });
    match err {
        Some(e) => error(e),
        None => div().py(z(40.)).flex().justify_center().child(description(crate::t!("gs2.loading"))).into_any_element(),
    }
}

pub fn error(e: String) -> AnyElement {
    div().py(z(16.)).child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, e)).into_any_element()
}

pub fn str_of<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

pub fn id_of(v: &Value, k: &str) -> Option<Id> {
    v.get(k).and_then(Value::as_str).and_then(|s| s.parse().ok()).map(Id)
}

pub fn features(v: &Value) -> Vec<String> {
    v.get("features").and_then(Value::as_array).into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect()
}

pub fn channel_label(app: &PhoenixApp, ch: Option<Id>) -> SharedString {
    match ch.and_then(|i| app.channels.get(&i)) {
        Some(c) => format!("#{}", c.name).into(),
        None => crate::t!("gs2.no_channel"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_pages() {
        let g = Id(77);
        assert_eq!(sub(g, "menu.gs.safety"), Sub::Root);
        set_sub(g, "menu.gs.safety", Sub::Automod);
        assert_eq!(sub(g, "menu.gs.safety"), Sub::Automod);
        assert_eq!(sub(g, "menu.gs.integrations"), Sub::Root);
        open_sub(g, "app:7");
        assert_eq!(sub(g, "menu.gs.integrations"), Sub::App("7".into()));
        assert!(parse_sub("nope").is_none());
    }

    #[test]
    fn keys() {
        assert_eq!(features(&serde_json::json!({ "features": ["COMMUNITY", "NEWS"] })), vec!["COMMUNITY", "NEWS"]);
        assert_eq!(id_of(&serde_json::json!({ "x": "12" }), "x"), Some(Id(12)));
    }
}
