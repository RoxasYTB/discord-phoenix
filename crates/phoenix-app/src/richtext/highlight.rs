use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    String,
    Number,
    Comment,
    Title,
    Literal,
    Meta,
}

impl Tok {
    pub fn color(self) -> u32 {
        use crate::theme::{generated::idx, tok};
        tok(match self {
            Tok::Plain => idx::TEXT_CODE,
            Tok::Keyword => idx::TEXT_CODE_KEYWORD,
            Tok::String => idx::TEXT_CODE_STRING,
            Tok::Number => idx::TEXT_CODE_NUMBER,
            Tok::Comment => idx::TEXT_CODE_COMMENT,
            Tok::Title => idx::TEXT_CODE_TITLE,
            Tok::Literal => idx::TEXT_CODE_VARIABLE,
            Tok::Meta => idx::TEXT_CODE_DECORATOR,
        })
    }
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "crate", "def", "default",
    "defer", "del", "do", "dyn", "elif", "else", "enum", "except", "export", "extends", "extern", "finally",
    "fn", "for", "from", "func", "function", "go", "if", "impl", "import", "in", "interface", "is", "lambda",
    "let", "local", "loop", "match", "mod", "move", "mut", "new", "not", "and", "or", "package", "pass", "pub",
    "raise", "ref", "return", "self", "Self", "static", "struct", "super", "switch", "then", "end", "throw",
    "trait", "try", "type", "typeof", "unsafe", "use", "var", "void", "where", "while", "with", "yield",
    "public", "private", "protected", "final", "abstract", "namespace", "using", "template", "typename",
    "select", "insert", "update", "delete", "where", "join", "into", "values", "create", "table", "echo",
    "fi", "done", "esac", "int", "char", "float", "double", "bool", "long", "short", "unsigned", "auto",
    "SELECT", "FROM", "WHERE", "INSERT", "UPDATE", "DELETE", "JOIN", "INTO", "VALUES", "CREATE", "TABLE",
];
const LITERALS: &[&str] = &["true", "false", "null", "None", "True", "False", "nil", "undefined", "NULL"];

fn hash_comments(lang: &str) -> bool {
    matches!(lang, "py" | "python" | "sh" | "bash" | "shell" | "zsh" | "rb" | "ruby" | "yaml" | "yml" | "toml" | "ini" | "conf" | "r" | "perl" | "nix")
}

pub fn highlight(code: &str, lang: Option<&str>) -> Vec<(Range<usize>, Tok)> {
    let lang = lang.unwrap_or("").to_ascii_lowercase();
    let hash = hash_comments(&lang);
    let dash = matches!(lang.as_str(), "sql" | "lua" | "hs" | "haskell");
    let b = code.as_bytes();
    let mut out: Vec<(Range<usize>, Tok)> = Vec::new();
    let mut push = |r: Range<usize>, t: Tok| {
        if r.is_empty() {
            return;
        }
        if let Some(last) = out.last_mut()
            && last.1 == t
            && last.0.end == r.start
        {
            last.0.end = r.end;
            return;
        }
        out.push((r, t));
    };
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let rest = &code[i..];
        let line_end = |from: usize| code[from..].find('\n').map_or(code.len(), |e| from + e);
        if rest.starts_with("//") && !hash || (hash && c == b'#') || (dash && rest.starts_with("--")) {
            let e = line_end(i);
            push(i..e, Tok::Comment);
            i = e;
            continue;
        }
        if rest.starts_with("/*") {
            let e = code[i + 2..].find("*/").map_or(code.len(), |e| i + 2 + e + 2);
            push(i..e, Tok::Comment);
            i = e;
            continue;
        }
        if c == b'"' || c == b'\'' || c == b'`' {
            let mut j = i + 1;
            let mut closed = false;
            while j < b.len() {
                if b[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if b[j] == c {
                    closed = true;
                    j += 1;
                    break;
                }
                if b[j] == b'\n' && c != b'`' {
                    break;
                }
                j += 1;
            }
            let j = j.min(b.len());
            if closed || c == b'"' {
                push(i..j, Tok::String);
                i = j;
            } else {
                push(i..i + 1, Tok::Plain);
                i += 1;
            }
            continue;
        }
        if c.is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') {
                j += 1;
            }
            push(i..j, Tok::Number);
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'@' {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            let w = &code[i..j];
            let next = code[j..].trim_start_matches(' ').as_bytes().first().copied();
            let t = if c == b'@' {
                Tok::Meta
            } else if KEYWORDS.contains(&w) {
                Tok::Keyword
            } else if LITERALS.contains(&w) {
                Tok::Literal
            } else if next == Some(b'(') || next == Some(b'!') || (w.as_bytes()[0].is_ascii_uppercase() && w.len() > 1) {
                Tok::Title
            } else {
                Tok::Plain
            };
            push(i..j, t);
            i = j;
            continue;
        }
        let len = rest.chars().next().map_or(1, char::len_utf8);
        push(i..i + len, Tok::Plain);
        i += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn covers_everything() {
        let src = "fn main() { let x = \"hi\"; // c\n 42 }";
        let h = highlight(src, Some("rust"));
        assert_eq!(h.iter().map(|(r, _)| r.len()).sum::<usize>(), src.len());
        assert!(h.iter().any(|(r, t)| *t == Tok::Keyword && &src[r.clone()] == "fn"));
        assert!(h.iter().any(|(r, t)| *t == Tok::Title && &src[r.clone()] == "main"));
        assert!(h.iter().any(|(r, t)| *t == Tok::String && &src[r.clone()] == "\"hi\""));
        assert!(h.iter().any(|(_, t)| *t == Tok::Comment));
    }
    #[test]
    fn python_hash_comment() {
        let h = highlight("x = 1 # note", Some("py"));
        assert_eq!(h.last().unwrap().1, Tok::Comment);
    }
}
