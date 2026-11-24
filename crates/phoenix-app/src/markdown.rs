use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Style(pub u16);

impl Style {
    pub const PLAIN: Style = Style(0);
    pub const BOLD: Style = Style(1);
    pub const ITALIC: Style = Style(1 << 1);
    pub const UNDERLINE: Style = Style(1 << 2);
    pub const STRIKE: Style = Style(1 << 3);
    pub const CODE: Style = Style(1 << 4);
    pub const SPOILER: Style = Style(1 << 5);
    pub const LINK: Style = Style(1 << 6);
    pub const MENTION: Style = Style(1 << 7);
    pub const TIMESTAMP: Style = Style(1 << 8);
    pub const SOUNDBOARD: Style = Style(1 << 9);

    pub fn has(self, other: Style) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }
}

impl std::ops::BitOr for Style {
    type Output = Style;
    fn bitor(self, rhs: Style) -> Style {
        Style(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mention {
    User(u64),
    Channel(u64),
    Role(u64),
    Everyone,
    Here,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
    pub link: Option<String>,
    pub mention: Option<Mention>,
    pub spoiler: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inline),
    Quote(Inline),
    Code { lang: Option<String>, code: String },
    Heading(u8, Inline),
    Subtext(Inline),
    ListItem { ordered: Option<u32>, depth: u8, inline: Inline },
}

impl Block {
    pub fn inline(&self) -> Option<&Inline> {
        match self {
            Block::Paragraph(i) | Block::Quote(i) | Block::Heading(_, i) | Block::Subtext(i) => Some(i),
            Block::ListItem { inline, .. } => Some(inline),
            Block::Code { .. } => None,
        }
    }
}

fn line_block(line: &str, r: &dyn MentionResolver, sp: &mut usize) -> Option<Block> {
    for (pfx, lvl) in [("### ", 3u8), ("## ", 2), ("# ", 1)] {
        if let Some(t) = line.strip_prefix(pfx)
            && !t.trim().is_empty()
        {
            return Some(Block::Heading(lvl, parse_inline(t.trim(), r, sp)));
        }
    }
    if let Some(t) = line.strip_prefix("-# ")
        && !t.trim().is_empty()
    {
        return Some(Block::Subtext(parse_inline(t.trim(), r, sp)));
    }
    let trimmed = line.trim_start_matches(' ');
    let depth = ((line.len() - trimmed.len()) / 2).min(2) as u8;
    if let Some(t) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "))
        && !t.trim().is_empty()
    {
        return Some(Block::ListItem { ordered: None, depth, inline: parse_inline(t, r, sp) });
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && digits < 10
        && let Some(t) = trimmed[digits..].strip_prefix(". ")
        && !t.trim().is_empty()
    {
        let n = trimmed[..digits].parse().unwrap_or(1);
        return Some(Block::ListItem { ordered: Some(n), depth, inline: parse_inline(t, r, sp) });
    }
    None
}

pub fn format_timestamp(ts: i64, style: char) -> Option<String> {
    use chrono::{Local, TimeZone};
    let dt = Local.timestamp_opt(ts, 0).single()?;
    Some(match style {
        't' => crate::i18n::clock(dt),
        'T' => if crate::ui::settings::effects::hour24().unwrap_or(crate::i18n::locale() != "en") { dt.format("%H:%M:%S").to_string() } else { dt.format("%-I:%M:%S %p").to_string() },
        'd' => crate::i18n::short_date(dt),
        'D' => crate::i18n::long_date(dt.date_naive()),
        'F' => crate::i18n::full_datetime(dt),
        'R' => relative(ts - Local::now().timestamp()),
        _ => format!("{} {}", crate::i18n::long_date(dt.date_naive()), crate::i18n::clock(dt)),
    })
}

fn relative(delta: i64) -> String {
    crate::i18n::relative_long(delta)
}

pub trait MentionResolver {
    fn user(&self, id: u64) -> Option<String>;
    fn channel(&self, id: u64) -> Option<String>;
    fn role(&self, _id: u64) -> Option<String> {
        None
    }
    fn role_color(&self, _id: u64) -> Option<u32> {
        None
    }
    fn has_role(&self, _id: u64) -> bool {
        false
    }
    fn soundboard(&self, _guild_id: u64, _sound_id: u64) -> Option<SoundboardMention> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundboardMention {
    pub name: String,
    pub emoji_name: Option<String>,
}

pub struct NoResolve;
impl MentionResolver for NoResolve {
    fn user(&self, _: u64) -> Option<String> {
        None
    }
    fn channel(&self, _: u64) -> Option<String> {
        None
    }
}

pub fn parse(src: &str, r: &dyn MentionResolver) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut rest = src;
    let mut para = String::new();
    let mut quote = String::new();
    let mut spoilers = 0usize;

    let flush = |buf: &mut String, quote: bool, blocks: &mut Vec<Block>, sp: &mut usize| {
        let t = buf.trim_end_matches('\n');
        if !t.is_empty() {
            let inline = parse_inline(t, r, sp);
            blocks.push(if quote { Block::Quote(inline) } else { Block::Paragraph(inline) });
        }
        buf.clear();
    };

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("```")
            && let Some(end) = after.find("```")
        {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            let body = &after[..end];
            let (lang, code) = match body.find('\n') {
                Some(nl)
                    if !body[..nl].trim().is_empty()
                        && !body[..nl].trim().contains(char::is_whitespace) =>
                {
                    (Some(body[..nl].trim().to_string()), &body[nl + 1..])
                }
                Some(0) => (None, &body[1..]),
                _ => (None, body),
            };
            blocks.push(Block::Code {
                lang,
                code: code.trim_end_matches('\n').to_string(),
            });
            rest = &after[end + 3..];
            rest = rest.strip_prefix('\n').unwrap_or(rest);
            continue;
        }
        let (line, next) = match rest.find('\n') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, ""),
        };
        if line.contains("```") && rest.find("```").is_some_and(|i| i < line.len()) {
            let i = rest.find("```").unwrap_or(0);
            if i > 0 {
                para.push_str(&rest[..i]);
                rest = &rest[i..];
                continue;
            }
        }
        if let Some(b) = line_block(line, r, &mut spoilers) {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            blocks.push(b);
        } else if let Some(q) = line.strip_prefix("> ").or_else(|| (line == ">").then_some("")) {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            quote.push_str(q);
            quote.push('\n');
        } else {
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            para.push_str(line);
            para.push('\n');
        }
        rest = next;
    }
    flush(&mut para, false, &mut blocks, &mut spoilers);
    flush(&mut quote, true, &mut blocks, &mut spoilers);
    blocks
}

mod link;

thread_local! {
    static EMOJI_LINKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn emoji_links() -> bool {
    EMOJI_LINKS.with(std::cell::Cell::get)
}

pub fn parse_emoji_links(src: &str, r: &dyn MentionResolver) -> Vec<Block> {
    let prev = EMOJI_LINKS.with(|e| e.replace(true));
    let out = parse(src, r);
    EMOJI_LINKS.with(|e| e.set(prev));
    out
}

pub fn parse_inline(src: &str, r: &dyn MentionResolver, spoiler_counter: &mut usize) -> Inline {
    let mut out = Inline::default();
    let mut ctx = Ctx {
        out: &mut out,
        r,
        spoilers: spoiler_counter,
    };
    ctx.run(src, Style::PLAIN, None);
    out
}

struct Ctx<'a> {
    out: &'a mut Inline,
    r: &'a dyn MentionResolver,
    spoilers: &'a mut usize,
}

const DELIMS: &[(&str, Style)] = &[
    ("**", Style::BOLD),
    ("__", Style::UNDERLINE),
    ("~~", Style::STRIKE),
    ("||", Style::SPOILER),
    ("*", Style::ITALIC),
    ("_", Style::ITALIC),
];

impl Ctx<'_> {
    fn push(&mut self, text: &str, style: Style, spoiler: Option<usize>) {
        self.push_full(text, style, None, None, spoiler)
    }

    fn push_full(
        &mut self,
        text: &str,
        style: Style,
        link: Option<String>,
        mention: Option<Mention>,
        spoiler: Option<usize>,
    ) {
        if text.is_empty() {
            return;
        }
        let start = self.out.text.len();
        self.out.text.push_str(text);
        let end = self.out.text.len();
        if let Some(last) = self.out.spans.last_mut()
            && last.style == style
            && last.link.is_none()
            && link.is_none()
            && last.mention.is_none()
            && mention.is_none()
            && last.spoiler == spoiler
            && last.range.end == start
        {
            last.range.end = end;
            return;
        }
        self.out.spans.push(Span {
            range: start..end,
            style,
            link,
            mention,
            spoiler,
        });
    }

    fn run(&mut self, src: &str, style: Style, spoiler: Option<usize>) {
        let bytes = src.as_bytes();
        let mut i = 0;
        let mut plain_start = 0;
        while i < bytes.len() {
            let rest = &src[i..];
            let c = bytes[i];

            if c == b'\\' && i + 1 < bytes.len() && !bytes[i + 1].is_ascii_alphanumeric() {
                self.push(&src[plain_start..i], style, spoiler);
                let ch_len = src[i + 1..].chars().next().map_or(1, char::len_utf8);
                self.push(&src[i + 1..i + 1 + ch_len], style, spoiler);
                i += 1 + ch_len;
                plain_start = i;
                continue;
            }

            if c == b'`' {
                let ticks = if rest.starts_with("``") { 2 } else { 1 };
                let close = &rest[..ticks];
                if let Some(end) = rest[ticks..].find(close)
                    && end > 0
                {
                    self.push(&src[plain_start..i], style, spoiler);
                    let inner = &rest[ticks..ticks + end];
                    self.push(inner, style | Style::CODE, spoiler);
                    i += ticks * 2 + end;
                    plain_start = i;
                    continue;
                }
            }

            if c == b'<'
                && let Some(end) = rest.find('>')
            {
                let inner = &rest[1..end];
                if let Some((guild, sound)) = inner
                    .strip_prefix("sound:")
                    .and_then(|ids| ids.split_once(':'))
                    .and_then(|(guild, sound)| Some((guild.parse::<u64>().ok()?, sound.parse::<u64>().ok()?)))
                    && let Some(sound) = self.r.soundboard(guild, sound)
                {
                    self.push(&src[plain_start..i], style, spoiler);
                    let mut label = String::new();
                    if let Some(emoji) = sound.emoji_name.filter(|e| !e.is_empty()) {
                        label.push_str(&emoji);
                        label.push(' ');
                    }
                    label.push_str(if sound.name.is_empty() { "Unknown" } else { &sound.name });
                    self.push_full(&label, style | Style::SOUNDBOARD, None, None, spoiler);
                    i += end + 1;
                    plain_start = i;
                    continue;
                }
                if let Some(t) = inner.strip_prefix("t:") {
                    let (num, st) = t.split_once(':').unwrap_or((t, "f"));
                    if let (Ok(ts), Some(stc)) = (num.parse::<i64>(), st.chars().next())
                        && st.len() == 1
                        && let Some(label) = format_timestamp(ts, stc)
                    {
                        self.push(&src[plain_start..i], style, spoiler);
                        self.push_full(&label, style | Style::TIMESTAMP, None, None, spoiler);
                        i += end + 1;
                        plain_start = i;
                        continue;
                    }
                }
                let parsed = if let Some(id) = inner.strip_prefix("@&") {
                    id.parse().ok().map(|id| {
                        let n = self.r.role(id).unwrap_or_else(|| "unknown-role".into());
                        (Mention::Role(id), format!("@{n}"))
                    })
                } else if let Some(id) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
                    id.parse().ok().map(|id| {
                        let n = self.r.user(id).unwrap_or_else(|| crate::t!("common.unknown_user").to_string());
                        (Mention::User(id), format!("@{n}"))
                    })
                } else if let Some(id) = inner.strip_prefix('#') {
                    id.parse().ok().map(|id| {
                        let n = self.r.channel(id).unwrap_or_else(|| crate::t!("common.unknown_lower").to_string());
                        (Mention::Channel(id), format!("#{n}"))
                    })
                } else {
                    None
                };
                if let Some((m, label)) = parsed {
                    self.push(&src[plain_start..i], style, spoiler);
                    self.push_full(&label, style | Style::MENTION, None, Some(m), spoiler);
                    i += end + 1;
                    plain_start = i;
                    continue;
                }
            }
            if c == b'@' {
                let (tok, m) = if rest.starts_with("@everyone") {
                    ("@everyone", Some(Mention::Everyone))
                } else if rest.starts_with("@here") {
                    ("@here", Some(Mention::Here))
                } else {
                    ("", None)
                };
                if let Some(m) = m {
                    self.push(&src[plain_start..i], style, spoiler);
                    self.push_full(tok, style | Style::MENTION, None, Some(m), spoiler);
                    i += tok.len();
                    plain_start = i;
                    continue;
                }
            }

            if c == b'['
                && let Some(m) = link::match_at(rest)
            {
                let label = &rest[m.label.clone()];
                let target = link::validate_url(&rest[m.url.clone()]).filter(|_| link::label_ok(label, emoji_links()));
                if let Some(url) = target {
                    self.push(&src[plain_start..i], style, spoiler);
                    let first = self.out.spans.len();
                    self.run(label, style | Style::LINK, spoiler);
                    for s in &mut self.out.spans[first..] {
                        s.link = Some(url.clone());
                    }
                    i += m.len;
                    plain_start = i;
                } else {
                    i += m.len;
                }
                continue;
            }

            if (c == b'h') && (rest.starts_with("https://") || rest.starts_with("http://")) {
                let at_boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                if at_boundary {
                    let end = rest
                        .find(|ch: char| ch.is_whitespace() || ch == '<' || ch == '>')
                        .unwrap_or(rest.len());
                    let mut url = &rest[..end];
                    while url.ends_with(['.', ',', ')', '!', '?', ':', ';']) {
                        url = &url[..url.len() - 1];
                    }
                    self.push(&src[plain_start..i], style, spoiler);
                    self.push_full(url, style | Style::LINK, Some(url.to_string()), None, spoiler);
                    i += url.len();
                    plain_start = i;
                    continue;
                }
            }

            let mut matched = false;
            for &(d, s) in DELIMS {
                if !rest.starts_with(d) {
                    continue;
                }
                if d == "_" && i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
                    continue;
                }
                let body = &rest[d.len()..];
                if body.starts_with(char::is_whitespace) && d.len() == 1 {
                    continue;
                }
                let Some(end) = find_close(body, d) else { continue };
                if end == 0 {
                    continue;
                }
                if d == "_" {
                    let after = body.as_bytes().get(end + 1);
                    if after.is_some_and(|b| b.is_ascii_alphanumeric()) {
                        continue;
                    }
                }
                self.push(&src[plain_start..i], style, spoiler);
                let sp = if s == Style::SPOILER {
                    let n = *self.spoilers;
                    *self.spoilers += 1;
                    Some(n)
                } else {
                    spoiler
                };
                self.run(&body[..end], style | s, sp);
                i += d.len() * 2 + end;
                plain_start = i;
                matched = true;
                break;
            }
            if matched {
                continue;
            }
            i += src[i..].chars().next().map_or(1, char::len_utf8);
        }
        self.push(&src[plain_start..], style, spoiler);
    }
}

fn find_close(body: &str, d: &str) -> Option<usize> {
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'`'
            && let Some(e) = body[i + 1..].find('`')
        {
            i += e + 2;
            continue;
        }
        if body[i..].starts_with(d) {
            if d == "*" && body[i..].starts_with("**") {
                i += 2;
                continue;
            }
            if d.len() == 2 && (d == "**" || d == "__") {
                let ch = d.as_bytes()[0];
                let run = b[i..].iter().take_while(|&&c| c == ch).count();
                return Some(i + run - 2);
            }
            return Some(i);
        }
        i += body[i..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

pub fn clickable(inline: &Inline) -> impl Iterator<Item = &Span> {
    inline
        .spans
        .iter()
        .filter(|s| s.link.is_some() || s.spoiler.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct R;
    impl MentionResolver for R {
        fn user(&self, id: u64) -> Option<String> {
            (id == 42).then(|| "wumpus".into())
        }
        fn channel(&self, id: u64) -> Option<String> {
            (id == 7).then(|| "general".into())
        }
        fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<SoundboardMention> {
            (guild_id == 9 && sound_id == 77).then(|| SoundboardMention {
                name: "Chime".into(),
                emoji_name: Some("🎵".into()),
            })
        }
    }

    #[test]
    fn triple_stars_are_bold_italic() {
        let i = inline("***both***");
        assert_eq!(i.text, "both");
        assert!(i.spans.iter().all(|s| s.style.has(Style::BOLD) && s.style.has(Style::ITALIC)));
        let u = inline("__*under*__");
        assert_eq!(u.text, "under");
    }

    fn inline(s: &str) -> Inline {
        let mut n = 0;
        parse_inline(s, &R, &mut n)
    }

    fn styled(i: &Inline) -> Vec<(&str, Style)> {
        i.spans.iter().map(|s| (&i.text[s.range.clone()], s.style)).collect()
    }

    #[test]
    fn plain_text() {
        let i = inline("hello world");
        assert_eq!(styled(&i), vec![("hello world", Style::PLAIN)]);
    }

    #[test]
    fn soundboard_mention_resolves_name_and_unicode_emoji() {
        let i = inline("try <sound:9:77> now");
        assert_eq!(i.text, "try 🎵 Chime now");
        let sound = i.spans.iter().find(|s| s.style.has(Style::SOUNDBOARD)).unwrap();
        assert_eq!(&i.text[sound.range.clone()], "🎵 Chime");
        assert!(sound.mention.is_none());
    }

    #[test]
    fn unresolved_soundboard_mention_stays_literal() {
        let i = inline("<sound:9:88>");
        assert_eq!(i.text, "<sound:9:88>");
        assert!(!i.spans.iter().any(|s| s.style.has(Style::SOUNDBOARD)));
    }

    #[test]
    fn bold_italic_nested() {
        let i = inline("a **b *c* d** e");
        assert_eq!(i.text, "a b c d e");
        assert_eq!(
            styled(&i),
            vec![
                ("a ", Style::PLAIN),
                ("b ", Style::BOLD),
                ("c", Style::BOLD | Style::ITALIC),
                (" d", Style::BOLD),
                (" e", Style::PLAIN),
            ]
        );
    }

    #[test]
    fn underline_strike_spoiler() {
        let i = inline("__u__ ~~s~~ ||x||");
        assert_eq!(i.text, "u s x");
        assert_eq!(i.spans[0].style, Style::UNDERLINE);
        assert_eq!(i.spans[2].style, Style::STRIKE);
        assert_eq!(i.spans[4].style, Style::SPOILER);
        assert_eq!(i.spans[4].spoiler, Some(0));
    }

    #[test]
    fn inline_code_is_literal() {
        let i = inline("run `**not bold**` now");
        assert_eq!(i.text, "run **not bold** now");
        assert_eq!(styled(&i)[1], ("**not bold**", Style::CODE));
    }

    #[test]
    fn snake_case_not_italic() {
        let i = inline("my_var_name and _it_");
        assert_eq!(i.text, "my_var_name and it");
        assert_eq!(styled(&i).last().unwrap(), &("it", Style::ITALIC));
    }

    #[test]
    fn unmatched_delimiters_stay() {
        let i = inline("2 * 3 = 6 and **open");
        assert_eq!(i.text, "2 * 3 = 6 and **open");
        assert_eq!(i.spans.len(), 1);
    }

    #[test]
    fn links_bare_and_masked() {
        let i = inline("see https://example.com/a, or [docs](https://docs.rs).");
        assert_eq!(i.text, "see https://example.com/a, or docs.");
        let links: Vec<_> = i.spans.iter().filter_map(|s| s.link.as_deref()).collect();
        assert_eq!(links, vec!["https://example.com/a", "https://docs.rs"]);
        assert_eq!(clickable(&i).count(), 2);
    }

    #[test]
    fn masked_link_label_keeps_markdown() {
        let i = inline("[**Burn** by **Usher**](https://x.y)");
        assert_eq!(i.text, "Burn by Usher");
        assert!(i.spans.iter().all(|s| s.link.as_deref() == Some("https://x.y")));
        assert!(i.spans.iter().any(|s| s.style.has(Style::BOLD)));
    }

    #[test]
    fn mentions() {
        let i = inline("hi <@42> in <#7>, <@!99> @everyone");
        assert_eq!(i.text, "hi @wumpus in #general, @Unknown user @everyone");
        let m: Vec<_> = i.spans.iter().filter_map(|s| s.mention.clone()).collect();
        assert_eq!(
            m,
            vec![Mention::User(42), Mention::Channel(7), Mention::User(99), Mention::Everyone]
        );
    }

    #[test]
    fn escapes() {
        let i = inline(r"\*not italic\*");
        assert_eq!(i.text, "*not italic*");
        assert_eq!(i.spans.len(), 1);
    }

    #[test]
    fn code_block_with_lang() {
        let b = parse("before\n```rust\nfn main() {}\n```\nafter", &R);
        assert_eq!(b.len(), 3);
        assert!(matches!(&b[0], Block::Paragraph(i) if i.text == "before"));
        assert_eq!(
            b[1],
            Block::Code { lang: Some("rust".into()), code: "fn main() {}".into() }
        );
        assert!(matches!(&b[2], Block::Paragraph(i) if i.text == "after"));
    }

    #[test]
    fn code_block_inline_fence() {
        let b = parse("```just code```", &R);
        assert_eq!(b, vec![Block::Code { lang: None, code: "just code".into() }]);
    }

    #[test]
    fn quotes() {
        let b = parse("> quoted **line**\n> two\nnormal", &R);
        assert_eq!(b.len(), 2);
        match &b[0] {
            Block::Quote(i) => {
                assert_eq!(i.text, "quoted line\ntwo");
                assert!(i.spans.iter().any(|s| s.style == Style::BOLD));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn multiple_spoilers_are_numbered() {
        let b = parse("||a|| and ||b||", &NoResolve);
        let Block::Paragraph(i) = &b[0] else { panic!() };
        let ids: Vec<_> = i.spans.iter().filter_map(|s| s.spoiler).collect();
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn unicode_safe() {
        let i = inline("héllo **wörld** 🎉 _ok_");
        assert_eq!(i.text, "héllo wörld 🎉 ok");
    }

    #[test]
    fn headings_lists_subtext() {
        let b = parse("# Big\n-# small\n- one\n  - two\n3. three\nplain", &NoResolve);
        assert!(matches!(&b[0], Block::Heading(1, i) if i.text == "Big"));
        assert!(matches!(&b[1], Block::Subtext(i) if i.text == "small"));
        assert!(matches!(&b[2], Block::ListItem { ordered: None, depth: 0, inline } if inline.text == "one"));
        assert!(matches!(&b[3], Block::ListItem { ordered: None, depth: 1, .. }));
        assert!(matches!(&b[4], Block::ListItem { ordered: Some(3), .. }));
        assert!(matches!(&b[5], Block::Paragraph(i) if i.text == "plain"));
    }

    #[test]
    fn timestamps_and_masked_links() {
        let i = inline("at <t:0:D> see [x](<https://a.b>)");
        assert!(i.spans.iter().any(|s| s.style.has(Style::TIMESTAMP)));
        assert!(i.spans.iter().any(|s| s.link.as_deref() == Some("https://a.b")));
        assert_eq!(relative(-120), "2 minutes ago");
        assert_eq!(relative(3600), "in an hour");
    }

    #[test]
    fn masked_link_takes_the_nearest_bracket() {
        let src = "[Test Phoenix] envoyé depuis le client officiel — surlignage de recherche : phxsurligne **phxsurligne** *phxsurligne* __phxsurligne__ ~~phxsurligne~~ [phxsurligne](https://example.com/a) `phxsurligne` ||phxsurligne|| <#7> fire 🔥 https://example.com/phxsurligne";
        let i = inline(src);
        assert!(i.text.starts_with("[Test Phoenix] envoyé"), "{}", i.text);
        assert_eq!(
            i.text,
            "[Test Phoenix] envoyé depuis le client officiel — surlignage de recherche : phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne #general fire 🔥 https://example.com/phxsurligne"
        );
        let first = &i.spans[0];
        assert!(first.link.is_none() && !first.style.has(Style::LINK));
        let links: Vec<_> = i.spans.iter().filter_map(|s| s.link.as_deref().map(|l| (&i.text[s.range.clone()], l))).collect();
        assert_eq!(links, vec![("phxsurligne", "https://example.com/a"), ("https://example.com/phxsurligne", "https://example.com/phxsurligne")]);
        let code: Vec<_> = styled(&i).into_iter().filter(|(_, st)| st.has(Style::CODE)).map(|(t, _)| t).collect();
        assert_eq!(code, vec!["phxsurligne"]);
        let at = i.text.find("phxsurligne phxsurligne phxsurligne #").unwrap();
        assert!(i.spans.iter().any(|s| s.range.start == at && s.link.is_some()));
    }

    #[test]
    fn rejected_masked_links_stay_literal() {
        for src in ["[a <@42>](https://x.y)", "[www.evil.com](https://x.y)", "[go **now**](javascript:alert(1))", "[🔥](https://x.y)", "[ ](https://x.y)"] {
            let i = inline(src);
            assert_eq!(i.text, src);
            assert_eq!(styled(&i), vec![(src, Style::PLAIN)], "{src}");
        }
        let b = parse_emoji_links("[🔥 hot](https://x.y)", &R);
        let i = b[0].inline().unwrap();
        assert!(i.spans.iter().all(|s| s.link.as_deref() == Some("https://x.y")));
        let i = inline("[a [b] c](https://x.y/w_(b)) d");
        assert_eq!(i.text, "a [b] c d");
        assert_eq!(i.spans[0].link.as_deref(), Some("https://x.y/w_(b)"));
        let i = inline("[a\nb](https://x.y)");
        assert!(i.spans.iter().all(|s| s.link.is_some()));
    }
}
