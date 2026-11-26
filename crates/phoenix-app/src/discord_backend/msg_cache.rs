#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

use parking_lot::Mutex;
use phoenix_api::{GatewayEvent, models as api};

const MEM_CAP: usize = 64;
const PAGE: usize = 50;
const DISK_CAP: usize = 400;

struct Page {
    msgs: Vec<api::Message>,
    fresh: bool,
    tick: u64,
}

#[derive(Default)]
struct State {
    pages: HashMap<u64, Page>,
    inflight: HashSet<u64>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);
static TICK: AtomicU64 = AtomicU64::new(0);
static WRITES: AtomicU64 = AtomicU64::new(0);

fn dir() -> Option<PathBuf> {
    Some(super::snapshot::dir()?.parent()?.join("messages"))
}

fn file(channel: u64) -> Option<PathBuf> {
    Some(dir()?.join(format!("{channel}.json")))
}

pub fn get(channel: u64) -> Option<(Vec<api::Message>, bool)> {
    {
        let mut st = STATE.lock();
        if let Some(p) = st.pages.get_mut(&channel) {
            p.tick = TICK.fetch_add(1, Ordering::Relaxed);
            return Some((p.msgs.clone(), p.fresh));
        }
    }
    let raw = fs::read(file(channel)?).ok()?;
    let mut msgs: Vec<api::Message> = serde_json::from_slice(&raw).ok()?;
    msgs.sort_by_key(|m| m.id);
    insert(channel, msgs.clone(), false);
    Some((msgs, false))
}

pub fn peek(channel: u64) -> Option<Vec<api::Message>> {
    STATE.lock().pages.get(&channel).map(|p| p.msgs.clone())
}

pub fn has_fresh(channel: u64) -> bool {
    STATE.lock().pages.get(&channel).is_some_and(|p| p.fresh)
}

pub fn begin_fetch(channel: u64) -> bool {
    STATE.lock().inflight.insert(channel)
}

pub fn end_fetch(channel: u64) {
    STATE.lock().inflight.remove(&channel);
}

fn insert(channel: u64, msgs: Vec<api::Message>, fresh: bool) {
    let mut st = STATE.lock();
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    st.pages.insert(channel, Page { msgs, fresh, tick });
    if st.pages.len() > MEM_CAP
        && let Some(old) = st.pages.iter().min_by_key(|(_, p)| p.tick).map(|(k, _)| *k)
    {
        st.pages.remove(&old);
    }
}

pub fn put(channel: u64, msgs: &[api::Message]) {
    put_with(channel, msgs, true);
}

pub fn put_with(channel: u64, msgs: &[api::Message], fresh: bool) {
    insert(channel, msgs.to_vec(), fresh);
    let Ok(json) = serde_json::to_vec(msgs) else { return };
    let _ = std::thread::Builder::new().name("phoenix-msgcache".into()).spawn(move || write(channel, &json));
}

fn write(channel: u64, json: &[u8]) {
    let (Some(d), Some(p)) = (dir(), file(channel)) else { return };
    let mut db = fs::DirBuilder::new();
    db.recursive(true);
    #[cfg(unix)]
    db.mode(0o700);
    if db.create(&d).is_err() {
        return;
    }
    let tmp = p.with_extension("part");
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let ok = opts
        .open(&tmp)
        .and_then(|mut f| f.write_all(json))
        .is_ok();
    if !(ok && fs::rename(&tmp, &p).is_ok()) {
        let _ = fs::remove_file(&tmp);
    }
    if WRITES.fetch_add(1, Ordering::Relaxed) & 31 == 0 {
        trim(&d);
    }
}

fn trim(d: &std::path::Path) {
    let mut files: Vec<(SystemTime, PathBuf)> = fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= DISK_CAP {
        return;
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, p) in files.into_iter().skip(DISK_CAP) {
        let _ = fs::remove_file(p);
    }
}

pub fn recent_on_disk(n: usize) -> Vec<u64> {
    let Some(d) = dir() else { return Vec::new() };
    let mut files: Vec<(SystemTime, u64)> = fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_str()?.strip_suffix(".json")?.parse().ok()?;
            Some((e.metadata().ok()?.modified().ok()?, id))
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.into_iter().take(n).map(|f| f.1).collect()
}

fn mark_stale(channel: u64) {
    if let Some(p) = STATE.lock().pages.get_mut(&channel) {
        p.fresh = false;
    }
}

pub fn mark_all_stale() {
    for p in STATE.lock().pages.values_mut() {
        p.fresh = false;
    }
}

fn remove(channel: u64, ids: &[api::Snowflake]) {
    let mut st = STATE.lock();
    let Some(p) = st.pages.get_mut(&channel) else { return };
    let full = p.msgs.len() >= PAGE;
    p.msgs.retain(|m| !ids.contains(&m.id));
    if full && p.msgs.len() < PAGE {
        st.pages.remove(&channel);
        if let Some(f) = file(channel) {
            let _ = fs::remove_file(f);
        }
    }
}

pub fn on_gateway(ev: &GatewayEvent) {
    match ev {
        GatewayEvent::MessageCreate(m) => {
            let mut st = STATE.lock();
            if let Some(p) = st.pages.get_mut(&m.channel_id.0)
                && !p.msgs.iter().any(|x| x.id == m.id)
            {
                p.msgs.push((**m).clone());
                if p.msgs.len() > PAGE {
                    let extra = p.msgs.len() - PAGE;
                    p.msgs.drain(..extra);
                }
            }
        }
        GatewayEvent::MessageDelete(d) => remove(d.channel_id.0, &[d.id]),
        GatewayEvent::MessageDeleteBulk(d) => remove(d.channel_id.0, &d.ids),
        GatewayEvent::MessageUpdate(v) => {
            if let Some(c) = v.get("channel_id").and_then(|c| c.as_str()).and_then(|c| c.parse().ok()) {
                mark_stale(c);
            }
        }
        GatewayEvent::MessageReactionAdd(r) | GatewayEvent::MessageReactionRemove(r) => mark_stale(r.channel_id.0),
        GatewayEvent::MessageReactionAddMany(r) => mark_stale(r.channel_id.0),
        GatewayEvent::MessageReactionRemoveAll(r) | GatewayEvent::MessageReactionRemoveEmoji(r) => mark_stale(r.channel_id.0),
        GatewayEvent::Ready(_) => mark_all_stale(),
        _ => {}
    }
}

pub fn clear() {
    *STATE.lock() = State::default();
    if let Some(d) = dir() {
        let _ = fs::remove_dir_all(d);
    }
}
