use chrono::{Datelike, Days, Months, NaiveDate, TimeZone};
use gpui::SharedString;
use phoenix_api::rest::search::{FilterKey, HasFilter, SearchKeys};

use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
};

pub const HAS_VALUES: [(HasFilter, &str, &str); 9] = [
    (HasFilter::Image, "search.kv.image", "image"),
    (HasFilter::Video, "search.kv.video", "voice/video-small"),
    (HasFilter::Link, "search.kv.link", "link"),
    (HasFilter::File, "search.kv.file", "file"),
    (HasFilter::Embed, "search.kv.embed", "angle-brackets"),
    (HasFilter::Sound, "search.kv.sound", "voice/soundboard"),
    (HasFilter::Poll, "search.kv.poll", "attach/poll"),
    (HasFilter::Sticker, "search.kv.sticker", "sticker"),
    (HasFilter::Snapshot, "search.kv.forward", "chat/forwarded"),
];

pub const AUTHOR_VALUES: [(&str, &str, &str); 3] =
    [("user", "search.kv.user", "search/user"), ("bot", "search.kv.bot", "mod/robot"), ("webhook", "search.kv.webhook", "settings/webhook")];

const KEY_WORDS: [(FilterKey, &str); 10] = [
    (FilterKey::From, "search.kw.from"),
    (FilterKey::Mentions, "search.kw.mentions"),
    (FilterKey::Has, "search.kw.has"),
    (FilterKey::Before, "search.kw.before"),
    (FilterKey::After, "search.kw.after"),
    (FilterKey::On, "search.kw.during"),
    (FilterKey::On, "search.kw.on"),
    (FilterKey::In, "search.kw.in"),
    (FilterKey::Pinned, "search.kw.pinned"),
    (FilterKey::AuthorType, "search.kw.author"),
];

fn w(key: &str) -> String {
    crate::i18n::t(key, &[]).to_string()
}

pub fn keys() -> SearchKeys {
    SearchKeys::localized(
        KEY_WORDS.iter().map(|(k, t)| (*k, w(t))).collect(),
        HAS_VALUES.iter().map(|(h, t, _)| (w(t), *h)).collect(),
        AUTHOR_VALUES.iter().map(|(v, t, _)| (w(t), *v)).collect(),
    )
}

pub fn token(key: FilterKey) -> String {
    format!("{}:", keys().label(key))
}

pub fn localize_token(english: &str) -> String {
    keys().key(english.trim_end_matches(':')).map_or_else(|| english.to_owned(), token)
}

#[derive(Clone, Debug, PartialEq)]
pub struct DateWords {
    pub today: String,
    pub yesterday: String,
    pub week: String,
    pub month: String,
    pub year: String,
    pub months: Vec<String>,
    pub weekdays: Vec<String>,
    pub sunday_first: bool,
}

impl DateWords {
    pub fn current() -> Self {
        let list = |k: &str| w(k).split(',').map(|s| s.trim().to_lowercase()).collect();
        Self {
            today: w("search.kv.today").to_lowercase(),
            yesterday: w("search.kv.yesterday").to_lowercase(),
            week: w("search.kv.week").to_lowercase(),
            month: w("search.kv.month").to_lowercase(),
            year: w("search.kv.year").to_lowercase(),
            months: list("search.months"),
            weekdays: list("search.weekdays"),
            sunday_first: crate::i18n::locale() == "en",
        }
    }
}

fn day(d: NaiveDate) -> (NaiveDate, NaiveDate) {
    (d, d + Days::new(1))
}

fn week_start(today: NaiveDate, sunday_first: bool) -> NaiveDate {
    let back = if sunday_first { today.weekday().num_days_from_sunday() } else { today.weekday().num_days_from_monday() };
    today - Days::new(back.into())
}

pub fn date_range(value: &str, today: NaiveDate, words: &DateWords) -> Option<(NaiveDate, NaiveDate)> {
    let v = value.trim().to_lowercase();
    let first_of_month = |y: i32, m: u32| NaiveDate::from_ymd_opt(y, m, 1);
    if v == words.today {
        return Some(day(today));
    }
    if v == words.yesterday {
        return Some(day(today - Days::new(1)));
    }
    if v == words.week {
        let s = week_start(today, words.sunday_first);
        return Some((s, s + Days::new(7)));
    }
    if v == words.month {
        let s = first_of_month(today.year(), today.month())?;
        return Some((s, s + Months::new(1)));
    }
    if v == words.year {
        let s = NaiveDate::from_ymd_opt(today.year(), 1, 1)?;
        return Some((s, s + Months::new(12)));
    }
    if let Some(m) = words.months.iter().position(|x| *x == v) {
        let s = first_of_month(today.year(), m as u32 + 1)?;
        return Some((s, s + Months::new(1)));
    }
    if let Some(wd) = words.weekdays.iter().position(|x| *x == v) {
        let start = week_start(today, words.sunday_first);
        let offset = if words.sunday_first { wd } else { (wd + 6) % 7 };
        return Some(day(start + Days::new(offset as u64)));
    }
    let parts: Vec<&str> = v.split('-').collect();
    let num = |s: &str, max: usize| (!s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u32>().ok()).flatten();
    match parts.as_slice() {
        [y] if y.len() == 4 => {
            let y = num(y, 4)? as i32;
            if (2015..=today.year()).contains(&y) {
                let s = NaiveDate::from_ymd_opt(y, 1, 1)?;
                return Some((s, s + Months::new(12)));
            }
            NaiveDate::from_ymd_opt(y, 1, 1).map(day)
        }
        [y, m] if y.len() == 4 => NaiveDate::from_ymd_opt(num(y, 4)? as i32, num(m, 2)?, 1).map(day),
        [y, m, d] if y.len() == 4 => NaiveDate::from_ymd_opt(num(y, 4)? as i32, num(m, 2)?, num(d, 2)?).map(day),
        _ => None,
    }
}

const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

pub fn local_snowflake(d: NaiveDate) -> u64 {
    let ms = d
        .and_hms_opt(0, 0, 0)
        .and_then(|t| chrono::Local.from_local_datetime(&t).earliest())
        .map_or(DISCORD_EPOCH_MS, |t| t.timestamp_millis());
    ((ms - DISCORD_EPOCH_MS).max(0) as u64) << 22
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnswerLead {
    User(Id),
    Channel(ChannelKind),
    Dm(Id),
    Glyph(&'static str),
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub value: String,
    pub label: SharedString,
    pub sub: Option<SharedString>,
    pub lead: AnswerLead,
}

pub fn fuzzy(needle: &str, hay: &str) -> bool {
    let mut it = hay.chars().flat_map(char::to_lowercase);
    needle.chars().flat_map(char::to_lowercase).all(|c| it.any(|h| h == c))
}

pub fn group_title(key: FilterKey) -> SharedString {
    match key {
        FilterKey::From => crate::t!("search.g_from"),
        FilterKey::Mentions => crate::t!("search.g_mentions"),
        FilterKey::In => crate::t!("search.g_in"),
        FilterKey::Has => crate::t!("search.g_has"),
        _ => SharedString::default(),
    }
}

pub const MAX_ANSWERS: usize = 10;

pub fn answers(app: &PhoenixApp, key: FilterKey, partial: &str) -> Vec<Answer> {
    let q = partial.trim().trim_start_matches(['@', '#']).to_lowercase();
    match key {
        FilterKey::From | FilterKey::Mentions => crate::ui::search_filters::user_options(app, &q)
            .into_iter()
            .take(MAX_ANSWERS)
            .map(|(id, value, label)| Answer { sub: Some(value.clone().into()), value, label, lead: AnswerLead::User(id) })
            .collect(),
        FilterKey::In if app.selected_guild.is_none() => app
            .dms
            .iter()
            .filter_map(|d| {
                let tag = crate::ui::search::dm_tag(app, *d);
                (!tag.is_empty() && fuzzy(&q, &tag)).then(|| Answer { value: tag.to_string(), label: tag, sub: None, lead: AnswerLead::Dm(*d) })
            })
            .take(MAX_ANSWERS)
            .collect(),
        FilterKey::In => crate::ui::search_filters::channel_options(app, &q)
            .into_iter()
            .take(MAX_ANSWERS)
            .map(|(kind, name)| Answer { value: name.to_string(), label: name, sub: None, lead: AnswerLead::Channel(kind) })
            .collect(),
        FilterKey::Has => HAS_VALUES
            .iter()
            .filter_map(|(_, t, glyph)| {
                let l = w(t);
                fuzzy(&q, &l).then(|| Answer { value: l.clone(), label: l.into(), sub: None, lead: AnswerLead::Glyph(glyph) })
            })
            .collect(),
        FilterKey::AuthorType => AUTHOR_VALUES
            .iter()
            .filter_map(|(_, t, glyph)| {
                let l = w(t);
                fuzzy(&q, &l).then(|| Answer { value: l.clone(), label: l.into(), sub: None, lead: AnswerLead::Glyph(glyph) })
            })
            .collect(),
        FilterKey::Pinned => ["true", "false"]
            .into_iter()
            .map(|v| Answer { value: v.into(), label: v.into(), sub: None, lead: AnswerLead::None })
            .collect(),
        FilterKey::Before | FilterKey::After | FilterKey::On => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en() -> DateWords {
        DateWords {
            today: "today".into(),
            yesterday: "yesterday".into(),
            week: "week".into(),
            month: "month".into(),
            year: "year".into(),
            months: "january,february,march,april,may,june,july,august,september,october,november,december".split(',').map(Into::into).collect(),
            weekdays: "sunday,monday,tuesday,wednesday,thursday,friday,saturday".split(',').map(Into::into).collect(),
            sunday_first: true,
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn dates_follow_the_client_forms() {
        let today = d(2026, 11, 26);
        let w = en();
        assert_eq!(date_range("2026-01-02", today, &w), Some((d(2026, 1, 2), d(2026, 1, 3))));
        assert_eq!(date_range("2026-1-2", today, &w), Some((d(2026, 1, 2), d(2026, 1, 3))));
        assert_eq!(date_range("2026-05", today, &w), Some((d(2026, 5, 1), d(2026, 5, 2))));
        assert_eq!(date_range("2024", today, &w), Some((d(2024, 1, 1), d(2025, 1, 1))));
        assert_eq!(date_range("2014", today, &w), Some((d(2014, 1, 1), d(2014, 1, 2))));
        assert_eq!(date_range("Today", today, &w), Some((today, d(2026, 11, 27))));
        assert_eq!(date_range("yesterday", today, &w), Some((d(2026, 11, 25), today)));
        assert_eq!(date_range("week", today, &w), Some((d(2026, 11, 22), d(2026, 11, 29))));
        assert_eq!(date_range("month", today, &w), Some((d(2026, 11, 1), d(2026, 12, 1))));
        assert_eq!(date_range("year", today, &w), Some((d(2026, 1, 1), d(2027, 1, 1))));
        assert_eq!(date_range("march", today, &w), Some((d(2026, 3, 1), d(2026, 4, 1))));
        assert_eq!(date_range("monday", today, &w), Some((d(2026, 11, 23), d(2026, 11, 24))));
        assert_eq!(date_range("2026-13-01", today, &w), None);
        assert_eq!(date_range("soon", today, &w), None);
        assert_eq!(date_range("12345", today, &w), None);
    }

    #[test]
    fn french_weeks_start_on_monday() {
        let w = DateWords { sunday_first: false, weekdays: "dimanche,lundi,mardi,mercredi,jeudi,vendredi,samedi".split(',').map(Into::into).collect(), ..en() };
        let today = d(2026, 11, 29);
        assert_eq!(date_range("week", today, &w), Some((d(2026, 11, 23), d(2026, 11, 30))));
        assert_eq!(date_range("lundi", today, &w), Some((d(2026, 11, 23), d(2026, 11, 24))));
        assert_eq!(date_range("dimanche", today, &w), Some((d(2026, 11, 29), d(2026, 11, 30))));
    }

    #[test]
    fn snowflakes_start_at_local_midnight() {
        assert_eq!(local_snowflake(d(2010, 1, 1)), 0);
        let a = local_snowflake(d(2026, 1, 1));
        let b = local_snowflake(d(2026, 1, 2));
        assert_eq!((b - a) >> 22, 86_400_000);
    }

    #[test]
    fn fuzzy_matches_in_order() {
        assert!(fuzzy("", "lien"));
        assert!(fuzzy("vd", "vidéo"));
        assert!(fuzzy("VID", "vidéo"));
        assert!(!fuzzy("dv", "vidéo"));
    }

    #[test]
    fn localized_tokens() {
        let k = keys();
        assert_eq!(k.key(&w("search.kw.from")), Some(FilterKey::From));
        assert_eq!(k.key("from"), Some(FilterKey::From));
        assert_eq!(token(FilterKey::On), format!("{}:", w("search.kw.during")));
        assert_eq!(k.has_value(&w("search.kv.forward")), Some(HasFilter::Snapshot));
        assert_eq!(localize_token("from:"), token(FilterKey::From));
        assert_eq!(localize_token("authorType:"), token(FilterKey::AuthorType));
        assert_eq!(localize_token("nope:"), "nope:");
    }
}
