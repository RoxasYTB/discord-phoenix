use serde::{Deserialize, Serialize};

use super::{GuildId, Permissions, RoleId, User, UserId, null_default};

/// A guild member. With `DEDUPE_USER_OBJECTS` the READY payload sends
/// `user_id` instead of a full `user`; use [`Member::user_id`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Member {
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default, rename = "user_id")]
    pub user_id_field: Option<UserId>,
    #[serde(default)]
    pub guild_id: Option<GuildId>,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    /// Per-guild profile banner hash.
    #[serde(default)]
    pub banner: Option<String>,
    /// Per-guild avatar decoration (`{asset, sku_id}`), overrides the user's.
    #[serde(default)]
    pub avatar_decoration_data: Option<serde_json::Value>,
    /// Per-guild collectibles (`{nameplate: {asset, palette, ...}}`), overrides the user's.
    #[serde(default)]
    pub collectibles: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "null_default")]
    pub roles: Vec<RoleId>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub premium_since: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub deaf: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub mute: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub flags: u64,
    #[serde(default)]
    pub pending: Option<bool>,
    #[serde(default)]
    pub communication_disabled_until: Option<String>,
    /// Only on interaction/some REST payloads.
    #[serde(default)]
    pub permissions: Option<Permissions>,
}

impl Member {
    pub fn user_id(&self) -> Option<UserId> {
        self.user.as_ref().map(|u| u.id).or(self.user_id_field)
    }

    pub fn display_name(&self) -> Option<&str> {
        self.nick
            .as_deref()
            .or_else(|| self.user.as_ref().map(|u| u.display_name()))
    }

    /// Per-guild nameplate (`collectibles.nameplate`), overrides the user's.
    pub fn nameplate(&self) -> Option<(String, Option<String>)> {
        super::user::nameplate_from_collectibles(self.collectibles.as_ref())
    }
}
