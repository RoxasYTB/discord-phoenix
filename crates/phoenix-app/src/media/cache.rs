use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::SystemTime,
};

use futures::{AsyncReadExt, FutureExt, future::BoxFuture};
use gpui::http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};

pub const CAP_BYTES: u64 = 300 * 1024 * 1024;

pub fn default_dir() -> Option<PathBuf> {
    Some(crate::paths::cache_dir()?.join("media"))
}

fn key(url: &str) -> String {
    let mut h = std::hash::DefaultHasher::new();
    url.hash(&mut h);
    format!("{:016x}{:04x}", h.finish(), url.len() & 0xffff)
}

struct Store {
    dir: PathBuf,
    cap: u64,
    used: AtomicU64,
    evicting: AtomicBool,
}

impl Store {
    fn path(&self, url: &str) -> PathBuf {
        self.dir.join(key(url))
    }

    fn read(&self, url: &str) -> Option<Vec<u8>> {
        let p = self.path(url);
        let bytes = fs::read(&p).ok()?;
        if let Ok(f) = fs::File::options().write(true).open(&p) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    fn write(&self, url: &str, bytes: &[u8]) {
        if bytes.is_empty() || bytes.len() as u64 > self.cap / 4 {
            return;
        }
        let p = self.path(url);
        let tmp = p.with_extension("part");
        if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, &p).is_ok() {
            let used = self.used.fetch_add(bytes.len() as u64, Ordering::Relaxed) + bytes.len() as u64;
            if used > self.cap {
                self.evict();
            }
        } else {
            let _ = fs::remove_file(&tmp);
        }
    }

    fn evict(&self) {
        if self.evicting.swap(true, Ordering::AcqRel) {
            return;
        }
        let total = evict_dir(&self.dir, self.cap * 9 / 10);
        self.used.store(total, Ordering::Relaxed);
        self.evicting.store(false, Ordering::Release);
    }
}

fn evict_dir(dir: &Path, target: u64) -> u64 {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    for (_, len, p) in files {
        if total <= target {
            break;
        }
        if fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
    total
}

pub struct CachingHttpClient {
    inner: Arc<dyn HttpClient>,
    store: Option<Arc<Store>>,
}

impl CachingHttpClient {
    pub fn new(inner: Arc<dyn HttpClient>) -> Self {
        Self::with_dir(inner, default_dir(), CAP_BYTES)
    }

    pub fn with_dir(inner: Arc<dyn HttpClient>, dir: Option<PathBuf>, cap: u64) -> Self {
        let store = dir.filter(|d| fs::create_dir_all(d).is_ok()).map(|dir| {
            let s = Arc::new(Store { dir, cap, used: AtomicU64::new(0), evicting: AtomicBool::new(true) });
            let bg = s.clone();
            std::thread::Builder::new()
                .name("media-cache-scan".into())
                .spawn(move || {
                    let total = evict_dir(&bg.dir, bg.cap);
                    bg.used.fetch_add(total, Ordering::Relaxed);
                    bg.evicting.store(false, Ordering::Release);
                })
                .ok();
            s
        });
        Self { inner, store }
    }
}

impl HttpClient for CachingHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        self.inner.user_agent()
    }

    fn proxy(&self) -> Option<&Url> {
        self.inner.proxy()
    }

    fn send(&self, req: Request<AsyncBody>) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let Some(store) = self.store.clone().filter(|_| req.method() == "GET") else {
            return self.inner.send(req);
        };
        let url = req.uri().to_string();
        let t0 = std::time::Instant::now();
        let inner = self.inner.clone();
        async move {
            if let Some(bytes) = store.read(&url) {
                crate::perf::span("media_disk", t0);
                return Ok(Response::builder().status(200).body(AsyncBody::from(bytes))?);
            }
            let resp = inner.send(req).await?;
            crate::perf::span("media_net", t0);
            if !resp.status().is_success() {
                return Ok(resp);
            }
            let (parts, mut body) = resp.into_parts();
            let mut bytes = Vec::new();
            body.read_to_end(&mut bytes).await?;
            store.write(&url, &bytes);
            Ok(Response::from_parts(parts, AsyncBody::from(bytes)))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_first() {
        let dir = std::env::temp_dir().join(format!("phoenix-media-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let p = dir.join(name);
            fs::write(&p, [0u8; 100]).unwrap();
            let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64);
            fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        assert_eq!(evict_dir(&dir, 200), 200);
        assert!(!dir.join("old").exists());
        assert!(dir.join("mid").exists() && dir.join("new").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(key("https://a/x.png"), key("https://a/x.png"));
        assert_ne!(key("https://a/x.png"), key("https://a/y.png"));
    }
}
