use reqwest::Method;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, GuildId, Snowflake, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceMod {
    Mute(bool),
    Deafen(bool),
    Move(Option<ChannelId>),
}

pub fn voice_mod_body(m: VoiceMod) -> Value {
    match m {
        VoiceMod::Mute(b) => json!({ "mute": b }),
        VoiceMod::Deafen(b) => json!({ "deaf": b }),
        VoiceMod::Move(c) => json!({ "channel_id": c }),
    }
}

pub fn nick_body(nick: &str) -> Value {
    let n = nick.trim();
    if n.is_empty() { json!({ "nick": null }) } else { json!({ "nick": n.chars().take(32).collect::<String>() }) }
}

pub fn member_path(guild: GuildId, user: Option<UserId>) -> String {
    match user {
        Some(u) => format!("/guilds/{guild}/members/{u}"),
        None => format!("/guilds/{guild}/members/@me"),
    }
}

pub fn friend_nick_body(nick: &str) -> Value {
    let n = nick.trim();
    json!({ "nickname": if n.is_empty() { Value::Null } else { json!(n) } })
}

impl RestClient {
    pub async fn add_member_role(&self, guild: GuildId, user: UserId, role: Snowflake) -> Result<()> {
        self.call_empty(Method::PUT, &format!("/guilds/{guild}/members/{user}/roles/{role}"), Body::None).await
    }

    pub async fn remove_member_role(&self, guild: GuildId, user: UserId, role: Snowflake) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/guilds/{guild}/members/{user}/roles/{role}"), Body::None).await
    }

    pub async fn set_member_nick(&self, guild: GuildId, user: Option<UserId>, nick: &str) -> Result<()> {
        self.call_empty(Method::PATCH, &member_path(guild, user), Body::Json(nick_body(nick))).await
    }

    pub async fn set_friend_nickname(&self, user: UserId, nick: &str) -> Result<()> {
        self.call_empty(Method::PATCH, &format!("/users/@me/relationships/{user}"), Body::Json(friend_nick_body(nick))).await
    }

    pub async fn set_ignored(&self, user: UserId, ignored: bool) -> Result<()> {
        let m = if ignored { Method::PUT } else { Method::DELETE };
        self.call_empty(m, &format!("/users/@me/relationships/{user}/ignore"), Body::None).await
    }

    pub async fn voice_moderate(&self, guild: GuildId, user: UserId, m: VoiceMod) -> Result<()> {
        self.call_empty(Method::PATCH, &member_path(guild, Some(user)), Body::Json(voice_mod_body(m))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies() {
        assert_eq!(nick_body("  "), json!({"nick": null}));
        assert_eq!(nick_body(" Bob "), json!({"nick": "Bob"}));
        assert_eq!(nick_body(&"a".repeat(40))["nick"].as_str().unwrap().len(), 32);
        assert_eq!(friend_nick_body(""), json!({"nickname": null}));
        assert_eq!(friend_nick_body("Al"), json!({"nickname": "Al"}));
        assert_eq!(voice_mod_body(VoiceMod::Mute(true)), json!({"mute": true}));
        assert_eq!(voice_mod_body(VoiceMod::Deafen(false)), json!({"deaf": false}));
        assert_eq!(voice_mod_body(VoiceMod::Move(None)), json!({"channel_id": null}));
        assert_eq!(voice_mod_body(VoiceMod::Move(Some(Snowflake(4)))), json!({"channel_id": "4"}));
    }

    #[test]
    fn paths() {
        assert_eq!(member_path(Snowflake(1), None), "/guilds/1/members/@me");
        assert_eq!(member_path(Snowflake(1), Some(Snowflake(2))), "/guilds/1/members/2");
    }
}
