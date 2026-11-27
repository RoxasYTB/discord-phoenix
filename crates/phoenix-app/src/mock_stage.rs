use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use phoenix_api::rest::stage::StageCall;
use serde_json::json;

use crate::model::{Channel, ChannelKind, Id, VoiceFlags};

pub const ENV: &str = "PHX_MOCK_STAGE";
pub const GUILD: Id = Id(100);
pub const USERS: [Id; 7] = [Id(5), Id(8), Id(9), Id(10), Id(11), Id(12), Id(13)];
pub const TOPIC: &str = "Phoenix office hours";
const INSTANCE: &str = "1";

static FLAGS: Mutex<Option<HashMap<Id, VoiceFlags>>> = Mutex::new(None);

fn flags() -> MutexGuard<'static, Option<HashMap<Id, VoiceFlags>>> {
    FLAGS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn enabled() -> bool {
    std::env::var_os(ENV).is_some_and(|v| !v.is_empty() && v != "0")
}

pub fn initial(user: Id) -> VoiceFlags {
    match user.0 {
        5 => VoiceFlags::default(),
        8 => VoiceFlags { self_mute: true, ..Default::default() },
        9 => VoiceFlags { suppress: true, hand: Some(1_764_237_600_000), ..Default::default() },
        10 => VoiceFlags { suppress: true, hand: Some(1_764_237_540_000), ..Default::default() },
        11 => VoiceFlags { hand: Some(1_764_237_660_000), ..Default::default() },
        _ => VoiceFlags { suppress: true, ..Default::default() },
    }
}

pub fn channel(channels: &[Channel]) -> Option<Id> {
    channels.iter().find(|c| c.guild == Some(GUILD) && c.kind == ChannelKind::Stage).map(|c| c.id)
}

fn instance(stage: Id, topic: &str) -> serde_json::Value {
    json!({ "id": INSTANCE, "guild_id": GUILD.0.to_string(), "channel_id": stage.0.to_string(), "topic": topic, "privacy_level": 2 })
}

pub fn seed(stage: Id) -> Vec<(Id, VoiceFlags)> {
    let all: Vec<(Id, VoiceFlags)> = USERS.iter().map(|u| (*u, initial(*u))).collect();
    *flags() = Some(all.iter().copied().collect());
    crate::guild_live::load(GUILD.0, &[instance(stage, TOPIC)], &[], &[]);
    all
}

pub fn join(me: Id) -> VoiceFlags {
    let f = VoiceFlags { suppress: true, ..Default::default() };
    flags().get_or_insert_with(HashMap::new).insert(me, f);
    f
}

pub fn apply(state: &mut HashMap<Id, VoiceFlags>, me: Id, call: &StageCall, now_ms: i64) -> Vec<(Id, VoiceFlags)> {
    let user = match call {
        StageCall::RequestToSpeak { .. } | StageCall::SetSelfSuppress { .. } | StageCall::MoveSelfToAudience { .. } => me,
        StageCall::SetUserSuppress { user, .. } | StageCall::InviteToSpeak { user, .. } | StageCall::MoveUserToAudience { user, .. } => Id(user.0),
        StageCall::Start { .. } | StageCall::Edit { .. } | StageCall::End { .. } => return Vec::new(),
    };
    let f = state.entry(user).or_default();
    match call {
        StageCall::RequestToSpeak { at, .. } => f.hand = at.as_ref().map(|_| now_ms),
        StageCall::SetSelfSuppress { suppress, .. } => {
            f.suppress = *suppress;
            f.hand = None;
        }
        StageCall::SetUserSuppress { suppress: false, .. } => f.suppress = false,
        StageCall::InviteToSpeak { .. } => {
            f.suppress = false;
            f.hand = Some(now_ms);
        }
        StageCall::MoveSelfToAudience { .. } => {
            f.suppress = true;
            f.video = false;
            f.stream = false;
        }
        _ => {
            f.suppress = true;
            f.hand = None;
            f.video = false;
            f.stream = false;
        }
    }
    vec![(user, *f)]
}

pub fn on_call(me: Id, call: &StageCall) -> Vec<(Id, VoiceFlags)> {
    match call {
        StageCall::Start { channel, topic, .. } => {
            crate::guild_live::on_event("STAGE_INSTANCE_CREATE", &instance(Id(channel.0), topic));
        }
        StageCall::Edit { channel, topic, .. } => {
            crate::guild_live::on_event("STAGE_INSTANCE_UPDATE", &instance(Id(channel.0), topic));
        }
        StageCall::End { .. } => {
            crate::guild_live::on_event("STAGE_INSTANCE_DELETE", &json!({ "id": INSTANCE, "guild_id": GUILD.0.to_string() }));
        }
        _ => {}
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    apply(flags().get_or_insert_with(HashMap::new), me, call, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::stage::{Rts, rts};
    use phoenix_api::models::Snowflake;

    #[test]
    fn demo_stage_has_speakers_requests_and_listeners() {
        let states: Vec<Rts> = USERS.iter().map(|u| rts(initial(*u))).collect();
        assert_eq!(states.iter().filter(|s| **s == Rts::OnStage).count(), 2);
        assert_eq!(states.iter().filter(|s| **s == Rts::Requested).count(), 2);
        assert_eq!(states.iter().filter(|s| **s == Rts::Awaiting).count(), 1);
        assert_eq!(states.iter().filter(|s| **s == Rts::None).count(), 2);
    }

    #[test]
    fn calls_move_people_between_audience_and_stage() {
        let (g, c, me) = (Snowflake(100), Snowflake(7), Id(1));
        let mut st: HashMap<Id, VoiceFlags> = HashMap::from([(me, VoiceFlags { suppress: true, ..Default::default() }), (Id(9), initial(Id(9)))]);
        let out = apply(&mut st, me, &StageCall::RequestToSpeak { guild: g, channel: c, at: Some("t".into()) }, 42);
        assert_eq!(rts(out[0].1), Rts::Requested);
        let out = apply(&mut st, me, &StageCall::SetUserSuppress { guild: g, channel: c, user: Snowflake(9), suppress: false }, 43);
        assert_eq!((out[0].0, rts(out[0].1)), (Id(9), Rts::Awaiting));
        let out = apply(&mut st, me, &StageCall::SetSelfSuppress { guild: g, channel: c, suppress: false, silent: false }, 44);
        assert_eq!(rts(out[0].1), Rts::OnStage);
        let out = apply(&mut st, me, &StageCall::MoveSelfToAudience { guild: g, channel: c }, 45);
        assert_eq!(rts(out[0].1), Rts::None);
        assert!(apply(&mut st, me, &StageCall::End { channel: c }, 46).is_empty());
    }
}
