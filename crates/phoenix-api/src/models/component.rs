use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{ChannelType, EmojiId, Snowflake, null_default};

pub const FLAG_IS_COMPONENTS_V2: u64 = 1 << 15;

int_enum! {
    pub enum ButtonStyle: u8 {
        Primary = 1,
        Secondary = 2,
        Success = 3,
        Danger = 4,
        Link = 5,
        Premium = 6,
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ButtonStyle {
    fn default() -> Self {
        Self::Primary
    }
}

int_enum! {
    pub enum SeparatorSpacing: u8 {
        Small = 1,
        Large = 2,
    }
}

#[allow(clippy::derivable_impls)]
impl Default for SeparatorSpacing {
    fn default() -> Self {
        Self::Small
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ComponentEmoji {
    #[serde(default)]
    pub id: Option<EmojiId>,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub animated: bool,
}

impl ComponentEmoji {
    pub fn token(&self) -> String {
        match self.id {
            Some(id) => format!("<{}:{}:{}>", if self.animated { "a" } else { "" }, self.name, id),
            None => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Button {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default)]
    pub style: ButtonStyle,
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<ComponentEmoji>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku_id: Option<Snowflake>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SelectOption {
    #[serde(default, deserialize_with = "null_default")]
    pub label: String,
    #[serde(default, deserialize_with = "null_default")]
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<ComponentEmoji>,
    #[serde(default, deserialize_with = "null_default")]
    pub default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SelectDefault {
    pub id: Snowflake,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectKind {
    #[default]
    String,
    User,
    Role,
    Mentionable,
    Channel,
}

impl SelectKind {
    pub fn type_id(self) -> u8 {
        match self {
            Self::String => 3,
            Self::User => 5,
            Self::Role => 6,
            Self::Mentionable => 7,
            Self::Channel => 8,
        }
    }

    fn from_type(t: u64) -> Option<Self> {
        Some(match t {
            3 => Self::String,
            5 => Self::User,
            6 => Self::Role,
            7 => Self::Mentionable,
            8 => Self::Channel,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SelectMenu {
    #[serde(skip)]
    pub kind: SelectKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub custom_id: String,
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<SelectOption>,
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub channel_types: Vec<ChannelType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub default_values: Vec<SelectDefault>,
    #[serde(default = "one")]
    pub min_values: u32,
    #[serde(default = "one")]
    pub max_values: u32,
    #[serde(default, deserialize_with = "null_default")]
    pub disabled: bool,
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub custom_id: String,
    #[serde(default = "one_u8")]
    pub style: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

fn one_u8() -> u8 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UnfurledMedia {
    #[serde(default, deserialize_with = "null_default")]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<Snowflake>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loading_state: Option<u8>,
}

impl UnfurledMedia {
    pub fn fetch_url(&self) -> &str {
        self.proxy_url.as_deref().filter(|u| !u.is_empty()).unwrap_or(&self.url)
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
            _ => None,
        }
    }

    pub fn is_video(&self) -> bool {
        self.content_type.as_deref().is_some_and(|c| c.starts_with("video/"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Section {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<Box<Component>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextDisplay {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Thumbnail {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default)]
    pub media: UnfurledMedia,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GalleryItem {
    #[serde(default)]
    pub media: UnfurledMedia,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MediaGallery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub items: Vec<GalleryItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FileComponent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default)]
    pub file: UnfurledMedia,
    #[serde(default, deserialize_with = "null_default")]
    pub spoiler: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl FileComponent {
    pub fn file_name(&self) -> String {
        if let Some(n) = self.name.as_deref().filter(|n| !n.is_empty()) {
            return n.to_owned();
        }
        let path = self.file.url.split(['?', '#']).next().unwrap_or_default();
        path.trim_start_matches("attachment://").rsplit('/').next().unwrap_or_default().to_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Separator {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default = "yes", deserialize_with = "null_true")]
    pub divider: bool,
    #[serde(default)]
    pub spacing: SeparatorSpacing,
}

impl Default for Separator {
    fn default() -> Self {
        Self { id: None, divider: true, spacing: SeparatorSpacing::Small }
    }
}

fn null_true<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(d)?.unwrap_or(true))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Container {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub components: Vec<Component>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ActionRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub components: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Component {
    ActionRow(ActionRow),
    Button(Button),
    Select(SelectMenu),
    TextInput(TextInput),
    Section(Section),
    TextDisplay(TextDisplay),
    Thumbnail(Thumbnail),
    MediaGallery(MediaGallery),
    File(FileComponent),
    Separator(Separator),
    Container(Container),
    Unknown(Value),
}

impl Component {
    pub fn type_id(&self) -> u8 {
        match self {
            Self::ActionRow(_) => 1,
            Self::Button(_) => 2,
            Self::Select(s) => s.kind.type_id(),
            Self::TextInput(_) => 4,
            Self::Section(_) => 9,
            Self::TextDisplay(_) => 10,
            Self::Thumbnail(_) => 11,
            Self::MediaGallery(_) => 12,
            Self::File(_) => 13,
            Self::Separator(_) => 14,
            Self::Container(_) => 17,
            Self::Unknown(v) => v.get("type").and_then(Value::as_u64).unwrap_or(0) as u8,
        }
    }

    pub fn children(&self) -> Vec<&Component> {
        match self {
            Self::ActionRow(r) => r.components.iter().collect(),
            Self::Section(s) => s.components.iter().chain(s.accessory.as_deref()).collect(),
            Self::Container(c) => c.components.iter().collect(),
            _ => Vec::new(),
        }
    }

    pub fn texts<'a>(&'a self, out: &mut Vec<&'a str>) {
        if let Self::TextDisplay(t) = self {
            out.push(&t.content);
        }
        for c in self.children() {
            c.texts(out);
        }
    }
}

impl<'de> Deserialize<'de> for Component {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from_value(Value::deserialize(d)?))
    }
}

impl Component {
    pub fn from_value(v: Value) -> Self {
        fn parse<T: serde::de::DeserializeOwned>(v: &Value) -> Option<T> {
            serde_json::from_value(v.clone()).ok()
        }
        let t = v.get("type").and_then(Value::as_u64).unwrap_or(0);
        let parsed = match t {
            1 => parse(&v).map(Self::ActionRow),
            2 => parse(&v).map(Self::Button),
            4 => parse(&v).map(Self::TextInput),
            9 => parse(&v).map(Self::Section),
            10 => parse(&v).map(Self::TextDisplay),
            11 => parse(&v).map(Self::Thumbnail),
            12 => parse(&v).map(Self::MediaGallery),
            13 => parse(&v).map(Self::File),
            14 => parse(&v).map(Self::Separator),
            17 => parse(&v).map(Self::Container),
            t => SelectKind::from_type(t).and_then(|kind| parse::<SelectMenu>(&v).map(|s| Self::Select(SelectMenu { kind, ..s }))),
        };
        parsed.unwrap_or(Self::Unknown(v))
    }
}

impl Serialize for Component {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let inner = match self {
            Self::ActionRow(x) => serde_json::to_value(x),
            Self::Button(x) => serde_json::to_value(x),
            Self::Select(x) => serde_json::to_value(x),
            Self::TextInput(x) => serde_json::to_value(x),
            Self::Section(x) => serde_json::to_value(x),
            Self::TextDisplay(x) => serde_json::to_value(x),
            Self::Thumbnail(x) => serde_json::to_value(x),
            Self::MediaGallery(x) => serde_json::to_value(x),
            Self::File(x) => serde_json::to_value(x),
            Self::Separator(x) => serde_json::to_value(x),
            Self::Container(x) => serde_json::to_value(x),
            Self::Unknown(v) => return v.serialize(s),
        };
        let mut v = inner.map_err(S::Error::custom)?;
        if let Value::Object(m) = &mut v {
            m.insert("type".into(), Value::from(self.type_id()));
        }
        v.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!([
            {"type": 1, "id": 1, "components": [
                {"type": 2, "id": 2, "custom_id": "a", "style": 1, "label": "Primary", "emoji": {"id": "100000000000000001", "name": "ok", "animated": true}},
                {"type": 2, "id": 3, "custom_id": "b", "style": 2, "label": "Secondary", "emoji": {"name": "🔥"}, "disabled": true},
                {"type": 2, "id": 4, "custom_id": "c", "style": 3, "label": "Success"},
                {"type": 2, "id": 5, "custom_id": "d", "style": 4, "label": "Danger"},
                {"type": 2, "id": 6, "style": 5, "label": "Link", "url": "https://example.com/"},
                {"type": 2, "id": 7, "style": 6, "sku_id": "100000000000000002"}]},
            {"type": 1, "id": 8, "components": [
                {"type": 3, "id": 9, "custom_id": "pick", "placeholder": "Pick", "min_values": 1, "max_values": 5,
                 "options": [{"label": "A", "value": "1", "description": "first"}, {"label": "B", "value": "2", "default": true, "emoji": {"name": "🌸"}}]}]},
            {"type": 1, "components": [{"type": 5, "custom_id": "u", "default_values": [{"id": "100000000000000003", "type": "user"}]}]},
            {"type": 1, "components": [{"type": 6, "custom_id": "r"}]},
            {"type": 1, "components": [{"type": 7, "custom_id": "m", "max_values": 3}]},
            {"type": 1, "components": [{"type": 8, "custom_id": "c", "channel_types": [0, 2]}]},
            {"type": 17, "id": 10, "accent_color": 16766720, "spoiler": false, "components": [
                {"type": 10, "id": 11, "content": "## Title"},
                {"type": 14, "id": 12, "spacing": 1, "divider": true},
                {"type": 9, "id": 13, "components": [{"type": 10, "id": 14, "content": "Row text"}],
                 "accessory": {"type": 2, "id": 15, "custom_id": "cfg", "style": 3, "label": "On"}},
                {"type": 9, "id": 16, "components": [{"type": 10, "content": "With picture"}],
                 "accessory": {"type": 11, "id": 17, "media": {"url": "https://cdn.example/a.png", "proxy_url": "https://media.example/a.png", "width": 128, "height": 128, "content_type": "image/png", "loading_state": 2}, "description": null, "spoiler": false}},
                {"type": 12, "id": 18, "items": [{"media": {"url": "https://cdn.example/b.png", "width": 800, "height": 400, "placeholder": "abcd", "placeholder_version": 1}, "description": null, "spoiler": true}]},
                {"type": 13, "id": 19, "file": {"url": "https://cdn.example/t.html?ex=1&hm=2", "width": 0, "height": 0, "content_type": "text/html; charset=utf-8"}, "name": "transcript.html", "size": 40104, "spoiler": false},
                {"type": 14, "spacing": 2, "divider": false}]},
            {"type": 17, "accent_color": null, "spoiler": true, "components": [{"type": 10, "content": "hidden"}]},
            {"type": 99, "id": 20, "future": true}
        ])
    }

    #[test]
    fn parses_every_type() {
        let c: Vec<Component> = serde_json::from_value(fixture()).unwrap();
        let types: Vec<u8> = c.iter().map(Component::type_id).collect();
        assert_eq!(types, [1, 1, 1, 1, 1, 1, 17, 17, 99]);
        let Component::ActionRow(r) = &c[0] else { panic!() };
        let styles: Vec<ButtonStyle> = r.components.iter().map(|b| match b {
            Component::Button(b) => b.style,
            _ => panic!(),
        }).collect();
        use ButtonStyle::*;
        assert_eq!(styles, [Primary, Secondary, Success, Danger, Link, Premium]);
        let Component::Button(b) = &r.components[0] else { panic!() };
        assert_eq!(b.emoji.as_ref().unwrap().token(), "<a:ok:100000000000000001>");
        let Component::Button(b) = &r.components[1] else { panic!() };
        assert!(b.disabled && b.emoji.as_ref().unwrap().token() == "🔥");
        let Component::Button(b) = &r.components[4] else { panic!() };
        assert_eq!(b.url.as_deref(), Some("https://example.com/"));
        let Component::Button(b) = &r.components[5] else { panic!() };
        assert!(b.sku_id.is_some() && b.label.is_none());

        let kinds: Vec<(SelectKind, u32)> = c[1..6]
            .iter()
            .map(|row| match row.children()[0] {
                Component::Select(s) => (s.kind, s.max_values),
                _ => panic!(),
            })
            .collect();
        use SelectKind as K;
        assert_eq!(kinds, [(K::String, 5), (K::User, 1), (K::Role, 1), (K::Mentionable, 3), (K::Channel, 1)]);
        let Component::Select(s) = c[1].children()[0] else { panic!() };
        assert!(s.options[1].default && s.options[0].description.as_deref() == Some("first"));
        let Component::Select(s) = c[2].children()[0] else { panic!() };
        assert_eq!(s.default_values[0].kind, "user");

        let Component::Container(k) = &c[6] else { panic!() };
        assert_eq!(k.accent_color, Some(0xFFD700));
        let Component::Separator(sep) = &k.components[1] else { panic!() };
        assert!(sep.divider && sep.spacing == SeparatorSpacing::Small);
        let Component::Section(sec) = &k.components[3] else { panic!() };
        let Some(Component::Thumbnail(t)) = sec.accessory.as_deref() else { panic!() };
        assert_eq!((t.media.fetch_url(), t.media.size()), ("https://media.example/a.png", Some((128, 128))));
        let Component::MediaGallery(g) = &k.components[4] else { panic!() };
        assert!(g.items[0].spoiler && g.items[0].media.size() == Some((800, 400)));
        let Component::File(f) = &k.components[5] else { panic!() };
        assert_eq!((f.file_name(), f.size, f.file.size()), ("transcript.html".into(), Some(40104), None));
        assert_eq!(k.components[6], Component::Separator(Separator { id: None, divider: false, spacing: SeparatorSpacing::Large }));
        let Component::Container(k) = &c[7] else { panic!() };
        assert!(k.spoiler && k.accent_color.is_none());
        assert!(matches!(&c[8], Component::Unknown(v) if v["future"] == true));

        let mut texts = Vec::new();
        c[6].texts(&mut texts);
        assert_eq!(texts, ["## Title", "Row text", "With picture"]);
    }

    #[test]
    fn round_trips() {
        let c: Vec<Component> = serde_json::from_value(fixture()).unwrap();
        let back: Vec<Component> = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(c, back);
        let v = serde_json::to_value(&c[1]).unwrap();
        assert_eq!(v["components"][0]["type"], 3);
    }

    #[test]
    fn tolerates_bad_fields() {
        let c: Vec<Component> = serde_json::from_value(json!([
            {"type": 2, "style": "big"},
            {"type": 10, "content": null},
            {"type": 14, "divider": null}
        ]))
        .unwrap();
        assert!(matches!(c[0], Component::Unknown(_)));
        assert_eq!(c[1], Component::TextDisplay(TextDisplay::default()));
        assert_eq!(c[2], Component::Separator(Separator::default()));
    }

    #[test]
    fn file_name_from_url() {
        let f = FileComponent {
            file: UnfurledMedia { url: "attachment://log.txt".into(), ..Default::default() },
            ..Default::default()
        };
        assert_eq!(f.file_name(), "log.txt");
        let f = FileComponent {
            file: UnfurledMedia { url: "https://cdn.example/a/b/c.zip?ex=1".into(), ..Default::default() },
            ..Default::default()
        };
        assert_eq!(f.file_name(), "c.zip");
    }
}
