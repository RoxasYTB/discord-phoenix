use std::{
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use phoenix_api::{Cache, models as api, rest::message_actions::{Reminder, parse_reminders}};
use serde_json::{Value, json};
use tokio::sync::Notify;

pub const WEEK_MS: i64 = 7 * 24 * 3600 * 1000;

#[derive(Clone, Debug)]
pub struct Entry {
    pub channel: u64,
    pub message_id: u64,
    pub due_ms: i64,
    pub message: Option<api::Message>,
}

static LIST: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
static WAKE: Notify = Notify::const_new();
static RUN: AtomicU64 = AtomicU64::new(0);

fn list() -> MutexGuard<'static, Vec<Entry>> {
    LIST.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn due_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp_millis())
}

pub fn entry(r: &Reminder) -> Option<Entry> {
    Some(Entry { channel: r.channel_id?.0, message_id: r.message_id?.0, due_ms: due_ms(&r.due_at)?, message: r.message.clone() })
}

pub fn next_due(list: &[Entry], now: i64) -> Option<&Entry> {
    list.iter().filter(|e| e.due_ms > now).min_by_key(|e| e.due_ms).filter(|e| e.due_ms - now <= WEEK_MS)
}

pub fn upsert(list: &mut Vec<Entry>, data: &Value) {
    let Some(r) = parse_reminders(&json!({ "reminders": [data] })).into_iter().next() else { return };
    let (Some(c), Some(m)) = (r.channel_id, r.message_id) else { return };
    list.retain(|e| !(e.channel == c.0 && e.message_id == m.0));
    list.extend(entry(&r));
}

pub fn remove(list: &mut Vec<Entry>, data: &Value) {
    let id = |k: &str| data.get(k).and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok());
    if let (Some(c), Some(m)) = (id("channel_id"), id("message_id")) {
        list.retain(|e| !(e.channel == c && e.message_id == m));
    }
}

pub fn set(reminders: &[Reminder]) {
    *list() = reminders.iter().filter_map(entry).collect();
    WAKE.notify_waiters();
}

pub fn on_saved_message_create(data: &Value) {
    upsert(&mut list(), data);
    WAKE.notify_waiters();
}

pub fn on_saved_message_delete(data: &Value) {
    remove(&mut list(), data);
    WAKE.notify_waiters();
}

pub fn restart() -> u64 {
    let gen = RUN.fetch_add(1, Ordering::SeqCst) + 1;
    list().clear();
    WAKE.notify_waiters();
    gen
}

pub async fn run(cache: Cache, gen: u64) {
    let mut floor = i64::MIN;
    loop {
        let notified = WAKE.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if RUN.load(Ordering::SeqCst) != gen {
            return;
        }
        let now = now_ms().max(floor);
        let next = next_due(&list(), now).cloned();
        let Some(e) = next else {
            notified.await;
            continue;
        };
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis((e.due_ms - now).max(0) as u64)) => {
                floor = e.due_ms;
                if RUN.load(Ordering::SeqCst) != gen {
                    return;
                }
                super::on_reminder_due(&cache, &e);
            }
            _ = &mut notified => {}
        }
    }
}
