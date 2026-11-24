use reqwest::Method;
use serde_json::{Value, json};

use super::{Body, RestClient, moderation::rfc3339};
use crate::error::Result;
use crate::models::{Channel, ChannelId, GuildId, Snowflake};

pub fn clone_channel_body(src: &Channel, name: &str) -> Value {
    let mut v = json!({
        "name": name,
        "type": src.kind,
        "permission_overwrites": src.permission_overwrites,
        "nsfw": src.nsfw,
    });
    let o = v.as_object_mut().expect("object");
    if let Some(t) = &src.topic {
        o.insert("topic".into(), json!(t));
    }
    if let Some(p) = src.parent_id {
        o.insert("parent_id".into(), json!(p));
    }
    if let Some(r) = src.rate_limit_per_user {
        o.insert("rate_limit_per_user".into(), json!(r));
    }
    if let Some(b) = src.bitrate {
        o.insert("bitrate".into(), json!(b));
    }
    if let Some(u) = src.user_limit {
        o.insert("user_limit".into(), json!(u));
    }
    v
}

pub fn move_entry(channel: ChannelId, position: i32, parent: Option<Option<ChannelId>>) -> Value {
    let mut v = json!({ "id": channel, "position": position });
    if let Some(p) = parent {
        v["parent_id"] = json!(p);
        v["lock_permissions"] = json!(false);
    }
    v
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventPlace {
    Voice(ChannelId),
    Stage(ChannelId),
    External(String),
}

pub fn event_body(name: &str, description: &str, start_unix: u64, end_unix: Option<u64>, place: &EventPlace) -> Value {
    let mut v = json!({
        "name": name.trim(),
        "privacy_level": 2,
        "scheduled_start_time": rfc3339(start_unix),
        "description": description.trim(),
    });
    match place {
        EventPlace::Voice(c) => {
            v["entity_type"] = json!(2);
            v["channel_id"] = json!(c);
        }
        EventPlace::Stage(c) => {
            v["entity_type"] = json!(1);
            v["channel_id"] = json!(c);
        }
        EventPlace::External(loc) => {
            v["entity_type"] = json!(3);
            v["channel_id"] = Value::Null;
            v["entity_metadata"] = json!({ "location": loc.trim() });
            v["scheduled_end_time"] = json!(rfc3339(end_unix.unwrap_or(start_unix + 3600)));
        }
    }
    if let (Some(e), false) = (end_unix, matches!(place, EventPlace::External(_))) {
        v["scheduled_end_time"] = json!(rfc3339(e));
    }
    v
}

impl RestClient {
    pub async fn clone_channel(&self, guild: GuildId, src: &Channel, name: &str) -> Result<Channel> {
        self.call(Method::POST, &format!("/guilds/{guild}/channels"), Body::Json(clone_channel_body(src, name))).await
    }

    pub async fn move_channels(&self, guild: GuildId, entries: Vec<Value>) -> Result<()> {
        self.call_empty(Method::PATCH, &format!("/guilds/{guild}/channels"), Body::Json(Value::Array(entries))).await
    }

    pub async fn scheduled_events(&self, guild: GuildId) -> Result<Vec<Value>> {
        self.call(Method::GET, &format!("/guilds/{guild}/scheduled-events?with_user_count=true"), Body::None).await
    }

    pub async fn guild_onboarding(&self, guild: GuildId) -> Result<Value> {
        self.call(Method::GET, &format!("/guilds/{guild}/onboarding"), Body::None).await
    }

    pub async fn guild_home_settings(&self, guild: GuildId) -> Result<Value> {
        self.call(Method::GET, &format!("/guilds/{guild}/new-member-welcome"), Body::None).await
    }

    pub async fn guild_profile(&self, guild: GuildId) -> Result<Value> {
        self.call(Method::GET, &format!("/guilds/{guild}/profile"), Body::None).await
    }

    pub async fn create_scheduled_event(&self, guild: GuildId, body: Value) -> Result<Snowflake> {
        self.post_for_id(&format!("/guilds/{guild}/scheduled-events"), body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_body() {
        let src: Channel = serde_json::from_value(json!({
            "id": "1", "type": 0, "name": "general", "topic": "hi", "parent_id": "9", "nsfw": false,
            "rate_limit_per_user": 5, "permission_overwrites": [{"id": "2", "type": 0, "allow": "1024", "deny": "0"}]
        }))
        .unwrap();
        let v = clone_channel_body(&src, "general");
        assert_eq!(v["type"], 0);
        assert_eq!(v["topic"], "hi");
        assert_eq!(v["parent_id"], "9");
        assert_eq!(v["rate_limit_per_user"], 5);
        assert_eq!(v["permission_overwrites"][0]["id"], "2");
        assert!(v.get("bitrate").is_none());
    }

    #[test]
    fn moves() {
        assert_eq!(move_entry(Snowflake(1), 0, None), json!({"id": "1", "position": 0}));
        let v = move_entry(Snowflake(1), 3, Some(Some(Snowflake(2))));
        assert_eq!(v["parent_id"], "2");
        assert_eq!(move_entry(Snowflake(1), 3, Some(None))["parent_id"], Value::Null);
    }

    #[test]
    fn events() {
        let v = event_body(" Party ", "", 0, None, &EventPlace::External("Paris".into()));
        assert_eq!(v["name"], "Party");
        assert_eq!(v["entity_type"], 3);
        assert_eq!(v["entity_metadata"]["location"], "Paris");
        assert_eq!(v["scheduled_end_time"], "1970-01-01T01:00:00Z");
        let v = event_body("x", "d", 60, None, &EventPlace::Voice(Snowflake(7)));
        assert_eq!(v["entity_type"], 2);
        assert_eq!(v["channel_id"], "7");
        assert!(v.get("scheduled_end_time").is_none());
        assert_eq!(v["privacy_level"], 2);
    }
}
