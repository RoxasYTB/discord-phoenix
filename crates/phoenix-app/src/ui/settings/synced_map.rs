use phoenix_api::rest::settings::{ProtoValue, proto_read};

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool { invert: bool },
    Enum(&'static [u64]),
    Str(&'static [&'static str]),
    Bits(u64),
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub id: &'static str,
    pub path: &'static [u32],
    pub wrapper: bool,
    pub kind: Kind,
    pub default: u64,
}

const fn b(id: &'static str, path: &'static [u32], default: bool) -> Entry {
    Entry { id, path, wrapper: true, kind: Kind::Bool { invert: false }, default: default as u64 }
}
const fn inv(id: &'static str, path: &'static [u32], default: bool) -> Entry {
    Entry { id, path, wrapper: true, kind: Kind::Bool { invert: true }, default: default as u64 }
}
const fn en(id: &'static str, path: &'static [u32], wrapper: bool, opts: &'static [u64], default: u64) -> Entry {
    Entry { id, path, wrapper, kind: Kind::Enum(opts), default }
}

pub const AFK_TIMEOUTS: &[u64] = &[60, 120, 180, 240, 300, 360, 420, 480, 540, 600];

pub static ENTRIES: &[Entry] = &[
    en("push-delay", &[5, 6], true, AFK_TIMEOUTS, 0),
    b("n-stream", &[5, 7], true),
    b("md-preview", &[6, 2], true),
    b("split-threads", &[6, 3], true),
    Entry { id: "spoilers", path: &[6, 4], wrapper: true, kind: Kind::Str(&["ON_CLICK", "ALWAYS", "IF_MODERATOR"]), default: 0 },
    b("alt-text", &[6, 7], false),
    b("slash-suggest", &[6, 8], true),
    b("media-uploads", &[6, 9], true),
    b("media-links", &[6, 10], true),
    b("gifs", &[6, 11], true),
    b("embeds", &[6, 12], true),
    b("reactions", &[6, 13], true),
    en("stickers", &[6, 15], true, &[0, 1, 2], 0),
    b("tts", &[6, 16], true),
    b("age-ios", &[6, 20], false),
    b("anim-emoji", &[6, 14], true),
    b("emoticons", &[6, 21], true),
    b("age-cmds", &[6, 23], false),
    b("legacy-input", &[6, 24], false),
    en("spam", &[6, 27], false, &[3, 2, 1], 1),
    b("ac-stickers", &[6, 28], true),
    en("cf-servers", &[6, 29, 1], false, &[1, 2, 3], 0),
    en("cf-friends", &[6, 29, 2], false, &[1, 2, 3], 0),
    en("cf-others", &[6, 29, 3], false, &[1, 2, 3], 2),
    en("cfv-servers", &[6, 32, 1], false, &[1, 2, 3], 0),
    en("cfv-friends", &[6, 32, 2], false, &[1, 2, 3], 0),
    en("cfv-others", &[6, 32, 3], false, &[1, 2, 3], 1),
    en("dm-search", &[6, 36], true, &[0, 1], 0),
    b("ac-games", &[6, 39], true),
    b("avatars", &[6, 41], true),
    b("act-stream", &[7, 26], true),
    en("n-react", &[7, 7], false, &[0, 1, 2], 0),
    b("n-online", &[7, 12], true),
    b("n-anniv", &[7, 14], true),
    b("n-profile", &[7, 16], true),
    b("n-event", &[7, 22], true),
    b("quiet-mode", &[7, 5], false),
    b("share-update", &[7, 24], true),
    b("act-online", &[7, 25], true),
    b("act-friends", &[8, 1], true),
    b("act-voice", &[8, 2], true),
    Entry { id: "fr-everyone", path: &[8, 11], wrapper: true, kind: Kind::Bits(8), default: 1 },
    Entry { id: "fr-fof", path: &[8, 11], wrapper: true, kind: Kind::Bits(2), default: 1 },
    Entry { id: "fr-members", path: &[8, 11], wrapper: true, kind: Kind::Bits(4), default: 1 },
    inv("msg-requests", &[8, 17], true),
    inv("data-sponsored", &[8, 18], true),
    b("fc-tab", &[8, 21], false),
    b("game-dms", &[8, 26], true),
    en("ingame", &[8, 28], false, &[1, 2, 3], 0),
    en("act-where", &[8, 29], false, &[3, 2, 1], 0),
    inv("data-third", &[8, 30], true),
    en("share-profile", &[8, 32], false, &[3, 2, 1], 0),
    inv("fr-msg", &[8, 33], true),
    inv("data-discovery", &[8, 35], false),
    b("act-share", &[11, 3], true),
    en("time-format", &[13, 9], false, &[0, 1, 2], 0),
    b("favs", &[15, 3], false),
    b("data-clips", &[19, 1], true),
    en("sb-volume", &[5, 9, 1], false, &[], 0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rest {
    Consent(&'static str),
    Email(&'static str),
}

pub static REST: &[(&str, Rest, bool)] = &[
    ("data-improve", Rest::Consent("usage_statistics"), true),
    ("data-personalize", Rest::Consent("personalization"), true),
    ("e-comm", Rest::Email("communication"), true),
    ("e-social", Rest::Email("social"), true),
    ("e-news", Rest::Email("updates_and_announcements"), false),
    ("e-tips", Rest::Email("tips"), false),
    ("e-recs", Rest::Email("recommendations_and_events"), false),
    ("e-family", Rest::Email("family_center_digest"), false),
];

pub fn rest(id: &str) -> Option<(Rest, bool)> {
    REST.iter().find(|r| r.0 == id).map(|r| (r.1, r.2))
}

pub fn read_rest(consent: Option<&serde_json::Value>, email: Option<&serde_json::Value>) -> std::collections::HashMap<&'static str, bool> {
    let cats = email.and_then(|e| e.get("categories").or_else(|| e.get("settings").and_then(|s| s.get("categories"))));
    REST.iter()
        .filter_map(|&(id, r, _)| {
            let v = match r {
                Rest::Consent(k) => consent?.get(k)?.get("consented")?.as_bool()?,
                Rest::Email(k) => cats?.get(k)?.as_bool()?,
            };
            Some((id, v))
        })
        .collect()
}

pub fn entry(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.id == id)
}

pub fn read_all(raw: &[u8]) -> std::collections::HashMap<&'static str, u64> {
    let mut out = std::collections::HashMap::new();
    for e in ENTRIES {
        let as_str = matches!(e.kind, Kind::Str(_));
        let v = match proto_read(raw, e.path, e.wrapper, as_str) {
            Some(ProtoValue::Int(v)) => v,
            Some(ProtoValue::Str(s)) => match e.kind {
                Kind::Str(opts) => match opts.iter().position(|o| *o == s) {
                    Some(i) => i as u64,
                    None => continue,
                },
                _ => continue,
            },
            None => continue,
        };
        out.insert(e.id, v);
    }
    out
}

fn bits_of(map: &std::collections::HashMap<&'static str, u64>, id: &str) -> u64 {
    map.get(id).copied().unwrap_or(2 | 4 | 8)
}

pub fn get_bool(map: &std::collections::HashMap<&'static str, u64>, id: &str) -> bool {
    let Some(e) = entry(id) else { return false };
    match (e.kind, map.get(id)) {
        (Kind::Bits(m), _) => bits_of(map, id) & m != 0,
        (Kind::Bool { invert }, Some(v)) => (*v != 0) != invert,
        _ if id == "data-discovery" => !ready_flags().discovery_default_opted_out,
        _ => e.default != 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadyFlags {
    pub ad_toggles_disabled: bool,
    pub discovery_locked: bool,
    pub discovery_default_opted_out: bool,
}

impl ReadyFlags {
    pub const DEFAULT: Self = Self { ad_toggles_disabled: false, discovery_locked: true, discovery_default_opted_out: true };

    pub fn from_ready(ad: Option<bool>, locked: Option<bool>, opted_out: Option<bool>) -> Self {
        let d = Self::DEFAULT;
        Self {
            ad_toggles_disabled: ad.unwrap_or(d.ad_toggles_disabled),
            discovery_locked: locked.unwrap_or(d.discovery_locked),
            discovery_default_opted_out: opted_out.unwrap_or(d.discovery_default_opted_out),
        }
    }
}

static READY_FLAGS: parking_lot::RwLock<ReadyFlags> = parking_lot::RwLock::new(ReadyFlags::DEFAULT);

pub fn set_ready_flags(f: ReadyFlags) {
    *READY_FLAGS.write() = f;
}

pub fn ready_flags() -> ReadyFlags {
    *READY_FLAGS.read()
}

pub fn disabled(map: &std::collections::HashMap<&'static str, u64>, compact: bool, id: &str) -> bool {
    let f = ready_flags();
    match id {
        "data-third" => f.ad_toggles_disabled || !get_bool(map, "data-sponsored"),
        "data-sponsored" => f.ad_toggles_disabled,
        "data-discovery" => f.discovery_locked,
        "avatars" => !compact,
        _ => false,
    }
}

pub fn get_pick(map: &std::collections::HashMap<&'static str, u64>, id: &str) -> usize {
    let Some(e) = entry(id) else { return 0 };
    let v = map.get(id).copied();
    match e.kind {
        Kind::Enum(opts) => v.and_then(|v| opts.iter().position(|o| *o == v)).unwrap_or(e.default as usize),
        Kind::Str(_) => v.map(|v| v as usize).unwrap_or(e.default as usize),
        _ => v.unwrap_or(e.default) as usize,
    }
}

pub fn bool_wire(map: &std::collections::HashMap<&'static str, u64>, id: &str, on: bool) -> u64 {
    match entry(id).map(|e| e.kind) {
        Some(Kind::Bits(m)) => {
            let cur = bits_of(map, id);
            match (m, on) {
                (8, true) => 2 | 4 | 8,
                (_, true) => cur | m,
                (_, false) => cur & !m,
            }
        }
        Some(Kind::Bool { invert }) => (on != invert) as u64,
        _ => on as u64,
    }
}

pub fn pick_wire(id: &str, i: usize) -> u64 {
    match entry(id).map(|e| e.kind) {
        Some(Kind::Enum(opts)) => opts.get(i).copied().unwrap_or(0),
        _ => i as u64,
    }
}

pub fn change(id: &str, wire: u64) -> Option<phoenix_api::rest::settings::SettingChange> {
    let e = entry(id)?;
    let value = match e.kind {
        Kind::Str(opts) => ProtoValue::Str(opts.get(wire as usize)?.to_string()),
        _ => ProtoValue::Int(wire),
    };
    Some(phoenix_api::rest::settings::SettingChange::Path { path: e.path.to_vec(), wrapper: e.wrapper, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_through_wire() {
        let mut raw = Vec::new();
        for (id, w) in [("embeds", 0), ("spoilers", 1), ("time-format", 2), ("msg-requests", 1), ("cf-others", 1)] {
            raw.extend(change(id, w).unwrap().encode());
        }
        let m = read_all(&raw);
        assert!(!get_bool(&m, "embeds"));
        assert!(get_bool(&m, "reactions"), "unset keeps the default");
        assert_eq!(get_pick(&m, "spoilers"), 1);
        assert_eq!(get_pick(&m, "time-format"), 2);
        assert!(!get_bool(&m, "msg-requests"), "inverted field");
        assert_eq!(get_pick(&m, "cf-others"), 0);
    }

    #[test]
    fn friend_source_bits() {
        let mut m = std::collections::HashMap::new();
        assert!(get_bool(&m, "fr-fof"));
        m.insert("fr-fof", bool_wire(&m, "fr-fof", false));
        assert_eq!(m["fr-fof"], 4 | 8);
        assert!(!get_bool(&m, "fr-fof"));
    }

    #[test]
    fn rest_responses() {
        let c = serde_json::json!({"usage_statistics": {"consented": false}, "personalization": {"consented": true}});
        let e = serde_json::json!({"initialized": true, "categories": {"social": false, "tips": true}});
        let m = read_rest(Some(&c), Some(&e));
        assert_eq!(m.get("data-improve"), Some(&false));
        assert_eq!(m.get("e-social"), Some(&false));
        assert_eq!(m.get("e-tips"), Some(&true));
        assert!(!m.contains_key("e-comm"));
    }

    #[test]
    fn privacy_wire_fields() {
        let c = change("data-sponsored", bool_wire(&Default::default(), "data-sponsored", false)).unwrap();
        let raw = c.encode();
        assert_eq!(raw, vec![0x42, 5, 0x92, 1, 2, 8, 1], "privacy(8){{drops_opted_out(18){{value: true}}}}");
        let m = read_all(&raw);
        assert!(!get_bool(&m, "data-sponsored"));
        assert!(disabled(&m, false, "data-third"), "3p switch greys out once activity personalisation is off");
        let raw = change("share-profile", pick_wire("share-profile", 2)).unwrap().encode();
        assert_eq!(raw, vec![0x42, 3, 0x80, 2, 1]);
        assert_eq!(get_pick(&read_all(&raw), "share-profile"), 2);
        assert_eq!(get_pick(&Default::default(), "share-profile"), 0, "UNSET = friends and all servers");
        let raw = change("push-delay", pick_wire("push-delay", 4)).unwrap().encode();
        assert_eq!(get_pick(&read_all(&raw), "push-delay"), 4);
        assert_eq!(get_pick(&Default::default(), "push-delay"), 0, "unset = 1 minute");
    }

    #[test]
    fn discovery_defaults_follow_ready() {
        let m = Default::default();
        set_ready_flags(ReadyFlags::DEFAULT);
        assert!(!get_bool(&m, "data-discovery"));
        assert!(disabled(&m, true, "data-discovery"));
        set_ready_flags(ReadyFlags::from_ready(None, Some(false), Some(false)));
        assert!(get_bool(&m, "data-discovery"));
        assert!(!disabled(&m, true, "data-discovery"));
        set_ready_flags(ReadyFlags::DEFAULT);
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = ENTRIES.iter().map(|e| e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ENTRIES.len());
    }
}
