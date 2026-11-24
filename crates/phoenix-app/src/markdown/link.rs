//! Masked links `[label](url "title")`, as the 2026 client parses them.
//!
//! Source: the local client bundle (`current-client/js/web.b63d846b329d6470.js`):
//! - module 694403 `R` (the `link` rule): `match` refuses when links are off or there is no
//!   `](`, and bails on more than 10 nested `[`, more than 100 characters at depth >= 2 or
//!   more than 200 brackets / parens without a `)`; then it runs simple-markdown's link
//!   regex (module 361020, with `[^\s\\()]` in the href instead of upstream's `[^\s\\]`):
//!   `^\[((?:\[[^\]]*\]|[^\[\]]|\](?=[^\[]*\]))*)\]\(\s*<?((?:\([^)]*\)|[^\s\\()]|\\.)*?)>?(?:\s+['"]([\s\S]*?)['"])?\s*\)`
//! - `parse`: a match that fails validation becomes ONE plain text node holding the whole
//!   match (`u()`), so nothing inside it is formatted. Validation: no masked link nested in
//!   the label, no suspicious URL characters (`C`), label and URL not blank, protocol in
//!   http / https / discord / tel / sms / mailto with a host for http(s) (`O`), label made of
//!   text formatting only (text, u, strong, em, s, inline code, spoiler, br, timestamp, and
//!   emoji only when `allowEmojiLinks`, which messages enable for webhooks only and rich
//!   embeds always), and a label that doesn't itself contain a URL (module 486889 `ei`).

/// A successful regex match, as byte offsets into the source (which starts with `[`).
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Match {
    pub label: std::ops::Range<usize>,
    pub url: std::ops::Range<usize>,
    /// Total length of the match.
    pub len: usize,
}

/// `R.match` pre-checks (bracket counters), on everything from the `[` on.
fn within_limits(s: &str) -> bool {
    if !s.contains("](") {
        return false;
    }
    let (mut depth, mut since_paren, mut deep) = (0u32, 0u32, 0u32);
    for ch in s.chars() {
        if depth >= 2 {
            deep += 1;
            if deep > 100 {
                return false;
            }
        } else {
            deep = 0;
        }
        match ch {
            ')' => {
                since_paren = 0;
                continue;
            }
            '[' => {
                depth += 1;
                if depth > 10 {
                    return false;
                }
            }
            ']' => depth = depth.saturating_sub(1),
            '(' => {}
            _ => continue,
        }
        since_paren += 1;
        if since_paren > 200 {
            return false;
        }
    }
    true
}

fn skip_ws(s: &str, mut p: usize) -> usize {
    while let Some(ch) = s[p..].chars().next().filter(|c| c.is_whitespace()) {
        p += ch.len_utf8();
    }
    p
}

/// `>?(?:\s+['"]([\s\S]*?)['"])?\s*\)` at `p`: the end of the match.
fn tail(s: &str, p: usize) -> Option<usize> {
    for gt in [true, false] {
        let mut p = p;
        if gt {
            if !s[p..].starts_with('>') {
                continue;
            }
            p += 1;
        }
        // With a title (greedy `?` first).
        let w = skip_ws(s, p);
        if w > p && s[w..].starts_with(['\'', '"']) {
            let mut k = w + 1;
            while let Some(q) = s[k..].find(['\'', '"']) {
                let e = skip_ws(s, k + q + 1);
                if s[e..].starts_with(')') {
                    return Some(e + 1);
                }
                k += q + 1;
            }
        }
        let e = skip_ws(s, p);
        if s[e..].starts_with(')') {
            return Some(e + 1);
        }
    }
    None
}

/// The href part from just after `](`: (url range, end of the match).
fn href(s: &str, start: usize) -> Option<(std::ops::Range<usize>, usize)> {
    let q = skip_ws(s, start);
    for lt in [true, false] {
        if lt && !s[q..].starts_with('<') {
            continue;
        }
        let us = q + usize::from(lt);
        let mut u = us;
        // Lazy `*?`: try the tail before every extra unit.
        loop {
            if let Some(end) = tail(s, u) {
                return Some((us..u, end));
            }
            let Some(ch) = s[u..].chars().next() else { break };
            match ch {
                '(' => match s[u + 1..].find(')') {
                    Some(e) => u += e + 2,
                    None => break,
                },
                // `\\.`: `.` doesn't match a newline.
                '\\' => match s[u + 1..].chars().next() {
                    Some(n) if n != '\n' => u += 1 + n.len_utf8(),
                    _ => break,
                },
                ')' => break,
                c if c.is_whitespace() => break,
                c => u += c.len_utf8(),
            }
        }
    }
    None
}

/// The simple-markdown link regex at the start of `s` (with the client's pre-checks).
pub(super) fn match_at(s: &str) -> Option<Match> {
    if !s.starts_with('[') || !within_limits(s) {
        return None;
    }
    let b = s.as_bytes();
    // Every place where the greedy label group may stop (a `]`), in order.
    let mut ends = Vec::new();
    let mut p = 1;
    while p < b.len() {
        match b[p] {
            // `\[[^\]]*\]`: a bracket pair with no `]` inside.
            b'[' => match s[p + 1..].find(']') {
                Some(e) => p += e + 2,
                None => break,
            },
            // `\](?=[^\[]*\])`: a lone `]` only if another `]` comes before any `[`.
            b']' => {
                ends.push(p);
                let after = &s[p + 1..];
                match (after.find(']'), after.find('[')) {
                    (Some(c), Some(o)) if c < o => p += 1,
                    (Some(_), None) => p += 1,
                    _ => break,
                }
            }
            _ => p += 1,
        }
    }
    // Greedy: the longest label that is followed by a valid `(href)`.
    ends.into_iter().rev().find_map(|end| {
        if !s[end..].starts_with("](") {
            return None;
        }
        let (url, len) = href(s, end + 2)?;
        Some(Match { label: 1..end, url, len })
    })
}

/// `417964` sanitising regex: Cf, Zl, Zp, Cc but `\n`, Zs but space, and Hangul / Khmer
/// fillers. A URL holding one of these is "suspicious"; they are stripped from labels.
fn invisible(c: char) -> bool {
    matches!(c,
        '\u{0}'..='\u{9}' | '\u{b}'..='\u{1f}' | '\u{7f}'..='\u{9f}'
        | '\u{a0}' | '\u{ad}' | '\u{600}'..='\u{605}' | '\u{61c}' | '\u{6dd}' | '\u{70f}'
        | '\u{1680}' | '\u{180e}' | '\u{2000}'..='\u{200f}' | '\u{2028}'..='\u{202f}'
        | '\u{205f}'..='\u{2064}' | '\u{2066}'..='\u{206f}' | '\u{3000}' | '\u{feff}'
        | '\u{fff9}'..='\u{fffb}' | '\u{34f}' | '\u{17b4}' | '\u{17b5}' | '\u{1160}'
        | '\u{3164}' | '\u{ffa0}' | '\u{e0001}' | '\u{e0020}'..='\u{e007f}')
}

/// simple-markdown `unescapeUrl`: `\\([^0-9A-Za-z\s])` → `$1`.
fn unescape(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let mut it = url.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\'
            && let Some(&n) = it.peek()
            && !n.is_ascii_alphanumeric()
            && !n.is_whitespace()
        {
            out.push(n);
            it.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// `C` (suspicious URL) then `O` (allowed protocol, host): the target, or `None`.
pub(super) fn validate_url(raw: &str) -> Option<String> {
    if raw.trim().is_empty() || raw.chars().any(invisible) {
        return None;
    }
    let url = unescape(raw);
    let colon = url.find(':')?;
    let scheme = url[..colon].to_ascii_lowercase();
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    match scheme.as_str() {
        "http" | "https" => {
            // `e.split("/")`: `scheme:` `` `host…`, the host not percent-encoded.
            let parts: Vec<&str> = raw.split('/').collect();
            if parts.len() < 3 || !parts[1].is_empty() || parts[2].contains('%') {
                return None;
            }
            let rest = url[colon + 1..].trim_start_matches(['/', '\\']);
            let auth = rest.split(['/', '?', '#', '\\']).next().unwrap_or("");
            let host = auth.rsplit('@').next().unwrap_or("");
            let host = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map_or(host, |(h, _)| h);
            if host.is_empty() || host.contains(char::is_whitespace) {
                return None;
            }
        }
        "discord" | "tel" | "sms" | "mailto" => {}
        _ => return None,
    }
    Some(url)
}

/// Module 486889 `I`: `(https?:)?//` or `www.`, optional `user@`, then `localhost`, an IP or
/// `label.` … + a 2+ letter TLD.
fn contains_url(s: &str) -> bool {
    let lower = s.to_lowercase();
    let mut i = 0;
    while i < lower.len() {
        let rest = &lower[i..];
        let after = rest.strip_prefix("//").or_else(|| rest.strip_prefix("www."));
        if let Some(a) = after {
            let host: &str = a.split(|c: char| c.is_whitespace() || "/?#\"()[]<>".contains(c)).next().unwrap_or("");
            let host = host.rsplit('@').next().unwrap_or(host);
            let host = host.split(':').next().unwrap_or(host);
            let labels: Vec<&str> = host.split('.').collect();
            let tld_ok = labels.len() >= 2
                && labels.last().is_some_and(|t| t.chars().count() >= 2 && t.chars().all(|c| c.is_alphabetic()))
                && labels[..labels.len() - 1].iter().all(|l| !l.is_empty());
            let ip = labels.len() == 4 && labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()));
            if host == "localhost" || tld_ok || ip {
                return true;
            }
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// The label's node types are restricted to text formatting (`I` / `p` in module 694403).
/// Phoenix's inline parser turns these other constructs into spans: refuse them up front.
pub(super) fn label_ok(label: &str, emoji_links: bool) -> bool {
    let clean: String = label.chars().filter(|&c| !invisible(c)).collect();
    if clean.trim().is_empty() || contains_url(&clean) {
        return false;
    }
    // Mentions, channel / role mentions, custom emoji, sound / command / static links, autolinks.
    let mut i = 0;
    while let Some(p) = clean[i..].find('<') {
        let rest = &clean[i + p..];
        if let Some(end) = rest.find('>') {
            let inner = &rest[1..end];
            let id = |pfx: &str| inner.strip_prefix(pfx).is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
            if id("@") || id("@!") || id("@&") || id("#") || inner.starts_with("sound:") || inner.starts_with('/') || inner.starts_with("id:") {
                return false;
            }
            if !inner.contains(' ') && inner.contains(":/") {
                return false;
            }
        }
        i += p + 1;
    }
    if clean.contains("@everyone") || clean.contains("@here") {
        return false;
    }
    if clean.contains("http://") || clean.contains("https://") || clean.contains("steam://") {
        return false;
    }
    // Nested masked link.
    if clean.char_indices().any(|(k, c)| c == '[' && match_at(&clean[k..]).is_some_and(|m| validate_url(&clean[k..][m.url.clone()]).is_some())) {
        return false;
    }
    if !emoji_links {
        // Custom emoji are kept as `<:name:id>` text by the inline parser; Unicode emoji
        // become `emoji` nodes in the client's text rule.
        if !crate::richtext::emoji_ranges(&clean).is_empty() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(s: &str) -> Option<(&str, &str, usize)> {
        match_at(s).map(|m| (&s[m.label.clone()], &s[m.url.clone()], m.len))
    }

    #[test]
    fn nearest_bracket_wins() {
        // The first `[` has an unbalanced `]` after it: no match there.
        let s = "[Test Phoenix] envoyé — [phxsurligne](https://example.com/a) `x`";
        assert_eq!(m(s), None);
        let k = s.find("[phx").unwrap();
        assert_eq!(m(&s[k..]), Some(("phxsurligne", "https://example.com/a", "[phxsurligne](https://example.com/a)".len())));
    }

    #[test]
    fn label_brackets() {
        assert_eq!(m("[a [b] c](https://x.y)").map(|r| r.0), Some("a [b] c"));
        // A lone `]` is allowed when another `]` follows before any `[`.
        assert_eq!(m("[a ] b](https://x.y)").map(|r| r.0), Some("a ] b"));
        assert_eq!(m("[a [b c](https://x.y)"), None);
    }

    #[test]
    fn href_forms() {
        assert_eq!(m("[a](<https://x.y/a b>)"), None);
        assert_eq!(m("[a](<https://x.y/a>)").map(|r| r.1), Some("https://x.y/a"));
        assert_eq!(m("[a](https://x.y/w_(b))").map(|r| r.1), Some("https://x.y/w_(b)"));
        assert_eq!(m("[a](https://x.y \"t\")").map(|r| r.1), Some("https://x.y"));
        assert_eq!(m("[a]( https://x.y )").map(|r| r.1), Some("https://x.y"));
        assert_eq!(m("[a](https://x.y"), None);
    }

    #[test]
    fn url_rules() {
        assert_eq!(validate_url("https://example.com/a").as_deref(), Some("https://example.com/a"));
        assert!(validate_url("javascript:alert(1)").is_none());
        assert!(validate_url("ftp://x.y").is_none());
        assert!(validate_url("https:x.y").is_none());
        assert!(validate_url("https://%65xample.com").is_none());
        assert!(validate_url("https://exa\u{200b}mple.com").is_none());
        assert!(validate_url("mailto:a@b.c").is_some());
        assert!(validate_url("https://").is_none());
    }

    #[test]
    fn label_rules() {
        assert!(label_ok("**docs**", false));
        assert!(!label_ok("  ", false));
        // No `//` / `www.`: not a URL for the client.
        assert!(label_ok("example.com", false));
        assert!(!label_ok("www.example.com", false));
        assert!(!label_ok("https://evil.com", false));
        assert!(!label_ok("<@123>", false));
        assert!(!label_ok("fire 🔥", false));
        assert!(label_ok("fire 🔥", true));
    }
}
