use serde_json::{Value, json};

const SUB: u64 = 1;
const GROUP: u64 = 2;

fn options(schema: &Value) -> &[Value] {
    schema.get("options").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn kind(o: &Value) -> u64 {
    o.get("type").and_then(Value::as_u64).unwrap_or(3)
}

fn name(o: &Value) -> &str {
    o.get("name").and_then(Value::as_str).unwrap_or("")
}

fn pairs(args: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for word in args.split_whitespace() {
        match word.split_once(':') {
            Some((k, v)) if !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') => {
                out.push((k.to_lowercase(), v.to_owned()))
            }
            _ => match out.last_mut() {
                Some((_, v)) => {
                    if !v.is_empty() {
                        v.push(' ');
                    }
                    v.push_str(word)
                }
                None => out.push((String::new(), word.to_owned())),
            },
        }
    }
    out
}

fn snowflake(v: &str) -> String {
    v.trim_start_matches('<').trim_end_matches('>').trim_start_matches(['@', '#', '&', '!']).to_owned()
}

fn typed(o: &Value, raw: &str) -> Result<Value, String> {
    let n = name(o);
    Ok(match kind(o) {
        4 => json!(raw.parse::<i64>().map_err(|_| format!("{n} must be a whole number"))?),
        10 => json!(raw.parse::<f64>().map_err(|_| format!("{n} must be a number"))?),
        5 => json!(matches!(raw.to_lowercase().as_str(), "true" | "yes" | "1" | "on")),
        6..=9 => json!(snowflake(raw)),
        _ => json!(raw),
    })
}

pub fn build_options(schema: &Value, args: &str) -> Result<Value, String> {
    let opts = options(schema);
    let mut rest = args.trim();
    if let Some(first) = rest.split_whitespace().next()
        && let Some(sub) = opts.iter().find(|o| matches!(kind(o), SUB | GROUP) && name(o) == first)
    {
        rest = rest[first.len()..].trim_start();
        return Ok(json!([{ "type": kind(sub), "name": name(sub), "options": build_options(sub, rest)? }]));
    }
    if opts.iter().any(|o| matches!(kind(o), SUB | GROUP)) {
        let names: Vec<_> = opts.iter().map(name).collect();
        return Err(format!("Pick a sub-command: {}", names.join(", ")));
    }
    let mut out = Vec::new();
    let mut given = pairs(rest);
    if let Some((k, _)) = given.first_mut()
        && k.is_empty()
    {
        match opts.first() {
            Some(o) => *k = name(o).to_owned(),
            None => return Err(crate::t!("slash.no_options").into()),
        }
    }
    for o in opts {
        match given.iter().position(|(k, _)| k == name(o)) {
            Some(i) => {
                let (_, v) = given.remove(i);
                out.push(json!({ "type": kind(o), "name": name(o), "value": typed(o, &v)? }));
            }
            None if o.get("required").and_then(Value::as_bool).unwrap_or(false) => {
                return Err(format!("Missing required option {}", name(o)));
            }
            None => {}
        }
    }
    if let Some((k, _)) = given.first() {
        return Err(format!("Unknown option {k}"));
    }
    Ok(Value::Array(out))
}

pub fn split_invocation(text: &str) -> Option<(&str, &str)> {
    let t = text.strip_prefix('/')?;
    let (n, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    (!n.is_empty()).then_some((n, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        json!({"name":"ban","options":[
            {"type":6,"name":"user","required":true},
            {"type":3,"name":"reason"},
            {"type":4,"name":"days"}]})
    }

    #[test]
    fn flat_options() {
        let o = build_options(&schema(), "user:<@!42> reason:being very rude days:3").unwrap();
        assert_eq!(o, json!([
            {"type":6,"name":"user","value":"42"},
            {"type":3,"name":"reason","value":"being very rude"},
            {"type":4,"name":"days","value":3}]));
        assert_eq!(build_options(&schema(), "<@7>").unwrap()[0]["value"], "7");
        assert!(build_options(&schema(), "reason:x").unwrap_err().contains("user"));
        assert!(build_options(&schema(), "user:1 days:x").is_err());
        assert!(build_options(&schema(), "user:1 colour:red").unwrap_err().contains("colour"));
    }

    #[test]
    fn subcommands() {
        let s = json!({"options":[{"type":1,"name":"add","options":[{"type":3,"name":"item","required":true}]}]});
        assert_eq!(build_options(&s, "add milk").unwrap(), json!([{"type":1,"name":"add","options":[{"type":3,"name":"item","value":"milk"}]}]));
        assert!(build_options(&s, "").is_err());
        assert_eq!(split_invocation("/ping"), Some(("ping", "")));
        assert_eq!(split_invocation("/ban user:1"), Some(("ban", "user:1")));
        assert_eq!(split_invocation("hi"), None);
    }
}
