use serde_json::{Value, json};

pub const SUB: u64 = 1;
pub const GROUP: u64 = 2;
pub const STRING: u64 = 3;
pub const INTEGER: u64 = 4;
pub const BOOLEAN: u64 = 5;
pub const USER: u64 = 6;
pub const CHANNEL: u64 = 7;
pub const ROLE: u64 = 8;
pub const MENTIONABLE: u64 = 9;
pub const NUMBER: u64 = 10;
pub const ATTACHMENT: u64 = 11;

pub fn options(schema: &Value) -> &[Value] {
    schema.get("options").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

pub fn kind(o: &Value) -> u64 {
    o.get("type").and_then(Value::as_u64).unwrap_or(STRING)
}

pub fn name(o: &Value) -> &str {
    o.get("name").and_then(Value::as_str).unwrap_or("")
}

pub fn description(o: &Value) -> &str {
    o.get("description").and_then(Value::as_str).unwrap_or("")
}

pub fn required(o: &Value) -> bool {
    o.get("required").and_then(Value::as_bool).unwrap_or(false)
}

pub fn autocomplete(o: &Value) -> bool {
    o.get("autocomplete").and_then(Value::as_bool).unwrap_or(false)
}

pub fn choices(o: &Value) -> Option<&[Value]> {
    o.get("choices").and_then(Value::as_array).map(Vec::as_slice).filter(|c| !c.is_empty())
}

pub fn choice_name(c: &Value) -> &str {
    c.get("name_localized").and_then(Value::as_str).or_else(|| c.get("name").and_then(Value::as_str)).unwrap_or("")
}

pub fn leaves(cmd: &Value) -> Vec<(Vec<(u64, String)>, Value)> {
    let opts = options(cmd);
    if !opts.iter().any(|o| matches!(kind(o), SUB | GROUP)) {
        return vec![(Vec::new(), cmd.clone())];
    }
    let mut out = Vec::new();
    for o in opts {
        match kind(o) {
            SUB => out.push((vec![(SUB, name(o).to_owned())], o.clone())),
            GROUP => {
                for s in options(o).iter().filter(|s| kind(s) == SUB) {
                    out.push((vec![(GROUP, name(o).to_owned()), (SUB, name(s).to_owned())], s.clone()));
                }
            }
            _ => {}
        }
    }
    out
}

pub fn display_name(cmd: &Value, path: &[(u64, String)]) -> String {
    let mut s = name(cmd).to_owned();
    for (_, p) in path {
        s.push(' ');
        s.push_str(p);
    }
    s
}

pub fn is_key(k: &str) -> bool {
    !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

pub fn pairs(args: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for word in args.split(' ') {
        match word.split_once(':') {
            Some((k, v)) if is_key(k) => out.push((k.to_lowercase(), v.to_owned())),
            _ => match out.last_mut() {
                Some((_, v)) => {
                    v.push(' ');
                    v.push_str(word)
                }
                None if word.is_empty() => {}
                None => out.push((String::new(), word.to_owned())),
            },
        }
    }
    for (_, v) in &mut out {
        *v = v.trim().to_owned();
    }
    out
}

pub fn key_spans(args: &str, offset: usize) -> Vec<(std::ops::Range<usize>, std::ops::Range<usize>, String)> {
    let mut out: Vec<(std::ops::Range<usize>, std::ops::Range<usize>, String)> = Vec::new();
    let mut at = 0;
    for word in args.split(' ') {
        if let Some((k, _)) = word.split_once(':')
            && is_key(k)
        {
            let start = offset + at;
            if let Some(last) = out.last_mut() {
                last.1.end = (start - 1).max(last.1.start);
            }
            out.push((start..start + k.len() + 1, start + k.len() + 1..offset + args.len(), k.to_lowercase()));
        }
        at += word.len() + 1;
    }
    out
}

fn snowflake(v: &str) -> Option<String> {
    let s = v.trim().trim_start_matches('<').trim_end_matches('>').trim_start_matches(['@', '#', '&', '!']);
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.to_owned())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptError {
    pub option: String,
    pub message: String,
}

pub fn invalid_message(o: &Value) -> String {
    if choices(o).is_some() {
        return crate::t!("slash.err_choice").to_string();
    }
    match kind(o) {
        BOOLEAN => crate::t!("slash.err_bool"),
        CHANNEL => crate::t!("slash.err_channel"),
        INTEGER => crate::t!("slash.err_int"),
        NUMBER => crate::t!("slash.err_number"),
        ROLE => crate::t!("slash.err_role"),
        USER => crate::t!("slash.err_user"),
        MENTIONABLE => crate::t!("slash.err_mentionable"),
        _ => crate::t!("slash.err_value"),
    }
    .to_string()
}

fn num_text(v: f64) -> String {
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

fn range_error(o: &Value, v: f64) -> Option<String> {
    let min = o.get("min_value").and_then(Value::as_f64);
    let max = o.get("max_value").and_then(Value::as_f64);
    let low = min.is_some_and(|m| v < m);
    let high = max.is_some_and(|m| v > m);
    if !low && !high {
        return None;
    }
    Some(
        match (min, max) {
            (Some(a), Some(b)) => crate::t!("slash.err_num_range", min = num_text(a), max = num_text(b)),
            (Some(a), None) => crate::t!("slash.err_num_min", min = num_text(a)),
            (None, Some(b)) => crate::t!("slash.err_num_max", max = num_text(b)),
            _ => unreachable!(),
        }
        .to_string(),
    )
}

fn length_error(o: &Value, v: &str) -> Option<String> {
    let min = o.get("min_length").and_then(Value::as_u64);
    let max = o.get("max_length").and_then(Value::as_u64);
    let n = v.chars().count() as u64;
    if !min.is_some_and(|m| n < m) && !max.is_some_and(|m| n > m) {
        return None;
    }
    Some(
        match (min, max) {
            (Some(a), Some(b)) if a == b => crate::t!("slash.err_len_exact", n = a),
            (Some(a), Some(b)) => crate::t!("slash.err_len_range", min = a, max = b),
            (Some(a), None) => crate::t!("slash.err_len_min", min = a),
            (None, Some(b)) => crate::t!("slash.err_len_max", max = b),
            _ => unreachable!(),
        }
        .to_string(),
    )
}

fn choice_value(o: &Value, raw: &str) -> Option<Value> {
    let cs = choices(o)?;
    let t = raw.trim();
    cs.iter()
        .find(|c| choice_name(c) == t || c.get("name").and_then(Value::as_str) == Some(t))
        .or_else(|| cs.iter().find(|c| c.get("value").map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())).as_deref() == Some(t)))
        .and_then(|c| c.get("value").cloned())
}

pub fn typed(o: &Value, raw: &str, auto: &[(String, Value)]) -> Result<Value, String> {
    let t = raw.trim();
    if choices(o).is_some() {
        return choice_value(o, t).ok_or_else(|| invalid_message(o));
    }
    if autocomplete(o)
        && let Some((_, v)) = auto.iter().find(|(n, _)| n == t)
    {
        return Ok(v.clone());
    }
    let bad = || invalid_message(o);
    Ok(match kind(o) {
        INTEGER => {
            let n: i64 = t.parse().map_err(|_| bad())?;
            if let Some(e) = range_error(o, n as f64) {
                return Err(e);
            }
            json!(n)
        }
        NUMBER => {
            let n: f64 = t.replace(',', ".").parse().map_err(|_| bad())?;
            if !n.is_finite() {
                return Err(bad());
            }
            if let Some(e) = range_error(o, n) {
                return Err(e);
            }
            json!(n)
        }
        BOOLEAN => match t.to_lowercase().as_str() {
            "true" => json!(true),
            "false" => json!(false),
            _ => return Err(bad()),
        },
        USER => {
            if t.starts_with("<@&") {
                return Err(bad());
            }
            json!(snowflake(t).ok_or_else(bad)?)
        }
        CHANNEL => {
            if !t.starts_with("<#") && !t.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            json!(snowflake(t).ok_or_else(bad)?)
        }
        ROLE => {
            if t.starts_with("<@") && !t.starts_with("<@&") {
                return Err(bad());
            }
            json!(snowflake(t).ok_or_else(bad)?)
        }
        MENTIONABLE => json!(snowflake(t).ok_or_else(bad)?),
        ATTACHMENT => json!(t),
        _ => {
            if let Some(e) = length_error(o, t) {
                return Err(e);
            }
            json!(t)
        }
    })
}

pub fn leaf_values(leaf: &Value, args: &str, auto: &[(String, Value)]) -> Result<Vec<Value>, OptError> {
    let opts = options(leaf);
    let mut given = pairs(args);
    if let Some((k, _)) = given.first_mut()
        && k.is_empty()
    {
        match opts.iter().find(|o| required(o)).or(opts.first()) {
            Some(o) => *k = name(o).to_owned(),
            None => return Err(OptError { option: String::new(), message: crate::t!("slash.no_options").into() }),
        }
    }
    let mut out = Vec::new();
    for o in opts {
        match given.iter().position(|(k, _)| k == name(o)) {
            Some(i) => {
                let (_, v) = given.remove(i);
                if v.trim().is_empty() {
                    let message = if required(o) { crate::t!("slash.err_required").to_string() } else { invalid_message(o) };
                    return Err(OptError { option: name(o).to_owned(), message });
                }
                let value = typed(o, &v, auto).map_err(|message| OptError { option: name(o).to_owned(), message })?;
                out.push(json!({ "type": kind(o), "name": name(o), "value": value }));
            }
            None if required(o) => {
                return Err(OptError { option: name(o).to_owned(), message: crate::t!("slash.err_required").into() });
            }
            None => {}
        }
    }
    if let Some((k, _)) = given.first() {
        return Err(OptError { option: k.clone(), message: crate::t!("slash.err_unknown", name = k).into() });
    }
    Ok(out)
}

pub fn wrap(path: &[(u64, String)], leaf: Vec<Value>) -> Value {
    let mut v = Value::Array(leaf);
    for (k, n) in path.iter().rev() {
        v = json!([{ "type": k, "name": n, "options": v }]);
    }
    v
}

pub fn build_options(schema: &Value, args: &str) -> Result<Value, String> {
    let rest = args.trim();
    let leaves = leaves(schema);
    if leaves.len() == 1 && leaves[0].0.is_empty() {
        return leaf_values(&leaves[0].1, rest, &[]).map(Value::Array).map_err(|e| e.message);
    }
    let words: Vec<&str> = rest.split_whitespace().collect();
    for (path, leaf) in &leaves {
        if words.len() >= path.len() && path.iter().zip(&words).all(|((_, p), w)| p == w) {
            let skip: usize = words[..path.len()].iter().map(|w| w.len()).sum::<usize>() + path.len();
            let tail = rest.get(skip.min(rest.len())..).unwrap_or("");
            return leaf_values(leaf, tail, &[]).map(|v| wrap(path, v)).map_err(|e| e.message);
        }
    }
    let names: Vec<String> = leaves.iter().map(|(p, _)| p.iter().map(|x| x.1.as_str()).collect::<Vec<_>>().join(" ")).collect();
    Err(format!("{}: {}", crate::t!("slash.pick_sub"), names.join(", ")))
}

pub fn split_invocation(text: &str) -> Option<(&str, &str)> {
    let t = text.strip_prefix('/')?;
    let (n, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    (!n.is_empty()).then_some((n, rest))
}

pub fn match_command<'a, T>(text: &'a str, cmds: &'a [T], display: impl Fn(&T) -> &str) -> Option<(&'a T, &'a str)> {
    let t = text.strip_prefix('/')?;
    let mut best: Option<(&T, &str)> = None;
    for c in cmds {
        let d = display(c);
        if let Some(rest) = t.strip_prefix(d)
            && (rest.is_empty() || rest.starts_with(' '))
            && best.is_none_or(|(b, _)| display(b).len() < d.len())
        {
            best = Some((c, rest.strip_prefix(' ').unwrap_or(rest)));
        }
    }
    best
}

pub fn option_at(args: &str, cursor: usize) -> Option<(String, String)> {
    let before = args.get(..cursor.min(args.len()))?;
    let mut cur: Option<(String, usize)> = None;
    let mut at = 0;
    for word in before.split(' ') {
        if let Some((k, _)) = word.split_once(':')
            && is_key(k)
        {
            cur = Some((k.to_lowercase(), at + k.len() + 1));
        }
        at += word.len() + 1;
    }
    let (k, start) = cur?;
    Some((k, before.get(start..).unwrap_or("").to_owned()))
}

pub fn score(name: &str, description: &str, opts: &[Value], q: &str) -> Option<u8> {
    let n = name.to_lowercase();
    let q = q.to_lowercase();
    if n.starts_with(&q) {
        return Some(0);
    }
    let (first, rest) = q.split_once(' ').unwrap_or((q.as_str(), ""));
    if n.starts_with(first) && n.split(' ').skip(1).collect::<Vec<_>>().join(" ").starts_with(rest) && q.contains(' ') {
        return Some(1);
    }
    if n.contains(&q) {
        return Some(2);
    }
    let mut partial = false;
    for o in opts {
        let on = name_lc(o);
        if on.starts_with(&q) || format!("{n} {on}").starts_with(&q) {
            return Some(3);
        }
        partial |= on.contains(&q);
    }
    if partial {
        return Some(4);
    }
    if description.to_lowercase().contains(&q) {
        return Some(7);
    }
    None
}

fn name_lc(o: &Value) -> String {
    name(o).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        json!({"name":"ban","options":[
            {"type":6,"name":"user","required":true},
            {"type":3,"name":"reason"},
            {"type":4,"name":"days","min_value":0,"max_value":7}]})
    }

    #[test]
    fn flat_options() {
        let o = build_options(&schema(), "user:<@!42> reason:being very rude days:3").unwrap();
        assert_eq!(
            o,
            json!([
            {"type":6,"name":"user","value":"42"},
            {"type":3,"name":"reason","value":"being very rude"},
            {"type":4,"name":"days","value":3}])
        );
        assert_eq!(build_options(&schema(), "<@7>").unwrap()[0]["value"], "7");
        assert!(build_options(&schema(), "reason:x").is_err());
        assert!(build_options(&schema(), "user:1 days:x").is_err());
        assert!(build_options(&schema(), "user:1 days:9").is_err());
        assert!(build_options(&schema(), "user:<@&5>").is_err());
        assert!(build_options(&schema(), "user:1 colour:red").is_err());
    }

    #[test]
    fn subcommands_and_groups() {
        let s = json!({"name":"list","options":[
            {"type":1,"name":"add","options":[{"type":3,"name":"item","required":true}]},
            {"type":2,"name":"cfg","options":[{"type":1,"name":"set","options":[{"type":5,"name":"on","required":true}]}]}]});
        assert_eq!(build_options(&s, "add item:milk").unwrap(), json!([{"type":1,"name":"add","options":[{"type":3,"name":"item","value":"milk"}]}]));
        assert_eq!(
            build_options(&s, "cfg set on:True").unwrap(),
            json!([{"type":2,"name":"cfg","options":[{"type":1,"name":"set","options":[{"type":5,"name":"on","value":true}]}]}])
        );
        assert!(build_options(&s, "").is_err());
        let l = leaves(&s);
        assert_eq!(l.iter().map(|(p, _)| display_name(&s, p)).collect::<Vec<_>>(), ["list add", "list cfg set"]);
        assert_eq!(split_invocation("/ping"), Some(("ping", "")));
        assert_eq!(split_invocation("hi"), None);
    }

    #[test]
    fn choices_map_names_to_values() {
        let o = json!({"type":4,"name":"d","choices":[{"name":"Previous Hour","value":3600}]});
        assert_eq!(typed(&o, "Previous Hour", &[]), Ok(json!(3600)));
        assert!(typed(&o, "nope", &[]).is_err());
        let s = json!({"type":3,"name":"q","max_length":3});
        assert!(typed(&s, "abcd", &[]).is_err());
        let a = json!({"type":3,"name":"c","autocomplete":true});
        assert_eq!(typed(&a, "Help", &[("Help".into(), json!("help"))]), Ok(json!("help")));
    }

    #[test]
    fn longest_command_wins() {
        let cmds = ["info", "info activity", "id"];
        assert_eq!(match_command("/info activity user:1", &cmds, |c| c), Some((&"info activity", "user:1")));
        assert_eq!(match_command("/info", &cmds, |c| c), Some((&"info", "")));
        assert_eq!(match_command("/inf", &cmds, |c| c), None);
        assert_eq!(option_at("member:<@1> role:ad", 19), Some(("role".into(), "ad".into())));
        assert_eq!(option_at("member:", 7), Some(("member".into(), "".into())));
        assert_eq!(option_at("", 0), None);
    }

    #[test]
    fn scores_like_the_client() {
        assert_eq!(score("info", "", &[], "i"), Some(0));
        assert_eq!(score("ping", "pong", &[], "on"), Some(7));
        assert_eq!(score("xping", "", &[], "pi"), Some(2));
        assert_eq!(score("id", "", &[json!({"name":"member"})], "mem"), Some(3));
        assert_eq!(score("id", "", &[], "zz"), None);
    }

    #[test]
    fn key_spans_cover_values() {
        let s = key_spans("member:<@1> role:x", 4);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].0, 4..11);
        assert_eq!(s[0].2, "member");
        assert_eq!(s[1].0, 16..21);
        assert_eq!(s[1].1, 21..22);
    }
}
