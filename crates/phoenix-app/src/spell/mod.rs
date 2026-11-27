pub mod dicts;
pub mod words;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use gpui::App;
use parking_lot::Mutex;
use spellbook::Dictionary;

pub const MAX_SUGGESTIONS: usize = 5;
const MAX_ELIDED_HEAD: usize = 6;
const LEARNED_FILE: &str = "spellcheck.json";

struct Loaded {
    key: Vec<String>,
    dicts: Arc<Vec<Dictionary>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Suggest {
    word: String,
    key: Vec<String>,
    list: Option<Vec<String>>,
}

struct Engine {
    available: Vec<dicts::Available>,
    wanted: Mutex<(String, Vec<String>)>,
    loaded: Mutex<Option<Loaded>>,
    cache: Mutex<HashMap<String, bool>>,
    cache_key: Mutex<Vec<String>>,
    pending: Mutex<HashSet<String>>,
    learned: Mutex<HashSet<String>>,
    suggest: Mutex<Option<Suggest>>,
    generation: AtomicU64,
}

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| Engine {
        available: dicts::scan(&dicts::search_dirs()),
        wanted: Mutex::new((String::new(), Vec::new())),
        loaded: Mutex::new(None),
        cache: Mutex::new(HashMap::new()),
        cache_key: Mutex::new(Vec::new()),
        pending: Mutex::new(HashSet::new()),
        learned: Mutex::new(load_learned()),
        suggest: Mutex::new(None),
        generation: AtomicU64::new(0),
    })
}

fn learned_path() -> Option<std::path::PathBuf> {
    crate::paths::config_file(LEARNED_FILE)
}

pub fn parse_learned(bytes: &[u8]) -> HashSet<String> {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|v| v.get("learnedWords").and_then(|w| w.as_array()).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|w| w.as_str().map(str::to_lowercase))
        .collect()
}

pub fn learned_json(words: &HashSet<String>) -> Vec<u8> {
    let mut list: Vec<&String> = words.iter().collect();
    list.sort();
    serde_json::to_vec_pretty(&serde_json::json!({ "learnedWords": list })).unwrap_or_default()
}

fn load_learned() -> HashSet<String> {
    learned_path().and_then(|p| std::fs::read(p).ok()).map(|b| parse_learned(&b)).unwrap_or_default()
}

fn save_learned(words: &HashSet<String>) {
    let Some(path) = learned_path() else { return };
    let data = learned_json(words);
    std::thread::spawn(move || {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &data).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    });
}

impl Engine {
    fn wanted(&self) -> Vec<String> {
        let locale = crate::i18n::locale();
        let mut w = self.wanted.lock();
        if w.0 != locale || w.0.is_empty() {
            let candidates = dicts::candidates(locale, &dicts::system_languages());
            w.1 = dicts::resolve(&candidates, &self.available).into_iter().map(|a| a.code.clone()).collect();
            w.0 = locale.to_owned();
        }
        w.1.clone()
    }

    fn sync_key(&self) -> Vec<String> {
        let key = self.wanted();
        let mut current = self.cache_key.lock();
        if *current != key {
            current.clone_from(&key);
            self.cache.lock().clear();
            self.pending.lock().clear();
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
        key
    }

    fn dictionaries(&self, key: &[String]) -> Arc<Vec<Dictionary>> {
        let mut loaded = self.loaded.lock();
        if let Some(l) = loaded.as_ref()
            && l.key == key
        {
            return l.dicts.clone();
        }
        let t0 = std::time::Instant::now();
        let list: Vec<Dictionary> = key.iter().filter_map(|c| self.available.iter().find(|a| &a.code == c)).filter_map(dicts::load).collect();
        tracing::info!("spellcheck: loaded {key:?} in {:?}", t0.elapsed());
        let dicts = Arc::new(list);
        *loaded = Some(Loaded { key: key.to_vec(), dicts: dicts.clone() });
        dicts
    }
}

pub fn has_dictionaries() -> bool {
    !engine().available.is_empty()
}

pub fn active_languages() -> Vec<String> {
    engine().wanted()
}

pub fn generation() -> u64 {
    engine().generation.load(Ordering::Relaxed)
}

fn bump() {
    engine().generation.fetch_add(1, Ordering::Relaxed);
}

pub fn check_word(dicts: &[Dictionary], word: &str) -> bool {
    if dicts.is_empty() {
        return true;
    }
    let ok = |w: &str| dicts.iter().any(|d| d.check(w));
    if ok(word) {
        return true;
    }
    for (from, to) in [('\'', '’'), ('’', '\'')] {
        if word.contains(from) && ok(&word.replace(from, &to.to_string())) {
            return true;
        }
    }
    let Some(i) = word.rfind(['\'', '’']) else { return false };
    let mark = word[i..].chars().next().map_or(1, char::len_utf8);
    let (head, tail) = (&word[..i], &word[i + mark..]);
    !tail.is_empty() && head.chars().count() <= MAX_ELIDED_HEAD && head.chars().all(char::is_alphabetic) && ok(tail)
}

pub fn suggest_word(dicts: &[Dictionary], word: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = Vec::new();
    for d in dicts {
        buf.clear();
        d.suggest(word, &mut buf);
        for s in buf.drain(..) {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out.truncate(MAX_SUGGESTIONS);
    out
}

pub fn classify(text: &str, caret: Option<usize>, lookup: impl Fn(&str) -> Option<bool>) -> (Vec<Range<usize>>, Vec<String>) {
    let mut bad = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for r in words::words(text, caret) {
        let w = &text[r.clone()];
        match lookup(w) {
            Some(false) => bad.push(r),
            Some(true) => {}
            None => {
                if !missing.iter().any(|m| m == w) {
                    missing.push(w.to_owned());
                }
            }
        }
    }
    (bad, missing)
}

pub fn misspelled(text: &str, caret: Option<usize>, cx: &App) -> Vec<Range<usize>> {
    let eng = engine();
    if eng.available.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let key = eng.sync_key();
    if key.is_empty() {
        return Vec::new();
    }
    let (bad, missing) = {
        let cache = eng.cache.lock();
        let learned = eng.learned.lock();
        classify(text, caret, |w| if learned.contains(&w.to_lowercase()) { Some(true) } else { cache.get(w).copied() })
    };
    if !missing.is_empty() {
        request(key, missing, cx);
    }
    bad
}

fn request(key: Vec<String>, words: Vec<String>, cx: &App) {
    let eng = engine();
    let words: Vec<String> = {
        let mut pending = eng.pending.lock();
        words.into_iter().filter(|w| pending.insert(w.clone())).collect()
    };
    if words.is_empty() {
        return;
    }
    let bg = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let job_key = key.clone();
        let results = bg
            .spawn(async move {
                let dicts = engine().dictionaries(&job_key);
                words.into_iter().map(|w| (check_word(&dicts, &w), w)).collect::<Vec<_>>()
            })
            .await;
        let eng = engine();
        if *eng.cache_key.lock() != key {
            return;
        }
        {
            let mut cache = eng.cache.lock();
            let mut pending = eng.pending.lock();
            for (ok, w) in results {
                pending.remove(&w);
                cache.insert(w, ok);
            }
        }
        bump();
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

pub fn is_misspelled(word: &str) -> bool {
    let eng = engine();
    !eng.learned.lock().contains(&word.to_lowercase()) && eng.cache.lock().get(word) == Some(&false)
}

pub fn suggestions(word: &str) -> Option<Vec<String>> {
    let eng = engine();
    let key = eng.cache_key.lock().clone();
    eng.suggest.lock().as_ref().filter(|s| s.word == word && s.key == key).and_then(|s| s.list.clone())
}

pub fn request_suggestions(word: &str, cx: &App) {
    let eng = engine();
    let key = eng.sync_key();
    {
        let mut s = eng.suggest.lock();
        if s.as_ref().is_some_and(|s| s.word == word && s.key == key) {
            return;
        }
        *s = Some(Suggest { word: word.to_owned(), key: key.clone(), list: None });
    }
    let bg = cx.background_executor().clone();
    let word = word.to_owned();
    cx.spawn(async move |cx| {
        let (job_key, job_word) = (key.clone(), word.clone());
        let list = bg.spawn(async move { suggest_word(&engine().dictionaries(&job_key), &job_word) }).await;
        {
            let mut s = engine().suggest.lock();
            match s.as_mut() {
                Some(cur) if cur.word == word && cur.key == key => cur.list = Some(list),
                _ => return,
            }
        }
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

pub fn learn(word: &str) {
    let eng = engine();
    let mut learned = eng.learned.lock();
    learned.insert(word.to_lowercase());
    save_learned(&learned);
    drop(learned);
    bump();
}

pub fn unlearn(word: &str) {
    let eng = engine();
    let mut learned = eng.learned.lock();
    learned.remove(&word.to_lowercase());
    save_learned(&learned);
    drop(learned);
    bump();
}

pub fn learned_in(text: &str) -> Option<String> {
    let learned = engine().learned.lock();
    let mut list: Vec<&String> = learned.iter().collect();
    list.sort();
    list.into_iter().find(|w| words::contains_word(text, w)).cloned()
}
