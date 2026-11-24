use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, Snowflake, null_default};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScheduledMessage {
    pub id: Snowflake,
    pub channel_id: ChannelId,
    #[serde(default, deserialize_with = "null_default")]
    pub content: String,
    #[serde(default, deserialize_with = "null_default")]
    pub scheduled_timestamp: String,
}

pub const TRAIT_FIRST_TIME: u64 = 1;
pub const TRAIT_DURATION: u64 = 2;
pub const TRAIT_IS_LIVE: u64 = 3;
pub const TRAIT_RESURRECTED: u64 = 5;
pub const TRAIT_STREAK: u64 = 8;
pub const TRAIT_TRENDING: u64 = 9;
pub const CONTENT_PLAYED_GAME: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryEntry {
    pub id: u64,
    pub author_id: u64,
    pub content_type: u64,
    pub trending: bool,
    pub streak_days: Option<u64>,
    pub application_id: Option<u64>,
    pub game_name: Option<String>,
    pub first_time: bool,
    pub resurrected: bool,
    pub live: bool,
    pub started_at: Option<String>,
    pub expires_at: Option<String>,
}

fn id_of(v: &Value) -> Option<u64> {
    v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_u64())
}

pub fn parse_inventory(v: &Value) -> Vec<InventoryEntry> {
    let Some(entries) = v.get("entries").and_then(Value::as_array) else { return Vec::new() };
    entries
        .iter()
        .map(|e| e.get("content").filter(|c| c.is_object()).unwrap_or(e))
        .filter_map(|e| {
            let author_id = id_of(e.get("author_id")?)?;
            let extra = e.get("extra");
            let traits = e.get("traits").and_then(Value::as_array).cloned().unwrap_or_default();
            let tr = |t: u64| traits.iter().find(|x| x.get("type").and_then(Value::as_u64) == Some(t)).cloned();
            Some(InventoryEntry {
                id: e.get("id").and_then(id_of).unwrap_or(0),
                author_id,
                content_type: e.get("content_type").and_then(Value::as_u64).unwrap_or(CONTENT_PLAYED_GAME),
                trending: tr(TRAIT_TRENDING).is_some_and(|t| t.get("trending").is_some_and(|x| !x.is_null() && x != &Value::Bool(false))),
                streak_days: tr(TRAIT_STREAK).and_then(|t| t.get("streak_count_days")?.as_u64()),
                application_id: extra.and_then(|x| x.get("application_id")).and_then(id_of),
                game_name: extra
                    .and_then(|x| x.get("game_name").or_else(|| x.get("activity_name")))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                first_time: tr(TRAIT_FIRST_TIME).and_then(|t| t.get("first_time")?.as_bool()).unwrap_or(false),
                resurrected: tr(TRAIT_RESURRECTED).is_some_and(|t| t.get("resurrected_last_played").is_some_and(|x| !x.is_null())),
                live: tr(TRAIT_IS_LIVE).and_then(|t| t.get("is_live")?.as_bool()).unwrap_or(false),
                started_at: e.get("started_at").and_then(Value::as_str).map(str::to_owned),
                expires_at: e.get("expires_at").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

impl RestClient {
    pub async fn get_scheduled_messages(&self) -> Result<Vec<ScheduledMessage>> {
        self.call(Method::GET, "/users/@me/scheduled-messages", Body::None).await
    }

    pub async fn get_content_inventory(&self) -> Result<Vec<InventoryEntry>> {
        let v: Value = self.call(Method::GET, "/content-inventory/users/@me", Body::None).await?;
        Ok(parse_inventory(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_traits() {
        let v = serde_json::json!({"entries": [
            {"author_id": "42", "extra": {"application_id": "7", "game_name": "Hades"},
             "traits": [{"type": 1, "first_time": true}]},
            {"author_id": "43", "traits": [{"type": 5, "resurrected_last_played": "2025-01-01T00:00:00Z"}]},
            {"traits": []}
        ]});
        let e = parse_inventory(&v);
        assert_eq!(e.len(), 2);
        assert!(e[0].first_time && !e[0].resurrected);
        assert_eq!(e[0].application_id, Some(7));
        assert!(e[1].resurrected && !e[1].first_time);
    }

    #[test]
    fn unwraps_content() {
        let v = serde_json::json!({"entries": [{"content": {"id": "9", "author_id": "42", "content_type": 1,
            "ended_at": "2026-11-21T18:32:38.874000+00:00", "extra": {"game_name": "Roblox"},
            "traits": [{"type": 8, "streak_count_days": 2}, {"type": 9, "trending": 1}]}}]});
        let e = parse_inventory(&v);
        assert_eq!((e[0].id, e[0].author_id, e[0].streak_days), (9, 42, Some(2)));
        assert!(e[0].trending);
        assert!(!e[0].live && e[0].started_at.is_none());
    }

    #[test]
    fn parses_live_session() {
        let v = serde_json::json!({"entries": [{"id": "1", "author_id": "42", "content_type": 1,
            "started_at": "2026-11-24T13:28:52.309000+00:00", "expires_at": "2026-11-24T14:03:52.308170+00:00",
            "extra": {"game_name": "War Thunder"}, "traits": [{"type": 2, "duration_seconds": 0}, {"type": 3, "is_live": true}]}]});
        let e = parse_inventory(&v);
        assert!(e[0].live);
        assert_eq!(e[0].started_at.as_deref(), Some("2026-11-24T13:28:52.309000+00:00"));
        assert_eq!(e[0].expires_at.as_deref(), Some("2026-11-24T14:03:52.308170+00:00"));
    }

    #[test]
    fn parses_scheduled() {
        let s: Vec<ScheduledMessage> = serde_json::from_str(r#"[{"id":"1","channel_id":"2","content":"hi","scheduled_timestamp":"2026-11-24T09:00:00+00:00"}]"#).unwrap();
        assert_eq!(s[0].content, "hi");
    }
}
