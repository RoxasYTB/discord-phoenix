use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use super::intl;

const MAP_SRC: &str = include_str!("../../locales/discord.yml");

pub struct Mapping {
    pub hash: &'static str,
    pub upper: bool,
    pub args: Vec<(&'static str, &'static str)>,
}

pub fn mappings() -> &'static HashMap<&'static str, Mapping> {
    static M: OnceLock<HashMap<&'static str, Mapping>> = OnceLock::new();
    M.get_or_init(|| parse_mappings(MAP_SRC))
}

fn parse_mappings(src: &'static str) -> HashMap<&'static str, Mapping> {
    let mut out = HashMap::new();
    for line in src.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim();
        let v = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(v);
        let mut parts = v.split_whitespace();
        let Some(hash) = parts.next() else { continue };
        let mut upper = false;
        let mut args = Vec::new();
        for tok in parts {
            if tok == "^upper" {
                upper = true;
            } else if let Some((from, to)) = tok.split_once('=') {
                args.push((from, to));
            }
        }
        out.insert(k.trim(), Mapping { hash, upper, args });
    }
    out
}

pub fn render(locale: &str, ast: &Value, args: &[(&str, &str)], mapping: &Mapping) -> String {
    let mapped: Vec<(&str, &str)> = args
        .iter()
        .map(|&(k, v)| {
            let target = mapping.args.iter().find(|&&(from, _)| from == k).map_or(k, |&(_, to)| to);
            (target, v)
        })
        .collect();
    let mut out = String::new();
    eval_node(locale, ast, &mapped, None, &mut out);
    if mapping.upper {
        out.to_uppercase()
    } else {
        out
    }
}

fn eval_node(locale: &str, node: &Value, args: &[(&str, &str)], pound: Option<&str>, out: &mut String) {
    match node {
        Value::String(s) => out.push_str(s),
        Value::Array(items) => {
            if items.is_empty() {
                return;
            }
            if let Some(kind) = items[0].as_u64() {
                eval_typed(locale, kind, items, args, pound, out);
            } else {
                for item in items {
                    eval_node(locale, item, args, pound, out);
                }
            }
        }
        _ => {}
    }
}

fn find_arg<'a>(args: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    args.iter().find(|&&(k, _)| k == name).map(|&(_, v)| v)
}

fn eval_typed(locale: &str, kind: u64, items: &[Value], args: &[(&str, &str)], pound: Option<&str>, out: &mut String) {
    match kind {
        1 => {
            if let Some(name) = items.get(1).and_then(Value::as_str)
                && let Some(val) = find_arg(args, name)
            {
                out.push_str(val);
            }
        }
        2 => {
            if let Some(name) = items.get(1).and_then(Value::as_str)
                && let Some(val) = find_arg(args, name)
            {
                if let Ok(n) = val.parse::<i64>() {
                    out.push_str(&intl::number(locale, n));
                } else if let Ok(f) = val.parse::<f64>() {
                    out.push_str(&intl::decimal(locale, f, 2));
                } else {
                    out.push_str(val);
                }
            }
        }
        3 | 4 => {
            if let Some(name) = items.get(1).and_then(Value::as_str)
                && let Some(val) = find_arg(args, name)
            {
                out.push_str(val);
            }
        }
        5 => {
            let Some(name) = items.get(1).and_then(Value::as_str) else { return };
            let Some(cases) = items.get(2).and_then(Value::as_object) else { return };
            let val = find_arg(args, name).unwrap_or("");
            if let Some(branch) = cases.get(val).or_else(|| cases.get("other")) {
                eval_node(locale, branch, args, pound, out);
            }
        }
        6 => {
            let Some(name) = items.get(1).and_then(Value::as_str) else { return };
            let Some(cases) = items.get(2).and_then(Value::as_object) else { return };
            let offset = items.get(3).and_then(Value::as_f64).unwrap_or(0.0);
            let raw = find_arg(args, name).unwrap_or("0");
            let n = raw.parse::<f64>().unwrap_or(0.0);
            let exact = format!("={}", n as i64);
            let cat = intl::plural(locale, (n - offset).max(0.0));
            let formatted = if (n - offset).fract() == 0.0 {
                intl::number(locale, (n - offset) as i64)
            } else {
                intl::decimal(locale, n - offset, 2)
            };
            if let Some(branch) = cases.get(&exact).or_else(|| cases.get(cat)).or_else(|| cases.get("other")) {
                eval_node(locale, branch, args, Some(&formatted), out);
            }
        }
        7 => {
            if let Some(p) = pound {
                out.push_str(p);
            }
        }
        8 => {
            if let Some(children) = items.get(2) {
                eval_node(locale, children, args, pound, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evaluates_icu_plural_and_hooks() {
        let ast = json!([
            [6, "count", {
                "=0": ["No items"],
                "one": [[7], " item"],
                "other": [[7], " items"]
            }, 0, "cardinal"]
        ]);
        let m = Mapping { hash: "x", upper: false, args: vec![("n", "count")] };
        assert_eq!(render("en-US", &ast, &[("n", "0")], &m), "No items");
        assert_eq!(render("en-US", &ast, &[("n", "1")], &m), "1 item");
        assert_eq!(render("en-US", &ast, &[("n", "1234")], &m), "1,234 items");
        assert_eq!(render("de", &ast, &[("n", "1234")], &m), "1.234 items");

        let hook = json!([[8, "$b", ["Hello ", [1, "name"]]]]);
        let m2 = Mapping { hash: "y", upper: true, args: vec![] };
        assert_eq!(render("en-US", &hook, &[("name", "Bob")], &m2), "HELLO BOB");
    }

    #[test]
    fn loads_discord_key_mappings() {
        let m = mappings();
        assert!(m.contains_key("status.online"));
        assert_eq!(m["status.online"].hash, "60kAzo");
        assert!(m["settings.appearance.theme"].upper);
    }
}
