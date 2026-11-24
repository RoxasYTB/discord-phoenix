use reqwest::Method;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, GuildId, MessageId, UserId};

pub const MAX_TIMEOUT_SECS: u64 = 28 * 24 * 3600;

pub fn timeout_body(now_unix: u64, secs: u64) -> Value {
    if secs == 0 {
        return json!({ "communication_disabled_until": null });
    }
    let until = now_unix + secs.min(MAX_TIMEOUT_SECS);
    json!({ "communication_disabled_until": rfc3339(until) })
}

pub(crate) fn rfc3339(unix: u64) -> String {
    let (days, rem) = ((unix / 86_400) as i64, unix % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem / 60 % 60, rem % 60)
}

impl RestClient {
    pub async fn timeout_member(&self, guild: GuildId, user: UserId, secs: u64) -> Result<()> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.call_empty(Method::PATCH, &format!("/guilds/{guild}/members/{user}"), Body::Json(timeout_body(now, secs))).await
    }

    pub async fn kick_member(&self, guild: GuildId, user: UserId) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/guilds/{guild}/members/{user}"), Body::None).await
    }

    pub async fn ban_member(&self, guild: GuildId, user: UserId, delete_message_seconds: u32) -> Result<()> {
        self.call_empty(
            Method::PUT,
            &format!("/guilds/{guild}/bans/{user}"),
            Body::Json(json!({ "delete_message_seconds": delete_message_seconds.min(604_800) })),
        )
        .await
    }

    pub async fn unban_member(&self, guild: GuildId, user: UserId) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/guilds/{guild}/bans/{user}"), Body::None).await
    }

    pub async fn bulk_delete_messages(&self, channel: ChannelId, ids: &[MessageId]) -> Result<()> {
        self.call_empty(Method::POST, &format!("/channels/{channel}/messages/bulk-delete"), Body::Json(json!({ "messages": ids }))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_bodies() {
        assert_eq!(timeout_body(0, 0), json!({"communication_disabled_until": null}));
        assert_eq!(timeout_body(0, 60), json!({"communication_disabled_until": "1970-01-01T00:01:00Z"}));
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(timeout_body(0, u64::MAX / 2)["communication_disabled_until"], "1970-01-29T00:00:00Z");
    }
}
