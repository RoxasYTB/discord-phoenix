use std::collections::HashMap;
use std::sync::OnceLock;

const DATA: &str = include_str!("../../locales/intl.txt");

#[derive(Default)]
pub struct LocaleData {
    pub hc12: bool,
    pub first_day: u8,
    pub rtl: bool,
    group: &'static str,
    decimal: &'static str,
    indian: bool,
    min_group: usize,
    plural: &'static [u8],
    fmts: HashMap<&'static str, &'static str>,
    months: HashMap<&'static str, Vec<&'static str>>,
    weekdays: Vec<&'static str>,
    periods_auto: Vec<&'static str>,
    periods_h12: Vec<&'static str>,
}

fn all() -> &'static HashMap<&'static str, LocaleData> {
    static T: OnceLock<HashMap<&'static str, LocaleData>> = OnceLock::new();
    T.get_or_init(|| parse(DATA))
}

fn parse(src: &'static str) -> HashMap<&'static str, LocaleData> {
    let mut out = HashMap::new();
    let mut cur: Option<(&'static str, LocaleData)> = None;
    for line in src.lines() {
        if let Some(code) = line.strip_prefix('@') {
            if let Some((c, d)) = cur.take() {
                out.insert(c, d);
            }
            cur = Some((code.trim(), LocaleData { first_day: 1, min_group: 1, group: ",", decimal: ".", ..Default::default() }));
            continue;
        }
        let Some((_, d)) = cur.as_mut() else { continue };
        let Some((k, v)) = line.split_once('\t') else { continue };
        match k {
            "hc" => d.hc12 = v == "h12",
            "first" => d.first_day = v.parse().unwrap_or(1),
            "dir" => d.rtl = v == "rtl",
            "num" => {
                let p: Vec<&'static str> = v.split('|').collect();
                if p.len() == 4 {
                    d.group = p[0];
                    d.decimal = p[1];
                    d.indian = p[2] == "3,2";
                    d.min_group = p[3].parse().unwrap_or(1);
                }
            }
            "plural" => d.plural = v.as_bytes(),
            "weekdays" => d.weekdays = v.split('|').collect(),
            "periods.auto" => d.periods_auto = v.split('|').collect(),
            "periods.h12" => d.periods_h12 = v.split('|').collect(),
            _ => {
                if let Some(style) = k.strip_suffix(".months") {
                    d.months.insert(style, v.split('|').collect());
                } else {
                    d.fmts.insert(k, v);
                }
            }
        }
    }
    if let Some((c, d)) = cur.take() {
        out.insert(c, d);
    }
    out
}

pub fn data(code: &str) -> &'static LocaleData {
    let t = all();
    t.get(code).or_else(|| t.get("en-US")).expect("intl.txt has en-US")
}

pub fn plural(code: &str, n: f64) -> &'static str {
    let d = data(code);
    if n.fract() != 0.0 || !n.is_finite() {
        let lang = code.split('-').next().unwrap_or("");
        return if matches!(lang, "cs" | "lt") { "many" } else { "other" };
    }
    let i = n.abs() as u64;
    if i != 0 && i.is_multiple_of(1_000_000) && d.plural.get(1) == Some(&b'o') {
        let lang = code.split('-').next().unwrap_or("");
        if matches!(lang, "fr" | "es" | "it" | "pt") {
            return "many";
        }
    }
    let ix = if (i as usize) < d.plural.len() { i as usize } else { 100 + (i % 100) as usize };
    match d.plural.get(ix) {
        Some(b'z') => "zero",
        Some(b'o') => "one",
        Some(b't') => "two",
        Some(b'f') => "few",
        Some(b'm') => "many",
        _ => "other",
    }
}

pub fn number(code: &str, n: i64) -> String {
    let d = data(code);
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    if n < 0 {
        out.push('-');
    }
    let len = digits.len();
    if len < 4 || (d.min_group >= 2 && len < 5) || d.group.is_empty() {
        out.push_str(&digits);
        return out;
    }
    let mut cuts = Vec::new();
    let mut at = len - 3;
    cuts.push(at);
    let step = if d.indian { 2 } else { 3 };
    while at > step {
        at -= step;
        cuts.push(at);
    }
    let mut prev = 0;
    for c in cuts.iter().rev() {
        out.push_str(&digits[prev..*c]);
        out.push_str(d.group);
        prev = *c;
    }
    out.push_str(&digits[prev..]);
    out
}

pub fn decimal(code: &str, n: f64, frac: usize) -> String {
    let d = data(code);
    let s = format!("{:.*}", frac, n.abs());
    let (int, f) = s.split_once('.').unwrap_or((&s, ""));
    let mut out = number(code, int.parse::<i64>().unwrap_or(0));
    if n < 0.0 {
        out.insert(0, '-');
    }
    let f = f.trim_end_matches('0');
    if !f.is_empty() {
        out.push_str(d.decimal);
        out.push_str(f);
    }
    out
}

pub struct Parts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Parts {
    pub fn of<T: chrono::Datelike + chrono::Timelike>(dt: &T) -> Self {
        Parts {
            year: dt.year(),
            month: dt.month(),
            day: dt.day(),
            weekday: dt.weekday().num_days_from_monday(),
            hour: dt.hour(),
            minute: dt.minute(),
            second: dt.second(),
        }
    }
}

fn months_for(style: &str) -> &'static str {
    match style {
        "ll" => "ll",
        "LLLL" => "LLLL",
        _ => "LL",
    }
}

pub fn format(code: &str, style: &str, p: &Parts, hour24: Option<bool>) -> String {
    let d = data(code);
    let pat = match hour24 {
        Some(true) => d.fmts.get(format!("{style}.h23").as_str()),
        Some(false) => d.fmts.get(format!("{style}.h12").as_str()),
        None => None,
    }
    .or_else(|| d.fmts.get(style))
    .copied()
    .unwrap_or("");
    let twelve = pat.contains("{a}");
    let periods = {
        let (a, b) = if hour24 == Some(false) { (&d.periods_h12, &d.periods_auto) } else { (&d.periods_auto, &d.periods_h12) };
        if a.get(p.hour as usize).is_some_and(|s| !s.is_empty()) { a } else { b }
    };
    let months = d.months.get(months_for(style));
    let mut out = String::with_capacity(pat.len() + 8);
    let mut rest = pat;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let tok = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        let h12 = match p.hour % 12 {
            0 => 12,
            h => h,
        };
        let hour = if twelve { h12 } else { p.hour };
        match tok {
            "y" => out.push_str(&p.year.to_string()),
            "yy" => out.push_str(&format!("{:02}", p.year.rem_euclid(100))),
            "by" => out.push_str(&(p.year + 543).to_string()),
            "byy" => out.push_str(&format!("{:02}", (p.year + 543).rem_euclid(100))),
            "M" => out.push_str(&p.month.to_string()),
            "MM" => out.push_str(&format!("{:02}", p.month)),
            "MMM" => out.push_str(months.and_then(|m| m.get(p.month as usize - 1)).copied().unwrap_or("")),
            "d" => out.push_str(&p.day.to_string()),
            "dd" => out.push_str(&format!("{:02}", p.day)),
            "E" => out.push_str(d.weekdays.get(p.weekday as usize).copied().unwrap_or("")),
            "h" => out.push_str(&hour.to_string()),
            "hh" => out.push_str(&format!("{hour:02}")),
            "m" => out.push_str(&p.minute.to_string()),
            "mm" => out.push_str(&format!("{:02}", p.minute)),
            "s" => out.push_str(&p.second.to_string()),
            "ss" => out.push_str(&format!("{:02}", p.second)),
            "a" => out.push_str(periods.get(p.hour as usize).copied().unwrap_or("")),
            _ => {}
        }
    }
    out.push_str(rest);
    out
}

pub fn month_name(code: &str, month: u32, short: bool) -> &'static str {
    let d = data(code);
    let key = if short { "ll" } else { "LL" };
    let pick = |k: &str| d.months.get(k).and_then(|m| m.get(month.wrapping_sub(1) as usize)).copied();
    match pick(key) {
        Some(s) if !s.chars().all(|c| c.is_ascii_digit()) => s,
        _ => pick("LL").unwrap_or(""),
    }
}

pub fn weekday_name(code: &str, from_monday: u32) -> &'static str {
    data(code).weekdays.get(from_monday as usize).copied().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Parts {
        let dt = chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 6).unwrap();
        Parts::of(&dt)
    }

    #[test]
    fn every_client_locale_has_data() {
        for l in super::super::languages::LANGUAGES {
            assert!(all().contains_key(l.code), "{} missing from intl.txt", l.code);
            assert!(!data(l.code).fmts.get("LT").copied().unwrap_or("").is_empty(), "{} has no LT", l.code);
        }
    }

    #[test]
    fn dates_match_the_client_dump() {
        let d = p(2009, 11, 25, 15, 44);
        assert_eq!(format("de", "LLL", &d, None), "25. November 2009 um 15:44");
        assert_eq!(format("en-US", "LT", &d, None), "3:44\u{202f}PM");
        assert_eq!(format("en-US", "LT", &d, Some(true)), "15:44");
        assert_eq!(format("ja", "LL", &d, None), "2009年11月25日");
        assert_eq!(format("pt-BR", "L", &d, None), "25/11/2009");
        assert_eq!(format("de", "L", &d, None), "25.11.09");
        assert_eq!(format("th", "LL", &d, None), "25 พฤศจิกายน 2552");
        assert_eq!(format("zh-TW", "LT", &p(2009, 2, 5, 3, 4), None), "凌晨3:04");
        assert_eq!(format("ru", "LL", &d, None), "25 ноября 2009\u{a0}г.");
    }

    #[test]
    fn numbers_use_locale_grouping() {
        assert_eq!(number("en-US", 1234567), "1,234,567");
        assert_eq!(number("de", 1234567), "1.234.567");
        assert_eq!(number("es-ES", 1234), "1234");
        assert_eq!(number("es-ES", 12345), "12.345");
        assert_eq!(number("hi", 1234567), "12,34,567");
        assert_eq!(number("fr", 1234), "1\u{202f}234");
        assert_eq!(number("en-US", 999), "999");
        assert_eq!(decimal("de", 1.5, 1), "1,5");
    }

    #[test]
    fn plurals_follow_cldr() {
        assert_eq!(plural("en-US", 1.0), "one");
        assert_eq!(plural("en-US", 0.0), "other");
        assert_eq!(plural("fr", 0.0), "one");
        assert_eq!(plural("fr", 1_000_000.0), "many");
        assert_eq!(plural("ru", 21.0), "one");
        assert_eq!(plural("ru", 22.0), "few");
        assert_eq!(plural("ru", 25.0), "many");
        assert_eq!(plural("ru", 111.0), "many");
        assert_eq!(plural("ru", 1021.0), "one");
        assert_eq!(plural("pl", 12.0), "many");
        assert_eq!(plural("pl", 1022.0), "few");
        assert_eq!(plural("ja", 1.0), "other");
        assert_eq!(plural("cs", 3.0), "few");
        assert_eq!(plural("cs", 1.5), "many");
        assert_eq!(plural("ro", 101.0), "few");
    }
}
