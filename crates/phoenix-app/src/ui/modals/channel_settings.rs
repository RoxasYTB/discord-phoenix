use std::rc::Rc;

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{InputState, TextareaState};
use phoenix_api::rest::guild_manage::AdminCall;
use serde_json::{Value, json};

use super::{
    Modal, input_box,
    settings_layout::{Act, Pick, SideItem, overlay, overlay_with, unsaved_bar},
};
use crate::{
    actions::{admin::sf, with_modal},
    app::PhoenixApp,
    model::{Channel, ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::switch,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Overview,
    Permissions,
    Invites,
    Integrations,
    Webhooks,
    Followed,
}

pub const SLOWMODE: [u32; 14] = [0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21_600];
pub const SPOILER_CHANNEL: u64 = 1 << 21;
pub const REQUIRE_TAG: u64 = 1 << 4;
pub const AUTO_ARCHIVE: [(u32, &str); 4] = [(60, "thread.aa_1h"), (1440, "thread.aa_24h"), (4320, "thread.aa_3d"), (10_080, "thread.aa_1w")];
pub const REGIONS: [(&str, &str); 12] = [
    ("brazil", "Brazil"),
    ("hongkong", "Hong Kong"),
    ("india", "India"),
    ("japan", "Japan"),
    ("rotterdam", "Rotterdam"),
    ("singapore", "Singapore"),
    ("southafrica", "South Africa"),
    ("sydney", "Sydney"),
    ("us-central", "US Central"),
    ("us-east", "US East"),
    ("us-south", "US South"),
    ("us-west", "US West"),
];

pub fn slowmode_label(s: u32) -> SharedString {
    match s {
        0 => crate::t!("cs.off"),
        s if s < 60 => crate::t!("cs.n_seconds", n = s),
        60 => crate::t!("cs.one_minute"),
        s if s < 3600 => crate::t!("cs.n_minutes", n = s / 60),
        3600 => crate::t!("cs.one_hour"),
        s => crate::t!("cs.n_hours", n = s / 3600),
    }
}

pub fn short_duration(s: u32) -> String {
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s => format!("{}h", s / 3600),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Overwrite {
    pub id: Id,
    pub kind: u8,
    pub allow: u64,
    pub deny: u64,
}

impl Overwrite {
    pub fn state(&self, bit: u64) -> i8 {
        if self.allow & bit != 0 {
            1
        } else if self.deny & bit != 0 {
            -1
        } else {
            0
        }
    }

    pub fn set(&mut self, bit: u64, state: i8) {
        self.allow &= !bit;
        self.deny &= !bit;
        match state {
            1 => self.allow |= bit,
            -1 => self.deny |= bit,
            _ => {}
        }
    }

    pub fn json(&self) -> Value {
        json!({ "id": self.id.0.to_string(), "type": self.kind, "allow": self.allow.to_string(), "deny": self.deny.to_string() })
    }
}

fn num(x: Option<&Value>) -> u64 {
    x.and_then(|x| x.as_str().and_then(|s| s.parse().ok()).or_else(|| x.as_u64())).unwrap_or(0)
}

pub fn parse_overwrites(v: &Value) -> Vec<Overwrite> {
    v.get("permission_overwrites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|o| Some(Overwrite { id: Id(num(o.get("id")).max(1)), kind: o.get("type")?.as_u64()? as u8, allow: num(o.get("allow")), deny: num(o.get("deny")) }))
        .collect()
}

pub fn same_overwrites(a: &[Overwrite], b: &[Overwrite]) -> bool {
    let norm = |l: &[Overwrite]| {
        let mut v: Vec<(u64, u8, u64, u64)> = l.iter().filter(|o| o.allow != 0 || o.deny != 0).map(|o| (o.id.0, o.kind, o.allow, o.deny)).collect();
        v.sort();
        v
    };
    norm(a) == norm(b)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Values {
    pub kind: u8,
    pub slow: u32,
    pub thread_slow: u32,
    pub nsfw: bool,
    pub flags: u64,
    pub bitrate: u32,
    pub user_limit: u32,
    pub region: Option<String>,
    pub video: u8,
    pub archive: u32,
    pub layout: u8,
    pub sort: Option<u8>,
    pub tag_setting: Option<String>,
}

impl Values {
    pub fn from_json(v: &Value) -> Self {
        let u = |k: &str| v.get(k).and_then(Value::as_u64);
        Self {
            kind: u("type").unwrap_or(0) as u8,
            slow: u("rate_limit_per_user").unwrap_or(0) as u32,
            thread_slow: u("default_thread_rate_limit_per_user").unwrap_or(0) as u32,
            nsfw: v.get("nsfw").and_then(Value::as_bool).unwrap_or(false),
            flags: u("flags").unwrap_or(0),
            bitrate: u("bitrate").unwrap_or(64_000) as u32,
            user_limit: u("user_limit").unwrap_or(0) as u32,
            region: v.get("rtc_region").and_then(Value::as_str).map(str::to_owned),
            video: u("video_quality_mode").unwrap_or(1) as u8,
            archive: u("default_auto_archive_duration").unwrap_or(4320) as u32,
            layout: u("default_forum_layout").unwrap_or(0) as u8,
            sort: u("default_sort_order").map(|x| x as u8),
            tag_setting: v.get("default_tag_setting").and_then(Value::as_str).map(str::to_owned),
        }
    }

    pub fn visibility(&self) -> usize {
        if self.nsfw {
            2
        } else if self.flags & SPOILER_CHANNEL != 0 {
            1
        } else {
            0
        }
    }

    pub fn set_visibility(&mut self, n: usize) {
        self.nsfw = n == 2;
        if n == 1 {
            self.flags |= SPOILER_CHANNEL;
        } else {
            self.flags &= !SPOILER_CHANNEL;
        }
    }
}

pub fn patch_body(name: &str, topic: &str, v0: &Values, v: &Values) -> Value {
    let mut b = json!({
        "name": name.trim(),
        "type": v.kind,
        "topic": topic,
        "bitrate": v.bitrate,
        "user_limit": v.user_limit,
        "nsfw": v.nsfw,
        "flags": v.flags,
        "rate_limit_per_user": v.slow,
    });
    if v.thread_slow != v0.thread_slow {
        b["default_thread_rate_limit_per_user"] = json!(v.thread_slow);
    }
    if v.archive != v0.archive {
        b["default_auto_archive_duration"] = json!(v.archive);
    }
    if v.region != v0.region {
        b["rtc_region"] = json!(v.region);
    }
    if v.video != v0.video {
        b["video_quality_mode"] = json!(v.video);
    }
    if v.layout != v0.layout {
        b["default_forum_layout"] = json!(v.layout);
    }
    if v.sort != v0.sort {
        b["default_sort_order"] = json!(v.sort);
    }
    if v.tag_setting != v0.tag_setting {
        b["default_tag_setting"] = json!(v.tag_setting);
    }
    b
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThreadFields {
    pub auto0: u32,
    pub auto: u32,
    pub flags0: u64,
    pub flags: u64,
}

pub fn thread_edit(name0: &str, name: &str, slow0: u32, slow: u32, t: &ThreadFields) -> phoenix_api::rest::guild_manage::ThreadEdit {
    let name = name.trim();
    phoenix_api::rest::guild_manage::ThreadEdit {
        name: (name != name0 && !name.is_empty()).then(|| name.to_owned()),
        slowmode: (slow != slow0).then_some(slow),
        auto_archive_duration: (t.auto != t.auto0).then_some(t.auto),
        flags: (t.flags != t.flags0).then_some(t.flags),
        ..Default::default()
    }
}

pub struct TagDraft {
    pub index: Option<usize>,
    pub name0: String,
    pub name: Option<Entity<InputState>>,
    pub emoji: Option<Value>,
    pub moderated: bool,
}

impl TagDraft {
    pub fn new() -> Self {
        Self { index: None, name0: String::new(), name: None, emoji: None, moderated: false }
    }

    pub fn edit(index: usize, tag: &Value) -> Self {
        let emoji = emoji_value(tag.get("emoji_id").and_then(Value::as_str), tag.get("emoji_name").and_then(Value::as_str));
        Self {
            index: Some(index),
            name0: tag.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            name: None,
            emoji,
            moderated: tag.get("moderated").and_then(Value::as_bool).unwrap_or(false),
        }
    }
}

impl Default for TagDraft {
    fn default() -> Self {
        Self::new()
    }
}

pub const TAG_MAX: usize = 20;

pub fn emoji_value(id: Option<&str>, name: Option<&str>) -> Option<Value> {
    match (id.filter(|i| !i.is_empty() && *i != "0"), name.filter(|n| !n.is_empty())) {
        (None, None) => None,
        (id, name) => Some(json!({ "emoji_id": id, "emoji_name": if id.is_some() { None } else { name } })),
    }
}

pub fn tag_json(prev: Option<&Value>, name: &str, emoji: Option<&Value>, moderated: bool) -> Value {
    let name: String = name.trim().chars().take(TAG_MAX).collect::<String>().trim_end().to_owned();
    let mut t = json!({
        "name": name,
        "emoji_id": emoji.and_then(|e| e.get("emoji_id")).cloned().unwrap_or(Value::Null),
        "emoji_name": emoji.and_then(|e| e.get("emoji_name")).cloned().unwrap_or(Value::Null),
        "moderated": moderated,
    });
    if let Some(id) = prev.and_then(|p| p.get("id")).filter(|i| !i.is_null()) {
        t["id"] = id.clone();
    }
    t
}

pub fn forum_extras(b: &mut Value, tags0: &[Value], tags: &[Value], r0: &Option<Value>, r: &Option<Value>) {
    if tags != tags0 {
        b["available_tags"] = json!(tags);
    }
    if r != r0 {
        b["default_reaction_emoji"] = r.clone().unwrap_or(Value::Null);
    }
}

pub fn insert_at(text: &str, range: std::ops::Range<usize>, ins: &str) -> (String, std::ops::Range<usize>) {
    let (a, b) = (range.start.min(text.len()), range.end.min(text.len()));
    let (a, b) = (a.min(b), a.max(b));
    let a = (0..=a).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
    let b = if range.start >= range.end { a } else { (b..=text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(text.len()) };
    let out = format!("{}{ins}{}", &text[..a], &text[b..]);
    let at = a + ins.len();
    (out, at..at)
}

pub fn pick_field_emoji(f: crate::ui::pickers::Field, text: String, custom: Option<(Id, String, bool)>, window: &mut Window, cx: &mut gpui::App) {
    use crate::ui::pickers::Field;
    let emoji = match &custom {
        Some((id, _, _)) => json!({ "emoji_id": id.0.to_string(), "emoji_name": Value::Null }),
        None => json!({ "emoji_id": Value::Null, "emoji_name": text.clone() }),
    };
    let mut name_input = None;
    let mut topic_input = None;
    with_state(|s| match f {
        Field::ChannelName => name_input = s.name.clone().filter(|_| custom.is_none()),
        Field::ChannelTopic => topic_input = s.topic.clone(),
        Field::ForumReaction => s.default_reaction = Some(emoji.clone()),
        Field::TagEmoji => {
            if let Some(t) = s.tag_edit.as_mut() {
                t.emoji = Some(emoji.clone());
            }
        }
    });
    if let Some(i) = name_input {
        i.update(cx, |st, cx| {
            let (t, r) = insert_at(&st.value(), st.selected_range(), &text);
            st.set_value(t, window, cx);
            st.set_selected_range(r, cx);
            st.focus(window, cx);
        });
    }
    if let Some(i) = topic_input {
        i.update(cx, |st, cx| {
            let (t, r) = insert_at(&st.value(), st.selected_range(), &text);
            st.set_value(t, window, cx);
            st.set_selected_range(r, cx);
            st.focus(window, cx);
        });
    }
}

pub struct State {
    pub channel: Id,
    pub guild: Option<Id>,
    pub parent: Option<Id>,
    pub kind: ChannelKind,
    pub section: Section,
    pub name0: String,
    pub topic0: String,
    pub v0: Values,
    pub v: Values,
    pub tags0: Vec<Value>,
    pub tags: Vec<Value>,
    pub default_reaction0: Option<Value>,
    pub default_reaction: Option<Value>,
    pub tag_edit: Option<TagDraft>,
    pub name: Option<Entity<InputState>>,
    pub topic: Option<Entity<TextareaState>>,
    pub perm_search: Option<Entity<InputState>>,
    pub overwrites0: Vec<Overwrite>,
    pub overwrites: Vec<Overwrite>,
    pub parent_overwrites: Option<Vec<Overwrite>>,
    pub roles: Option<Vec<Value>>,
    pub selected: Option<Id>,
    pub loaded: bool,
    pub invites: Option<Vec<Value>>,
    pub webhooks: Option<Vec<Value>>,
    pub open_webhook: Option<String>,
    pub webhook_name: Option<Entity<InputState>>,
    pub webhook_avatar: Option<std::path::PathBuf>,
    pub webhook_channel: Option<Id>,
    pub thread: Option<ThreadFields>,
    pub slow0: u32,
    pub slow: u32,
    pub open_select: Option<u8>,
    pub advanced_open: bool,
    pub add_members: Option<super::channel_perms::AddMembers>,
    pub confirm_delete: bool,
    pub security: Option<super::privacy::Security>,
}

impl State {
    pub fn new(ch: &Channel) -> Self {
        let topic = ch.topic.as_ref().map(|t| t.to_string()).unwrap_or_default();
        let kind = match ch.kind {
            ChannelKind::Text => 0,
            ChannelKind::Voice => 2,
            ChannelKind::Category => 4,
            ChannelKind::Announcement => 5,
            ChannelKind::Stage => 13,
            ChannelKind::Forum => 15,
        };
        let v = Values { kind, bitrate: 64_000, video: 1, archive: 4320, ..Default::default() };
        Self {
            channel: ch.id,
            guild: ch.guild,
            parent: ch.parent,
            kind: ch.kind,
            section: Section::Overview,
            name0: ch.name.to_string(),
            topic0: topic,
            v0: v.clone(),
            v,
            tags0: Vec::new(),
            tags: Vec::new(),
            default_reaction0: None,
            default_reaction: None,
            tag_edit: None,
            name: None,
            topic: None,
            perm_search: None,
            overwrites0: Vec::new(),
            overwrites: Vec::new(),
            parent_overwrites: None,
            roles: None,
            selected: None,
            loaded: false,
            invites: None,
            webhooks: None,
            open_webhook: None,
            webhook_name: None,
            webhook_avatar: None,
            webhook_channel: None,
            thread: None,
            slow0: 0,
            slow: 0,
            open_select: None,
            advanced_open: true,
            add_members: None,
            confirm_delete: false,
            security: None,
        }
    }

    pub fn for_thread(ch: &Channel, info: &crate::model::ThreadInfo) -> Self {
        let mut s = Self::new(ch);
        s.slow0 = info.slowmode;
        s.slow = info.slowmode;
        s.thread = Some(ThreadFields { auto0: info.auto_archive, auto: info.auto_archive, flags0: info.flags, flags: info.flags });
        s.loaded = true;
        s
    }

    pub fn is_category(&self) -> bool {
        self.kind == ChannelKind::Category
    }

    fn thread_dirty(&self, cx: &gpui::App) -> Option<phoenix_api::rest::guild_manage::ThreadEdit> {
        let t = self.thread.as_ref()?;
        let name = self.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.name0.clone());
        let e = thread_edit(&self.name0, &name, self.slow0, self.slow, t);
        (e != Default::default()).then_some(e)
    }

    pub fn load_channel(&mut self, v: &Value) {
        self.overwrites0 = parse_overwrites(v);
        self.overwrites = self.overwrites0.clone();
        self.v0 = Values::from_json(v);
        self.v = self.v0.clone();
        if let Some(t) = v.get("topic").and_then(Value::as_str) {
            self.topic0 = t.to_owned();
        }
        self.tags0 = v.get("available_tags").and_then(Value::as_array).cloned().unwrap_or_default();
        self.tags = self.tags0.clone();
        self.default_reaction0 = v.get("default_reaction_emoji").filter(|x| !x.is_null()).cloned();
        self.default_reaction = self.default_reaction0.clone();
        self.loaded = true;
    }

    pub fn name_value(&self, cx: &gpui::App) -> String {
        self.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.name0.clone())
    }

    pub fn topic_value(&self, cx: &gpui::App) -> String {
        self.topic.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.topic0.clone())
    }

    pub fn overview_dirty(&self, cx: &gpui::App) -> bool {
        let name = self.name_value(cx);
        (name.trim() != self.name0 && !name.trim().is_empty()) || self.topic_value(cx) != self.topic0 || self.v != self.v0 || self.forum_dirty()
    }

    pub fn forum_dirty(&self) -> bool {
        self.tags != self.tags0 || self.default_reaction != self.default_reaction0
    }

    pub fn overwrites_dirty(&self) -> bool {
        !same_overwrites(&self.overwrites, &self.overwrites0)
    }

    pub fn everyone(&self) -> Option<&Overwrite> {
        self.guild.and_then(|g| self.overwrites.iter().find(|o| o.id == g))
    }

    pub fn is_private(&self) -> bool {
        self.everyone().is_some_and(|o| o.deny & phoenix_api::rest::guild_manage::VIEW_CHANNEL != 0)
    }

    pub fn overwrite_mut(&mut self, id: Id, kind: u8) -> &mut Overwrite {
        if let Some(i) = self.overwrites.iter().position(|o| o.id == id) {
            return &mut self.overwrites[i];
        }
        self.overwrites.push(Overwrite { id, kind, allow: 0, deny: 0 });
        self.overwrites.last_mut().expect("just pushed")
    }

    pub fn synced(&self) -> Option<bool> {
        self.parent?;
        let p = self.parent_overwrites.as_ref()?;
        Some(same_overwrites(&self.overwrites0, p))
    }
}

pub(super) fn with_state(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::ChannelSettings(s)) = m {
            f(s)
        }
    });
}

pub fn prepare_area(s: &mut State, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if s.topic.is_none() && s.thread.is_none() && matches!(s.kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum) {
        let ph = if s.kind == ChannelKind::Forum { crate::t!("cs.post_guidelines_ph") } else { crate::t!("cs.topic_ph") };
        let init = s.topic0.clone();
        s.topic = Some(cx.new(|cx| TextareaState::new(window, cx).auto_grow(8, 8).placeholder(ph).default_value(init)));
    }
}

pub fn prepare(s: &mut State, mk: &mut dyn FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    if s.name.is_none() {
        s.name = Some(mk(SharedString::default(), &s.name0.clone(), false));
    }
    if s.perm_search.is_none() && s.thread.is_none() {
        s.perm_search = Some(mk(crate::t!("cs.search_perms"), "", false));
    }
    if s.webhook_name.is_none()
        && let Some(id) = s.open_webhook.as_deref().and_then(|x| x.parse::<u64>().ok())
        && let Some(name) = s.webhooks.iter().flatten().find(|w| super::webhooks::webhook_id(w) == Some(id)).and_then(|w| w.get("name")?.as_str().map(str::to_owned))
    {
        s.webhook_name = Some(mk(SharedString::default(), &name, false));
    }
    if let Some(t) = s.tag_edit.as_mut()
        && t.name.is_none()
    {
        let init = t.name0.clone();
        t.name = Some(mk(crate::t!("cs.tag_name_ph"), &init, true));
    }
    if let Some(a) = s.add_members.as_mut()
        && a.search.is_none()
    {
        a.search = Some(mk(crate::t!("cs.add_members_ph"), "", true));
    }
}

pub fn pick_section(a: &mut PhoenixApp, sec: Section, cx: &mut Context<PhoenixApp>) {
    let mut need = Vec::new();
    with_state(|s| {
        s.section = sec;
        s.open_select = None;
        match sec {
            Section::Invites if s.invites.is_none() => need.push(AdminCall::ChannelInvites { channel: sf(s.channel) }),
            Section::Integrations | Section::Webhooks | Section::Followed if s.webhooks.is_none() => need.push(AdminCall::ChannelWebhooks { channel: sf(s.channel) }),
            Section::Permissions => {
                if s.roles.is_none()
                    && let Some(g) = s.guild
                {
                    need.push(AdminCall::Roles { guild: sf(g) });
                }
                if s.parent_overwrites.is_none()
                    && let Some(p) = s.parent
                {
                    need.push(AdminCall::Channel { channel: sf(p) });
                }
            }
            _ => {}
        }
    });
    for call in need {
        let kind = match &call {
            AdminCall::ChannelInvites { .. } => 0,
            AdminCall::ChannelWebhooks { .. } => 1,
            AdminCall::Roles { .. } => 2,
            _ => 3,
        };
        a.admin(call, cx, move |a, r, cx| {
            let v = match r {
                Ok(v) => v,
                Err(e) => {
                    a.parity_toast(e, cx);
                    Value::Null
                }
            };
            let list = v.as_array().cloned().unwrap_or_default();
            with_state(|s| match kind {
                0 => s.invites = Some(list),
                1 => s.webhooks = Some(list),
                2 => s.roles = Some(list),
                _ => s.parent_overwrites = Some(parse_overwrites(&v)),
            });
            cx.notify();
        });
    }
}

pub fn channel_icon(kind: ChannelKind, private: bool) -> &'static str {
    match (kind, private) {
        (ChannelKind::Voice, false) => "speaker",
        (ChannelKind::Voice, true) => "voice-lock",
        (ChannelKind::Announcement, false) => "megaphone",
        (ChannelKind::Announcement, true) => "announcement-lock",
        (ChannelKind::Forum, false) => "forum",
        (ChannelKind::Forum, true) => "forum-lock",
        (ChannelKind::Stage, false) => "stage",
        (ChannelKind::Stage, true) => "stage-lock",
        (ChannelKind::Category, _) => "folder",
        (ChannelKind::Text, false) => "hash",
        (ChannelKind::Text, true) => "text-lock",
    }
}

fn label(t: SharedString) -> gpui::Div {
    div().pb(z(8.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t)
}

fn thread_overview(s: &State, t: &ThreadFields, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    use super::settings_form::{OnPick, description, dropdown};
    let e = cx.entity();
    let toggle = |which: u8| -> Rc<dyn Fn(&mut gpui::App)> { Rc::new(move |_| with_state(|s| s.open_select = if s.open_select == Some(which) { None } else { Some(which) })) };
    let slow_pick: OnPick = {
        let e = e.clone();
        Rc::new(move |k, _, cx| {
            with_state(|s| {
                s.slow = SLOWMODE[k.min(SLOWMODE.len() - 1)];
                s.open_select = None;
            });
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let auto_pick: OnPick = Rc::new(move |k, _, cx| {
        with_state(|s| {
            if let Some(t) = s.thread.as_mut() {
                t.auto = AUTO_ARCHIVE[k.min(AUTO_ARCHIVE.len() - 1)].0;
            }
            s.open_select = None;
        });
        e.update(cx, |_, cx| cx.notify());
    });
    let mut d = div().flex().flex_col();
    if let Some(i) = &s.name {
        d = d.child(label(crate::t!("thread.name"))).child(input_box(i));
    }
    let slow_sel = SLOWMODE.iter().position(|x| *x == s.slow).unwrap_or(0);
    let auto_sel = AUTO_ARCHIVE.iter().position(|x| x.0 == t.auto).unwrap_or(2);
    d.child(
        div()
            .mt(z(24.))
            .child(label(crate::t!("admin.slowmode")))
            .child(dropdown("ts-slow", SLOWMODE.iter().map(|x| slowmode_label(*x)).collect(), slow_sel, s.open_select == Some(1), toggle(1), slow_pick, cx))
            .child(description(crate::t!("thread.slowmode_desc")).pt(z(8.))),
    )
    .child(
        div()
            .mt(z(24.))
            .child(label(crate::t!("thread.hide_after")))
            .child(dropdown("ts-auto", AUTO_ARCHIVE.iter().map(|(_, k)| crate::t!(k)).collect(), auto_sel, s.open_select == Some(2), toggle(2), auto_pick, cx))
            .child(description(crate::t!("thread.hide_after_desc")).pt(z(8.))),
    )
    .child(
        div()
            .id("ts-spoiler")
            .mt(z(24.))
            .flex()
            .items_center()
            .gap(z(16.))
            .cursor_pointer()
            .on_click(cx.listener(|_, _, _, cx| {
                with_state(|s| {
                    if let Some(t) = s.thread.as_mut() {
                        t.flags ^= SPOILER_CHANNEL;
                    }
                });
                cx.notify();
            }))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(super::settings_form::label(crate::t!("thread.spoiler")))
                    .child(description(crate::t!("thread.spoiler_desc"))),
            )
            .group(crate::ui::controls::SWITCH_ROW)
            .child(switch("ts-spoiler-switch", t.flags & SPOILER_CHANNEL != 0)),
    )
}

fn render_thread(app: &PhoenixApp, s: &State, t: &ThreadFields, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let parent = app.backend.thread_info(s.channel).and_then(|i| i.parent).and_then(|p| app.channels.get(&p)).map(|p| p.name.to_string()).unwrap_or_default();
    let items = vec![
        SideItem::ChannelHeader { icon: "threads", name: s.name0.clone().into(), category: Some(parent.into()) },
        SideItem::Item { label: crate::t!("admin.cs_overview"), index: 0, danger: false, icon: Some("info"), external: false },
        SideItem::Separator,
        SideItem::Item { label: crate::t!("thread.delete"), index: 99, danger: true, icon: Some("chat/trash"), external: false },
    ];
    let channel = s.channel;
    let pick: Pick = Rc::new(move |n, a, cx| {
        if n == 99 {
            a.confirm_delete_thread(channel, false, cx);
        }
    });
    let content = thread_overview(s, t, cx);
    let notice = s.thread_dirty(cx).map(|_| {
        let reset: Act = Rc::new(|_, w, cx| {
            let mut input = None;
            with_state(|s| {
                s.slow = s.slow0;
                if let Some(t) = s.thread.as_mut() {
                    t.auto = t.auto0;
                    t.flags = t.flags0;
                }
                input = s.name.clone().map(|i| (i, s.name0.clone()));
            });
            if let Some((i, v)) = input {
                i.update(cx, |st, cx| st.set_value(v, w, cx));
            }
        });
        let save: Act = Rc::new(|a, _, cx| save_thread(a, cx));
        unsaved_bar(reset, save, cx)
    });
    overlay("thread-settings", items, 0, pick, crate::t!("admin.cs_overview"), content.into_any_element(), notice, cx)
}

fn save_thread(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let mut out = None;
    with_modal(|m| {
        if let Some(Modal::ChannelSettings(s)) = m
            && let Some(edit) = s.thread_dirty(cx)
        {
            if let Some(n) = &edit.name {
                s.name0 = n.clone();
            }
            s.slow0 = s.slow;
            if let Some(t) = s.thread.as_mut() {
                t.auto0 = t.auto;
                t.flags0 = t.flags;
            }
            out = Some((s.channel, edit));
        }
    });
    let Some((thread, edit)) = out else { return };
    if let Some(n) = &edit.name {
        a.apply_channel_overview(thread, n, "");
    }
    a.admin_toast(AdminCall::EditThread { thread: sf(thread), edit }, None, cx);
}

const SECTIONS: [(Section, &str, &str); 4] = [
    (Section::Overview, "admin.cs_overview", "info"),
    (Section::Permissions, "admin.cs_permissions", "text-lock"),
    (Section::Invites, "admin.cs_invites", "invite"),
    (Section::Integrations, "admin.cs_integrations", "settings/puzzle"),
];

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if let Some(t) = s.thread.clone() {
        return render_thread(app, s, &t, cx);
    }
    let cat = s.is_category();
    let private = s.is_private();
    let category = s.parent.and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
    let mut items = vec![SideItem::ChannelHeader { icon: channel_icon(s.kind, private), name: s.name0.clone().into(), category }];
    for (n, (sec, key, icon)) in SECTIONS.iter().enumerate() {
        if cat && matches!(sec, Section::Invites | Section::Integrations) {
            continue;
        }
        items.push(SideItem::Item { label: crate::t!(key), index: n, danger: false, icon: Some(icon), external: false });
    }
    items.push(SideItem::Separator);
    items.push(SideItem::Item {
        label: crate::t!(if cat { "menu.delete_category" } else { "menu.delete_channel" }),
        index: 99,
        danger: true,
        icon: Some("chat/trash"),
        external: false,
    });
    let pick: Pick = Rc::new(move |n, a, cx| {
        if n == 99 {
            with_state(|s| s.confirm_delete = true);
        } else {
            pick_section(a, SECTIONS[n].0, cx);
        }
    });
    let selected = match s.section {
        Section::Webhooks | Section::Followed => 3,
        sec => SECTIONS.iter().position(|x| x.0 == sec).unwrap_or(0),
    };
    let (title, content) = match s.section {
        Section::Overview => (crate::t!("admin.cs_overview"), super::channel_overview::render(app, s, cx)),
        Section::Permissions => (SharedString::default(), super::channel_perms::render(app, s, cx)),
        Section::Invites => (crate::t!("admin.cs_invites"), super::webhooks::invites(s, cx)),
        Section::Integrations => (crate::t!("admin.cs_integrations"), super::webhooks::integrations(s, cx)),
        Section::Webhooks => (SharedString::default(), super::webhooks::webhooks_page(app, s, cx)),
        Section::Followed => (SharedString::default(), super::webhooks::followed_page(app, s, cx)),
    };
    let dirty = match s.section {
        Section::Overview => s.overview_dirty(cx),
        Section::Permissions => s.overwrites_dirty(),
        _ => false,
    };
    let hook_notice = super::webhooks::webhook_dirty(s, cx).map(|_| {
        let reset: Act = Rc::new(|_, _, cx| {
            with_state(|s| {
                s.webhook_name = None;
                s.webhook_avatar = None;
                s.webhook_channel = None;
            });
            cx.notify();
        });
        let save: Act = Rc::new(|a, _, cx| {
            super::webhooks::save_webhook(a, cx);
            with_state(|s| {
                s.webhook_name = None;
                s.webhook_avatar = None;
                s.webhook_channel = None;
            });
        });
        unsaved_bar(reset, save, cx)
    });
    let notice = dirty.then(|| {
        let reset: Act = Rc::new(|_, w, cx| {
            let mut inputs = Vec::new();
            let mut area = None;
            with_state(|s| {
                s.v = s.v0.clone();
                s.tags = s.tags0.clone();
                s.default_reaction = s.default_reaction0.clone();
                s.overwrites = s.overwrites0.clone();
                inputs = s.name.clone().map(|i| (i, s.name0.clone())).into_iter().collect();
                area = s.topic.clone().map(|i| (i, s.topic0.clone()));
            });
            for (i, v) in inputs {
                i.update(cx, |st, cx| st.set_value(v, w, cx));
            }
            if let Some((i, v)) = area {
                i.update(cx, |st, cx| st.set_value(v, w, cx));
            }
        });
        let save: Act = Rc::new(|a, _, cx| save(a, cx));
        unsaved_bar(reset, save, cx)
    });
    let notice = notice.or(hook_notice);
    let extra = match (&s.add_members, s.confirm_delete, &s.security) {
        (Some(am), _, _) => Some(super::channel_perms::add_members_modal(app, s, am, cx)),
        (None, true, _) => Some(super::channel_perms::delete_modal(s, cx)),
        (None, false, _) if s.tag_edit.is_some() => s.tag_edit.as_ref().map(|t| super::channel_overview::tag_modal(s, t, cx)),
        (None, false, Some(sec)) => {
            let e = cx.entity();
            let close: crate::ui::modal::Action = Rc::new(move |_, cx| {
                with_state(|s| s.security = None);
                e.update(cx, |_, cx| cx.notify());
            });
            Some(super::privacy::render_security_with(sec, close, false, cx))
        }
        _ => None,
    };
    overlay_with("channel-settings", items, selected, pick, title, content, notice, extra, cx)
}

pub fn save(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let mut out = None;
    with_modal(|m| {
        if let Some(Modal::ChannelSettings(s)) = m {
            let (name, topic) = (s.name_value(cx), s.topic_value(cx));
            let overview = s.overview_dirty(cx).then(|| {
                let name = if name.trim().is_empty() { s.name0.clone() } else { crate::actions::admin::channel_name(s.kind, &name) };
                let mut body = patch_body(&name, &topic, &s.v0, &s.v);
                forum_extras(&mut body, &s.tags0, &s.tags, &s.default_reaction0, &s.default_reaction);
                s.tags0 = s.tags.clone();
                s.default_reaction0 = s.default_reaction.clone();
                s.name0 = name.clone();
                s.topic0 = topic.clone();
                s.v0 = s.v.clone();
                (body, name, topic)
            });
            let removed: Vec<Id> = s.overwrites0.iter().filter(|o| !s.overwrites.iter().any(|x| x.id == o.id && (x.allow != 0 || x.deny != 0))).map(|o| o.id).collect();
            let changed: Vec<Overwrite> = s.overwrites.iter().filter(|o| (o.allow != 0 || o.deny != 0) && !s.overwrites0.contains(o)).cloned().collect();
            s.overwrites.retain(|o| o.allow != 0 || o.deny != 0 || Some(o.id) == s.guild);
            s.overwrites0 = s.overwrites.clone();
            out = Some((s.channel, overview, changed, removed));
        }
    });
    let Some((channel, overview, changed, removed)) = out else { return };
    if let Some((body, name, topic)) = overview {
        a.apply_channel_overview(channel, &name, &topic);
        a.admin_toast(AdminCall::PatchChannel { channel: sf(channel), body }, None, cx);
    }
    for o in changed {
        a.admin_toast(AdminCall::SetOverwrite { channel: sf(channel), target: sf(o.id), kind: o.kind, allow: o.allow, deny: o.deny }, None, cx);
    }
    for t in removed {
        a.admin_toast(AdminCall::DeleteOverwrite { channel: sf(channel), target: sf(t) }, None, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_state() {
        let mut o = Overwrite { id: Id(1), kind: 0, allow: 0, deny: 0 };
        o.set(1024, 1);
        assert_eq!((o.allow, o.deny, o.state(1024)), (1024, 0, 1));
        o.set(1024, -1);
        assert_eq!((o.allow, o.deny, o.state(1024)), (0, 1024, -1));
        o.set(1024, 0);
        assert_eq!(o.state(1024), 0);
    }

    #[test]
    fn parse() {
        let v = serde_json::json!({ "permission_overwrites": [{ "id": "7", "type": 1, "allow": "1024", "deny": "0" }] });
        assert_eq!(parse_overwrites(&v), vec![Overwrite { id: Id(7), kind: 1, allow: 1024, deny: 0 }]);
        assert_eq!(short_duration(5), "5s");
        assert_eq!(short_duration(120), "2m");
        assert_eq!(short_duration(21_600), "6h");
    }

    #[test]
    fn patch_body_matches_the_client() {
        let live = json!({ "id": "2", "type": 0, "name": "phoenix", "topic": null, "rate_limit_per_user": 0, "nsfw": false, "flags": 0 });
        let v0 = Values::from_json(&live);
        let mut v = v0.clone();
        v.slow = 5;
        assert_eq!(
            patch_body("phoenix", "", &v0, &v),
            json!({"name": "phoenix", "type": 0, "topic": "", "bitrate": 64000, "user_limit": 0, "nsfw": false, "flags": 0, "rate_limit_per_user": 5})
        );
        v.set_visibility(1);
        v.region = Some("rotterdam".into());
        let b = patch_body("phoenix", "", &v0, &v);
        assert_eq!((b["flags"].clone(), b["nsfw"].clone(), b["rtc_region"].clone()), (json!(SPOILER_CHANNEL), json!(false), json!("rotterdam")));
        v.set_visibility(2);
        assert_eq!((v.nsfw, v.flags & SPOILER_CHANNEL), (true, 0));
    }

    #[test]
    fn overwrite_order_and_empty_entries_do_not_count() {
        let a = vec![Overwrite { id: Id(1), kind: 0, allow: 0, deny: 1024 }, Overwrite { id: Id(2), kind: 0, allow: 1024, deny: 0 }];
        let mut b = vec![a[1].clone(), a[0].clone(), Overwrite { id: Id(3), kind: 1, allow: 0, deny: 0 }];
        assert!(same_overwrites(&a, &b));
        b[0].allow = 0;
        assert!(!same_overwrites(&a, &b));
    }

    #[test]
    fn forum_tags_and_reaction_go_in_the_patch_only_when_changed() {
        let tags0 = vec![json!({"id": "5", "name": "Bug", "emoji_id": null, "emoji_name": null, "moderated": false})];
        let mut b = json!({});
        forum_extras(&mut b, &tags0, &tags0, &None, &None);
        assert_eq!(b, json!({}));
        let mut tags = tags0.clone();
        tags[0] = tag_json(Some(&tags0[0]), "  Bug fixé et validé, très long  ", emoji_value(None, Some("✅")).as_ref(), true);
        tags.push(tag_json(None, "New", None, false));
        let r = emoji_value(Some("1424806138028888325"), Some("ignored"));
        forum_extras(&mut b, &tags0, &tags, &None, &r);
        assert_eq!(b["available_tags"][0], json!({"id": "5", "name": "Bug fixé et validé,", "emoji_id": null, "emoji_name": "✅", "moderated": true}));
        assert_eq!(b["available_tags"][1].get("id"), None);
        assert_eq!(b["default_reaction_emoji"], json!({"emoji_id": "1424806138028888325", "emoji_name": null}));
        let mut c = json!({});
        forum_extras(&mut c, &tags0, &tags0, &r, &None);
        assert_eq!(c["default_reaction_emoji"], Value::Null);
        assert_eq!(emoji_value(Some("0"), None), None);
    }

    #[test]
    fn emoji_insertion_keeps_the_cursor_after_the_emoji() {
        assert_eq!(insert_at("ab", 1..1, "✅"), ("a✅b".into(), 4..4));
        assert_eq!(insert_at("abc", 1..2, "x"), ("axc".into(), 2..2));
        assert_eq!(insert_at("é", 1..1, "x").0, "xé");
        assert_eq!(insert_at("", 5..5, "x"), ("x".into(), 1..1));
    }

    #[test]
    fn thread_settings_only_send_changes() {
        let t = ThreadFields { auto0: 4320, auto: 4320, flags0: 0, flags: 0 };
        assert_eq!(thread_edit("a", " a ", 0, 0, &t), Default::default());
        let e = thread_edit("a", "b", 0, 30, &ThreadFields { auto0: 4320, auto: 10_080, flags0: 0, flags: 0 });
        let body = phoenix_api::rest::guild_manage::thread_body(&e);
        assert_eq!(body, serde_json::json!({"name": "b", "rate_limit_per_user": 30, "auto_archive_duration": 10_080}));
        assert_eq!(thread_edit("a", "  ", 0, 0, &t).name, None);
        let spoiler = ThreadFields { flags0: 2, flags: 2 | SPOILER_CHANNEL, ..t };
        let body = phoenix_api::rest::guild_manage::thread_body(&thread_edit("a", "a", 0, 0, &spoiler));
        assert_eq!(body, serde_json::json!({"flags": 2 | (1 << 21)}));
    }
}
