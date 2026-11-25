use reqwest::Method;

use super::ActionRequest;
use crate::commands::ParityRequest;

pub fn read_only_action(req: &ActionRequest) -> bool {
    match req {
        ActionRequest::Admin(c) => phoenix_api::rest::guild_manage::build(c).method == Method::GET,
        ActionRequest::Reminders | ActionRequest::ReportMenu(_) => true,
        _ => false,
    }
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
}
