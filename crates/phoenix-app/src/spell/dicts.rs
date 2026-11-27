use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Available {
    pub code: String,
    pub aff: PathBuf,
    pub dic: PathBuf,
}

pub fn search_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(d) = crate::paths::config_file("dictionaries") {
        out.push(d);
    }
    if cfg!(test) {
        return out;
    }
    if let Some(list) = std::env::var_os("DICPATH") {
        out.extend(std::env::split_paths(&list));
    }
    let home = crate::paths::home();
    if cfg!(target_os = "macos") {
        if let Some(h) = &home {
            out.push(h.join("Library/Spelling"));
        }
        out.push(PathBuf::from("/Library/Spelling"));
        out.push(PathBuf::from("/opt/homebrew/share/hunspell"));
        out.push(PathBuf::from("/usr/local/share/hunspell"));
    } else if cfg!(windows) {
        for var in ["LOCALAPPDATA", "ProgramData"] {
            if let Some(base) = std::env::var_os(var) {
                out.push(PathBuf::from(base).join("hunspell"));
            }
        }
    } else {
        if let Some(h) = &home {
            out.push(h.join(".local/share/hunspell"));
        }
        for d in ["/usr/share/hunspell", "/usr/share/myspell", "/usr/share/myspell/dicts", "/usr/local/share/hunspell", "/app/share/hunspell"] {
            out.push(PathBuf::from(d));
        }
    }
    out
}

pub fn scan(dirs: &[PathBuf]) -> Vec<Available> {
    let mut out: Vec<Available> = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        let mut names: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        names.sort();
        for dic in names {
            if dic.extension().and_then(|e| e.to_str()) != Some("dic") {
                continue;
            }
            let aff = dic.with_extension("aff");
            let Some(code) = dic.file_stem().and_then(|s| s.to_str()) else { continue };
            if !aff.is_file() || code.starts_with("hyph") || out.iter().any(|a| a.code == code) {
                continue;
            }
            out.push(Available { code: code.to_owned(), aff, dic });
        }
    }
    out
}

pub fn normalize(tag: &str) -> String {
    let tag = tag.split(['.', '@']).next().unwrap_or("").replace('-', "_");
    let mut parts = tag.split('_');
    let lang = parts.next().unwrap_or("").to_lowercase();
    match parts.next() {
        Some(region) if !region.is_empty() => format!("{lang}_{}", region.to_uppercase()),
        _ => lang,
    }
}

pub fn candidates(app_locale: &str, system: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in std::iter::once(app_locale.to_owned()).chain(system.iter().cloned()) {
        let n = normalize(&tag);
        if n.is_empty() || n == "c" || n == "posix" || out.contains(&n) {
            continue;
        }
        out.push(n);
    }
    out
}

pub fn system_languages() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(list) = std::env::var("LANGUAGE") {
        out.extend(list.split(':').filter(|s| !s.is_empty()).map(str::to_owned));
    }
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
        {
            out.push(v);
        }
    }
    out
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

pub fn resolve<'a>(wanted: &[String], available: &'a [Available]) -> Vec<&'a Available> {
    let mut out: Vec<&Available> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut push = |a: &'a Available, out: &mut Vec<&'a Available>| {
        let real = canonical(&a.dic);
        if !seen.contains(&real) {
            seen.push(real);
            out.push(a);
        }
    };
    for w in wanted {
        let lang = w.split('_').next().unwrap_or(w);
        let exact = available.iter().find(|a| normalize(&a.code) == *w);
        let fallback = || {
            available.iter().find(|a| normalize(&a.code) == lang).or_else(|| {
                let mut same: Vec<&Available> = available.iter().filter(|a| normalize(&a.code).split('_').next() == Some(lang)).collect();
                same.sort_by_key(|a| {
                    let n = normalize(&a.code);
                    let region = n.split('_').nth(1).unwrap_or("").to_lowercase();
                    (region != lang, n)
                });
                same.first().copied()
            })
        };
        if let Some(a) = exact.or_else(fallback) {
            push(a, &mut out);
        }
    }
    out
}

const LATIN9: [(u8, char); 8] =
    [(0xA4, '€'), (0xA6, 'Š'), (0xA8, 'š'), (0xB4, 'Ž'), (0xB8, 'ž'), (0xBC, 'Œ'), (0xBD, 'œ'), (0xBE, 'Ÿ')];

pub fn encoding(aff: &[u8]) -> String {
    let head = String::from_utf8_lossy(&aff[..aff.len().min(64 * 1024)]);
    head.lines()
        .find_map(|l| l.trim_start_matches('\u{feff}').strip_prefix("SET "))
        .map(|s| s.trim().to_uppercase())
        .unwrap_or_else(|| "UTF-8".into())
}

pub fn decode(bytes: &[u8], enc: &str) -> Option<String> {
    match enc {
        "UTF-8" | "UTF8" => Some(String::from_utf8_lossy(bytes).trim_start_matches('\u{feff}').to_owned()),
        "ISO8859-1" | "ISO-8859-1" => Some(bytes.iter().map(|&b| b as char).collect()),
        "ISO8859-15" | "ISO-8859-15" => {
            Some(bytes.iter().map(|&b| LATIN9.iter().find(|(k, _)| *k == b).map_or(b as char, |(_, c)| *c)).collect())
        }
        _ => None,
    }
}

pub fn load(a: &Available) -> Option<spellbook::Dictionary> {
    let aff = std::fs::read(&a.aff).ok()?;
    let dic = std::fs::read(&a.dic).ok()?;
    let enc = encoding(&aff);
    let (Some(aff), Some(dic)) = (decode(&aff, &enc), decode(&dic, &enc)) else {
        tracing::info!("spellcheck: {} uses unsupported encoding {enc}", a.code);
        return None;
    };
    match spellbook::Dictionary::new(&aff, &dic) {
        Ok(d) => Some(d),
        Err(e) => {
            tracing::info!("spellcheck: cannot parse {}: {e}", a.code);
            None
        }
    }
}
