use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, deferred, div, prelude::*};
use phoenix_api::rest::guild_server::ServerCall;
use serde_json::{Value, json};

use super::onboarding::{self as ob, Chan};
use super::{fetch, icon, sf, str_of, t, text};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button, radio_indicator},
};

pub const VIEW: u64 = 0;
pub const CHAT: u64 = 1;
pub const WELCOME_MIN: usize = 7;
pub const WELCOME_MAX: usize = 300;
pub const TASK_TITLE_MIN: usize = 7;
pub const TASK_TITLE_MAX: usize = 60;
pub const RES_TITLE_MAX: usize = 30;
pub const RES_DESC_MAX: usize = 200;
pub const ANSWER_TITLE_MAX: usize = 50;
pub const ANSWER_DESC_MAX: usize = 100;
pub const MAX_TASKS: usize = 5;
pub const MAX_RESOURCES: usize = 7;

#[derive(Clone, Debug, PartialEq)]
pub enum Editor {
    Welcome { author: Option<String> },
    Task { index: Option<usize>, channel: Option<String>, action: Option<u64>, emoji: Value, icon_path: Option<std::path::PathBuf>, icon_data: Option<String>, thumb_menu: bool },
    Resource { index: Option<usize>, channel: Option<String>, icon_path: Option<std::path::PathBuf>, icon_data: Option<String> },
    Answer { prompt: String, index: Option<usize>, channels: Vec<String>, roles: Vec<String>, emoji: Value, title_error: bool },
    Unlock { review: bool, selected: Vec<String> },
    BoostUpsell,
}

thread_local! {
    static EDITOR: RefCell<Option<(Id, Editor)>> = const { RefCell::new(None) };
    static OPEN_SELECT: RefCell<Option<&'static str>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<(Id, String)>> = const { RefCell::new(None) };
}

pub fn request(guild: Id, spec: &str) {
    PENDING.with(|p| *p.borrow_mut() = Some((guild, spec.to_owned())));
}

pub fn apply_pending(guild: Id, home: &Value, prompts: &[ob::Prompt], private: Option<&str>) {
    let Some(spec) = PENDING.with(|p| p.borrow_mut().take_if(|(g, _)| *g == guild).map(|(_, s)| s)) else { return };
    let (what, arg) = spec.split_once('=').unwrap_or((spec.as_str(), ""));
    let index = arg.parse::<usize>().ok();
    match what {
        "welcome" => open_welcome(guild, home),
        "task" => open_task(guild, home, index),
        "res" => open_resource(guild, home, index),
        "answer" => {
            if let Some(p) = prompts.get(index.unwrap_or(0)) {
                open_answer(guild, p, p.options.first().map(|_| 0));
            }
        }
        "unlock" | "unlock-review" => {
            if let Some(ch) = private {
                open_unlock(guild, ch);
                if what == "unlock-review" {
                    update(|e| {
                        if let Editor::Unlock { review, .. } = e {
                            *review = true;
                        }
                    });
                }
            }
        }
        _ => {}
    }
}

pub fn current() -> Option<(Id, Editor)> {
    EDITOR.with(|e| e.borrow().clone())
}

fn update(f: impl FnOnce(&mut Editor)) {
    EDITOR.with(|e| {
        if let Some((_, ed)) = e.borrow_mut().as_mut() {
            f(ed);
        }
    });
}

fn toggle_select(key: &'static str) {
    OPEN_SELECT.with(|o| {
        let mut o = o.borrow_mut();
        *o = if *o == Some(key) { None } else { Some(key) };
    });
}

fn select_open(key: &'static str) -> bool {
    OPEN_SELECT.with(|o| *o.borrow() == Some(key))
}

pub fn open(guild: Id, ed: Editor) {
    super::forget_prefix(guild, "obe-");
    OPEN_SELECT.with(|o| *o.borrow_mut() = None);
    EDITOR.with(|e| *e.borrow_mut() = Some((guild, ed)));
    super::open_modal(guild, "obw-editor");
}

pub fn close() {
    EDITOR.with(|e| *e.borrow_mut() = None);
    OPEN_SELECT.with(|o| *o.borrow_mut() = None);
    super::close_modal();
}

pub fn open_welcome(guild: Id, home: &Value) {
    let author = home.pointer("/welcome_message/author_ids/0").and_then(Value::as_str).map(str::to_owned);
    open(guild, Editor::Welcome { author });
}

pub fn open_task(guild: Id, home: &Value, index: Option<usize>) {
    let a = index.and_then(|i| home.get("new_member_actions")?.get(i).cloned()).unwrap_or(Value::Null);
    open(
        guild,
        Editor::Task {
            index,
            channel: a.get("channel_id").and_then(Value::as_str).map(str::to_owned),
            action: a.get("action_type").and_then(Value::as_u64),
            emoji: a.get("emoji").cloned().unwrap_or(Value::Null),
            icon_path: None,
            icon_data: None,
            thumb_menu: false,
        },
    );
}

pub fn open_resource(guild: Id, home: &Value, index: Option<usize>) {
    let r = index.and_then(|i| home.get("resource_channels")?.get(i).cloned()).unwrap_or(Value::Null);
    open(
        guild,
        Editor::Resource {
            index,
            channel: r.get("channel_id").and_then(Value::as_str).map(str::to_owned),
            icon_path: None,
            icon_data: None,
        },
    );
}

pub fn open_boost_upsell(guild: Id) {
    open(guild, Editor::BoostUpsell);
}

pub fn pick_emoji(val: Value) {
    update(|e| match e {
        Editor::Task { emoji, icon_path, icon_data, thumb_menu, .. } => {
            *emoji = val;
            *icon_path = None;
            *icon_data = None;
            *thumb_menu = false;
        }
        Editor::Answer { emoji, .. } => {
            *emoji = val;
        }
        _ => {}
    });
}

pub fn open_answer(guild: Id, prompt: &ob::Prompt, index: Option<usize>) {
    let o = index.and_then(|i| prompt.options.get(i)).cloned().unwrap_or_default();
    open(
        guild,
        Editor::Answer { prompt: prompt.id.clone(), index, channels: o.channel_ids, roles: o.role_ids, emoji: o.emoji, title_error: false },
    );
}

pub fn open_unlock(guild: Id, channel: &str) {
    open(guild, Editor::Unlock { review: false, selected: vec![channel.to_owned()] });
}

pub fn access_overwrite(channel: &Value, guild: &str, kind: u64) -> Value {
    let need = if ob::is_voice(kind) { ob::VIEW_CHANNEL | ob::CONNECT } else { ob::VIEW_CHANNEL };
    let cur = channel
        .get("permission_overwrites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|o| str_of(o, "id") == guild);
    let num = |k: &str| cur.and_then(|o| o.get(k)).and_then(|v| v.as_str().and_then(|s| s.parse::<u64>().ok()).or_else(|| v.as_u64())).unwrap_or(0);
    json!({ "id": guild, "type": 0, "allow": (num("allow") | need).to_string(), "deny": (num("deny") & !need).to_string() })
}

pub fn welcome_error(message: &str, has_author: bool) -> Option<&'static str> {
    let n = message.chars().count();
    match (n, has_author) {
        (0, _) => None,
        (n, false) if n < WELCOME_MIN => Some("obe.welcome_err_both"),
        (_, false) => Some("obe.welcome_err_author"),
        (n, true) if n < WELCOME_MIN => Some("obe.welcome_err_short"),
        _ => None,
    }
}

pub fn task_title_error(title: &str) -> bool {
    let n = title.chars().count();
    n > 0 && n < TASK_TITLE_MIN
}

pub fn action_allowed(action: u64, ch: &Chan) -> bool {
    match action {
        VIEW => ch.public,
        CHAT => ch.chattable,
        _ => false,
    }
}

pub fn task_channel(ch: &Chan) -> bool {
    matches!(ch.kind, 0 | 5 | 15 | 16) && ch.public
}

pub fn resource_channel(ch: &Chan) -> bool {
    ch.kind == 0 && ch.public && !ch.chattable
}

pub fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn emoji_json(e: &Value) -> Value {
    let mut m = serde_json::Map::new();
    for k in ["id", "name", "animated"] {
        if let Some(v) = e.get(k).filter(|v| !v.is_null()) {
            m.insert(k.to_owned(), v.clone());
        }
    }
    Value::Object(m)
}

pub fn home_body(guild: &str, home: &Value, list: &[Chan]) -> Value {
    let exists = |v: &Value| list.iter().any(|c| c.id == str_of(v, "channel_id"));
    let entry = |v: &Value, action: bool| {
        let mut o = serde_json::Map::new();
        o.insert("channel_id".into(), json!(str_of(v, "channel_id")));
        if action {
            o.insert("action_type".into(), v.get("action_type").cloned().unwrap_or(Value::Null));
        }
        o.insert("title".into(), json!(str_of(v, "title")));
        o.insert("description".into(), json!(str_of(v, "description")));
        o.insert("emoji".into(), emoji_json(v.get("emoji").unwrap_or(&Value::Null)));
        if let Some(i) = v.get("icon").filter(|i| !i.is_null()) {
            o.insert("icon".into(), i.clone());
        }
        Value::Object(o)
    };
    let pick = |k: &str, action: bool| -> Vec<Value> {
        home.get(k).and_then(Value::as_array).into_iter().flatten().filter(|v| exists(v)).map(|v| entry(v, action)).collect()
    };
    json!({
        "guild_id": guild,
        "welcome_message": {
            "author_ids": home.pointer("/welcome_message/author_ids").cloned().unwrap_or_else(|| json!([])),
            "message": home.pointer("/welcome_message/message").and_then(Value::as_str).unwrap_or(""),
        },
        "new_member_actions": pick("new_member_actions", true),
        "resource_channels": pick("resource_channels", false),
        "enabled": home.get("enabled").cloned().unwrap_or(Value::Bool(false)),
    })
}

fn heading(s: SharedString, required: bool) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, s))
        .when(required, |d| d.child(div().ml(z(4.)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_FEEDBACK_CRITICAL, "*"))))
}

fn group() -> gpui::Div {
    div().flex().flex_col().gap(z(8.))
}

fn sep() -> gpui::Div {
    div().my(z(24.)).w_full().h(z(1.)).bg(t(idx::BORDER_SUBTLE))
}

fn hint(s: SharedString) -> gpui::Div {
    text(12., 16., FontWeight::MEDIUM, idx::TEXT_MUTED, s)
}

fn err(s: SharedString) -> gpui::Div {
    text(12., 16., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL, s)
}

fn with_placeholder(field: gpui::Div, empty: bool, placeholder: SharedString) -> gpui::Div {
    field.relative().when(empty, |d| {
        d.child(
            div()
                .absolute()
                .top_0()
                .left(z(12.))
                .right(z(12.))
                .h_full()
                .flex()
                .items_center()
                .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_MUTED, placeholder).truncate()),
        )
    })
}

fn input(guild: Id, key: &str, initial: &str, placeholder: SharedString, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let empty = super::value(guild, key, cx).map_or(initial.is_empty(), |v| v.is_empty());
    with_placeholder(super::field(guild, key, initial, cx), empty, placeholder)
}

fn area(guild: Id, key: &str, initial: &str, h: f32, placeholder: SharedString, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let empty = super::area_value(guild, key, cx).map_or(initial.is_empty(), |v| v.is_empty());
    super::area_field(guild, key, initial, 4, h, cx).when(empty, |d| {
        d.child(div().absolute().top(z(10.)).left(z(10.)).right(z(10.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_MUTED, placeholder)))
    })
}

type Pick = Rc<dyn Fn(String)>;

fn select(
    key: &'static str,
    options: Vec<(String, SharedString, Option<&'static str>)>,
    selected: &[String],
    multi: bool,
    placeholder: SharedString,
    pick: Pick,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let open = select_open(key);
    let shown: Vec<SharedString> = options.iter().filter(|o| selected.contains(&o.0)).map(|o| o.1.clone()).collect();
    let (value, col) = if shown.is_empty() { (placeholder, idx::TEXT_MUTED) } else { (SharedString::from(shown.join(", ")), idx::TEXT_STRONG) };
    let boxed = crate::ui::modals::settings_form::select_box(SharedString::from(format!("obe-sel-{key}")), "", open)
        .relative()
        .child(div().absolute().left(z(12.)).right(z(36.)).top_0().bottom_0().flex().items_center().child(text(16., 20., FontWeight::NORMAL, col, value).truncate()))
        .on_click(cx.listener(move |_, _, _, cx| {
            toggle_select(key);
            cx.notify();
        }));
    let mut wrap = div().relative().w_full().child(boxed);
    if open {
        let mut list = div()
            .id(SharedString::from(format!("obe-list-{key}")))
            .occlude()
            .max_h(z(40. * 6. + 8.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .bg(t(idx::BACKGROUND_SURFACE_HIGHER))
            .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.16), offset: gpui::point(z(0.), z(2.)), blur_radius: z(8.), spread_radius: z(0.), inset: false }])
            .flex()
            .flex_col();
        if options.is_empty() {
            list = list.child(div().h(z(40.)).px(z(8.)).flex().items_center().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("obe.no_options"))));
        }
        for (i, (id, label, ic)) in options.into_iter().enumerate() {
            let on = selected.contains(&id);
            let pick = pick.clone();
            list = list.child(
                div()
                    .id((key, i))
                    .flex_none()
                    .h(z(40.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .when(on, |d| d.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                    .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                    .cursor_pointer()
                    .when_some(ic, |d, ic| d.child(icon(ic, 16., idx::INTERACTIVE_TEXT_DEFAULT)))
                    .child(div().flex_1().min_w_0().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_STRONG, label).truncate()))
                    .when(on, |d| d.child(icon("check", 20., idx::TEXT_BRAND)))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        pick(id.clone());
                        if !multi {
                            OPEN_SELECT.with(|o| *o.borrow_mut() = None);
                        }
                        cx.notify();
                    })),
            );
        }
        wrap = wrap.child(deferred(div().absolute().top(z(44.)).left_0().w_full().child(list)).with_priority(40));
    }
    wrap.into_any_element()
}

fn channel_icon(c: &Chan) -> &'static str {
    ob::channel_icon(c.kind, c.public)
}

fn critical_link(id: &'static str, label: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t(idx::TEXT_FEEDBACK_CRITICAL))
        .hover(|s| s.underline())
        .child(label)
}

fn footer(remove: Option<gpui::Stateful<gpui::Div>>, save_ok: bool, save: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(div().flex_1().min_w_0().children(remove))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(button("obe-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
                    close();
                    cx.notify();
                })))
                .child(
                    button("obe-save", crate::t!("common.save"), BtnKind::Primary)
                        .when(!save_ok, |d| d.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed))
                        .when(save_ok, |d| {
                            d.on_click(cx.listener(move |a, _, _, cx| {
                                save(a, cx);
                                cx.notify();
                            }))
                        }),
                ),
        )
        .into_any_element()
}

fn close_action(cx: &mut Context<PhoenixApp>) -> crate::ui::modal::Action {
    let e = cx.entity();
    Rc::new(move |_, cx| {
        close();
        e.update(cx, |_, cx| cx.notify());
    })
}

struct Data {
    guild: Value,
    channels: Value,
    list: Vec<Chan>,
    home: Value,
}

fn data(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> Option<Data> {
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx)?;
    let ch = fetch(app, guild, ServerCall::Channels { guild: sf(guild) }, cx)?;
    let gid = str_of(&g, "id").to_owned();
    let list = ob::chans(&ch, &gid, ob::everyone_perms(&g));
    let home = ob::home_draft(guild).unwrap_or(Value::Null);
    Some(Data { guild: g, channels: ch, list, home })
}

pub fn modal(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let (g, ed) = current()?;
    if g != guild {
        return None;
    }
    let d = data(app, guild, cx)?;
    Some(match ed {
        Editor::Welcome { author } => welcome(app, guild, &d, author, cx),
        Editor::Task { index, channel, action, emoji, icon_path, icon_data, thumb_menu } => task(guild, &d, index, channel, action, emoji, icon_path, icon_data, thumb_menu, cx),
        Editor::Resource { index, channel, icon_path, icon_data } => resource(guild, &d, index, channel, icon_path, icon_data, cx),
        Editor::Answer { prompt, index, channels, roles, emoji, title_error } => answer(guild, &d, &prompt, index, channels, roles, emoji, title_error, cx),
        Editor::Unlock { review, selected } => unlock(&d, review, selected, cx),
        Editor::BoostUpsell => boost_upsell(cx),
    })
}

pub const DANGEROUS_PERMS: &[(u64, &str)] = &[
    (1 << 3, "Administrator"),
    (1 << 1, "Kick Members"),
    (1 << 2, "Ban Members"),
    (1 << 5, "Manage Server"),
    (1 << 4, "Manage Channels"),
    (1 << 28, "Manage Roles"),
    (1 << 13, "Manage Messages"),
    (1 << 30, "Manage Expressions"),
    (1 << 43, "Create Expressions"),
    (1 << 33, "Manage Events"),
    (1 << 44, "Create Events"),
    (1 << 40, "Timeout Members"),
    (1 << 17, "Mention @everyone"),
    (1 << 49, "Manage Official Messages"),
];

fn role_perm_bits(v: &Value) -> u64 {
    v.get("permissions")
        .and_then(|p| p.as_str().and_then(|s| s.parse().ok()).or_else(|| p.as_u64()))
        .unwrap_or(0)
}

pub fn dangerous_role_perms(guild: &Value, role_ids: &[String]) -> Option<Vec<&'static str>> {
    let roles = guild.get("roles").and_then(Value::as_array)?;
    let mask: u64 = DANGEROUS_PERMS.iter().fold(0, |acc, (b, _)| acc | b);
    for id in role_ids {
        let Some(role) = roles.iter().find(|r| str_of(r, "id") == id) else { continue };
        let bits = role_perm_bits(role);
        if bits & mask != 0 {
            let names: Vec<&'static str> = DANGEROUS_PERMS.iter().filter(|(b, _)| bits & b != 0).map(|(_, n)| *n).collect();
            if !names.is_empty() {
                return Some(names);
            }
        }
    }
    None
}

pub fn member_can_author(guild: &Value, user_id: &str, role_ids: &[Id]) -> bool {
    use crate::actions::{MANAGE_GUILD, MANAGE_ROLES};
    if !user_id.is_empty() && str_of(guild, "owner_id") == user_id {
        return true;
    }
    let mut perms = ob::everyone_perms(guild);
    if let Some(roles) = guild.get("roles").and_then(Value::as_array) {
        for rid in role_ids {
            let rstr = rid.0.to_string();
            if let Some(role) = roles.iter().find(|r| str_of(r, "id") == rstr) {
                perms |= role_perm_bits(role);
            }
        }
    }
    perms & ob::ADMINISTRATOR != 0 || (perms & MANAGE_GUILD != 0 && perms & MANAGE_ROLES != 0)
}

fn author_options(app: &PhoenixApp, d: &Data) -> Vec<(String, SharedString, Option<&'static str>)> {
    let gid = str_of(&d.guild, "id").parse::<u64>().ok().map(Id);
    let mut out: Vec<(String, SharedString, Option<&'static str>)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let push = |out: &mut Vec<(String, SharedString, Option<&'static str>)>, seen: &mut std::collections::HashSet<String>, id: String, label: SharedString| {
        if !id.is_empty() && seen.insert(id.clone()) {
            out.push((id, label, None));
        }
    };
    if let Some(g) = gid
        && let Some(members) = app.members.get(&g)
    {
        for m in members {
            let uid = m.user.0.to_string();
            if member_can_author(&d.guild, &uid, &m.roles) {
                let label = m
                    .nick
                    .clone()
                    .or_else(|| if m.user == app.me.id { Some(app.me.display_name.clone()) } else { app.users.get(&m.user).map(|u| u.display_name.clone()) })
                    .unwrap_or_else(|| SharedString::from(uid.clone()));
                push(&mut out, &mut seen, uid, label);
            }
        }
    }
    let owner = str_of(&d.guild, "owner_id").to_owned();
    if !owner.is_empty() && !seen.contains(&owner) {
        let name = owner
            .parse::<u64>()
            .ok()
            .and_then(|n| if n == app.me.id.0 { Some(app.me.display_name.clone()) } else { app.users.get(&Id(n)).map(|u| u.display_name.clone()) })
            .unwrap_or_else(|| SharedString::from(owner.clone()));
        push(&mut out, &mut seen, owner, name);
    }
    let me = app.me.id.0.to_string();
    if !seen.contains(&me) && ob::can_manage(app, &d.guild) {
        push(&mut out, &mut seen, me, app.me.display_name.clone());
    }
    out
}

fn welcome(app: &PhoenixApp, guild: Id, d: &Data, author: Option<String>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let initial = d.home.pointer("/welcome_message/message").and_then(Value::as_str).unwrap_or("").to_owned();
    let message = super::area_value(guild, "obe-welcome", cx).unwrap_or_else(|| initial.clone());
    let error = welcome_error(&message, author.is_some());
    let picked: Vec<String> = author.iter().cloned().collect();
    let pick: Pick = Rc::new(|id| update(|e| {
        if let Editor::Welcome { author } = e {
            *author = Some(id);
        }
    }));
    let body = div()
        .flex()
        .flex_col()
        .child(
            group()
                .child(heading(crate::t!("obe.author"), true))
                .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("obe.author_desc")))
                .child(select("author", author_options(app, d), &picked, false, crate::t!("obe.select"), pick, cx)),
        )
        .child(sep())
        .child(
            group()
                .pb(z(16.))
                .child(heading(crate::t!("obe.welcome_message"), false))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .child(text(14., 18., FontWeight::BOLD, idx::TEXT_DEFAULT, crate::t!("obe.tip")))
                        .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("obe.tip_body"))),
                )
                .child(area(guild, "obe-welcome", &initial, 200., crate::t!("obe.welcome_placeholder"), cx))
                .when_some(error, |dv, k| dv.child(err(crate::t!(k, n = WELCOME_MIN)))),
        );
    let ok = !message.is_empty() && error.is_none();
    let remove = critical_link("obe-remove", crate::t!("obe.remove")).on_click(cx.listener(move |_, _, _, cx| {
        ob::edit_home(guild, |h| {
            h["welcome_message"] = json!({ "author_ids": [], "message": "" });
        });
        close();
        cx.notify();
    }));
    let save = move |_: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
        let msg = clip(&super::area_value(guild, "obe-welcome", cx).unwrap_or_default(), WELCOME_MAX);
        let a = current().and_then(|(_, e)| if let Editor::Welcome { author } = e { author } else { None });
        ob::edit_home(guild, |h| {
            h["welcome_message"] = json!({ "author_ids": a.into_iter().collect::<Vec<_>>(), "message": msg });
        });
        close();
    };
    let footer = footer(Some(remove), ok, save, cx);
    crate::ui::modals::card_with(480., crate::t!("obe.welcome_title"), None, body.into_any_element(), Some(footer), close_action(cx))
}

fn prompt_thumb_image(is_task: bool, cx: &mut Context<PhoenixApp>) {
    let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(crate::t!("obe.thumb_upload_image")),
    });
    cx.spawn(async move |this, cx| {
        let Ok(Ok(Some(paths))) = rx.await else { return };
        let Some(path) = paths.into_iter().next() else { return };
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let mime = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => return,
        };
        let Ok(bytes) = std::fs::read(&path) else { return };
        use base64::Engine as _;
        let data_uri = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes));
        let _ = this.update(cx, |_, cx| {
            update(|e| match e {
                Editor::Task { emoji, icon_path, icon_data, thumb_menu, .. } if is_task => {
                    *emoji = Value::Null;
                    *icon_path = Some(path);
                    *icon_data = Some(data_uri);
                    *thumb_menu = false;
                }
                Editor::Resource { icon_path, icon_data, .. } if !is_task => {
                    *icon_path = Some(path);
                    *icon_data = Some(data_uri);
                }
                _ => {}
            });
            cx.notify();
        });
    })
    .detach();
}

#[allow(clippy::too_many_arguments)]
fn task(
    guild: Id,
    d: &Data,
    index: Option<usize>,
    channel: Option<String>,
    action: Option<u64>,
    emoji: Value,
    icon_path: Option<std::path::PathBuf>,
    _icon_data: Option<String>,
    thumb_menu: bool,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let existing = index.and_then(|i| d.home.get("new_member_actions")?.get(i).cloned());
    let initial = existing.as_ref().map(|a| str_of(a, "title").to_owned()).unwrap_or_default();
    let title = super::value(guild, "obe-task-title", cx).unwrap_or_else(|| initial.clone());
    let used: Vec<String> = d
        .home
        .get("new_member_actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(i, _)| Some(*i) != index)
        .map(|(_, a)| str_of(a, "channel_id").to_owned())
        .collect();
    let options: Vec<_> = d.list.iter().filter(|c| task_channel(c) && !used.contains(&c.id)).map(|c| (c.id.clone(), SharedString::from(c.name.clone()), Some(channel_icon(c)))).collect();
    let picked: Vec<String> = channel.iter().cloned().collect();
    let ch = channel.as_ref().and_then(|id| d.list.iter().find(|c| &c.id == id));
    let pick: Pick = Rc::new(|id| update(|e| {
        if let Editor::Task { channel, .. } = e {
            *channel = Some(id);
        }
    }));
    let chat_label = match ch.map(|c| c.kind) {
        Some(16) => crate::t!("obe.task_chat_media"),
        Some(15) => crate::t!("obe.task_chat_forum"),
        _ => crate::t!("obe.task_chat"),
    };
    let mut radios = group();
    for (value, label) in [(VIEW, crate::t!("obe.task_view")), (CHAT, chat_label)] {
        let allowed = ch.is_none_or(|c| action_allowed(value, c));
        let on = action == Some(value);
        radios = radios.child(
            div()
                .id(("obe-act", value as usize))
                .p(z(12.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(12.))
                .bg(t(if on { idx::BACKGROUND_MOD_STRONG } else { idx::BACKGROUND_MOD_NORMAL }))
                .when(!allowed, |dv| dv.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed))
                .when(allowed, |dv| {
                    dv.cursor_pointer().on_click(cx.listener(move |_, _, _, cx| {
                        update(|e| {
                            if let Editor::Task { action, .. } = e {
                                *action = Some(value);
                            }
                        });
                        cx.notify();
                    }))
                })
                .child(radio_indicator(on))
                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, label)),
        );
    }
    let action_err = match (action, ch) {
        (Some(a), Some(c)) if !action_allowed(a, c) => Some(if a == VIEW { crate::t!("obe.task_err_view") } else { crate::t!("obe.task_err_chat") }),
        _ => None,
    };
    let title_err = task_title_error(&title);
    let ok = !title.is_empty() && action.is_some() && channel.is_some() && !title_err && action_err.is_none();
    let has_thumb = !emoji.is_null() || icon_path.is_some();
    let thumb_btn = div()
        .id("obe-task-thumb")
        .relative()
        .cursor_pointer()
        .when(has_thumb, |dv| {
            dv.p(z(8.))
                .rounded(z(12.))
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .child(
                    div()
                        .size(z(56.))
                        .rounded(z(8.))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if let Some(p) = icon_path.clone() {
                            gpui::img(p).size(z(56.)).object_fit(gpui::ObjectFit::Cover).into_any_element()
                        } else {
                            ob::emoji_el(&emoji)
                        }),
                )
        })
        .when(!has_thumb, |dv| {
            dv.size(z(40.))
                .rounded(z(8.))
                .border_1()
                .border_color(t(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
                .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("image-plus", 20., idx::CONTROL_SECONDARY_TEXT_DEFAULT))
        })
        .on_click(cx.listener(|_, _, _, cx| {
            update(|e| {
                if let Editor::Task { thumb_menu, .. } = e {
                    *thumb_menu = !*thumb_menu;
                }
            });
            cx.notify();
        }));
    let mut thumb_wrap = div().relative().child(thumb_btn);
    if thumb_menu {
        let mut menu = div()
            .id("obe-task-thumb-menu")
            .occlude()
            .w(z(180.))
            .p(z(6.))
            .rounded(z(8.))
            .border_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .bg(t(idx::BACKGROUND_SURFACE_HIGHER))
            .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.24), offset: gpui::point(z(0.), z(4.)), blur_radius: z(12.), spread_radius: z(0.), inset: false }])
            .flex()
            .flex_col()
            .gap(z(2.))
            .child(
                div()
                    .id("obe-thumb-emoji")
                    .px(z(8.))
                    .py(z(6.))
                    .rounded(z(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(icon("emoji", 16., idx::INTERACTIVE_TEXT_DEFAULT))
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("obe.thumb_choose_emoji")))
                    .on_click(cx.listener(|a, _, w, cx| {
                        update(|e| {
                            if let Editor::Task { thumb_menu, .. } = e {
                                *thumb_menu = false;
                            }
                        });
                        crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::GuideTask, w, cx);
                    })),
            )
            .child(
                div()
                    .id("obe-thumb-upload")
                    .px(z(8.))
                    .py(z(6.))
                    .rounded(z(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(icon("chat/upload", 16., idx::INTERACTIVE_TEXT_DEFAULT))
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("obe.thumb_upload_image")))
                    .on_click(cx.listener(|_, _, _, cx| {
                        prompt_thumb_image(true, cx);
                    })),
            );
        if has_thumb {
            menu = menu.child(
                div()
                    .id("obe-thumb-clear")
                    .px(z(8.))
                    .py(z(6.))
                    .rounded(z(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(icon("chat/trash", 16., idx::TEXT_FEEDBACK_CRITICAL))
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_FEEDBACK_CRITICAL, crate::t!("obe.remove")))
                    .on_click(cx.listener(|_, _, _, cx| {
                        update(|e| {
                            if let Editor::Task { emoji, icon_path, icon_data, thumb_menu, .. } = e {
                                *emoji = Value::Null;
                                *icon_path = None;
                                *icon_data = None;
                                *thumb_menu = false;
                            }
                        });
                        cx.notify();
                    })),
            );
        }
        thumb_wrap = thumb_wrap.child(deferred(div().absolute().top(z(44.)).right_0().child(menu)).with_priority(45));
    }
    let thumb = div()
        .flex()
        .justify_between()
        .items_center()
        .child(group().gap(z(0.)).child(heading(crate::t!("obe.task_thumb"), false)).child(hint(crate::t!("obe.thumb_hint"))))
        .child(thumb_wrap);
    let body = div()
        .flex()
        .flex_col()
        .child(
            group()
                .child(heading(crate::t!("obe.task_what"), true))
                .child(input(guild, "obe-task-title", &initial, crate::t!("obe.task_placeholder"), cx))
                .when(title_err, |dv| dv.child(err(crate::t!("obe.task_err_title", n = TASK_TITLE_MIN)))),
        )
        .child(sep())
        .child(
            group()
                .child(heading(crate::t!("obe.task_where"), true))
                .child(select("task-channel", options, &picked, false, crate::t!("obe.select"), pick, cx))
                .child(hint(crate::t!("obe.task_where_hint"))),
        )
        .child(sep())
        .child(thumb)
        .child(sep())
        .child(group().pb(z(16.)).child(heading(crate::t!("obe.task_done"), true)).child(radios).children(action_err.map(err)));
    let remove = index.map(|i| {
        critical_link("obe-remove", crate::t!("obe.remove")).on_click(cx.listener(move |_, _, _, cx| {
            ob::edit_home(guild, |h| {
                if let Some(a) = h.get_mut("new_member_actions").and_then(Value::as_array_mut)
                    && i < a.len()
                {
                    a.remove(i);
                }
            });
            close();
            cx.notify();
        }))
    });
    let prev = existing.clone();
    let save = move |_: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
        let Some((_, Editor::Task { channel: Some(ch), action: Some(act), emoji, icon_data, .. })) = current() else { return };
        let title = clip(&super::value(guild, "obe-task-title", cx).unwrap_or_default(), TASK_TITLE_MAX);
        let mut entry = json!({ "channel_id": ch, "action_type": act, "title": title, "description": "", "emoji": emoji });
        if let Some(icon) = icon_data {
            entry["icon"] = json!(icon);
        } else if let Some(icon) = prev.as_ref().and_then(|p| p.get("icon")).filter(|i| !i.is_null()) {
            entry["icon"] = icon.clone();
        }
        ob::edit_home(guild, |h| {
            if !h.get("new_member_actions").is_some_and(Value::is_array) {
                h["new_member_actions"] = json!([]);
            }
            if let Some(a) = h.get_mut("new_member_actions").and_then(Value::as_array_mut) {
                match index {
                    Some(i) if i < a.len() => a[i] = entry,
                    _ => a.push(entry),
                }
            }
        });
        close();
    };
    let footer = footer(remove, ok, save, cx);
    crate::ui::modals::card_with(480., crate::t!("obe.task_title"), None, body.into_any_element(), Some(footer), close_action(cx))
}

fn resource(
    guild: Id,
    d: &Data,
    index: Option<usize>,
    channel: Option<String>,
    icon_path: Option<std::path::PathBuf>,
    _icon_data: Option<String>,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let existing = index.and_then(|i| d.home.get("resource_channels")?.get(i).cloned());
    let init_title = existing.as_ref().map(|r| str_of(r, "title").to_owned()).unwrap_or_default();
    let init_desc = existing.as_ref().map(|r| str_of(r, "description").to_owned()).unwrap_or_default();
    let title = super::value(guild, "obe-res-title", cx).unwrap_or_else(|| init_title.clone());
    let used: Vec<String> = d
        .home
        .get("resource_channels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(i, _)| Some(*i) != index)
        .map(|(_, r)| str_of(r, "channel_id").to_owned())
        .collect();
    let options: Vec<_> = d.list.iter().filter(|c| resource_channel(c) && !used.contains(&c.id)).map(|c| (c.id.clone(), SharedString::from(c.name.clone()), Some(channel_icon(c)))).collect();
    let picked: Vec<String> = channel.iter().cloned().collect();
    let pick: Pick = Rc::new(|id| update(|e| {
        if let Editor::Resource { channel, .. } = e {
            *channel = Some(id);
        }
    }));
    let existing_icon_url = existing.as_ref().and_then(|r| {
        let ch_id = str_of(r, "channel_id");
        let ic = r.get("icon").and_then(Value::as_str)?;
        (!ch_id.is_empty() && !ic.is_empty() && !ic.starts_with("data:")).then(|| SharedString::from(format!("https://cdn.discordapp.com/channel-icons/{ch_id}/{ic}.webp?size=96")))
    });
    let mut thumb_box = div()
        .id("obe-res-thumb")
        .group("obe-res-thumb")
        .relative()
        .overflow_hidden()
        .size(z(72.))
        .rounded(z(4.))
        .border_1()
        .border_color(t(idx::BACKGROUND_BASE_LOWEST))
        .bg(t(idx::BACKGROUND_MOD_NORMAL))
        .cursor_pointer()
        .on_click(cx.listener(|_, _, _, cx| {
            prompt_thumb_image(false, cx);
        }));
    if let Some(p) = icon_path {
        thumb_box = thumb_box.child(gpui::img(p).size(z(72.)).object_fit(gpui::ObjectFit::Cover));
    } else if let Some(u) = existing_icon_url {
        thumb_box = thumb_box.child(gpui::img(u).size(z(72.)).object_fit(gpui::ObjectFit::Cover));
    }
    thumb_box = thumb_box.child(
        div()
            .absolute()
            .top(z(24.))
            .left(z(24.))
            .size(z(24.))
            .rounded(z(4.))
            .bg(t(idx::BACKGROUND_MOD_NORMAL))
            .group_hover("obe-res-thumb", |s| s.bg(t(idx::BACKGROUND_SURFACE_HIGH)))
            .child(icon("chat/upload", 24., idx::INTERACTIVE_TEXT_DEFAULT)),
    );
    let thumb = div()
        .flex()
        .justify_between()
        .child(group().gap(z(0.)).child(heading(crate::t!("obe.res_thumb"), false)).child(hint(crate::t!("obe.thumb_hint"))))
        .child(thumb_box);
    let body = div()
        .flex()
        .flex_col()
        .child(
            group()
                .child(heading(crate::t!("obe.res_channel"), true))
                .child(select("res-channel", options, &picked, false, crate::t!("obe.select"), pick, cx))
                .child(hint(crate::t!("obe.res_channel_hint"))),
        )
        .child(sep())
        .child(group().child(heading(crate::t!("obe.res_name"), true)).child(input(guild, "obe-res-title", &init_title, crate::t!("obe.res_name_placeholder"), cx)))
        .child(sep())
        .child(group().child(heading(crate::t!("obe.res_desc"), false)).child(area(guild, "obe-res-desc", &init_desc, 80., crate::t!("obe.res_desc_placeholder"), cx)))
        .child(sep())
        .child(thumb.pb(z(16.)));
    let ok = !title.is_empty() && channel.is_some();
    let remove = index.map(|i| {
        critical_link("obe-remove", crate::t!("obe.remove")).on_click(cx.listener(move |_, _, _, cx| {
            ob::edit_home(guild, |h| {
                if let Some(a) = h.get_mut("resource_channels").and_then(Value::as_array_mut)
                    && i < a.len()
                {
                    a.remove(i);
                }
            });
            close();
            cx.notify();
        }))
    });
    let prev = existing.clone();
    let save = move |_: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
        let Some((_, Editor::Resource { channel: Some(ch), icon_data, .. })) = current() else { return };
        let title = clip(&super::value(guild, "obe-res-title", cx).unwrap_or_default(), RES_TITLE_MAX);
        let desc = clip(&super::area_value(guild, "obe-res-desc", cx).unwrap_or_default(), RES_DESC_MAX);
        let mut entry = json!({ "channel_id": ch, "title": title, "description": desc, "emoji": Value::Null });
        if let Some(icon) = icon_data {
            entry["icon"] = json!(icon);
        } else if let Some(icon) = prev.as_ref().and_then(|p| p.get("icon")).filter(|i| !i.is_null()) {
            entry["icon"] = icon.clone();
        }
        ob::edit_home(guild, |h| {
            if !h.get("resource_channels").is_some_and(Value::is_array) {
                h["resource_channels"] = json!([]);
            }
            if let Some(a) = h.get_mut("resource_channels").and_then(Value::as_array_mut) {
                match index {
                    Some(i) if i < a.len() => a[i] = entry,
                    _ => a.push(entry),
                }
            }
        });
        close();
    };
    let footer = footer(remove, ok, save, cx);
    crate::ui::modals::card_with(480., crate::t!("obe.res_title"), None, body.into_any_element(), Some(footer), close_action(cx))
}

#[allow(clippy::too_many_arguments)]
fn answer(guild: Id, d: &Data, prompt: &str, index: Option<usize>, channels: Vec<String>, roles: Vec<String>, emoji: Value, title_error: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let draft = ob::prompts_draft(guild);
    let (n, p) = draft.iter().enumerate().find(|(_, p)| p.id == prompt).map(|(i, p)| (i, p.clone())).unwrap_or_default();
    let existing = index.and_then(|i| p.options.get(i).cloned());
    let init_title = existing.as_ref().map(|o| o.title.clone()).unwrap_or_default();
    let init_desc = existing.as_ref().map(|o| o.description.clone()).unwrap_or_default();
    let channel_opts: Vec<_> = d.list.iter().filter(|c| c.kind != 4 && c.public).map(|c| (c.id.clone(), SharedString::from(c.name.clone()), Some(channel_icon(c)))).collect();
    let gid = str_of(&d.guild, "id").to_owned();
    let role_opts: Vec<_> = d
        .guild
        .get("roles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|r| str_of(r, "id") != gid && !r.get("managed").and_then(Value::as_bool).unwrap_or(false))
        .map(|r| (str_of(r, "id").to_owned(), SharedString::from(str_of(r, "name").to_owned()), None))
        .collect();
    let pick_ch: Pick = Rc::new(|id| update(|e| {
        if let Editor::Answer { channels, .. } = e {
            if let Some(i) = channels.iter().position(|x| *x == id) {
                channels.remove(i);
            } else {
                channels.push(id);
            }
        }
    }));
    let pick_role: Pick = Rc::new(|id| update(|e| {
        if let Editor::Answer { roles, .. } = e {
            if let Some(i) = roles.iter().position(|x| *x == id) {
                roles.remove(i);
            } else {
                roles.push(id);
            }
        }
    }));
    let subtitle = if p.title.is_empty() { crate::t!("obe.answer_default_q") } else { SharedString::from(p.title.clone()) };
    let dangerous = dangerous_role_perms(&d.guild, &roles);
    let has_emoji = !emoji.is_null();
    let emoji_box = div()
        .id("obe-answer-emoji-wrap")
        .group("obe-answer-emoji-wrap")
        .relative()
        .flex_none()
        .child(
            div()
                .id("obe-answer-emoji")
                .relative()
                .flex_none()
                .overflow_hidden()
                .size(z(40.))
                .rounded(z(4.))
                .border_1()
                .border_color(t(idx::BACKGROUND_BASE_LOWEST))
                .bg(t(idx::BACKGROUND_MOD_NORMAL))
                .hover(|s| s.bg(t(idx::BACKGROUND_SURFACE_HIGH)))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .child(if !has_emoji { icon("emoji", 20., idx::INTERACTIVE_TEXT_DEFAULT).into_any_element() } else { ob::emoji_el(&emoji) })
                .on_click(cx.listener(|a, _, w, cx| {
                    crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::GuideAnswer, w, cx);
                })),
        )
        .when(has_emoji, |dv| {
            dv.child(
                div()
                    .id("obe-answer-emoji-clear")
                    .absolute()
                    .top(z(-6.))
                    .right(z(-6.))
                    .size(z(18.))
                    .rounded_full()
                    .bg(t(idx::BACKGROUND_SURFACE_HIGHEST))
                    .border_1()
                    .border_color(t(idx::BORDER_SUBTLE))
                    .invisible()
                    .group_hover("obe-answer-emoji-wrap", |s| s.visible())
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("close", 10., idx::INTERACTIVE_TEXT_DEFAULT))
                    .on_click(cx.listener(|_, _, _, cx| {
                        update(|e| {
                            if let Editor::Answer { emoji, .. } = e {
                                *emoji = Value::Null;
                            }
                        });
                        cx.notify();
                    })),
            )
        });
    let body = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .pb(z(16.))
        .child(
            group()
                .child(heading(crate::t!("obe.answer_label"), true))
                .child(input(guild, "obe-ans-title", &init_title, crate::t!("obe.answer_placeholder"), cx))
                .when(title_error, |dv| dv.child(err(crate::t!("obe.answer_err_title"))))
                .child(input(guild, "obe-ans-desc", &init_desc, crate::t!("obe.answer_desc_placeholder"), cx)),
        )
        .child(div().w_full().h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
        .child(
            group()
                .child(heading(crate::t!("obe.answer_assign"), false))
                .child(select("ans-channels", channel_opts, &channels, true, crate::t!("obe.answer_channels"), pick_ch, cx))
                .child(hint(crate::t!("obe.answer_channels_hint")))
                .child(select("ans-roles", role_opts, &roles, true, crate::t!("obe.answer_roles"), pick_role, cx))
                .child(hint(crate::t!("obe.answer_roles_hint")))
                .when_some(dangerous, |dv, perms| {
                    dv.child(
                        div()
                            .mt(z(8.))
                            .p(z(12.))
                            .rounded(z(8.))
                            .border_1()
                            .border_color(t(idx::RED_400))
                            .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                            .flex()
                            .items_start()
                            .gap(z(8.))
                            .child(div().flex_none().pt(z(1.)).child(icon("circle-error", 20., idx::TEXT_FEEDBACK_CRITICAL)))
                            .child(
                                div().flex_1().min_w_0().child(text(
                                    14.,
                                    18.,
                                    FontWeight::NORMAL,
                                    idx::TEXT_DEFAULT,
                                    crate::t!("obe.answer_dangerous_role", permissions = perms.join(", ")),
                                )),
                            ),
                    )
                }),
        )
        .child(div().w_full().h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
        .child(
            div()
                .flex()
                .justify_between()
                .child(group().gap(z(4.)).child(heading(crate::t!("obe.answer_emoji"), false)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("obe.answer_emoji_desc"))))
                .child(emoji_box),
        );
    let pid = prompt.to_owned();
    let remove = critical_link("obe-remove", crate::t!("obe.remove")).on_click(cx.listener({
        let pid = pid.clone();
        move |_, _, _, cx| {
            if let Some(i) = index {
                ob::edit_prompt(guild, &pid, |p| {
                    if i < p.options.len() {
                        p.options.remove(i);
                    }
                });
            }
            close();
            cx.notify();
        }
    }));
    let save = move |_: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
        let title = clip(&super::value(guild, "obe-ans-title", cx).unwrap_or_default(), ANSWER_TITLE_MAX);
        if title.is_empty() {
            update(|e| {
                if let Editor::Answer { title_error, .. } = e {
                    *title_error = true;
                }
            });
            return;
        }
        let desc = clip(&super::value(guild, "obe-ans-desc", cx).unwrap_or_default(), ANSWER_DESC_MAX);
        let Some((_, Editor::Answer { mut channels, mut roles, emoji, .. })) = current() else { return };
        channels.sort();
        roles.sort();
        ob::edit_prompt(guild, &pid, |p| {
            let id = index.and_then(|i| p.options.get(i)).map(|o| o.id.clone()).unwrap_or_else(ob::now_id);
            let o = ob::Opt { id, title, description: desc, emoji, role_ids: roles, channel_ids: channels };
            match index {
                Some(i) if i < p.options.len() => p.options[i] = o,
                _ => p.options.push(o),
            }
        });
        close();
    };
    let footer = footer(Some(remove), true, save, cx);
    crate::ui::modals::card_with(480., crate::t!("obe.answer_title", n = n + 1), Some(subtitle), body.into_any_element(), Some(footer), close_action(cx))
}

fn rich(parts: &[(SharedString, bool)], size: f32, lh: f32, base: usize, strong: usize, strong_weight: FontWeight) -> AnyElement {
    let mut full = String::new();
    let mut hl = Vec::new();
    for (s, em) in parts {
        let start = full.len();
        full.push_str(s);
        if *em {
            hl.push((start..full.len(), gpui::HighlightStyle { color: Some(t(strong)), font_weight: Some(strong_weight), ..Default::default() }));
        }
    }
    div()
        .text_size(z(size))
        .line_height(z(lh))
        .text_color(t(base))
        .child(gpui::StyledText::new(SharedString::from(full)).with_highlights(hl))
        .into_any_element()
}

fn unlock(d: &Data, review: bool, selected: Vec<String>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if !review {
        let item = |emoji: &'static str, a: &'static str, em: &'static str, b: &'static str| {
            div()
                .flex()
                .gap(z(16.))
                .child(div().flex_none().size(z(40.)).p(z(8.)).rounded_full().bg(t(idx::BACKGROUND_BASE_LOWEST)).flex().items_center().justify_center().child(text(20., 24., FontWeight::NORMAL, idx::TEXT_DEFAULT, emoji)))
                .child(div().flex_1().min_w_0().child(rich(&[(crate::t!(a), false), (crate::t!(em), true), (crate::t!(b), false)], 16., 20., idx::TEXT_MUTED, idx::TEXT_STRONG, FontWeight::MEDIUM)))
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(z(24.))
            .pb(z(16.))
            .child(item("\u{1F440}", "obe.unlock_b1", "obe.unlock_b1_em", "obe.unlock_b1_end"))
            .child(item("\u{1F929}", "obe.unlock_b2", "obe.unlock_b2_em", "obe.unlock_b2_end"))
            .child(item("\u{1F4DA}", "obe.unlock_b3", "obe.unlock_b3_em", "obe.unlock_b3_end"));
        let footer = div()
            .w_full()
            .flex()
            .justify_end()
            .gap(z(8.))
            .child(button("obe-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
                close();
                cx.notify();
            })))
            .child(button("obe-review", crate::t!("obe.unlock_review"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| {
                update(|e| {
                    if let Editor::Unlock { review, .. } = e {
                        *review = true;
                    }
                });
                cx.notify();
            })));
        return crate::ui::modals::card_with(440., crate::t!("obe.unlock_title"), None, body.into_any_element(), Some(footer.into_any_element()), close_action(cx));
    }
    let private: Vec<&Chan> = d.list.iter().filter(|c| c.kind != 4 && c.private).collect();
    let all = !private.is_empty() && private.iter().all(|c| selected.contains(&c.id));
    let ids: Vec<String> = private.iter().map(|c| c.id.clone()).collect();
    let mut table = div().id("obe-priv-list").max_h(z(360.)).overflow_y_scroll().mb(z(8.)).rounded(z(8.)).bg(t(idx::BACKGROUND_BASE_LOWER)).flex().flex_col();
    let n = private.len();
    for (i, c) in private.iter().enumerate() {
        let on = selected.contains(&c.id);
        let id = c.id.clone();
        table = table.child(
            div()
                .id(("obe-priv", i))
                .px(z(16.))
                .py(z(12.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                .child(
                    div()
                        .h(z(32.))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .child(div().mr(z(4.)).child(icon(channel_icon(c), 16., idx::TEXT_DEFAULT)))
                        .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, c.name.clone()).truncate()),
                )
                .child(crate::ui::community::checkbox(on))
                .on_click(cx.listener(move |_, _, _, cx| {
                    update(|e| {
                        if let Editor::Unlock { selected, .. } = e {
                            if let Some(i) = selected.iter().position(|x| *x == id) {
                                selected.remove(i);
                            } else {
                                selected.push(id.clone());
                            }
                        }
                    });
                    cx.notify();
                })),
        );
        if i + 1 < n {
            table = table.child(div().mx(z(16.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)));
        }
    }
    let head = div()
        .mt(z(20.))
        .mb(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .child(text(14., 16., FontWeight::SEMIBOLD, idx::TEXT_MUTED, crate::t!("obe.unlock_count", n = n)))
        .child(
            div()
                .id("obe-unlock-all")
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .child(crate::ui::community::checkbox(all))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!("obe.unlock_all")))
                .on_click(cx.listener(move |_, _, _, cx| {
                    let ids = ids.clone();
                    update(|e| {
                        if let Editor::Unlock { selected, .. } = e {
                            *selected = if all { Vec::new() } else { ids };
                        }
                    });
                    cx.notify();
                })),
        );
    let explainer = div().mt(z(8.)).mb(z(24.)).child(rich(
        &[
            (crate::t!("obe.unlock_ex_a"), false),
            (SharedString::from("@everyone"), true),
            (crate::t!("obe.unlock_ex_b"), false),
            (crate::t!("obe.unlock_ex_view"), false),
            (crate::t!("obe.unlock_ex_and"), false),
            (crate::t!("obe.unlock_ex_connect"), false),
            (crate::t!("obe.unlock_ex_end"), false),
        ],
        12.,
        16.,
        idx::TEXT_MUTED,
        idx::INTERACTIVE_TEXT_ACTIVE,
        FontWeight::MEDIUM,
    ));
    let body = div().flex().flex_col().child(head).child(table).child(explainer);
    let gid = str_of(&d.guild, "id").to_owned();
    let raw = d.channels.clone();
    let kinds: Vec<(String, u64)> = d.list.iter().map(|c| (c.id.clone(), c.kind)).collect();
    let ok = !selected.is_empty();
    let footer = div()
        .w_full()
        .flex()
        .justify_end()
        .gap(z(8.))
        .child(button("obe-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
            close();
            cx.notify();
        })))
        .child(
            button("obe-unlock-save", crate::t!("obe.unlock_save"), BtnKind::Primary)
                .when(!ok, |dv| dv.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed))
                .when(ok, |dv| {
                    dv.on_click(cx.listener(move |a, _, _, cx| {
                        for id in &selected {
                            let Some(ch) = raw.as_array().and_then(|l| l.iter().find(|c| str_of(c, "id") == id)) else { continue };
                            let kind = kinds.iter().find(|(k, _)| k == id).map_or(0, |(_, k)| *k);
                            let body = access_overwrite(ch, &gid, kind);
                            if let (Ok(c), Ok(g)) = (id.parse::<u64>(), gid.parse::<u64>()) {
                                a.blocked_write(crate::actions::guard::GuardedWrite::put_channel_overwrite(c, g, body), cx);
                            }
                        }
                        close();
                        cx.notify();
                    }))
                }),
        );
    crate::ui::modals::card_with(480., crate::t!("obe.unlock_review_title"), Some(crate::t!("obe.unlock_review_sub")), body.into_any_element(), Some(footer.into_any_element()), close_action(cx))
}

fn boost_upsell(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let body = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(16.))
        .py(z(16.))
        .child(
            div()
                .size(z(64.))
                .rounded_full()
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("boost", 32., idx::GUILD_BOOSTING_PINK)),
        )
        .child(div().text_center().child(text(14., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("obe.boost_upsell_body"))));
    let footer = div()
        .w_full()
        .flex()
        .justify_end()
        .gap(z(8.))
        .child(button("obe-boost-close", crate::t!("common.close"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
            close();
            cx.notify();
        })))
        .child(
            crate::ui::controls::button_icon("obe-boost-cta", "boost", crate::t!("obe.boost_upsell_cta"), BtnKind::Expressive).on_click(cx.listener(|_, _, _, cx| {
                close();
                cx.notify();
            })),
        );
    crate::ui::modals::card_with(440., crate::t!("obe.boost_upsell_title"), None, body.into_any_element(), Some(footer.into_any_element()), close_action(cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chan(id: &str, kind: u64, public: bool, chattable: bool) -> Chan {
        Chan { id: id.into(), name: id.into(), kind, parent: None, position: 0, public, private: !public, chattable }
    }

    #[test]
    fn welcome_validation_matches_the_modal() {
        assert_eq!(welcome_error("", false), None);
        assert_eq!(welcome_error("hey", false), Some("obe.welcome_err_both"));
        assert_eq!(welcome_error("hello there", false), Some("obe.welcome_err_author"));
        assert_eq!(welcome_error("hey", true), Some("obe.welcome_err_short"));
        assert_eq!(welcome_error("hello there", true), None);
    }

    #[test]
    fn task_and_resource_channels_follow_the_client_filters() {
        let text = chan("1", 0, true, true);
        let readonly = chan("2", 0, true, false);
        let hidden = chan("3", 0, false, false);
        let forum = chan("4", 15, true, true);
        let voice = chan("5", 2, true, true);
        assert!(task_channel(&text) && task_channel(&forum) && task_channel(&readonly));
        assert!(!task_channel(&hidden) && !task_channel(&voice));
        assert!(resource_channel(&readonly));
        assert!(!resource_channel(&text) && !resource_channel(&hidden) && !resource_channel(&forum));
        assert!(action_allowed(VIEW, &readonly) && !action_allowed(CHAT, &readonly));
        assert!(task_title_error("short") && !task_title_error("") && !task_title_error("long enough"));
    }

    #[test]
    fn unlock_grants_view_and_connect_on_the_everyone_overwrite() {
        let ch = json!({ "id": "9", "permission_overwrites": [{ "id": "5", "type": 0, "allow": "0", "deny": "1049600" }] });
        assert_eq!(access_overwrite(&ch, "5", 2), json!({ "id": "5", "type": 0, "allow": "1049600", "deny": "0" }));
        let text = json!({ "id": "9", "permission_overwrites": [] });
        assert_eq!(access_overwrite(&text, "5", 0), json!({ "id": "5", "type": 0, "allow": "1024", "deny": "0" }));
    }

    #[test]
    fn home_body_is_the_client_serializer() {
        let list = vec![chan("10", 0, true, false)];
        let home = json!({
            "enabled": false,
            "welcome_message": { "author_ids": ["7"], "message": "Welcome aboard" },
            "new_member_actions": [
                { "channel_id": "10", "action_type": 0, "title": "Read it", "description": "", "emoji": { "name": "👋" } },
                { "channel_id": "99", "action_type": 1, "title": "Gone", "description": "" }
            ],
            "resource_channels": [{ "channel_id": "10", "title": "Rules", "description": "", "icon": "abc" }]
        });
        let body = home_body("5", &home, &list);
        assert_eq!(
            body,
            json!({
                "guild_id": "5",
                "welcome_message": { "author_ids": ["7"], "message": "Welcome aboard" },
                "new_member_actions": [{ "channel_id": "10", "action_type": 0, "title": "Read it", "description": "", "emoji": { "name": "👋" } }],
                "resource_channels": [{ "channel_id": "10", "title": "Rules", "description": "", "emoji": {}, "icon": "abc" }],
                "enabled": false
            })
        );
    }

    #[test]
    fn member_author_eligibility_matches_client_569944() {
        let g = json!({
            "id": "1",
            "owner_id": "100",
            "roles": [
                { "id": "1", "permissions": "0" },
                { "id": "10", "permissions": (1u64 << 3).to_string() },
                { "id": "11", "permissions": (1u64 << 5).to_string() },
                { "id": "12", "permissions": (1u64 << 28).to_string() }
            ]
        });
        assert!(member_can_author(&g, "100", &[]));
        assert!(member_can_author(&g, "200", &[Id(10)]));
        assert!(!member_can_author(&g, "201", &[Id(11)]));
        assert!(!member_can_author(&g, "202", &[Id(12)]));
        assert!(member_can_author(&g, "203", &[Id(11), Id(12)]));
    }

    #[test]
    fn dangerous_role_permissions_detected() {
        let g = json!({
            "id": "1",
            "roles": [
                { "id": "10", "permissions": "0" },
                { "id": "11", "permissions": ((1u64 << 1) | (1u64 << 2)).to_string() }
            ]
        });
        assert_eq!(dangerous_role_perms(&g, &["10".into()]), None);
        assert_eq!(dangerous_role_perms(&g, &["10".into(), "11".into()]), Some(vec!["Kick Members", "Ban Members"]));
    }
}
