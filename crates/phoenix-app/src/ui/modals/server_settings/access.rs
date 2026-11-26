use std::collections::HashMap;

use gpui::{AnyElement, Entity, FontWeight, HighlightStyle, IntoElement, MouseButton, ParentElement, SharedString, StyledText, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use phoenix_api::rest::guard::GuardedWrite;
use serde_json::{Value, json};

use super::{
    State,
    kit::{self, Cx, labels, text},
    with_state,
};
use crate::{
    app::PhoenixApp,
    theme::{
        c,
        generated::{global, idx},
        tok, z,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinType {
    Invite,
    Apply,
    Discoverable,
}

impl JoinType {
    pub fn from_sub(s: &str) -> Option<Self> {
        match s.split('+').next()? {
            "invite" => Some(Self::Invite),
            "apply" => Some(Self::Apply),
            "discoverable" => Some(Self::Discoverable),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Terms,
    Text,
    Paragraph,
    Choice,
}

impl FieldKind {
    pub fn of(v: &Value) -> Option<Self> {
        Some(match v.get("field_type")?.as_str()? {
            "TERMS" => Self::Terms,
            "TEXT_INPUT" => Self::Text,
            "PARAGRAPH" => Self::Paragraph,
            "MULTIPLE_CHOICE" => Self::Choice,
            _ => return None,
        })
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Terms => "TERMS",
            Self::Text => "TEXT_INPUT",
            Self::Paragraph => "PARAGRAPH",
            Self::Choice => "MULTIPLE_CHOICE",
        }
    }

    fn label(self) -> SharedString {
        match self {
            Self::Terms => crate::t!("gs.access.terms_kind"),
            Self::Text => crate::t!("gs.access.short"),
            Self::Paragraph => crate::t!("gs.access.paragraph"),
            Self::Choice => crate::t!("gs.access.choice"),
        }
    }

    fn icon(self) -> Option<&'static str> {
        match self {
            Self::Terms => Some("forum/book-check"),
            Self::Text => None,
            Self::Paragraph => Some("access/channel-list"),
            Self::Choice => Some("access/list-bullets"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Ineligible,
    Disqualified,
    EligibleDisabled,
    EligibleEnabled,
}

pub const AGE_RESTRICTED: u64 = 3;
pub const MAX_RULES: usize = 16;
pub const RULE_LEN: usize = 300;
pub const MAX_FIELDS: usize = 5;
pub const LABEL_LEN: usize = 300;
pub const CHOICE_LEN: usize = 150;
pub const MAX_CHOICES: usize = 8;
pub const DESCRIPTION_LEN: usize = 300;
pub const MAX_TAGS: usize = 10;
pub const TAG_LEN: usize = 30;
pub const PUBLIC: u8 = 1;
pub const PUBLIC_WITH_RECRUITMENT: u8 = 3;

fn has(guild: &Value, feature: &str) -> bool {
    guild["features"].as_array().is_some_and(|a| a.iter().any(|v| v.as_str() == Some(feature)))
}

pub fn join_type(guild: &Value) -> JoinType {
    if has(guild, "DISCOVERABLE") {
        JoinType::Discoverable
    } else if has(guild, "MEMBER_VERIFICATION_GATE_ENABLED") && has(guild, "MEMBER_VERIFICATION_MANUAL_APPROVAL") {
        JoinType::Apply
    } else {
        JoinType::Invite
    }
}

pub fn rules_required(guild: &Value) -> bool {
    has(guild, "MEMBER_VERIFICATION_GATE_ENABLED")
}

fn content_level(guild: &Value, key: &str) -> Option<u64> {
    guild.get(key).and_then(Value::as_u64)
}

pub fn age_restricted(guild: &Value) -> bool {
    content_level(guild, "owner_configured_content_level") == Some(AGE_RESTRICTED)
}

pub fn age_locked(guild: &Value, join: JoinType) -> bool {
    join == JoinType::Discoverable
        || (content_level(guild, "nsfw_level") == Some(AGE_RESTRICTED) && content_level(guild, "owner_configured_content_level") != Some(AGE_RESTRICTED))
}

pub fn options(guild: &Value, active: JoinType, admin: bool) -> [(JoinType, bool); 3] {
    let community = has(guild, "COMMUNITY");
    let locked = active == JoinType::Discoverable && !admin;
    [(JoinType::Invite, !locked), (JoinType::Apply, !locked), (JoinType::Discoverable, community && (active == JoinType::Discoverable || admin))]
}

fn ineligible_text(kind: JoinType, guild: &Value) -> SharedString {
    match kind {
        JoinType::Invite | JoinType::Apply => crate::t!("gs.access.only_admin_off"),
        JoinType::Discoverable if has(guild, "COMMUNITY") => crate::t!("gs.access.only_admin_on"),
        JoinType::Discoverable => crate::t!("gs.access.need_community"),
    }
}

pub fn settings_view(guild: &Value, checklist: &Value, now_ms: i64) -> View {
    let listed = has(guild, "DISCOVERABLE");
    if has(guild, "VERIFIED") {
        return if listed { View::EligibleEnabled } else { View::EligibleDisabled };
    }
    let sufficient = checklist.get("sufficient").and_then(Value::as_bool).unwrap_or(false);
    if listed {
        let grace = checklist.get("sufficient_without_grace_period").and_then(Value::as_bool).unwrap_or(false)
            && checklist
                .get("grace_period_end_date")
                .and_then(Value::as_str)
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .is_some_and(|d| d.timestamp_millis() > now_ms);
        return if sufficient || grace { View::EligibleEnabled } else { View::Disqualified };
    }
    if sufficient { View::EligibleDisabled } else { View::Ineligible }
}

pub fn form_fields(form: &Value) -> Vec<Value> {
    form.get("form_fields").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn terms_values(form: &Value) -> Vec<String> {
    form_fields(form)
        .iter()
        .find(|f| FieldKind::of(f) == Some(FieldKind::Terms))
        .and_then(|f| f.get("values").and_then(Value::as_array))
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: u64,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub join: JoinType,
    pub age: bool,
    pub terms: bool,
    pub rules: Vec<Rule>,
    pub fields: Vec<Value>,
    pub visibility: Option<u8>,
    pub description: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub emoji_info: Option<bool>,
    pub locale: Option<String>,
    pub next: u64,
}

impl Draft {
    pub fn new(guild: &Value, form: &Value, join: JoinType, default_question: &str) -> Self {
        let mut d = Self {
            join,
            age: join != JoinType::Discoverable && age_restricted(guild),
            terms: rules_required(guild),
            rules: Vec::new(),
            fields: Vec::new(),
            visibility: None,
            description: None,
            keywords: None,
            emoji_info: None,
            locale: None,
            next: 1,
        };
        let values = terms_values(form);
        if values.is_empty() {
            d.push_rule(String::new());
        } else {
            for v in values {
                d.push_rule(v);
            }
        }
        if join == JoinType::Apply {
            let fields = form_fields(form);
            let terms = fields.iter().find(|f| FieldKind::of(f) == Some(FieldKind::Terms)).cloned();
            d.fields = if fields.is_empty() || (fields.len() == 1 && terms.is_some()) {
                let q = json!({ "field_type": "TEXT_INPUT", "label": default_question, "required": true });
                terms.into_iter().chain(std::iter::once(q)).collect()
            } else {
                fields
            };
        }
        d
    }

    fn push_rule(&mut self, value: String) -> u64 {
        let id = self.next;
        self.next += 1;
        self.rules.push(Rule { id, value });
        id
    }

    pub fn add_rule(&mut self, example: Option<&str>) -> Option<u64> {
        if self.rules.len() >= MAX_RULES {
            return None;
        }
        if let Some(e) = example
            && let Some(last) = self.rules.last_mut()
            && last.value.is_empty()
        {
            last.value = clip(e, RULE_LEN);
            return Some(last.id);
        }
        Some(self.push_rule(example.map(|e| clip(e, RULE_LEN)).unwrap_or_default()))
    }

    pub fn remove_rule(&mut self, i: usize) {
        if i < self.rules.len() {
            self.rules.remove(i);
        }
        if self.rules.is_empty() {
            self.push_rule(String::new());
        }
    }

    pub fn rule_values(&self) -> Vec<String> {
        self.rules.iter().map(|r| r.value.trim().to_owned()).filter(|v| !v.is_empty()).collect()
    }

    pub fn add_field(&mut self, f: Value) -> bool {
        if self.fields.len() >= MAX_FIELDS {
            return false;
        }
        self.fields.push(f);
        true
    }

    pub fn remove_field(&mut self, i: usize) {
        if self.fields.len() > 1 && i < self.fields.len() {
            self.fields.remove(i);
        }
    }

    pub fn has_terms_field(&self) -> bool {
        self.fields.iter().any(|f| FieldKind::of(f) == Some(FieldKind::Terms))
    }
}

pub fn move_item<T>(v: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from == to || from >= v.len() || to >= v.len() {
        return false;
    }
    let x = v.remove(from);
    v.insert(to, x);
    true
}

pub fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn one_line(s: &str, max: usize) -> String {
    clip(&s.replace("\r\n", " ").replace(['\n', '\r'], " "), max)
}

fn verification_changed(guild: &Value, form: &Value, d: &Draft) -> bool {
    if has(guild, "MEMBER_VERIFICATION_MANUAL_APPROVAL") || rules_required(guild) != d.terms {
        return true;
    }
    if form_fields(form).len() > 1 {
        return true;
    }
    terms_values(form) != d.rule_values()
}

pub struct Ctx<'a> {
    pub guild: &'a Value,
    pub form: &'a Value,
    pub profile: &'a Value,
    pub metadata: &'a Value,
    pub splash: Option<String>,
}

impl Ctx<'_> {
    fn visibility(&self) -> Option<u8> {
        self.profile.get("visibility").and_then(Value::as_u64).map(|v| v as u8)
    }

    fn discovery_changed(&self, d: &Draft) -> bool {
        self.splash.is_some()
            || d.description.as_deref().is_some_and(|v| v != self.guild.get("description").and_then(Value::as_str).unwrap_or(""))
            || d.locale.as_deref().is_some_and(|v| Some(v) != self.guild.get("preferred_locale").and_then(Value::as_str))
            || d.keywords.as_ref().is_some_and(|k| *k != keywords(self.metadata))
            || d.emoji_info.is_some_and(|e| Some(e) != self.metadata.get("emoji_discoverability_enabled").and_then(Value::as_bool))
    }
}

pub fn keywords(metadata: &Value) -> Vec<String> {
    metadata.get("keywords").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dirty {
    pub verification: bool,
    pub guild: bool,
    pub profile: bool,
    pub age: bool,
}

impl Dirty {
    pub fn any(self) -> bool {
        self.verification || self.guild || self.profile || self.age
    }
}

pub fn dirty(x: &Ctx, d: &Draft) -> Dirty {
    let guild = join_type(x.guild) != d.join;
    let age = age_restricted(x.guild) != d.age;
    match d.join {
        JoinType::Invite => Dirty { verification: verification_changed(x.guild, x.form, d), guild, profile: false, age },
        JoinType::Apply => {
            let gate = rules_required(x.guild) && has(x.guild, "MEMBER_VERIFICATION_MANUAL_APPROVAL");
            let changed = x.form.is_null() || form_fields(x.form) != d.fields;
            Dirty { verification: !gate || changed, guild, profile: d.visibility.is_some_and(|v| Some(v) != x.visibility()), age }
        }
        JoinType::Discoverable => Dirty { verification: verification_changed(x.guild, x.form, d), guild: guild || x.discovery_changed(d), profile: false, age },
    }
}

pub fn validate(d: &Draft) -> Result<(), &'static str> {
    match d.join {
        JoinType::Invite | JoinType::Discoverable if d.terms && d.rule_values().is_empty() => Err("gs.access.need_rule"),
        JoinType::Apply if !d.fields.iter().any(|f| matches!(FieldKind::of(f), Some(FieldKind::Text | FieldKind::Paragraph | FieldKind::Choice))) => Err("gs.access.need_question"),
        _ => Ok(()),
    }
}

fn terms_fields(values: Vec<String>, label: &str) -> Value {
    if values.is_empty() { json!([]) } else { json!([{ "field_type": "TERMS", "label": label, "values": values, "required": true }]) }
}

fn features_without(guild: &Value, drop: &[&str], add: &[&str]) -> Value {
    let mut out: Vec<String> = guild["features"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).filter(|f| !drop.contains(f)).map(str::to_owned).collect();
    for a in add {
        if !out.iter().any(|f| f == a) {
            out.push((*a).to_owned());
        }
    }
    json!(out)
}

pub fn writes(guild_id: u64, x: &Ctx, d: &Draft, terms_label: &str) -> Vec<GuardedWrite> {
    let dirty = dirty(x, d);
    let mut out = Vec::new();
    if dirty.age {
        out.push(GuardedWrite::edit_guild(guild_id, json!({ "owner_configured_content_level": if d.age { AGE_RESTRICTED } else { 0 } })));
    }
    let listed = has(x.guild, "DISCOVERABLE");
    match d.join {
        JoinType::Invite => {
            if listed {
                out.push(GuardedWrite::edit_guild(guild_id, json!({ "features": features_without(x.guild, &["DISCOVERABLE"], &[]) })));
            }
            if dirty.verification {
                out.push(GuardedWrite::member_verification(guild_id, terms_fields(d.rule_values(), terms_label), d.terms));
            }
        }
        JoinType::Apply => {
            if listed {
                out.push(GuardedWrite::edit_guild(guild_id, json!({ "features": features_without(x.guild, &["DISCOVERABLE"], &[]) })));
            }
            if dirty.verification {
                out.push(GuardedWrite::member_verification(guild_id, json!(d.fields), true));
            }
            if dirty.profile
                && let Some(v) = d.visibility
            {
                out.push(GuardedWrite::guild_profile_visibility(guild_id, v));
            }
        }
        JoinType::Discoverable => {
            if dirty.verification {
                out.push(GuardedWrite::member_verification(guild_id, terms_fields(d.rule_values(), terms_label), d.terms));
            }
            if dirty.guild {
                let mut body = json!({
                    "features": features_without(x.guild, &["MEMBER_VERIFICATION_MANUAL_APPROVAL"], &["DISCOVERABLE"]),
                    "description": d.description.clone().map_or_else(|| x.guild.get("description").cloned().unwrap_or(Value::Null), Value::String),
                    "preferred_locale": d.locale.clone().map_or_else(|| x.guild.get("preferred_locale").cloned().unwrap_or(Value::Null), Value::String),
                });
                if let Some(s) = &x.splash {
                    body["discovery_splash"] = json!(s);
                }
                out.push(GuardedWrite::edit_guild(guild_id, body));
                let mut meta = x.metadata.as_object().cloned().unwrap_or_default();
                if let Some(k) = &d.keywords {
                    meta.insert("keywords".into(), json!(k));
                }
                if let Some(e) = d.emoji_info {
                    meta.insert("emoji_discoverability_enabled".into(), json!(e));
                }
                meta.remove("guild_id");
                out.push(GuardedWrite::discovery_metadata(guild_id, Value::Object(meta)));
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragList {
    Rules,
    Fields,
    Choices,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drag {
    pub list: DragList,
    pub from: usize,
    pub over: usize,
}

pub struct Question {
    pub index: Option<usize>,
    pub kind: FieldKind,
    pub base: Value,
    pub label: String,
    pub choices: Vec<Rule>,
    pub next: u64,
    pub error: Option<SharedString>,
}

impl Question {
    pub fn new(kind: FieldKind, index: Option<usize>, base: Value) -> Self {
        let label = base.get("label").and_then(Value::as_str).unwrap_or_default().to_owned();
        let key = if kind == FieldKind::Terms { "values" } else { "choices" };
        let mut choices: Vec<String> = base.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        if choices.is_empty() {
            choices.push(String::new());
        }
        let n = choices.len() as u64;
        Self { index, kind, base, label, choices: choices.into_iter().enumerate().map(|(i, value)| Rule { id: i as u64 + 1, value }).collect(), next: n + 1, error: None }
    }

    fn max_choices(&self) -> usize {
        if self.kind == FieldKind::Terms { MAX_RULES } else { MAX_CHOICES }
    }

    fn choice_len(&self) -> usize {
        if self.kind == FieldKind::Terms { RULE_LEN } else { CHOICE_LEN }
    }

    pub fn add_choice(&mut self) -> Option<u64> {
        if self.choices.len() >= self.max_choices() {
            return None;
        }
        let id = self.next;
        self.next += 1;
        self.choices.push(Rule { id, value: String::new() });
        Some(id)
    }

    pub fn remove_choice(&mut self, i: usize) {
        if i < self.choices.len() {
            self.choices.remove(i);
        }
        if self.choices.is_empty() {
            self.add_choice();
        }
    }

    pub fn build(&self, terms_label: &str) -> Result<Value, &'static str> {
        let values: Vec<String> = self.choices.iter().map(|c| c.value.trim().to_owned()).filter(|v| !v.is_empty()).collect();
        if self.kind == FieldKind::Terms {
            if values.is_empty() {
                return Err("gs.access.need_rule");
            }
            return Ok(json!({ "field_type": "TERMS", "label": terms_label, "values": values, "required": true }));
        }
        let label = self.label.trim();
        if label.is_empty() {
            return Err("gs.access.need_label");
        }
        let mut f = self.base.as_object().cloned().unwrap_or_default();
        f.insert("field_type".into(), json!(self.kind.tag()));
        f.insert("label".into(), json!(label));
        f.insert("required".into(), json!(true));
        if self.kind == FieldKind::Choice {
            if values.is_empty() {
                return Err("gs.access.need_choices");
            }
            f.insert("choices".into(), json!(self.choices.iter().map(|c| c.value.clone()).collect::<Vec<_>>()));
        }
        Ok(Value::Object(f))
    }
}

#[derive(Default)]
pub struct Pending {
    pub draft: Option<Draft>,
    pub areas: HashMap<String, Entity<TextareaState>>,
    pub inputs: HashMap<String, Entity<InputState>>,
    pub drag: Option<Drag>,
    pub question: Option<Question>,
    pub menu: bool,
    pub error: Option<SharedString>,
    pub focus: Option<String>,
    pub pick: Option<JoinType>,
    pub open_question: bool,
}

fn ctx(s: &State) -> Ctx<'_> {
    static NULL: Value = Value::Null;
    Ctx {
        guild: s.guild_json(),
        form: s.get("verification").unwrap_or(&NULL),
        profile: s.get("profile").unwrap_or(&NULL),
        metadata: s.get("disc_meta").unwrap_or(&NULL),
        splash: s.uploads.get("discovery_splash").and_then(|p| crate::ui::file_pick::data_uri(p)),
    }
}

fn ready(s: &State) -> bool {
    s.data.contains_key("guild") && s.data.contains_key("verification") && !s.guild_json().is_null()
}

fn fresh(s: &State, join: JoinType) -> Draft {
    let x = ctx(s);
    Draft::new(x.guild, x.form, join, &crate::t!("gs.access.q_default"))
}

pub fn ensure(s: &mut State) {
    if s.access.draft.is_some() || !ready(s) {
        return;
    }
    let join = s.access.pick.take().unwrap_or_else(|| join_type(s.guild_json()));
    let d = fresh(s, join);
    s.access.draft = Some(d);
    if std::mem::take(&mut s.access.open_question)
        && let Some(f) = s.access.draft.as_ref().and_then(|d| d.fields.iter().position(|f| FieldKind::of(f) != Some(FieldKind::Terms)).map(|i| (i, d.fields[i].clone())))
    {
        open_question(s, FieldKind::of(&f.1).unwrap_or(FieldKind::Text), Some(f.0), f.1);
    }
}

pub fn preselect(s: &mut State, sub: &str) {
    s.access.pick = JoinType::from_sub(sub);
    s.access.menu = sub.ends_with("+menu");
    s.access.open_question = sub.ends_with("+question");
    s.access.draft = None;
}

pub fn extra_needs(s: &State) -> &'static [&'static str] {
    let join = s.access.draft.as_ref().map(|d| d.join).or(s.access.pick).unwrap_or_else(|| join_type(s.guild_json()));
    if join == JoinType::Discoverable { &["requirements", "disc_meta"] } else { &[] }
}

pub fn is_dirty(s: &State) -> bool {
    s.access.draft.as_ref().is_some_and(|d| dirty(&ctx(s), d).any())
}

pub fn reset(s: &mut State) {
    let pick = s.access.pick;
    s.access = Pending { pick, ..Pending::default() };
    s.uploads.remove("discovery_splash");
}

pub fn submit(s: &mut State) -> Option<Vec<GuardedWrite>> {
    let d = s.access.draft.as_ref()?;
    if let Err(k) = validate(d) {
        s.access.error = Some(crate::t!(k));
        return None;
    }
    s.access.error = None;
    Some(writes(s.guild.0, &ctx(s), d, &crate::t!("gs.access.terms_label")))
}

pub fn view(s: &State) -> Option<View> {
    let req = s.get("requirements")?;
    if req.is_null() {
        return Some(View::Ineligible);
    }
    Some(settings_view(s.guild_json(), req, chrono::Utc::now().timestamp_millis()))
}

pub fn bar(s: &State) -> Option<(crate::ui::modals::settings_layout::BarText, bool)> {
    let d = s.access.draft.as_ref()?;
    let view = (d.join == JoinType::Discoverable).then(|| view(s)).flatten();
    if view == Some(View::Ineligible) {
        return Some((crate::ui::modals::settings_layout::BarText { message: Some(crate::t!("gs.access.d_cannot")), ..Default::default() }, false));
    }
    if !is_dirty(s) && s.access.error.is_none() {
        return None;
    }
    let listing = view == Some(View::EligibleDisabled);
    let x = ctx(s);
    let complete = d.description.clone().or_else(|| x.guild.get("description").and_then(Value::as_str).map(str::to_owned)).is_some_and(|v| !v.is_empty())
        && x.metadata.get("primary_category_id").and_then(Value::as_u64).unwrap_or(0) != 0
        && !d.keywords.clone().unwrap_or_else(|| keywords(x.metadata)).is_empty();
    Some((
        crate::ui::modals::settings_layout::BarText {
            message: listing.then(|| crate::t!("gs.access.d_fill")),
            error: s.access.error.clone(),
            save: listing.then(|| crate::t!("gs.access.d_list")),
            save_disabled: listing && !complete,
        },
        true,
    ))
}

fn open_question(s: &mut State, kind: FieldKind, index: Option<usize>, base: Value) {
    s.access.inputs.retain(|k, _| !k.starts_with("q:"));
    s.access.question = Some(Question::new(kind, index, base));
    s.access.menu = false;
    s.access.focus = Some("q:label".into());
}

fn sync(s: &mut State, key: &str, v: String) {
    let p = &mut s.access;
    p.error = None;
    if let Some(id) = key.strip_prefix("rule:").and_then(|x| x.parse::<u64>().ok()) {
        if let Some(r) = p.draft.as_mut().and_then(|d| d.rules.iter_mut().find(|r| r.id == id)) {
            r.value = v;
        }
    } else if key == "q:label" {
        if let Some(q) = p.question.as_mut() {
            q.label = v;
            q.error = None;
        }
    } else if let Some(id) = key.strip_prefix("q:choice:").and_then(|x| x.parse::<u64>().ok()) {
        if let Some(q) = p.question.as_mut() {
            q.error = None;
            if let Some(c) = q.choices.iter_mut().find(|c| c.id == id) {
                c.value = v;
            }
        }
    } else if key == "d:description"
        && let Some(d) = p.draft.as_mut()
    {
        d.description = Some(v);
    }
}

fn limit_for(key: &str, s: &State) -> (usize, bool) {
    if key.starts_with("rule:") {
        (RULE_LEN, false)
    } else if key == "q:label" {
        (LABEL_LEN, true)
    } else if key.starts_with("q:choice:") {
        (s.access.question.as_ref().map_or(CHOICE_LEN, Question::choice_len), s.access.question.as_ref().is_some_and(|q| q.kind != FieldKind::Terms))
    } else if key == "d:description" {
        (DESCRIPTION_LEN, false)
    } else if key == "d:tag" {
        (TAG_LEN, true)
    } else {
        (usize::MAX, false)
    }
}

fn add_tag(s: &mut State, tag: &str) -> bool {
    let tag = tag.trim();
    if tag.is_empty() {
        return false;
    }
    let meta = keywords(s.get("disc_meta").unwrap_or(&Value::Null));
    let Some(d) = s.access.draft.as_mut() else { return false };
    let k = d.keywords.get_or_insert(meta);
    if k.len() >= MAX_TAGS || k.iter().any(|x| x == tag) {
        return false;
    }
    k.push(tag.to_owned());
    true
}

pub fn prepare(s: &mut State, window: &mut Window, cx: &mut Cx) {
    ensure(s);
    let Some(d) = s.access.draft.as_ref() else { return };
    let mut want_areas: Vec<(String, String, SharedString, usize)> = d
        .rules
        .iter()
        .map(|r| (format!("rule:{}", r.id), r.value.clone(), crate::t!("gs.access.rule_ph"), 1))
        .filter(|(k, ..)| !s.access.areas.contains_key(k))
        .collect();
    if d.join == JoinType::Discoverable && !s.access.areas.contains_key("d:description") {
        let init = d.description.clone().unwrap_or_else(|| kit::s(s.guild_json(), "description"));
        want_areas.push(("d:description".into(), init, crate::t!("gs.access.d_description_ph"), 3));
    }
    let mut want_inputs: Vec<(String, String, SharedString)> = Vec::new();
    if d.join == JoinType::Discoverable && !s.access.inputs.contains_key("d:tag") {
        want_inputs.push(("d:tag".into(), String::new(), crate::t!("gs.access.d_tags_ph")));
    }
    if let Some(q) = s.access.question.as_ref() {
        if q.kind != FieldKind::Terms && !s.access.inputs.contains_key("q:label") {
            want_inputs.push(("q:label".into(), q.label.clone(), crate::t!("gs.access.question_ph")));
        }
        for (i, ch) in q.choices.iter().enumerate() {
            let k = format!("q:choice:{}", ch.id);
            if !s.access.inputs.contains_key(&k) {
                let ph = if q.kind == FieldKind::Terms { crate::t!("gs.access.rule_ph") } else { crate::t!("gs.access.option", n = i + 1) };
                want_inputs.push((k, ch.value.clone(), ph));
            }
        }
    }
    for (key, init, ph, rows) in want_areas {
        let e = cx.new(|cx| TextareaState::new(window, cx).auto_grow(rows, 12).placeholder(ph).default_value(init));
        let k = key.clone();
        let sub = cx.subscribe_in(&e, window, move |_, st, ev: &InputEvent, w, cx| {
            if let InputEvent::Change = ev {
                let v = st.read(cx).value().to_string();
                let (max, _) = with_state(|s| limit_for(&k, s)).unwrap_or((usize::MAX, false));
                let cut = clip(&v, max);
                if cut != v {
                    st.update(cx, |s, cx| s.set_value(cut.clone(), w, cx));
                }
                with_state(|s| sync(s, &k, cut));
                cx.notify();
            }
        });
        s.subs.push(sub);
        s.access.areas.insert(key, e);
    }
    for (key, init, ph) in want_inputs {
        let e = cx.new(|cx| InputState::new(window, cx).placeholder(ph));
        if !init.is_empty() {
            e.update(cx, |st, cx| st.set_value(init, window, cx));
        }
        let k = key.clone();
        let sub = cx.subscribe_in(&e, window, move |_, st, ev: &InputEvent, w, cx| match ev {
            InputEvent::Change => {
                let v = st.read(cx).value().to_string();
                let (max, flat) = with_state(|s| limit_for(&k, s)).unwrap_or((usize::MAX, false));
                let cut = if flat { one_line(&v, max) } else { clip(&v, max) };
                if cut != v {
                    st.update(cx, |s, cx| s.set_value(cut.clone(), w, cx));
                }
                with_state(|s| sync(s, &k, cut));
                cx.notify();
            }
            InputEvent::PressEnter { .. } if k == "d:tag" => {
                let v = st.read(cx).value().to_string();
                if with_state(|s| add_tag(s, &v)).unwrap_or(false) {
                    st.update(cx, |s, cx| s.set_value("", w, cx));
                }
                cx.notify();
            }
            _ => {}
        });
        s.subs.push(sub);
        s.access.inputs.insert(key, e);
    }
    if let Some(f) = s.access.focus.take() {
        if let Some(e) = s.access.areas.get(&f) {
            e.update(cx, |st, cx| st.focus(window, cx));
        } else if let Some(e) = s.access.inputs.get(&f) {
            e.update(cx, |st, cx| st.focus(window, cx));
        }
    }
}

fn with_link(body: SharedString, link: SharedString) -> StyledText {
    let start = body.len();
    let full: SharedString = format!("{body}{link}").into();
    let end = full.len();
    StyledText::new(full).with_highlights([(start..end, HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })])
}

fn with_marked(body: SharedString, word: SharedString, color: usize, weight: FontWeight) -> StyledText {
    let range = body.find(&*word).map(|i| i..i + word.len());
    StyledText::new(body).with_highlights(range.map(|r| (r, HighlightStyle { color: Some(c(tok(color))), font_weight: Some(weight), ..Default::default() })))
}

fn edit(cx: &mut Cx, f: impl Fn(&mut State) + 'static) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    cx.listener(move |_, _, _, cx| {
        with_state(&f);
        cx.notify();
    })
}

fn card(guild: &Value, kind: JoinType, active: bool, enabled: bool, cx: &mut Cx) -> AnyElement {
    let (icon, title, body) = match kind {
        JoinType::Invite => ("lock", crate::t!("gs.access.invite"), crate::t!("gs.access.invite_desc")),
        JoinType::Apply => ("server/envelope", crate::t!("gs.access.apply"), crate::t!("gs.access.apply_desc")),
        JoinType::Discoverable => ("settings/globe", crate::t!("gs.access.discoverable"), crate::t!("gs.access.discoverable_desc")),
    };
    let fg = if active { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT };
    let tip = (!enabled).then(|| ineligible_text(kind, guild));
    div()
        .id(("gs-access-type", kind as usize))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(8.))
        .py(z(12.))
        .px(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(if active { c(tok(idx::BORDER_NORMAL)) } else { gpui::transparent_black() })
        .text_center()
        .when(active, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .when(!active && enabled, |d| d.cursor_pointer().hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
        .when(!enabled, |d| d.opacity(0.5).cursor_not_allowed().hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_MUTED)))))
        .when_some(tip, |d, t| d.tooltip(crate::ui::tooltip::tip(t)))
        .when(enabled && !active, |d| {
            d.on_click(cx.listener(move |a, _, _, cx| {
                with_state(|s| {
                    let next = fresh(s, kind);
                    s.access.draft = Some(next);
                    s.access.areas.clear();
                    s.access.inputs.clear();
                    s.access.question = None;
                    s.access.menu = false;
                    s.access.error = None;
                });
                super::load(a, cx);
                cx.notify();
            }))
        })
        .child(gpui::svg().path(crate::icons::path(icon)).flex_none().size(z(24.)).text_color(c(tok(fg))))
        .child(text(title, 16., 20., FontWeight::SEMIBOLD, fg))
        .child(text(body, 12., 16., FontWeight::MEDIUM, fg))
        .into_any_element()
}

fn switch_row(id: &'static str, label: SharedString, desc: impl IntoElement, on: bool, enabled: bool, cx: &mut Cx, flip: impl Fn(&mut State) + 'static) -> AnyElement {
    div()
        .id(id)
        .flex()
        .gap(z(24.))
        .group(crate::ui::controls::SWITCH_ROW)
        .when(!enabled, |d| d.opacity(0.5).cursor_not_allowed())
        .when(enabled, |d| d.cursor_pointer().on_click(edit(cx, flip)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pt(z(2.))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(labels(label, None))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(desc)),
        )
        .child(crate::ui::controls::switch((id, 1usize), on))
        .into_any_element()
}

fn divider() -> AnyElement {
    div().flex_none().h(z(1.)).w_full().bg(c(tok(idx::BORDER_SUBTLE))).into_any_element()
}

fn small_title(t: SharedString) -> gpui::Div {
    text(t, 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED)
}

fn preview_link() -> AnyElement {
    div()
        .id("gs-access-preview")
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .text_color(c(tok(idx::TEXT_BRAND)))
        .hover(|d| d.underline())
        .child(text(crate::t!("gs.access.preview"), 14., 18., FontWeight::SEMIBOLD, idx::TEXT_BRAND))
        .child(gpui::svg().path(crate::icons::path("viewer/open")).size(z(12.)).text_color(c(tok(idx::TEXT_BRAND))))
        .into_any_element()
}

fn add_button(id: &'static str, label: SharedString, tooltip: Option<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(z(4.))
        .py(z(12.))
        .px(z(24.))
        .rounded(z(8.))
        .border_2()
        .border_dashed()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .hover(|d| d.border_color(c(tok(idx::BORDER_STRONG))))
        .cursor_pointer()
        .when_some(tooltip, |d, t| d.tooltip(crate::ui::tooltip::tip(t)))
        .child(gpui::svg().path(crate::icons::path("access/plus-small")).size(z(24.)).text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))
        .child(text(label, 16., 20., FontWeight::SEMIBOLD, idx::INTERACTIVE_TEXT_ACTIVE))
}

fn pills(id: &'static str, items: Vec<(SharedString, bool, Option<SharedString>)>, lowest: bool, cx: &mut Cx, on: impl Fn(&mut State, usize) + Clone + 'static) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .gap(z(8.))
        .children(items.into_iter().enumerate().map(|(i, (label, disabled, tip))| {
            let on = on.clone();
            div()
                .id((id, i))
                .flex_none()
                .py(z(8.))
                .px(z(12.))
                .rounded(z(global::RADIUS_ROUND))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(if lowest { idx::BACKGROUND_BASE_LOWEST } else { idx::BACKGROUND_BASE_LOWER })))
                .when(!disabled, |d| d.cursor_pointer().hover(|d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))).on_click(edit(cx, move |s| on(s, i))))
                .when(disabled, |d| d.opacity(0.8).cursor_not_allowed())
                .when_some(tip, |d, t| d.tooltip(crate::ui::tooltip::tip(t)))
                .child(text(label, 14., 18., FontWeight::MEDIUM, idx::INTERACTIVE_TEXT_DEFAULT))
        }))
        .into_any_element()
}

fn drag_row(list: DragList, i: usize, cx: &mut Cx) -> impl Fn(gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    let move_l = cx.listener(move |_, _: &gpui::MouseMoveEvent, _, cx| {
        let changed = with_state(|s| match s.access.drag.as_mut() {
            Some(d) if d.list == list && d.over != i => {
                d.over = i;
                true
            }
            _ => false,
        })
        .unwrap_or(false);
        if changed {
            cx.notify();
        }
    });
    let up_l = std::rc::Rc::new(cx.listener(move |_, _: &gpui::MouseUpEvent, _, cx| {
        with_state(|s| {
            let Some(d) = s.access.drag.take() else { return };
            if d.list != list {
                return;
            }
            let p = &mut s.access;
            match list {
                DragList::Rules => {
                    if let Some(dr) = p.draft.as_mut() {
                        move_item(&mut dr.rules, d.from, d.over);
                    }
                }
                DragList::Fields => {
                    if let Some(dr) = p.draft.as_mut() {
                        move_item(&mut dr.fields, d.from, d.over);
                    }
                }
                DragList::Choices => {
                    if let Some(q) = p.question.as_mut() {
                        move_item(&mut q.choices, d.from, d.over);
                    }
                }
            }
        });
        cx.notify();
    }));
    let move_l = std::rc::Rc::new(move_l);
    move |d: gpui::Stateful<gpui::Div>| {
        let m = move_l.clone();
        let u = up_l.clone();
        d.on_mouse_move(move |e, w, cx| m(e, w, cx)).on_mouse_up(MouseButton::Left, move |e, w, cx| u(e, w, cx))
    }
}

fn drag_handle(id: (&'static str, usize), list: DragList, i: usize, cx: &mut Cx) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .cursor_grab()
        .child(gpui::svg().path(crate::icons::path("server/drag")).size(z(12.)).text_color(c(tok(idx::ICON_MUTED))))
        .on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, cx| {
            with_state(|s| s.access.drag = Some(Drag { list, from: i, over: i }));
            cx.notify();
        }))
}

fn rules_editor(s: &State, disabled: bool, cx: &mut Cx) -> AnyElement {
    let Some(d) = s.access.draft.as_ref() else { return div().into_any_element() };
    let drag = s.access.drag.filter(|g| g.list == DragList::Rules);
    let can_drag = !disabled && d.rules.len() > 1;
    let full = d.rules.len() >= MAX_RULES;
    let mut list = div().my(z(12.)).flex().flex_col().gap(z(12.));
    for (i, r) in d.rules.iter().enumerate() {
        let key = format!("rule:{}", r.id);
        let group: SharedString = format!("gs-access-rule-{}", r.id).into();
        let body = match s.access.areas.get(&key) {
            Some(e) => div().child(Textarea::new(e).appearance(false).bordered(false).disabled(disabled)).into_any_element(),
            None => text(r.value.clone(), 16., 22., FontWeight::NORMAL, idx::TEXT_DEFAULT).into_any_element(),
        };
        let row = div()
            .id(("gs-access-rule", r.id as usize))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .when(drag.is_some_and(|g| g.over == i), |d| d.opacity(0.6))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .relative()
                    .rounded(z(global::RADIUS_SM))
                    .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                    .pl(z(12.))
                    .pr(z(40.))
                    .py(z(11.))
                    .text_size(z(16.))
                    .line_height(z(22.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(body)
                    .child(
                        div()
                            .id(("gs-access-rule-clear", r.id as usize))
                            .absolute()
                            .top(z(14.))
                            .right(z(8.))
                            .invisible()
                            .group_hover(group.clone(), |d| d.visible())
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip::tip(crate::t!("gs.access.clear")))
                            .child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                            .when(!disabled, |d| {
                                d.on_click(edit(cx, move |s| {
                                    if let Some(dr) = s.access.draft.as_mut() {
                                        dr.remove_rule(i);
                                    }
                                }))
                            }),
                    ),
            )
            .when(can_drag, |d| d.child(drag_handle(("gs-access-rule-drag", r.id as usize), DragList::Rules, i, cx).absolute().left(z(4.)).top(z(20.))));
        list = list.child(drag_row(DragList::Rules, i, cx)(row));
    }
    let tip = full.then(|| crate::t!("gs.access.rules_max", n = MAX_RULES));
    let examples: Vec<(SharedString, bool, Option<SharedString>)> = ["ex1", "ex2", "ex3", "ex4"].iter().map(|k| (crate::t!(&format!("gs.access.{k}")), full || disabled, tip.clone())).collect();
    div()
        .flex()
        .flex_col()
        .child(small_title(crate::t!("gs.access.rules")))
        .child(list)
        .when(!full, |d| {
            d.child(add_button("gs-access-add-rule", crate::t!("gs.access.add_rule"), None).when(!disabled, |d| {
                d.on_click(edit(cx, |s| {
                    if let Some(id) = s.access.draft.as_mut().and_then(|d| d.add_rule(None)) {
                        s.access.focus = Some(format!("rule:{id}"));
                    }
                }))
            }))
        })
        .child(small_title(crate::t!("gs.access.example_rules")).mt(z(24.)).mb(z(8.)))
        .child(pills("gs-access-ex-rule", examples, true, cx, |s, i| {
            let body = crate::t!(&format!("gs.access.ex{}_body", i + 1));
            if let Some(id) = s.access.draft.as_mut().and_then(|d| d.add_rule(Some(&body))) {
                let k = format!("rule:{id}");
                s.access.areas.remove(&k);
                s.access.focus = Some(k);
            }
        }))
        .into_any_element()
}

fn rules_block(s: &State, cx: &mut Cx) -> AnyElement {
    let Some(d) = s.access.draft.as_ref() else {
        return text(crate::t!("common.loading"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).into_any_element();
    };
    let on = d.terms;
    div()
        .flex()
        .flex_col()
        .child(switch_row("gs-access-rules", crate::t!("gs.access.rules_title"), crate::t!("gs.access.rules_desc"), on, true, cx, |s| {
            if let Some(d) = s.access.draft.as_mut() {
                d.terms = !d.terms;
            }
            s.access.error = None;
        }))
        .when(on, |d| d.child(div().mt(z(8.)).child(preview_link())))
        .child(div().mt(z(24.)).p(z(16.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).when(!on, |d| d.opacity(0.5).cursor_not_allowed()).child(rules_editor(s, !on, cx)))
        .into_any_element()
}

fn short_answer_icon() -> AnyElement {
    let bar = |w: f32, top: f32| div().absolute().left_0().top(z(top)).w(z(w)).h(z(2.)).rounded(z(1.)).bg(c(tok(idx::TEXT_MUTED)));
    div().relative().flex_none().w(z(12.75)).h(z(12.)).child(bar(12., 3.75)).child(bar(6., 6.75)).into_any_element()
}

fn field_row(s: &State, i: usize, f: &Value, count: usize, cx: &mut Cx) -> AnyElement {
    let kind = FieldKind::of(f).unwrap_or(FieldKind::Text);
    let title: SharedString = if kind == FieldKind::Terms { crate::t!("gs.access.terms_title") } else { f.get("label").and_then(Value::as_str).unwrap_or_default().to_owned().into() };
    let group: SharedString = format!("gs-access-field-{i}").into();
    let hovered = s.access.drag.is_some_and(|g| g.list == DragList::Fields && g.over == i);
    let icon = match kind.icon() {
        Some(n) => gpui::svg().path(crate::icons::path(n)).size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).into_any_element(),
        None => short_answer_icon(),
    };
    let base = f.clone();
    let action = |id: (&'static str, usize), icon: &'static str, tip: SharedString| {
        div()
            .id(id)
            .size(z(32.))
            .flex_none()
            .rounded(z(16.))
            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tip(tip))
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(12.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
    };
    let row = div()
        .id(("gs-access-field", i))
        .group(group.clone())
        .relative()
        .flex()
        .items_center()
        .gap(z(8.))
        .when(hovered, |d| d.opacity(0.5))
        .child(
            div()
                .w(z(25.))
                .flex_none()
                .flex()
                .justify_center()
                .invisible()
                .when(count > 1, |d| d.group_hover(group.clone(), |d| d.visible()).child(drag_handle(("gs-access-field-drag", i), DragList::Fields, i, cx))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .p(z(16.))
                .flex()
                .items_center()
                .justify_between()
                .gap(z(16.))
                .child(text(title, 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).flex_1().min_w_0())
                .child(div().flex_none().flex().items_center().gap(z(4.)).text_color(c(tok(idx::TEXT_MUTED))).child(icon).child(text(kind.label(), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED))),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap(z(8.))
                .invisible()
                .group_hover(group.clone(), |d| d.visible())
                .child(action(("gs-access-field-edit", i), "pen", crate::t!("gs.access.edit")).on_click(edit(cx, move |s| open_question(s, kind, Some(i), base.clone()))))
                .when(count > 1, |d| {
                    d.child(action(("gs-access-field-rm", i), "chat/trash", crate::t!("gs.access.remove")).on_click(edit(cx, move |s| {
                        if let Some(dr) = s.access.draft.as_mut() {
                            dr.remove_field(i);
                        }
                    })))
                }),
        );
    drag_row(DragList::Fields, i, cx)(row).into_any_element()
}

fn add_question_menu(allow_terms: bool, cx: &mut Cx) -> AnyElement {
    let mut items = vec![(FieldKind::Text, "text-input"), (FieldKind::Paragraph, "paragraph"), (FieldKind::Choice, "multiple-choice")];
    if allow_terms {
        items.push((FieldKind::Terms, "terms"));
    }
    div()
        .id("gs-access-add-menu")
        .absolute()
        .bottom(z(56.))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .w(z(220.))
                .p(z(8.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
                .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.24), offset: gpui::point(z(0.), z(8.)), blur_radius: z(16.), spread_radius: z(0.), inset: false }])
                .occlude()
                .flex()
                .flex_col()
                .children(items.into_iter().map(|(kind, id)| {
                    let icon = match kind.icon() {
                        Some(n) => gpui::svg().path(crate::icons::path(n)).size(z(18.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element(),
                        None => short_answer_icon(),
                    };
                    div()
                        .id(id)
                        .h(z(32.))
                        .px(z(8.))
                        .rounded(z(4.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                        .child(div().w(z(18.)).flex().justify_center().child(icon))
                        .child(text(kind.label(), 14., 18., FontWeight::MEDIUM, idx::INTERACTIVE_TEXT_DEFAULT))
                        .on_click(edit(cx, move |s| open_question(s, kind, None, Value::Null)))
                })),
        )
        .into_any_element()
}

fn profile_preview(app: &PhoenixApp, s: &State) -> AnyElement {
    let name = app.guilds.iter().find(|g| g.id == s.guild).map(|g| g.name.clone()).unwrap_or_default();
    div()
        .relative()
        .h(z(100.))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .p(z(16.))
                .rounded_tl(z(8.))
                .rounded_tr(z(8.))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .flex()
                .flex_col()
                .gap(z(12.))
                .child(text(name, 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).truncate())
                .child(crate::ui::controls::button("gs-access-apply-preview", crate::t!("gs.access.apply_btn"), crate::ui::controls::BtnKind::Primary).h(z(32.)).w_full()),
        )
        .child(div().absolute().top_0().left_0().right_0().bottom_0().bg(gpui::linear_gradient(
            180.,
            gpui::linear_color_stop(c(tok(idx::BACKGROUND_BASE_LOW)), 0.),
            gpui::linear_color_stop(c(tok(idx::BACKGROUND_BASE_LOW)).opacity(0.), 0.7),
        )))
        .into_any_element()
}

fn apply_block(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let Some(d) = s.access.draft.as_ref() else { return div().into_any_element() };
    let full = d.fields.len() >= MAX_FIELDS;
    let profile_vis = s.get("profile").and_then(|p| p.get("visibility")).and_then(Value::as_u64).map(|v| v as u8);
    let vis = d.visibility.or(profile_vis);
    let visible = matches!(vis, Some(PUBLIC | PUBLIC_WITH_RECRUITMENT));
    let recruit = vis == Some(PUBLIC_WITH_RECRUITMENT);
    let mut fields = div().flex().flex_col().gap(z(8.));
    for (i, f) in d.fields.iter().enumerate() {
        fields = fields.child(field_row(s, i, f, d.fields.len(), cx));
    }
    let examples: Vec<(SharedString, bool, Option<SharedString>)> = ["q_games", "q_found", "q_unique"].iter().map(|k| (crate::t!(&format!("gs.access.{k}")), false, None)).collect();
    let allow_terms = !d.has_terms_field();
    let menu = s.access.menu;
    let form = div()
        .my(z(24.))
        .flex()
        .flex_col()
        .child(small_title(crate::t!("gs.access.questions", n = d.fields.len(), max = MAX_FIELDS)).mb(z(8.)))
        .child(fields)
        .when(!full, |el| {
            el.child(
                div()
                    .relative()
                    .mt(z(8.))
                    .child(add_button("gs-access-add-question", crate::t!("gs.access.add_question"), Some(crate::t!("gs.access.questions_max"))).on_click(edit(cx, |s| s.access.menu = !s.access.menu)))
                    .when(menu, |el| el.child(gpui::deferred(add_question_menu(allow_terms, cx)).with_priority(30))),
            )
            .child(small_title(crate::t!("gs.access.example_questions")).mt(z(24.)).mb(z(8.)))
            .child(pills("gs-access-ex-q", examples, false, cx, |s, i| {
                let label = crate::t!(["gs.access.q_games", "gs.access.q_found", "gs.access.q_unique"][i]);
                if let Some(d) = s.access.draft.as_mut() {
                    d.add_field(json!({ "field_type": "TEXT_INPUT", "label": label.to_string(), "required": true }));
                }
            }))
        });
    div()
        .flex()
        .flex_col()
        .child(text(crate::t!("gs.access.application"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG))
        .child(text(crate::t!("gs.access.application_desc"), 14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE).my(z(8.)))
        .child(preview_link())
        .child(form)
        .child(div().my(z(32.)).h(z(1.)).w_full().bg(c(tok(idx::BORDER_STRONG))))
        .child(
            div()
                .flex()
                .gap(z(32.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .child(switch_row("gs-access-from-profile", crate::t!("gs.access.from_profile"), div(), recruit, visible, cx, move |s| {
                            if let Some(d) = s.access.draft.as_mut() {
                                d.visibility = Some(if recruit { PUBLIC } else { PUBLIC_WITH_RECRUITMENT });
                            }
                        }))
                        .child(text(crate::t!("gs.access.from_profile_desc"), 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE))
                        .when(!visible, |d| d.child(text(crate::t!("gs.access.from_profile_off"), 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED))),
                )
                .child(div().flex_1().min_w_0().child(profile_preview(app, s))),
        )
        .into_any_element()
}

fn section_header(title: SharedString, desc: SharedString) -> AnyElement {
    div().flex().flex_col().child(text(title, 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).mb(z(8.))).child(text(desc, 14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE)).into_any_element()
}

fn sub_header(title: SharedString, required: bool, desc: SharedString) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .mb(z(4.))
                .child(small_title(title))
                .when(required, |d| d.child(text("*", 12., 16., FontWeight::SEMIBOLD, idx::TEXT_FEEDBACK_CRITICAL).ml(z(4.)))),
        )
        .child(text(desc, 14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE))
        .into_any_element()
}

fn discovery_card(app: &PhoenixApp, s: &State) -> AnyElement {
    let g = app.guilds.iter().find(|g| g.id == s.guild);
    let name = g.map(|g| g.name.clone()).unwrap_or_default();
    let desc = s.access.draft.as_ref().and_then(|d| d.description.clone()).unwrap_or_else(|| kit::s(s.guild_json(), "description"));
    div()
        .flex_none()
        .w(z(254.))
        .min_h(z(266.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .flex()
        .flex_col()
        .child(div().h(z(110.)).w_full().bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .child(
            div()
                .p(z(16.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(text(name, 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG).truncate())
                .child(text(desc, 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE).line_clamp(3)),
        )
        .into_any_element()
}

fn discovery_form(app: &PhoenixApp, s: &State, view: View, cx: &mut Cx) -> AnyElement {
    let required = view == View::EligibleDisabled;
    let d = s.access.draft.as_ref();
    let meta = s.get("disc_meta").cloned().unwrap_or(Value::Null);
    let tags = d.and_then(|d| d.keywords.clone()).unwrap_or_else(|| keywords(&meta));
    let emoji = d.and_then(|d| d.emoji_info).unwrap_or_else(|| meta.get("emoji_discoverability_enabled").and_then(Value::as_bool).unwrap_or(false));
    let locale = d.and_then(|d| d.locale.clone()).unwrap_or_else(|| kit::s(s.guild_json(), "preferred_locale"));
    let desc = match s.access.areas.get("d:description") {
        Some(e) => div()
            .min_h(z(88.))
            .px(z(10.))
            .py(z(10.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
            .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
            .text_size(z(16.))
            .line_height(z(20.))
            .child(Textarea::new(e).appearance(false).bordered(false))
            .into_any_element(),
        None => div().h(z(88.)).into_any_element(),
    };
    let profile = div()
        .flex()
        .gap(z(48.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(32.))
                .child(section_header(crate::t!("gs.access.d_profile"), crate::t!("gs.access.d_profile_desc")))
                .child(
                    div().flex().flex_col().child(sub_header(crate::t!("gs.access.d_banner"), false, crate::t!("gs.access.d_banner_desc"))).child(
                        div().mt(z(16.)).flex().gap(z(8.)).child(
                            kit::btn_sm("gs-access-upload", crate::t!("gs.access.d_upload"), crate::ui::controls::BtnKind::Secondary)
                                .on_click(cx.listener(|_, _, _, cx| super::pick_upload("discovery_splash", &["png", "jpg", "jpeg", "gif", "webp"], cx))),
                        ),
                    ),
                )
                .child(div().flex().flex_col().child(sub_header(crate::t!("gs.access.d_description"), required, crate::t!("gs.access.d_description_desc"))).child(div().mt(z(16.)).child(desc))),
        )
        .child(discovery_card(app, s));
    let category = div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(section_header(crate::t!("gs.access.d_category_title"), crate::t!("gs.access.d_category_title_desc")))
        .child(div().flex().flex_col().child(sub_header(crate::t!("gs.access.d_category"), required, crate::t!("gs.access.d_category_desc"))).child(div().mt(z(16.)).child(kit::select_box("gs-access-cat", div().into_any_element(), false))))
        .child(div().flex().flex_col().child(sub_header(crate::t!("gs.access.d_subcategories"), false, crate::t!("gs.access.d_subcategories_desc"))).child(div().mt(z(16.)).child(kit::select_box("gs-access-subcat", div().into_any_element(), false))));
    let tag_input = s.access.inputs.get("d:tag").cloned();
    let chips = div().flex().flex_wrap().gap(z(4.)).children(tags.iter().enumerate().map(|(i, t)| {
        div()
            .id(("gs-access-tag", i))
            .h(z(24.))
            .px(z(8.))
            .rounded(z(4.))
            .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
            .flex()
            .items_center()
            .gap(z(4.))
            .cursor_pointer()
            .child(text(t.clone(), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
            .child(gpui::svg().path(crate::icons::path("close-small")).size(z(12.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
            .on_click(edit(cx, move |s| {
                let meta = keywords(s.get("disc_meta").unwrap_or(&Value::Null));
                if let Some(d) = s.access.draft.as_mut() {
                    let k = d.keywords.get_or_insert(meta);
                    if i < k.len() {
                        k.remove(i);
                    }
                }
            }))
    }));
    let tags_box = div()
        .min_h(z(40.))
        .px(z(8.))
        .py(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(z(4.))
        .child(chips)
        .when(tags.len() < MAX_TAGS, |d| d.children(tag_input.map(|i| div().flex_1().min_w(z(120.)).child(Input::new(&i).appearance(false).bordered(false).text_size(z(16.))))));
    let tags_section = div()
        .flex()
        .flex_col()
        .child(sub_header(crate::t!("gs.access.d_tags"), required, crate::t!("gs.access.d_tags_desc")))
        .child(div().mt(z(16.)).child(tags_box))
        .when(tags.len() >= MAX_TAGS, |d| d.child(text(crate::t!("gs.access.d_tags_max"), 14., 18., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.))));
    let emoji_section = div().flex().flex_col().child(section_header(crate::t!("gs.access.d_emoji"), crate::t!("gs.access.d_emoji_desc"))).child(
        div().mt(z(16.)).flex().child(
            kit::btn_sm(
                "gs-access-emoji",
                if emoji { crate::t!("gs.access.d_emoji_off") } else { crate::t!("gs.access.d_emoji_on") },
                if emoji { crate::ui::controls::BtnKind::Secondary } else { crate::ui::controls::BtnKind::Primary },
            )
            .on_click(edit(cx, move |s| {
                if let Some(d) = s.access.draft.as_mut() {
                    d.emoji_info = Some(!emoji);
                }
            })),
        ),
    );
    let langs = crate::i18n::languages::LANGUAGES;
    let cur = langs.iter().position(|l| l.code == locale);
    let opts: Vec<SharedString> = langs.iter().map(|l| SharedString::from(l.native)).collect();
    let value = text(cur.map(|i| SharedString::from(langs[i].native)).unwrap_or_default(), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).into_any_element();
    let locale_row = div()
        .flex()
        .gap(z(24.))
        .child(labels(crate::t!("gs.access.d_locale"), Some(crate::t!("gs.access.d_locale_desc"))).flex_1())
        .child(
            div().flex_1().child(kit::dropdown(
                s,
                "gs-access-locale",
                value,
                opts,
                cur,
                std::rc::Rc::new(move |_: &mut PhoenixApp, i: usize, _: &mut Cx| {
                    with_state(|s| {
                        if let (Some(d), Some(l)) = (s.access.draft.as_mut(), langs.get(i)) {
                            d.locale = Some(l.code.to_owned());
                        }
                    });
                }),
                cx,
            )),
        );
    div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(profile)
        .child(divider())
        .child(category)
        .child(divider())
        .child(tags_section)
        .child(divider())
        .child(emoji_section)
        .child(divider())
        .child(locale_row)
        .child(divider())
        .child(rules_block(s, cx))
        .into_any_element()
}

fn discoverable_block(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    match view(s) {
        None => text(crate::t!("common.loading"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).into_any_element(),
        Some(View::Disqualified) => div()
            .flex()
            .flex_col()
            .child(div().mb(z(4.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(with_marked(
                crate::t!("gs.access.d_removed"),
                crate::t!("gs.access.d_removed_word"),
                idx::TEXT_FEEDBACK_CRITICAL,
                FontWeight::BOLD,
            )))
            .child(text(crate::t!("gs.access.d_removed_desc"), 14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE))
            .into_any_element(),
        Some(View::Ineligible) => div()
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(tok(idx::TEXT_STRONG)))
            .child(with_marked(crate::t!("gs.access.d_ineligible"), crate::t!("gs.access.d_ineligible_word"), idx::TEXT_FEEDBACK_CRITICAL, FontWeight::BOLD))
            .into_any_element(),
        Some(v) => discovery_form(app, s, v, cx),
    }
}

pub fn modal(s: &State, cx: &mut Cx) -> Option<AnyElement> {
    let q = s.access.question.as_ref()?;
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        with_state(|s| {
            s.access.question = None;
            s.access.inputs.retain(|k, _| !k.starts_with("q:"));
        });
        e.update(cx, |_, cx| cx.notify());
    });
    let mut body = div().flex().flex_col();
    if q.kind != FieldKind::Terms {
        body = body.child(match s.access.inputs.get("q:label") {
            Some(i) => crate::ui::modals::input_box(i).into_any_element(),
            None => div().h(z(40.)).into_any_element(),
        });
    }
    if matches!(q.kind, FieldKind::Choice | FieldKind::Terms) {
        if q.kind == FieldKind::Choice {
            body = body.child(div().my(z(16.)).h(z(1.)).w_full().bg(c(tok(idx::BORDER_SUBTLE))));
        }
        let drag = s.access.drag.filter(|g| g.list == DragList::Choices);
        let mut list = div().flex().flex_col().gap(z(8.));
        for (i, ch) in q.choices.iter().enumerate() {
            let k = format!("q:choice:{}", ch.id);
            let group: SharedString = format!("gs-access-choice-{}", ch.id).into();
            let input = match s.access.inputs.get(&k) {
                Some(inp) => crate::ui::modals::input_box(inp).pr(z(32.)).into_any_element(),
                None => div().h(z(40.)).into_any_element(),
            };
            let row = div()
                .id(("gs-access-choice", ch.id as usize))
                .group(group.clone())
                .flex()
                .items_center()
                .gap(z(8.))
                .when(drag.is_some_and(|g| g.over == i), |d| d.opacity(0.6))
                .when(q.kind == FieldKind::Choice, |d| d.child(div().size(z(20.)).flex_none().rounded_full().border_2().border_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))))
                .child(
                    div().flex_1().min_w_0().relative().child(input).child(
                        div()
                            .id(("gs-access-choice-clear", ch.id as usize))
                            .absolute()
                            .top(z(12.))
                            .right(z(8.))
                            .invisible()
                            .group_hover(group.clone(), |d| d.visible())
                            .cursor_pointer()
                            .child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                            .on_click(edit(cx, move |s| {
                                if let Some(q) = s.access.question.as_mut() {
                                    q.remove_choice(i);
                                }
                            })),
                    ),
                )
                .child(drag_handle(("gs-access-choice-drag", ch.id as usize), DragList::Choices, i, cx));
            list = list.child(drag_row(DragList::Choices, i, cx)(row));
        }
        body = body.child(list);
        if q.choices.len() < q.max_choices() {
            let label = if q.kind == FieldKind::Terms { crate::t!("gs.access.add_rule") } else { crate::t!("gs.access.add_option") };
            body = body.child(
                div()
                    .id("gs-access-add-choice")
                    .mt(z(12.))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .cursor_pointer()
                    .child(gpui::svg().path(crate::icons::path("plus-circle")).size(z(17.)).text_color(c(tok(idx::TEXT_LINK))))
                    .child(text(label, 16., 20., FontWeight::NORMAL, idx::TEXT_LINK))
                    .on_click(edit(cx, |s| {
                        if let Some(id) = s.access.question.as_mut().and_then(Question::add_choice) {
                            s.access.focus = Some(format!("q:choice:{id}"));
                        }
                    })),
            );
        }
    }
    if let Some(err) = q.error.clone() {
        body = body.child(text(err, 14., 18., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL).mt(z(12.)));
    }
    let close2 = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .justify_end()
        .gap(z(8.))
        .child(crate::ui::controls::button("gs-access-q-cancel", crate::t!("common.cancel"), crate::ui::controls::BtnKind::Secondary).on_click(move |_, w, cx| close2(w, cx)))
        .child(crate::ui::controls::button("gs-access-q-save", crate::t!("common.save"), crate::ui::controls::BtnKind::Primary).on_click(edit(cx, |s| {
            let label = crate::t!("gs.access.terms_label");
            let Some(q) = s.access.question.as_mut() else { return };
            match q.build(&label) {
                Ok(f) => {
                    let index = q.index;
                    s.access.question = None;
                    s.access.inputs.retain(|k, _| !k.starts_with("q:"));
                    if let Some(d) = s.access.draft.as_mut() {
                        match index {
                            Some(i) if i < d.fields.len() => d.fields[i] = f,
                            _ => {
                                d.add_field(f);
                            }
                        }
                    }
                }
                Err(k) => q.error = Some(crate::t!(k)),
            }
        })));
    let title = if q.kind == FieldKind::Terms { crate::t!("gs.access.terms_title") } else { q.kind.label() };
    Some(crate::ui::modals::card_with(440., title, None, body.into_any_element(), Some(footer.into_any_element()), close))
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let guild = s.guild_json();
    let head = text(crate::t!("menu.gs.access"), 20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG);
    let Some(d) = s.access.draft.as_ref() else {
        return div().flex().flex_col().gap(z(32.)).child(head).into_any_element();
    };
    let admin = app.backend.guild_permissions(s.guild) & crate::actions::ADMINISTRATOR != 0;
    let active = d.join;
    let types = div()
        .flex()
        .flex_col()
        .child(text(crate::t!("gs.access.how"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG))
        .child(
            div()
                .mt(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(with_link(crate::t!("gs.access.how_desc"), crate::t!("gs.access.learn_more"))),
        )
        .child(
            div()
                .mt(z(global::SPACE_16))
                .p(z(global::SPACE_8))
                .flex()
                .gap(z(global::SPACE_8))
                .rounded(z(global::RADIUS_MD))
                .border_1()
                .border_color(c(tok(idx::BORDER_MUTED)))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .children(options(guild, join_type(guild), admin).into_iter().map(|(k, enabled)| card(guild, k, k == active, enabled, cx))),
        );
    let age = switch_row(
        "gs-access-age",
        crate::t!("gs.access.age"),
        with_link(crate::t!("gs.access.age_desc"), crate::t!("gs.access.age_more")),
        d.age,
        !age_locked(guild, active),
        cx,
        |s| {
            if let Some(d) = s.access.draft.as_mut() {
                d.age = !d.age;
            }
        },
    );
    let below = match active {
        JoinType::Invite => rules_block(s, cx),
        JoinType::Apply => apply_block(app, s, cx),
        JoinType::Discoverable => discoverable_block(app, s, cx),
    };
    div()
        .id("gs-access")
        .flex()
        .flex_col()
        .gap(z(32.))
        .on_mouse_up(MouseButton::Left, cx.listener(|_, _, _, cx| {
            if with_state(|s| s.access.drag.take().is_some()).unwrap_or(false) {
                cx.notify();
            }
        }))
        .child(head)
        .child(types)
        .child(divider())
        .child(age)
        .child(divider())
        .child(below)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn x<'a>(guild: &'a Value, form: &'a Value) -> Ctx<'a> {
        static NULL: Value = Value::Null;
        Ctx { guild, form, profile: &NULL, metadata: &NULL, splash: None }
    }

    fn terms_form(values: &[&str]) -> Value {
        json!({ "form_fields": [{ "field_type": "TERMS", "label": "Read", "values": values, "required": true }] })
    }

    #[test]
    fn join_type_follows_the_guild_features() {
        assert_eq!(join_type(&json!({ "features": [] })), JoinType::Invite);
        assert_eq!(join_type(&json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED"] })), JoinType::Invite);
        assert_eq!(join_type(&json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED", "MEMBER_VERIFICATION_MANUAL_APPROVAL"] })), JoinType::Apply);
        assert_eq!(join_type(&json!({ "features": ["DISCOVERABLE", "MEMBER_VERIFICATION_MANUAL_APPROVAL"] })), JoinType::Discoverable);
        assert!(rules_required(&json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED"] })));
        assert_eq!(JoinType::from_sub("apply+question"), Some(JoinType::Apply));
        assert_eq!(JoinType::from_sub("x"), None);
    }

    #[test]
    fn discovery_needs_community_and_admins() {
        let plain = json!({ "features": [] });
        assert_eq!(options(&plain, JoinType::Invite, true).map(|o| o.1), [true, true, false]);
        let community = json!({ "features": ["COMMUNITY"] });
        assert_eq!(options(&community, JoinType::Invite, false).map(|o| o.1), [true, true, false]);
        assert_eq!(options(&community, JoinType::Invite, true).map(|o| o.1), [true, true, true]);
        let listed = json!({ "features": ["COMMUNITY", "DISCOVERABLE"] });
        assert_eq!(options(&listed, JoinType::Discoverable, false).map(|o| o.1), [false, false, true]);
    }

    #[test]
    fn age_switch_state() {
        assert!(age_restricted(&json!({ "owner_configured_content_level": 3 })));
        assert!(!age_restricted(&json!({ "owner_configured_content_level": 0, "nsfw_level": 3 })));
        assert!(age_locked(&json!({ "features": [], "nsfw_level": 3, "owner_configured_content_level": 0 }), JoinType::Invite));
        assert!(age_locked(&json!({ "features": [] }), JoinType::Discoverable));
        assert!(!age_locked(&json!({ "features": [], "nsfw_level": 0 }), JoinType::Invite));
    }

    #[test]
    fn draft_starts_from_the_form() {
        let g = json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED"], "owner_configured_content_level": 3 });
        let d = Draft::new(&g, &terms_form(&["a", "b"]), JoinType::Invite, "Why?");
        assert!(d.terms && d.age);
        assert_eq!(d.rules.iter().map(|r| r.value.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(!dirty(&x(&g, &terms_form(&["a", "b"])), &d).any());
        let empty = Draft::new(&g, &Value::Null, JoinType::Invite, "Why?");
        assert_eq!(empty.rules.len(), 1);
        assert_eq!(empty.rules[0].value, "");
        let apply = Draft::new(&g, &terms_form(&["a"]), JoinType::Apply, "Why?");
        assert_eq!(apply.fields.len(), 2);
        assert_eq!(apply.fields[1], json!({ "field_type": "TEXT_INPUT", "label": "Why?", "required": true }));
        assert!(!Draft::new(&g, &Value::Null, JoinType::Discoverable, "Why?").age);
    }

    #[test]
    fn rules_editor_limits() {
        let g = json!({ "features": [] });
        let mut d = Draft::new(&g, &Value::Null, JoinType::Invite, "q");
        let first = d.rules[0].id;
        assert_eq!(d.add_rule(Some("Be nice")), Some(first));
        assert_eq!(d.rules.len(), 1);
        for _ in 1..MAX_RULES {
            assert!(d.add_rule(None).is_some());
        }
        assert_eq!(d.add_rule(None), None);
        assert_eq!(d.rules.len(), MAX_RULES);
        let long = "x".repeat(400);
        let mut e = Draft::new(&g, &Value::Null, JoinType::Invite, "q");
        e.add_rule(Some(&long));
        assert_eq!(e.rules[0].value.chars().count(), RULE_LEN);
        e.remove_rule(0);
        assert_eq!(e.rules.len(), 1);
        assert!(e.rules[0].value.is_empty());
        let mut v = vec![1, 2, 3];
        assert!(move_item(&mut v, 0, 2));
        assert_eq!(v, [2, 3, 1]);
        assert!(!move_item(&mut v, 1, 1));
        assert_eq!(one_line("a\nb\r\nc", 4), "a b ");
    }

    #[test]
    fn invite_save_builds_the_client_requests() {
        let g = json!({ "features": ["DISCOVERABLE", "COMMUNITY"], "owner_configured_content_level": 0 });
        let form = Value::Null;
        let mut d = Draft::new(&g, &form, JoinType::Invite, "q");
        d.terms = true;
        assert_eq!(validate(&d), Err("gs.access.need_rule"));
        d.rules[0].value = "  Be kind  ".into();
        d.add_rule(None);
        d.age = true;
        assert_eq!(validate(&d), Ok(()));
        let w = writes(7, &x(&g, &form), &d, "Read and agree");
        assert_eq!(w.len(), 3);
        assert_eq!((w[0].path.as_str(), w[0].body.clone()), ("/guilds/7", Some(json!({ "owner_configured_content_level": 3 }))));
        assert_eq!(w[1].body, Some(json!({ "features": ["COMMUNITY"] })));
        assert_eq!(w[2].path, "/guilds/7/member-verification");
        assert_eq!(w[2].body, Some(json!({ "form_fields": [{ "field_type": "TERMS", "label": "Read and agree", "values": ["Be kind"], "required": true }], "enabled": true })));
        assert!(w.iter().all(|w| w.refusal().is_write_blocked()));
    }

    #[test]
    fn apply_needs_a_real_question() {
        let g = json!({ "features": [] });
        let form = terms_form(&["r"]);
        let mut d = Draft::new(&g, &form, JoinType::Apply, "Why?");
        d.remove_field(1);
        assert_eq!(d.fields.len(), 1);
        assert_eq!(validate(&d), Err("gs.access.need_question"));
        assert!(d.add_field(json!({ "field_type": "PARAGRAPH", "label": "Tell us", "required": true })));
        assert_eq!(validate(&d), Ok(()));
        for _ in 0..10 {
            d.add_field(json!({ "field_type": "TEXT_INPUT", "label": "x", "required": true }));
        }
        assert_eq!(d.fields.len(), MAX_FIELDS);
        let w = writes(7, &x(&g, &form), &d, "Read");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].body.as_ref().unwrap()["enabled"], json!(true));
        assert_eq!(w[0].body.as_ref().unwrap()["form_fields"].as_array().unwrap().len(), MAX_FIELDS);
        let gated = json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED", "MEMBER_VERIFICATION_MANUAL_APPROVAL"] });
        let f2 = json!({ "form_fields": [{ "field_type": "TEXT_INPUT", "label": "a", "required": true }] });
        let same = Draft::new(&gated, &f2, JoinType::Apply, "Why?");
        assert!(!dirty(&x(&gated, &f2), &same).any());
    }

    #[test]
    fn question_editor_limits() {
        let mut q = Question::new(FieldKind::Choice, None, Value::Null);
        assert_eq!(q.build("r"), Err("gs.access.need_label"));
        q.label = "Pick".into();
        assert_eq!(q.build("r"), Err("gs.access.need_choices"));
        q.choices[0].value = "A".into();
        for _ in 0..20 {
            q.add_choice();
        }
        assert_eq!(q.choices.len(), MAX_CHOICES);
        let f = q.build("r").unwrap();
        assert_eq!(f["field_type"], "MULTIPLE_CHOICE");
        assert_eq!(f["choices"].as_array().unwrap().len(), MAX_CHOICES);
        q.remove_choice(0);
        assert_eq!(q.choices.len(), MAX_CHOICES - 1);
        let edit = Question::new(FieldKind::Paragraph, Some(0), json!({ "field_type": "PARAGRAPH", "label": "Old", "placeholder": "p", "required": true }));
        assert_eq!(edit.label, "Old");
        assert_eq!(edit.build("r").unwrap()["placeholder"], "p");
        let mut t = Question::new(FieldKind::Terms, None, Value::Null);
        assert_eq!(t.build("Read"), Err("gs.access.need_rule"));
        t.choices[0].value = "rule".into();
        assert_eq!(t.build("Read").unwrap(), json!({ "field_type": "TERMS", "label": "Read", "values": ["rule"], "required": true }));
    }

    #[test]
    fn discovery_view_and_save() {
        let g = json!({ "features": ["COMMUNITY"], "description": "old" });
        assert_eq!(settings_view(&g, &json!({ "sufficient": false }), 0), View::Ineligible);
        assert_eq!(settings_view(&g, &json!({ "sufficient": true }), 0), View::EligibleDisabled);
        let listed = json!({ "features": ["COMMUNITY", "DISCOVERABLE"] });
        assert_eq!(settings_view(&listed, &json!({ "sufficient": false }), 0), View::Disqualified);
        assert_eq!(
            settings_view(&listed, &json!({ "sufficient": false, "sufficient_without_grace_period": true, "grace_period_end_date": "2030-01-01T00:00:00+00:00" }), 0),
            View::EligibleEnabled
        );
        assert_eq!(settings_view(&json!({ "features": ["VERIFIED"] }), &Value::Null, 0), View::EligibleDisabled);
        let mut d = Draft::new(&g, &Value::Null, JoinType::Discoverable, "q");
        d.keywords = Some(vec!["rust".into()]);
        let meta = json!({ "guild_id": "7", "primary_category_id": 1, "keywords": [], "emoji_discoverability_enabled": false });
        let ctx = Ctx { guild: &g, form: &Value::Null, profile: &Value::Null, metadata: &meta, splash: None };
        let w = writes(7, &ctx, &d, "Read");
        let guild = w.iter().find(|w| w.path == "/guilds/7").unwrap();
        assert_eq!(guild.body.as_ref().unwrap()["features"], json!(["COMMUNITY", "DISCOVERABLE"]));
        let m = w.iter().find(|w| w.path == "/guilds/7/discovery-metadata").unwrap();
        assert_eq!(m.body, Some(json!({ "primary_category_id": 1, "keywords": ["rust"], "emoji_discoverability_enabled": false })));
    }
}
