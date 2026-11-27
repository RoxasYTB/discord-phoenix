use std::sync::Mutex;

use reqwest::Method;
use serde_json::Value;

use crate::error::Error;
use crate::models::Channel;

pub const KNOWN: &[u64] = &[1507838362562859119];

static SEEN: Mutex<Vec<u64>> = Mutex::new(Vec::new());

fn letters(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

pub fn trap_text(name: &str, topic: Option<&str>) -> bool {
    letters(name).contains("honeypot") || topic.is_some_and(|t| letters(t).contains("honeypot"))
}

pub fn is_trap(id: u64) -> bool {
    KNOWN.contains(&id) || SEEN.lock().is_ok_and(|s| s.contains(&id))
}

pub fn mark(id: u64) {
    if let Ok(mut s) = SEEN.lock()
        && !s.contains(&id)
    {
        s.push(id);
    }
}

pub fn note(c: &Channel) {
    let parent = c.parent_id.is_some_and(|p| is_trap(p.0));
    if parent || trap_text(c.name.as_deref().unwrap_or_default(), c.topic.as_deref()) {
        mark(c.id.0);
    }
}

fn id(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

pub fn targets(path: &str, body: Option<&Value>) -> bool {
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let mut seg = path.trim_start_matches('/').split('/');
    let in_path = seg.next() == Some("channels") && seg.next().and_then(id).is_some_and(is_trap);
    let in_body = body.and_then(|b| b.get("channel_id")).is_some_and(|v| match v {
        Value::String(s) => id(s).is_some_and(is_trap),
        Value::Number(n) => n.as_u64().is_some_and(is_trap),
        _ => false,
    });
    in_path || in_body
}

pub fn refusal(method: &Method, path: &str, body: Option<&Value>) -> Option<Error> {
    (*method != Method::GET && targets(path, body)).then(|| crate::rest::guard::blocked(method, format!("{path} (trap channel)")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Snowflake;
    use serde_json::json;

    const HONEYPOT: u64 = 1507838362562859119;

    #[test]
    fn the_aperture_honeypot_only_takes_reads() {
        let c = format!("/channels/{HONEYPOT}");
        for (m, p) in [
            (Method::POST, format!("{c}/messages")),
            (Method::POST, format!("{c}/typing")),
            (Method::POST, format!("{c}/messages/1/ack")),
            (Method::POST, format!("{c}/pins/ack")),
            (Method::PUT, format!("{c}/messages/1/reactions/x/@me")),
            (Method::POST, format!("{c}/attachments")),
            (Method::PATCH, c.clone()),
            (Method::DELETE, c.clone()),
        ] {
            assert!(refusal(&m, &p, None).is_some_and(|e| e.is_write_blocked()), "{m} {p}");
        }
        assert!(refusal(&Method::GET, &format!("{c}/messages?limit=50"), None).is_none());
        assert!(refusal(&Method::POST, "/interactions", Some(&json!({ "channel_id": HONEYPOT.to_string() }))).is_some());
        assert!(refusal(&Method::POST, "/users/@me/scheduled-messages", Some(&json!({ "channel_id": HONEYPOT }))).is_some());
        assert!(refusal(&Method::POST, "/channels/1403447573389381652/messages", None).is_none());
    }

    #[test]
    fn honeypots_are_found_by_name_or_topic() {
        assert!(trap_text("〢🍯・honeypot", None));
        assert!(trap_text("Honey-Pot", None));
        assert!(trap_text("piège", Some("🍯 Honeypot — Ne pas envoyer de messages ici / Do not post here")));
        assert!(!trap_text("général", Some("discussions")));
        assert!(!trap_text("honey", Some("pot de miel")));
        let found = Channel { id: Snowflake(77), name: Some("honey_pot".into()), ..Default::default() };
        assert!(!is_trap(77));
        note(&found);
        assert!(is_trap(77));
        assert!(refusal(&Method::POST, "/channels/77/messages", None).is_some());
        let thread = Channel { id: Snowflake(78), parent_id: Some(Snowflake(77)), name: Some("fil".into()), ..Default::default() };
        note(&thread);
        assert!(is_trap(78));
        let plain = Channel { id: Snowflake(79), name: Some("général".into()), ..Default::default() };
        note(&plain);
        assert!(!is_trap(79));
    }
}
