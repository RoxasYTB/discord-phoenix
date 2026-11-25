use serde::{Deserialize, Serialize};

use super::{Snowflake, UserId, null_default};

pub const CDN_BASE: &str = "https://cdn.discordapp.com";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct User {
    pub id: UserId,
    #[serde(default, deserialize_with = "null_default")]
    pub username: String,
    #[serde(default, deserialize_with = "null_default")]
    pub discriminator: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    #[serde(default, deserialize_with = "null_default")]
    pub bot: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub system: bool,
    #[serde(default)]
    pub public_flags: Option<u64>,
    #[serde(default)]
    pub avatar_decoration_data: Option<serde_json::Value>,
    #[serde(default)]
    pub primary_guild: Option<Box<PrimaryGuild>>,
    #[serde(default)]
    pub collectibles: Option<Box<serde_json::Value>>,
    #[serde(default)]
    pub display_name_styles: Option<Box<DisplayNameStyles>>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub mfa_enabled: Option<bool>,
    #[serde(default)]
    pub premium_type: Option<u8>,
    #[serde(default)]
    pub premium_state: Option<serde_json::Value>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub nsfw_allowed: Option<bool>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub flags: Option<u64>,
    #[serde(default)]
    pub bio: Option<String>,
}

pub(crate) fn nameplate_from_collectibles(collectibles: Option<&serde_json::Value>) -> Option<(String, Option<String>)> {
    let n = collectibles?.get("nameplate")?;
    let palette = n.get("palette").and_then(|p| p.as_str()).map(str::to_owned);
    if let Some(asset) = n.get("asset").and_then(|a| a.as_str()).filter(|a| !a.is_empty()) {
        let sep = if asset.ends_with('/') { "" } else { "/" };
        return Some((format!("https://cdn.discordapp.com/assets/collectibles/{asset}{sep}static.png"), palette));
    }
    let sku_id = n.get("sku_id").and_then(|s| s.as_str()).filter(|s| !s.is_empty())?;
    Some((format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku_id}/static"), palette))
}

pub const PREMIUM_SOURCE_SUBSCRIPTION_GROUP: u64 = 4;
pub const PREMIUM_GROUP_ROLE_MEMBER: u64 = 2;

impl User {
    pub fn is_premium_group_member(&self) -> bool {
        let Some(p) = self.premium_state.as_ref() else { return false };
        self.premium_type == Some(2)
            && p.get("premium_source").and_then(serde_json::Value::as_u64) == Some(PREMIUM_SOURCE_SUBSCRIPTION_GROUP)
            && p.get("premium_subscription_group_role").and_then(serde_json::Value::as_u64) == Some(PREMIUM_GROUP_ROLE_MEMBER)
    }

    pub fn nameplate(&self) -> Option<(String, Option<String>)> {
        nameplate_from_collectibles(self.collectibles.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DisplayNameStyles {
    #[serde(default)]
    pub font_id: u32,
    #[serde(default)]
    pub effect_id: u32,
    #[serde(default, deserialize_with = "null_default")]
    pub colors: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrimaryGuild {
    #[serde(default)]
    pub identity_guild_id: Option<Snowflake>,
    #[serde(default)]
    pub identity_enabled: Option<bool>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub badge: Option<String>,
}

impl PrimaryGuild {
    pub fn shown_tag(&self) -> Option<&str> {
        (self.identity_enabled != Some(false)).then_some(())?;
        self.tag.as_deref().filter(|t| !t.is_empty())
    }

    pub fn badge_url(&self) -> Option<String> {
        let (g, b) = (self.identity_guild_id?, self.badge.as_ref()?);
        Some(format!("{CDN_BASE}/guild-tag-badges/{g}/{b}.png?size=16"))
    }
}

impl User {
    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.username)
    }

    pub fn tag(&self) -> String {
        if self.discriminator.is_empty() || self.discriminator == "0" {
            self.username.clone()
        } else {
            format!("{}#{}", self.username, self.discriminator)
        }
    }

    pub fn avatar_url(&self, size: u32) -> String {
        match &self.avatar {
            Some(hash) => {
                let ext = if hash.starts_with("a_") { "gif" } else { "png" };
                format!("{CDN_BASE}/avatars/{}/{hash}.{ext}?size={size}", self.id)
            }
            None => {
                let idx = if self.discriminator.is_empty() || self.discriminator == "0" {
                    (self.id.0 >> 22) % 6
                } else {
                    self.discriminator.parse::<u64>().unwrap_or(0) % 5
                };
                format!("{CDN_BASE}/embed/avatars/{idx}.png")
            }
        }
    }
}

int_enum! {
    pub enum RelationshipType: u8 {
        None = 0,
        Friend = 1,
        Blocked = 2,
        IncomingRequest = 3,
        OutgoingRequest = 4,
        Implicit = 5,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: RelationshipType,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub user_ignored: bool,
    #[serde(default)]
    pub is_spam_request: bool,
    #[serde(default)]
    pub note: Option<String>,
}

#[cfg(test)]
mod nameplate_tests {
    use super::User;

    #[test]
    fn nameplate_from_collectibles() {
        let u: User = serde_json::from_str(
            r#"{"id":"1","username":"a","discriminator":"0","collectibles":{"nameplate":{"asset":"nameplates/nameplates/test/","palette":"crimson","sku_id":"2"}}}"#,
        )
        .unwrap();
        let (url, pal) = u.nameplate().unwrap();
        assert_eq!(url, "https://cdn.discordapp.com/assets/collectibles/nameplates/nameplates/test/static.png");
        assert_eq!(pal.as_deref(), Some("crimson"));
        let none: User = serde_json::from_str(r#"{"id":"1","username":"a","discriminator":"0","collectibles":null}"#).unwrap();
        assert!(none.nameplate().is_none());
    }

    #[test]
    fn premium_group_member_from_premium_state() {
        let m: User = serde_json::from_str(r#"{"id":"1","username":"a","discriminator":"0","premium_type":2,"premium_state":{"premium_source":4,"premium_subscription_group_role":2}}"#).unwrap();
        assert!(m.is_premium_group_member());
        let owner: User = serde_json::from_str(r#"{"id":"1","username":"a","discriminator":"0","premium_type":2,"premium_state":{"premium_source":4,"premium_subscription_group_role":1}}"#).unwrap();
        assert!(!owner.is_premium_group_member());
        let solo: User = serde_json::from_str(r#"{"id":"1","username":"a","discriminator":"0","premium_type":2}"#).unwrap();
        assert!(!solo.is_premium_group_member());
    }
}
