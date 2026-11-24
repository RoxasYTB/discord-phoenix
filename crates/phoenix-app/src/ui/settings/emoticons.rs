const TABLE: &[(&[&str], &str)] = &[
    (&[">:(", ">:-(", ">=(", ">=-("], "😠"),
    (&[":\")", ":-\")", "=\")", "=-\")"], "😊"),
    (&["</3", "<\\3"], "💔"),
    (&[":-\\", ":-/", "=-\\", "=-/"], "😕"),
    (&[":'(", ":'-(", ":,(", ":,-(", "='(", "='-(", "=,(", "=,-("], "😢"),
    (&[":(", ":-(", "=(", "=-("], "🙁"),
    (&["<3", "♡"], "❤️"),
    (&["]:(", "]:-(", "]=(", "]=-("], "👿"),
    (&["o:)", "O:)", "o:-)", "O:-)", "0:)", "0:-)", "o=)", "O=)", "o=-)", "O=-)", "0=)", "0=-)"], "😇"),
    (&[":'D", ":'-D", ":,D", ":,-D", "='D", "='-D", "=,D", "=,-D"], "😂"),
    (&[":*", ":-*", "=*", "=-*"], "😗"),
    (&["x-)", "X-)"], "😆"),
    (&[":|", ":-|", "=|", "=-|"], "😐"),
    (&[":o", ":-o", ":O", ":-O", "=o", "=-o", "=O", "=-O"], "😮"),
    (&[":@", ":-@", "=@", "=-@"], "😡"),
    (&[":D", ":-D", "=D", "=-D"], "😄"),
    (&[":')", ":'-)", ":,)", ":,-)", "=')", "='-)", "=,)", "=,-)"], "🥲"),
    (&[":)", ":-)", "=)", "=-)"], "🙂"),
    (&["]:)", "]:-)", "]=)", "]=-)"], "😈"),
    (&[":,'(", ":,'-(", ";(", ";-(", "=,'(", "=,'-("], "😭"),
    (&[":P", ":-P", "=P", "=-P"], "😛"),
    (&["8-)", "B-)"], "😎"),
    (&[",:(", ",:-(", ",=(", ",=-("], "😓"),
    (&[",:)", ",:-)", ",=)", ",=-)"], "😅"),
    (&[":s", ":-S", ":z", ":-Z", ":$", ":-$", "=s", "=-S", "=z", "=-Z", "=$", "=-$"], "😒"),
    (&[";)", ";-)"], "😉"),
];

fn lookup(tok: &str) -> Option<&'static str> {
    TABLE.iter().find(|(e, _)| e.contains(&tok)).map(|(_, g)| *g)
}

pub fn convert(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut ticks = 0usize;
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String, in_code: bool| {
        match if in_code { None } else { lookup(word) } {
            Some(g) => out.push_str(g),
            None => out.push_str(word),
        }
        word.clear();
    };
    for ch in text.chars() {
        if ch.is_whitespace() {
            let in_code = ticks % 2 == 1 || word.contains('`');
            flush(&mut word, &mut out, in_code);
            out.push(ch);
        } else {
            if ch == '`' {
                ticks += 1;
            }
            word.push(ch);
        }
    }
    let in_code = ticks % 2 == 1 || word.contains('`');
    flush(&mut word, &mut out, in_code);
    out
}

#[cfg(test)]
mod tests {
    use super::convert;

    #[test]
    fn converts_words_only() {
        assert_eq!(convert("hi :) there"), "hi 🙂 there");
        assert_eq!(convert(":D"), "😄");
        assert_eq!(convert("a:)b <3"), "a:)b ❤️");
        assert_eq!(convert("`:)` and ``` :( ```"), "`:)` and ``` :( ```");
        assert_eq!(convert("line\n;)"), "line\n😉");
    }
}
