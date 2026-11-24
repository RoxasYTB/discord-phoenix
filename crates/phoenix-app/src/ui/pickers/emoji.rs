pub use super::emoji_data::EMOJI;

pub const CATEGORIES: [&str; 8] = [
    "People",
    "Nature",
    "Food & Drink",
    "Activities",
    "Travel & Places",
    "Objects",
    "Symbols",
    "Flags",
];

pub const CATEGORY_ICON: [&str; 8] = ["😀", "🐻", "🍔", "⚽", "🚗", "💡", "❤️", "🏁"];

pub const TONE_PREVIEW: &str = "👏";

pub fn glyph(ix: usize) -> &'static str {
    EMOJI[ix].0
}

pub fn shortcode(ix: usize) -> &'static str {
    let e = &EMOJI[ix];
    e.2.split(' ').find(|a| !a.is_empty()).unwrap_or(e.1)
}

pub fn supports_tone(ix: usize) -> bool {
    EMOJI[ix].4
}

pub fn with_tone(e: &str, tone: u8) -> String {
    if tone == 0 || tone > 5 {
        return e.to_owned();
    }
    let modifier = char::from_u32(0x1F3FA + tone as u32).unwrap_or('\u{1F3FB}');
    let mut chars = e.chars();
    let Some(first) = chars.next() else { return String::new() };
    let rest: String = chars.collect();
    let mut out = String::with_capacity(e.len() + 4);
    out.push(first);
    out.push(modifier);
    out.push_str(rest.strip_prefix('\u{FE0F}').unwrap_or(&rest));
    out
}

pub fn toned(ix: usize, tone: u8) -> String {
    if supports_tone(ix) { with_tone(glyph(ix), tone) } else { glyph(ix).to_owned() }
}

pub fn index_of(glyph: &str) -> Option<usize> {
    EMOJI.iter().position(|e| e.0 == glyph)
}

pub fn name_score(name: &str, q: &str) -> Option<u8> {
    if name == q {
        Some(0)
    } else if name.starts_with(q) {
        Some(1)
    } else if name.split(['_', '-']).any(|w| w.starts_with(q)) {
        Some(2)
    } else if q.len() >= 2 && name.contains(q) {
        Some(3)
    } else {
        None
    }
}

pub fn search(q: &str, limit: usize) -> Vec<usize> {
    let q = q.trim().trim_matches(':').to_lowercase().replace(' ', "_");
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize)> = EMOJI
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let alias = e.2.split(' ').filter(|a| !a.is_empty()).filter_map(|a| name_score(a, &q)).min();
            let main = name_score(e.1, &q);
            alias.into_iter().chain(main).min().map(|s| (s, i))
        })
        .collect();
    hits.sort();
    hits.truncate(limit);
    hits.into_iter().map(|(_, i)| i).collect()
}

pub fn by_shortcode(name: &str) -> Option<usize> {
    let name = name.trim_matches(':');
    EMOJI.iter().position(|e| e.1 == name || e.2.split(' ').any(|a| a == name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sane() {
        assert!(EMOJI.len() > 1500);
        assert_eq!(glyph(0), "😀");
        assert_eq!(shortcode(0), "grinning");
        assert!(EMOJI.iter().all(|e| (e.3 as usize) < CATEGORIES.len()));
        for icon in CATEGORY_ICON {
            assert!(index_of(icon).is_some(), "{icon}");
        }
    }

    #[test]
    fn search_ranks_exact_first() {
        let r = search("thumbsup", 5);
        assert_eq!(glyph(r[0]), "👍");
        let r = search(":joy", 5);
        assert_eq!(glyph(r[0]), "😂");
        assert!(search("", 5).is_empty());
        assert!(search("zzzzqqq", 5).is_empty());
        assert_eq!(by_shortcode(":fire:").map(glyph), Some("🔥"));
    }

    #[test]
    fn skin_tones() {
        assert_eq!(with_tone("👍", 3), "👍🏽");
        assert_eq!(with_tone("👍", 0), "👍");
        assert_eq!(with_tone("☝️", 1), "☝🏻");
        assert_eq!(with_tone("👩‍💻", 5), "👩🏿‍💻");
        let thumbs = index_of("👍").unwrap();
        assert!(supports_tone(thumbs));
        assert!(!supports_tone(index_of("🔥").unwrap()));
    }
}
