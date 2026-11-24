use serde::{Deserialize, Serialize};

use super::{Snowflake, UserId, null_default};

pub const CDN_BASE: &str = "https://cdn.discordapp.com";

/// A Discord user. Fields only present for the current user (email, mfa...)
/// are optional.
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
    /// Server tag the user chose to display ("GLAD"), if any.
    #[serde(default)]
    pub primary_guild: Option<Box<PrimaryGuild>>,
    /// Profile collectibles (`{"nameplate": {"asset", "palette", ...}}`).
    #[serde(default)]
    pub collectibles: Option<Box<serde_json::Value>>,
    /// Display name style (font, effect, colors) chosen by the user.
    #[serde(default)]
    pub display_name_styles: Option<Box<DisplayNameStyles>>,
    // Current-user only:
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub mfa_enabled: Option<bool>,
    #[serde(default)]
    pub premium_type: Option<u8>,
    #[serde(default)]
    pub phone: Option<String>,
    /// Adult age group (Account Standing > Age group).
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

impl User {
    /// Static image of the nameplate shown behind the name in the DM list
    /// and member list (`collectibles.nameplate.asset`), with its palette.
    pub fn nameplate(&self) -> Option<(String, Option<String>)> {
        nameplate_from_collectibles(self.collectibles.as_deref())
    }
}

/// `user.display_name_styles`: `font_id` (`DisplayNameFont`), `effect_id`
/// (`DisplayNameEffect`: 1 solid, 2 gradient, 3 neon, 4 toon, 5 pop, 6 glow,
/// 7 prism, 8 gummy) and up to two 0xRRGGBB colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DisplayNameStyles {
    #[serde(default)]
    pub font_id: u32,
    #[serde(default)]
    pub effect_id: u32,
    #[serde(default, deserialize_with = "null_default")]
    pub colors: Vec<u32>,
}

/// `user.primary_guild`: the server tag shown next to a user's name.
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
    /// The tag to show, if it is enabled.
    pub fn shown_tag(&self) -> Option<&str> {
        (self.identity_enabled != Some(false)).then_some(())?;
        self.tag.as_deref().filter(|t| !t.is_empty())
    }

    /// CDN URL of the tag badge.
    pub fn badge_url(&self) -> Option<String> {
        let (g, b) = (self.identity_guild_id?, self.badge.as_ref()?);
        Some(format!("{CDN_BASE}/guild-tag-badges/{g}/{b}.png?size=16"))
    }
}

impl User {
    /// Name to display: global name, falling back to username.
    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.username)
    }

    /// `username` or legacy `username#1234`.
    pub fn tag(&self) -> String {
        if self.discriminator.is_empty() || self.discriminator == "0" {
            self.username.clone()
        } else {
            format!("{}#{}", self.username, self.discriminator)
        }
    }

    /// Avatar URL (or the default embed avatar when none is set).
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

/// An entry in the user's relationship list (friends, blocks, requests).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: RelationshipType,
    /// Present unless the READY payload deduplicated users.
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub nickname: Option<String>,
    /// ISO timestamp the relationship started (friends: "Friends Since").
    #[serde(default)]
    pub since: Option<String>,
    /// "Ignore" (the client's RelationshipStore ignored set).
    #[serde(default)]
    pub user_ignored: bool,
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
}
