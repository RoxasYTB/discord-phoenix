use std::sync::Mutex;

use reqwest::Method;
use serde_json::Value;

use crate::error::Error;

pub const SANDBOX: u64 = 1574797385463496724;
pub const GENERAL: u64 = 1403447573389381652;
pub const VOICE: u64 = 1574900893613039696;

const CHANNEL_FIELDS: &[&str] = &["rate_limit_per_user", "topic"];
const THREAD_FIELDS: &[&str] = &["archived", "locked", "rate_limit_per_user"];

static THREADS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static SCHEDULED: Mutex<Vec<u64>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Sandbox,
    Thread,
    General,
    Voice,
}

fn scope(id: u64) -> Option<Scope> {
    match id {
        SANDBOX => Some(Scope::Sandbox),
        GENERAL => Some(Scope::General),
        VOICE => Some(Scope::Voice),
        _ if THREADS.lock().is_ok_and(|t| t.contains(&id)) => Some(Scope::Thread),
        _ => None,
    }
}

fn remember(set: &Mutex<Vec<u64>>, id: u64) {
    if let Ok(mut t) = set.lock()
        && !t.contains(&id)
    {
        t.push(id);
    }
}

fn known(set: &Mutex<Vec<u64>>, s: &str) -> bool {
    id(s).is_some_and(|id| set.lock().is_ok_and(|t| t.contains(&id)))
}

pub fn scoped(agent: bool, base: &str) -> bool {
    let rest = base.strip_prefix("http://").unwrap_or(base);
    agent && !(rest.starts_with("127.0.0.1:") || rest.starts_with("localhost:") || rest.starts_with("[::1]:"))
}

fn id(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn body_channel(body: Option<&Value>) -> Option<u64> {
    match body?.get("channel_id")? {
        Value::String(s) => id(s),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

fn fields_within(body: Option<&Value>, allowed: &[&str]) -> bool {
    body.and_then(Value::as_object).is_some_and(|o| !o.is_empty() && o.keys().all(|k| allowed.contains(&k.as_str())))
}

fn sandboxed(s: Scope) -> bool {
    matches!(s, Scope::Sandbox | Scope::Thread)
}

fn channel(method: &Method, s: Scope, rest: &[&str], body: Option<&Value>) -> bool {
    let m = |x: &[Method]| x.contains(method);
    match rest {
        [] => match s {
            Scope::Sandbox => m(&[Method::PATCH]) && fields_within(body, CHANNEL_FIELDS),
            Scope::Thread => m(&[Method::PATCH]) && fields_within(body, THREAD_FIELDS),
            Scope::General | Scope::Voice => false,
        },
        ["messages"] => m(&[Method::POST]) && s != Scope::Voice,
        ["typing"] | ["attachments"] => m(&[Method::POST]) && s != Scope::Voice,
        ["messages", msg] => m(&[Method::PATCH, Method::DELETE]) && sandboxed(s) && id(msg).is_some(),
        ["messages", msg, "reactions", _, ..] => m(&[Method::PUT, Method::DELETE]) && sandboxed(s) && id(msg).is_some(),
        ["pins", msg] | ["messages", "pins", msg] => m(&[Method::PUT, Method::DELETE]) && sandboxed(s) && id(msg).is_some(),
        ["messages", msg, "threads"] => m(&[Method::POST]) && s == Scope::Sandbox && id(msg).is_some(),
        ["threads"] => m(&[Method::POST]) && s == Scope::Sandbox,
        ["polls", msg, "answers", "@me"] => m(&[Method::PUT]) && sandboxed(s) && id(msg).is_some(),
        ["polls", msg, "expire"] => m(&[Method::POST]) && sandboxed(s) && id(msg).is_some(),
        ["thread-members", "@me"] => m(&[Method::POST, Method::PUT, Method::DELETE]) && s == Scope::Thread,
        ["thread-members", "@me", "settings"] => m(&[Method::PATCH]) && s == Scope::Thread,
        ["send-soundboard-sound"] => m(&[Method::POST]) && s == Scope::Voice,
        _ => false,
    }
}

pub fn allows(method: &Method, path: &str, body: Option<&Value>) -> bool {
    if *method == Method::GET {
        return true;
    }
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let seg: Vec<&str> = path.strip_prefix('/').unwrap_or(path).split('/').collect();
    if seg.iter().any(|s| s.is_empty() || *s == "." || *s == "..") {
        return false;
    }
    let post = *method == Method::POST;
    match seg.as_slice() {
        ["channels", c, rest @ ..] => id(c).and_then(scope).is_some_and(|s| channel(method, s, rest, body)),
        ["interactions"] => post && body_channel(body).and_then(scope).is_some_and(sandboxed),
        ["users", "@me", "scheduled-messages"] => post && body_channel(body) == Some(SANDBOX),
        ["users", "@me", "scheduled-messages", m] => (*method == Method::PATCH || *method == Method::DELETE) && known(&SCHEDULED, m),
        ["users", "@me", "scheduled-messages", m, "send"] => post && known(&SCHEDULED, m),
        ["streams", key, "preview"] => post && key.split(':').any(|p| id(p) == Some(VOICE)),
        ["guilds", g, "members-search"] | ["guilds", g, "members", "supplemental"] => post && id(g).is_some(),
        ["users", "@me", "messages", "search", "tabs"] => post,
        _ => false,
    }
}

pub fn refusal(method: &Method, path: &str, body: Option<&Value>) -> Option<Error> {
    (!allows(method, path, body)).then(|| super::guard::blocked(method, format!("{path} (outside the agent test channels)")))
}

pub fn observe(method: &Method, path: &str, reply: &[u8]) {
    if *method != Method::POST {
        return;
    }
    let path = path.split('?').next().unwrap_or_default();
    let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let thread = matches!(seg.as_slice(), ["channels", c, "threads"] | ["channels", c, "messages", _, "threads"] if id(c) == Some(SANDBOX));
    let scheduled = matches!(seg.as_slice(), ["users", "@me", "scheduled-messages"]);
    if !thread && !scheduled {
        return;
    }
    let Ok(v) = serde_json::from_slice::<Value>(reply) else { return };
    let field = |k: &str| v.get(k).and_then(Value::as_str).and_then(id);
    let Some(new) = field("id") else { return };
    if thread && field("parent_id") == Some(SANDBOX) {
        remember(&THREADS, new);
    }
    if scheduled && field("channel_id") == Some(SANDBOX) {
        remember(&SCHEDULED, new);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ok(m: Method, p: &str) -> bool {
        allows(&m, p, None)
    }

    fn obj(pairs: &[(&str, Value)]) -> Value {
        Value::Object(pairs.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect())
    }

    #[test]
    fn deleting_or_editing_the_sandbox_channel_itself_is_refused() {
        let s = format!("/channels/{SANDBOX}");
        assert!(!ok(Method::DELETE, &s));
        assert!(!ok(Method::DELETE, &format!("{s}/")));
        assert!(!ok(Method::DELETE, &format!("{s}?reason=x")));
        assert!(!ok(Method::PUT, &s));
        assert!(!ok(Method::POST, &s));
        assert!(!allows(&Method::PATCH, &s, None));
        assert!(!allows(&Method::PATCH, &s, Some(&json!({}))));
        for field in ["name", "nsfw", "position", "parent_id", "permission_overwrites", "type", "default_auto_archive_duration"] {
            assert!(!allows(&Method::PATCH, &s, Some(&obj(&[(field, json!("x"))]))), "{field}");
            assert!(!allows(&Method::PATCH, &s, Some(&obj(&[("topic", json!("t")), (field, json!("x"))]))), "{field}");
        }
        assert!(refusal(&Method::DELETE, &s, None).is_some_and(|e| e.is_write_blocked()));
        for c in [GENERAL, VOICE] {
            assert!(!ok(Method::DELETE, &format!("/channels/{c}")));
            assert!(!allows(&Method::PATCH, &format!("/channels/{c}"), Some(&json!({ "topic": "t" }))));
        }
    }

    #[test]
    fn the_channel_builders_cannot_reach_the_sandbox_itself() {
        use super::super::guild_manage::{AdminCall, ChannelEdit, build};
        use crate::models::Snowflake;
        let del = build(&AdminCall::DeleteChannel { channel: Snowflake(SANDBOX) });
        assert!(!allows(&del.method, &del.path, del.body.as_ref()));
        let rename = build(&AdminCall::EditChannel { channel: Snowflake(SANDBOX), edit: ChannelEdit { name: Some("x".into()), ..Default::default() } });
        assert!(!allows(&rename.method, &rename.path, rename.body.as_ref()));
        let slow = build(&AdminCall::EditChannel { channel: Snowflake(SANDBOX), edit: ChannelEdit { slowmode: Some(5), ..Default::default() } });
        assert!(allows(&slow.method, &slow.path, slow.body.as_ref()));
        let topic = build(&AdminCall::EditChannel { channel: Snowflake(SANDBOX), edit: ChannelEdit { topic: Some("t".into()), ..Default::default() } });
        assert!(allows(&topic.method, &topic.path, topic.body.as_ref()));
    }

    #[test]
    fn slowmode_and_topic_are_the_only_channel_fields() {
        let s = format!("/channels/{SANDBOX}");
        assert!(allows(&Method::PATCH, &s, Some(&json!({ "rate_limit_per_user": 10 }))));
        assert!(allows(&Method::PATCH, &s, Some(&json!({ "topic": "x", "rate_limit_per_user": 0 }))));
    }

    #[test]
    fn message_subresources_of_the_sandbox_are_allowed() {
        let s = format!("/channels/{SANDBOX}");
        assert!(ok(Method::POST, &format!("{s}/messages")));
        assert!(ok(Method::PATCH, &format!("{s}/messages/42")));
        assert!(ok(Method::DELETE, &format!("{s}/messages/42")));
        assert!(ok(Method::PUT, &format!("{s}/messages/42/reactions/%F0%9F%91%8D/@me?location=Message&type=0")));
        assert!(ok(Method::DELETE, &format!("{s}/messages/42/reactions/%F0%9F%91%8D/0/@me")));
        assert!(ok(Method::PUT, &format!("{s}/pins/42")));
        assert!(ok(Method::DELETE, &format!("{s}/messages/pins/42")));
        assert!(ok(Method::POST, &format!("{s}/messages/42/threads")));
        assert!(ok(Method::POST, &format!("{s}/threads")));
        assert!(ok(Method::POST, &format!("{s}/typing")));
        assert!(ok(Method::POST, &format!("{s}/attachments")));
        assert!(ok(Method::PUT, &format!("{s}/polls/42/answers/@me")));
        assert!(!ok(Method::POST, &format!("{s}/messages/42/ack")));
        assert!(!ok(Method::POST, &format!("{s}/pins/ack")));
        assert!(!ok(Method::POST, "/guilds/1369382919948210248/ack"));
        assert!(!ok(Method::POST, "/read-states/ack-bulk"));
    }

    #[test]
    fn other_sandbox_channel_routes_are_refused() {
        let s = format!("/channels/{SANDBOX}");
        for (m, p) in [
            (Method::POST, "/messages/bulk-delete"),
            (Method::POST, "/messages/42/crosspost"),
            (Method::DELETE, "/messages"),
            (Method::PUT, "/permissions/1369382919948210248"),
            (Method::DELETE, "/permissions/1369382919948210248"),
            (Method::POST, "/webhooks"),
            (Method::POST, "/invites"),
            (Method::POST, "/followers"),
            (Method::PUT, "/recipients/1"),
            (Method::PUT, "/voice-status"),
            (Method::POST, "/call/ring"),
            (Method::DELETE, "/messages/42/../.."),
            (Method::DELETE, "/messages/abc"),
            (Method::POST, "/send-soundboard-sound"),
        ] {
            assert!(!ok(m.clone(), &format!("{s}{p}")), "{m} {p}");
        }
    }

    #[test]
    fn general_only_takes_new_messages() {
        let g = format!("/channels/{GENERAL}");
        assert!(ok(Method::POST, &format!("{g}/messages")));
        assert!(ok(Method::POST, &format!("{g}/attachments")));
        assert!(!ok(Method::PATCH, &format!("{g}/messages/42")));
        assert!(!ok(Method::DELETE, &format!("{g}/messages/42")));
        assert!(!ok(Method::PUT, &format!("{g}/messages/42/reactions/x/@me")));
        assert!(!ok(Method::PUT, &format!("{g}/pins/42")));
        assert!(!ok(Method::POST, &format!("{g}/threads")));
        assert!(ok(Method::POST, &format!("/channels/{VOICE}/send-soundboard-sound")));
        assert!(!ok(Method::POST, &format!("/channels/{VOICE}/messages")));
    }

    #[test]
    fn guild_role_member_and_account_writes_are_refused() {
        for (m, p) in [
            (Method::PATCH, "/guilds/1369382919948210248"),
            (Method::DELETE, "/guilds/1369382919948210248"),
            (Method::POST, "/guilds/1369382919948210248/delete"),
            (Method::POST, "/guilds/1369382919948210248/channels"),
            (Method::PATCH, "/guilds/1369382919948210248/channels"),
            (Method::POST, "/guilds/1369382919948210248/roles"),
            (Method::PATCH, "/guilds/1369382919948210248/roles/1"),
            (Method::DELETE, "/guilds/1369382919948210248/roles/1"),
            (Method::PATCH, "/guilds/1369382919948210248/members/2"),
            (Method::DELETE, "/guilds/1369382919948210248/members/2"),
            (Method::PUT, "/guilds/1369382919948210248/members/2/roles/3"),
            (Method::PUT, "/guilds/1369382919948210248/bans/2"),
            (Method::POST, "/guilds/1369382919948210248/prune"),
            (Method::PATCH, "/guilds/1369382919948210248/voice-states/2"),
            (Method::PATCH, "/users/@me"),
            (Method::PATCH, "/users/@me/settings-proto/1"),
            (Method::POST, "/users/@me/channels"),
            (Method::DELETE, "/channels/1"),
            (Method::POST, "/channels/1/messages"),
            (Method::DELETE, "/webhooks/1"),
            (Method::POST, "/stage-instances"),
            (Method::POST, "/reporting/message"),
        ] {
            assert!(!ok(m.clone(), p), "{m} {p}");
        }
        assert!(ok(Method::POST, "/guilds/1369382919948210248/members-search"));
        assert!(ok(Method::POST, "/users/@me/messages/search/tabs"));
        assert!(ok(Method::GET, "/guilds/1369382919948210248/audit-logs"));
    }

    #[test]
    fn body_scoped_writes_need_the_sandbox() {
        assert!(allows(&Method::POST, "/interactions", Some(&json!({ "channel_id": SANDBOX.to_string() }))));
        assert!(!allows(&Method::POST, "/interactions", Some(&json!({ "channel_id": GENERAL.to_string() }))));
        assert!(!allows(&Method::POST, "/interactions", None));
        assert!(allows(&Method::POST, "/users/@me/scheduled-messages", Some(&json!({ "channel_id": SANDBOX }))));
        assert!(!allows(&Method::POST, "/users/@me/scheduled-messages", Some(&json!({ "channel_id": "1" }))));
        assert!(!ok(Method::PATCH, "/users/@me/scheduled-messages/9"));
        observe(&Method::POST, "/users/@me/scheduled-messages", br#"{"id":"1575000000000000009","channel_id":"1574797385463496724"}"#);
        assert!(ok(Method::PATCH, "/users/@me/scheduled-messages/1575000000000000009"));
        assert!(ok(Method::DELETE, "/users/@me/scheduled-messages/1575000000000000009"));
        assert!(ok(Method::POST, "/users/@me/scheduled-messages/1575000000000000009/send"));
        observe(&Method::POST, "/users/@me/scheduled-messages", br#"{"id":"1575000000000000010","channel_id":"1"}"#);
        assert!(!ok(Method::DELETE, "/users/@me/scheduled-messages/1575000000000000010"));
        assert!(ok(Method::POST, &format!("/streams/guild:1369382919948210248:{VOICE}:1/preview")));
        assert!(!ok(Method::POST, "/streams/guild:1:2:3/preview"));
    }

    #[test]
    fn threads_created_in_the_sandbox_get_message_rights_but_cannot_be_deleted() {
        let t = 1575000000000000001u64;
        assert!(!ok(Method::POST, &format!("/channels/{t}/messages")));
        observe(&Method::POST, &format!("/channels/{SANDBOX}/messages/42/threads"), br#"{"id":"1575000000000000001","parent_id":"1574797385463496724"}"#);
        assert!(ok(Method::POST, &format!("/channels/{t}/messages")));
        assert!(ok(Method::POST, &format!("/channels/{t}/thread-members/@me")));
        assert!(allows(&Method::PATCH, &format!("/channels/{t}"), Some(&json!({ "archived": true }))));
        assert!(!allows(&Method::PATCH, &format!("/channels/{t}"), Some(&json!({ "name": "x" }))));
        assert!(!ok(Method::DELETE, &format!("/channels/{t}")));
        assert!(!ok(Method::POST, &format!("/channels/{t}/threads")));
        observe(&Method::POST, &format!("/channels/{GENERAL}/threads"), br#"{"id":"1575000000000000002","parent_id":"1403447573389381652"}"#);
        assert!(!ok(Method::POST, "/channels/1575000000000000002/messages"));
        observe(&Method::POST, &format!("/channels/{SANDBOX}/threads"), br#"{"id":"1575000000000000003","parent_id":"1369413820958904493"}"#);
        assert!(!ok(Method::POST, "/channels/1575000000000000003/messages"));
    }

    #[test]
    fn only_real_hosts_are_scoped() {
        assert!(scoped(true, "https://discord.com/api/v10"));
        assert!(scoped(true, "https://canary.discord.com/api/v9"));
        assert!(!scoped(false, "https://discord.com/api/v10"));
        assert!(!scoped(true, "http://127.0.0.1:4567"));
        assert!(!scoped(true, "http://localhost:80"));
        assert!(scoped(true, "http://127.0.0.1.evil.example:80"));
    }
}
