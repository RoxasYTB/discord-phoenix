const SHAPES: &[&str] = &["path", "circle", "rect", "ellipse", "polygon"];

fn close_brace(s: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    while i < s.len() {
        match s[i] {
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < s.len() && s[i] != q {
                    if s[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn kebab(k: &str) -> String {
    let mut o = String::new();
    for ch in k.chars() {
        if ch.is_ascii_uppercase() {
            o.push('-');
            o.push(ch.to_ascii_lowercase());
        } else {
            o.push(ch);
        }
    }
    o
}

fn literal_attrs(obj: &str) -> Vec<(String, String)> {
    let b = obj.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut depth = 0i32;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i += 1;
            while i < b.len() && b[i] != c {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if matches!(c, b'{' | b'(' | b'[') {
            depth += 1;
        } else if matches!(c, b'}' | b')' | b']') {
            depth -= 1;
        }
        let at_key = depth == 0 && (i == 0 || b[i - 1] == b',') && (c.is_ascii_alphabetic());
        if at_key {
            let ks = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            if i < b.len() && b[i] == b':' {
                let key = &obj[ks..i];
                let vs = i + 1;
                if vs < b.len() && b[vs] == b'"' {
                    if let Some(end) = obj[vs + 1..].find('"') {
                        out.push((key.to_owned(), obj[vs + 1..vs + 1 + end].to_owned()));
                        i = vs + 2 + end;
                        continue;
                    }
                } else {
                    let mut e = vs;
                    while e < b.len() && (b[e].is_ascii_digit() || b[e] == b'.' || b[e] == b'-') {
                        e += 1;
                    }
                    if e > vs && (e == b.len() || b[e] == b',' || b[e] == b'}') {
                        out.push((key.to_owned(), obj[vs..e].to_owned()));
                    }
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

fn attr_ok(k: &str) -> bool {
    matches!(
        k,
        "d" | "cx" | "cy" | "r" | "rx" | "ry" | "x" | "y" | "width" | "height" | "points" | "fillRule" | "clipRule" | "transform"
    )
}

pub fn icon_svg(js: &str, name: &str) -> Option<String> {
    let (name, second) = match name.strip_suffix("#secondary") {
        Some(n) => (n, true),
        None => (name, false),
    };
    let needle = format!("{name}:function(");
    let mut from = 0;
    while let Some(pos) = js[from..].find(&needle) {
        let at = from + pos;
        from = at + needle.len();
        if at > 0 && js.as_bytes()[at - 1].is_ascii_alphanumeric() {
            continue;
        }
        let open = at + js[at..].find('{')?;
        let close = close_brace(js.as_bytes(), open)?;
        if let Some(svg) = build(&js[open..=close], second) {
            return Some(svg);
        }
    }
    None
}

fn build(body: &str, second: bool) -> Option<String> {
    if body.contains("\"mask\"") || body.contains("\"defs\"") {
        return None;
    }
    let svg_at = body.find("(\"svg\",{")?;
    let svg_open = svg_at + "(\"svg\",".len();
    let svg_close = close_brace(body.as_bytes(), svg_open)?;
    let root = &body[svg_open + 1..svg_close];
    let view_box = literal_attrs(root)
        .into_iter()
        .find(|(k, _)| k == "viewBox")
        .map(|(_, v)| v)
        .unwrap_or_else(|| "0 0 24 24".into());
    let secondary = body.find("secondaryColor:").map(|i| {
        let v = &body[i + "secondaryColor:".len()..];
        v[..v.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$')).unwrap_or(v.len())].to_owned()
    });
    let mut shapes = String::new();
    for tag in SHAPES {
        let pat = format!("(\"{tag}\",{{");
        let mut f = 0;
        while let Some(p) = root[f..].find(&pat) {
            let o = f + p + pat.len() - 1;
            let c = close_brace(root.as_bytes(), o)?;
            let is_second = secondary.as_deref().filter(|v| !v.is_empty()).is_some_and(|sv| {
                let obj = &root[o + 1..c];
                obj.contains(&format!("typeof {sv}?")) || obj.contains(&format!("fill:{sv},")) || obj.contains(&format!("fill:{sv}}}"))
            });
            if is_second != second {
                f = c;
                continue;
            }
            let attrs: Vec<_> = literal_attrs(&root[o + 1..c]).into_iter().filter(|(k, _)| attr_ok(k)).collect();
            if !attrs.is_empty() {
                shapes.push('<');
                shapes.push_str(tag);
                for (k, v) in attrs {
                    shapes.push_str(&format!(" {}=\"{}\"", kebab(&k), v.replace('"', "")));
                }
                shapes.push_str("/>");
            }
            f = c;
        }
    }
    if shapes.is_empty() {
        return None;
    }
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}" fill="black">{shapes}</svg>"#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    const JS: &str = r#"n.d(e,{},{DemoIcon:function(t){let{size:e="md"}=t;return(0,i.jsxs)("svg",{xmlns:"http://www.w3.org/2000/svg",width:_,fill:"none",viewBox:"0 0 24 24",children:[(0,i.jsx)("path",{fill:"string"==typeof o?o:o.css,fillRule:"evenodd",d:"M1 1h2v2z"}),(0,i.jsx)("circle",{cx:12,cy:12,r:"3",fill:o})]})},OtherIcon:function(){}})"#;
    #[test]
    fn extracts_literal_shapes() {
        let s = icon_svg(JS, "DemoIcon").unwrap();
        assert!(s.contains(r#"<path fill-rule="evenodd" d="M1 1h2v2z"/>"#), "{s}");
        assert!(s.contains(r#"<circle cx="12" cy="12" r="3"/>"#), "{s}");
        assert!(icon_svg(JS, "MissingIcon").is_none());
    }
}
