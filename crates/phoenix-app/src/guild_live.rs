use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use serde_json::Value;

pub const EVENT_ACTIVE: u64 = 2;
pub const ENTITY_NONE: u64 = 0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Live {
    pub stages: HashMap<u64, u64>,
    pub events: HashMap<u64, Option<u64>>,
    pub activities: HashMap<(String, u64), Activity>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    pub channel: Option<u64>,
    pub users: Vec<u64>,
}

fn activity_entry(instance: &Value) -> Option<((String, u64), Activity)> {
    let location = instance.get("location")?;
    let key = (location.get("id")?.as_str()?.to_string(), id(instance.get("application_id"))?);
    let users = instance
        .get("participants")
        .and_then(Value::as_array)
        .map(|p| p.iter().filter_map(|u| id(u.get("user_id"))).collect())
        .unwrap_or_default();
    Some((key, Activity { channel: id(location.get("channel_id")), users }))
}

fn upsert_activity(l: &mut Live, instance: &Value) -> bool {
    let Some((key, a)) = activity_entry(instance) else { return false };
    if a.users.is_empty() {
        l.activities.remove(&key).is_some()
    } else {
        l.activities.insert(key, a.clone()) != Some(a)
    }
}

fn activity_guild(instance: &Value) -> Option<u64> {
    id(instance.get("guild_id")).or_else(|| id(instance.get("location")?.get("guild_id")))
}

static STORE: Mutex<Option<HashMap<u64, Live>>> = Mutex::new(None);

fn store() -> MutexGuard<'static, Option<HashMap<u64, Live>>> {
    STORE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn id(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

pub fn active_event(e: &Value) -> bool {
    e.get("status").and_then(Value::as_u64) == Some(EVENT_ACTIVE) && e.get("entity_type").and_then(Value::as_u64).is_some_and(|t| t != ENTITY_NONE)
}

pub fn from_guild(stages: &[Value], events: &[Value], activities: &[Value]) -> Live {
    let mut l = Live::default();
    for a in activities {
        upsert_activity(&mut l, a);
    }
    for s in stages {
        if let (Some(i), Some(c)) = (id(s.get("id")), id(s.get("channel_id"))) {
            l.stages.insert(i, c);
        }
    }
    for e in events.iter().filter(|e| active_event(e)) {
        if let Some(i) = id(e.get("id")) {
            l.events.insert(i, id(e.get("channel_id")));
        }
    }
    l
}

pub fn load(guild: u64, stages: &[Value], events: &[Value], activities: &[Value]) {
    store().get_or_insert_with(HashMap::new).insert(guild, from_guild(stages, events, activities));
}

pub fn load_activities(guild: u64, activities: &[Value]) {
    let mut st = store();
    let l = st.get_or_insert_with(HashMap::new).entry(guild).or_default();
    l.activities.clear();
    for a in activities {
        upsert_activity(l, a);
    }
}

pub fn forget(guild: u64) {
    if let Some(m) = store().as_mut() {
        m.remove(&guild);
    }
}

pub fn apply(live: &mut HashMap<u64, Live>, name: &str, data: &Value) -> bool {
    if name == "EMBEDDED_ACTIVITY_UPDATE_V2" {
        let Some(guild) = activity_guild(data) else { return false };
        return upsert_activity(live.entry(guild).or_default(), data);
    }
    let (Some(guild), Some(i)) = (id(data.get("guild_id")), id(data.get("id"))) else { return false };
    let l = live.entry(guild).or_default();
    match name {
        "STAGE_INSTANCE_CREATE" | "STAGE_INSTANCE_UPDATE" => match id(data.get("channel_id")) {
            Some(c) => l.stages.insert(i, c) != Some(c),
            None => false,
        },
        "STAGE_INSTANCE_DELETE" => l.stages.remove(&i).is_some(),
        "GUILD_SCHEDULED_EVENT_CREATE" | "GUILD_SCHEDULED_EVENT_UPDATE" if active_event(data) => l.events.insert(i, id(data.get("channel_id"))).is_none(),
        "GUILD_SCHEDULED_EVENT_CREATE" | "GUILD_SCHEDULED_EVENT_UPDATE" | "GUILD_SCHEDULED_EVENT_DELETE" => l.events.remove(&i).is_some(),
        _ => false,
    }
}

pub fn on_event(name: &str, data: &Value) -> bool {
    apply(store().get_or_insert_with(HashMap::new), name, data)
}

pub fn get(guild: u64) -> Live {
    store().as_ref().and_then(|m| m.get(&guild).cloned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn guild_snapshot_keeps_live_stages_and_active_events() {
        let l = from_guild(
            &[json!({ "id": "7", "channel_id": "70", "guild_id": "1" })],
            &[
                json!({ "id": "8", "status": 2, "entity_type": 2, "channel_id": "80" }),
                json!({ "id": "9", "status": 1, "entity_type": 3, "channel_id": null }),
                json!({ "id": "10", "status": 2, "entity_type": 3, "channel_id": null }),
                json!({ "id": "11", "status": 2, "entity_type": 0 }),
            ],
            &[],
        );
        assert_eq!(l.stages, HashMap::from([(7, 70)]));
        assert_eq!(l.events, HashMap::from([(8, Some(80)), (10, None)]));
    }

    #[test]
    fn embedded_activities_follow_their_participants() {
        let inst = |users: &[&str]| {
            json!({
                "application_id": "880218394199220334",
                "guild_id": "5",
                "location": { "id": "gc-5-50", "kind": "gc", "channel_id": "50", "guild_id": "5" },
                "participants": users.iter().map(|u| json!({ "user_id": u, "session_id": "s" })).collect::<Vec<_>>(),
            })
        };
        let l = from_guild(&[], &[], &[inst(&["1", "2"]), json!({ "application_id": "9" })]);
        assert_eq!(l.activities.len(), 1);
        assert_eq!(l.activities[&("gc-5-50".to_string(), 880218394199220334)], Activity { channel: Some(50), users: vec![1, 2] });
        let mut m = HashMap::new();
        assert!(apply(&mut m, "EMBEDDED_ACTIVITY_UPDATE_V2", &inst(&["1"])));
        assert!(!apply(&mut m, "EMBEDDED_ACTIVITY_UPDATE_V2", &inst(&["1"])));
        assert_eq!(m[&5].activities.len(), 1);
        assert!(apply(&mut m, "EMBEDDED_ACTIVITY_UPDATE_V2", &inst(&[])));
        assert!(m[&5].activities.is_empty());
    }

    #[test]
    fn gateway_events_update_the_store() {
        let mut m = HashMap::new();
        assert!(apply(&mut m, "STAGE_INSTANCE_CREATE", &json!({ "id": "1", "guild_id": "5", "channel_id": "50" })));
        assert!(!apply(&mut m, "STAGE_INSTANCE_UPDATE", &json!({ "id": "1", "guild_id": "5", "channel_id": "50" })));
        assert!(apply(&mut m, "GUILD_SCHEDULED_EVENT_UPDATE", &json!({ "id": "2", "guild_id": "5", "status": 2, "entity_type": 1, "channel_id": "50" })));
        assert_eq!(m[&5].events.get(&2), Some(&Some(50)));
        assert!(apply(&mut m, "GUILD_SCHEDULED_EVENT_UPDATE", &json!({ "id": "2", "guild_id": "5", "status": 3, "entity_type": 1 })));
        assert!(m[&5].events.is_empty());
        assert!(apply(&mut m, "STAGE_INSTANCE_DELETE", &json!({ "id": "1", "guild_id": "5" })));
        assert!(m[&5].stages.is_empty());
        assert!(!apply(&mut m, "MESSAGE_CREATE", &json!({ "id": "1", "guild_id": "5" })));
    }
}
