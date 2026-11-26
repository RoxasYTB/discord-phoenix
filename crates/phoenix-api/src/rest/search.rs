use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use url::form_urlencoded::Serializer;

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, GuildId, Message, Snowflake, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HasFilter {
    Link,
    Embed,
    File,
    Video,
    Image,
    Sound,
    Sticker,
    Poll,
    Snapshot,
}

impl HasFilter {
    pub const ALL: [HasFilter; 9] = [
        Self::Link,
        Self::Embed,
        Self::File,
        Self::Video,
        Self::Image,
        Self::Sound,
        Self::Sticker,
        Self::Poll,
        Self::Snapshot,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Embed => "embed",
            Self::File => "file",
            Self::Video => "video",
            Self::Image => "image",
            Self::Sound => "sound",
            Self::Sticker => "sticker",
            Self::Poll => "poll",
            Self::Snapshot => "snapshot",
        }
    }

    pub fn english(self) -> &'static str {
        match self {
            Self::Snapshot => "forward",
            h => h.as_str(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.as_str().eq_ignore_ascii_case(s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchSort {
    #[default]
    Newest,
    Oldest,
    Relevance,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchQuery {
    pub content: Option<String>,
    pub author_ids: Vec<UserId>,
    pub mentions: Vec<UserId>,
    pub channel_ids: Vec<ChannelId>,
    pub has: Vec<HasFilter>,
    pub min_id: Option<Snowflake>,
    pub max_id: Option<Snowflake>,
    pub pinned: Option<bool>,
    pub author_types: Vec<String>,
    pub attachment_extensions: Vec<String>,
    pub attachment_filenames: Vec<String>,
    pub link_hostnames: Vec<String>,
    pub sort: SearchSort,
    pub offset: u32,
    pub include_nsfw: bool,
}

impl SearchQuery {
    pub fn text(content: impl Into<String>) -> Self {
        Self { content: Some(content.into()), ..Default::default() }
    }

    pub fn is_empty(&self) -> bool {
        self.content.as_deref().is_none_or(|c| c.trim().is_empty())
            && self.author_ids.is_empty()
            && self.mentions.is_empty()
            && self.channel_ids.is_empty()
            && self.has.is_empty()
            && self.min_id.is_none()
            && self.max_id.is_none()
            && self.pinned.is_none()
            && self.author_types.is_empty()
            && self.attachment_extensions.is_empty()
            && self.attachment_filenames.is_empty()
            && self.link_hostnames.is_empty()
    }

    pub fn to_query(&self) -> String {
        let mut s = Serializer::new(String::new());
        if let Some(c) = self.content.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            s.append_pair("content", c);
        }
        for a in &self.author_ids {
            s.append_pair("author_id", &a.to_string());
        }
        for m in &self.mentions {
            s.append_pair("mentions", &m.to_string());
        }
        for c in &self.channel_ids {
            s.append_pair("channel_id", &c.to_string());
        }
        for h in &self.has {
            s.append_pair("has", h.as_str());
        }
        if let Some(v) = self.min_id {
            s.append_pair("min_id", &v.to_string());
        }
        if let Some(v) = self.max_id {
            s.append_pair("max_id", &v.to_string());
        }
        if let Some(p) = self.pinned {
            s.append_pair("pinned", if p { "true" } else { "false" });
        }
        for t in &self.author_types {
            s.append_pair("author_type", t);
        }
        for e in &self.attachment_extensions {
            s.append_pair("attachment_extension", e);
        }
        for f in &self.attachment_filenames {
            s.append_pair("attachment_filename", f);
        }
        for h in &self.link_hostnames {
            s.append_pair("link_hostname", h);
        }
        match self.sort {
            SearchSort::Newest => {
                s.append_pair("sort_by", "timestamp");
                s.append_pair("sort_order", "desc");
            }
            SearchSort::Oldest => {
                s.append_pair("sort_by", "timestamp");
                s.append_pair("sort_order", "asc");
            }
            SearchSort::Relevance => {
                s.append_pair("sort_by", "relevance");
                s.append_pair("sort_order", "desc");
            }
        }
        if self.offset > 0 {
            s.append_pair("offset", &self.offset.to_string());
        }
        if self.include_nsfw {
            s.append_pair("include_nsfw", "true");
        }
        s.finish()
    }
}

impl SearchQuery {
    pub fn to_tabs_body(&self, limit: u32) -> serde_json::Value {
        use serde_json::{Map, Value, json};
        let ids = |v: &[Snowflake]| Value::Array(v.iter().map(|x| Value::String(x.to_string())).collect());
        let (by, order) = match self.sort {
            SearchSort::Newest => ("timestamp", "desc"),
            SearchSort::Oldest => ("timestamp", "asc"),
            SearchSort::Relevance => ("relevance", "desc"),
        };
        let mut tab = Map::new();
        tab.insert("sort_by".into(), json!(by));
        tab.insert("sort_order".into(), json!(order));
        if let Some(c) = self.content.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            tab.insert("content".into(), json!(c));
        }
        if !self.author_ids.is_empty() {
            tab.insert("author_id".into(), ids(&self.author_ids));
        }
        if !self.mentions.is_empty() {
            tab.insert("mentions".into(), ids(&self.mentions));
        }
        if !self.has.is_empty() {
            tab.insert("has".into(), Value::Array(self.has.iter().map(|h| json!(h.as_str())).collect()));
        }
        if let Some(v) = self.min_id {
            tab.insert("min_id".into(), json!(v.to_string()));
        }
        if let Some(v) = self.max_id {
            tab.insert("max_id".into(), json!(v.to_string()));
        }
        if let Some(p) = self.pinned {
            tab.insert("pinned".into(), json!(p));
        }
        if !self.author_types.is_empty() {
            tab.insert("author_type".into(), json!(self.author_types));
        }
        if !self.attachment_extensions.is_empty() {
            tab.insert("attachment_extension".into(), json!(self.attachment_extensions));
        }
        if !self.attachment_filenames.is_empty() {
            tab.insert("attachment_filename".into(), json!(self.attachment_filenames));
        }
        if !self.link_hostnames.is_empty() {
            tab.insert("link_hostname".into(), json!(self.link_hostnames));
        }
        tab.insert("offset".into(), json!(self.offset));
        tab.insert("limit".into(), json!(limit));
        let mut body = Map::new();
        body.insert("include_nsfw".into(), json!(self.include_nsfw));
        if !self.channel_ids.is_empty() {
            body.insert("channel_ids".into(), ids(&self.channel_ids));
        }
        body.insert("tabs".into(), json!({ "messages": Value::Object(tab) }));
        body.insert("track_exact_total_hits".into(), json!(true));
        Value::Object(body)
    }
}

pub fn parse_tabs_reply(status: StatusCode, bytes: &[u8]) -> Result<SearchOutcome> {
    if status == StatusCode::ACCEPTED {
        return parse_search_reply(status, bytes);
    }
    let v: serde_json::Value = serde_json::from_slice(bytes)?;
    let tab = v.get("tabs").and_then(|t| t.get("messages")).cloned().unwrap_or_default();
    let total_results = tab.get("total_results").and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
    let mut messages = Vec::new();
    for m in tab.get("messages").and_then(serde_json::Value::as_array).into_iter().flatten() {
        let group = if m.is_array() { serde_json::from_value::<Vec<Message>>(m.clone()) } else { serde_json::from_value::<Message>(m.clone()).map(|x| vec![x]) };
        if let Ok(g) = group {
            messages.push(g);
        }
    }
    Ok(SearchOutcome::Results(SearchResponse { total_results, messages, doing_deep_historical_index: v.get("doing_deep_historical_index").and_then(serde_json::Value::as_bool).unwrap_or(false) }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterKey {
    From,
    Mentions,
    Has,
    Before,
    After,
    On,
    In,
    Pinned,
    AuthorType,
    LinkFrom,
    FileType,
    FileName,
}

impl FilterKey {
    pub const ALL: [FilterKey; 12] = [
        Self::From,
        Self::Mentions,
        Self::Has,
        Self::LinkFrom,
        Self::FileType,
        Self::FileName,
        Self::Before,
        Self::After,
        Self::On,
        Self::In,
        Self::Pinned,
        Self::AuthorType,
    ];

    pub fn english(self) -> &'static str {
        match self {
            Self::From => "from",
            Self::Mentions => "mentions",
            Self::Has => "has",
            Self::Before => "before",
            Self::After => "after",
            Self::On => "during",
            Self::In => "in",
            Self::Pinned => "pinned",
            Self::AuthorType => "authorType",
            Self::LinkFrom => "linkFrom",
            Self::FileType => "fileType",
            Self::FileName => "fileName",
        }
    }

    pub fn is_date(self) -> bool {
        matches!(self, Self::Before | Self::After | Self::On)
    }
}

pub const AUTHOR_TYPES: [&str; 3] = ["user", "bot", "webhook"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchKeys {
    pub keys: Vec<(FilterKey, String)>,
    pub has: Vec<(String, HasFilter)>,
    pub author_types: Vec<(String, &'static str)>,
}

impl Default for SearchKeys {
    fn default() -> Self {
        Self::english()
    }
}

impl SearchKeys {
    pub fn english() -> Self {
        let mut keys: Vec<(FilterKey, String)> = FilterKey::ALL.iter().map(|k| (*k, k.english().to_owned())).collect();
        keys.push((FilterKey::On, "on".into()));
        keys.push((FilterKey::AuthorType, "author_type".into()));
        Self {
            keys,
            has: HasFilter::ALL.iter().map(|h| (h.english().to_owned(), *h)).collect(),
            author_types: AUTHOR_TYPES.iter().map(|t| ((*t).to_owned(), *t)).collect(),
        }
    }

    pub fn localized(keys: Vec<(FilterKey, String)>, has: Vec<(String, HasFilter)>, author_types: Vec<(String, &'static str)>) -> Self {
        let en = Self::english();
        let mut out = Self { keys, has, author_types };
        out.keys.extend(en.keys);
        out.has.extend(en.has);
        out.author_types.extend(en.author_types);
        out
    }

    pub fn key(&self, word: &str) -> Option<FilterKey> {
        self.keys.iter().find(|(_, w)| w.to_lowercase() == word.to_lowercase()).map(|(k, _)| *k)
    }

    pub fn label(&self, key: FilterKey) -> &str {
        self.keys.iter().find(|(k, _)| *k == key).map_or(key.english(), |(_, w)| w.as_str())
    }

    pub fn has_value(&self, word: &str) -> Option<HasFilter> {
        let w = word.to_lowercase();
        self.has.iter().find(|(n, _)| n.to_lowercase() == w).map(|(_, h)| *h).or_else(|| HasFilter::parse(&w))
    }

    pub fn has_label(&self, h: HasFilter) -> &str {
        self.has.iter().find(|(_, x)| *x == h).map_or(h.english(), |(n, _)| n.as_str())
    }

    pub fn author_type(&self, word: &str) -> Option<String> {
        let (neg, w) = match word.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, word),
        };
        let w = w.to_lowercase();
        let t = self.author_types.iter().find(|(n, _)| n.to_lowercase() == w).map(|(_, t)| *t)?;
        Some(if neg { format!("-{t}") } else { t.to_owned() })
    }

    pub fn author_label(&self, t: &str) -> &str {
        self.author_types.iter().find(|(_, x)| *x == t).map_or("", |(n, _)| n.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedSearch {
    pub content: String,
    pub from: Vec<String>,
    pub mentions: Vec<String>,
    pub in_channels: Vec<String>,
    pub has: Vec<HasFilter>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub during: Option<String>,
    pub pinned: Option<bool>,
    pub author_types: Vec<String>,
    pub link_from: Vec<String>,
    pub file_types: Vec<String>,
    pub file_names: Vec<String>,
}

fn push_unique(list: &mut Vec<String>, v: String) {
    if !list.contains(&v) {
        list.push(v);
    }
}

pub fn parse_search(input: &str) -> ParsedSearch {
    parse_search_with(input, &SearchKeys::english())
}

pub fn parse_search_with(input: &str, keys: &SearchKeys) -> ParsedSearch {
    let mut out = ParsedSearch::default();
    let mut words = Vec::new();
    for tok in tokenize(input) {
        let Some((key, val)) = tok.split_once(':') else {
            words.push(tok);
            continue;
        };
        let val = val.trim_matches('"').to_string();
        let handled = match keys.key(key) {
            _ if val.is_empty() => false,
            Some(FilterKey::From) => {
                out.from.push(val.trim_start_matches('@').to_string());
                true
            }
            Some(FilterKey::Mentions) => {
                out.mentions.push(val.trim_start_matches('@').to_string());
                true
            }
            Some(FilterKey::In) => {
                out.in_channels.push(val.trim_start_matches('#').to_string());
                true
            }
            Some(FilterKey::Has) => match keys.has_value(&val) {
                Some(h) => {
                    if !out.has.contains(&h) {
                        out.has.push(h);
                    }
                    true
                }
                None => false,
            },
            Some(FilterKey::Before) => {
                out.before = Some(val);
                true
            }
            Some(FilterKey::After) => {
                out.after = Some(val);
                true
            }
            Some(FilterKey::On) => {
                out.during = Some(val);
                true
            }
            Some(FilterKey::Pinned) => match val.to_ascii_lowercase().as_str() {
                "true" => {
                    out.pinned = Some(true);
                    true
                }
                "false" => {
                    out.pinned = Some(false);
                    true
                }
                _ => false,
            },
            Some(FilterKey::LinkFrom) => {
                push_unique(&mut out.link_from, val);
                true
            }
            Some(FilterKey::FileType) => {
                push_unique(&mut out.file_types, val);
                true
            }
            Some(FilterKey::FileName) => {
                push_unique(&mut out.file_names, val);
                true
            }
            Some(FilterKey::AuthorType) => match keys.author_type(&val) {
                Some(v) => {
                    if !out.author_types.contains(&v) {
                        out.author_types.push(v);
                    }
                    true
                }
                None => false,
            },
            None => false,
        };
        if !handled {
            words.push(tok);
        }
    }
    out.content = words.join(" ");
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFilter {
    pub key: FilterKey,
    pub start: usize,
    pub partial: String,
}

pub fn pending_filter(input: &str, keys: &SearchKeys) -> Option<PendingFilter> {
    let mut quoted = false;
    let mut start = 0;
    for (i, ch) in input.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        } else if ch.is_whitespace() && !quoted {
            start = i + ch.len_utf8();
        }
    }
    let last = &input[start..];
    let (k, v) = last.split_once(':')?;
    let key = keys.key(k)?;
    Some(PendingFilter { key, start: start + k.len() + 1, partial: v.trim_start_matches('"').to_owned() })
}

pub fn complete_filter(input: &str, pending: &PendingFilter, value: &str) -> String {
    let v = if value.contains(char::is_whitespace) { format!("\"{value}\"") } else { value.to_owned() };
    format!("{}{v} ", &input[..pending.start])
}

fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                cur.push(ch);
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn date_to_snowflake(date: &str) -> Option<Snowflake> {
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let ms = days_from_civil(y, m, d) * 86_400_000;
    let rel = ms - DISCORD_EPOCH_MS;
    (rel >= 0).then_some(Snowflake((rel as u64) << 22))
}

const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub total_results: u32,
    #[serde(default)]
    pub messages: Vec<Vec<Message>>,
    #[serde(default)]
    pub doing_deep_historical_index: bool,
}

impl SearchResponse {
    pub fn hits(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter_map(|group| group.first())
    }
}

#[derive(Debug, Clone)]
pub enum SearchOutcome {
    Results(SearchResponse),
    Indexing { retry_after: f64 },
}

fn parse_search_reply(status: StatusCode, bytes: &[u8]) -> Result<SearchOutcome> {
    if status == StatusCode::ACCEPTED {
        #[derive(Deserialize)]
        struct Pending {
            #[serde(default)]
            retry_after: Option<f64>,
        }
        let p: Option<Pending> = serde_json::from_slice(bytes).ok();
        let retry_after = p.and_then(|p| p.retry_after).filter(|r| *r > 0.).unwrap_or(5.0).clamp(0.5, 60.0);
        return Ok(SearchOutcome::Indexing { retry_after });
    }
    Ok(SearchOutcome::Results(serde_json::from_slice(bytes)?))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Gif {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub src: String,
    #[serde(default)]
    pub gif_src: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GifCategory {
    pub name: String,
    #[serde(default)]
    pub src: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct GifTrending {
    #[serde(default)]
    pub categories: Vec<GifCategory>,
    #[serde(default)]
    pub gifs: Vec<Gif>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GifFormat {
    #[default]
    Mp4,
    Gif,
    TinyGif,
    Webm,
}

impl GifFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Gif => "gif",
            Self::TinyGif => "tinygif",
            Self::Webm => "webm",
        }
    }
}

fn gif_query(extra: &[(&str, &str)], locale: &str, fmt: GifFormat) -> String {
    let mut s = Serializer::new(String::new());
    for (k, v) in extra {
        s.append_pair(k, v);
    }
    s.append_pair("media_format", fmt.as_str());
    s.append_pair("locale", locale);
    s.finish()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sticker {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub format_type: u8,
    #[serde(default)]
    pub pack_id: Option<Snowflake>,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default = "yes")]
    pub available: bool,
}

fn yes() -> bool {
    true
}

impl Sticker {
    pub fn url(&self) -> String {
        let ext = match self.format_type {
            3 => "json",
            4 => "gif",
            _ => "png",
        };
        format!("{}/stickers/{}.{ext}", crate::models::CDN_BASE, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct StickerPack {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub stickers: Vec<Sticker>,
    #[serde(default)]
    pub cover_sticker_id: Option<Snowflake>,
}

#[derive(Deserialize)]
struct StickerPacks {
    #[serde(default)]
    sticker_packs: Vec<StickerPack>,
}

impl RestClient {
    pub async fn search_guild_messages(&self, guild: GuildId, q: &SearchQuery) -> Result<SearchOutcome> {
        let path = format!("/guilds/{guild}/messages/search?{}", q.to_query());
        let (status, bytes) = self.raw(Method::GET, &path, Body::None).await?;
        parse_search_reply(status, &bytes)
    }

    pub async fn search_channel_messages(&self, channel: ChannelId, q: &SearchQuery) -> Result<SearchOutcome> {
        let path = format!("/channels/{channel}/messages/search?{}", q.to_query());
        let (status, bytes) = self.raw(Method::GET, &path, Body::None).await?;
        parse_search_reply(status, &bytes)
    }

    pub async fn search_dms_tabs(&self, q: &SearchQuery, limit: u32) -> Result<SearchOutcome> {
        let (status, bytes) = self.raw(Method::POST, "/users/@me/messages/search/tabs", Body::Json(q.to_tabs_body(limit))).await?;
        parse_tabs_reply(status, &bytes)
    }

    pub async fn gif_trending(&self, locale: &str, fmt: GifFormat) -> Result<GifTrending> {
        let path = format!("/gifs/trending?{}", gif_query(&[], locale, fmt));
        self.call(Method::GET, &path, Body::None).await
    }

    pub async fn gif_trending_gifs(&self, locale: &str, fmt: GifFormat) -> Result<Vec<Gif>> {
        let path = format!("/gifs/trending-gifs?{}", gif_query(&[], locale, fmt));
        self.call(Method::GET, &path, Body::None).await
    }

    pub async fn gif_search(&self, query: &str, locale: &str, fmt: GifFormat) -> Result<Vec<Gif>> {
        let path = format!("/gifs/search?{}", gif_query(&[("q", query)], locale, fmt));
        self.call(Method::GET, &path, Body::None).await
    }

    pub async fn gif_suggest(&self, query: &str, limit: u8, locale: &str) -> Result<Vec<String>> {
        let mut s = Serializer::new(String::new());
        s.append_pair("q", query);
        s.append_pair("limit", &limit.clamp(1, 20).to_string());
        s.append_pair("locale", locale);
        let path = format!("/gifs/suggest?{}", s.finish());
        self.call(Method::GET, &path, Body::None).await
    }

    pub async fn get_sticker_packs(&self, locale: &str) -> Result<Vec<StickerPack>> {
        let path = format!("/sticker-packs?locale={}", url::form_urlencoded::byte_serialize(locale.as_bytes()).collect::<String>());
        let r: StickerPacks = self.call(Method::GET, &path, Body::None).await?;
        Ok(r.sticker_packs)
    }

    pub async fn get_guild_stickers(&self, guild: GuildId) -> Result<Vec<Sticker>> {
        self.call(Method::GET, &format!("/guilds/{guild}/stickers"), Body::None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filters() {
        let p = parse_search(r#"hello from:bob in:#general has:image has:link world from:"Big Al""#);
        assert_eq!(p.content, "hello world");
        assert_eq!(p.from, vec!["bob", "Big Al"]);
        assert_eq!(p.in_channels, vec!["general"]);
        assert_eq!(p.has, vec![HasFilter::Image, HasFilter::Link]);
    }

    #[test]
    fn unknown_keys_stay_in_content() {
        let p = parse_search("see https://x.y has:nope from:");
        assert_eq!(p.content, "see https://x.y has:nope from:");
        assert!(p.from.is_empty() && p.has.is_empty());
    }

    #[test]
    fn dates_and_pinned() {
        let p = parse_search("before:2024-01-02 after:2023-05-06 pinned:true");
        assert_eq!(p.before.as_deref(), Some("2024-01-02"));
        assert_eq!(p.after.as_deref(), Some("2023-05-06"));
        assert_eq!(p.pinned, Some(true));
        assert!(p.content.is_empty());
    }

    fn french() -> SearchKeys {
        let k = |key, w: &str| (key, w.to_owned());
        SearchKeys::localized(
            vec![
                k(FilterKey::From, "de"),
                k(FilterKey::Mentions, "mentions"),
                k(FilterKey::Has, "contient"),
                k(FilterKey::Before, "avant"),
                k(FilterKey::After, "après"),
                k(FilterKey::On, "pendant"),
                k(FilterKey::On, "le"),
                k(FilterKey::In, "dans"),
                k(FilterKey::Pinned, "épinglé"),
                k(FilterKey::AuthorType, "auteur"),
            ],
            vec![("lien".into(), HasFilter::Link), ("transférer".into(), HasFilter::Snapshot), ("vidéo".into(), HasFilter::Video)],
            vec![("utilisateur".into(), "user"), ("bot".into(), "bot")],
        )
    }

    #[test]
    fn french_keys_and_values() {
        let fr = french();
        let p = parse_search_with(r#"salut de:ninette dans:"Coin Café" contient:lien contient:transférer Épinglé:true avant:2026-01-02 le:hier auteur:-utilisateur"#, &fr);
        assert_eq!(p.content, "salut");
        assert_eq!(p.from, vec!["ninette"]);
        assert_eq!(p.in_channels, vec!["Coin Café"]);
        assert_eq!(p.has, vec![HasFilter::Link, HasFilter::Snapshot]);
        assert_eq!(p.pinned, Some(true));
        assert_eq!(p.before.as_deref(), Some("2026-01-02"));
        assert_eq!(p.during.as_deref(), Some("hier"));
        assert_eq!(p.author_types, vec!["-user"]);
        let en = parse_search_with("from:bob has:link has:forward pinned:maybe", &fr);
        assert_eq!(en.from, vec!["bob"]);
        assert_eq!(en.has, vec![HasFilter::Link, HasFilter::Snapshot]);
        assert_eq!(en.pinned, None);
        assert_eq!(en.content, "pinned:maybe");
        assert_eq!(fr.label(FilterKey::From), "de");
        assert_eq!(fr.has_label(HasFilter::Snapshot), "transférer");
        assert_eq!(SearchKeys::english().has_label(HasFilter::Snapshot), "forward");
        assert_eq!(HasFilter::parse("snapshot"), Some(HasFilter::Snapshot));
    }

    #[test]
    fn link_and_file_filters() {
        let p = parse_search("logs linkFrom:github.com fileType:png filetype:png fileName:crash.txt fileType:");
        assert_eq!(p.content, "logs fileType:");
        assert_eq!(p.link_from, vec!["github.com"]);
        assert_eq!(p.file_types, vec!["png"]);
        assert_eq!(p.file_names, vec!["crash.txt"]);
        let fr = SearchKeys::localized(
            vec![(FilterKey::LinkFrom, "lienDe".into()), (FilterKey::FileType, "typeFichier".into()), (FilterKey::FileName, "nomFichier".into())],
            vec![],
            vec![],
        );
        let f = parse_search_with("liende:youtube.com typeFichier:mp4 nomFichier:clip", &fr);
        assert_eq!((f.link_from, f.file_types, f.file_names), (vec!["youtube.com".to_owned()], vec!["mp4".to_owned()], vec!["clip".to_owned()]));
        assert_eq!(fr.label(FilterKey::FileType), "typeFichier");
        let q = SearchQuery { link_hostnames: vec!["github.com".into()], attachment_filenames: vec!["crash.txt".into()], ..Default::default() };
        assert!(!q.is_empty());
        assert_eq!(q.to_query(), "attachment_filename=crash.txt&link_hostname=github.com&sort_by=timestamp&sort_order=desc");
        let b = q.to_tabs_body(25);
        assert_eq!(b["tabs"]["messages"]["link_hostname"], serde_json::json!(["github.com"]));
        assert_eq!(b["tabs"]["messages"]["attachment_filename"], serde_json::json!(["crash.txt"]));
    }

    #[test]
    fn pending_filter_tracks_the_last_token() {
        let fr = french();
        let p = pending_filter("salut de:nin", &fr).unwrap();
        assert_eq!((p.key, p.partial.as_str(), p.start), (FilterKey::From, "nin", 9));
        assert_eq!(complete_filter("salut de:nin", &p, "ninette"), "salut de:ninette ");
        let q = pending_filter(r#"dans:"Coin Ca"#, &fr).unwrap();
        assert_eq!((q.key, q.partial.as_str()), (FilterKey::In, "Coin Ca"));
        assert_eq!(complete_filter(r#"dans:"Coin Ca"#, &q, "Coin Café"), "dans:\"Coin Café\" ");
        let e = pending_filter("contient:", &fr).unwrap();
        assert_eq!((e.key, e.partial.as_str()), (FilterKey::Has, ""));
        assert_eq!(pending_filter("de:ninette ", &fr), None);
        assert_eq!(pending_filter("hello", &fr), None);
        assert_eq!(pending_filter("http://x", &fr), None);
        assert_eq!(pending_filter("épinglé:t", &fr).map(|p| p.key), Some(FilterKey::Pinned));
    }

    #[test]
    fn date_snowflakes() {
        assert_eq!(date_to_snowflake("2015-01-01"), Some(Snowflake(0)));
        let s = date_to_snowflake("2015-01-02").unwrap();
        assert_eq!(s.0 >> 22, 86_400_000);
        assert_eq!(date_to_snowflake("2014-12-31"), None);
        assert_eq!(date_to_snowflake("2024-13-01"), None);
        assert_eq!(date_to_snowflake("garbage"), None);
    }

    #[test]
    fn query_string() {
        let q = SearchQuery {
            content: Some("a b&c".into()),
            author_ids: vec![Snowflake(1), Snowflake(2)],
            channel_ids: vec![Snowflake(3)],
            has: vec![HasFilter::Image],
            offset: 25,
            ..Default::default()
        };
        assert_eq!(
            q.to_query(),
            "content=a+b%26c&author_id=1&author_id=2&channel_id=3&has=image&sort_by=timestamp&sort_order=desc&offset=25"
        );
        assert!(!q.is_empty());
        assert!(SearchQuery::text("  ").is_empty());
        let r = SearchQuery { sort: SearchSort::Relevance, ..Default::default() };
        assert_eq!(r.to_query(), "sort_by=relevance&sort_order=desc");
        let m = SearchQuery { author_ids: vec![Snowflake(4)], attachment_extensions: vec!["png".into(), "mp4".into()], ..Default::default() };
        assert_eq!(m.to_query(), "author_id=4&attachment_extension=png&attachment_extension=mp4&sort_by=timestamp&sort_order=desc");
        assert!(!SearchQuery { attachment_extensions: vec!["gif".into()], ..Default::default() }.is_empty());
        assert_eq!(m.to_tabs_body(25)["tabs"]["messages"]["attachment_extension"], serde_json::json!(["png", "mp4"]));
    }

    #[test]
    fn tabs_body_matches_the_client_payload() {
        let q = SearchQuery {
            content: Some(" salut ".into()),
            author_ids: vec![Snowflake(7)],
            channel_ids: vec![Snowflake(3)],
            has: vec![HasFilter::Link],
            pinned: Some(true),
            offset: 25,
            ..Default::default()
        };
        let b = q.to_tabs_body(25);
        assert_eq!(
            b,
            serde_json::json!({
                "include_nsfw": false,
                "channel_ids": ["3"],
                "tabs": {"messages": {"sort_by": "timestamp", "sort_order": "desc", "content": "salut", "author_id": ["7"], "has": ["link"], "pinned": true, "offset": 25, "limit": 25}},
                "track_exact_total_hits": true
            })
        );
        let r = SearchQuery { sort: SearchSort::Oldest, ..Default::default() }.to_tabs_body(25);
        assert_eq!(r["tabs"]["messages"]["sort_order"], "asc");
        assert!(r.get("channel_ids").is_none());
    }

    #[test]
    fn tabs_reply() {
        let m = r#"{"id":"10","channel_id":"5","author":{"id":"1","username":"a","discriminator":"0"},"content":"hi","timestamp":"2024-01-01T00:00:00+00:00","edited_timestamp":null,"tts":false,"mention_everyone":false,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0}"#;
        let body = format!(r#"{{"analytics_id":"x","tabs":{{"messages":{{"total_results":4,"messages":[[{m}],{m}],"cursor":{{}}}}}}}}"#);
        match parse_tabs_reply(StatusCode::OK, body.as_bytes()).unwrap() {
            SearchOutcome::Results(r) => {
                assert_eq!(r.total_results, 4);
                assert_eq!(r.hits().count(), 2);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn search_reply() {
        let body = br#"{"total_results":2,"analytics_id":"x","messages":[[{"id":"10","channel_id":"5","author":{"id":"1","username":"a","discriminator":"0"},"content":"hi","timestamp":"2024-01-01T00:00:00+00:00","edited_timestamp":null,"tts":false,"mention_everyone":false,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0,"hit":true}]]}"#;
        match parse_search_reply(StatusCode::OK, body).unwrap() {
            SearchOutcome::Results(r) => {
                assert_eq!(r.total_results, 2);
                assert_eq!(r.hits().next().unwrap().content, "hi");
            }
            _ => panic!(),
        }
        match parse_search_reply(StatusCode::ACCEPTED, br#"{"message":"Index not yet available","code":110000,"retry_after":3}"#).unwrap() {
            SearchOutcome::Indexing { retry_after } => assert_eq!(retry_after, 3.0),
            _ => panic!(),
        }
    }

    #[test]
    fn gif_shapes() {
        let t: GifTrending = serde_json::from_str(
            r#"{"categories":[{"name":"happy","src":"https://m/x.mp4"}],"gifs":[{"type":"gif","src":"https://m/t.mp4"}]}"#,
        )
        .unwrap();
        assert_eq!(t.categories[0].name, "happy");
        let g: Vec<Gif> = serde_json::from_str(
            r#"[{"id":"1","title":"t","url":"https://tenor.com/v","src":"https://m/a.mp4","gif_src":"https://m/a.gif","width":220,"height":124,"preview":"https://m/p.png"}]"#,
        )
        .unwrap();
        assert_eq!(g[0].width, 220);
        assert_eq!(gif_query(&[("q", "cat dog")], "en-US", GifFormat::Mp4), "q=cat+dog&media_format=mp4&locale=en-US");
    }

    #[test]
    fn sticker_shapes() {
        let p: StickerPacks = serde_json::from_str(
            r#"{"sticker_packs":[{"id":"9","name":"Wumpus","description":"","cover_sticker_id":"7","stickers":[{"id":"7","name":"Wave","tags":"👋","format_type":3,"pack_id":"9","type":1}]}]}"#,
        )
        .unwrap();
        let s = &p.sticker_packs[0].stickers[0];
        assert_eq!(s.name, "Wave");
        assert!(s.url().ends_with("/stickers/7.json"));
    }
}
