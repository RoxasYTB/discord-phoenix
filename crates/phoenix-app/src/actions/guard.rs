use reqwest::Method;

pub use phoenix_api::rest::guard::{GuardedWrite, agent_session, blocked, read_only, set_writes_enabled, writes_enabled};

use super::ActionRequest;
use crate::commands::ParityRequest;

fn action_method(req: &ActionRequest) -> Option<Method> {
    match req {
        ActionRequest::Admin(c) => Some(phoenix_api::rest::guild_manage::build(c).method),
        ActionRequest::Reminders | ActionRequest::ReportMenu(_) => Some(Method::GET),
        _ => None,
    }
}

pub fn read_only_action(req: &ActionRequest) -> bool {
    action_method(req).is_some_and(|m| read_only(&m))
}

pub fn read_only_write(w: &GuardedWrite) -> bool {
    read_only(&w.method)
}

pub fn read_only_parity(req: &ParityRequest) -> bool {
    matches!(
        req,
        ParityRequest::ForumPosts { .. }
            | ParityRequest::ContentInventory
            | ParityRequest::ScheduledEvents { .. }
            | ParityRequest::Onboarding { .. }
            | ParityRequest::ServerGuide { .. }
            | ParityRequest::FetchThread { .. }
            | ParityRequest::ArchivedThreads { .. }
            | ParityRequest::Mentions { .. }
            | ParityRequest::SavedMessages
            | ParityRequest::ScheduledMessages
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_writes_stay_off_by_default_and_examples_never_enable_them() {
        assert!(!crate::ui::settings::LocalPrefs::default().protected_writes());
        assert!(!writes_enabled());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "rs") {
                let src = std::fs::read_to_string(&path).unwrap();
                assert!(!src.contains("set_writes_enabled"), "{} enables protected writes", path.display());
                seen += 1;
            }
        }
        assert!(seen > 0);
    }
    use crate::{commands::ModAction, model::Id};
    use phoenix_api::{models::Snowflake, rest::guild_manage::AdminCall};

    const G: Snowflake = Snowflake(1);
    const U: Snowflake = Snowflake(2);

    #[test]
    fn mod_view_reads_pass() {
        for c in [AdminCall::Member { guild: G, user: U }, AdminCall::AuditLogTarget { guild: G, target: U, limit: 50 }, AdminCall::Roles { guild: G }] {
            assert!(read_only_action(&ActionRequest::Admin(c)));
        }
    }

    #[test]
    fn mod_view_writes_are_refused() {
        for c in [
            AdminCall::Kick { guild: G, user: U },
            AdminCall::Ban { guild: G, user: U, delete_message_seconds: 0 },
            AdminCall::MemberSupplemental { guild: G, users: vec![U] },
            AdminCall::AddRoleMember { guild: G, role: Snowflake(3), user: U },
            AdminCall::FollowChannel { channel: Snowflake(4), webhook_channel: Snowflake(5) },
        ] {
            assert!(!read_only_action(&ActionRequest::Admin(c)));
        }
        assert!(!read_only_action(&ActionRequest::SetMemberRoles { guild: Id(1), user: Id(2), roles: vec![] }));
        assert!(!read_only_action(&ActionRequest::SetRole { guild: Id(1), user: Id(2), role: Id(3), add: true }));
        for action in [ModAction::Kick, ModAction::Ban(0), ModAction::Timeout(60), ModAction::Timeout(0), ModAction::Unban] {
            assert!(!read_only_parity(&ParityRequest::Moderate { guild: Id(1), user: Id(2), action }));
        }
        assert!(read_only_parity(&ParityRequest::SavedMessages));
    }

    #[test]
    fn guarded_writes_share_the_same_policy() {
        for w in [
            GuardedWrite::change_password("a", "b"),
            GuardedWrite::wishlist_add_sku("1"),
            GuardedWrite::crosspost(3, 4),
            GuardedWrite::put_onboarding(5, serde_json::json!({})),
            GuardedWrite::put_home_settings(5, serde_json::json!({})),
            GuardedWrite::change_call_region(5, "rotterdam"),
            GuardedWrite::automod_alert_action(5, 6, 7, 3),
            GuardedWrite::automod_clear_mention_raid(5),
        ] {
            assert!(!read_only_write(&w));
            assert!(w.refusal().is_write_blocked());
        }
        let kick = phoenix_api::rest::guild_manage::build(&AdminCall::Kick { guild: G, user: U });
        assert!(!read_only(&kick.method));
        assert!(blocked(&kick.method, "/guilds/1/members/2").is_write_blocked());
    }
}
