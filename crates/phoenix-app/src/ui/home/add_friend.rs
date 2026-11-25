use gpui::SharedString;
use serde_json::{Map, Value, json};

pub const NOTE_MAX: usize = 120;
pub const TAG_MAX: usize = 37;

const TOO_MANY_FRIENDS: u32 = 30002;
const TOO_MANY_BLOCKED_USERS: u32 = 30059;
const TOO_MANY_PENDING_OUTGOING: u32 = 30078;
const EMAIL_VERIFICATION_REQUIRED: u32 = 40002;
const USER_QUARANTINED: u32 = 40068;
const RELATIONSHIP_INCOMING_DISABLED: u32 = 80000;
const RELATIONSHIP_ALREADY_FRIENDS: u32 = 80007;
pub const NO_CONFIRMATION: u32 = 80013;
const USER_FRIEND_REQUEST_LIMITED_ACCESS: u32 = 340007;
const USER_LIMITED_ACCESS: std::ops::RangeInclusive<u32> = 340000..=349999;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub status: u16,
    pub code: Option<u32>,
    pub note_rejected: bool,
}

impl Failure {
    pub fn encode(&self) -> String {
        format!("add-friend:{}:{}:{}", self.status, self.code.unwrap_or(0), u8::from(self.note_rejected))
    }

    pub fn decode(s: &str) -> Option<Self> {
        let mut it = s.strip_prefix("add-friend:")?.split(':');
        let status = it.next()?.parse().ok()?;
        let code: u32 = it.next()?.parse().ok()?;
        let note_rejected = it.next()? == "1";
        Some(Self { status, code: (code != 0).then_some(code), note_rejected })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    TooFast,
    VerifyEmail,
    Failed(SharedString),
}

pub fn normalize(tag: &str) -> String {
    let t = tag.trim();
    match t.strip_prefix('@') {
        Some(rest) if !t.contains('#') => rest.to_owned(),
        _ => t.to_owned(),
    }
}

fn plain_username(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\\' || c == '.')
}

fn email_like(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    (1..n).any(|at| chars[at] == '@' && (at + 2..n.saturating_sub(1)).any(|dot| chars[dot] == '.'))
}

fn legacy_tag(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    n >= 6 && chars[n - 5] == '#' && chars[n - 4..].iter().all(char::is_ascii_digit)
}

pub fn valid(tag: &str) -> bool {
    !tag.contains('\n') && (plain_username(tag) || (tag.contains('#') && (email_like(tag) || legacy_tag(tag))))
}

fn parse_int(s: Option<&str>) -> Option<i64> {
    let s = s?.trim_start();
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let end = digits.bytes().take_while(u8::is_ascii_digit).count();
    let v: i64 = digits[..end].parse().ok()?;
    Some(if neg { -v } else { v })
}

pub fn clean_note(note: &str) -> Option<String> {
    let n = note.replace('\n', " ");
    let n = n.trim();
    (!n.is_empty()).then(|| n.to_owned())
}

pub fn note_too_long(note: &str) -> bool {
    note.chars().map(char::len_utf16).sum::<usize>() > NOTE_MAX
}

pub fn request_body(tag: &str, note: Option<&str>) -> Value {
    let mut parts = tag.split('#');
    let username = parts.next().unwrap_or_default();
    let discriminator = parse_int(parts.next());
    let mut m = Map::new();
    m.insert("username".into(), json!(username));
    m.insert("discriminator".into(), discriminator.map_or(Value::Null, Value::from));
    if let Some(n) = note {
        m.insert("note".into(), json!(n));
    }
    Value::Object(m)
}

pub fn error_text(code: Option<u32>, tag: &str) -> SharedString {
    match code {
        Some(RELATIONSHIP_INCOMING_DISABLED) => crate::t!("friends.err_incoming_disabled", tag = tag),
        Some(TOO_MANY_FRIENDS) => crate::t!("friends.err_too_many_friends"),
        Some(RELATIONSHIP_ALREADY_FRIENDS) => crate::t!("friends.err_already_friends"),
        Some(USER_QUARANTINED | USER_FRIEND_REQUEST_LIMITED_ACCESS) => crate::t!("friends.err_limited"),
        Some(TOO_MANY_BLOCKED_USERS) => crate::t!("friends.err_too_many_blocked"),
        Some(TOO_MANY_PENDING_OUTGOING) => crate::t!("friends.err_too_many_pending"),
        _ => crate::t!("friends.request_failed"),
    }
}

pub fn inline_error(f: &Failure, tag: &str) -> SharedString {
    if f.note_rejected { crate::t!("friends.err_note") } else { error_text(f.code, tag) }
}

pub fn dialog(f: &Failure, tag: &str) -> Option<Dialog> {
    match (f.status, f.code) {
        (429, _) => Some(Dialog::TooFast),
        (403, Some(EMAIL_VERIFICATION_REQUIRED)) => Some(Dialog::VerifyEmail),
        (_, Some(USER_QUARANTINED | NO_CONFIRMATION)) => None,
        (s, Some(c)) if (400..500).contains(&s) && USER_LIMITED_ACCESS.contains(&c) => None,
        (_, code) => Some(Dialog::Failed(error_text(code, tag))),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormState {
    pub can_send: bool,
    pub error: Option<SharedString>,
    pub success: Option<SharedString>,
}

impl FormState {
    pub fn on_change(&mut self, value: &str) {
        *self = Self { can_send: !value.is_empty(), ..Self::default() };
    }

    pub fn on_success(&mut self, tag: &str) {
        *self = Self { success: Some(crate::t!("friends.request_sent", name = tag)), ..Self::default() };
    }

    pub fn on_error(&mut self, text: SharedString) {
        self.can_send = true;
        self.error = Some(text);
    }
}

pub fn hint_suffix(value: &str) -> Option<String> {
    let after = value.split('#').nth(1)?;
    let n = after.chars().count();
    ("#0000".len() > n + 1).then(|| "#0000"[n + 1..].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_leading_at_without_hash() {
        assert_eq!(normalize("  @wumpus "), "wumpus");
        assert_eq!(normalize("@a#1234"), "@a#1234");
    }

    #[test]
    fn validates_like_the_client() {
        assert!(valid("wumpus"));
        assert!(valid("w.u_mp\\us"));
        assert!(valid("old#1234"));
        assert!(valid("me@x.io#"));
        assert!(!valid("with space"));
        assert!(!valid("old#123"));
        assert!(!valid("me@x.io"));
        assert!(!valid(""));
        assert!(!valid("é"));
    }

    #[test]
    fn body_matches_send_request() {
        assert_eq!(request_body("wumpus", None), json!({ "username": "wumpus", "discriminator": null }));
        assert_eq!(request_body("old#0042", Some("hi")), json!({ "username": "old", "discriminator": 42, "note": "hi" }));
        assert_eq!(request_body("x#12ab", None)["discriminator"], json!(12));
    }

    #[test]
    fn note_is_flattened_and_bounded() {
        assert_eq!(clean_note("a\nb  ").as_deref(), Some("a b"));
        assert_eq!(clean_note(" \n "), None);
        assert!(!note_too_long(&"x".repeat(120)));
        assert!(note_too_long(&"x".repeat(121)));
    }

    #[test]
    fn failure_roundtrip_and_dialogs() {
        let f = Failure { status: 400, code: Some(80007), note_rejected: false };
        assert_eq!(Failure::decode(&f.encode()), Some(f.clone()));
        assert!(matches!(dialog(&f, "a"), Some(Dialog::Failed(_))));
        assert_eq!(dialog(&Failure { status: 429, code: None, note_rejected: false }, "a"), Some(Dialog::TooFast));
        assert_eq!(dialog(&Failure { status: 403, code: Some(40002), note_rejected: false }, "a"), Some(Dialog::VerifyEmail));
        assert_eq!(dialog(&Failure { status: 403, code: Some(340007), note_rejected: false }, "a"), None);
        assert_eq!(dialog(&Failure { status: 400, code: Some(80013), note_rejected: false }, "a"), None);
        assert_eq!(Failure::decode("other"), None);
    }

    #[test]
    fn form_reducer() {
        let mut s = FormState::default();
        assert!(!s.can_send);
        s.on_change("a");
        assert!(s.can_send);
        s.on_error("e".into());
        assert!(s.can_send && s.error.is_some());
        s.on_change("ab");
        assert!(s.error.is_none());
        s.on_success("ab");
        assert!(!s.can_send && s.success.is_some());
        s.on_change("");
        assert_eq!(s, FormState::default());
    }

    #[test]
    fn legacy_hint() {
        assert_eq!(hint_suffix("abc#12").as_deref(), Some("00"));
        assert_eq!(hint_suffix("abc#").as_deref(), Some("0000"));
        assert_eq!(hint_suffix("abc#1234"), None);
        assert_eq!(hint_suffix("abc"), None);
    }
}
