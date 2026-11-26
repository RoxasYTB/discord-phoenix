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
    pub primary: Option<u64>,
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
            primary: None,
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
        add_rule_to(&mut self.rules, &mut self.next, example)
    }

    pub fn remove_rule(&mut self, i: usize) {
        remove_rule_from(&mut self.rules, &mut self.next, i);
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

pub fn add_rule_to(rules: &mut Vec<Rule>, next: &mut u64, example: Option<&str>) -> Option<u64> {
    if rules.len() >= MAX_RULES {
        return None;
    }
    if let Some(e) = example
        && let Some(last) = rules.last_mut()
        && last.value.is_empty()
    {
        last.value = clip(e, RULE_LEN);
        return Some(last.id);
    }
    let id = *next;
    *next += 1;
    rules.push(Rule { id, value: example.map(|e| clip(e, RULE_LEN)).unwrap_or_default() });
    Some(id)
}

pub fn remove_rule_from(rules: &mut Vec<Rule>, next: &mut u64, i: usize) {
    if i < rules.len() {
        rules.remove(i);
    }
    if rules.is_empty() {
        rules.push(Rule { id: *next, value: String::new() });
        *next += 1;
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
            || d.primary.is_some_and(|p| p != primary_category(self.metadata))
    }
}

pub fn keywords(metadata: &Value) -> Vec<String> {
    metadata.get("keywords").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

pub const MAX_SUBCATEGORIES: usize = 5;
pub const OTHER_CATEGORY: u64 = 13;

pub const POPULAR_TAGS: &[(u64, &[&str])] = &[
    (14, &["gaming", "roblox", "youtube", "twitch", "minecraft", "community", "streamer", "fortnite", "anime", "roleplay"]),
    (1, &["minecraft", "roblox", "twitch", "youtube", "fortnite", "esports", "roleplay", "valorant", "leagueoflegends", "fivem"]),
    (2, &["edm", "rap", "kpop", "youtube", "music_production", "hiphop", "singing", "gaming", "bts", "metal"]),
    (3, &["anime", "youtube", "gaming", "twitch", "manga", "memes", "streamer", "social", "reddit"]),
    (4, &["art", "anime", "design", "youtube", "animation", "t3d", "nft", "manga", "drawing", "photography"]),
    (5, &["programming", "tech", "coding", "github", "opensource", "javascript", "cryptocurrency", "hardware", "software", "linux"]),
    (6, &["study", "language", "school", "english", "learning", "homework", "programming", "college", "stocks", "youtube"]),
    (7, &["football", "nba", "nfl", "soccer", "basketball", "formula1", "premier_league", "racing", "reddit", "fantasy_football"]),
    (8, &["fashion", "streetwear", "sneakers", "clothing", "reddit", "yeezy", "supreme", "hypebeast", "style"]),
    (9, &["lgbtq", "anime", "social", "friends", "community"]),
    (10, &["food", "disney", "beverage", "snacks", "youtube", "travel", "disneyworld"]),
    (11, &["gym", "bodybuilding", "exercise", "calisthenics", "mental_health", "self_improvement", "education", "crossfit", "nutrition", "sports"]),
    (12, &["stocks", "trading", "investing", "bitcoin", "cryptocurrency", "ethereum", "options", "defi", "forex"]),
    (13, &["gaming", "youtube", "twitch", "community", "anime", "events", "social", "memes"]),
    (43, &["emoji", "emotes", "nitro", "anime", "cute", "giveaways", "aesthetic", "art", "memes"]),
    (49, &["apps", "bots", "discord_bot", "giveaways", "support", "fun", "memes"]),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: u64,
    pub name: String,
}

fn category_name(c: &Value, locale: &str) -> Option<String> {
    let n = &c["name"];
    n.as_str()
        .or_else(|| n["localizations"][locale].as_str())
        .or_else(|| n["localizations"][locale.split('-').next().unwrap_or(locale)].as_str())
        .or_else(|| n["default"].as_str())
        .map(str::to_owned)
}

pub fn categories(v: &Value, locale: &str) -> (Vec<Category>, Vec<Category>) {
    let mut list: Vec<(Category, bool)> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let id = c["id"].as_u64().or_else(|| c["id"].as_str()?.parse().ok())?;
            Some((Category { id, name: category_name(c, locale)? }, c["is_primary"].as_bool() == Some(true)))
        })
        .collect();
    list.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    let (mut primary, mut all, mut other) = (Vec::new(), Vec::new(), None);
    for (c, is_primary) in list {
        if c.id == 0 {
            continue;
        }
        if c.id == OTHER_CATEGORY {
            other = Some(c);
            continue;
        }
        if is_primary {
            primary.push(c.clone());
        }
        all.push(c);
    }
    primary.extend(other);
    (primary, all)
}

pub fn popular_tags(primary: u64, keywords: &[String], tr: impl Fn(&str) -> String) -> Vec<String> {
    POPULAR_TAGS.iter().find(|(id, _)| *id == primary).map(|(_, t)| t.iter().map(|k| tr(k)).filter(|t| !keywords.contains(t)).collect()).unwrap_or_default()
}

pub fn primary_category(metadata: &Value) -> u64 {
    metadata.get("primary_category_id").and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(0)
}

pub fn subcategories(metadata: &Value) -> Vec<u64> {
    metadata.get("category_ids").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok())).collect()).unwrap_or_default()
}

pub fn toggle_subcategory(guild: u64, current: &[u64], id: u64) -> Option<GuardedWrite> {
    let on = current.contains(&id);
    if !on && current.len() >= MAX_SUBCATEGORIES {
        return None;
    }
    Some(GuardedWrite::discovery_category(guild, id, !on))
}

const METADATA_KEYS: [&str; 9] =
    ["primary_category_id", "emoji_discoverability_enabled", "partner_actioned_timestamp", "partner_application_timestamp", "keywords", "is_published", "reasons_to_join", "social_links", "about"];

pub fn metadata_body(metadata: &Value, d: &Draft) -> Value {
    let mut out = serde_json::Map::new();
    for k in METADATA_KEYS {
        if let Some(v) = metadata.get(k) {
            out.insert(k.into(), v.clone());
        }
    }
    if let Some(p) = d.primary {
        out.insert("primary_category_id".into(), json!(p));
    }
    if let Some(k) = &d.keywords {
        out.insert("keywords".into(), json!(k));
    }
    if let Some(e) = d.emoji_info {
        out.insert("emoji_discoverability_enabled".into(), json!(e));
    }
    Value::Object(out)
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
                out.push(GuardedWrite::discovery_metadata(guild_id, metadata_body(x.metadata, d)));
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragList {
    Rules,
    Fields,
    Choices,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drag {
    pub list: DragList,
    pub origin: usize,
    pub from: usize,
    pub over: usize,
}

pub fn hover_target(d: &Drag, over: usize, offset: f32, height: f32) -> Option<usize> {
    let mid = height / 2.;
    let skip = match d.list {
        DragList::Fields => offset < mid || d.origin == over,
        DragList::Rules | DragList::Choices => (d.origin < over && offset < mid) || (d.origin > over && offset > mid),
    };
    (!skip && over != d.from).then_some(over)
}

thread_local! {
    static ROW_BOUNDS: std::cell::RefCell<HashMap<(DragList, usize), gpui::Bounds<gpui::Pixels>>> = std::cell::RefCell::new(HashMap::new());
}

fn bounds_probe(list: DragList, i: usize) -> AnyElement {
    gpui::canvas(move |b, _, _| ROW_BOUNDS.with(|m| m.borrow_mut().insert((list, i), b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full().into_any_element()
}

fn apply_move(p: &mut Pending, list: DragList, from: usize, to: usize) {
    match list {
        DragList::Rules => {
            if let Some(dr) = p.draft.as_mut() {
                move_item(&mut dr.rules, from, to);
            }
        }
        DragList::Fields => {
            if let Some(dr) = p.draft.as_mut() {
                move_item(&mut dr.fields, from, to);
            }
        }
        DragList::Choices => {
            if let Some(q) = p.question.as_mut() {
                move_item(&mut q.choices, from, to);
            }
        }
    }
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
    pub open: Option<String>,
    pub preview: bool,
    pub agree: bool,
    pub viewport: f32,
    pub viewport_h: f32,
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
    match s.access.open.take().as_deref() {
        Some("question") => {
            if let Some(f) = s.access.draft.as_ref().and_then(|d| d.fields.iter().position(|f| FieldKind::of(f) != Some(FieldKind::Terms)).map(|i| (i, d.fields[i].clone()))) {
                open_question(s, FieldKind::of(&f.1).unwrap_or(FieldKind::Text), Some(f.0), f.1);
            }
        }
        Some("text") => open_question(s, FieldKind::Text, None, Value::Null),
        Some("paragraph") => open_question(s, FieldKind::Paragraph, None, Value::Null),
        Some("choice") => open_question(s, FieldKind::Choice, None, Value::Null),
        Some("terms") => {
            let base = s.access.draft.as_ref().map(|d| json!({ "values": d.rule_values() })).unwrap_or(Value::Null);
            open_question(s, FieldKind::Terms, None, base);
        }
        Some("preview") => s.access.preview = true,
        _ => {}
    }
}

pub fn preselect(s: &mut State, sub: &str) {
    s.access.pick = JoinType::from_sub(sub);
    let extra = sub.split_once('+').map(|x| x.1);
    s.access.menu = extra == Some("menu");
    s.access.open = extra.filter(|e| *e != "menu").map(str::to_owned);
    s.access.preview = false;
    s.access.draft = None;
}

pub fn extra_needs(s: &State) -> &'static [&'static str] {
    let join = s.access.draft.as_ref().map(|d| d.join).or(s.access.pick).unwrap_or_else(|| join_type(s.guild_json()));
    if join == JoinType::Discoverable { &["requirements", "disc_meta", "disc_cats"] } else { &[] }
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
        && d.primary.unwrap_or_else(|| primary_category(x.metadata)) != 0
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
    close_question(s);
    s.access.question = Some(Question::new(kind, index, base));
    s.access.menu = false;
    s.access.focus = Some(if kind == FieldKind::Terms { "qrule:1".into() } else { "q:label".into() });
}

fn close_question(s: &mut State) {
    s.access.question = None;
    s.access.inputs.retain(|k, _| !k.starts_with("q:"));
    s.access.areas.retain(|k, _| !k.starts_with("qrule:"));
}

fn save_question(s: &mut State) {
    let label = crate::t!("gs.access.terms_label");
    let Some(q) = s.access.question.as_mut() else { return };
    match q.build(&label) {
        Ok(f) => {
            let index = q.index;
            close_question(s);
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
    } else if let Some(id) = key.strip_prefix("q:choice:").or_else(|| key.strip_prefix("qrule:")).and_then(|x| x.parse::<u64>().ok()) {
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
    if key.starts_with("rule:") || key.starts_with("qrule:") {
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
    s.access.viewport = f32::from(window.viewport_size().width) / crate::theme::zoom();
    s.access.viewport_h = f32::from(window.viewport_size().height) / crate::theme::zoom();
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
            want_inputs.push(("q:label".into(), q.label.clone(), crate::t!("gs.access.question_input_ph")));
        }
        for (i, ch) in q.choices.iter().enumerate() {
            if q.kind == FieldKind::Terms {
                let k = rule_key(true, ch.id);
                if !s.access.areas.contains_key(&k) {
                    want_areas.push((k, ch.value.clone(), crate::t!("gs.access.rule_ph"), 1));
                }
            } else if q.kind == FieldKind::Choice {
                let k = format!("q:choice:{}", ch.id);
                if !s.access.inputs.contains_key(&k) {
                    want_inputs.push((k, ch.value.clone(), crate::t!("gs.access.option", n = i + 1)));
                }
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
            InputEvent::PressEnter { .. } if k == "q:label" && with_state(|s| s.access.question.as_ref().is_some_and(|q| matches!(q.kind, FieldKind::Text | FieldKind::Paragraph))).unwrap_or(false) => {
                with_state(save_question);
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

fn preview_link(cx: &mut Cx) -> AnyElement {
    div()
        .id("gs-access-preview")
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .on_click(edit(cx, |s| s.access.preview = true))
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
    let move_l = std::rc::Rc::new(cx.listener(move |_, e: &gpui::MouseMoveEvent, _, cx| {
        let changed = with_state(|s| {
            let Some(d) = s.access.drag.filter(|d| d.list == list) else { return false };
            let Some(b) = ROW_BOUNDS.with(|m| m.borrow().get(&(list, i)).copied()) else { return false };
            let offset = f32::from(e.position.y - b.origin.y);
            let Some(to) = hover_target(&d, i, offset, f32::from(b.size.height)) else { return false };
            apply_move(&mut s.access, list, d.from, to);
            s.access.drag = Some(Drag { from: to, over: to, ..d });
            true
        })
        .unwrap_or(false);
        if changed {
            cx.notify();
        }
    }));
    let up_l = std::rc::Rc::new(cx.listener(move |_, _: &gpui::MouseUpEvent, _, cx| {
        if with_state(|s| s.access.drag.take().is_some()).unwrap_or(false) {
            cx.notify();
        }
    }));
    move |d: gpui::Stateful<gpui::Div>| {
        let m = move_l.clone();
        let u = up_l.clone();
        d.child(bounds_probe(list, i)).on_mouse_move(move |e, w, cx| m(e, w, cx)).on_mouse_up(MouseButton::Left, move |e, w, cx| u(e, w, cx))
    }
}

fn drag_handle(id: (&'static str, usize), list: DragList, i: usize, cx: &mut Cx) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .cursor_grab()
        .child(gpui::svg().path(crate::icons::path("server/drag")).size(z(12.)).text_color(c(tok(idx::ICON_MUTED))))
        .on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, cx| {
            with_state(|s| s.access.drag = Some(Drag { list, origin: i, from: i, over: i }));
            cx.notify();
        }))
}

fn rules_mut(s: &mut State, modal: bool) -> Option<(&mut Vec<Rule>, &mut u64)> {
    if modal {
        s.access.question.as_mut().map(|q| (&mut q.choices, &mut q.next))
    } else {
        s.access.draft.as_mut().map(|d| (&mut d.rules, &mut d.next))
    }
}

fn rule_key(modal: bool, id: u64) -> String {
    if modal { format!("qrule:{id}") } else { format!("rule:{id}") }
}

fn rules_editor(s: &State, disabled: bool, modal: bool, cx: &mut Cx) -> AnyElement {
    let rules = if modal { s.access.question.as_ref().map(|q| q.choices.as_slice()) } else { s.access.draft.as_ref().map(|d| d.rules.as_slice()) };
    let Some(rules) = rules else { return div().into_any_element() };
    let list_kind = if modal { DragList::Choices } else { DragList::Rules };
    let drag = s.access.drag.filter(|g| g.list == list_kind);
    let can_drag = !disabled && rules.len() > 1;
    let full = rules.len() >= MAX_RULES;
    let mut list = div().my(z(12.)).flex().flex_col().gap(z(12.));
    for (i, r) in rules.iter().enumerate() {
        let key = rule_key(modal, r.id);
        let group: SharedString = format!("gs-access-rule-{}-{}", modal as u8, r.id).into();
        let body = match s.access.areas.get(&key) {
            Some(e) => div().child(Textarea::new(e).appearance(false).bordered(false).disabled(disabled)).into_any_element(),
            None => text(r.value.clone(), 16., 22., FontWeight::NORMAL, idx::TEXT_DEFAULT).into_any_element(),
        };
        let row = div()
            .id((if modal { "gs-access-qrule" } else { "gs-access-rule" }, r.id as usize))
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
                            .id((if modal { "gs-access-qrule-clear" } else { "gs-access-rule-clear" }, r.id as usize))
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
                                    if let Some((rules, next)) = rules_mut(s, modal) {
                                        remove_rule_from(rules, next, i);
                                    }
                                    if let Some(q) = s.access.question.as_mut() {
                                        q.error = None;
                                    }
                                }))
                            }),
                    ),
            )
            .when(can_drag, |d| d.child(drag_handle((if modal { "gs-access-qrule-drag" } else { "gs-access-rule-drag" }, r.id as usize), list_kind, i, cx).absolute().left(z(4.)).top(z(20.))));
        list = list.child(drag_row(list_kind, i, cx)(row));
    }
    let tip = full.then(|| crate::t!("gs.access.rules_max", n = MAX_RULES));
    let examples: Vec<(SharedString, bool, Option<SharedString>)> = ["ex1", "ex2", "ex3", "ex4"].iter().map(|k| (crate::t!(&format!("gs.access.{k}")), full || disabled, tip.clone())).collect();
    div()
        .flex()
        .flex_col()
        .child(small_title(crate::t!("gs.access.rules")))
        .child(list)
        .when(!full, |d| {
            d.child(add_button(if modal { "gs-access-add-qrule" } else { "gs-access-add-rule" }, crate::t!("gs.access.add_rule"), None).when(!disabled, |d| {
                d.on_click(edit(cx, move |s| {
                    if let Some(id) = rules_mut(s, modal).and_then(|(rules, next)| add_rule_to(rules, next, None)) {
                        s.access.focus = Some(rule_key(modal, id));
                    }
                }))
            }))
        })
        .child(small_title(crate::t!("gs.access.example_rules")).mt(z(24.)).mb(z(8.)))
        .child(pills(if modal { "gs-access-ex-qrule" } else { "gs-access-ex-rule" }, examples, true, cx, move |s, i| {
            let body = crate::t!(&format!("gs.access.ex{}_body", i + 1));
            if let Some(id) = rules_mut(s, modal).and_then(|(rules, next)| add_rule_to(rules, next, Some(&body))) {
                let k = rule_key(modal, id);
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
        .when(on, |d| d.child(div().mt(z(8.)).child(preview_link(cx))))
        .child(div().mt(z(24.)).p(z(16.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).when(!on, |d| d.opacity(0.5).cursor_not_allowed()).child(rules_editor(s, !on, false, cx)))
        .into_any_element()
}

fn short_answer_icon() -> AnyElement {
    let bar = |w: f32, top: f32| div().absolute().left_0().top(z(top)).w(z(w)).h(z(2.)).rounded(z(1.)).bg(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)));
    div().relative().flex_none().w(z(12.75)).h(z(12.)).child(bar(12., 3.75)).child(bar(6., 6.75)).into_any_element()
}

fn field_row(s: &State, i: usize, f: &Value, count: usize, cx: &mut Cx) -> AnyElement {
    let kind = FieldKind::of(f).unwrap_or(FieldKind::Text);
    let title: SharedString = if kind == FieldKind::Terms { crate::t!("gs.access.terms_title") } else { f.get("label").and_then(Value::as_str).unwrap_or_default().to_owned().into() };
    let group: SharedString = format!("gs-access-field-{i}").into();
    let hovered = s.access.drag.is_some_and(|g| g.list == DragList::Fields && g.over == i);
    let icon = match kind.icon() {
        Some(n) => gpui::svg().path(crate::icons::path(n)).size(z(12.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element(),
        None => short_answer_icon(),
    };
    let base = f.clone();
    let can_remove = count > 1;
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
        .mt(z(8.))
        .flex()
        .items_center()
        .ml(z(-25.))
        .when(hovered, |d| d.opacity(0.5))
        .child(
            div()
                .id(("gs-access-field-grip", i))
                .relative()
                .top(z(6.))
                .w(z(25.))
                .h(z(40.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .when(count > 1, |d| d.child(drag_handle(("gs-access-field-drag", i), DragList::Fields, i, cx).invisible().group_hover(group.clone(), |d| d.visible()))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .relative()
                .overflow_hidden()
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .p(z(16.))
                .flex()
                .items_center()
                .justify_between()
                .gap(z(16.))
                .child(text(title, 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT).flex_1().min_w_0().overflow_hidden())
                .child(div().flex_none().flex().items_center().gap(z(4.)).child(icon).child(text(kind.label(), 14., 18., FontWeight::MEDIUM, idx::INTERACTIVE_TEXT_DEFAULT))),
        )
        .child(
            div()
                .absolute()
                .top(z(2.))
                .right(z(if can_remove { -88. } else { -48. }))
                .p(z(8.))
                .flex()
                .gap(z(8.))
                .invisible()
                .group_hover(group.clone(), |d| d.visible())
                .child(action(("gs-access-field-edit", i), "pen", crate::t!("gs.access.edit")).on_click(edit(cx, move |s| open_question(s, kind, Some(i), base.clone()))))
                .when(can_remove, |d| {
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
    let mut fields = div().flex().flex_col();
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
        .child(small_title(crate::t!("gs.access.questions", n = d.fields.len(), max = MAX_FIELDS)))
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
        .child(preview_link(cx))
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

fn guild_counts(app: &PhoenixApp, s: &State) -> (Option<u64>, Option<u64>) {
    let p = s.get("profile");
    let g = s.guild_json();
    let pick = |k: &str, alt: &str| p.and_then(|p| p.get(k)).or_else(|| g.get(alt)).and_then(Value::as_u64);
    let members = pick("member_count", "approximate_member_count").or_else(|| app.guilds.iter().find(|x| x.id == s.guild).and_then(|x| x.member_count).map(u64::from));
    (pick("online_count", "approximate_presence_count"), members)
}

pub fn card_guild(app: &PhoenixApp, s: &State) -> crate::ui::discovery::StaticCard {
    let g = s.guild_json();
    let id = s.guild.0;
    let name = app.guilds.iter().find(|x| x.id == s.guild).map(|x| x.name.clone()).unwrap_or_else(|| kit::s(g, "name").into());
    let description = s.access.draft.as_ref().and_then(|d| d.description.clone()).unwrap_or_else(|| kit::s(g, "description"));
    let banner: Option<SharedString> =
        g.get("discovery_splash").and_then(Value::as_str).filter(|h| !h.is_empty()).map(|h| format!("https://cdn.discordapp.com/discovery-splashes/{id}/{h}.jpg?size=480").into());
    let (online, members) = guild_counts(app, s);
    crate::ui::discovery::StaticCard {
        guild: crate::ui::discovery::DiscoverableGuild {
            id,
            name,
            description: description.into(),
            icon: super::profile::icon_url(id, g, 40),
            banner,
            online: online.unwrap_or(0),
            members: members.unwrap_or(0),
            verified: has(g, "VERIFIED"),
            partnered: has(g, "PARTNERED"),
        },
        online,
        members,
        local_splash: s.uploads.get("discovery_splash").cloned(),
    }
}

fn discovery_card(app: &PhoenixApp, s: &State) -> AnyElement {
    crate::ui::discovery::static_card(&card_guild(app, s), 254., 266.).into_any_element()
}

fn clear_box(id: &'static str, value: AnyElement, open: bool, clear: Option<gpui::Stateful<gpui::Div>>) -> gpui::Stateful<gpui::Div> {
    kit::select_box(id, div().flex().items_center().child(div().flex_1().min_w_0().truncate().child(value)).children(clear).into_any_element(), open)
}

fn primary_select(s: &State, primary: &[Category], current: u64, disabled: bool, cx: &mut Cx) -> AnyElement {
    if primary.is_empty() {
        return text(crate::t!("common.loading"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).into_any_element();
    }
    let id = "gs-access-cat";
    let open = s.open_select == Some(id);
    let cur = primary.iter().position(|c| c.id == current);
    let value = match cur {
        Some(i) => text(primary[i].name.clone(), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT),
        None => text(crate::t!("gs.access.d_category_ph"), 16., 20., FontWeight::MEDIUM, idx::INPUT_PLACEHOLDER_TEXT_DEFAULT),
    };
    let clear = cur.map(|_| {
        div()
            .id("gs-access-cat-clear")
            .flex_none()
            .cursor_pointer()
            .child(gpui::svg().path(crate::icons::path("close-small")).size(z(16.)).text_color(c(tok(idx::ICON_STRONG))))
            .on_click(cx.listener(|_, _, _, cx| {
                cx.stop_propagation();
                with_state(|s| {
                    if let Some(d) = s.access.draft.as_mut() {
                        d.primary = Some(0);
                    }
                    s.open_select = None;
                });
                cx.notify();
            }))
    });
    let field = clear_box(id, value.into_any_element(), open, clear).when(disabled, |d| d.opacity(0.5).cursor_not_allowed()).when(!disabled, |d| {
        d.on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| s.open_select = if s.open_select == Some(id) { None } else { Some(id) });
            cx.notify();
        }))
    });
    let items: Vec<(u64, String, bool)> = primary.iter().map(|c| (c.id, c.name.clone(), c.id == current)).collect();
    select_list(
        id,
        field,
        open,
        items,
        false,
        cx,
        std::rc::Rc::new(|_: &mut PhoenixApp, cat: u64, _: &mut Cx| {
            with_state(|s| {
                if let Some(d) = s.access.draft.as_mut() {
                    d.primary = Some(cat);
                }
                s.open_select = None;
            });
        }),
    )
}

type OnCategory = std::rc::Rc<dyn Fn(&mut PhoenixApp, u64, &mut Cx<'_>)>;

fn select_list(id: &'static str, field: gpui::Stateful<gpui::Div>, open: bool, items: Vec<(u64, String, bool)>, multi: bool, cx: &mut Cx, on: OnCategory) -> AnyElement {
    let mut wrap = div().relative().w_full().child(field);
    if open {
        let list = div()
            .id(SharedString::from(format!("{id}-list")))
            .absolute()
            .top(z(44.))
            .left_0()
            .w_full()
            .max_h(z(320.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.16), offset: gpui::point(z(0.), z(2.)), blur_radius: z(8.), spread_radius: z(0.), inset: false }])
            .occlude()
            .children(items.into_iter().map(|(cat, name, on_now)| {
                let f = on.clone();
                let check = if multi {
                    div()
                        .size(z(20.))
                        .flex_none()
                        .rounded(z(4.))
                        .border_1()
                        .border_color(c(tok(if on_now { idx::CONTROL_BRAND_FOREGROUND_NEW } else { idx::INTERACTIVE_TEXT_DEFAULT })))
                        .when(on_now, |d| d.bg(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("check")).size(z(14.)).text_color(gpui::white())))
                        .into_any_element()
                } else if on_now {
                    gpui::svg().path(crate::icons::path("circle-check")).flex_none().size(z(20.)).text_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))).into_any_element()
                } else {
                    div().into_any_element()
                };
                div()
                    .id(SharedString::from(format!("{id}-{cat}")))
                    .h(z(40.))
                    .pl(z(8.))
                    .pr(z(4.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(z(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .on_click(cx.listener(move |a, _, _, cx| {
                        f(a, cat, cx);
                        cx.notify();
                    }))
                    .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(name))
                    .child(check)
            }));
        wrap = wrap.child(gpui::deferred(list).with_priority(30));
    }
    wrap.into_any_element()
}

fn subcategory_select(s: &State, all: &[Category], current: &[u64], disabled: bool, cx: &mut Cx) -> AnyElement {
    if all.is_empty() {
        return text(crate::t!("common.loading"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).into_any_element();
    }
    let id = "gs-access-subcat";
    let open = s.open_select == Some(id);
    let names: Vec<String> = current.iter().filter_map(|c| all.iter().find(|x| x.id == *c).map(|x| x.name.clone())).collect();
    let value = if names.is_empty() { text(crate::t!("gs.access.d_category_ph"), 16., 20., FontWeight::MEDIUM, idx::INPUT_PLACEHOLDER_TEXT_DEFAULT) } else { text(names.join(", "), 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT) };
    let field = clear_box(id, value.into_any_element(), open, None).when(disabled, |d| d.opacity(0.5).cursor_not_allowed()).when(!disabled, |d| {
        d.on_click(cx.listener(move |_, _, _, cx| {
            with_state(|s| s.open_select = if s.open_select == Some(id) { None } else { Some(id) });
            cx.notify();
        }))
    });
    let items: Vec<(u64, String, bool)> = all.iter().map(|c| (c.id, c.name.clone(), current.contains(&c.id))).collect();
    select_list(
        id,
        field,
        open,
        items,
        true,
        cx,
        std::rc::Rc::new(|a: &mut PhoenixApp, cat: u64, cx: &mut Cx| {
            let w = with_state(|s| toggle_subcategory(s.guild.0, &subcategories(s.get("disc_meta").unwrap_or(&Value::Null)), cat)).flatten();
            if let Some(w) = w {
                a.blocked_write(w, cx);
            }
        }),
    )
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
    let (primary_cats, all_cats) = categories(s.get("disc_cats").unwrap_or(&Value::Null), crate::i18n::locale());
    let primary = d.and_then(|d| d.primary).unwrap_or_else(|| primary_category(&meta));
    let subs = subcategories(&meta);
    let category = div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(section_header(crate::t!("gs.access.d_category_title"), crate::t!("gs.access.d_category_title_desc")))
        .child(div().flex().flex_col().child(sub_header(crate::t!("gs.access.d_category"), required, crate::t!("gs.access.d_category_desc"))).child(div().mt(z(16.)).child(primary_select(s, &primary_cats, primary, false, cx))))
        .child(div().flex().flex_col().child(sub_header(crate::t!("gs.access.d_subcategories"), false, crate::t!("gs.access.d_subcategories_desc"))).child(div().mt(z(16.)).child(subcategory_select(s, &all_cats, &subs, false, cx))));
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
    let full = tags.len() >= MAX_TAGS;
    let popular = popular_tags(primary, &tags, |k| crate::t!(&format!("gs.access.tag.{k}")).to_string());
    let popular_pills: Vec<(SharedString, bool, Option<SharedString>)> = popular.iter().map(|t| (SharedString::from(t.clone()), full, full.then(|| crate::t!("gs.access.d_tags_max")))).collect();
    let tags_section = div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(
            div()
                .flex()
                .flex_col()
                .child(sub_header(crate::t!("gs.access.d_tags"), required, crate::t!("gs.access.d_tags_desc")))
                .child(div().mt(z(16.)).child(tags_box))
                .when(full, |d| d.child(text(crate::t!("gs.access.d_tags_max"), 14., 18., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.)))),
        )
        .when(!popular.is_empty(), |d| {
            d.child(div().flex().flex_col().child(small_title(crate::t!("gs.access.d_popular")).mb(z(4.))).child(div().mt(z(16.)).child(pills("gs-access-popular", popular_pills, false, cx, move |s, i| {
                if let Some(t) = popular.get(i) {
                    add_tag(s, t);
                }
            }))))
        });
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

pub const CHECKLIST_OK: &str = "https://discord.com/assets/eaee201e4840b582.svg";
pub const CHECKLIST_FAIL: &str = "https://discord.com/assets/47fa1a531cbfd55c.svg";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Pass,
    Fail,
    Pending,
}

pub struct CheckItem {
    pub key: &'static str,
    pub state: Check,
}

fn flag(req: &Value, k: &str) -> bool {
    req.get(k).and_then(Value::as_bool).unwrap_or(false)
}

pub fn nsfw_clean(req: &Value) -> bool {
    match req.get("nsfw_properties") {
        None | Some(Value::Null) => true,
        Some(Value::Object(o)) => o.is_empty(),
        _ => false,
    }
}

pub fn check_items(req: &Value) -> Vec<CheckItem> {
    let pass = |b: bool| if b { Check::Pass } else { Check::Fail };
    vec![
        CheckItem { key: "safe", state: pass(flag(req, "safe_environment")) },
        CheckItem { key: "size", state: pass(flag(req, "size")) },
        CheckItem { key: "age", state: pass(flag(req, "age")) },
        CheckItem { key: "health", state: if flag(req, "health_score_pending") { Check::Pending } else { pass(flag(req, "healthy")) } },
        CheckItem { key: "nsfw", state: pass(nsfw_clean(req)) },
        CheckItem { key: "2fa", state: pass(flag(req, "protected")) },
    ]
}

fn minimum_size(req: &Value) -> u64 {
    req.get("minimum_size").and_then(Value::as_u64).unwrap_or(1000)
}

pub fn minimum_weeks(req: &Value) -> u64 {
    req.get("minimum_age").and_then(Value::as_u64).unwrap_or(0).div_ceil(7)
}

pub fn pass_date_ms(guild: u64, req: &Value) -> i64 {
    let created = ((guild >> 22) + 1_420_070_400_000) as i64;
    created + req.get("minimum_age").and_then(Value::as_i64).unwrap_or(0) * 86_400_000
}

fn muted(t: SharedString) -> AnyElement {
    text(t, 14., 18., FontWeight::NORMAL, idx::TEXT_MUTED).into_any_element()
}

fn critical(t: SharedString) -> AnyElement {
    text(t, 14., 18., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL).into_any_element()
}

fn check_texts(item: &CheckItem, req: &Value, s: &State) -> (SharedString, AnyElement) {
    let min = discovery_count(minimum_size(req));
    let weeks = minimum_weeks(req).to_string();
    let partner = has(s.guild_json(), "PARTNERED");
    let fail = item.state == Check::Fail;
    match (item.key, item.state) {
        ("safe", _) => (
            if fail { crate::t!("gs.access.ck_safe_fail") } else { crate::t!("gs.access.ck_safe") },
            muted(if fail { crate::t!("gs.access.ck_safe_fail_desc") } else { crate::t!("gs.access.ck_safe_desc") }),
        ),
        ("size", _) if fail => (
            crate::t!("gs.access.ck_size_fail", minMembers = min),
            muted(if partner { crate::t!("gs.access.ck_size_fail_desc_partner", minMembers = min) } else { crate::t!("gs.access.ck_size_fail_desc", minMembers = min) }),
        ),
        ("size", _) => (crate::t!("gs.access.ck_size", minMembers = minimum_size(req)), div().into_any_element()),
        ("age", _) if fail => {
            let when = chrono::DateTime::from_timestamp_millis(pass_date_ms(s.guild.0, req)).map(|d| d.with_timezone(&chrono::Local)).map(|d| crate::i18n::intl::format(crate::i18n::locale(), "LL", &crate::i18n::intl::Parts::of(&d), None)).unwrap_or_default();
            (crate::t!("gs.access.ck_age_fail"), muted(crate::t!("gs.access.ck_age_fail_desc", minimumGuildAge = weeks, passDate = when)))
        }
        ("age", _) => (crate::t!("gs.access.ck_age"), muted(crate::t!("gs.access.ck_age_desc", minimumGuildAge = weeks))),
        ("health", Check::Pending) => (
            crate::t!("gs.access.ck_health_pending"),
            muted(if flag(req, "size") { crate::t!("gs.access.ck_health_pending_desc") } else { crate::t!("gs.access.ck_health_small", minMembers = 200) }),
        ),
        ("health", Check::Fail) => (
            crate::t!("gs.access.ck_health_fail"),
            div()
                .flex()
                .flex_col()
                .child(div().mb(z(8.)).child(muted(crate::t!("gs.access.ck_health_fail_desc"))))
                .when(!flag(req, "engagement_healthy"), |d| d.child(critical(crate::t!("gs.access.ck_engagement"))))
                .when(!flag(req, "retention_healthy"), |d| d.child(critical(crate::t!("gs.access.ck_retention"))))
                .when(req.get("health_score").is_some_and(|h| !h.is_null()), |d| d.child(text(crate::t!("gs.access.ck_health_details"), 14., 18., FontWeight::NORMAL, idx::TEXT_LINK)))
                .into_any_element(),
        ),
        ("health", _) => (crate::t!("gs.access.ck_health"), muted(crate::t!("gs.access.ck_health_desc"))),
        ("nsfw", _) if fail => {
            let n = req.get("nsfw_properties").cloned().unwrap_or(Value::Null);
            let present = |k: &str| n.get(k).is_some_and(|v| !v.is_null());
            (
                crate::t!("gs.access.ck_nsfw_fail"),
                div()
                    .flex()
                    .flex_col()
                    .when(present("channels"), |d| d.child(muted(crate::t!("gs.access.ck_nsfw_channels"))))
                    .when(present("name"), |d| d.child(muted(crate::t!("gs.access.ck_nsfw_name"))))
                    .when(present("description"), |d| d.child(muted(crate::t!("gs.access.ck_nsfw_description"))))
                    .when(["icon", "discovery_splash", "invite_splash", "banner"].iter().any(|k| present(k)), |d| d.child(muted(crate::t!("gs.access.ck_nsfw_images"))))
                    .into_any_element(),
            )
        }
        ("nsfw", _) => (crate::t!("gs.access.ck_nsfw"), muted(crate::t!("gs.access.ck_nsfw_desc"))),
        (_, _) => (
            if fail { crate::t!("gs.access.ck_2fa_fail") } else { crate::t!("gs.access.ck_2fa") },
            muted(if fail { crate::t!("gs.access.ck_2fa_fail_desc") } else { crate::t!("gs.access.ck_2fa_desc") }),
        ),
    }
}

fn discovery_count(n: u64) -> String {
    crate::ui::discovery::group_digits(n)
}

fn checklist(s: &State, header: AnyElement) -> AnyElement {
    let shell = || {
        div()
            .flex()
            .flex_col()
            .rounded(z(global::RADIUS_SM))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::CARD_BACKGROUND_DEFAULT)))
            .shadow(crate::ui::inbox::shadow_medium())
            .overflow_hidden()
    };
    let Some(req) = s.get("requirements").filter(|r| !r.is_null()) else {
        return shell().py(z(64.)).items_center().child(text(crate::t!("gs.access.ck_loading"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).mb(z(24.))).into_any_element();
    };
    let items = check_items(req);
    let n = items.len();
    let mut out = shell().child(
        div()
            .flex()
            .items_center()
            .p(z(16.))
            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .child(gpui::img(if flag(req, "sufficient") { CHECKLIST_OK } else { CHECKLIST_FAIL }).w(z(40.)).h(z(40.)).flex_none().mr(z(16.)))
            .child(div().flex_1().min_w_0().child(header)),
    );
    for (i, it) in items.iter().enumerate() {
        let (name, desc) = check_texts(it, req, s);
        let icon = match it.state {
            Check::Pending => gpui::svg().path(crate::icons::path("settings/circle-error")).size(z(20.)).text_color(c(tok(idx::YELLOW_300))),
            Check::Pass => gpui::svg().path(crate::icons::path("circle-check")).size(z(20.)).text_color(c(tok(idx::GREEN_360))),
            Check::Fail => gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(20.)).text_color(c(tok(idx::RED_400))),
        };
        out = out.child(
            div()
                .flex()
                .p(z(16.))
                .child(div().w(z(20.)).flex_none().mr(z(16.)).child(icon))
                .child(div().flex_1().min_w_0().flex().flex_col().child(text(name, 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG)).child(desc)),
        );
        if i + 1 < n {
            out = out.child(div().mx(z(16.)).h(z(1.)).bg(c(tok(idx::BORDER_MUTED))));
        }
    }
    out.into_any_element()
}

fn discoverable_block(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    match view(s) {
        None => text(crate::t!("common.loading"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).into_any_element(),
        Some(View::Disqualified) => checklist(
            s,
            div()
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
        ),
        Some(View::Ineligible) => checklist(
            s,
            div()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(with_marked(crate::t!("gs.access.d_ineligible"), crate::t!("gs.access.d_ineligible_word"), idx::TEXT_FEEDBACK_CRITICAL, FontWeight::BOLD))
                .into_any_element(),
        ),
        Some(v) => discovery_form(app, s, v, cx),
    }
}

pub fn modal_width(viewport: f32) -> f32 {
    let inner = (viewport - 48.).max(0.);
    if inner >= 1080. { (viewport * 0.8).min(1280.) } else { inner.min(1280.) }
}

pub fn preview_fields(d: &Draft, terms_label: &str) -> Vec<Value> {
    if d.join == JoinType::Apply {
        return d.fields.clone();
    }
    vec![json!({ "field_type": "TERMS", "label": terms_label, "values": d.rule_values(), "required": true })]
}

fn field_title(t: SharedString) -> gpui::Div {
    div().mb(z(12.)).overflow_hidden().child(text(t, 16., 20., FontWeight::NORMAL, idx::TEXT_STRONG))
}

fn fake_input(placeholder: SharedString, min_h: f32, counter: Option<&'static str>) -> AnyElement {
    div()
        .relative()
        .min_h(z(min_h))
        .px(z(12.))
        .py(z(10.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .child(text(placeholder, 16., 20., FontWeight::NORMAL, idx::INPUT_PLACEHOLDER_TEXT_DEFAULT))
        .children(counter.map(|n| div().absolute().right(z(10.)).bottom(z(6.)).child(text(n, 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED))))
        .into_any_element()
}

fn preview_field(f: &Value, agree: bool, cx: &mut Cx) -> AnyElement {
    let label: SharedString = f.get("label").and_then(Value::as_str).unwrap_or_default().to_owned().into();
    let strings = |k: &str| f.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect::<Vec<_>>()).unwrap_or_default();
    let body = match FieldKind::of(f) {
        Some(FieldKind::Terms) => {
            let values = strings("values");
            let n = values.len();
            let rows = div().rounded(z(8.)).py(z(10.)).px(z(17.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).flex().flex_col().children(values.into_iter().enumerate().map(|(i, v)| {
                div()
                    .flex()
                    .items_start()
                    .px(z(4.))
                    .pt(z(if i == 0 { 8. } else { 16. }))
                    .pb(z(if i + 1 == n { 8. } else { 16. }))
                    .when(i + 1 < n, |d| d.border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))))
                    .child(text(format!("{}.", i + 1), 16., 20., FontWeight::NORMAL, idx::TEXT_MUTED).flex_none())
                    .child(div().flex_1().min_w_0().pl(z(12.)).child(text(v, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT)))
            }));
            let check = div()
                .id("gs-access-pv-agree")
                .mt(z(16.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .on_click(edit(cx, |s| s.access.agree = !s.access.agree))
                .child(
                    div()
                        .size(z(24.))
                        .flex_none()
                        .rounded(z(6.))
                        .border_1()
                        .border_color(c(tok(if agree { idx::CONTROL_BRAND_FOREGROUND_NEW } else { idx::INTERACTIVE_TEXT_DEFAULT })))
                        .when(agree, |d| d.bg(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(gpui::white()))),
                )
                .child(text(crate::t!("gs.access.pv_agree"), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT));
            return div().mt(z(40.)).p(z(1.)).child(field_title(crate::t!("gs.access.pv_rules"))).child(rows).child(check).into_any_element();
        }
        Some(FieldKind::Paragraph) => fake_input(crate::t!("gs.access.pv_answer_ph"), 88., Some("1000")),
        Some(FieldKind::Choice) => div()
            .flex()
            .flex_col()
            .gap(z(8.))
            .children(strings("choices").into_iter().map(|o| {
                div()
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .px(z(12.))
                    .py(z(10.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                    .child(div().size(z(20.)).flex_none().rounded_full().border_2().border_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                    .child(text(o, 16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
            }))
            .into_any_element(),
        _ => fake_input(crate::t!("gs.access.pv_answer_ph"), 40., None),
    };
    div().mt(z(40.)).p(z(1.)).child(field_title(label)).child(body).into_any_element()
}

fn requirement(level: u64) -> Option<AnyElement> {
    let (icon, label) = match level {
        0 => return None,
        4 => ("settings/mobile-phone", crate::t!("gs.access.pv_phone")),
        _ => ("server/envelope", crate::t!("gs.access.pv_email")),
    };
    Some(
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(z(40.))
                    .mb(z(12.))
                    .flex()
                    .items_center()
                    .p(z(12.))
                    .rounded(z(8.))
                    .border_1()
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                    .child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).mr(z(8.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                    .child(text(label, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).flex_1())
                    .child(
                        div()
                            .id("gs-access-pv-verify")
                            .tooltip(crate::ui::tooltip::tip(crate::t!("gs.access.pv_preview_tip")))
                            .child(kit::btn_sm("gs-access-pv-verify-btn", crate::t!("gs.access.pv_verify"), crate::ui::controls::BtnKind::Primary).opacity(0.5).cursor_not_allowed()),
                    ),
            )
            .child(text(crate::t!("gs.access.pv_footnote"), 12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT).mt(z(8.)))
            .into_any_element(),
    )
}

fn sidebar_card(app: &PhoenixApp, s: &State) -> (AnyElement, gpui::Background) {
    let lower = c(tok(idx::BACKGROUND_BASE_LOWER));
    let lowest = c(tok(idx::BACKGROUND_BASE_LOWEST));
    if let Some(p) = s.get("profile").filter(|p| p.is_object()) {
        let brand = p.get("brand_color_primary").and_then(Value::as_str).and_then(|h| u32::from_str_radix(h.trim_start_matches('#'), 16).ok()).map(|v| gpui::Hsla::from(gpui::rgb(v))).unwrap_or(lowest);
        let bg = gpui::linear_gradient(315., gpui::linear_color_stop(c(tok(idx::BORDER_SUBTLE)), 0.), gpui::linear_color_stop(brand, 1.));
        return (div().w(z(280.)).child(super::profile::preview(s, s.guild.0)).into_any_element(), bg);
    }
    let card = crate::ui::discovery::static_card(&card_guild(app, s), 280., crate::ui::discovery::STATIC_CARD_H).into_any_element();
    (card, gpui::linear_gradient(315., gpui::linear_color_stop(lower, 0.), gpui::linear_color_stop(lowest, 1.)))
}

fn preview_modal(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        with_state(|s| {
            s.access.preview = false;
            s.access.agree = false;
        });
        e.update(cx, |_, cx| cx.notify());
    });
    let fields = s.access.draft.as_ref().map(|d| preview_fields(d, &crate::t!("gs.access.terms_label"))).unwrap_or_default();
    let manual = fields.iter().any(|f| FieldKind::of(f) != Some(FieldKind::Terms));
    let name = app.guilds.iter().find(|g| g.id == s.guild).map(|g| g.name.clone()).unwrap_or_default();
    let level = s.guild_json().get("verification_level").and_then(Value::as_u64).unwrap_or(0);
    let (card, gradient) = sidebar_card(app, s);
    let mut content = div()
        .w_full()
        .max_w(z(640.))
        .mb(z(20.))
        .flex()
        .flex_col()
        .child(text(if manual { crate::t!("gs.access.pv_title", guildName = name) } else { crate::t!("gs.access.pv_title_rules") }, 32., 40., FontWeight::MEDIUM, idx::TEXT_STRONG).my(z(8.)))
        .child(text(if manual { crate::t!("gs.access.pv_desc") } else { crate::t!("gs.access.pv_desc_rules") }, 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT))
        .when(!manual, |d| d.children(requirement(level)));
    for f in &fields {
        content = content.child(preview_field(f, s.access.agree, cx));
    }
    let footer = div()
        .w_full()
        .flex_none()
        .flex()
        .flex_row_reverse()
        .items_center()
        .justify_between()
        .gap(z(16.))
        .py(z(8.))
        .child(div().flex_none().ml(z(12.)).child(crate::ui::controls::button("gs-access-pv-submit", crate::t!("gs.access.pv_submit"), crate::ui::controls::BtnKind::Primary).opacity(0.5).cursor_not_allowed()))
        .when(manual, |d| {
            d.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(text(crate::t!("gs.access.pv_footer1"), 12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT))
                    .child(text(crate::t!("gs.access.pv_footer2"), 12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT)),
            )
        });
    let close2 = close.clone();
    let body = div()
        .relative()
        .w(z(modal_width(if s.access.viewport > 0. { s.access.viewport } else { 1280. })))
        .min_h(z(400.))
        .max_h(z(if s.access.viewport_h > 0. { s.access.viewport_h * 0.95 } else { 760. }))
        .flex()
        .rounded(z(global::RADIUS_MD))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .shadow(crate::ui::inbox::shadow_high())
        .child(
            div()
                .relative()
                .flex_none()
                .w(gpui::relative(0.33))
                .min_w(z(400.))
                .my(z(8.))
                .mx(z(16.))
                .p(z(12.))
                .rounded(z(8.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().absolute().top_0().left_0().size_full().rounded(z(8.)).opacity(0.33).bg(gradient))
                .child(card),
        )
        .child(
            div().flex_1().min_w_0().flex().flex_col().items_center().child(
                div()
                    .w_full()
                    .max_w(z(720.))
                    .flex_1()
                    .min_h_0()
                    .pt(z(60.))
                    .px(z(24.))
                    .pb(z(36.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(div().id("gs-access-pv-scroll").w_full().flex_1().min_h_0().overflow_y_scroll().flex().flex_col().items_center().child(content))
                    .child(footer),
            ),
        )
        .child(
            div()
                .id("gs-access-pv-close")
                .absolute()
                .top(z(32.))
                .right(z(32.))
                .size(z(24.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(move |_, w, cx| close2(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(24.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        );
    crate::ui::modal::frame("gs-access-preview-backdrop", close, body)
}

fn choice_rows(s: &State, q: &Question, cx: &mut Cx) -> gpui::Div {
    let drag = s.access.drag.filter(|g| g.list == DragList::Choices);
    let mut list = div().flex().flex_col();
    for (i, ch) in q.choices.iter().enumerate() {
        let k = format!("q:choice:{}", ch.id);
        let group: SharedString = format!("gs-access-choice-{}", ch.id).into();
        let input = match s.access.inputs.get(&k) {
            Some(inp) => crate::ui::modals::input_box(inp).pr(z(32.)).into_any_element(),
            None => div().h(z(40.)).into_any_element(),
        };
        let row = div()
            .id(("gs-access-choice", ch.id as usize))
            .relative()
            .mr(z(-8.))
            .flex()
            .items_center()
            .when(drag.is_some_and(|g| g.over == i), |d| d.opacity(0.6))
            .child(div().flex().flex_none().mb(z(12.)).pr(z(10.)).child(div().size(z(20.)).rounded_full().border_2().border_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))))
            .child(
                div().group(group.clone()).flex_1().min_w_0().mb(z(12.)).relative().child(input).child(
                    div()
                        .id(("gs-access-choice-clear", ch.id as usize))
                        .absolute()
                        .top(z(6.))
                        .right(z(8.))
                        .size(z(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.)
                        .group_hover(group.clone(), |d| d.opacity(1.))
                        .cursor_pointer()
                        .tooltip(crate::ui::tooltip::tip(crate::t!("gs.access.clear")))
                        .child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                        .on_click(edit(cx, move |s| {
                            if let Some(q) = s.access.question.as_mut() {
                                q.error = None;
                                q.remove_choice(i);
                            }
                        })),
                ),
            )
            .child(div().w(z(32.)).mb(z(12.)).flex_none().flex().items_center().justify_center().child(drag_handle(("gs-access-choice-drag", ch.id as usize), DragList::Choices, i, cx)));
        list = list.child(drag_row(DragList::Choices, i, cx)(row));
    }
    if q.choices.len() < MAX_CHOICES {
        list = list.child(
            div().mt(z(8.)).flex().child(
                div()
                    .id("gs-access-add-choice")
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .cursor_pointer()
                    .child(gpui::svg().path(crate::icons::path("plus-circle")).size(z(17.)).text_color(c(tok(idx::BLUE_345))))
                    .child(text(crate::t!("gs.access.add_option"), 16., 20., FontWeight::NORMAL, idx::TEXT_LINK))
                    .on_click(edit(cx, |s| {
                        if let Some(q) = s.access.question.as_mut() {
                            q.error = None;
                            if let Some(id) = q.add_choice() {
                                s.access.focus = Some(format!("q:choice:{id}"));
                            }
                        }
                    })),
            ),
        );
    }
    list
}

pub fn modal(app: &PhoenixApp, s: &State, cx: &mut Cx) -> Option<AnyElement> {
    if s.access.preview {
        return Some(preview_modal(app, s, cx));
    }
    let q = s.access.question.as_ref()?;
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        with_state(close_question);
        e.update(cx, |_, cx| cx.notify());
    });
    let label_input = || match s.access.inputs.get("q:label") {
        Some(i) => crate::ui::modals::input_box(i).into_any_element(),
        None => div().h(z(40.)).into_any_element(),
    };
    let (title, subtitle, mut body) = match q.kind {
        FieldKind::Text => (crate::t!("gs.access.short"), Some(crate::t!("gs.access.text_desc")), div().flex().flex_col().child(label_input())),
        FieldKind::Paragraph => (crate::t!("gs.access.paragraph"), Some(crate::t!("gs.access.paragraph_desc")), div().flex().flex_col().child(label_input())),
        FieldKind::Choice => (
            crate::t!("gs.access.choice"),
            None,
            div().flex().flex_col().child(div().mb(z(16.)).child(label_input())).child(div().mt(z(8.)).mb(z(24.)).h(z(1.)).w_full().bg(c(tok(idx::BORDER_SUBTLE)))).child(choice_rows(s, q, cx)),
        ),
        FieldKind::Terms => (crate::t!("gs.access.terms_kind"), None, div().flex().flex_col().child(rules_editor(s, false, true, cx))),
    };
    if let Some(err) = q.error.clone() {
        body = body.child(text(err, 12., 16., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.)));
    }
    let close2 = close.clone();
    let footer = div()
        .w_full()
        .flex()
        .justify_end()
        .gap(z(8.))
        .child(crate::ui::controls::button("gs-access-q-cancel", crate::t!("common.cancel"), crate::ui::controls::BtnKind::Secondary).on_click(move |_, w, cx| close2(w, cx)))
        .child(crate::ui::controls::button("gs-access-q-save", crate::t!("common.save"), crate::ui::controls::BtnKind::Primary).on_click(edit(cx, save_question)));
    Some(crate::ui::modals::card_with(440., title, subtitle, body.into_any_element(), Some(footer.into_any_element()), close))
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

    #[test]
    fn rows_move_while_hovering_past_the_middle() {
        let rule = Drag { list: DragList::Rules, origin: 0, from: 0, over: 0 };
        assert_eq!(hover_target(&rule, 1, 10., 44.), None);
        assert_eq!(hover_target(&rule, 1, 30., 44.), Some(1));
        let moved = Drag { from: 2, over: 2, ..rule };
        assert_eq!(hover_target(&moved, 2, 30., 44.), None);
        let up = Drag { list: DragList::Rules, origin: 3, from: 3, over: 3 };
        assert_eq!(hover_target(&up, 2, 30., 44.), None);
        assert_eq!(hover_target(&up, 2, 10., 44.), Some(2));
        let field = Drag { list: DragList::Fields, origin: 2, from: 2, over: 2 };
        assert_eq!(hover_target(&field, 1, 10., 52.), None);
        assert_eq!(hover_target(&field, 1, 40., 52.), Some(1));
        assert_eq!(hover_target(&Drag { from: 1, ..field }, 2, 40., 52.), None);
        let mut v = vec!["a", "b", "c"];
        let mut d = Drag { list: DragList::Choices, origin: 0, from: 0, over: 0 };
        for (over, y) in [(1, 30.), (2, 30.)] {
            if let Some(to) = hover_target(&d, over, y, 44.) {
                move_item(&mut v, d.from, to);
                d = Drag { from: to, over: to, ..d };
            }
        }
        assert_eq!(v, ["b", "c", "a"]);
    }

    #[test]
    fn categories_follow_the_client_store() {
        let v = json!([
            { "id": 0, "name": "Zero", "is_primary": true },
            { "id": 49, "name": "Bots", "is_primary": true },
            { "id": 13, "name": "Autre", "is_primary": true },
            { "id": 3, "name": "Anime", "is_primary": false },
            { "id": "14", "name": { "default": "General", "localizations": { "fr": "Discussion" } }, "is_primary": true }
        ]);
        let (primary, all) = categories(&v, "fr");
        assert_eq!(primary.iter().map(|c| c.id).collect::<Vec<_>>(), [49, 14, 13]);
        assert_eq!(all.iter().map(|c| c.id).collect::<Vec<_>>(), [3, 49, 14]);
        assert_eq!(all[2].name, "Discussion");
        assert_eq!(primary_category(&json!({ "primary_category_id": "49" })), 49);
        assert_eq!(subcategories(&json!({ "category_ids": [3, "14"] })), [3, 14]);
    }

    #[test]
    fn popular_tags_skip_the_ones_already_added() {
        let tags = popular_tags(49, &["bots".to_owned()], |k| k.replace('_', " "));
        assert_eq!(tags, ["apps", "discord bot", "giveaways", "support", "fun", "memes"]);
        assert!(popular_tags(999, &[], str::to_owned).is_empty());
        assert!(POPULAR_TAGS.iter().all(|(_, t)| t.len() <= 10));
    }

    #[test]
    fn subcategories_are_written_one_by_one() {
        let add = toggle_subcategory(7, &[1, 2], 3).unwrap();
        assert_eq!((add.method.as_str(), add.path.as_str()), ("PUT", "/guilds/7/discovery-categories/3"));
        let del = toggle_subcategory(7, &[1, 2], 2).unwrap();
        assert_eq!(del.method.as_str(), "DELETE");
        assert!(toggle_subcategory(7, &[1, 2, 3, 4, 5], 6).is_none());
        assert!(toggle_subcategory(7, &[1, 2, 3, 4, 5], 5).is_some());
        assert!(add.refusal().is_write_blocked());
    }

    #[test]
    fn primary_category_goes_into_the_metadata_patch() {
        let g = json!({ "features": ["COMMUNITY", "DISCOVERABLE"], "description": "d" });
        let meta = json!({ "guild_id": "7", "primary_category_id": 49, "category_ids": [3], "keywords": ["a"], "emoji_discoverability_enabled": true, "about": null });
        let mut d = Draft::new(&g, &Value::Null, JoinType::Discoverable, "q");
        let ctx = Ctx { guild: &g, form: &Value::Null, profile: &Value::Null, metadata: &meta, splash: None };
        assert!(!dirty(&ctx, &d).guild);
        d.primary = Some(49);
        assert!(!dirty(&ctx, &d).guild);
        d.primary = Some(14);
        assert!(dirty(&ctx, &d).guild);
        let body = metadata_body(&meta, &d);
        assert_eq!(body, json!({ "primary_category_id": 14, "keywords": ["a"], "emoji_discoverability_enabled": true, "about": null }));
        let w = writes(7, &ctx, &d, "Read");
        assert!(w.iter().any(|w| w.path == "/guilds/7/discovery-metadata" && w.body.as_ref() == Some(&body)));
    }

    #[test]
    fn checklist_reads_the_requirements() {
        let req = json!({ "safe_environment": true, "size": false, "age": true, "healthy": false, "health_score_pending": true, "protected": false, "nsfw_properties": {}, "minimum_age": 56, "minimum_size": 1000 });
        let states: Vec<(&str, Check)> = check_items(&req).iter().map(|c| (c.key, c.state)).collect();
        assert_eq!(states, [("safe", Check::Pass), ("size", Check::Fail), ("age", Check::Pass), ("health", Check::Pending), ("nsfw", Check::Pass), ("2fa", Check::Fail)]);
        assert!(!nsfw_clean(&json!({ "nsfw_properties": { "name": ["x"] } })));
        assert!(nsfw_clean(&json!({ "nsfw_properties": null })));
        assert_eq!(minimum_weeks(&req), 8);
        assert_eq!(minimum_weeks(&json!({ "minimum_age": 50 })), 8);
        let created = 1_420_070_400_000i64 + 1_000;
        assert_eq!(pass_date_ms(1_000 << 22, &req), created + 56 * 86_400_000);
    }

    #[test]
    fn preview_shows_the_pending_form() {
        let g = json!({ "features": [] });
        let mut d = Draft::new(&g, &Value::Null, JoinType::Invite, "q");
        d.rules[0].value = " Be kind ".into();
        d.add_rule(None);
        assert_eq!(preview_fields(&d, "Read"), [json!({ "field_type": "TERMS", "label": "Read", "values": ["Be kind"], "required": true })]);
        let apply = Draft::new(&g, &terms_form(&["r"]), JoinType::Apply, "Why?");
        assert_eq!(preview_fields(&apply, "Read").len(), 2);
        assert_eq!(modal_width(1920.), 1280.);
        assert_eq!(modal_width(1400.), 1120.);
        assert_eq!(modal_width(1000.), 952.);
    }

    #[test]
    fn rules_modal_uses_the_rules_editor_limits() {
        let mut q = Question::new(FieldKind::Terms, None, json!({ "values": ["a"] }));
        assert_eq!(add_rule_to(&mut q.choices, &mut q.next, Some("Be nice")), Some(2));
        let mut e = Question::new(FieldKind::Terms, None, Value::Null);
        assert_eq!(add_rule_to(&mut e.choices, &mut e.next, Some("Be nice")), Some(1));
        assert_eq!(e.choices.len(), 1);
        remove_rule_from(&mut e.choices, &mut e.next, 0);
        assert_eq!(e.choices.len(), 1);
        assert!(e.choices[0].value.is_empty());
        assert_eq!(e.build("Read"), Err("gs.access.need_rule"));
    }
}
