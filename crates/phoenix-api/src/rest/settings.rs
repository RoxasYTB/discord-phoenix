use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::{Error, Result};

#[derive(Clone, PartialEq, prost::Message)]
pub struct BoolValue {
    #[prost(bool, tag = "1")]
    pub value: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StringValue {
    #[prost(string, tag = "1")]
    pub value: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TextAndImagesSettings {
    #[prost(message, optional, tag = "10")]
    pub inline_embed_media: Option<BoolValue>,
    #[prost(message, optional, tag = "11")]
    pub gif_auto_play: Option<BoolValue>,
    #[prost(message, optional, tag = "14")]
    pub animate_emoji: Option<BoolValue>,
    #[prost(message, optional, tag = "17")]
    pub message_display_compact: Option<BoolValue>,
    #[prost(message, optional, tag = "21")]
    pub convert_emoticons: Option<BoolValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NotificationSettings {
    #[prost(message, optional, tag = "1")]
    pub show_in_app_notifications: Option<BoolValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PrivacySettings {
    #[prost(fixed64, repeated, tag = "3")]
    pub restricted_guild_ids: Vec<u64>,
    #[prost(bool, tag = "4")]
    pub default_guilds_restricted: bool,
    #[prost(fixed64, repeated, tag = "13")]
    pub activity_restricted_guild_ids: Vec<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StatusSettings {
    #[prost(message, optional, tag = "1")]
    pub status: Option<StringValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LocalizationSettings {
    #[prost(message, optional, tag = "1")]
    pub locale: Option<StringValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Ash,
    Onyx,
}

impl Theme {
    pub fn from_proto(v: i32) -> Self {
        match v {
            2 => Self::Light,
            3 => Self::Ash,
            4 => Self::Onyx,
            _ => Self::Dark,
        }
    }
    pub fn to_proto(self) -> i32 {
        match self {
            Self::Dark => 1,
            Self::Light => 2,
            Self::Ash => 3,
            Self::Onyx => 4,
        }
    }
    pub fn from_json(s: &str) -> Self {
        match s {
            "light" => Self::Light,
            "darker" => Self::Ash,
            "midnight" => Self::Onyx,
            _ => Self::Dark,
        }
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppearanceSettings {
    #[prost(int32, tag = "1")]
    pub theme: i32,
    #[prost(bool, tag = "2")]
    pub developer_mode: bool,
    #[prost(int32, tag = "12")]
    pub ui_density: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PreloadedUserSettings {
    #[prost(message, optional, tag = "6")]
    pub text_and_images: Option<TextAndImagesSettings>,
    #[prost(message, optional, tag = "7")]
    pub notifications: Option<NotificationSettings>,
    #[prost(message, optional, tag = "8")]
    pub privacy: Option<PrivacySettings>,
    #[prost(message, optional, tag = "11")]
    pub status: Option<StatusSettings>,
    #[prost(message, optional, tag = "12")]
    pub localization: Option<LocalizationSettings>,
    #[prost(message, optional, tag = "13")]
    pub appearance: Option<AppearanceSettings>,
}

fn b(v: &Option<BoolValue>) -> Option<bool> {
    v.as_ref().map(|b| b.value)
}

impl PreloadedUserSettings {
    pub fn decode_base64(s: &str) -> Result<Self> {
        let bytes = STANDARD.decode(s.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))?;
        <Self as prost::Message>::decode(bytes.as_slice()).map_err(|e| Error::Protocol(format!("settings proto: {e}")))
    }
    pub fn decode_raw(bytes: &[u8]) -> Result<Self> {
        <Self as prost::Message>::decode(bytes).map_err(|e| Error::Protocol(format!("settings proto: {e}")))
    }
    pub fn encode_base64(&self) -> String {
        STANDARD.encode(prost::Message::encode_to_vec(self))
    }

    pub fn theme(&self) -> Theme {
        self.appearance.as_ref().map(|a| Theme::from_proto(a.theme)).unwrap_or_default()
    }
    pub fn developer_mode(&self) -> bool {
        self.appearance.as_ref().is_some_and(|a| a.developer_mode)
    }
    pub fn ui_density(&self) -> i32 {
        self.appearance.as_ref().map_or(0, |a| a.ui_density)
    }
    pub fn compact(&self) -> bool {
        self.text_and_images.as_ref().and_then(|t| b(&t.message_display_compact)).unwrap_or(false)
    }
    pub fn locale(&self) -> Option<&str> {
        self.localization.as_ref()?.locale.as_ref().map(|s| s.value.as_str())
    }
    pub fn status(&self) -> Option<&str> {
        self.status.as_ref()?.status.as_ref().map(|s| s.value.as_str())
    }
    pub fn default_guilds_restricted(&self) -> bool {
        self.privacy.as_ref().is_some_and(|p| p.default_guilds_restricted)
    }

    pub fn patch_appearance(theme: Theme, developer_mode: bool) -> Self {
        Self {
            appearance: Some(AppearanceSettings { theme: theme.to_proto(), developer_mode, ui_density: 0 }),
            ..Default::default()
        }
    }
    pub fn patch_compact(compact: bool) -> Self {
        Self {
            text_and_images: Some(TextAndImagesSettings {
                message_display_compact: Some(BoolValue { value: compact }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    pub fn patch_locale(locale: &str) -> Self {
        Self {
            localization: Some(LocalizationSettings { locale: Some(StringValue { value: locale.to_owned() }) }),
            ..Default::default()
        }
    }
    pub fn patch_default_guilds_restricted(v: bool) -> Self {
        Self {
            privacy: Some(PrivacySettings { default_guilds_restricted: v, ..Default::default() }),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingChange {
    Theme(Theme),
    DeveloperMode(bool),
    Compact(bool),
    Locale(String),
    DefaultGuildsRestricted(bool),
    UiDensity(i32),
    Path { path: Vec<u32>, wrapper: bool, value: ProtoValue },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtoValue {
    Int(u64),
    Str(String),
}

fn read_varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte < 0x80 {
            return Some(v);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn fields(b: &[u8]) -> Vec<(u32, u8, u64, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let Some(key) = read_varint(b, &mut i) else { break };
        let (tag, wt) = ((key >> 3) as u32, (key & 7) as u8);
        match wt {
            0 => match read_varint(b, &mut i) {
                Some(v) => out.push((tag, 0, v, &b[0..0])),
                None => break,
            },
            1 | 5 => {
                let n = if wt == 1 { 8 } else { 4 };
                let Some(raw) = b.get(i..i + n) else { break };
                let mut v = 0u64;
                for (k, x) in raw.iter().enumerate() {
                    v |= (*x as u64) << (8 * k);
                }
                out.push((tag, wt, v, &b[0..0]));
                i += n;
            }
            2 => {
                let Some(n) = read_varint(b, &mut i) else { break };
                let Some(d) = b.get(i..i + n as usize) else { break };
                out.push((tag, 2, 0, d));
                i += n as usize;
            }
            _ => break,
        }
    }
    out
}

pub fn proto_read(raw: &[u8], path: &[u32], wrapper: bool, as_str: bool) -> Option<ProtoValue> {
    let (&first, rest) = path.split_first()?;
    let mut found = None;
    for (tag, wt, v, d) in fields(raw) {
        if tag != first {
            continue;
        }
        let got = if !rest.is_empty() {
            if wt != 2 {
                continue;
            }
            proto_read(d, rest, wrapper, as_str)
        } else if wrapper {
            if wt != 2 {
                continue;
            }
            Some(proto_read(d, &[1], false, as_str).unwrap_or(if as_str { ProtoValue::Str(String::new()) } else { ProtoValue::Int(0) }))
        } else if as_str {
            (wt == 2).then(|| ProtoValue::Str(String::from_utf8_lossy(d).into_owned()))
        } else {
            (wt != 2).then_some(ProtoValue::Int(v))
        };
        if got.is_some() {
            found = got;
        }
    }
    found
}

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn field_varint(tag: u32, v: u64, out: &mut Vec<u8>) {
    varint((tag as u64) << 3, out);
    varint(v, out);
}

fn field_bytes(tag: u32, data: &[u8], out: &mut Vec<u8>) {
    varint(((tag as u64) << 3) | 2, out);
    varint(data.len() as u64, out);
    out.extend_from_slice(data);
}

impl SettingChange {
    pub fn encode(&self) -> Vec<u8> {
        let mut inner = Vec::new();
        let mut out = Vec::new();
        match self {
            Self::Theme(t) => {
                field_varint(1, t.to_proto() as u64, &mut inner);
                field_bytes(13, &inner, &mut out);
            }
            Self::DeveloperMode(b) => {
                field_varint(2, *b as u64, &mut inner);
                field_bytes(13, &inner, &mut out);
            }
            Self::Compact(b) => {
                let mut w = Vec::new();
                field_varint(1, *b as u64, &mut w);
                field_bytes(17, &w, &mut inner);
                field_bytes(6, &inner, &mut out);
            }
            Self::Locale(l) => {
                let mut w = Vec::new();
                field_bytes(1, l.as_bytes(), &mut w);
                field_bytes(1, &w, &mut inner);
                field_bytes(12, &inner, &mut out);
            }
            Self::DefaultGuildsRestricted(b) => {
                field_varint(4, *b as u64, &mut inner);
                field_bytes(8, &inner, &mut out);
            }
            Self::UiDensity(v) => {
                field_varint(12, *v as u64, &mut inner);
                field_bytes(13, &inner, &mut out);
            }
            Self::Path { path, wrapper, value } => {
                let Some((&last, outer)) = path.split_last() else { return out };
                let put = |tag: u32, o: &mut Vec<u8>| match value {
                    ProtoValue::Int(v) => field_varint(tag, *v, o),
                    ProtoValue::Str(s) => field_bytes(tag, s.as_bytes(), o),
                };
                let mut cur = Vec::new();
                if *wrapper {
                    let mut w = Vec::new();
                    put(1, &mut w);
                    field_bytes(last, &w, &mut cur);
                } else {
                    put(last, &mut cur);
                }
                for &tag in outer.iter().rev() {
                    let mut o = Vec::new();
                    field_bytes(tag, &cur, &mut o);
                    cur = o;
                }
                out = cur;
            }
        }
        out
    }
}

#[derive(Deserialize)]
struct ProtoBody {
    settings: String,
}

impl RestClient {
    pub async fn get_settings_proto(&self) -> Result<PreloadedUserSettings> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/1", Body::None).await?;
        PreloadedUserSettings::decode_base64(&b.settings)
    }

    pub async fn patch_settings_proto(&self, patch: &PreloadedUserSettings) -> Result<PreloadedUserSettings> {
        let b: ProtoBody = self
            .call(
                Method::PATCH,
                "/users/@me/settings-proto/1",
                Body::Json(json!({ "settings": patch.encode_base64() })),
            )
            .await?;
        PreloadedUserSettings::decode_base64(&b.settings)
    }

    pub async fn patch_setting(&self, change: &SettingChange) -> Result<PreloadedUserSettings> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/1", Body::None).await?;
        let mut raw = STANDARD.decode(b.settings.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))?;
        raw.extend(change.encode());
        let b: ProtoBody = self
            .call(Method::PATCH, "/users/@me/settings-proto/1", Body::Json(json!({ "settings": STANDARD.encode(raw) })))
            .await?;
        PreloadedUserSettings::decode_base64(&b.settings)
    }

    pub async fn get_settings_proto_raw(&self) -> Result<Vec<u8>> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/1", Body::None).await?;
        STANDARD.decode(b.settings.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))
    }

    pub async fn patch_setting_raw(&self, change: &SettingChange) -> Result<Vec<u8>> {
        let mut raw = self.get_settings_proto_raw().await?;
        raw.extend(change.encode());
        let b: ProtoBody = self
            .call(Method::PATCH, "/users/@me/settings-proto/1", Body::Json(json!({ "settings": STANDARD.encode(raw) })))
            .await?;
        STANDARD.decode(b.settings.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))
    }

    pub async fn get_consent(&self) -> Result<Value> {
        self.call(Method::GET, "/users/@me/consent", Body::None).await
    }

    pub async fn set_consent(&self, kind: &str, granted: bool) -> Result<()> {
        let (grant, revoke): (Vec<&str>, Vec<&str>) = if granted { (vec![kind], vec![]) } else { (vec![], vec![kind]) };
        self.call_empty(Method::POST, "/users/@me/consent", Body::Json(json!({ "grant": grant, "revoke": revoke }))).await
    }

    pub async fn get_email_settings(&self) -> Result<Value> {
        self.call(Method::GET, "/users/@me/email-settings", Body::None).await
    }

    pub async fn patch_email_setting(&self, category: &str, on: bool) -> Result<Value> {
        self.call(
            Method::PATCH,
            "/users/@me/email-settings",
            Body::Json(json!({ "settings": { "categories": { category: on } } })),
        )
        .await
    }

    pub async fn get_settings_json(&self) -> Result<Value> {
        self.call(Method::GET, "/users/@me/settings", Body::None).await
    }

    pub async fn patch_settings_json(&self, patch: Value) -> Result<Value> {
        self.call(Method::PATCH, "/users/@me/settings", Body::Json(patch)).await
    }
}

pub fn from_json(v: &Value) -> PreloadedUserSettings {
    let mut s = PreloadedUserSettings::patch_appearance(
        Theme::from_json(v.get("theme").and_then(Value::as_str).unwrap_or("dark")),
        v.get("developer_mode").and_then(Value::as_bool).unwrap_or(false),
    );
    s.text_and_images = Some(TextAndImagesSettings {
        message_display_compact: v.get("message_display_compact").and_then(Value::as_bool).map(|value| BoolValue { value }),
        ..Default::default()
    });
    if let Some(l) = v.get("locale").and_then(Value::as_str) {
        s.localization = Some(LocalizationSettings { locale: Some(StringValue { value: l.to_owned() }) });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_unknown_fields() {
        let mut s = PreloadedUserSettings::patch_appearance(Theme::Onyx, true);
        s.text_and_images = PreloadedUserSettings::patch_compact(true).text_and_images;
        let enc = s.encode_base64();
        let d = PreloadedUserSettings::decode_base64(&enc).unwrap();
        assert_eq!(d.theme(), Theme::Onyx);
        assert!(d.developer_mode());
        assert!(d.compact());

        let mut bytes = STANDARD.decode(&enc).unwrap();
        bytes.extend_from_slice(&[0x72, 0x02, 0x08, 0x01]);
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(bytes)).unwrap();
        assert_eq!(d.theme(), Theme::Onyx);
    }

    #[test]
    fn theme_wire_values() {
        let bytes = [0x6a, 0x02, 0x08, 0x02];
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(bytes)).unwrap();
        assert_eq!(d.theme(), Theme::Light);
        assert!(!d.compact());
    }

    #[test]
    fn appended_changes_override_including_false() {
        let base = PreloadedUserSettings::patch_appearance(Theme::Light, true);
        let mut raw = prost::Message::encode_to_vec(&base);
        raw.extend(SettingChange::DeveloperMode(false).encode());
        raw.extend(SettingChange::Compact(true).encode());
        raw.extend(SettingChange::Locale("fr".into()).encode());
        raw.extend(SettingChange::DefaultGuildsRestricted(true).encode());
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(&raw)).unwrap();
        assert_eq!(d.theme(), Theme::Light, "untouched field kept");
        assert!(!d.developer_mode());
        assert!(d.compact());
        assert_eq!(d.locale(), Some("fr"));
        assert!(d.default_guilds_restricted());
        raw.extend(SettingChange::Theme(Theme::Ash).encode());
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(&raw)).unwrap();
        assert_eq!(d.theme(), Theme::Ash);
    }

    #[test]
    fn path_changes_roundtrip() {
        let base = PreloadedUserSettings::patch_appearance(Theme::Light, true);
        let mut raw = prost::Message::encode_to_vec(&base);
        let ch = |path: &[u32], wrapper, value| SettingChange::Path { path: path.to_vec(), wrapper, value };
        raw.extend(ch(&[6, 12], true, ProtoValue::Int(0)).encode());
        raw.extend(ch(&[6, 4], true, ProtoValue::Str("ALWAYS".into())).encode());
        raw.extend(ch(&[13, 9], false, ProtoValue::Int(2)).encode());
        assert_eq!(proto_read(&raw, &[6, 12], true, false), Some(ProtoValue::Int(0)));
        assert_eq!(proto_read(&raw, &[6, 4], true, true), Some(ProtoValue::Str("ALWAYS".into())));
        assert_eq!(proto_read(&raw, &[13, 9], false, false), Some(ProtoValue::Int(2)));
        assert_eq!(proto_read(&raw, &[6, 13], true, false), None);
        raw.extend(ch(&[6, 12], true, ProtoValue::Int(1)).encode());
        assert_eq!(proto_read(&raw, &[6, 12], true, false), Some(ProtoValue::Int(1)));
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(&raw)).unwrap();
        assert_eq!(d.theme(), Theme::Light, "untouched field kept");
        assert!(d.developer_mode());
    }

    #[test]
    fn json_fallback() {
        let s = from_json(&json!({"theme": "darker", "message_display_compact": true, "locale": "fr"}));
        assert_eq!(s.theme(), Theme::Ash);
        assert!(s.compact());
        assert_eq!(s.locale(), Some("fr"));
    }
}
