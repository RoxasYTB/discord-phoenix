use phoenix_api::{
    Cache,
    gateway::{GatewayCommand, UpdatePresence},
    rest::settings::CustomStatusProto,
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Once,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use super::nav::fields;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Custom {
    pub proto: CustomStatusProto,
    pub label: Option<String>,
    pub animated: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Local {
    pub status: Option<String>,
    pub custom: Option<Custom>,
    pub afk_timeout: Option<u32>,
    pub idle_since: u64,
    pub afk: bool,
}

pub(crate) const IDLE_MS: u32 = 600_000;
const AFK_DEFAULT_S: u32 = 60;

static IDLE_SINCE: AtomicU64 = AtomicU64::new(0);
static AFK: AtomicBool = AtomicBool::new(false);
static WATCH: Once = Once::new();

fn sub(b: &[u8], n: u64) -> Option<&[u8]> {
    fields(b).into_iter().rev().find(|f| f.0 == n && f.1 == 2).map(|f| f.2)
}

fn text(b: &[u8], n: u64) -> String {
    sub(b, n).and_then(|v| std::str::from_utf8(v).ok()).unwrap_or_default().to_owned()
}

fn fixed(b: &[u8], n: u64) -> u64 {
    fields(b).into_iter().rev().find(|f| f.0 == n && f.1 == 1).map_or(0, |f| f.3)
}

fn custom(b: &[u8]) -> Custom {
    Custom {
        proto: CustomStatusProto {
            text: text(b, 1),
            emoji_id: fixed(b, 2),
            emoji_name: text(b, 3),
            expires_at_ms: fixed(b, 4),
            created_at_ms: fixed(b, 5),
        },
        label: sub(b, 6).map(|l| text(l, 1)),
        animated: None,
    }
}

impl Local {
    pub fn from_proto(raw: &[u8]) -> Self {
        let mut l = Self::default();
        l.apply(raw, false);
        l
    }

    pub fn from_base64(b64: Option<&str>) -> Self {
        use base64::Engine;
        b64.and_then(|b| base64::engine::general_purpose::STANDARD.decode(b.trim()).ok()).map(|b| Self::from_proto(&b)).unwrap_or_default()
    }

    pub fn apply(&mut self, raw: &[u8], partial: bool) {
        if let Some(v) = sub(raw, 5).and_then(|vv| sub(vv, 6)) {
            self.afk_timeout = fields(v).into_iter().rev().find(|f| f.0 == 1 && f.1 == 0).map(|f| f.3 as u32).or(Some(0));
        } else if !partial {
            self.afk_timeout = None;
        }
        let st = sub(raw, 11);
        if partial && st.is_none() {
            return;
        }
        let st = st.unwrap_or_default();
        let status = sub(st, 1).map(|v| text(v, 1));
        let cs = sub(st, 2).map(custom);
        if partial {
            if status.is_some() {
                self.status = status;
            }
            if cs.is_some() {
                self.custom = cs;
            }
        } else {
            self.status = status;
            self.custom = cs;
        }
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = Some(status.to_owned());
    }

    pub fn status(&self) -> &str {
        match self.status.as_deref() {
            None | Some("" | "unknown") => "online",
            Some(s) => s,
        }
    }

    pub fn payload(&self, now_ms: u64, animated: impl Fn(u64) -> bool) -> UpdatePresence {
        let status = match self.status() {
            "online" if self.idle_since > 0 => "idle",
            s => s,
        }
        .to_owned();
        let activities = match &self.custom {
            Some(c) if status != "invisible" && (c.proto.expires_at_ms == 0 || c.proto.expires_at_ms > now_ms) => {
                vec![activity(c, &animated)]
            }
            _ => Vec::new(),
        };
        UpdatePresence { since: self.idle_since, activities, status, afk: self.afk }
    }
}

fn activity(c: &Custom, animated: &dyn Fn(u64) -> bool) -> Value {
    let p = &c.proto;
    let mut a = json!({"name": "Custom Status", "type": 4, "emoji": null, "metadata": {}});
    if !p.text.is_empty() {
        a["state"] = json!(p.text);
    }
    if p.expires_at_ms > 0 {
        a["timestamps"] = json!({"end": p.expires_at_ms});
    }
    if p.emoji_id != 0 {
        let anim = c.animated.unwrap_or_else(|| animated(p.emoji_id));
        a["emoji"] = json!({"id": p.emoji_id.to_string(), "name": p.emoji_name, "animated": anim});
    } else if !p.emoji_name.is_empty() {
        a["emoji"] = json!({"id": null, "name": p.emoji_name, "animated": false});
    }
    if let Some(l) = &c.label {
        a["details"] = json!(l);
        a["metadata"] = json!({"label": l});
    }
    a
}

fn emoji_animated(cache: &Cache, emoji: u64) -> bool {
    cache.guilds().iter().flat_map(|g| g.emojis.iter()).find(|e| e.id.map(|i| i.0) == Some(emoji)).is_some_and(|e| e.animated)
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub(super) fn send(s: &super::Session) {
    let p = {
        let mut l = s.presence.lock();
        l.idle_since = IDLE_SINCE.load(Ordering::Relaxed);
        l.afk = AFK.load(Ordering::Relaxed);
        l.payload(now_ms(), |e| emoji_animated(&s.cache, e))
    };
    let _ = s.gateway.send(GatewayCommand::UpdatePresence(p));
}

pub(crate) fn afk_ms(afk_timeout: Option<u32>) -> Option<u32> {
    match afk_timeout.unwrap_or(AFK_DEFAULT_S) {
        0 => None,
        t => Some(t.saturating_mul(1000).min(IDLE_MS)),
    }
}

pub(crate) fn on_idle(which: usize, idle: bool, now: u64) -> bool {
    match which {
        0 => {
            let since = if idle { now.saturating_sub(u64::from(IDLE_MS)) } else { 0 };
            IDLE_SINCE.swap(since, Ordering::Relaxed) != since
        }
        _ => AFK.swap(idle, Ordering::Relaxed) != idle,
    }
}

pub(super) fn watch_idle(inner: &Arc<super::Inner>, afk_timeout: Option<u32>) {
    WATCH.call_once(|| {
        let mut timeouts = vec![IDLE_MS];
        match afk_ms(afk_timeout) {
            Some(t) => timeouts.push(t),
            None => AFK.store(true, Ordering::Relaxed),
        }
        let inner = inner.clone();
        crate::idle::spawn(
            timeouts,
            Box::new(move |which: usize, idle: bool| {
                if on_idle(which, idle, now_ms())
                    && let Some(s) = inner.session.lock().clone()
                {
                    send(&s);
                }
            }),
        );
    });
}

pub(super) fn on_settings_update(s: &super::Session, data: &Value) {
    let Some((raw, partial)) = settings_update(data) else { return };
    let changed = {
        let mut l = s.presence.lock();
        let before = l.clone();
        l.apply(&raw, partial);
        *l != before
    };
    if changed {
        send(s);
    }
}

fn settings_update(data: &Value) -> Option<(Vec<u8>, bool)> {
    use base64::Engine;
    let s = data.get("settings")?;
    if s.get("type").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let raw = base64::engine::general_purpose::STANDARD.decode(s.get("proto")?.as_str()?.trim()).ok()?;
    Some((raw, data.get("partial").and_then(Value::as_bool).unwrap_or(false)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_api::rest::settings::encode_custom_status;

    fn fb(tag: u8, data: &[u8], out: &mut Vec<u8>) {
        out.push((tag << 3) | 2);
        out.push(data.len() as u8);
        out.extend_from_slice(data);
    }

    fn settings(status: Option<&str>, cs: Option<&CustomStatusProto>) -> Vec<u8> {
        let mut st = Vec::new();
        if let Some(s) = status {
            let mut sv = Vec::new();
            fb(1, s.as_bytes(), &mut sv);
            fb(1, &sv, &mut st);
        }
        if let Some(c) = cs {
            fb(2, &encode_custom_status(c), &mut st);
        }
        let mut out = Vec::new();
        fb(11, &st, &mut out);
        out
    }

    #[test]
    fn ready_status_is_sent_as_is() {
        let l = Local::from_proto(&settings(Some("dnd"), None));
        let p = l.payload(0, |_| false);
        assert_eq!(serde_json::to_value(&p).unwrap(), json!({"since": 0, "activities": [], "status": "dnd", "afk": false}));
    }

    #[test]
    fn missing_or_unknown_status_means_online() {
        assert_eq!(Local::from_proto(&[]).status(), "online");
        assert_eq!(Local::from_proto(&settings(Some("unknown"), None)).status(), "online");
        assert_eq!(Local::from_base64(None).status(), "online");
    }

    #[test]
    fn custom_status_activity_matches_the_client() {
        let c = CustomStatusProto { text: "Lunch".into(), emoji_id: 42, emoji_name: "dance".into(), expires_at_ms: 2_000, created_at_ms: 1_000 };
        let l = Local::from_proto(&settings(Some("idle"), Some(&c)));
        let p = l.payload(1_500, |id| id == 42);
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({"since": 0, "status": "idle", "afk": false, "activities": [{
                "name": "Custom Status", "type": 4, "state": "Lunch", "timestamps": {"end": 2_000},
                "emoji": {"id": "42", "name": "dance", "animated": true}, "metadata": {}
            }]})
        );
        assert!(l.payload(2_000, |_| false).activities.is_empty());
    }

    #[test]
    fn unicode_emoji_only_and_invisible() {
        let c = CustomStatusProto { emoji_name: "\u{1f354}".into(), ..Default::default() };
        let mut l = Local::from_proto(&settings(Some("online"), Some(&c)));
        assert_eq!(
            l.payload(0, |_| false).activities,
            vec![json!({"name": "Custom Status", "type": 4, "emoji": {"id": null, "name": "\u{1f354}", "animated": false}, "metadata": {}})]
        );
        l.set_status("invisible");
        assert!(l.payload(0, |_| false).activities.is_empty());
    }

    #[test]
    fn partial_updates_merge_and_full_updates_replace() {
        let c = CustomStatusProto { text: "hi".into(), ..Default::default() };
        let mut l = Local::from_proto(&settings(Some("dnd"), Some(&c)));
        l.apply(&settings(Some("idle"), None), true);
        assert_eq!((l.status(), l.custom.as_ref().map(|c| c.proto.text.as_str())), ("idle", Some("hi")));
        l.apply(&[], true);
        assert_eq!(l.status(), "idle");
        l.apply(&settings(Some("online"), None), false);
        assert_eq!((l.status(), l.custom.is_none()), ("online", true));
    }

    #[test]
    fn idle_turns_online_into_idle_with_since_and_afk() {
        let mut l = Local::from_proto(&settings(Some("online"), None));
        l.idle_since = 1_000;
        l.afk = true;
        assert_eq!(serde_json::to_value(l.payload(0, |_| false)).unwrap(), json!({"since": 1_000, "activities": [], "status": "idle", "afk": true}));
        l.set_status("dnd");
        assert_eq!((l.payload(0, |_| false).status.as_str(), l.payload(0, |_| false).since), ("dnd", 1_000));
    }

    #[test]
    fn afk_timeout_follows_the_voice_setting() {
        assert_eq!(afk_ms(None), Some(60_000));
        assert_eq!(afk_ms(Some(300)), Some(300_000));
        assert_eq!(afk_ms(Some(3_600)), Some(600_000));
        assert_eq!(afk_ms(Some(0)), None);
        let mut raw = Vec::new();
        fb(5, &[0x32, 0x02, 0x08, 0x78], &mut raw);
        assert_eq!(Local::from_proto(&raw).afk_timeout, Some(120));
        assert_eq!(Local::from_proto(&[]).afk_timeout, None);
    }

    #[test]
    fn idle_events_only_report_changes() {
        assert!(on_idle(0, true, 700_000));
        assert_eq!(IDLE_SINCE.load(Ordering::Relaxed), 100_000);
        assert!(!on_idle(0, true, 700_000));
        assert!(on_idle(0, false, 800_000));
        assert_eq!(IDLE_SINCE.load(Ordering::Relaxed), 0);
        assert!(on_idle(1, true, 0));
        assert!(!on_idle(1, true, 0));
        assert!(on_idle(1, false, 0));
    }

    #[test]
    fn settings_update_reads_preloaded_proto_only() {
        use base64::Engine;
        let b = base64::engine::general_purpose::STANDARD.encode(settings(Some("dnd"), None));
        let d = json!({"settings": {"type": 1, "proto": b}, "partial": true});
        let (raw, partial) = settings_update(&d).unwrap();
        assert!(partial);
        assert_eq!(Local::from_proto(&raw).status(), "dnd");
        assert!(settings_update(&json!({"settings": {"type": 2, "proto": ""}})).is_none());
    }
}
