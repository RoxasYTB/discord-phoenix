//! "Online on mobile" presence (client `isMobileOnline`): the user is online
//! and the only online client in `client_status` is `mobile`. The member list
//! then draws the phone glyph (`svg-mask-status-online-mobile`, 10x15 at
//! 22,17 on a 32px avatar) instead of the round dot.

use std::collections::HashSet;
use std::sync::{LazyLock, RwLock};

use serde_json::Value;

static MOBILE: LazyLock<RwLock<HashSet<u64>>> = LazyLock::new(|| RwLock::new(HashSet::new()));

/// Whether a presence object (`status` + `client_status`) is online on mobile only.
pub fn is_mobile_only(status: &str, client_status: Option<&Value>) -> bool {
    let Some(cs) = client_status.and_then(Value::as_object) else { return false };
    let online = |k: &str| cs.get(k).and_then(Value::as_str) == Some("online");
    status == "online" && online("mobile") && !online("desktop") && !online("web") && !online("embedded")
}

/// Records a presence for `user`.
pub fn note(user: u64, status: &str, client_status: Option<&Value>) {
    let m = is_mobile_only(status, client_status);
    if let Ok(mut s) = MOBILE.write() {
        if m {
            s.insert(user);
        } else {
            s.remove(&user);
        }
    }
}

/// Records a raw presence JSON object (`status`, `client_status`).
pub fn note_value(user: u64, p: &Value) {
    note(user, p.get("status").and_then(Value::as_str).unwrap_or("offline"), p.get("client_status"));
}

pub fn is_mobile(user: u64) -> bool {
    MOBILE.read().is_ok_and(|s| s.contains(&user))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mobile_only() {
        assert!(is_mobile_only("online", Some(&json!({"mobile": "online"}))));
        assert!(!is_mobile_only("online", Some(&json!({"mobile": "online", "desktop": "online"}))));
        assert!(!is_mobile_only("idle", Some(&json!({"mobile": "idle"}))));
        assert!(!is_mobile_only("online", None));
    }
}
