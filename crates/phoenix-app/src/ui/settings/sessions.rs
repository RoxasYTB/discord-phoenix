use gpui::SharedString;
use parking_lot::RwLock;
use serde_json::{Value, json};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub id_hash: String,
    pub os: Option<String>,
    pub platform: Option<String>,
    pub location: Option<String>,
    pub last_used: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Screen,
    Phone,
    Headset,
}

impl Kind {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Screen => "golive/screen",
            Self::Phone => "settings/mobile-phone",
            Self::Headset => "settings/vr-headset",
        }
    }
}

static LIST: RwLock<Option<Vec<Session>>> = RwLock::new(None);
static CURRENT: RwLock<Option<String>> = RwLock::new(None);

pub fn set(list: Vec<Session>) {
    *LIST.write() = Some(list);
}

pub fn list() -> Option<Vec<Session>> {
    LIST.read().clone()
}

pub fn set_current_hash(h: Option<String>) {
    *CURRENT.write() = h.filter(|h| !h.is_empty());
}

pub fn current_hash() -> Option<String> {
    CURRENT.read().clone()
}

pub fn forget(hashes: &[String]) {
    if let Some(l) = LIST.write().as_mut() {
        l.retain(|s| !hashes.contains(&s.id_hash));
    }
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_owned)
}

pub fn parse(v: &Value) -> Vec<Session> {
    v.get("user_sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|e| {
            let info = e.get("client_info").cloned().unwrap_or(Value::Null);
            Session {
                id_hash: s(e, "id_hash").unwrap_or_default(),
                os: s(&info, "os"),
                platform: s(&info, "platform"),
                location: s(&info, "location").or_else(|| s(&info, "ip")),
                last_used: s(e, "approx_last_used_time").and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok()).map_or(0, |t| t.timestamp()),
            }
        })
        .collect()
}

pub fn split(list: &[Session], current: Option<&str>) -> (Option<Session>, Vec<Session>) {
    let mut rest = list.to_vec();
    let cur = current.and_then(|h| rest.iter().position(|s| s.id_hash == h)).map(|i| rest.remove(i));
    rest.sort_by_key(|s| std::cmp::Reverse(s.last_used));
    (cur, rest)
}

pub fn os_label(os: Option<&str>) -> (SharedString, Kind) {
    let raw = os.unwrap_or_default();
    match raw.trim().to_lowercase().as_str() {
        "" => (crate::t!("settings.devices.unknown"), Kind::Screen),
        "ios" | "android" => (raw.to_owned().into(), Kind::Phone),
        "horizon os" => (raw.to_owned().into(), Kind::Headset),
        _ => (raw.to_owned().into(), Kind::Screen),
    }
}

pub fn from_now(secs: i64) -> String {
    let d = secs.unsigned_abs() as f64;
    let r = |x: f64| x.round() as u64;
    let (seconds, minutes, hours, days) = (r(d), r(d / 60.), r(d / 3600.), r(d / 86400.));
    let months = r(d / 86400. * 4800. / 146_097.);
    let years = r(d / 86400. * 400. / 146_097.);
    let unit = |n: u64, u: &str| {
        if n <= 1 { crate::t!(&format!("unit.{u}_a")).to_string() } else { format!("{n} {}", crate::t!(&format!("unit.{u}_many"))) }
    };
    let what = if seconds <= 44 {
        crate::t!("settings.devices.few_seconds").to_string()
    } else if minutes < 45 {
        unit(minutes, "minute")
    } else if hours < 22 {
        unit(hours, "hour")
    } else if days < 26 {
        unit(days, "day")
    } else if months < 11 {
        unit(months, "month")
    } else {
        unit(years, "year")
    };
    crate::t!(if secs >= 0 { "time.ago" } else { "time.in" }, what = what).to_string()
}

pub fn last_active(now: i64, used: i64) -> String {
    if now - used < 3600 { crate::t!("settings.devices.lt_hour").to_string() } else { from_now(now - used) }
}

pub fn description(s: &Session, current: bool, now: i64) -> String {
    let last = (!current).then(|| last_active(now, s.last_used));
    [s.location.clone(), last].into_iter().flatten().collect::<Vec<_>>().join(" \u{b7} ")
}

pub fn logout_request(hashes: &[String]) -> Option<(&'static str, &'static str, Value)> {
    (!hashes.is_empty()).then(|| ("POST", "/auth/sessions/logout", json!({ "session_id_hashes": hashes })))
}

pub fn fetch(app: &crate::app::PhoenixApp, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    let fut = app.backend.auth_sessions();
    cx.spawn(async move |this, cx| {
        if let Ok(Some(l)) = fut.await {
            set(l);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

pub fn logout(app: &crate::app::PhoenixApp, hashes: Vec<String>, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    if hashes.is_empty() {
        return;
    }
    let fut = app.backend.logout_sessions(hashes.clone());
    cx.spawn(async move |this, cx| {
        if fut.await.is_ok() {
            forget(&hashes);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({ "user_sessions": [
            { "id_hash": "a", "approx_last_used_time": "2026-11-25T10:00:00+00:00", "client_info": { "os": "Linux", "platform": "Chrome", "location": "Paris, France" } },
            { "id_hash": "b", "approx_last_used_time": "2026-11-25T12:00:00+00:00", "client_info": { "os": "Android", "platform": "Discord Android", "location": "Lyon, France" } },
            { "id_hash": "c", "approx_last_used_time": "2026-11-20T12:00:00+00:00", "client_info": { "os": null, "ip": "192.0.2.1" } },
            { "id_hash": "d", "approx_last_used_time": "2026-11-25T13:00:00+00:00", "client_info": { "os": "Linux", "platform": "Discord Client", "location": "Paris, France" } }
        ]})
    }

    #[test]
    fn parses_and_splits_like_the_client() {
        let l = parse(&sample());
        assert_eq!(l.len(), 4);
        assert_eq!(l[2].location.as_deref(), Some("192.0.2.1"));
        let (cur, rest) = split(&l, Some("d"));
        assert_eq!(cur.map(|s| s.id_hash), Some("d".into()));
        assert_eq!(rest.iter().map(|s| s.id_hash.as_str()).collect::<Vec<_>>(), ["b", "a", "c"]);
        let (none, all) = split(&l, None);
        assert!(none.is_none());
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn icons_follow_the_os() {
        assert_eq!(os_label(Some("Android")).1, Kind::Phone);
        assert_eq!(os_label(Some("iOS")).1, Kind::Phone);
        assert_eq!(os_label(Some("Horizon OS")).1, Kind::Headset);
        assert_eq!(os_label(Some("Windows")), ("Windows".into(), Kind::Screen));
        assert_eq!(os_label(None).1, Kind::Screen);
    }

    #[test]
    fn last_active_uses_moment_thresholds() {
        assert_eq!(last_active(10_000, 10_000 - 1200), crate::t!("settings.devices.lt_hour").to_string());
        assert_eq!(from_now(3600), crate::t!("time.ago", what = crate::t!("unit.hour_a")).to_string());
        assert_eq!(from_now(5 * 3600), crate::t!("time.ago", what = format!("5 {}", crate::t!("unit.hour_many"))).to_string());
        assert_eq!(from_now(22 * 3600), crate::t!("time.ago", what = crate::t!("unit.day_a")).to_string());
        assert_eq!(from_now(5 * 86400), crate::t!("time.ago", what = format!("5 {}", crate::t!("unit.day_many"))).to_string());
        assert_eq!(from_now(27 * 86400), crate::t!("time.ago", what = crate::t!("unit.month_a")).to_string());
        assert_eq!(from_now(400 * 86400), crate::t!("time.ago", what = crate::t!("unit.year_a")).to_string());
    }

    #[test]
    fn description_joins_location_and_last_use() {
        let l = parse(&sample());
        let now = l[1].last_used + 2 * 86400;
        assert_eq!(description(&l[0], true, now), "Paris, France");
        assert!(description(&l[1], false, now).starts_with("Lyon, France \u{b7} "));
    }

    #[test]
    fn logout_body_matches_the_client() {
        assert!(logout_request(&[]).is_none());
        let (m, p, b) = logout_request(&["a".into(), "b".into()]).unwrap();
        assert_eq!((m, p), ("POST", "/auth/sessions/logout"));
        assert_eq!(b, json!({ "session_id_hashes": ["a", "b"] }));
    }
}
