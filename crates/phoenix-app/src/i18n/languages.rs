#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub native: &'static str,
    pub english: &'static str,
}

pub const LANGUAGES: &[Language] = &[
    Language { code: "da", native: "Dansk", english: "Danish" },
    Language { code: "de", native: "Deutsch", english: "German" },
    Language { code: "en-GB", native: "English, GB", english: "English, GB" },
    Language { code: "en-US", native: "English, USA", english: "English, USA" },
    Language { code: "es-ES", native: "Español", english: "Spanish" },
    Language { code: "es-419", native: "Español, LATAM", english: "Spanish, LATAM" },
    Language { code: "fr", native: "Français", english: "French" },
    Language { code: "hr", native: "Hrvatski", english: "Croatian" },
    Language { code: "it", native: "Italiano", english: "Italian" },
    Language { code: "lt", native: "Lietuviškai", english: "Lithuanian" },
    Language { code: "hu", native: "Magyar", english: "Hungarian" },
    Language { code: "nl", native: "Nederlands", english: "Dutch" },
    Language { code: "no", native: "Norwegian", english: "Norwegian" },
    Language { code: "pl", native: "Polski", english: "Polish" },
    Language { code: "pt-BR", native: "Português do Brasil", english: "Portuguese, Brazilian" },
    Language { code: "ro", native: "Română", english: "Romanian" },
    Language { code: "fi", native: "Suomi", english: "Finnish" },
    Language { code: "sv-SE", native: "Svenska", english: "Swedish" },
    Language { code: "vi", native: "Tiếng Việt", english: "Vietnamese" },
    Language { code: "tr", native: "Türkçe", english: "Turkish" },
    Language { code: "cs", native: "Čeština", english: "Czech" },
    Language { code: "el", native: "Ελληνικά", english: "Greek" },
    Language { code: "bg", native: "български", english: "Bulgarian" },
    Language { code: "ru", native: "Русский", english: "Russian" },
    Language { code: "uk", native: "Українська", english: "Ukrainian" },
    Language { code: "hi", native: "हिंदी", english: "Hindi" },
    Language { code: "th", native: "ไทย", english: "Thai" },
    Language { code: "zh-CN", native: "中文", english: "Chinese Simplified" },
    Language { code: "ja", native: "日本語", english: "Japanese" },
    Language { code: "zh-TW", native: "繁體中文", english: "Traditional Chinese" },
    Language { code: "ko", native: "한국어", english: "Korean" },
];

pub fn normalize(code: &str) -> &'static str {
    supported_code(code).unwrap_or("en-US")
}

pub(super) fn supported_code(code: &str) -> Option<&'static str> {
    let trimmed = code.trim();
    if let Some(l) = LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(trimmed)) {
        return Some(l.code);
    }
    let norm = trimmed.replace('_', "-");
    if let Some(l) = LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(&norm)) {
        return Some(l.code);
    }
    let primary = norm.split('-').next().unwrap_or("").to_ascii_lowercase();
    match primary.as_str() {
        "en" => Some("en-US"),
        "es" => Some("es-ES"),
        "pt" => Some("pt-BR"),
        "zh" => Some("zh-CN"),
        "sv" => Some("sv-SE"),
        "nb" | "nn" => Some("no"),
        _ => LANGUAGES.iter().find(|l| l.code == primary).map(|l| l.code),
    }
}

pub fn index_of(code: &str) -> usize {
    let norm = normalize(code);
    LANGUAGES.iter().position(|l| l.code == norm).unwrap_or(3)
}

pub fn find(code: &str) -> &'static Language {
    &LANGUAGES[index_of(code)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_aliases_and_variants() {
        assert_eq!(normalize("en"), "en-US");
        assert_eq!(normalize("en_GB"), "en-GB");
        assert_eq!(normalize("pt"), "pt-BR");
        assert_eq!(normalize("pt-br"), "pt-BR");
        assert_eq!(normalize("de"), "de");
        assert_eq!(normalize("ja"), "ja");
        assert_eq!(normalize("nb"), "no");
        assert_eq!(normalize("xx-YY"), "en-US");
        assert_eq!(supported_code("fr-FR"), Some("fr"));
        assert_eq!(supported_code("fr_CA"), Some("fr"));
        assert_eq!(supported_code("en-US"), Some("en-US"));
        assert_eq!(supported_code("xx-YY"), None);
    }

    #[test]
    fn has_31_enabled_client_languages() {
        assert_eq!(LANGUAGES.len(), 31);
        assert_eq!(find("en-US").native, "English, USA");
        assert_eq!(find("ja").native, "日本語");
        assert_eq!(find("pt-BR").native, "Português do Brasil");
    }
}
