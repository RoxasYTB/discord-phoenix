use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use reqwest::header::HeaderMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RateLimitInfo {
    pub limit: Option<u32>,
    pub remaining: Option<u32>,
    pub reset_after: Option<Duration>,
    pub bucket: Option<String>,
    pub global: bool,
    pub scope: Option<String>,
}

impl RateLimitInfo {
    pub fn from_headers(h: &HeaderMap) -> Self {
        let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok());
        Self {
            limit: get("x-ratelimit-limit").and_then(|v| v.parse().ok()),
            remaining: get("x-ratelimit-remaining").and_then(|v| v.parse().ok()),
            reset_after: get("x-ratelimit-reset-after")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
                .map(Duration::from_secs_f64),
            bucket: get("x-ratelimit-bucket").map(str::to_owned),
            global: get("x-ratelimit-global").is_some_and(|v| v.eq_ignore_ascii_case("true")),
            scope: get("x-ratelimit-scope").map(str::to_owned),
        }
    }
}

pub fn route_key(method: &str, path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path);
    let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut out = String::with_capacity(path.len() + 8);
    out.push_str(method);
    out.push(' ');
    let mut i = 0;
    while i < segs.len() {
        let seg = segs[i];
        out.push('/');
        let numeric = !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_digit());
        let is_major = i == 1 && matches!(segs[0], "channels" | "guilds" | "webhooks");
        if seg == "reactions" {
            out.push_str("reactions/*");
            break;
        } else if numeric && !is_major {
            out.push_str(":id");
        } else {
            out.push_str(seg);
        }
        i += 1;
    }
    out
}

pub fn major_param(path: &str) -> &str {
    let mut it = path.trim_matches('/').split('/');
    match (it.next(), it.next()) {
        (Some("channels" | "guilds" | "webhooks"), Some(id)) => id.split('?').next().unwrap_or(id),
        _ => "",
    }
}

#[derive(Debug, Clone)]
struct Bucket {
    limit: u32,
    remaining: u32,
    reset_at: Instant,
}

#[derive(Debug, Default)]
struct Inner {
    hashes: HashMap<String, String>,
    buckets: HashMap<String, Bucket>,
    probes: HashMap<String, Instant>,
    unlimited: HashSet<String>,
    global_until: Option<Instant>,
}

pub const PROBE_WAIT: Duration = Duration::from_millis(25);
pub const PROBE_EXPIRY: Duration = Duration::from_secs(15);

#[derive(Debug, Default)]
pub struct RateLimiter {
    inner: Mutex<Inner>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    fn bucket_id(inner: &Inner, route: &str, major: &str) -> String {
        match inner.hashes.get(route) {
            Some(h) => format!("{h}:{major}"),
            None => route.to_owned(),
        }
    }

    pub fn acquire(&self, route: &str, major: &str, now: Instant) -> Option<Duration> {
        let mut inner = self.inner.lock();
        if let Some(until) = inner.global_until {
            if until > now {
                return Some(until - now);
            }
            inner.global_until = None;
        }
        let id = Self::bucket_id(&inner, route, major);
        if !inner.buckets.contains_key(&id) {
            if inner.unlimited.contains(&id) {
                return None;
            }
            if let Some(&since) = inner.probes.get(&id)
                && now.saturating_duration_since(since) < PROBE_EXPIRY
            {
                return Some(PROBE_WAIT);
            }
            inner.probes.insert(id, now);
            return None;
        }
        let b = inner.buckets.get_mut(&id)?;
        if b.reset_at <= now {
            b.remaining = b.limit;
            b.reset_at = now + Duration::from_secs(1);
        }
        if b.remaining == 0 {
            return Some(b.reset_at - now);
        }
        b.remaining -= 1;
        None
    }

    pub fn update(&self, route: &str, major: &str, info: &RateLimitInfo, now: Instant) {
        let mut inner = self.inner.lock();
        let old = Self::bucket_id(&inner, route, major);
        inner.probes.remove(&old);
        if let Some(hash) = &info.bucket {
            inner.hashes.insert(route.to_owned(), hash.clone());
        }
        let id = Self::bucket_id(&inner, route, major);
        inner.probes.remove(&id);
        let (Some(limit), Some(remaining), Some(reset_after)) =
            (info.limit, info.remaining, info.reset_after)
        else {
            if info.bucket.is_none() {
                inner.unlimited.insert(id);
            }
            return;
        };
        let remaining = match inner.buckets.get(&id) {
            Some(b) if b.reset_at > now => remaining.min(b.remaining),
            _ => remaining,
        };
        inner.buckets.insert(id, Bucket { limit, remaining, reset_at: now + reset_after });
    }

    pub fn release(&self, route: &str, major: &str) {
        let mut inner = self.inner.lock();
        let id = Self::bucket_id(&inner, route, major);
        inner.probes.remove(&id);
    }

    pub fn on_429(&self, route: &str, major: &str, retry_after: Duration, global: bool, now: Instant) {
        let mut inner = self.inner.lock();
        if global {
            inner.global_until = Some(now + retry_after);
            return;
        }
        let id = Self::bucket_id(&inner, route, major);
        inner.probes.remove(&id);
        inner.unlimited.remove(&id);
        let b = inner.buckets.entry(id).or_insert(Bucket { limit: 1, remaining: 0, reset_at: now });
        b.remaining = 0;
        b.reset_at = now + retry_after;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderName, HeaderValue};

    #[test]
    fn route_keys() {
        assert_eq!(
            route_key("PATCH", "/channels/123/messages/456"),
            "PATCH /channels/123/messages/:id"
        );
        assert_eq!(
            route_key("PUT", "/channels/1/messages/2/reactions/%F0%9F%91%8D/@me"),
            "PUT /channels/1/messages/:id/reactions/*"
        );
        assert_eq!(route_key("GET", "/users/@me/guilds?limit=200"), "GET /users/@me/guilds");
        assert_eq!(route_key("GET", "/users/42"), "GET /users/:id");
        assert_eq!(major_param("/channels/99/typing"), "99");
        assert_eq!(major_param("/users/@me"), "");
    }

    #[test]
    fn parse_headers() {
        let mut h = HeaderMap::new();
        for (k, v) in [
            ("x-ratelimit-limit", "5"),
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset-after", "1.250"),
            ("x-ratelimit-bucket", "abcd"),
        ] {
            h.insert(HeaderName::from_static(k), HeaderValue::from_static(v));
        }
        let info = RateLimitInfo::from_headers(&h);
        assert_eq!(info.limit, Some(5));
        assert_eq!(info.remaining, Some(0));
        assert_eq!(info.reset_after, Some(Duration::from_millis(1250)));
        assert_eq!(info.bucket.as_deref(), Some("abcd"));
        assert!(!info.global);
    }

    #[test]
    fn bucket_exhaustion_and_reset() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        let route = "POST /channels/1/messages";
        assert_eq!(rl.acquire(route, "1", t0), None);
        let info = RateLimitInfo {
            limit: Some(2),
            remaining: Some(1),
            reset_after: Some(Duration::from_secs(2)),
            bucket: Some("h".into()),
            ..Default::default()
        };
        rl.update(route, "1", &info, t0);
        assert_eq!(rl.acquire(route, "1", t0), None);
        assert_eq!(rl.acquire(route, "1", t0), Some(Duration::from_secs(2)));
        assert_eq!(rl.acquire(route, "2", t0), None);
        let t1 = t0 + Duration::from_secs(3);
        assert_eq!(rl.acquire(route, "1", t1), None);
        assert_eq!(rl.acquire(route, "1", t1), None);
        assert!(rl.acquire(route, "1", t1).is_some());
    }

    #[test]
    fn shared_hash_buckets() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        let info = RateLimitInfo {
            limit: Some(1),
            remaining: Some(0),
            reset_after: Some(Duration::from_secs(1)),
            bucket: Some("shared".into()),
            ..Default::default()
        };
        rl.update("PATCH /channels/1/messages/:id", "1", &info, t0);
        rl.update(
            "DELETE /channels/1/messages/:id",
            "1",
            &RateLimitInfo { bucket: Some("shared".into()), ..Default::default() },
            t0,
        );
        assert!(rl.acquire("DELETE /channels/1/messages/:id", "1", t0).is_some());
    }

    #[test]
    fn global_and_429() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        rl.on_429("GET /x", "", Duration::from_millis(500), false, t0);
        assert_eq!(rl.acquire("GET /x", "", t0), Some(Duration::from_millis(500)));
        assert_eq!(rl.acquire("GET /y", "", t0), None);
        rl.on_429("GET /y", "", Duration::from_secs(1), true, t0);
        assert_eq!(rl.acquire("GET /z", "", t0), Some(Duration::from_secs(1)));
        assert_eq!(rl.acquire("GET /z", "", t0 + Duration::from_secs(2)), None);
    }

    #[test]
    fn unknown_bucket_lets_one_request_through_until_headers_arrive() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        let route = "GET /users/@me/notes/:id";
        assert_eq!(rl.acquire(route, "", t0), None);
        assert_eq!(rl.acquire(route, "", t0), Some(PROBE_WAIT));
        assert_eq!(rl.acquire("GET /users/:id", "", t0), None);
        let info = RateLimitInfo {
            limit: Some(3),
            remaining: Some(2),
            reset_after: Some(Duration::from_secs(1)),
            bucket: Some("n".into()),
            ..Default::default()
        };
        rl.update(route, "", &info, t0);
        assert_eq!(rl.acquire(route, "", t0), None);
        assert_eq!(rl.acquire(route, "", t0), None);
        assert_eq!(rl.acquire(route, "", t0), Some(Duration::from_secs(1)));
    }

    #[test]
    fn stale_responses_do_not_give_back_spent_requests() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        let route = "GET /users/:id/profile";
        let info = |remaining| RateLimitInfo {
            limit: Some(5),
            remaining: Some(remaining),
            reset_after: Some(Duration::from_secs(5)),
            bucket: Some("p".into()),
            ..Default::default()
        };
        assert_eq!(rl.acquire(route, "", t0), None);
        rl.update(route, "", &info(4), t0);
        for _ in 0..4 {
            assert_eq!(rl.acquire(route, "", t0), None);
        }
        rl.update(route, "", &info(3), t0);
        assert!(rl.acquire(route, "", t0).is_some());
        rl.update(route, "", &info(4), t0 + Duration::from_secs(6));
        assert_eq!(rl.acquire(route, "", t0 + Duration::from_secs(6)), None);
    }

    #[test]
    fn probes_are_released_on_failure_and_expire() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        assert_eq!(rl.acquire("GET /a", "", t0), None);
        rl.release("GET /a", "");
        assert_eq!(rl.acquire("GET /a", "", t0), None);
        assert_eq!(rl.acquire("GET /a", "", t0), Some(PROBE_WAIT));
        assert_eq!(rl.acquire("GET /a", "", t0 + PROBE_EXPIRY), None);
    }

    #[test]
    fn routes_without_rate_limit_headers_are_not_serialised() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        assert_eq!(rl.acquire("GET /b", "", t0), None);
        rl.update("GET /b", "", &RateLimitInfo::default(), t0);
        assert_eq!(rl.acquire("GET /b", "", t0), None);
        assert_eq!(rl.acquire("GET /b", "", t0), None);
        rl.on_429("GET /b", "", Duration::from_secs(1), false, t0);
        assert!(rl.acquire("GET /b", "", t0).is_some());
    }
}
