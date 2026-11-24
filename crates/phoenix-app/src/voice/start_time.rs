use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use phoenix_api::gateway::events::GatewayEvent;
use serde_json::Value;

#[derive(Default)]
struct Store {
    start: HashMap<u64, i64>,
    requested: HashSet<u64>,
}

static STORE: LazyLock<Mutex<Store>> = LazyLock::new(Default::default);

pub fn request(guild: u64) -> Option<(u8, Value)> {
    STORE
        .lock()
        .ok()?
        .requested
        .insert(guild)
        .then(|| (43, serde_json::json!({ "guild_id": guild.to_string(), "fields": ["status", "voice_start_time"] })))
}

fn id(v: &Value) -> Option<u64> {
    v.get("id")?.as_str()?.parse().ok()
}

fn ts(v: &Value) -> Option<i64> {
    v.get("voice_start_time").and_then(Value::as_i64)
}

pub fn apply(name: &str, data: &Value) -> bool {
    let Ok(mut s) = STORE.lock() else { return false };
    match name {
        "CHANNEL_INFO" => {
            for c in data.get("channels").and_then(Value::as_array).into_iter().flatten() {
                if let Some(i) = id(c) {
                    match ts(c) {
                        Some(t) => s.start.insert(i, t),
                        None => s.start.remove(&i),
                    };
                }
            }
            true
        }
        "VOICE_CHANNEL_START_TIME_UPDATE" => {
            let Some(i) = id(data) else { return false };
            match ts(data) {
                Some(t) => s.start.insert(i, t),
                None => s.start.remove(&i),
            };
            true
        }
        _ => false,
    }
}

pub fn on_gateway(ev: &GatewayEvent) -> bool {
    match ev {
        GatewayEvent::Unknown { name, data } => apply(name, data),
        _ => false,
    }
}

pub fn get(channel: u64) -> Option<i64> {
    STORE.lock().ok()?.start.get(&channel).copied()
}

pub fn ticking() -> bool {
    STORE.lock().is_ok_and(|s| !s.start.is_empty())
}

pub fn set(channel: u64, start: Option<i64>) {
    if let Ok(mut s) = STORE.lock() {
        match start {
            Some(t) => s.start.insert(channel, t),
            None => s.start.remove(&channel),
        };
    }
}

pub fn format(elapsed_s: i64) -> String {
    let e = elapsed_s.max(0);
    let (h, m, s) = (e / 3600, e / 60 % 60, e % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

pub fn now_s() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_the_client() {
        assert_eq!(format(47 * 3600 + 50 * 60 + 7), "47:50:07");
        assert_eq!(format(65), "1:05");
    }

    #[test]
    fn parses_channel_info() {
        apply("CHANNEL_INFO", &serde_json::json!({"guild_id":"1","channels":[{"id":"77","voice_start_time":1700000000}]}));
        assert_eq!(get(77), Some(1_700_000_000));
        apply("VOICE_CHANNEL_START_TIME_UPDATE", &serde_json::json!({"id":"77","guild_id":"1","voice_start_time":null}));
        assert_eq!(get(77), None);
        assert!(request(5).is_some());
        assert!(request(5).is_none());
    }
}
