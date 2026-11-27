use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use parking_lot::Mutex;
use phoenix_api::{models as api, rest::forum::Poll};
use serde_json::Value;

use crate::model::Id;

pub use phoenix_api::models::Component as Comp;

fn snowflake(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Emoji {
    pub name: String,
    pub id: Option<u64>,
    pub animated: bool,
}

impl Emoji {
    fn from_value(v: Option<&Value>) -> Option<Self> {
        let v = v?;
        let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let id = v.get("id").and_then(snowflake);
        (id.is_some() || !name.is_empty()).then_some(Self {
            name,
            id,
            animated: v.get("animated").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    pub fn token(&self) -> String {
        match self.id {
            Some(id) => format!("<{}:{}:{}>", if self.animated { "a" } else { "" }, self.name, id),
            None => self.name.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PollAnswer {
    pub id: u32,
    pub text: String,
    pub emoji: Option<Emoji>,
    pub count: u32,
    pub me: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PollView {
    pub question: String,
    pub answers: Vec<PollAnswer>,
    pub multi: bool,
    pub expiry: Option<i64>,
    pub finalized: bool,
}

impl PollView {
    pub fn from_value(v: &Value) -> Option<Self> {
        let p = Poll::from_value(v)?;
        Some(Self {
            question: p.question.text.clone().unwrap_or_default(),
            answers: p
                .answers
                .iter()
                .map(|a| {
                    let (count, me) = p.count(a.answer_id);
                    PollAnswer {
                        id: a.answer_id,
                        text: a.poll_media.text.clone().unwrap_or_default(),
                        emoji: Emoji::from_value(a.poll_media.emoji.as_ref()),
                        count,
                        me,
                    }
                })
                .collect(),
            multi: p.allow_multiselect,
            expiry: p.expiry.as_deref().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).map(|d| d.timestamp()),
            finalized: p.results.as_ref().is_some_and(|r| r.is_finalized),
        })
    }

    pub fn total(&self) -> u32 {
        self.answers.iter().map(|a| a.count).sum()
    }

    pub fn voted(&self) -> bool {
        self.answers.iter().any(|a| a.me)
    }

    pub fn ended(&self, now: i64) -> bool {
        self.finalized || self.expiry.is_some_and(|e| e <= now)
    }

    pub fn apply_vote(&mut self, answer: u32, add: bool, mine: bool) {
        if let Some(a) = self.answers.iter_mut().find(|a| a.id == answer) {
            a.count = if add { a.count + 1 } else { a.count.saturating_sub(1) };
            if mine {
                a.me = add;
            }
        }
    }

    pub fn apply_gateway_vote(&mut self, answer: u32, add: bool, mine: bool) {
        if mine && self.answers.iter().any(|a| a.id == answer && a.me == add) {
            return;
        }
        self.apply_vote(answer, add, mine);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parts {
    pub app: Id,
    pub flags: u64,
    pub components: Vec<Comp>,
    pub poll: Option<PollView>,
}

#[derive(Clone, Debug)]
pub struct InteractionInfo {
    pub app_name: String,
    pub guild_owner: Option<Id>,
    pub user_owner: Option<Id>,
    pub user: Option<crate::model::User>,
}

pub const GUILD_INSTALL: &str = "0";
pub const USER_INSTALL: &str = "1";

pub fn interaction_of(m: &api::Message) -> Option<InteractionInfo> {
    let meta = m.interaction_metadata.as_ref()?;
    let owner = |k: &str| meta.authorizing_integration_owners.get(k).filter(|s| s.0 != 0).map(|s| Id(s.0));
    Some(InteractionInfo {
        app_name: m.author.username.clone(),
        guild_owner: owner(GUILD_INSTALL),
        user_owner: owner(USER_INSTALL),
        user: meta.user.as_ref().map(crate::discord_backend::convert::user),
    })
}

fn interactions() -> &'static Mutex<HashMap<u64, InteractionInfo>> {
    static I: OnceLock<Mutex<HashMap<u64, InteractionInfo>>> = OnceLock::new();
    I.get_or_init(Default::default)
}

pub fn interaction_info(msg: Id) -> Option<InteractionInfo> {
    interactions().lock().get(&msg.0).cloned()
}

fn slot() -> &'static Mutex<HashMap<u64, Arc<Parts>>> {
    static S: OnceLock<Mutex<HashMap<u64, Arc<Parts>>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

const MAX_STORED: usize = 2048;

fn insert_bounded<V>(map: &mut HashMap<u64, V>, id: u64, val: V) {
    if map.len() >= MAX_STORED
        && !map.contains_key(&id)
        && let Some(&old) = map.keys().min()
    {
        map.remove(&old);
    }
    map.insert(id, val);
}

pub fn record(m: &api::Message) {
    let app = m.application_id.or(m.webhook_id).map(|s| s.0).unwrap_or(m.author.id.0);
    if let Some(info) = interaction_of(m) {
        insert_bounded(&mut interactions().lock(), m.id.0, info);
    }
    let components = m.components.clone();
    let poll = m.poll.as_ref().and_then(PollView::from_value);
    if components.is_empty() && poll.is_none() {
        return;
    }
    let parts = Parts { app: Id(app), flags: m.flags, components, poll };
    insert_bounded(&mut slot().lock(), m.id.0, Arc::new(parts));
}

pub fn get(msg: Id) -> Option<Arc<Parts>> {
    slot().lock().get(&msg.0).cloned()
}

pub fn update_poll(msg: Id, f: impl FnOnce(&mut PollView)) -> bool {
    let mut map = slot().lock();
    let Some(p) = map.get_mut(&msg.0) else { return false };
    let mut parts = (**p).clone();
    let Some(poll) = parts.poll.as_mut() else { return false };
    f(poll);
    *p = Arc::new(parts);
    true
}

pub fn insert(msg: Id, parts: Parts) {
    slot().lock().insert(msg.0, Arc::new(parts));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn interaction_info_follows_the_client() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../phoenix-api/tests/fixtures/message_create.json");
        let mut raw: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let plain: api::Message = serde_json::from_value(raw.clone()).unwrap();
        assert!(interaction_of(&plain).is_none());
        raw["interaction_metadata"] = json!({"id": "1", "type": 2, "user": {"id": "9", "username": "ann"}, "authorizing_integration_owners": {"0": "77", "1": "9"}});
        let m: api::Message = serde_json::from_value(raw).unwrap();
        let info = interaction_of(&m).unwrap();
        assert_eq!((info.guild_owner, info.user_owner), (Some(Id(77)), Some(Id(9))));
        assert_eq!(info.user.map(|u| u.id), Some(Id(9)));
        assert_eq!(info.app_name, m.author.username);
    }

    #[test]
    fn poll_counts_and_votes() {
        let v = json!({"question":{"text":"Q?"},"allow_multiselect":false,
            "answers":[{"answer_id":1,"poll_media":{"text":"Yes"}},{"answer_id":2,"poll_media":{"text":"No"}}],
            "expiry":"2030-01-01T00:00:00+00:00",
            "results":{"is_finalized":false,"answer_counts":[{"id":1,"count":3,"me_voted":true}]}});
        let mut p = PollView::from_value(&v).unwrap();
        assert_eq!(p.total(), 3);
        assert!(p.voted());
        assert!(!p.ended(0));
        p.apply_vote(2, true, false);
        assert_eq!(p.answers[1].count, 1);
        p.apply_vote(1, false, true);
        assert!(!p.voted());
        p.apply_gateway_vote(1, false, true);
        assert_eq!(p.answers[0].count, 2);
        p.apply_gateway_vote(2, true, true);
        assert_eq!((p.answers[1].count, p.answers[1].me), (2, true));
    }

    #[test]
    fn insert_bounded_evicts_oldest_snowflake() {
        let mut map = HashMap::new();
        for i in 1..=(MAX_STORED as u64 + 10) {
            insert_bounded(&mut map, i, i);
        }
        assert_eq!(map.len(), MAX_STORED);
        assert!(!map.contains_key(&1));
        assert!(map.contains_key(&(MAX_STORED as u64 + 10)));
    }
}
