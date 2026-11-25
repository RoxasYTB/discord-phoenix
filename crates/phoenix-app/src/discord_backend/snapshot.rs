#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    fs,
    io::{BufRead, BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::{Duration, SystemTime},
};

use phoenix_api::{
    Cache, GatewayEvent,
    gateway::{RawDispatchHook, ResumeState, events::Ready, parse_dispatch},
};

use crate::backend::{AppEvent, ReadyData};

use super::{
    convert::{self, Adapter},
    nav,
};

const NAMES: [&str; 2] = ["READY", "READY_SUPPLEMENTAL"];
const RESUME_WINDOW: Duration = Duration::from_secs(150);
const JOURNAL_CAP: u64 = 48 * 1024 * 1024;

pub fn dir() -> Option<PathBuf> {
    Some(crate::paths::cache_dir()?.join("session"))
}

fn file(name: &str) -> Option<PathBuf> {
    Some(dir()?.join(format!("{name}.json")))
}

fn journal_path() -> Option<PathBuf> {
    Some(dir()?.join("journal.log"))
}

fn owner_path() -> Option<PathBuf> {
    Some(dir()?.join("owner.lock"))
}

fn try_own(p: &std::path::Path) -> Option<fs::File> {
    let mut o = fs::OpenOptions::new();
    o.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    o.mode(0o600);
    let f = o.open(p).ok()?;
    match f.try_lock() {
        Ok(()) => Some(f),
        Err(fs::TryLockError::WouldBlock) => None,
        Err(fs::TryLockError::Error(e)) if e.kind() == std::io::ErrorKind::Unsupported => Some(f),
        Err(fs::TryLockError::Error(_)) => None,
    }
}

fn other_owner() -> bool {
    static HELD: Mutex<Option<fs::File>> = Mutex::new(None);
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    if held.is_none() {
        *held = owner_path().and_then(|p| try_own(&p));
    }
    held.is_none()
}

fn open_private(p: &std::path::Path, append: bool) -> std::io::Result<fs::File> {
    let mut o = fs::OpenOptions::new();
    o.create(true);
    #[cfg(unix)]
    o.mode(0o600);
    if append {
        o.append(true);
    } else {
        o.write(true).truncate(true);
    }
    o.open(p)
}

fn write_atomic(p: &std::path::Path, bytes: &[u8]) {
    let tmp = p.with_extension("part");
    let ok = open_private(&tmp, false).and_then(|mut f| f.write_all(bytes)).is_ok();
    if !(ok && fs::rename(&tmp, p).is_ok()) {
        let _ = fs::remove_file(&tmp);
    }
}

struct Rec {
    name: String,
    raw: String,
    seq: Option<u64>,
}

struct Files {
    jp: PathBuf,
    journal: Option<BufWriter<fs::File>>,
    size: u64,
}

impl Files {
    fn handle(&mut self, r: Rec) {
        match r.name.as_str() {
            "READY" => {
                self.journal = open_private(&self.jp, false).ok().map(BufWriter::new);
                self.size = 0;
                if let Some(p) = file("READY") {
                    write_atomic(&p, r.raw.as_bytes());
                }
            }
            "READY_SUPPLEMENTAL" => {
                if let Some(p) = file("READY_SUPPLEMENTAL") {
                    write_atomic(&p, r.raw.as_bytes());
                }
            }
            "TYPING_START" => {}
            _ => {
                if self.journal.is_none() && self.size == 0 && exists() {
                    self.journal = open_private(&self.jp, true).ok().map(BufWriter::new);
                    self.size = fs::metadata(&self.jp).map_or(0, |m| m.len());
                }
                if let Some(w) = self.journal.as_mut() {
                    let line = format!("{}\t{}\t{}\n", r.seq.unwrap_or(0), r.name, r.raw);
                    self.size += line.len() as u64;
                    if w.write_all(line.as_bytes()).is_err() || self.size > JOURNAL_CAP {
                        self.journal = None;
                        self.size = u64::MAX;
                        let _ = fs::remove_file(&self.jp);
                    }
                }
            }
        }
    }
}

fn writer(rx: mpsc::Receiver<Rec>) {
    let Some(d) = dir() else { return };
    let mut db = fs::DirBuilder::new();
    db.recursive(true);
    #[cfg(unix)]
    db.mode(0o700);
    if db.create(&d).is_err() || other_owner() {
        return;
    }
    let Some(jp) = journal_path() else { return };
    let mut files = Files { jp, journal: None, size: 0 };
    while let Ok(first) = rx.recv() {
        let mut next = Some(first);
        while let Some(r) = next {
            files.handle(r);
            next = rx.try_recv().ok();
        }
        if let Some(w) = files.journal.as_mut() {
            let _ = w.flush();
        }
    }
}

pub fn hook() -> RawDispatchHook {
    static TX: OnceLock<Mutex<mpsc::Sender<Rec>>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new().name("phoenix-snapshot".into()).spawn(move || writer(rx));
        Mutex::new(tx)
    });
    Arc::new(move |name: &str, raw: &str, seq: Option<u64>| {
        let rec = Rec { name: name.to_owned(), raw: raw.to_owned(), seq };
        let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(rec);
    })
}

pub fn exists() -> bool {
    file("READY").is_some_and(|p| p.is_file())
}

pub fn clear() {
    for n in NAMES {
        if let Some(p) = file(n) {
            let _ = fs::remove_file(p);
        }
    }
    if let Some(p) = journal_path() {
        let _ = fs::remove_file(p);
    }
    if let Some(d) = dir() {
        let _ = fs::remove_file(d.join("owner.pid"));
    }
}

pub struct Core {
    pub cache: Cache,
    pub ready: Box<Ready>,
    pub adapter: Adapter,
    pub resume: Option<ResumeState>,
}

type Ui = (ReadyData, Vec<AppEvent>);

fn load() -> Option<(Ui, Core)> {
    let t0 = std::time::Instant::now();
    let raw = fs::read_to_string(file("READY")?).ok()?;
    crate::perf::span("snapshot_read", t0);
    let GatewayEvent::Ready(ready) = parse_dispatch("READY", &raw) else { return None };
    crate::perf::span("snapshot_parsed", t0);
    drop(raw);
    let cache = Cache::default();
    let ev = GatewayEvent::Ready(ready);
    cache.apply(&ev);
    let GatewayEvent::Ready(ready) = ev else { return None };
    let mut adapter = Adapter::new(ready.user.id, convert::Prefs::from_ready(&ready));
    let mut people = Vec::new();
    if let Some(sup) = file("READY_SUPPLEMENTAL").and_then(|p| fs::read_to_string(p).ok()) {
        let ev = parse_dispatch("READY_SUPPLEMENTAL", &sup);
        if matches!(ev, GatewayEvent::ReadySupplemental(_)) {
            cache.apply(&ev);
            people.extend(presence_events(nav::on_gateway(&cache, &ev)));
            people.extend(presence_events(adapter.map(&cache, &ev)));
        }
    }
    let (last_seq, fresh) = replay(&cache, &mut adapter, &mut people);
    crate::perf::span("snapshot_replayed", t0);
    let data = convert::build_ready(&cache, &ready, &adapter.prefs);
    crate::perf::span("snapshot_built", t0);
    let mut events: Vec<AppEvent> = convert::self_activities(&ready).into_iter().collect();
    events.extend(convert::self_status(&ready));
    events.extend(nav::snapshot(&cache, &ready));
    events.extend(people);
    let resume = (fresh && !other_owner())
        .then_some(last_seq)
        .flatten()
        .map(|seq| ResumeState { session_id: ready.session_id.clone(), resume_url: ready.resume_gateway_url.clone(), seq });
    Some(((data, events), Core { cache, ready, adapter, resume }))
}

fn presence_events(events: Vec<AppEvent>) -> impl Iterator<Item = AppEvent> {
    events.into_iter().filter(|e| {
        matches!(e, AppEvent::PresenceReset | AppEvent::PresenceUpdate { .. } | AppEvent::Activities { .. } | AppEvent::Nav(crate::ui::home::data::NavEvent::Activity { .. }))
    })
}

fn replay(cache: &Cache, adapter: &mut Adapter, people: &mut Vec<AppEvent>) -> (Option<u64>, bool) {
    let Some(jp) = journal_path() else { return (None, false) };
    let Ok(f) = fs::File::open(&jp) else { return (None, false) };
    let fresh = f
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < RESUME_WINDOW);
    let mut last = None;
    for line in std::io::BufReader::new(f).lines() {
        let Ok(line) = line else { break };
        let mut parts = line.splitn(3, '\t');
        let (Some(seq), Some(name), Some(raw)) = (parts.next(), parts.next(), parts.next()) else { break };
        let ev = parse_dispatch(name, raw);
        cache.apply(&ev);
        let mapped = adapter.map(cache, &ev);
        if matches!(ev, GatewayEvent::PresenceUpdate(_)) {
            people.extend(presence_events(nav::on_gateway(cache, &ev)));
            people.extend(presence_events(mapped));
        }
        if let Ok(s) = seq.parse::<u64>()
            && s > 0
        {
            last = Some(s);
        }
    }
    (last, fresh)
}

#[derive(Default)]
struct Shared {
    rx: Option<mpsc::Receiver<Option<(Ui, Core)>>>,
    ui: Option<Ui>,
    core: Option<Core>,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

pub fn preload() {
    if !exists() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    if std::thread::Builder::new()
        .name("phoenix-snapshot-load".into())
        .spawn(move || {
            let _ = tx.send(load());
        })
        .is_ok()
    {
        *SHARED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Shared { rx: Some(rx), ..Default::default() });
    }
}

fn with_loaded<T>(f: impl FnOnce(&mut Shared) -> Option<T>) -> Option<T> {
    let mut g = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    let sh = g.as_mut()?;
    if let Some(rx) = sh.rx.take()
        && let Ok(Some((ui, core))) = rx.recv()
    {
        sh.ui = Some(ui);
        sh.core = Some(core);
    }
    f(sh)
}

pub fn take() -> Option<Ui> {
    with_loaded(|s| s.ui.take())
}

pub fn take_core() -> Option<Core> {
    with_loaded(|s| s.core.take()).filter(|c| c.resume.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phoenix-owner-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn second_owner_is_refused_while_first_holds_the_lock() {
        let d = scratch("held");
        let p = d.join("owner.lock");
        let first = try_own(&p).expect("first owner");
        assert!(try_own(&p).is_none());
        drop(first);
        assert!(try_own(&p).is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn stale_lock_file_does_not_block_a_new_owner() {
        let d = scratch("stale");
        let p = d.join("owner.lock");
        fs::write(&p, b"12345").unwrap();
        assert!(try_own(&p).is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn lock_survives_its_file_being_reopened() {
        let d = scratch("reopen");
        let p = d.join("owner.lock");
        let _first = try_own(&p).expect("first owner");
        let again = fs::OpenOptions::new().read(true).write(true).open(&p).unwrap();
        assert!(matches!(again.try_lock(), Err(fs::TryLockError::WouldBlock)));
        let _ = fs::remove_dir_all(&d);
    }
}
