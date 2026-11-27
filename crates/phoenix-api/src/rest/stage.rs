use reqwest::Method;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, GuildId, UserId};

pub const PRIVACY_GUILD_ONLY: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageCall {
    RequestToSpeak { guild: GuildId, channel: ChannelId, at: Option<String> },
    SetSelfSuppress { guild: GuildId, channel: ChannelId, suppress: bool, silent: bool },
    MoveSelfToAudience { guild: GuildId, channel: ChannelId },
    SetUserSuppress { guild: GuildId, channel: ChannelId, user: UserId, suppress: bool },
    InviteToSpeak { guild: GuildId, channel: ChannelId, user: UserId, at: String },
    MoveUserToAudience { guild: GuildId, channel: ChannelId, user: UserId },
    Start { channel: ChannelId, topic: String, privacy_level: u8, send_start_notification: bool },
    Edit { channel: ChannelId, topic: String, privacy_level: u8 },
    End { channel: ChannelId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageRequest {
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
}

fn voice_state_path(guild: GuildId, user: Option<UserId>) -> String {
    match user {
        Some(u) => format!("/guilds/{guild}/voice-states/{u}"),
        None => format!("/guilds/{guild}/voice-states/@me"),
    }
}

fn patch(path: String, body: Value) -> StageRequest {
    StageRequest { method: Method::PATCH, path, body: Some(body) }
}

pub fn build(c: &StageCall) -> StageRequest {
    match c {
        StageCall::RequestToSpeak { guild, channel, at } => {
            patch(voice_state_path(*guild, None), json!({ "request_to_speak_timestamp": at, "channel_id": channel }))
        }
        StageCall::SetSelfSuppress { guild, channel, suppress, silent } => {
            let mut body = json!({ "suppress": suppress, "request_to_speak_timestamp": null, "channel_id": channel });
            if *silent {
                body["silent"] = json!(true);
            }
            patch(voice_state_path(*guild, None), body)
        }
        StageCall::MoveSelfToAudience { guild, channel } => patch(
            voice_state_path(*guild, None),
            json!({ "suppress": true, "channel_id": channel, "self_video": false, "self_stream": false }),
        ),
        StageCall::SetUserSuppress { guild, channel, user, suppress } => {
            patch(voice_state_path(*guild, Some(*user)), json!({ "suppress": suppress, "channel_id": channel }))
        }
        StageCall::InviteToSpeak { guild, channel, user, at } => patch(
            voice_state_path(*guild, Some(*user)),
            json!({ "suppress": false, "request_to_speak_timestamp": at, "channel_id": channel }),
        ),
        StageCall::MoveUserToAudience { guild, channel, user } => patch(
            voice_state_path(*guild, Some(*user)),
            json!({ "suppress": true, "channel_id": channel, "self_video": false, "self_stream": false }),
        ),
        StageCall::Start { channel, topic, privacy_level, send_start_notification } => StageRequest {
            method: Method::POST,
            path: "/stage-instances".into(),
            body: Some(json!({
                "channel_id": channel,
                "topic": topic,
                "privacy_level": privacy_level,
                "guild_scheduled_event_id": null,
                "send_start_notification": send_start_notification,
            })),
        },
        StageCall::Edit { channel, topic, privacy_level } => {
            patch(format!("/stage-instances/{channel}"), json!({ "topic": topic, "privacy_level": privacy_level }))
        }
        StageCall::End { channel } => StageRequest { method: Method::DELETE, path: format!("/stage-instances/{channel}"), body: None },
    }
}

impl RestClient {
    pub async fn stage(&self, c: &StageCall) -> Result<()> {
        let r = build(c);
        let body = r.body.map_or(Body::None, Body::Json);
        self.call_empty(r.method, &r.path, body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Snowflake;

    const G: Snowflake = Snowflake(1);
    const C: Snowflake = Snowflake(2);
    const U: Snowflake = Snowflake(3);

    #[test]
    fn raise_and_lower_hand() {
        let r = build(&StageCall::RequestToSpeak { guild: G, channel: C, at: Some("2026-11-27T10:00:00.000Z".into()) });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/guilds/1/voice-states/@me"));
        assert_eq!(r.body, Some(json!({ "request_to_speak_timestamp": "2026-11-27T10:00:00.000Z", "channel_id": "2" })));
        let r = build(&StageCall::RequestToSpeak { guild: G, channel: C, at: None });
        assert_eq!(r.body, Some(json!({ "request_to_speak_timestamp": null, "channel_id": "2" })));
    }

    #[test]
    fn become_speaker_and_audience() {
        let r = build(&StageCall::SetSelfSuppress { guild: G, channel: C, suppress: false, silent: false });
        assert_eq!(r.path, "/guilds/1/voice-states/@me");
        assert_eq!(r.body, Some(json!({ "suppress": false, "request_to_speak_timestamp": null, "channel_id": "2" })));
        let r = build(&StageCall::SetSelfSuppress { guild: G, channel: C, suppress: false, silent: true });
        assert_eq!(r.body.unwrap()["silent"], json!(true));
        let r = build(&StageCall::MoveSelfToAudience { guild: G, channel: C });
        assert_eq!(r.body, Some(json!({ "suppress": true, "channel_id": "2", "self_video": false, "self_stream": false })));
    }

    #[test]
    fn moderator_calls() {
        let r = build(&StageCall::SetUserSuppress { guild: G, channel: C, user: U, suppress: false });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/guilds/1/voice-states/3"));
        assert_eq!(r.body, Some(json!({ "suppress": false, "channel_id": "2" })));
        let r = build(&StageCall::InviteToSpeak { guild: G, channel: C, user: U, at: "t".into() });
        assert_eq!(r.body, Some(json!({ "suppress": false, "request_to_speak_timestamp": "t", "channel_id": "2" })));
        let r = build(&StageCall::MoveUserToAudience { guild: G, channel: C, user: U });
        assert_eq!(r.path, "/guilds/1/voice-states/3");
        assert_eq!(r.body, Some(json!({ "suppress": true, "channel_id": "2", "self_video": false, "self_stream": false })));
    }

    #[test]
    fn stage_instance_lifecycle() {
        let r = build(&StageCall::Start { channel: C, topic: "Q&A".into(), privacy_level: PRIVACY_GUILD_ONLY, send_start_notification: false });
        assert_eq!((r.method, r.path.as_str()), (Method::POST, "/stage-instances"));
        assert_eq!(
            r.body,
            Some(json!({ "channel_id": "2", "topic": "Q&A", "privacy_level": 2, "guild_scheduled_event_id": null, "send_start_notification": false }))
        );
        let r = build(&StageCall::Edit { channel: C, topic: "New".into(), privacy_level: 2 });
        assert_eq!((r.method, r.path.as_str()), (Method::PATCH, "/stage-instances/2"));
        assert_eq!(r.body, Some(json!({ "topic": "New", "privacy_level": 2 })));
        let r = build(&StageCall::End { channel: C });
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::DELETE, "/stage-instances/2", None));
    }
}
