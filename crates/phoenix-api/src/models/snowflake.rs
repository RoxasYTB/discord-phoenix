use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Snowflake(pub u64);

pub type UserId = Snowflake;
pub type GuildId = Snowflake;
pub type ChannelId = Snowflake;
pub type MessageId = Snowflake;
pub type RoleId = Snowflake;
pub type EmojiId = Snowflake;

impl Snowflake {
    pub const fn new(v: u64) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn timestamp_ms(self) -> u64 {
        (self.0 >> 22) + DISCORD_EPOCH_MS
    }

    pub fn created_at(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.timestamp_ms())
    }

    pub const fn from_timestamp_ms(unix_ms: u64) -> Self {
        Self(unix_ms.saturating_sub(DISCORD_EPOCH_MS) << 22)
    }

    pub fn now() -> Self {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(DISCORD_EPOCH_MS);
        Self::from_timestamp_ms(ms)
    }
}

impl fmt::Debug for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snowflake({})", self.0)
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<u64> for Snowflake {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl FromStr for Snowflake {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl Serialize for Snowflake {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Snowflake;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a snowflake as string or integer")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Snowflake, E> {
                Ok(Snowflake(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Snowflake, E> {
                u64::try_from(v)
                    .map(Snowflake)
                    .map_err(|_| E::custom("negative snowflake"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Snowflake, E> {
                v.parse().map(Snowflake).map_err(E::custom)
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_roundtrip() {
        let id = Snowflake(175928847299117063);
        assert_eq!(id.timestamp_ms(), 1462015105796);
        let s = Snowflake::from_timestamp_ms(1462015105796);
        assert_eq!(s.timestamp_ms(), 1462015105796);
    }

    #[test]
    fn serde_forms() {
        let a: Snowflake = serde_json::from_str("\"42\"").unwrap();
        let b: Snowflake = serde_json::from_str("42").unwrap();
        assert_eq!(a, b);
        assert_eq!(serde_json::to_string(&a).unwrap(), "\"42\"");
    }
}
