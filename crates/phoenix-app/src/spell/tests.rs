use super::{dicts, words, *};

const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ\n";
const DIC: &str = "6\nbonjour\nfaux\nami\neau\nhello\nworld\n";

fn dict() -> Dictionary {
    Dictionary::new(AFF, DIC).expect("test dictionary")
}

fn texts(text: &str, ranges: &[Range<usize>]) -> Vec<String> {
    ranges.iter().map(|r| text[r.clone()].to_owned()).collect()
}

#[test]
fn words_skip_code_mentions_emoji_urls_and_commands() {
    let text = "/giphy chat bonjour `fauxx` <@123456> @someone #salon :smile: https://exemple.fr/chemin www.site.org fin";
    assert_eq!(texts(text, &words::words(text, None)), ["chat", "bonjour", "fin"]);
    let block = "avant\n```rust\nlet zzz = 1;\n```\napres";
    assert_eq!(texts(block, &words::words(block, None)), ["avant", "apres"]);
    let emoji = "coucou <:pepe:123456789> <a:dance:987654321> :partial";
    assert_eq!(texts(emoji, &words::words(emoji, None)), ["coucou"]);
    let mixed = "abc123 snake_case l'eau";
    assert_eq!(texts(mixed, &words::words(mixed, None)), ["l'eau"]);
}

#[test]
fn the_word_being_typed_is_not_checked_yet() {
    let text = "bonjour fauxx";
    assert_eq!(texts(text, &words::words(text, Some(text.len()))), ["bonjour"]);
    assert_eq!(texts(text, &words::words(text, Some(3))), ["bonjour", "fauxx"]);
    assert_eq!(texts(text, &words::words(text, None)), ["bonjour", "fauxx"]);
}

#[test]
fn check_accepts_case_elision_and_rejects_typos() {
    let d = [dict()];
    assert!(check_word(&d, "bonjour"));
    assert!(check_word(&d, "Bonjour"));
    assert!(check_word(&d, "l'eau"));
    assert!(check_word(&d, "l’eau"));
    assert!(!check_word(&d, "fauxx"));
    assert!(!check_word(&d, "bonjur"));
    assert!(check_word(&[], "fauxx"));
}

#[test]
fn multilingual_check_accepts_a_word_from_any_dictionary() {
    let en = Dictionary::new(AFF, "2\nhouse\ncat\n").expect("en");
    let d = [dict(), en];
    assert!(check_word(&d, "house"));
    assert!(check_word(&d, "ami"));
    assert!(!check_word(&d, "hous"));
}

#[test]
fn suggestions_are_capped_like_the_client() {
    let d = [dict()];
    let s = suggest_word(&d, "bonjur");
    assert_eq!(s.first().map(String::as_str), Some("bonjour"));
    assert!(s.len() <= MAX_SUGGESTIONS);
}

#[test]
fn classify_marks_known_typos_and_queues_new_words() {
    let text = "bonjour fauxx ami zzz ";
    let lookup = |w: &str| match w {
        "bonjour" | "ami" => Some(true),
        "fauxx" => Some(false),
        _ => None,
    };
    let (bad, missing) = classify(text, None, lookup);
    assert_eq!(texts(text, &bad), ["fauxx"]);
    assert_eq!(missing, ["zzz"]);
}

#[test]
fn learned_words_round_trip_lowercased() {
    let set = parse_learned(br#"{"learnedWords":["Phoenix","gpui"]}"#);
    assert!(set.contains("phoenix") && set.contains("gpui"));
    assert_eq!(parse_learned(&learned_json(&set)), set);
    assert!(parse_learned(b"not json").is_empty());
    assert!(words::contains_word("Salut PHOENIX !", "phoenix"));
    assert!(!words::contains_word("phoenixes", "phoenix"));
}

#[test]
fn nearest_occurrence_is_replaced() {
    let text = "fauxx et fauxx";
    assert_eq!(words::nearest(text, "fauxx", 13), Some(9..14));
    assert_eq!(words::nearest(text, "fauxx", 0), Some(0..5));
    assert_eq!(words::word_at(text, 7), Some(6..8));
}

#[test]
fn languages_follow_the_app_then_the_system() {
    assert_eq!(dicts::normalize("fr-FR"), "fr_FR");
    assert_eq!(dicts::normalize("en_US.UTF-8"), "en_US");
    assert_eq!(dicts::candidates("fr", &["fr_FR.UTF-8".into(), "C".into(), "en_US.UTF-8".into()]), ["fr", "fr_FR", "en_US"]);
    let dir = std::env::temp_dir().join(format!("phx-spell-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    for code in ["en_GB", "en_US", "fr_FR", "de_DE_frami"] {
        std::fs::write(dir.join(format!("{code}.aff")), AFF).expect("aff");
        std::fs::write(dir.join(format!("{code}.dic")), DIC).expect("dic");
    }
    std::fs::write(dir.join("hyph_fr.dic"), "x").expect("hyph");
    let avail = dicts::scan(std::slice::from_ref(&dir));
    assert_eq!(avail.iter().map(|a| a.code.as_str()).collect::<Vec<_>>(), ["de_DE_frami", "en_GB", "en_US", "fr_FR"]);
    let codes = |want: &[&str]| {
        let w: Vec<String> = want.iter().map(|s| s.to_string()).collect();
        dicts::resolve(&w, &avail).into_iter().map(|a| a.code.clone()).collect::<Vec<_>>()
    };
    assert_eq!(codes(&["fr"]), ["fr_FR"]);
    assert_eq!(codes(&["en_GB"]), ["en_GB"]);
    assert_eq!(codes(&["en", "en_US"]), ["en_GB", "en_US"]);
    assert_eq!(codes(&["de"]), ["de_DE_frami"]);
    assert!(codes(&["ja"]).is_empty());
    assert!(dicts::load(&avail[3]).is_some_and(|d| d.check("bonjour")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn legacy_encodings_are_decoded() {
    assert_eq!(dicts::encoding(b"# x\nSET ISO8859-15\nTRY a"), "ISO8859-15");
    assert_eq!(dicts::encoding(b"TRY a"), "UTF-8");
    assert_eq!(dicts::decode(&[0x63, 0x6F, 0x65, 0xE9], "ISO8859-1").as_deref(), Some("coeé"));
    assert_eq!(dicts::decode(&[0xBD, 0x75, 0x66], "ISO8859-15").as_deref(), Some("œuf"));
    assert_eq!(dicts::decode(b"x", "KOI8-R"), None);
}
