use serde::{Deserialize, Serialize};

use super::{Permissions, RoleId, Snowflake, null_default};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RoleTags {
    #[serde(default)]
    pub bot_id: Option<Snowflake>,
    #[serde(default)]
    pub integration_id: Option<Snowflake>,
    #[serde(default)]
    pub subscription_listing_id: Option<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Role {
    pub id: RoleId,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub color: u32,
    #[serde(default, deserialize_with = "null_default")]
    pub hoist: bool,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub unicode_emoji: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub position: i32,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default, deserialize_with = "null_default")]
    pub managed: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub mentionable: bool,
    #[serde(default)]
    pub flags: u64,
    #[serde(default)]
    pub tags: Option<RoleTags>,
    #[serde(default)]
    pub colors: Option<RoleColors>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RoleColors {
    #[serde(default, deserialize_with = "null_default")]
    pub primary_color: u32,
    #[serde(default)]
    pub secondary_color: Option<u32>,
    #[serde(default)]
    pub tertiary_color: Option<u32>,
}
