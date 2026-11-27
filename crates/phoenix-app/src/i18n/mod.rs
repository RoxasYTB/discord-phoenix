pub mod intl;
pub mod languages;
pub mod messages;
pub mod remote;

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::SharedString;

const LOCAL_EN: &str = include_str!("../../locales/en.yml");
const LOCAL_FR: &str = include_str!("../../locales/fr.yml");

static CURRENT: OnceLock<AtomicUsize> = OnceLock::new();

type Table = HashMap<&'static str, &'static str>;

fn local_tables() -> &'static (Table, Table) {
    static T: OnceLock<(Table, Table)> = OnceLock::new();
    T.get_or_init(|| (parse_yml(LOCAL_EN), parse_yml(LOCAL_FR)))
}

fn parse_yml(src: &'static str) -> Table {
    let mut m = HashMap::new();
    for line in src.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim();
        let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
        m.insert(k.trim(), v);
    }
    m
}

pub fn set_locale(code: &str) {
    let forced = env_locale();
    let target = forced.as_deref().unwrap_or(code);
    let idx = languages::index_of(target);
    current().store(idx, Ordering::Relaxed);
    let norm = languages::LANGUAGES[idx].code;
    if forced.is_some() {
        let _ = remote::load_blocking(norm);
    } else {
        remote::ensure_loaded(norm);
    }
}

fn env_locale() -> Option<String> {
    if cfg!(test) {
        return None;
    }
    std::env::var("PHOENIX_LOCALE").ok().filter(|s| !s.is_empty())
}

fn current() -> &'static AtomicUsize {
    CURRENT.get_or_init(|| {
        let forced = env_locale();
        let code = match forced.as_deref() {
            Some(code) => languages::normalize(code),
            None if cfg!(test) => "en-US",
            None => sys_locale::get_locales()
                .find_map(|code| languages::supported_code(&code))
                .unwrap_or("en-US"),
        };
        if forced.is_some() {
            let _ = remote::load_blocking(code);
        } else if code != "fr" {
            remote::ensure_loaded(code);
        }
        AtomicUsize::new(languages::index_of(code))
    })
}

pub fn locale() -> &'static str {
    languages::LANGUAGES[current().load(Ordering::Relaxed)].code
}

pub fn is_rtl() -> bool {
    intl::data(locale()).rtl
}

pub fn number(n: i64) -> String {
    intl::number(locale(), n)
}

pub fn decimal(n: f64, frac: usize) -> String {
    intl::decimal(locale(), n, frac)
}

fn local_lookup(code: &str, key: &str) -> Option<&'static str> {
    let (en, fr) = local_tables();
    if code == "fr" {
        fr.get(key).or_else(|| en.get(key)).copied()
    } else {
        en.get(key).copied()
    }
}

fn fill(s: &str, args: &[(&str, &str)]) -> String {
    let mut out = s.replace("\\n", "\n").replace("\\\"", "\"");
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

pub fn t(key: &str, args: &[(&str, &str)]) -> SharedString {
    t_in(locale(), key, args)
}

pub fn t_in(code: &str, key: &str, args: &[(&str, &str)]) -> SharedString {
    if code != "en-US"
        && let Some(mapping) = messages::mappings().get(key)
        && let Some(table) = remote::get(code)
        && let Some(ast) = table.get(mapping.hash)
    {
        return messages::render(code, ast, args, mapping).into();
    }
    match local_lookup(code, key) {
        Some(s) if args.is_empty() && !s.contains('\\') => SharedString::new_static(s),
        Some(s) => fill(s, args).into(),
        None => SharedString::from(key.to_owned()),
    }
}

#[macro_export]
macro_rules! t {
    ($key:expr) => { $crate::i18n::t($key, &[]) };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::i18n::t($key, &[$((stringify!($name), &$val.to_string())),+])
    };
}

pub fn calendar(dt: chrono::DateTime<chrono::Local>, now: chrono::DateTime<chrono::Local>) -> SharedString {
    let time = clock(dt);
    let days = (now.date_naive() - dt.date_naive()).num_days();
    match days {
        0 => time.into(),
        1 => t!("time.yesterday_at", time = time),
        _ => intl::format(locale(), "L LT", &intl::Parts::of(&dt), crate::ui::settings::effects::hour24()).into(),
    }
}

pub fn clock<Tz: chrono::TimeZone>(dt: chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    intl::format(locale(), "LT", &intl::Parts::of(&dt), crate::ui::settings::effects::hour24())
}

pub fn short_date<Tz: chrono::TimeZone>(dt: chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    intl::format(locale(), "L", &intl::Parts::of(&dt), None)
}

pub fn long_date(date: chrono::NaiveDate) -> String {
    let dt = date.and_hms_opt(0, 0, 0).unwrap_or_default();
    intl::format(locale(), "LL", &intl::Parts::of(&dt), None)
}

pub fn medium_date(date: chrono::NaiveDate) -> String {
    let dt = date.and_hms_opt(0, 0, 0).unwrap_or_default();
    intl::format(locale(), "ll", &intl::Parts::of(&dt), None)
}

pub fn full_datetime(dt: chrono::DateTime<chrono::Local>) -> String {
    intl::format(locale(), "LLLL", &intl::Parts::of(&dt), crate::ui::settings::effects::hour24())
}

pub fn relative_long(delta: i64) -> String {
    let a = delta.unsigned_abs();
    let (n, unit) = match a {
        0..60 => (a, "second"),
        60..3600 => (a / 60, "minute"),
        3600..86400 => (a / 3600, "hour"),
        86400..2_592_000 => (a / 86400, "day"),
        2_592_000..31_536_000 => (a / 2_592_000, "month"),
        _ => (a / 31_536_000, "year"),
    };
    let what = if n == 1 && unit != "second" {
        t(&format!("unit.{unit}_a"), &[]).to_string()
    } else {
        let u = t(&format!("unit.{unit}_{}", if n == 1 { "one" } else { "many" }), &[]);
        format!("{} {u}", number(n as i64))
    };
    t(if delta >= 0 { "time.in" } else { "time.ago" }, &[("what", &what)]).to_string()
}

pub fn ago_short(secs: i64) -> SharedString {
    let s = secs.max(0);
    match s {
        0..60 => t!("time.now"),
        60..3600 => t!("time.minutes_ago_short", n = s / 60),
        3600..86_400 => t!("time.hours_ago_short", n = s / 3600),
        86_400..2_592_000 => t!("time.days_ago_short", n = s / 86_400),
        _ => t!("time.months_ago_short", n = s / 2_592_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_english_key_exists_in_french() {
        let (en, fr) = local_tables();
        let missing: Vec<_> = en.keys().filter(|k| !fr.contains_key(*k)).collect();
        assert!(missing.is_empty(), "fr.yml is missing keys: {missing:?}");
        let extra: Vec<_> = fr.keys().filter(|k| !en.contains_key(*k)).collect();
        assert!(extra.is_empty(), "fr.yml has keys not in en.yml: {extra:?}");
    }

    #[test]
    fn placeholders_match_between_locales() {
        let holes = |s: &str| {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|p| p.split_once('}').map(|x| x.0.to_owned()))
                .collect();
            v.sort();
            v
        };
        let (en, fr) = local_tables();
        for (k, ev) in en {
            if let Some(fv) = fr.get(k) {
                assert_eq!(holes(ev), holes(fv), "placeholder mismatch for {k} in fr");
            }
        }
    }

    #[test]
    fn paragraphs_and_line_breaks_in_local_tables() {
        let fr = t_in("fr", "gs.access.hd_retention_desc", &[]);
        let (first, second) = fr.split_once('\n').expect("two paragraphs");
        assert!(first.ends_with("semaine."));
        assert!(second.starts_with("Observe ton serveur"));
        let en = t_in("en-US", "gs.access.hd_retention_desc", &[]);
        assert_eq!(en.lines().count(), 2);
        assert!(!t_in("fr", "gs.eng.afk_desc", &[]).contains('\n'));
        assert_eq!(t_in("en-US", "cs.change_avatar", &[]).as_ref(), "Change\nAvatar");
        let glued = regex::Regex::new(r"[a-z][.?!][A-Z]").expect("regex");
        let (en, fr) = local_tables();
        for (k, v) in en.iter().chain(fr.iter()) {
            assert!(!glued.is_match(v) || v.contains("://") || v.contains('`'), "glued sentences in {k}: {v}");
        }
    }

    #[test]
    fn lookup_and_fallback() {
        assert_eq!(t_in("fr", "status.online", &[]).as_ref(), "En ligne");
        assert_eq!(t_in("fr", "chat.typing_one", &[("a", "Bob")]).as_ref(), "Bob est en train d'écrire…");
        assert_eq!(t_in("en-US", "no.such.key", &[]).as_ref(), "no.such.key");
        assert_eq!(t_in("de", "status.online", &[]).as_ref(), "Online");
    }
}
