use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*,
};
use phoenix_api::rest::{account::GuardedWrite, guild_server::ServerCall};
use serde_json::{Value, json};

use super::{fetch, icon, sf, str_of, t, text};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button, button_sm, checkbox_option},
};

pub const VIEW_CHANNEL: u64 = 1 << 10;
pub const SEND_MESSAGES: u64 = 1 << 11;
pub const SEND_IN_THREADS: u64 = 1 << 38;
pub const ADMINISTRATOR: u64 = 1 << 3;
pub const CONNECT: u64 = 1 << 20;
pub const MAX_PREJOIN: usize = 4;
pub const MAX_OPTIONS: usize = 50;
pub const STEPS: usize = 5;
pub const PREVIEW_NEW: &str = "https://discord.com/assets/d16043e709602f96.png";
pub const PREVIEW_EXISTING: &str = "https://discord.com/assets/47c38ca77bf70eb5.png";

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Opt {
    pub id: String,
    pub title: String,
    pub description: String,
    pub emoji: Value,
    pub role_ids: Vec<String>,
    pub channel_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Prompt {
    pub id: String,
    pub title: String,
    pub options: Vec<Opt>,
    pub single_select: bool,
    pub required: bool,
    pub in_onboarding: bool,
    pub kind: u64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PromptErr {
    pub title: Option<&'static str>,
    pub options: Option<&'static str>,
    pub config: bool,
    pub option_errs: Vec<Option<&'static str>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chan {
    pub id: String,
    pub name: String,
    pub kind: u64,
    pub parent: Option<String>,
    pub position: i64,
    pub public: bool,
    pub private: bool,
    pub chattable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Draft {
    pub defaults: Vec<String>,
    pub prompts: Vec<Prompt>,
    pub orig_defaults: Vec<String>,
    pub orig_prompts: Vec<Prompt>,
    pub edited: bool,
}

thread_local!(static DRAFTS: RefCell<HashMap<Id, Draft>> = RefCell::new(HashMap::new()));

fn ids(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_owned))
        .collect()
}

pub fn parse_prompts(ob: &Value) -> Vec<Prompt> {
    ob.get("prompts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|p| Prompt {
            id: str_of(p, "id").to_owned(),
            title: str_of(p, "title").to_owned(),
            options: p
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|o| Opt {
                    id: str_of(o, "id").to_owned(),
                    title: str_of(o, "title").to_owned(),
                    description: str_of(o, "description").to_owned(),
                    emoji: o.get("emoji").cloned().unwrap_or(Value::Null),
                    role_ids: ids(o.get("role_ids")),
                    channel_ids: ids(o.get("channel_ids")),
                })
                .collect(),
            single_select: p
                .get("single_select")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            required: p.get("required").and_then(Value::as_bool).unwrap_or(false),
            in_onboarding: p
                .get("in_onboarding")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            kind: p.get("type").and_then(Value::as_u64).unwrap_or(0),
        })
        .collect()
}

pub fn prompt_json(p: &Prompt) -> Value {
    json!({
        "id": p.id,
        "options": p.options.iter().map(|o| {
            let e = &o.emoji;
            json!({
                "id": o.id,
                "channel_ids": o.channel_ids,
                "role_ids": o.role_ids,
                "emoji": e,
                "emoji_id": e.get("id"),
                "emoji_name": e.get("name"),
                "emoji_animated": e.get("animated"),
                "title": o.title,
                "description": o.description,
            })
        }).collect::<Vec<_>>(),
        "title": p.title,
        "single_select": p.single_select,
        "required": p.required,
        "in_onboarding": p.in_onboarding,
        "type": p.kind,
    })
}

pub fn defaults_body(defaults: &[String], prompts: &[Prompt], advanced: bool) -> Value {
    let mut b = json!({ "default_channel_ids": defaults });
    if advanced {
        b["prompts"] = prompts.iter().map(prompt_json).collect();
    }
    b
}

pub fn prompts_body(prompts: &[Prompt]) -> Value {
    json!({ "prompts": prompts.iter().map(prompt_json).collect::<Vec<_>>() })
}

pub fn new_prompt(id: String, in_onboarding: bool) -> Prompt {
    Prompt {
        id,
        in_onboarding,
        ..Default::default()
    }
}

pub fn validate(
    prompts: &[Prompt],
    roles: &HashSet<String>,
    channels: &HashSet<String>,
) -> Vec<Option<PromptErr>> {
    let prejoin = prompts.iter().filter(|p| p.in_onboarding).count();
    prompts
        .iter()
        .map(|p| {
            let mut e = PromptErr::default();
            let mut bad = false;
            if p.title.is_empty() {
                e.title = Some("gs2.obw.err_title");
                bad = true;
            }
            if p.options.is_empty() {
                e.options = Some("gs2.obw.err_options");
                bad = true;
            }
            if p.in_onboarding && prejoin > MAX_PREJOIN {
                e.config = true;
                bad = true;
            }
            e.option_errs = p
                .options
                .iter()
                .map(|o| option_error(prompts, p, o, roles, channels))
                .collect();
            bad |= e.option_errs.iter().any(Option::is_some);
            bad.then_some(e)
        })
        .collect()
}

fn option_error(
    prompts: &[Prompt],
    p: &Prompt,
    o: &Opt,
    roles: &HashSet<String>,
    channels: &HashSet<String>,
) -> Option<&'static str> {
    if p.single_select {
        let mine: HashSet<&String> = o.role_ids.iter().collect();
        for other in prompts.iter().filter(|q| q.id != p.id) {
            if other
                .options
                .iter()
                .any(|x| x.role_ids.iter().any(|r| mine.contains(r)))
            {
                return Some("gs2.obw.err_single");
            }
        }
    }
    let r = o.role_ids.iter().filter(|r| roles.contains(*r)).count();
    let c = o
        .channel_ids
        .iter()
        .filter(|c| channels.contains(*c))
        .count();
    (r == 0 && c == 0).then_some("gs2.obw.err_option")
}

fn perm(v: Option<&Value>) -> u64 {
    v.and_then(|p| {
        p.as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| p.as_u64())
    })
    .unwrap_or(0)
}

pub fn everyone_perms(guild: &Value) -> u64 {
    let gid = str_of(guild, "id");
    perm(
        guild
            .get("roles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|r| str_of(r, "id") == gid)
            .and_then(|r| r.get("permissions")),
    )
}

pub fn channel_perms(ch: &Value, guild_id: &str, everyone: u64) -> u64 {
    if everyone & ADMINISTRATOR != 0 {
        return u64::MAX;
    }
    let ow = ch
        .get("permission_overwrites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|o| str_of(o, "id") == guild_id);
    match ow {
        Some(o) => (everyone & !perm(o.get("deny"))) | perm(o.get("allow")),
        None => everyone,
    }
}

pub fn chans(list: &Value, guild_id: &str, everyone: u64) -> Vec<Chan> {
    let mut out: Vec<Chan> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| !matches!(c.get("type").and_then(Value::as_u64), Some(10..=12)))
        .map(|c| {
            let kind = c.get("type").and_then(Value::as_u64).unwrap_or(0);
            let p = channel_perms(c, guild_id, everyone);
            let need = if is_voice(kind) { VIEW_CHANNEL | CONNECT } else { VIEW_CHANNEL };
            let public = p & need == need;
            let denied = c
                .get("permission_overwrites")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|o| str_of(o, "id") == guild_id)
                .is_some_and(|o| perm(o.get("deny")) & VIEW_CHANNEL != 0);
            let private = (is_voice(kind) && p & CONNECT == 0) || denied;
            let send = if matches!(kind, 15 | 16) {
                SEND_IN_THREADS
            } else {
                SEND_MESSAGES
            };
            Chan {
                id: str_of(c, "id").to_owned(),
                name: str_of(c, "name").to_owned(),
                kind,
                parent: c
                    .get("parent_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                position: c.get("position").and_then(Value::as_i64).unwrap_or(0),
                public,
                private,
                chattable: public && p & send != 0,
            }
        })
        .collect();
    out.sort_by_key(|c| {
        (
            c.kind == 4,
            is_voice(c.kind),
            c.position,
            c.id.parse::<u64>().unwrap_or(0),
        )
    });
    out
}

pub fn is_voice(kind: u64) -> bool {
    matches!(kind, 2 | 13)
}

pub fn groups(list: &[Chan]) -> Vec<(Option<&Chan>, Vec<&Chan>)> {
    let mut cats: Vec<&Chan> = list.iter().filter(|c| c.kind == 4).collect();
    cats.sort_by_key(|c| (c.position, c.id.parse::<u64>().unwrap_or(0)));
    let mut out = vec![(
        None,
        list.iter()
            .filter(|c| c.kind != 4 && c.parent.is_none())
            .collect::<Vec<_>>(),
    )];
    for cat in cats {
        out.push((
            Some(cat),
            list.iter()
                .filter(|c| c.kind != 4 && c.parent.as_deref() == Some(cat.id.as_str()))
                .collect(),
        ));
    }
    out
}

pub fn selected<'a>(list: &'a [Chan], set: &[String]) -> Vec<&'a Chan> {
    list.iter()
        .filter(|c| {
            c.public
                && ((set.contains(&c.id) && c.kind != 4)
                    || c.parent.as_ref().is_some_and(|p| set.contains(p)))
        })
        .collect()
}

pub fn chattable_defaults(list: &[Chan], defaults: &[String]) -> (usize, usize) {
    let s = selected(list, defaults);
    (s.iter().filter(|c| c.chattable).count(), s.len())
}

pub fn assigned_minimum(list: &[Chan], defaults: &[String], prompts: &[Prompt]) -> usize {
    let mut r: Vec<String> = selected(list, defaults)
        .into_iter()
        .map(|c| c.id.clone())
        .collect();
    for p in prompts.iter().filter(|p| p.required) {
        let first: Vec<String> = selected(
            list,
            p.options
                .first()
                .map(|o| o.channel_ids.as_slice())
                .unwrap_or(&[]),
        )
        .into_iter()
        .map(|c| c.id.clone())
        .collect();
        let pick = p.options.iter().fold(first, |acc, o| {
            let a: Vec<String> = selected(list, &o.channel_ids)
                .into_iter()
                .filter(|c| !r.contains(&c.id))
                .map(|c| c.id.clone())
                .collect();
            if a.len() < acc.len() { a } else { acc }
        });
        r.extend(pick);
    }
    r.len()
}

pub fn coverage<'a>(
    list: &'a [Chan],
    defaults: &[String],
    prompts: &[Prompt],
) -> (Vec<&'a Chan>, Vec<&'a Chan>) {
    let mut set: HashSet<&str> = defaults.iter().map(String::as_str).collect();
    for p in prompts {
        for o in &p.options {
            set.extend(o.channel_ids.iter().map(String::as_str));
        }
    }
    let pool: Vec<&Chan> = list.iter().filter(|c| c.kind != 4 && !c.private).collect();
    let hit = |c: &&Chan| {
        set.contains(c.id.as_str()) || c.parent.as_deref().is_some_and(|p| set.contains(p))
    };
    (
        pool.iter().copied().filter(hit).collect(),
        pool.iter().copied().filter(|c| !hit(c)).collect(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    None,
    Good,
    Warning,
    Required,
}

pub fn questions_status(
    list: &[Chan],
    defaults: &[String],
    prompts: &[Prompt],
) -> (Status, usize, usize) {
    let (a, m) = coverage(list, defaults, prompts);
    let total = a.len() + m.len();
    let pct = if total == 0 {
        f32::NAN
    } else {
        a.len() as f32 / total as f32 * 100.
    };
    let st = if prompts.is_empty() {
        Status::None
    } else if pct >= 85. {
        Status::Good
    } else {
        Status::Warning
    };
    (st, a.len(), total)
}

pub fn home_configured(home: &Value) -> bool {
    let msg = home
        .get("welcome_message")
        .map(|w| str_of(w, "message"))
        .unwrap_or("");
    !msg.is_empty()
        || home
            .get("new_member_actions")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
        || home
            .get("resource_channels")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
}

pub fn home_valid(home: &Value, list: &[Chan]) -> bool {
    let w = home.get("welcome_message");
    let msg = w.map(|w| str_of(w, "message")).unwrap_or("");
    let authors = w
        .and_then(|w| w.get("author_ids"))
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    let actions_ok = home
        .get("new_member_actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .all(|a| list.iter().any(|c| c.id == str_of(a, "channel_id")));
    !msg.trim().is_empty() && authors && actions_ok
}

pub fn home_status(home: &Value, list: &[Chan]) -> (Status, &'static str) {
    let d = home_configured(home);
    let r = home_valid(home, list);
    let st = match (d, r) {
        (true, true) => Status::Good,
        (true, false) => Status::Required,
        _ => Status::None,
    };
    let desc = if !r {
        "gs2.obw.r_home_invalid"
    } else if d {
        "gs2.obw.r_home_done"
    } else {
        "gs2.obw.r_home_todo"
    };
    (st, desc)
}

fn draft_for(guild: Id, ob: &Value, list: &[Chan]) -> Draft {
    DRAFTS.with(|d| {
        d.borrow_mut()
            .entry(guild)
            .or_insert_with(|| {
                let defaults: Vec<String> = ids(ob.get("default_channel_ids"))
                    .into_iter()
                    .filter(|i| list.iter().any(|c| &c.id == i))
                    .collect();
                let prompts = parse_prompts(ob);
                Draft {
                    orig_defaults: defaults.clone(),
                    defaults,
                    orig_prompts: prompts.clone(),
                    prompts,
                    edited: false,
                }
            })
            .clone()
    })
}

fn edit(guild: Id, f: impl FnOnce(&mut Draft)) {
    DRAFTS.with(|d| {
        if let Some(x) = d.borrow_mut().get_mut(&guild) {
            f(x);
            x.edited = true;
        }
    });
}

pub fn reset(guild: Id) {
    DRAFTS.with(|d| {
        if let Some(x) = d.borrow_mut().get_mut(&guild) {
            x.defaults = x.orig_defaults.clone();
            x.prompts = x.orig_prompts.clone();
            x.edited = false;
        }
    });
    super::forget_prefix(guild, "obw-q:");
}

pub fn step(guild: Id) -> usize {
    super::with(|s| {
        s.text
            .get(&(guild, "ob-step".to_owned()))
            .and_then(|v| v.parse::<usize>().ok())
    })
    .unwrap_or(0)
    .min(STEPS - 1)
}

pub fn set_step(guild: Id, i: usize) {
    super::with(|s| {
        s.text
            .insert((guild, "ob-step".into()), i.min(STEPS - 1).to_string());
    });
}

fn tip_open(guild: Id, i: usize) -> bool {
    i < 4 && !super::flag(guild, &format!("ob-tip-{i}")).unwrap_or(false)
}

pub fn dismiss_tips(guild: Id) {
    for i in 0..4 {
        super::set_flag(guild, &format!("ob-tip-{i}"), true);
    }
}

struct Ctx {
    guild: Id,
    advanced: bool,
    list: Vec<Chan>,
    roles: HashSet<String>,
    draft: Draft,
    home: Value,
    rules_channel: Option<String>,
}

fn load(app: &PhoenixApp, guild: Id, ob: &Value, cx: &mut Context<PhoenixApp>) -> Option<Ctx> {
    let g = fetch(app, guild, ServerCall::Guild { guild: sf(guild) }, cx);
    let ch = fetch(app, guild, ServerCall::Channels { guild: sf(guild) }, cx);
    let home = fetch(
        app,
        guild,
        ServerCall::NewMemberWelcome { guild: sf(guild) },
        cx,
    );
    let (g, ch) = (g?, ch?);
    let gid = str_of(&g, "id").to_owned();
    let list = chans(&ch, &gid, everyone_perms(&g));
    let roles = g
        .get("roles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|r| str_of(r, "id").to_owned())
        .collect();
    let draft = draft_for(guild, ob, &list);
    sync_titles(guild, &draft, cx);
    let draft = draft_for(guild, ob, &list);
    Some(Ctx {
        guild,
        advanced: ob.get("mode").and_then(Value::as_u64) == Some(1),
        list,
        roles,
        draft,
        home: home.unwrap_or(Value::Null),
        rules_channel: g
            .get("rules_channel_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn sync_titles(guild: Id, draft: &Draft, cx: &mut Context<PhoenixApp>) {
    for p in &draft.prompts {
        if let Some(v) = super::value(guild, &format!("obw-q:{}", p.id), cx)
            && v != p.title
        {
            let id = p.id.clone();
            edit(guild, move |d| {
                if let Some(x) = d.prompts.iter_mut().find(|x| x.id == id) {
                    x.title = v;
                }
            });
        }
    }
}

impl Ctx {
    fn channel_ids(&self) -> HashSet<String> {
        self.list.iter().map(|c| c.id.clone()).collect()
    }

    fn errors(&self) -> Vec<Option<PromptErr>> {
        if !self.draft.edited {
            return vec![None; self.draft.prompts.len()];
        }
        validate(&self.draft.prompts, &self.roles, &self.channel_ids())
    }

    fn has_changes(&self, step: usize) -> bool {
        let d = &self.draft;
        match step {
            1 => d.defaults != d.orig_defaults || (self.advanced && d.prompts != d.orig_prompts),
            2 => d.prompts != d.orig_prompts,
            _ => false,
        }
    }

    fn configured(&self, step: usize) -> bool {
        match step {
            0 => true,
            1 => !self.draft.defaults.is_empty(),
            2 => !self.draft.prompts.is_empty(),
            3 => home_configured(&self.home),
            _ => false,
        }
    }

    fn has_errors(&self, step: usize) -> bool {
        step == 2 && self.errors().iter().any(Option::is_some)
    }

    fn name(&self, id: &str) -> String {
        self.list
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }
}

pub fn wizard(app: &PhoenixApp, guild: Id, ob: &Value, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(c) = load(app, guild, ob, cx) else {
        return super::page().child(super::loading()).into_any_element();
    };
    let cur = step(guild);
    let tip = tip_open(guild, cur);
    let body = match cur {
        0 => super::safety::render(app, guild, cx),
        1 => defaults_step(&c, cx),
        2 => questions_step(&c, cx),
        3 => home_step(&c, cx),
        _ => review_step(&c, cx),
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(z(32.))
        .when(tip, |d| d.opacity(0.4))
        .child(progress(&c, cur, cx))
        .child(div().w_full().child(body))
        .into_any_element()
}

fn step_label(advanced: bool, i: usize) -> SharedString {
    let n = i + 1;
    match i {
        0 => crate::t!("gs2.ob_step1"),
        1 if advanced => crate::t!("gs2.obw.s2_adv", n = n),
        1 => crate::t!("gs2.obw.s2", n = n),
        2 if advanced => crate::t!("gs2.obw.s3_adv", n = n),
        2 => crate::t!("gs2.obw.s3", n = n),
        3 => crate::t!("gs2.obw.s4", n = n),
        _ => crate::t!("gs2.obw.s5", n = n),
    }
}

fn progress(c: &Ctx, cur: usize, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = c.guild;
    let mut bar = div().w_full().flex().gap(z(8.));
    for i in 0..STEPS {
        let clickable = i < STEPS - 1 || cur == STEPS - 1;
        let now = i == cur;
        let col = if now {
            idx::TEXT_BRAND
        } else {
            idx::BACKGROUND_MOD_MUTED
        };
        let mut item = div()
            .id(("gs2-ob-step", i))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(z(8.))
            .group("obw-step")
            .child(
                div()
                    .w_full()
                    .h(z(4.))
                    .rounded(z(8.))
                    .bg(t(col))
                    .when(clickable && !now, |d| {
                        d.group_hover("obw-step", |s| s.bg(t(idx::INTERACTIVE_TEXT_HOVER)))
                    }),
            )
            .child(text(
                12.,
                16.,
                FontWeight::MEDIUM,
                if now {
                    idx::TEXT_BRAND
                } else {
                    idx::TEXT_MUTED
                },
                step_label(c.advanced, i),
            ));
        if clickable {
            item = item
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    set_step(guild, i);
                    cx.notify();
                }));
        }
        bar = bar.child(item);
    }
    bar.into_any_element()
}

fn link(s: SharedString) -> gpui::Div {
    text(14., 18., FontWeight::MEDIUM, idx::TEXT_LINK, s)
        .cursor_pointer()
        .hover(|s| s.underline())
}

fn h2(s: SharedString) -> gpui::Div {
    text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, s)
}

fn body_sm(s: SharedString) -> gpui::Div {
    text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, s)
}

fn channel_icon(kind: u64, public: bool) -> &'static str {
    use crate::model::ChannelKind as K;
    let k = match kind {
        2 => K::Voice,
        5 => K::Announcement,
        13 => K::Stage,
        15 | 16 => K::Forum,
        4 => K::Category,
        _ => K::Text,
    };
    crate::ui::modals::channel_settings::channel_icon(k, !public)
}

fn checkbox(on: bool) -> gpui::Div {
    crate::ui::community::checkbox(on)
}

fn unlock_button(id: impl Into<gpui::ElementId>) -> gpui::Stateful<gpui::Div> {
    button_sm(id, SharedString::default(), BtnKind::Secondary)
        .gap(z(4.))
        .child(icon("lock", 16., idx::CONTROL_SECONDARY_TEXT_DEFAULT))
        .child(crate::t!("gs2.obw.unlock"))
}

fn toggle_default(guild: Id, id: String) {
    edit(guild, move |d| {
        if let Some(i) = d.defaults.iter().position(|x| *x == id) {
            d.defaults.remove(i);
        } else {
            d.defaults.push(id);
        }
    });
}

fn channel_row(
    c: &Ctx,
    ch: &Chan,
    first: bool,
    last: bool,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let guild = c.guild;
    let on = c.draft.defaults.contains(&ch.id);
    let id = ch.id.clone();
    let action: AnyElement = if ch.public {
        checkbox(on).into_any_element()
    } else {
        unlock_button(SharedString::from(format!("obw-unlock-{}", ch.id))).into_any_element()
    };
    div()
        .id(SharedString::from(format!("obw-ch-{}", ch.id)))
        .w_full()
        .min_h(z(36.))
        .px(z(16.))
        .py(z(12.))
        .bg(t(idx::BACKGROUND_BASE_LOWER))
        .when(first, |d| d.rounded_tl(z(8.)).rounded_tr(z(8.)))
        .when(last, |d| d.rounded_bl(z(8.)).rounded_br(z(8.)))
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
        .when(ch.public, |d| {
            d.on_click(cx.listener(move |_, _, _, cx| {
                toggle_default(guild, id.clone());
                cx.notify();
            }))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .mr(z(8.))
                .flex()
                .items_center()
                .child(icon(
                    channel_icon(ch.kind, ch.public),
                    20.,
                    idx::ICON_DEFAULT,
                ))
                .child(
                    div().min_w_0().truncate().child(
                        text(
                            16.,
                            20.,
                            FontWeight::MEDIUM,
                            idx::TEXT_DEFAULT,
                            ch.name.clone(),
                        )
                        .truncate(),
                    ),
                ),
        )
        .child(div().flex_none().flex().child(action))
        .into_any_element()
}

fn browser(c: &Ctx, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let guild = c.guild;
    let mut col = div().w_full().flex().flex_col();
    for (cat, kids) in groups(&c.list) {
        if cat.is_none() && kids.is_empty() {
            continue;
        }
        let head = match cat {
            None => div()
                .w_full()
                .pb(z(8.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    text(
                        14.,
                        18.,
                        FontWeight::MEDIUM,
                        idx::TEXT_MUTED,
                        crate::t!("gs2.obw.uncategorized"),
                    )
                    .truncate(),
                ),
            Some(k) => {
                let on = c.draft.defaults.contains(&k.id);
                let kid = k.id.clone();
                let ctl = div()
                    .id(SharedString::from(format!("obw-cat-{}", k.id)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        toggle_default(guild, kid.clone());
                        cx.notify();
                    }))
                    .child(checkbox(on));
                div()
                    .w_full()
                    .pt(z(24.))
                    .pb(z(8.))
                    .pr(z(16.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div().min_w_0().truncate().child(
                            text(
                                14.,
                                18.,
                                FontWeight::MEDIUM,
                                idx::TEXT_MUTED,
                                k.name.clone(),
                            )
                            .truncate(),
                        ),
                    )
                    .child(ctl)
            }
        };
        col = col.child(head);
        let n = kids.len();
        for (i, ch) in kids.into_iter().enumerate() {
            if i > 0 {
                col = col.child(
                    div()
                        .mx(z(16.))
                        .mt(z(-1.))
                        .h(z(1.))
                        .bg(t(idx::BORDER_SUBTLE)),
                );
            }
            col = col.child(channel_row(c, ch, i == 0, i + 1 == n, cx));
        }
    }
    col
}

fn summary(c: &Ctx) -> gpui::Div {
    let (chat, _) = chattable_defaults(&c.list, &c.draft.defaults);
    let total = assigned_minimum(&c.list, &c.draft.defaults, &c.draft.prompts);
    let from_q = total.saturating_sub(selected(&c.list, &c.draft.defaults).len());
    let chat_key = if chat <= 1 {
        "gs2.obw.chattable_one"
    } else {
        "gs2.obw.chattable"
    };
    let info = |s: SharedString| {
        div()
            .mt(z(12.))
            .flex()
            .gap(z(8.))
            .child(icon("info", 18., idx::TEXT_MUTED))
            .child(div().flex_1().min_w_0().child(text(
                12.,
                16.,
                FontWeight::NORMAL,
                idx::TEXT_MUTED,
                s,
            )))
    };
    let shown: Vec<&Chan> = selected(&c.list, &c.draft.defaults)
        .into_iter()
        .filter(|x| x.chattable)
        .collect();
    let mut list = div()
        .pt(z(8.))
        .pb(z(16.))
        .bg(t(idx::BACKGROUND_BASE_LOWER))
        .rounded_bl(z(8.))
        .rounded_br(z(8.))
        .child(div().my(z(8.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)));
    if shown.is_empty() {
        list = list.child(
            div()
                .p(z(16.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(z(40.))
                        .mb(z(8.))
                        .rounded(z(40.))
                        .bg(t(idx::BACKGROUND_BASE_LOWEST))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("hash", 24., idx::TEXT_MUTED)),
                )
                .child(
                    text(
                        16.,
                        20.,
                        FontWeight::NORMAL,
                        idx::TEXT_MUTED,
                        crate::t!("gs2.obw.preview_empty"),
                    )
                    .text_center(),
                ),
        );
    } else {
        for ch in shown {
            list = list.child(
                div()
                    .h(z(28.))
                    .mt(z(4.))
                    .px(z(18.))
                    .flex()
                    .items_center()
                    .child(div().mr(z(8.)).child(icon(
                        channel_icon(ch.kind, true),
                        16.,
                        idx::TEXT_MUTED,
                    )))
                    .child(
                        div().flex_1().min_w_0().truncate().child(
                            text(
                                16.,
                                20.,
                                FontWeight::MEDIUM,
                                idx::TEXT_MUTED,
                                ch.name.clone(),
                            )
                            .truncate(),
                        ),
                    )
                    .child(
                        div()
                            .size(z(8.))
                            .flex_none()
                            .rounded_full()
                            .bg(t(idx::TEXT_BRAND)),
                    ),
            );
        }
    }
    div()
        .w(z(240.))
        .flex_none()
        .flex()
        .flex_col()
        .child(
            div()
                .bg(t(idx::BACKGROUND_BASE_LOWER))
                .rounded_tl(z(12.))
                .rounded_tr(z(12.))
                .px(z(16.))
                .pt(z(16.))
                .child(
                    div()
                        .flex()
                        .gap(z(8.))
                        .items_center()
                        .child(text(
                            12.,
                            16.,
                            FontWeight::SEMIBOLD,
                            idx::TEXT_BRAND,
                            crate::t!(chat_key, n = chat),
                        ))
                        .child(text(
                            12.,
                            16.,
                            FontWeight::SEMIBOLD,
                            idx::TEXT_MUTED,
                            crate::t!("gs2.obw.total", n = total),
                        )),
                )
                .child(info(crate::t!("gs2.obw.total_hint")))
                .child(info(crate::t!("gs2.obw.minimum", n = from_q))),
        )
        .child(list)
}

fn defaults_step(c: &Ctx, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = c.guild;
    let collapsed = super::flag(guild, "obw-collapse-defaults").unwrap_or(false);
    let mut left = div()
        .w(z(400.))
        .flex_none()
        .relative()
        .flex()
        .flex_col()
        .child(
            div()
                .id("obw-collapse")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    super::set_flag(guild, "obw-collapse-defaults", !collapsed);
                    cx.notify();
                }))
                .child(icon(
                    if collapsed { "chevron" } else { "chevron-up" },
                    16.,
                    idx::INTERACTIVE_TEXT_DEFAULT,
                )),
        )
        .child(h2(crate::t!("gs2.obw.defaults_title")).pr(z(24.)))
        .child(
            div()
                .mt(z(8.))
                .child(body_sm(crate::t!("gs2.obw.defaults_desc"))),
        )
        .child(
            div()
                .mt(z(8.))
                .mb(z(16.))
                .child(link(crate::t!(if c.advanced {
                    "gs2.obw.to_regular"
                } else {
                    "gs2.obw.to_advanced"
                }))),
        );
    if !collapsed {
        left = left.child(browser(c, cx));
    }
    if c.advanced {
        left = left
            .child(div().my(z(24.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
            .child(h2(crate::t!("gs2.obw.prejoin_title")).mt(z(16.)))
            .child(
                div()
                    .mb(z(16.))
                    .child(body_sm(crate::t!("gs2.obw.prejoin_desc"))),
            )
            .child(prompt_list(c, true, cx));
    }
    div()
        .w_full()
        .flex()
        .items_start()
        .justify_between()
        .child(left)
        .child(summary(c))
        .into_any_element()
}

fn prompt_list(c: &Ctx, prejoin: bool, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let guild = c.guild;
    let errs = c.errors();
    let mut col = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(z(if prejoin { 8. } else { 16. }));
    let mut n = 0;
    for (i, p) in c.draft.prompts.iter().enumerate() {
        if p.in_onboarding != prejoin {
            continue;
        }
        n += 1;
        col = col.child(prompt_card(c, p, n, errs.get(i).cloned().flatten(), cx));
    }
    col.child(
        div()
            .id(if prejoin {
                "obw-add-prejoin"
            } else {
                "obw-add-postjoin"
            })
            .w_full()
            .p(z(16.))
            .rounded(z(12.))
            .border_2()
            .border_dashed()
            .border_color(t(idx::BORDER_SUBTLE))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                let id = format!(
                    "{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                );
                edit(guild, move |d| d.prompts.push(new_prompt(id, prejoin)));
                cx.notify();
            }))
            .child(
                div()
                    .mr(z(8.))
                    .child(icon("plus-circle", 24., idx::TEXT_BRAND)),
            )
            .child(text(
                16.,
                20.,
                FontWeight::MEDIUM,
                idx::TEXT_BRAND,
                crate::t!("gs2.obw.add_question"),
            )),
    )
}

fn emoji(e: &Value) -> AnyElement {
    if let Some(id) = e.get("id").and_then(Value::as_str) {
        let ext = if e.get("animated").and_then(Value::as_bool).unwrap_or(false) {
            "gif"
        } else {
            "webp"
        };
        return gpui::img(SharedString::from(format!(
            "https://cdn.discordapp.com/emojis/{id}.{ext}?size=48"
        )))
        .size(z(24.))
        .flex_none()
        .into_any_element();
    }
    match e.get("name").and_then(Value::as_str) {
        Some(n) if !n.is_empty() => div()
            .size(z(24.))
            .flex_none()
            .text_size(z(20.))
            .line_height(z(24.))
            .child(n.to_owned())
            .into_any_element(),
        _ => div().into_any_element(),
    }
}

fn err_line(key: &'static str) -> gpui::Div {
    div().mt(z(8.)).child(text(
        12.,
        16.,
        FontWeight::MEDIUM,
        idx::TEXT_FEEDBACK_CRITICAL,
        crate::t!(key),
    ))
}

fn prompt_card(
    c: &Ctx,
    p: &Prompt,
    n: usize,
    err: Option<PromptErr>,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let guild = c.guild;
    let key = format!("obw-q:{}", p.id);
    let e = err.unwrap_or_default();
    let mut options = div().w_full().flex().flex_wrap().gap(z(8.));
    for (j, o) in p.options.iter().enumerate() {
        let bad = e.option_errs.get(j).copied().flatten();
        options = options.child(
            div()
                .id(SharedString::from(format!("obw-opt-{}-{}", p.id, o.id)))
                .w_full()
                .px(z(16.))
                .py(z(12.))
                .rounded(z(12.))
                .border_2()
                .border_color(t(if bad.is_some() {
                    idx::BORDER_FEEDBACK_CRITICAL
                } else {
                    idx::BORDER_SUBTLE
                }))
                .flex()
                .flex_col()
                .justify_center()
                .cursor_pointer()
                .child(
                    div().flex().items_center().child(emoji(&o.emoji)).child(
                        div()
                            .ml(z(12.))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(z(4.))
                            .child(
                                text(
                                    16.,
                                    20.,
                                    FontWeight::MEDIUM,
                                    idx::TEXT_STRONG,
                                    o.title.clone(),
                                )
                                .truncate(),
                            )
                            .when(!o.description.is_empty(), |d| {
                                d.child(
                                    text(
                                        14.,
                                        18.,
                                        FontWeight::NORMAL,
                                        idx::TEXT_DEFAULT,
                                        o.description.clone(),
                                    )
                                    .line_clamp(2),
                                )
                            }),
                    ),
                )
                .when_some(bad, |d, k| {
                    d.child(
                        text(
                            12.,
                            16.,
                            FontWeight::MEDIUM,
                            idx::TEXT_FEEDBACK_CRITICAL,
                            crate::t!(k),
                        )
                        .mt(z(4.)),
                    )
                }),
        );
    }
    options = options.child(
        div()
            .w_full()
            .px(z(16.))
            .py(z(12.))
            .rounded(z(12.))
            .border_2()
            .border_dashed()
            .border_color(t(idx::BORDER_SUBTLE))
            .flex()
            .items_center()
            .cursor_pointer()
            .child(icon("plus-circle", 24., idx::INTERACTIVE_TEXT_DEFAULT))
            .child(div().ml(z(12.)).child(text(
                16.,
                20.,
                FontWeight::MEDIUM,
                idx::TEXT_STRONG,
                crate::t!("gs2.obw.add_answer"),
            ))),
    );
    let pid = p.id.clone();
    let pid2 = p.id.clone();
    let pid3 = p.id.clone();
    let multi = !p.single_select;
    let required = p.required;
    let title_field = super::field(guild, &key, &p.title, cx);
    div()
        .id(SharedString::from(format!("obw-card-{}", p.id)))
        .group("obw-card")
        .relative()
        .w_full()
        .p(z(16.))
        .rounded(z(12.))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().mb(z(8.)).child(text(
                    16.,
                    20.,
                    FontWeight::MEDIUM,
                    idx::TEXT_STRONG,
                    crate::t!("gs2.obw.question_n", n = n),
                )))
                .child(title_field)
                .when_some(e.title, |d, k| d.child(err_line(k)))
                .when(e.config, |d| {
                    d.child(div().mt(z(8.)).child(text(
                        12.,
                        16.,
                        FontWeight::MEDIUM,
                        idx::TEXT_FEEDBACK_CRITICAL,
                        crate::t!("gs2.obw.err_max", n = MAX_PREJOIN),
                    )))
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().mb(z(8.)).child(text(
                    16.,
                    20.,
                    FontWeight::MEDIUM,
                    idx::TEXT_STRONG,
                    crate::t!("gs2.obw.answers", n = p.options.len(), of = MAX_OPTIONS),
                )))
                .child(options)
                .when_some(e.options, |d, k| d.child(err_line(k))),
        )
        .child(
            div()
                .mt(z(8.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .gap(z(24.))
                        .child(
                            checkbox_option(
                                SharedString::from(format!("obw-multi-{}", p.id)),
                                crate::t!("gs2.obw.multiple"),
                                multi,
                            )
                            .max_w(z(200.))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    let id = pid.clone();
                                    edit(guild, move |d| {
                                        if let Some(x) = d.prompts.iter_mut().find(|x| x.id == id) {
                                            x.single_select = !x.single_select;
                                        }
                                    });
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            checkbox_option(
                                SharedString::from(format!("obw-req-{}", p.id)),
                                crate::t!("gs2.obw.required"),
                                required,
                            )
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    let id = pid2.clone();
                                    edit(guild, move |d| {
                                        if let Some(x) = d.prompts.iter_mut().find(|x| x.id == id) {
                                            x.required = !x.required;
                                        }
                                    });
                                    cx.notify();
                                },
                            )),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("obw-del-{}", p.id)))
                        .size(z(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(z(8.))
                        .invisible()
                        .group_hover("obw-card", |s| s.visible())
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            let id = pid3.clone();
                            edit(guild, move |d| d.prompts.retain(|x| x.id != id));
                            cx.notify();
                        }))
                        .child(icon("chat/trash", 20., idx::ICON_DEFAULT)),
                ),
        )
        .into_any_element()
}

fn dot() -> gpui::Div {
    div()
        .size(z(4.))
        .flex_none()
        .rounded(z(8.))
        .bg(t(idx::BACKGROUND_MOD_SUBTLE))
}

fn questions_step(c: &Ctx, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = c.guild;
    let (st, a, total) = questions_status(&c.list, &c.draft.defaults, &c.draft.prompts);
    let (_, missing) = coverage(&c.list, &c.draft.defaults, &c.draft.prompts);
    let open = super::flag(guild, "obw-coverage").unwrap_or(false);
    let pct = if total == 0 {
        0.
    } else {
        a as f32 / total as f32
    };
    let fill = if st == Status::Good {
        idx::STATUS_POSITIVE
    } else {
        idx::STATUS_WARNING
    };
    let missing_n = missing.len();
    let mut cover = div()
        .mt(z(24.))
        .w_full()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(t(idx::BACKGROUND_BASE_LOWEST))
        .child(
            div()
                .px(z(16.))
                .py(z(12.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(icon(
                            "warning",
                            16.,
                            if missing_n == 0 {
                                idx::STATUS_POSITIVE
                            } else {
                                idx::STATUS_WARNING
                            },
                        ))
                        .child(text(
                            12.,
                            16.,
                            FontWeight::MEDIUM,
                            idx::TEXT_MUTED,
                            if missing_n == 0 {
                                crate::t!("gs2.obw.missing_none")
                            } else {
                                crate::t!("gs2.obw.missing", n = missing_n)
                            },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .child(
                            div()
                                .w(z(120.))
                                .h(z(6.))
                                .rounded(z(3.))
                                .bg(t(idx::BACKGROUND_MOD_STRONG))
                                .child(div().h_full().w(z(120. * pct)).rounded(z(3.)).bg(t(fill))),
                        )
                        .child(
                            div()
                                .id("obw-cover-toggle")
                                .size(z(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    super::set_flag(guild, "obw-coverage", !open);
                                    cx.notify();
                                }))
                                .child(icon(
                                    if open { "chevron" } else { "chevron-up" },
                                    24.,
                                    idx::INTERACTIVE_TEXT_DEFAULT,
                                )),
                        ),
                ),
        );
    if open && !missing.is_empty() {
        let mut pills = div().mt(z(8.)).mb(z(16.)).flex().flex_wrap().gap(z(4.));
        for ch in &missing {
            pills = pills.child(
                div()
                    .px(z(4.))
                    .py(z(2.))
                    .rounded(z(2.))
                    .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                    .flex()
                    .items_center()
                    .child(icon(channel_icon(ch.kind, true), 16., idx::TEXT_MUTED))
                    .child(text(
                        14.,
                        18.,
                        FontWeight::MEDIUM,
                        idx::TEXT_MUTED,
                        ch.name.clone(),
                    )),
            );
        }
        cover = cover.child(
            div()
                .px(z(16.))
                .py(z(12.))
                .bg(t(idx::BACKGROUND_BASE_LOWER))
                .child(pills),
        );
    }
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(h2(crate::t!(if c.advanced {
            "gs2.obw.postjoin_title"
        } else {
            "gs2.obw.questions_title"
        })))
        .child(
            div()
                .mt(z(8.))
                .child(body_sm(crate::t!("gs2.obw.questions_desc"))),
        )
        .child(
            div()
                .mt(z(8.))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(link(crate::t!("gs2.obw.examples")))
                .child(dot())
                .child(link(crate::t!("gs2.obw.preview")))
                .child(dot())
                .child(link(crate::t!(if c.advanced {
                    "gs2.obw.to_regular"
                } else {
                    "gs2.obw.to_advanced"
                }))),
        )
        .child(cover)
        .child(
            div()
                .mt(z(24.))
                .flex()
                .flex_col()
                .gap(z(16.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().my(z(20.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
                        .child(text(
                            16.,
                            20.,
                            FontWeight::SEMIBOLD,
                            idx::TEXT_STRONG,
                            crate::t!("gs2.obw.postjoin_title"),
                        ))
                        .child(
                            div()
                                .mt(z(8.))
                                .child(body_sm(crate::t!("gs2.obw.postjoin_desc"))),
                        ),
                )
                .child(prompt_list(c, false, cx)),
        )
        .into_any_element()
}

fn section_sep() -> gpui::Div {
    div().my(z(32.)).w_full().h(z(1.)).bg(t(idx::BORDER_SUBTLE))
}

fn desc_md(s: SharedString) -> gpui::Div {
    div()
        .mt(z(8.))
        .mb(z(24.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, s))
}

fn emoji_disc(child: AnyElement) -> gpui::Div {
    div()
        .p(z(12.))
        .rounded_full()
        .bg(t(idx::BACKGROUND_BASE_LOWER))
        .flex()
        .items_center()
        .justify_center()
        .child(child)
}

fn dashed_add(id: &'static str, label: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .mt(z(12.))
        .w_full()
        .p(z(16.))
        .rounded(z(8.))
        .border_2()
        .border_dashed()
        .border_color(t(idx::BORDER_MUTED))
        .flex()
        .items_center()
        .justify_center()
        .gap(z(8.))
        .cursor_pointer()
        .child(icon("plus-circle", 20., idx::TEXT_LINK))
        .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_LINK, label))
}

fn home_step(c: &Ctx, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = c.guild;
    let h = &c.home;
    let welcome = h
        .get("welcome_message")
        .map(|w| str_of(w, "message").to_owned())
        .unwrap_or_default();
    let mut left = div()
        .flex_1()
        .min_w_0()
        .pr(z(32.))
        .flex()
        .flex_col()
        .child(h2(crate::t!("gs2.obw.welcome_title")))
        .child(desc_md(crate::t!("gs2.obw.welcome_desc")));
    left = if welcome.is_empty() {
        left.child(div().my(z(8.)).flex().child(button_sm(
            "obw-welcome",
            crate::t!("gs2.obw.welcome_setup"),
            BtnKind::Primary,
        )))
    } else {
        left.child(
            div()
                .my(z(8.))
                .p(z(16.))
                .rounded(z(12.))
                .bg(t(idx::BACKGROUND_BASE_LOWER))
                .child(text(
                    14.,
                    18.,
                    FontWeight::NORMAL,
                    idx::TEXT_DEFAULT,
                    welcome,
                )),
        )
    };
    left = left
        .child(section_sep())
        .child(h2(crate::t!("gs2.obw.todo_title")))
        .child(desc_md(crate::t!("gs2.obw.todo_desc")))
        .child(
            div().my(z(8.)).w_full().child(
                div()
                    .mb(z(32.))
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .child(text(
                        14.,
                        18.,
                        FontWeight::MEDIUM,
                        idx::TEXT_DEFAULT,
                        crate::t!("gs2.obw.bad_example"),
                    ))
                    .child(
                        div()
                            .px(z(12.))
                            .py(z(8.))
                            .rounded(z(12.))
                            .border_2()
                            .border_color(t(idx::BORDER_SUBTLE))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .child(emoji_disc(
                                icon("deny", 24., idx::RED_400).into_any_element(),
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(text(
                                        16.,
                                        20.,
                                        FontWeight::SEMIBOLD,
                                        idx::TEXT_STRONG,
                                        crate::t!("gs2.obw.bad_title"),
                                    ))
                                    .child(text(
                                        12.,
                                        16.,
                                        FontWeight::MEDIUM,
                                        idx::TEXT_MUTED,
                                        crate::t!("gs2.obw.bad_where"),
                                    )),
                            ),
                    ),
            ),
        );
    let mut actions = div().my(z(8.)).w_full().flex().flex_col();
    let list = h
        .get("new_member_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut items: Vec<(AnyElement, SharedString)> = Vec::new();
    if c.rules_channel.is_some() {
        items.push((
            icon("forum/book-check", 24., idx::INTERACTIVE_TEXT_DEFAULT).into_any_element(),
            crate::t!("gs2.obw.read_rules"),
        ));
    }
    for a in &list {
        items.push((
            emoji(a.get("emoji").unwrap_or(&Value::Null)),
            str_of(a, "title").to_owned().into(),
        ));
    }
    for (e, title) in items {
        actions = actions.child(
            div().mt(z(12.)).child(
                div()
                    .w_full()
                    .px(z(16.))
                    .py(z(12.))
                    .rounded(z(12.))
                    .bg(t(idx::BACKGROUND_BASE_LOWER))
                    .flex()
                    .items_center()
                    .gap(z(16.))
                    .child(emoji_disc(e))
                    .child(div().flex_1().min_w_0().child(text(
                        16.,
                        20.,
                        FontWeight::SEMIBOLD,
                        idx::TEXT_STRONG,
                        title,
                    ))),
            ),
        );
    }
    left = left.child(actions.child(dashed_add("obw-add-task", crate::t!("gs2.obw.add_task"))));
    left = left
        .child(section_sep())
        .child(h2(crate::t!("gs2.obw.resources_title")))
        .child(
            div()
                .mt(z(8.))
                .mb(z(24.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(text(
                    14.,
                    18.,
                    FontWeight::MEDIUM,
                    idx::TEXT_DEFAULT,
                    crate::t!("gs2.obw.resources_desc"),
                ))
                .children(
                    [
                        "gs2.obw.res_perk1",
                        "gs2.obw.res_perk2",
                        "gs2.obw.res_perk3",
                    ]
                    .into_iter()
                    .map(|k| {
                        div()
                            .pl(z(16.))
                            .flex()
                            .gap(z(6.))
                            .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, "•"))
                            .child(div().flex_1().min_w_0().child(text(
                                14.,
                                18.,
                                FontWeight::MEDIUM,
                                idx::TEXT_DEFAULT,
                                crate::t!(k),
                            )))
                    }),
                ),
        );
    let mut res = div().my(z(8.)).w_full().pr(z(8.)).flex().flex_col();
    for (i, r) in h
        .get("resource_channels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let cid = str_of(r, "channel_id").to_owned();
        let writable = c
            .list
            .iter()
            .find(|x| x.id == cid)
            .is_none_or(|x| x.chattable);
        let title = if str_of(r, "title").is_empty() {
            c.name(&cid)
        } else {
            str_of(r, "title").to_owned()
        };
        res = res.child(
            div()
                .mt(z(12.))
                .child(
                    div()
                        .w_full()
                        .px(z(16.))
                        .py(z(12.))
                        .rounded(z(12.))
                        .bg(t(idx::BACKGROUND_BASE_LOWER))
                        .when(writable, |d| {
                            d.border_2().border_color(t(idx::RED_400)).mb(z(4.))
                        })
                        .flex()
                        .items_center()
                        .gap(z(16.))
                        .child(
                            div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(
                                text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, title)
                                    .truncate(),
                            ),
                        )
                        .child(
                            button_sm(
                                ("obw-res-edit", i),
                                SharedString::default(),
                                BtnKind::Primary,
                            )
                            .min_w(z(32.))
                            .w(z(32.))
                            .px(z(0.))
                            .child(icon("pen", 16., idx::WHITE)),
                        ),
                )
                .when(writable, |d| {
                    d.child(text(
                        12.,
                        16.,
                        FontWeight::MEDIUM,
                        idx::TEXT_FEEDBACK_CRITICAL,
                        crate::t!("gs2.obw.res_readonly"),
                    ))
                }),
        );
    }
    left = left.child(res.child(dashed_add("obw-add-res", crate::t!("gs2.obw.add_resource"))));
    left = left
        .child(section_sep())
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(h2(crate::t!("gs2.obw.banner_title")))
                .child(
                    div()
                        .h(z(18.))
                        .pl(z(8.))
                        .pr(z(8.))
                        .rounded(z(12.))
                        .bg(t(idx::BACKGROUND_MOD_STRONG))
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(icon("boost", 12., idx::GUILD_BOOSTING_PINK))
                        .child(text(
                            14.,
                            18.,
                            FontWeight::MEDIUM,
                            idx::TEXT_SUBTLE,
                            crate::t!("gs2.obw.unlocked"),
                        )),
                ),
        )
        .child(
            div()
                .my(z(8.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .mb(z(16.))
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap(z(8.))
                        .child(body_sm(crate::t!("gs2.obw.banner_desc")))
                        .child(div().mt(z(8.)).flex().child(button(
                            "obw-banner-up",
                            crate::t!("gs2.obw.banner_upload"),
                            BtnKind::Primary,
                        ))),
                )
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(z(180.))
                        .rounded(z(12.))
                        .bg(gpui::linear_gradient(
                            45.,
                            gpui::linear_color_stop(crate::theme::c(0x4E76E0), 0.),
                            gpui::linear_color_stop(crate::theme::c(0xA56CEB), 1.),
                        ))
                        .child(
                            div()
                                .absolute()
                                .top(z(16.))
                                .right(z(16.))
                                .size(z(28.))
                                .rounded_full()
                                .bg(gpui::white())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("image-plus", 18., idx::BLACK)),
                        ),
                ),
        );
    let tab = super::flag(guild, "obw-preview-member").unwrap_or(false);
    let right = div()
        .w(z(264.))
        .flex_none()
        .pt(z(8.))
        .flex()
        .flex_col()
        .child(div().flex().justify_center().child(text(
            14.,
            18.,
            FontWeight::MEDIUM,
            idx::TEXT_DEFAULT,
            crate::t!("gs2.obw.example"),
        )))
        .child(
            gpui::img(SharedString::from(if tab {
                PREVIEW_EXISTING
            } else {
                PREVIEW_NEW
            }))
            .mt(z(10.))
            .w(z(264.))
            .h(z(264. * 1624. / 750.)),
        )
        .child(
            div()
                .p(z(4.))
                .gap(z(4.))
                .rounded(z(12.))
                .bg(t(idx::BACKGROUND_BASE_LOWER))
                .flex()
                .child(pill(
                    guild,
                    "obw-tab-new",
                    crate::t!("gs2.obw.tab_new"),
                    !tab,
                    false,
                    cx,
                ))
                .child(pill(
                    guild,
                    "obw-tab-member",
                    crate::t!("gs2.obw.tab_member"),
                    tab,
                    true,
                    cx,
                )),
        );
    div()
        .w_full()
        .pb(z(48.))
        .flex()
        .items_start()
        .child(left)
        .child(right)
        .into_any_element()
}

fn pill(
    guild: Id,
    id: &'static str,
    label: SharedString,
    on: bool,
    v: bool,
    cx: &mut Context<PhoenixApp>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_1()
        .h(z(44.))
        .px(z(16.))
        .py(z(8.))
        .rounded(z(8.))
        .when(on, |d| d.bg(t(idx::BACKGROUND_SURFACE_HIGH)))
        .when(!on, |d| d.hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE))))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(cx.listener(move |_, _, _, cx| {
            super::set_flag(guild, "obw-preview-member", v);
            cx.notify();
        }))
        .child(text(
            14.,
            14.,
            FontWeight::MEDIUM,
            if on {
                idx::INTERACTIVE_TEXT_ACTIVE
            } else {
                idx::INTERACTIVE_TEXT_DEFAULT
            },
            label,
        ))
}

fn status_el(st: Status) -> Option<gpui::Div> {
    let (ic, col, key) = match st {
        Status::Good => ("check", idx::STATUS_POSITIVE, "gs2.obw.good"),
        Status::Warning => ("warning", idx::STATUS_WARNING, "gs2.obw.warning"),
        Status::Required => (
            "warning",
            idx::TEXT_FEEDBACK_CRITICAL,
            "gs2.obw.required_status",
        ),
        Status::None => return None,
    };
    let text_col = if st == Status::Warning {
        idx::TEXT_FEEDBACK_WARNING
    } else {
        col
    };
    Some(
        div()
            .flex()
            .items_center()
            .child(
                div()
                    .mr(z(if st == Status::Good { 2. } else { 4. }))
                    .child(icon(ic, 16., col)),
            )
            .child(text(
                14.,
                18.67,
                FontWeight::MEDIUM,
                text_col,
                crate::t!(key),
            )),
    )
}

#[allow(clippy::too_many_arguments)]
fn review_row(
    id: (&'static str, usize),
    title: SharedString,
    desc: AnyElement,
    ic: &'static str,
    started: bool,
    st: Status,
    target: usize,
    guild: Id,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let btn = if started {
        button_sm(id, crate::t!("gs2.edit"), BtnKind::Secondary)
    } else {
        button_sm(id, SharedString::default(), BtnKind::Secondary)
            .gap(z(4.))
            .child(crate::t!("gs2.obw.set_up"))
            .child(icon(
                "chat/arrow-right",
                16.,
                idx::CONTROL_SECONDARY_TEXT_DEFAULT,
            ))
    };
    div()
        .w_full()
        .p(z(16.))
        .bg(t(idx::BACKGROUND_BASE_LOWER))
        .flex()
        .justify_between()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(
                    div()
                        .size(z(40.))
                        .flex_none()
                        .rounded(z(48.))
                        .bg(t(idx::BACKGROUND_BASE_LOWEST))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(ic, 24., idx::INTERACTIVE_TEXT_DEFAULT)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(2.))
                        .child(text(
                            16.,
                            20.,
                            FontWeight::SEMIBOLD,
                            idx::TEXT_STRONG,
                            title,
                        ))
                        .child(desc),
                ),
        )
        .child(
            div()
                .ml(z(8.))
                .flex()
                .items_center()
                .children(status_el(st))
                .child(
                    div()
                        .ml(z(16.))
                        .child(btn.on_click(cx.listener(move |_, _, _, cx| {
                            set_step(guild, target);
                            cx.notify();
                        }))),
                ),
        )
        .into_any_element()
}

fn review_step(c: &Ctx, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = c.guild;
    let d = &c.draft;
    let (_, sel) = chattable_defaults(&c.list, &d.defaults);
    let from_q = assigned_minimum(&c.list, &d.defaults, &d.prompts).saturating_sub(sel);
    let started1 = sel > 0 || c.advanced;
    let desc1 = if c.advanced {
        crate::t!(
            "gs2.obw.r_defaults_adv",
            n = plural_channels(sel),
            m = plural_plain(from_q)
        )
    } else {
        crate::t!("gs2.obw.r_defaults", n = plural_channels(sel))
    };
    let (st2, a, total) = questions_status(&c.list, &d.defaults, &d.prompts);
    let pct = if total == 0 {
        0.
    } else {
        a as f32 / total as f32 * 100.
    };
    let lead = crate::t!("gs2.obw.r_of", n = a, of = total);
    let full: SharedString = format!("{lead}{}", crate::t!("gs2.obw.r_assignable")).into();
    let hl = t(if pct > 85. { idx::TEXT_STRONG } else { idx::STATUS_WARNING });
    let desc2 = div().text_size(z(12.)).line_height(z(16.)).text_color(t(idx::TEXT_DEFAULT)).child(gpui::StyledText::new(full).with_highlights([(0..lead.len(), gpui::HighlightStyle { color: Some(hl), ..Default::default() })]));
    let (st3, k3) = home_status(&c.home, &c.list);
    let started3 = home_configured(&c.home);
    let desc3 = text(
        12.,
        16.,
        FontWeight::NORMAL,
        if started3 && st3 == Status::Required {
            idx::TEXT_FEEDBACK_CRITICAL
        } else {
            idx::TEXT_DEFAULT
        },
        crate::t!(k3),
    );
    let divider = || {
        div()
            .mx(z(16.))
            .h(z(1.))
            .border_t_1()
            .border_color(t(idx::BORDER_SUBTLE))
    };
    div()
        .w_full()
        .rounded(z(8.))
        .overflow_hidden()
        .child(
            div()
                .px(z(16.))
                .py(z(12.))
                .rounded_tl(z(8.))
                .rounded_tr(z(8.))
                .bg(t(idx::BACKGROUND_BASE_LOWEST))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(2.))
                        .child(text(
                            16.,
                            20.,
                            FontWeight::SEMIBOLD,
                            idx::TEXT_STRONG,
                            crate::t!("gs2.obw.ready"),
                        ))
                        .child(text(
                            12.,
                            16.,
                            FontWeight::NORMAL,
                            idx::TEXT_DEFAULT,
                            crate::t!("gs2.obw.ready_desc"),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .gap(z(8.))
                        .child(button_sm(
                            "obw-preview",
                            crate::t!("gs2.obw.preview"),
                            BtnKind::Secondary,
                        ))
                        .child(
                            button_sm("obw-enable", crate::t!("gs2.obw.enable"), BtnKind::Primary)
                                .on_click(cx.listener(move |a, _, _, cx| {
                                    a.blocked_write(
                                        GuardedWrite::put_onboarding(
                                            guild.0,
                                            json!({ "enabled": true }),
                                        ),
                                        cx,
                                    );
                                })),
                        ),
                ),
        )
        .child(review_row(
            ("obw-r", 0),
            crate::t!(if c.advanced {
                "gs2.obw.defaults_adv"
            } else {
                "gs2.obw.defaults_title"
            }),
            text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, desc1).into_any_element(),
            "hash",
            started1,
            if started1 { Status::Good } else { Status::None },
            1,
            guild,
            cx,
        ))
        .child(divider())
        .child(divider())
        .child(review_row(
            ("obw-r", 1),
            crate::t!(if c.advanced {
                "gs2.obw.postjoin_title"
            } else {
                "gs2.obw.questions_short"
            }),
            desc2.into_any_element(),
            "friends",
            !d.prompts.is_empty(),
            st2,
            2,
            guild,
            cx,
        ))
        .child(divider())
        .child(review_row(
            ("obw-r", 2),
            crate::t!("gs2.obw.guide"),
            desc3.into_any_element(),
            "signpost",
            started3,
            st3,
            3,
            guild,
            cx,
        ))
        .into_any_element()
}

fn plural_channels(n: usize) -> SharedString {
    if n <= 1 {
        crate::t!("gs2.obw.n_default_one", n = n)
    } else {
        crate::t!("gs2.obw.n_default", n = n)
    }
}

fn plural_plain(n: usize) -> SharedString {
    if n <= 1 {
        crate::t!("gs2.obw.n_channel_one", n = n)
    } else {
        crate::t!("gs2.obw.n_channel", n = n)
    }
}

fn tip(i: usize) -> (&'static str, &'static str) {
    match i {
        0 => ("gs2.obw.tip1_title", "gs2.obw.tip1_body"),
        1 => ("gs2.obw.tip2_title", "gs2.obw.tip2_body"),
        2 => ("gs2.obw.tip3_title", "gs2.obw.tip3_body"),
        _ => ("gs2.obw.tip4_title", "gs2.obw.tip4_body"),
    }
}

fn arrow_button(
    id: &'static str,
    label: SharedString,
    kind: BtnKind,
    left: bool,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    let fg = if matches!(kind, BtnKind::Primary) {
        idx::CONTROL_PRIMARY_TEXT_DEFAULT
    } else {
        idx::CONTROL_SECONDARY_TEXT_DEFAULT
    };
    let ic = icon(
        if left {
            "settings/arrow-left"
        } else {
            "chat/arrow-right"
        },
        16.,
        fg,
    );
    let b = button_sm(id, SharedString::default(), kind).gap(z(4.));
    let b = if left {
        b.child(ic).child(label)
    } else {
        b.child(label).child(ic)
    };
    if enabled {
        b
    } else {
        b.opacity(0.5).cursor_default()
    }
}

pub fn notice(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !super::flag(guild, "onboarding-wizard").unwrap_or(false) {
        return None;
    }
    let ob = super::with(|s| {
        match s.data.get(&(
            guild,
            phoenix_api::rest::guild_server::build(&ServerCall::Onboarding { guild: sf(guild) })
                .path,
        )) {
            Some(super::Load::Ready(v)) => Some(v.clone()),
            _ => None,
        }
    })?;
    let c = load(app, guild, &ob, cx)?;
    let cur = step(guild);
    if c.has_changes(cur) {
        let advanced = c.advanced;
        let reset_fn: crate::ui::modals::settings_layout::Act = Rc::new(move |_, _, cx| {
            reset(guild);
            cx.notify();
        });
        let save_fn: crate::ui::modals::settings_layout::Act = Rc::new(move |a, _, cx| {
            let d = DRAFTS
                .with(|d| d.borrow().get(&guild).cloned())
                .unwrap_or_default();
            let body = if cur == 1 {
                defaults_body(&d.defaults, &d.prompts, advanced)
            } else {
                prompts_body(&d.prompts)
            };
            a.blocked_write(GuardedWrite::put_onboarding(guild.0, body), cx);
        });
        return Some(crate::ui::modals::settings_layout::unsaved_bar(
            reset_fn, save_fn, cx,
        ));
    }
    let tip_on = tip_open(guild, cur);
    let errors = c.has_errors(cur);
    let configured = c.configured(cur);
    let last = cur == STEPS - 1;
    let back = arrow_button(
        "obw-back",
        crate::t!("gs2.obw.back"),
        BtnKind::Secondary,
        true,
        !tip_on,
    )
    .when(!tip_on, |b| {
        b.on_click(cx.listener(move |_, _, _, cx| {
            if cur == 0 {
                super::set_flag(guild, "onboarding-wizard", false);
            } else {
                set_step(guild, cur - 1);
            }
            cx.notify();
        }))
    });
    let next_ok = !tip_on && !errors;
    let next = (!last).then(|| {
        arrow_button(
            "obw-next",
            crate::t!(if configured {
                "gs2.obw.next"
            } else {
                "gs2.obw.skip"
            }),
            if configured {
                BtnKind::Primary
            } else {
                BtnKind::Secondary
            },
            false,
            next_ok,
        )
        .when(next_ok, |b| {
            b.on_click(cx.listener(move |_, _, _, cx| {
                set_step(guild, cur + 1);
                cx.notify();
            }))
        })
    });
    let mut bar = div()
        .relative()
        .p(z(12.))
        .rounded(z(8.))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(4.)),
            blur_radius: z(12.),
            spread_radius: z(0.),
            inset: false,
        }])
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .child(div().when(tip_on, |d| d.opacity(0.)).child(back))
                .child(div().flex().items_center().gap(z(8.)).children(next)),
        );
    if tip_on {
        let (tk, bk) = tip(cur);
        bar = bar.child(
            div()
                .absolute()
                .bottom(z(64.))
                .left_0()
                .w(z(400.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_SURFACE_HIGH))
                .border_1()
                .border_color(t(idx::BORDER_SUBTLE))
                .flex()
                .flex_col()
                .child(div().mb(z(8.)).child(text(
                    14.,
                    18.,
                    FontWeight::MEDIUM,
                    idx::TEXT_DEFAULT,
                    crate::t!("gs2.obw.tip_step", n = cur + 1),
                )))
                .child(div().mb(z(16.)).child(text(
                    20.,
                    24.,
                    FontWeight::SEMIBOLD,
                    idx::TEXT_STRONG,
                    crate::t!(tk),
                )))
                .child(text(
                    14.,
                    18.,
                    FontWeight::NORMAL,
                    idx::TEXT_DEFAULT,
                    crate::t!(bk),
                ))
                .child(div().mt(z(16.)).flex().child(
                    button("obw-tip-ok", crate::t!("gs2.obw.got_it"), BtnKind::Primary).on_click(
                        cx.listener(move |_, _, _, cx| {
                            super::set_flag(guild, &format!("ob-tip-{cur}"), true);
                            cx.notify();
                        }),
                    ),
                )),
        );
    }
    Some(bar.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guild() -> Value {
        json!({ "id": "1", "roles": [{ "id": "1", "permissions": "1051648" }, { "id": "9", "permissions": "0" }] })
    }

    fn channels() -> Value {
        json!([
            { "id": "10", "type": 0, "name": "honeypot", "position": 0 },
            { "id": "20", "type": 4, "name": "Important", "position": 1 },
            { "id": "21", "type": 0, "name": "rules", "position": 0, "parent_id": "20", "permission_overwrites": [{ "id": "1", "type": 0, "allow": "0", "deny": "2048" }] },
            { "id": "22", "type": 5, "name": "news", "position": 1, "parent_id": "20", "permission_overwrites": [{ "id": "1", "type": 0, "allow": "0", "deny": "1024" }] },
            { "id": "23", "type": 2, "name": "voice", "position": 0, "parent_id": "20" },
            { "id": "30", "type": 11, "name": "thread", "parent_id": "10" }
        ])
    }

    fn list() -> Vec<Chan> {
        chans(&channels(), "1", everyone_perms(&guild()))
    }

    #[test]
    fn public_and_chattable_follow_everyone_overwrites() {
        let l = list();
        let get = |id: &str| l.iter().find(|c| c.id == id).unwrap().clone();
        assert!(get("10").public && get("10").chattable);
        assert!(get("21").public && !get("21").chattable);
        assert!(!get("22").public);
        assert!(get("22").private && !get("10").private);
        let muted = json!([{ "id": "40", "type": 2, "name": "afk", "position": 0, "permission_overwrites": [{ "id": "1", "type": 0, "allow": "0", "deny": "1048576" }] }]);
        let afk = chans(&muted, "1", everyone_perms(&guild()));
        assert!(!afk[0].public && afk[0].private);
        assert!(l.iter().all(|c| c.id != "30"));
        let g = groups(&l);
        assert_eq!(
            g[0].1.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["10"]
        );
        assert_eq!(
            g[1].1.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["21", "22", "23"]
        );
        assert_eq!(
            chans(&channels(), "1", ADMINISTRATOR)
                .iter()
                .filter(|c| c.public)
                .count(),
            5
        );
    }

    #[test]
    fn counts_match_the_client_rules() {
        let l = list();
        assert_eq!(chattable_defaults(&l, &["20".into()]), (1, 2));
        assert_eq!(chattable_defaults(&l, &["10".into(), "22".into()]), (1, 1));
        let p = Prompt {
            required: true,
            options: vec![
                Opt {
                    channel_ids: vec!["21".into()],
                    ..Default::default()
                },
                Opt {
                    channel_ids: vec!["23".into(), "21".into()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            assigned_minimum(&l, &["10".into()], std::slice::from_ref(&p)),
            2
        );
        let (a, m) = coverage(&l, &[], std::slice::from_ref(&p));
        assert_eq!((a.len(), m.len()), (2, 1));
        assert_eq!(questions_status(&l, &[], &[]).0, Status::None);
        assert_eq!(questions_status(&l, &[], std::slice::from_ref(&p)).0, Status::Warning);
        assert_eq!(questions_status(&l, &["10".into()], &[p]).0, Status::Good);
    }

    #[test]
    fn local_validation_matches_the_client() {
        let roles: HashSet<String> = ["9".to_owned()].into();
        let chs: HashSet<String> = ["10".to_owned()].into();
        let empty = new_prompt("5".into(), true);
        let e = validate(std::slice::from_ref(&empty), &roles, &chs);
        let e0 = e[0].clone().unwrap();
        assert_eq!(
            (e0.title, e0.options, e0.config),
            (
                Some("gs2.obw.err_title"),
                Some("gs2.obw.err_options"),
                false
            )
        );
        let ok = Prompt {
            id: "6".into(),
            title: "Q".into(),
            options: vec![Opt {
                role_ids: vec!["9".into()],
                ..Default::default()
            }],
            in_onboarding: true,
            ..Default::default()
        };
        assert_eq!(
            validate(std::slice::from_ref(&ok), &roles, &chs),
            vec![None]
        );
        let dead = Prompt {
            options: vec![Opt {
                role_ids: vec!["404".into()],
                channel_ids: vec!["405".into()],
                ..Default::default()
            }],
            ..ok.clone()
        };
        assert_eq!(
            validate(&[dead], &roles, &chs)[0]
                .clone()
                .unwrap()
                .option_errs,
            vec![Some("gs2.obw.err_option")]
        );
        let single = Prompt {
            id: "7".into(),
            single_select: true,
            ..ok.clone()
        };
        assert_eq!(
            validate(&[single, ok.clone()], &roles, &chs)[0]
                .clone()
                .unwrap()
                .option_errs,
            vec![Some("gs2.obw.err_single")]
        );
        let many: Vec<Prompt> = (0..5)
            .map(|i| Prompt {
                id: i.to_string(),
                ..ok.clone()
            })
            .collect();
        assert!(
            validate(&many, &roles, &chs)
                .iter()
                .all(|e| e.as_ref().is_some_and(|e| e.config))
        );
    }

    #[test]
    fn bodies_match_the_client_serializer() {
        let ob = json!({ "prompts": [{ "id": "1", "title": "T", "options": [{ "id": "2", "title": "Oui", "description": "", "emoji": { "id": "3", "name": "smirk", "animated": false }, "role_ids": ["4"], "channel_ids": [] }], "single_select": false, "required": false, "in_onboarding": true, "type": 0 }] });
        let p = parse_prompts(&ob);
        let j = prompt_json(&p[0]);
        assert_eq!(j["options"][0]["emoji_id"], "3");
        assert_eq!(j["options"][0]["emoji_name"], "smirk");
        assert_eq!(j["in_onboarding"], true);
        assert_eq!(parse_prompts(&json!({ "prompts": [j] })), p);
        assert_eq!(
            defaults_body(&["10".into()], &p, false),
            json!({ "default_channel_ids": ["10"] })
        );
        assert_eq!(defaults_body(&[], &p, true)["prompts"][0]["title"], "T");
        assert_eq!(prompts_body(&[])["prompts"], json!([]));
    }

    #[test]
    fn home_status_follows_the_review_rules() {
        let l = list();
        assert_eq!(
            home_status(&json!({}), &l),
            (Status::None, "gs2.obw.r_home_invalid")
        );
        let res = json!({ "welcome_message": { "author_ids": [], "message": "" }, "resource_channels": [{ "channel_id": "21" }] });
        assert_eq!(home_status(&res, &l).0, Status::Required);
        let ok = json!({ "welcome_message": { "author_ids": ["5"], "message": "Salut" }, "new_member_actions": [{ "channel_id": "10" }] });
        assert_eq!(home_status(&ok, &l), (Status::Good, "gs2.obw.r_home_done"));
    }

    #[test]
    fn steps_are_clamped() {
        let g = Id(91);
        set_step(g, 9);
        assert_eq!(step(g), STEPS - 1);
        assert!(tip_open(g, 1));
        dismiss_tips(g);
        assert!(!tip_open(g, 1));
    }
}
