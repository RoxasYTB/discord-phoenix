use phoenix_voice::gateway::protocol::speaking;

use super::VoiceControl;
use crate::app::PhoenixApp;
use crate::model::Id;

pub const PRIORITY_SPEAKER: u64 = 1 << 8;

pub fn wants_priority(flags: u32) -> bool {
    let both = speaking::MICROPHONE | speaking::PRIORITY;
    flags & both == both
}

pub fn can_have_priority(permissions: Option<u64>) -> bool {
    permissions.is_some_and(|p| p & PRIORITY_SPEAKER != 0)
}

pub fn on_flags(app: &PhoenixApp, user: Id, flags: u32) {
    let on = wants_priority(flags) && app.voice_channel.is_some_and(|c| can_have_priority(app.backend.member_permissions(c, user)));
    super::call::manager().apply_control(None, VoiceControl::SetPrioritySpeaker(user, on));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_needs_voice_and_the_permission() {
        assert!(wants_priority(5));
        assert!(!wants_priority(4));
        assert!(!wants_priority(1));
        assert!(can_have_priority(Some(1 << 8)));
        assert!(!can_have_priority(Some(1 << 3)));
        assert!(!can_have_priority(None));
    }
}
