use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, GuildId, Message, MessageId, null_default};

#[derive(Debug, Clone, Copy)]
pub struct MentionsQuery {
    pub limit: u8,
    pub before: Option<MessageId>,
    pub roles: bool,
    pub everyone: bool,
    pub guild: Option<GuildId>,
}

impl Default for MentionsQuery {
    fn default() -> Self {
        Self { limit: 25, before: None, roles: true, everyone: true, guild: None }
    }
}

pub(crate) fn mentions_path(q: MentionsQuery) -> String {
    let mut s = format!(
        "/users/@me/mentions?limit={}&roles={}&everyone={}",
        q.limit.clamp(1, 100),
        q.roles,
        q.everyone
    );
    if let Some(b) = q.before {
        s.push_str(&format!("&before={b}"));
    }
    if let Some(g) = q.guild {
        s.push_str(&format!("&guild_id={g}"));
    }
    s
}

#[derive(Debug, Clone, Deserialize)]
pub struct SavedMessage {
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default)]
    pub save_data: Value,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SavedMessages {
    #[serde(default, deserialize_with = "null_default")]
    pub results: Vec<SavedMessage>,
}

impl RestClient {
    pub async fn get_mentions(&self, q: MentionsQuery) -> Result<Vec<Message>> {
        self.call(Method::GET, &mentions_path(q), Body::None).await
    }

    pub async fn dismiss_mention(&self, message: MessageId) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/users/@me/mentions/{message}"), Body::None).await
    }

    pub async fn get_saved_messages(&self) -> Result<SavedMessages> {
        self.call(Method::GET, "/users/@me/saved-messages", Body::None).await
    }

    pub async fn save_message(&self, channel: ChannelId, message: MessageId) -> Result<()> {
        self.call_empty(
            Method::PUT,
            "/users/@me/saved-messages",
            Body::Json(json!({ "channel_id": channel, "message_id": message, "save_data": { "type": 0 } })),
        )
        .await
    }

    pub async fn unsave_message(&self, channel: ChannelId, message: MessageId) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/users/@me/saved-messages/{channel}/{message}"), Body::None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Snowflake;

    #[test]
    fn paths() {
        assert_eq!(mentions_path(MentionsQuery::default()), "/users/@me/mentions?limit=25&roles=true&everyone=true");
        let q = MentionsQuery { before: Some(Snowflake(5)), guild: Some(Snowflake(6)), everyone: false, ..Default::default() };
        assert_eq!(mentions_path(q), "/users/@me/mentions?limit=25&roles=true&everyone=false&before=5&guild_id=6");
    }

    #[test]
    fn parse_saved() {
        let s: SavedMessages = serde_json::from_str(r#"{"results":[{"save_data":{"type":0}}]}"#).unwrap();
        assert_eq!(s.results.len(), 1);
        assert!(s.results[0].message.is_none());
    }
}
