use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{GuildId, Snowflake, User, UserId, null_default};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProfileMetadata {
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default, deserialize_with = "null_default")]
    pub bio: String,
    #[serde(default, deserialize_with = "null_default")]
    pub pronouns: String,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    #[serde(default)]
    pub theme_colors: Option<Vec<u32>>,
    #[serde(default, deserialize_with = "null_default")]
    pub collectibles: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProfileGuildMember {
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub avatar_decoration_data: Option<Value>,
    #[serde(default)]
    pub collectibles: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MutualGuild {
    pub id: GuildId,
    #[serde(default)]
    pub nick: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserProfile {
    pub user: User,
    #[serde(default)]
    pub user_profile: Option<ProfileMetadata>,
    #[serde(default)]
    pub guild_member: Option<ProfileGuildMember>,
    #[serde(default)]
    pub guild_member_profile: Option<ProfileMetadata>,
    #[serde(default)]
    pub premium_since: Option<String>,
    #[serde(default)]
    pub premium_type: Option<u8>,
    #[serde(default, deserialize_with = "null_default")]
    pub mutual_guilds: Vec<MutualGuild>,
    #[serde(default)]
    pub mutual_friends_count: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub mutual_friends: Vec<User>,
    #[serde(default, deserialize_with = "null_default")]
    pub badges: Vec<Value>,
    #[serde(default, deserialize_with = "null_default")]
    pub widgets: Vec<Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl UserProfile {
    pub fn bio(&self) -> &str {
        pick(self.guild_member_profile.as_ref().map(|p| p.bio.as_str()), self.user_profile.as_ref().map(|p| p.bio.as_str()))
    }
    pub fn pronouns(&self) -> &str {
        pick(
            self.guild_member_profile.as_ref().map(|p| p.pronouns.as_str()),
            self.user_profile.as_ref().map(|p| p.pronouns.as_str()),
        )
    }
}

fn pick<'a>(guild: Option<&'a str>, global: Option<&'a str>) -> &'a str {
    guild.filter(|s| !s.trim().is_empty()).or(global).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CustomStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji_id: Option<Snowflake>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

pub(crate) fn profile_path(user: UserId, guild: Option<GuildId>) -> String {
    let mut p = format!("/users/{user}/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=true");
    if let Some(g) = guild {
        p.push_str(&format!("&guild_id={g}"));
    }
    p
}

pub(crate) fn status_body(status: Option<&str>, custom: Option<Option<&CustomStatus>>) -> Value {
    let mut v = json!({});
    if let Some(s) = status {
        v["status"] = json!(s);
    }
    if let Some(c) = custom {
        v["custom_status"] = match c {
            Some(c) => serde_json::to_value(c).unwrap_or(Value::Null),
            None => Value::Null,
        };
    }
    v
}

#[derive(Deserialize)]
struct Note {
    #[serde(default, deserialize_with = "null_default")]
    note: String,
}

impl RestClient {
    pub async fn get_user_profile(&self, user: UserId, guild: Option<GuildId>) -> Result<UserProfile> {
        self.call(Method::GET, &profile_path(user, guild), Body::None).await
    }

    pub async fn get_mutual_friends(&self, user: UserId) -> Result<Vec<User>> {
        self.call(Method::GET, &format!("/users/{user}/relationships"), Body::None).await
    }

    pub async fn get_user_note(&self, user: UserId) -> Result<String> {
        let cell = self.inner.notes.lock().entry(user.0).or_default().clone();
        cell.get_or_try_init(|| async {
            match self.call::<Note>(Method::GET, &format!("/users/@me/notes/{user}"), Body::None).await {
                Ok(n) => Ok(n.note),
                Err(crate::Error::Api { status: 404, .. }) => Ok(String::new()),
                Err(e) => Err(e),
            }
        })
        .await
        .cloned()
    }

    pub fn cache_user_note(&self, user: UserId, note: &str) {
        let cell = tokio::sync::OnceCell::new_with(Some(note.to_owned()));
        self.inner.notes.lock().insert(user.0, std::sync::Arc::new(cell));
    }

    pub async fn set_user_note(&self, user: UserId, note: &str) -> Result<()> {
        self.guarded_write(&super::guard::GuardedWrite::set_note(user.0, note)).await?;
        self.cache_user_note(user, note);
        Ok(())
    }

    pub async fn update_status_settings(
        &self,
        status: Option<&str>,
        custom: Option<Option<&CustomStatus>>,
    ) -> Result<()> {
        self.call_empty(Method::PATCH, "/users/@me/settings", Body::Json(status_body(status, custom))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(
            profile_path(Snowflake(7), None),
            "/users/7/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=true"
        );
        assert!(profile_path(Snowflake(7), Some(Snowflake(9))).ends_with("&guild_id=9"));
    }

    #[test]
    fn status_bodies() {
        assert_eq!(status_body(Some("dnd"), None), json!({"status": "dnd"}));
        assert_eq!(status_body(None, Some(None)), json!({"custom_status": null}));
        let c = CustomStatus { text: Some("hi".into()), ..Default::default() };
        assert_eq!(status_body(Some("idle"), Some(Some(&c))), json!({"status": "idle", "custom_status": {"text": "hi"}}));
    }

    #[test]
    fn parse_profile() {
        let p: UserProfile = serde_json::from_str(
            r#"{"user":{"id":"1","username":"a","global_name":"A","bio":"x"},
                "user_profile":{"bio":"global bio","pronouns":"they/them","accent_color":null,"banner":"abc"},
                "guild_member":{"nick":"Nick","roles":["5","6"],"joined_at":"2020-01-01T00:00:00+00:00"},
                "guild_member_profile":{"bio":"","pronouns":"he/him"},
                "mutual_guilds":[{"id":"3","nick":null}],"badges":null,"premium_type":2}"#,
        )
        .unwrap();
        assert_eq!(p.bio(), "global bio");
        assert_eq!(p.pronouns(), "he/him");
        assert_eq!(p.guild_member.as_ref().unwrap().roles, vec![Snowflake(5), Snowflake(6)]);
        assert_eq!(p.mutual_guilds[0].id, Snowflake(3));
        assert!(p.badges.is_empty());
    }
}
