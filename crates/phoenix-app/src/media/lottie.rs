use std::{collections::HashMap, fmt::Write as _, sync::Arc, time::Instant};

use gpui::{App, AppContext as _, Global, ImageCacheError, RenderImage, SharedString, SvgRenderer};
use serde_json::Value;

const MAX_FPS: f64 = 30.;
const MAX_FRAMES: usize = 180;
const MAX_ANIMS: usize = 8;

pub fn url(id: u64) -> SharedString {
    format!("https://cdn.discordapp.com/stickers/{id}.json").into()
}

type M = [f64; 6];
const IDENT: M = [1., 0., 0., 1., 0., 0.];

fn mul(a: &M, b: &M) -> M {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

fn jnum(x: f64) -> Value {
    serde_json::Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null)
}

fn num_of(v: &Value, d: f64) -> f64 {
    match v {
        Value::Array(a) => a.first().map_or(d, |x| num_of(x, d)),
        Value::Number(n) => n.as_f64().unwrap_or(d),
        _ => d,
    }
}

fn vec2(v: &Value, d: [f64; 2]) -> [f64; 2] {
    match v {
        Value::Array(a) if a.len() >= 2 => [num_of(&a[0], d[0]), num_of(&a[1], d[1])],
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(d[0]);
            [x, x]
        }
        _ => d,
    }
}

fn unwrap(v: &Value) -> &Value {
    match v {
        Value::Array(a) if a.len() == 1 && a[0].is_object() => &a[0],
        _ => v,
    }
}

fn lerp(a: &Value, b: &Value, t: f64) -> Value {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(0.), y.as_f64().unwrap_or(0.));
            jnum(x + (y - x) * t)
        }
        (Value::Array(x), Value::Array(y)) => Value::Array(x.iter().zip(y).map(|(p, q)| lerp(p, q, t)).collect()),
        (Value::Object(x), Value::Object(y)) => {
            let mut o = x.clone();
            for k in ["i", "o", "v"] {
                if let (Some(p), Some(q)) = (x.get(k), y.get(k)) {
                    o.insert(k.into(), lerp(p, q, t));
                }
            }
            Value::Object(o)
        }
        _ => a.clone(),
    }
}

fn cubic(p1: f64, p2: f64, u: f64) -> f64 {
    3. * (1. - u) * (1. - u) * u * p1 + 3. * (1. - u) * u * u * p2 + u * u * u
}

fn ease(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let (mut lo, mut hi, mut u) = (0., 1., t);
    for _ in 0..30 {
        let x = cubic(x1, x2, u);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            lo = u;
        } else {
            hi = u;
        }
        u = (lo + hi) / 2.;
    }
    cubic(y1, y2, u)
}

fn value(p: Option<&Value>, f: f64) -> Value {
    let Some(p) = p else { return Value::Null };
    if p.get("s").and_then(Value::as_bool) == Some(true) {
        return Value::Array(vec![value(p.get("x"), f), value(p.get("y"), f)]);
    }
    let k = p.get("k").unwrap_or(&Value::Null);
    let kf = match k {
        Value::Array(a) if a.first().is_some_and(|x| x.get("t").is_some()) => a,
        _ => return unwrap(k).clone(),
    };
    let t_of = |x: &Value| x.get("t").and_then(Value::as_f64).unwrap_or(0.);
    let start = |x: &Value| x.get("s").map(unwrap).cloned().unwrap_or(Value::Null);
    if f <= t_of(&kf[0]) {
        return start(&kf[0]);
    }
    for w in kf.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if f < t_of(b) {
            let s = start(a);
            if a.get("h").and_then(Value::as_f64) == Some(1.) {
                return s;
            }
            let e = match a.get("e") {
                Some(e) => unwrap(e).clone(),
                None => b.get("s").map(unwrap).cloned().unwrap_or_else(|| s.clone()),
            };
            let span = (t_of(b) - t_of(a)).max(1e-6);
            let mut t = (f - t_of(a)) / span;
            if let (Some(o), Some(i)) = (a.get("o"), a.get("i")) {
                let g = |v: &Value, k: &str, d: f64| v.get(k).map_or(d, |x| num_of(x, d));
                t = ease(g(o, "x", 0.), g(o, "y", 0.), g(i, "x", 1.), g(i, "y", 1.), t);
            }
            return lerp(&s, &e, t);
        }
    }
    let last = &kf[kf.len() - 1];
    if last.get("s").is_some() {
        return start(last);
    }
    match kf.len() {
        n if n >= 2 => kf[n - 2].get("e").or_else(|| kf[n - 2].get("s")).map(unwrap).cloned().unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn transform(ks: Option<&Value>, f: f64) -> (M, f64) {
    let Some(ks) = ks else { return (IDENT, 1.) };
    let a = vec2(&value(ks.get("a"), f), [0., 0.]);
    let p = vec2(&value(ks.get("p"), f), [0., 0.]);
    let s = vec2(&value(ks.get("s"), f), [100., 100.]);
    let r = num_of(&value(ks.get("r").or_else(|| ks.get("rz")), f), 0.).to_radians();
    let o = num_of(&value(ks.get("o"), f), 100.) / 100.;
    let mut m = [1., 0., 0., 1., p[0], p[1]];
    m = mul(&m, &[r.cos(), r.sin(), -r.sin(), r.cos(), 0., 0.]);
    m = mul(&m, &[s[0] / 100., 0., 0., s[1] / 100., 0., 0.]);
    m = mul(&m, &[1., 0., 0., 1., -a[0], -a[1]]);
    (m, o)
}

fn mstr(m: &M) -> String {
    format!("matrix({:.4} {:.4} {:.4} {:.4} {:.3} {:.3})", m[0], m[1], m[2], m[3], m[4], m[5])
}

fn color(c: &Value) -> String {
    let ch = |i: usize| match c {
        Value::Array(a) => a.get(i).map_or(0., |x| num_of(x, 0.)),
        _ => 0.,
    };
    let b = |x: f64| (x * 255.).round().clamp(0., 255.) as u8;
    format!("#{:02x}{:02x}{:02x}", b(ch(0)), b(ch(1)), b(ch(2)))
}

fn points(v: Option<&Value>) -> Vec<[f64; 2]> {
    match v {
        Some(Value::Array(a)) => a.iter().map(|p| vec2(p, [0., 0.])).collect(),
        _ => Vec::new(),
    }
}

fn path_d(sh: &Value) -> String {
    let (v, i, o) = (points(sh.get("v")), points(sh.get("i")), points(sh.get("o")));
    let n = v.len();
    if n == 0 || i.len() < n || o.len() < n {
        return String::new();
    }
    let closed = sh.get("c").and_then(Value::as_bool).unwrap_or(false);
    let mut d = format!("M{:.2} {:.2}", v[0][0], v[0][1]);
    for j in 1..n + usize::from(closed) {
        let (a, b) = (v[(j - 1) % n], v[j % n]);
        let (ao, bi) = (o[(j - 1) % n], i[j % n]);
        let _ = write!(d, "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}", a[0] + ao[0], a[1] + ao[1], b[0] + bi[0], b[1] + bi[1], b[0], b[1]);
    }
    if closed {
        d.push('Z');
    }
    d
}

fn ellipse_d(p: [f64; 2], s: [f64; 2]) -> String {
    let (rx, ry) = (s[0] / 2., s[1] / 2.);
    format!(
        "M{:.2} {:.2}a{rx:.2} {ry:.2} 0 1 0 {:.2} 0a{rx:.2} {ry:.2} 0 1 0 {:.2} 0Z",
        p[0] - rx,
        p[1],
        2. * rx,
        -2. * rx
    )
}

fn rect_d(p: [f64; 2], s: [f64; 2], r: f64) -> String {
    let (x, y, w, h) = (p[0] - s[0] / 2., p[1] - s[1] / 2., s[0], s[1]);
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    if r <= 0. {
        return format!("M{x:.2} {y:.2}h{w:.2}v{h:.2}h{:.2}Z", -w);
    }
    format!(
        "M{:.2} {y:.2}h{:.2}a{r:.2} {r:.2} 0 0 1 {r:.2} {r:.2}v{:.2}a{r:.2} {r:.2} 0 0 1 {:.2} {r:.2}h{:.2}a{r:.2} {r:.2} 0 0 1 {:.2} {:.2}v{:.2}a{r:.2} {r:.2} 0 0 1 {r:.2} {:.2}Z",
        x + r,
        w - 2. * r,
        h - 2. * r,
        -r,
        -(w - 2. * r),
        -r,
        -r,
        -(h - 2. * r),
        -r
    )
}

fn hidden(it: &Value) -> bool {
    it.get("hd").and_then(Value::as_bool).unwrap_or(false)
}

fn ty(it: &Value) -> &str {
    it.get("ty").and_then(Value::as_str).unwrap_or("")
}

fn group_tr(it: &Value) -> Option<&Value> {
    it.get("it")?.as_array()?.iter().find(|x| ty(x) == "tr")
}

fn items(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Trim {
    s: f64,
    e: f64,
    o: f64,
}

fn trim_at(it: &Value, f: f64) -> Trim {
    Trim {
        s: num_of(&value(it.get("s"), f), 0.) / 100.,
        e: num_of(&value(it.get("e"), f), 100.) / 100.,
        o: num_of(&value(it.get("o"), f), 0.) / 360.,
    }
}

fn trims_after(items_: &[Value], j: usize, f: f64, inherited: &[Trim]) -> Vec<Trim> {
    let mut out: Vec<Trim> = items_[j + 1..].iter().filter(|x| ty(x) == "tm" && !hidden(x)).map(|x| trim_at(x, f)).collect();
    out.extend_from_slice(inherited);
    out
}

pub(crate) fn visible(trims: &[Trim]) -> Option<(f64, f64)> {
    let (mut a, mut b) = (0., 1.);
    for t in trims {
        let (s, e) = (t.s.min(t.e).clamp(0., 1.), t.s.max(t.e).clamp(0., 1.));
        let span = b - a;
        let (na, nb) = (a + span * s + t.o * span, a + span * e + t.o * span);
        a = na;
        b = nb;
    }
    (b - a > 1e-4).then_some((a, b))
}

fn cubic_len(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], p3: [f64; 2]) -> f64 {
    let at = |u: f64| {
        let v = 1. - u;
        let k = [v * v * v, 3. * v * v * u, 3. * v * u * u, u * u * u];
        [k[0] * p0[0] + k[1] * p1[0] + k[2] * p2[0] + k[3] * p3[0], k[0] * p0[1] + k[1] * p1[1] + k[2] * p2[1] + k[3] * p3[1]]
    };
    let mut prev = p0;
    let mut len = 0.;
    for n in 1..=24 {
        let q = at(n as f64 / 24.);
        len += ((q[0] - prev[0]).powi(2) + (q[1] - prev[1]).powi(2)).sqrt();
        prev = q;
    }
    len
}

fn path_len(sh: &Value) -> f64 {
    let (v, i, o) = (points(sh.get("v")), points(sh.get("i")), points(sh.get("o")));
    let n = v.len();
    if n == 0 || i.len() < n || o.len() < n {
        return 0.;
    }
    let closed = sh.get("c").and_then(Value::as_bool).unwrap_or(false);
    (1..n + usize::from(closed))
        .map(|j| {
            let (a, b) = (v[(j - 1) % n], v[j % n]);
            let (ao, bi) = (o[(j - 1) % n], i[j % n]);
            cubic_len(a, [a[0] + ao[0], a[1] + ao[1]], [b[0] + bi[0], b[1] + bi[1]], b)
        })
        .sum()
}

struct Geo {
    d: String,
    m: M,
    len: f64,
    trims: Vec<Trim>,
}

fn geometry(items_: &[Value], limit: usize, f: f64, m: &M, inherited: &[Trim], out: &mut Vec<Geo>) {
    for (j, it) in items_.iter().enumerate().take(limit) {
        if hidden(it) {
            continue;
        }
        let (d, len) = match ty(it) {
            "sh" => {
                let sh = value(it.get("ks"), f);
                (path_d(&sh), path_len(&sh))
            }
            "el" => {
                let s = vec2(&value(it.get("s"), f), [0., 0.]);
                let (a, b) = (s[0] / 2., s[1] / 2.);
                let len = std::f64::consts::PI * (3. * (a + b) - ((3. * a + b) * (a + 3. * b)).max(0.).sqrt());
                (ellipse_d(vec2(&value(it.get("p"), f), [0., 0.]), s), len)
            }
            "rc" => {
                let s = vec2(&value(it.get("s"), f), [0., 0.]);
                (
                    rect_d(vec2(&value(it.get("p"), f), [0., 0.]), s, num_of(&value(it.get("r"), f), 0.)),
                    2. * (s[0] + s[1]),
                )
            }
            "gr" => {
                let (gm, _) = transform(group_tr(it), f);
                let sub = items(it.get("it"));
                geometry(sub, sub.len(), f, &mul(m, &gm), &trims_after(items_, j, f, inherited), out);
                continue;
            }
            _ => continue,
        };
        if !d.is_empty() {
            out.push(Geo { d, m: *m, len, trims: trims_after(items_, j, f, inherited) });
        }
    }
}

fn dash(g: &Geo) -> Option<String> {
    if g.trims.is_empty() {
        return Some(String::new());
    }
    let (a, b) = visible(&g.trims)?;
    if b - a >= 1. - 1e-4 || g.len <= 0. {
        return Some(String::new());
    }
    let l = g.len;
    let shift = a.floor();
    let (a, b) = (a - shift, b - shift);
    Some(if b <= 1. {
        format!(r#" stroke-dasharray="{:.3} {:.3}" stroke-dashoffset="{:.3}""#, (b - a) * l, l * 2., -a * l)
    } else {
        format!(r#" stroke-dasharray="{:.3} {:.3} {:.3} {:.3}""#, (b - 1.) * l, (a - (b - 1.)) * l, (1. - a) * l, l * 2.)
    })
}

fn shapes(items_: &[Value], f: f64, tint: Option<&str>, inherited: &[Trim], out: &mut String) {
    for (j, it) in items_.iter().enumerate().rev() {
        if hidden(it) {
            continue;
        }
        match ty(it) {
            "gr" => {
                let (m, o) = transform(group_tr(it), f);
                let _ = write!(out, r#"<g transform="{}" opacity="{o:.3}">"#, mstr(&m));
                shapes(items(it.get("it")), f, tint, &trims_after(items_, j, f, inherited), out);
                out.push_str("</g>");
            }
            kind @ ("fl" | "st") => {
                let mut geo = Vec::new();
                geometry(items_, j, f, &IDENT, inherited, &mut geo);
                if geo.is_empty() {
                    continue;
                }
                let c = tint.map_or_else(|| color(&value(it.get("c"), f)), str::to_owned);
                let op = num_of(&value(it.get("o"), f), 100.) / 100.;
                if kind == "fl" {
                    let rule = if it.get("r").and_then(Value::as_u64) == Some(2) { "evenodd" } else { "nonzero" };
                    let _ = write!(out, r#"<g fill="{c}" fill-opacity="{op:.3}" fill-rule="{rule}" stroke="none">"#);
                } else {
                    let w = num_of(&value(it.get("w"), f), 1.);
                    let lc = match it.get("lc").and_then(Value::as_u64) {
                        Some(2) => "round",
                        Some(3) => "square",
                        _ => "butt",
                    };
                    let lj = match it.get("lj").and_then(Value::as_u64) {
                        Some(2) => "round",
                        Some(3) => "bevel",
                        _ => "miter",
                    };
                    let ml = it.get("ml").and_then(Value::as_f64).unwrap_or(4.);
                    let _ = write!(
                        out,
                        r#"<g fill="none" stroke="{c}" stroke-opacity="{op:.3}" stroke-width="{w:.2}" stroke-linecap="{lc}" stroke-linejoin="{lj}" stroke-miterlimit="{ml}">"#
                    );
                }
                if kind == "fl" {
                    let mut merged: Vec<(M, String)> = Vec::new();
                    for g in geo.iter().filter(|g| g.trims.is_empty() || visible(&g.trims).is_some()) {
                        match merged.iter_mut().find(|(m, _)| *m == g.m) {
                            Some((_, d)) => d.push_str(&g.d),
                            None => merged.push((g.m, g.d.clone())),
                        }
                    }
                    for (m, d) in &merged {
                        let _ = write!(out, r#"<path d="{d}" transform="{}"/>"#, mstr(m));
                    }
                } else {
                    for g in &geo {
                        let Some(extra) = dash(g) else { continue };
                        let _ = write!(out, r#"<path d="{}" transform="{}"{extra}/>"#, g.d, mstr(&g.m));
                    }
                }
                out.push_str("</g>");
            }
            _ => {}
        }
    }
}

struct Ctx<'a> {
    assets: HashMap<&'a str, &'a [Value]>,
    images: HashMap<&'a str, (f64, f64, &'a str)>,
    ids: u32,
    tint: Option<String>,
}

impl Ctx<'_> {
    fn id(&mut self) -> u32 {
        self.ids += 1;
        self.ids
    }
}

const BIG: &str = r#"maskUnits="userSpaceOnUse" x="-100000" y="-100000" width="200000" height="200000""#;

fn layer_matrix(l: &Value, by: &HashMap<i64, &Value>, f: f64, depth: u32) -> M {
    let (m, _) = transform(l.get("ks"), f);
    match l.get("parent").and_then(Value::as_i64).and_then(|p| by.get(&p)) {
        Some(p) if depth < 32 => mul(&layer_matrix(p, by, f, depth + 1), &m),
        _ => m,
    }
}

fn layer(cx: &mut Ctx, l: &Value, by: &HashMap<i64, &Value>, f: f64, depth: u32) -> String {
    let g = |k: &str, d: f64| l.get(k).and_then(Value::as_f64).unwrap_or(d);
    if f < g("ip", 0.) || f >= g("op", f64::MAX) {
        return String::new();
    }
    let m = layer_matrix(l, by, f, 0);
    let (_, o) = transform(l.get("ks"), f);
    let mut body = String::new();
    match l.get("ty").and_then(Value::as_i64) {
        Some(4) => shapes(items(l.get("shapes")), f, cx.tint.as_deref(), &[], &mut body),
        Some(0) if depth < 8 => {
            let Some(sub) = l.get("refId").and_then(Value::as_str).and_then(|r| cx.assets.get(r).copied()) else {
                return String::new();
            };
            let sr = g("sr", 1.);
            let lf = (f - g("st", 0.)) / if sr == 0. { 1. } else { sr };
            body = layers(cx, sub, lf, Some((g("w", 0.), g("h", 0.))), depth + 1);
        }
        Some(2) => {
            let Some(&(w, h, href)) = l.get("refId").and_then(Value::as_str).and_then(|r| cx.images.get(r)) else {
                return String::new();
            };
            if href.is_empty() {
                return String::new();
            }
            let _ = write!(body, r#"<image width="{w}" height="{h}" preserveAspectRatio="none" href="{href}"/>"#);
        }
        Some(1) => {
            let sc = l.get("sc").and_then(Value::as_str).unwrap_or("#000000");
            let _ = write!(body, r#"<rect width="{}" height="{}" fill="{sc}"/>"#, g("sw", 0.), g("sh", 0.));
        }
        _ => return String::new(),
    }
    let masks: Vec<&Value> = items(l.get("masksProperties"))
        .iter()
        .filter(|mk| matches!(mk.get("mode").and_then(Value::as_str), Some("a" | "s")))
        .collect();
    if !masks.is_empty() {
        let id = cx.id();
        let mut md = String::new();
        let first_sub = masks[0].get("mode").and_then(Value::as_str) == Some("s");
        let first_inv = masks[0].get("inv").and_then(Value::as_bool) == Some(true);
        if first_sub || first_inv {
            md.push_str(r##"<rect x="-100000" y="-100000" width="200000" height="200000" fill="#ffffff"/>"##);
        }
        for mk in masks {
            let mo = num_of(&value(mk.get("o"), f), 100.) / 100.;
            let sub = mk.get("mode").and_then(Value::as_str) == Some("s");
            let inv = mk.get("inv").and_then(Value::as_bool) == Some(true);
            let fill = if sub != inv { "#000000" } else { "#ffffff" };
            let _ = write!(md, r#"<path d="{}" fill="{fill}" fill-opacity="{mo:.3}"/>"#, path_d(&value(mk.get("pt"), f)));
        }
        body = format!(r#"<mask id="m{id}" mask-type="luminance" {BIG}>{md}</mask><g mask="url(#m{id})">{body}</g>"#);
    }
    format!(r#"<g transform="{}" opacity="{o:.3}">{body}</g>"#, mstr(&m))
}

fn layers(cx: &mut Ctx, ls: &[Value], f: f64, clip: Option<(f64, f64)>, depth: u32) -> String {
    let by: HashMap<i64, &Value> = ls.iter().filter_map(|l| Some((l.get("ind")?.as_i64()?, l))).collect();
    let flag = |l: &Value, k: &str| l.get(k).is_some_and(|v| v.as_bool() == Some(true) || v.as_i64() == Some(1));
    let mut out = String::new();
    for k in (0..ls.len()).rev() {
        let l = &ls[k];
        if flag(l, "hd") || flag(l, "td") {
            continue;
        }
        let mut s = layer(cx, l, &by, f, depth);
        let tt = l.get("tt").and_then(Value::as_i64);
        if !s.is_empty()
            && let Some(kind @ 1..=4) = tt
            && k > 0
            && flag(&ls[k - 1], "td")
        {
            let id = cx.id();
            let saved = cx.tint.take();
            if kind == 2 {
                cx.tint = Some("#000000".into());
            }
            let matte = layer(cx, &ls[k - 1], &by, f, depth);
            cx.tint = saved;
            let content = match kind {
                1 => matte,
                2 => format!(r##"<rect x="-100000" y="-100000" width="200000" height="200000" fill="#ffffff"/>{matte}"##),
                3 => matte,
                _ => format!(r#"<g filter="url(#i{id})">{matte}</g>"#),
            };
            let defs = if kind == 4 {
                format!(r#"<filter id="i{id}"><feColorMatrix type="matrix" values="-1 0 0 0 1 0 -1 0 0 1 0 0 -1 0 1 0 0 0 1 0"/></filter>"#)
            } else {
                String::new()
            };
            let mt = if kind == 1 { "alpha" } else { "luminance" };
            s = format!(r#"{defs}<mask id="t{id}" mask-type="{mt}" {BIG}>{content}</mask><g mask="url(#t{id})">{s}</g>"#);
        }
        out.push_str(&s);
    }
    if let Some((w, h)) = clip.filter(|(w, h)| *w > 0. && *h > 0.) {
        let id = cx.id();
        out = format!(r#"<clipPath id="c{id}"><rect width="{w}" height="{h}"/></clipPath><g clip-path="url(#c{id})">{out}</g>"#);
    }
    out
}

pub fn frame_svg(root: &Value, f: f64, px: u32) -> String {
    frame_svg_tinted(root, f, px, None)
}

pub fn frame_svg_tinted(root: &Value, f: f64, px: u32, tint: Option<(&str, f32)>) -> String {
    let mut cx = Ctx { assets: HashMap::new(), images: HashMap::new(), ids: 0, tint: tint.map(|t| t.0.to_owned()) };
    for a in items(root.get("assets")) {
        if let (Some(id), Some(ls)) = (a.get("id").and_then(Value::as_str), a.get("layers").and_then(Value::as_array)) {
            cx.assets.insert(id, ls.as_slice());
        } else if let (Some(id), Some(p)) = (a.get("id").and_then(Value::as_str), a.get("p").and_then(Value::as_str)) {
            let d = |k: &str| a.get(k).and_then(Value::as_f64).unwrap_or(0.);
            cx.images.insert(id, (d("w"), d("h"), p));
        }
    }
    let w = root.get("w").and_then(Value::as_f64).unwrap_or(512.);
    let h = root.get("h").and_then(Value::as_f64).unwrap_or(512.);
    let ph = (px as f64 * h / w.max(1.)).round();
    let mut body = layers(&mut cx, items(root.get("layers")), f, Some((w, h)), 0);
    if let Some((_, a)) = tint.filter(|t| t.1 < 1.) {
        body = format!(r#"<g opacity="{a:.3}">{body}</g>"#);
    }
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{px}" height="{ph}" viewBox="0 0 {w} {h}">{body}</svg>"#)
}

fn timeline(root: &Value) -> (f64, f64, usize, u32) {
    let fr = root.get("fr").and_then(Value::as_f64).filter(|v| *v > 0.).unwrap_or(30.);
    let ip = root.get("ip").and_then(Value::as_f64).unwrap_or(0.);
    let op = root.get("op").and_then(Value::as_f64).filter(|op| *op > ip).unwrap_or(ip + fr);
    let step = (fr / MAX_FPS).max(1.);
    let count = (((op - ip) / step).ceil() as usize).clamp(1, MAX_FRAMES);
    (ip, step, count, (step / fr * 1000.).round().max(1.) as u32)
}

fn rasterize(bytes: &[u8], renderer: &SvgRenderer, px: u32, still: bool) -> Option<RenderImage> {
    let root: Value = serde_json::from_slice(bytes).ok()?;
    rasterize_value(&root, renderer, px, still)
}

pub fn duration(root: &Value) -> std::time::Duration {
    let fr = root.get("fr").and_then(Value::as_f64).filter(|v| *v > 0.).unwrap_or(30.);
    let ip = root.get("ip").and_then(Value::as_f64).unwrap_or(0.);
    let op = root.get("op").and_then(Value::as_f64).unwrap_or(ip + fr);
    std::time::Duration::from_secs_f64(((op - ip) / fr).max(0.))
}

pub fn rasterize_value(root: &Value, renderer: &SvgRenderer, px: u32, still: bool) -> Option<RenderImage> {
    frames_of(root, renderer, px, still, 1., 1)
}

pub fn rasterize_speed(root: &Value, renderer: &SvgRenderer, px: u32, speed: f64) -> Option<RenderImage> {
    frames_of(root, renderer, px, false, speed, effect_workers())
}

fn frames_of(root: &Value, renderer: &SvgRenderer, px: u32, still: bool, speed: f64, workers: usize) -> Option<RenderImage> {
    let (ip, step, count, delay) = timeline(root);
    let delay = ((delay as f64) / speed.max(0.01)).round().max(1.) as u32;
    let total = if still { 1 } else { count };
    let one = |n: usize| -> Option<image::Frame> {
        let svg = frame_svg(root, ip + n as f64 * step, (px / 2).max(1));
        let one = renderer.render_single_frame(svg.as_bytes(), 1.0).ok()?;
        let size = one.size(0);
        let (w, h) = (u32::try_from(size.width.0).ok()?, u32::try_from(size.height.0).ok()?);
        let buf = image::RgbaImage::from_raw(w, h, one.as_bytes(0)?.to_vec())?;
        Some(image::Frame::from_parts(buf, 0, 0, image::Delay::from_numer_denom_ms(delay, 1)))
    };
    let workers = if workers > 1 { workers.min(total) } else { 1 };
    let frames: Option<Vec<image::Frame>> = if workers <= 1 {
        (0..total).map(one).collect()
    } else {
        let mut slots: Vec<Option<image::Frame>> = (0..total).map(|_| None).collect();
        let chunk = total.div_ceil(workers);
        std::thread::scope(|s| {
            for (k, part) in slots.chunks_mut(chunk).enumerate() {
                let one = &one;
                s.spawn(move || {
                    for (j, slot) in part.iter_mut().enumerate() {
                        *slot = one(k * chunk + j);
                    }
                });
            }
        });
        slots.into_iter().collect()
    };
    Some(RenderImage::new(frames?))
}

pub fn effect_workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).clamp(1, 4))
}

enum State {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

#[derive(Default)]
struct Store {
    anims: HashMap<SharedString, (State, Instant)>,
}

impl Global for Store {}

fn load(url: &SharedString, px: u32, still: bool, cx: &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
    let key: SharedString = format!("{px}:{still}:{url}").into();
    let s = cx.default_global::<Store>();
    if let Some((st, used)) = s.anims.get_mut(&key) {
        *used = Instant::now();
        return match st {
            State::Loading => None,
            State::Ready(img) => Some(Ok(img.clone())),
            State::Failed => Some(Err(ImageCacheError::Other(Arc::new(anyhow::anyhow!("lottie sticker failed"))))),
        };
    }
    let evicted = if s.anims.len() >= MAX_ANIMS
        && let Some(old) = s.anims.iter().filter(|(_, (st, _))| !matches!(st, State::Loading)).min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone())
    {
        match s.anims.remove(&old) {
            Some((State::Ready(img), _)) => Some(img),
            _ => None,
        }
    } else {
        None
    };
    s.anims.insert(key.clone(), (State::Loading, Instant::now()));
    if let Some(img) = evicted {
        cx.drop_image(img, None);
    }
    let http = cx.http_client();
    let renderer = cx.svg_renderer();
    let fetch = url.clone();
    cx.spawn(async move |cx| {
        use futures::AsyncReadExt;
        let mut bytes = Vec::new();
        let ok = match http.get(&fetch, Default::default(), true).await {
            Ok(mut r) => r.status().is_success() && r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let img = if ok { cx.background_spawn(async move { rasterize(&bytes, &renderer, px, still) }).await } else { None };
        cx.update(|cx| {
            if let Some(e) = cx.default_global::<Store>().anims.get_mut(&key) {
                e.0 = match img {
                    Some(i) => State::Ready(Arc::new(i)),
                    None => State::Failed,
                };
            }
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

fn load_bytes(name: &SharedString, bytes: &Arc<Vec<u8>>, px: u32, still: bool, cx: &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
    let key: SharedString = format!("{px}:{still}:bytes:{name}").into();
    let s = cx.default_global::<Store>();
    if let Some((st, used)) = s.anims.get_mut(&key) {
        *used = Instant::now();
        return match st {
            State::Loading => None,
            State::Ready(img) => Some(Ok(img.clone())),
            State::Failed => Some(Err(ImageCacheError::Other(Arc::new(anyhow::anyhow!("lottie sticker failed"))))),
        };
    }
    let evicted = if s.anims.len() >= MAX_ANIMS
        && let Some(old) = s.anims.iter().filter(|(_, (st, _))| !matches!(st, State::Loading)).min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone())
    {
        match s.anims.remove(&old) {
            Some((State::Ready(img), _)) => Some(img),
            _ => None,
        }
    } else {
        None
    };
    s.anims.insert(key.clone(), (State::Loading, Instant::now()));
    if let Some(img) = evicted {
        cx.drop_image(img, None);
    }
    let renderer = cx.svg_renderer();
    let bytes = bytes.clone();
    cx.spawn(async move |cx| {
        let img = cx.background_spawn(async move { rasterize(&bytes, &renderer, px, still) }).await;
        cx.update(|cx| {
            if let Some(e) = cx.default_global::<Store>().anims.get_mut(&key) {
                e.0 = match img {
                    Some(i) => State::Ready(Arc::new(i)),
                    None => State::Failed,
                };
            }
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

pub fn image_bytes(name: SharedString, bytes: Arc<Vec<u8>>, px: u32, still: bool) -> gpui::Img {
    gpui::img(move |_: &mut gpui::Window, cx: &mut App| load_bytes(&name, &bytes, px, still, cx))
}

pub fn image(url: SharedString, px: u32, still: bool) -> gpui::Img {
    gpui::img(move |_: &mut gpui::Window, cx: &mut App| load(&url, px, still, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_and_interpolates() {
        assert!((ease(0., 0., 1., 1., 0.5) - 0.5).abs() < 1e-3);
        let p = serde_json::json!({"a": 1, "k": [
            {"t": 0, "s": [0, 10], "o": {"x": [0], "y": [0]}, "i": {"x": [1], "y": [1]}},
            {"t": 10, "s": [100, 20]}
        ]});
        assert_eq!(vec2(&value(Some(&p), 5.), [0., 0.]), [50., 15.]);
        assert_eq!(vec2(&value(Some(&p), 20.), [0., 0.]), [100., 20.]);
        let hold = serde_json::json!({"a": 1, "k": [{"t": 0, "s": [3], "h": 1}, {"t": 10, "s": [9]}]});
        assert_eq!(num_of(&value(Some(&hold), 5.), 0.), 3.);
    }

    #[test]
    fn trims_strokes_with_a_dash() {
        let root = serde_json::json!({
            "fr": 60, "ip": 0, "op": 60, "w": 24, "h": 24,
            "layers": [{"ty": 4, "ind": 1, "ip": 0, "op": 60, "ks": {},
                "shapes": [
                    {"ty": "gr", "it": [
                        {"ty": "sh", "ks": {"a": 0, "k": {"c": false, "v": [[0, 0], [10, 0]], "i": [[0, 0], [0, 0]], "o": [[0, 0], [0, 0]]}}},
                        {"ty": "st", "c": {"a": 0, "k": [0.2, 0.4, 0.9, 1]}, "o": {"a": 0, "k": 100}, "w": {"a": 0, "k": 2}},
                        {"ty": "tr"}
                    ]},
                    {"ty": "tm", "s": {"a": 0, "k": 0}, "e": {"a": 1, "k": [{"t": 0, "s": [0]}, {"t": 10, "s": [100]}]}, "o": {"a": 0, "k": 0}, "m": 1}
                ]}]
        });
        let half = frame_svg_tinted(&root, 5., 24, Some(("#abcdef", 1.)));
        assert!(half.contains(r#"stroke-dasharray="5.000 20.000""#), "{half}");
        assert!(half.contains(r##"stroke="#abcdef""##), "{half}");
        let none = frame_svg(&root, 0., 24);
        assert!(!none.contains("<path"), "{none}");
        let full = frame_svg(&root, 10., 24);
        assert!(full.contains("<path") && !full.contains("dasharray"), "{full}");
        assert!(full.contains(r##"stroke="#3366e6""##), "{full}");
        let faded = frame_svg_tinted(&root, 10., 24, Some(("#000000", 0.5)));
        assert!(faded.contains(r#"<g opacity="0.500">"#), "{faded}");
    }

    #[test]
    fn fills_merge_the_paths_of_a_group_so_holes_cut_through() {
        let sh = |r: f64, cw: bool| {
            let v = if cw { serde_json::json!([[0, -r], [r, 0], [0, r], [-r, 0]]) } else { serde_json::json!([[0, -r], [-r, 0], [0, r], [r, 0]]) };
            serde_json::json!({"ty": "sh", "ks": {"a": 0, "k": {"c": true, "v": v, "i": [[0, 0], [0, 0], [0, 0], [0, 0]], "o": [[0, 0], [0, 0], [0, 0], [0, 0]]}}})
        };
        let root = serde_json::json!({
            "fr": 60, "ip": 0, "op": 60, "w": 24, "h": 24,
            "layers": [{"ty": 4, "ind": 1, "ip": 0, "op": 60, "ks": {}, "shapes": [{"ty": "gr", "it": [
                sh(4., false), sh(10., true),
                {"ty": "fl", "c": {"a": 0, "k": [1, 1, 1, 1]}, "o": {"a": 0, "k": 100}, "r": 1},
                {"ty": "tr"}
            ]}]}]
        });
        let svg = frame_svg(&root, 0., 24);
        assert_eq!(svg.matches("<path ").count(), 1, "{svg}");
        assert_eq!(svg.matches('M').count(), 2, "{svg}");
    }

    #[test]
    fn composes_trims_and_wraps_offsets() {
        let t = |s: f64, e: f64, o: f64| Trim { s, e, o };
        assert_eq!(visible(&[t(0., 1., 0.)]), Some((0., 1.)));
        assert_eq!(visible(&[t(0.5, 0.5, 0.)]), None);
        let (a, b) = visible(&[t(0., 0.5, 0.), t(0.5, 1., 0.)]).unwrap();
        assert!((a - 0.25).abs() < 1e-9 && (b - 0.5).abs() < 1e-9);
        let (a, b) = visible(&[t(1., 0.2, 0.)]).unwrap();
        assert!((a - 0.2).abs() < 1e-9 && (b - 1.).abs() < 1e-9);
        let g = Geo { d: String::new(), m: IDENT, len: 10., trims: vec![t(0., 0.5, 0.75)] };
        assert_eq!(dash(&g).as_deref(), Some(r#" stroke-dasharray="2.500 5.000 2.500 20.000""#));
    }

    #[test]
    fn subtract_masks_and_inverted_mattes() {
        let sq = |x: f64| serde_json::json!({"a": 0, "k": {"c": true, "v": [[x, x], [x + 4., x], [x + 4., x + 4.]], "i": [[0, 0], [0, 0], [0, 0]], "o": [[0, 0], [0, 0], [0, 0]]}});
        let fill = serde_json::json!({"ty": "fl", "c": {"a": 0, "k": [1, 1, 1, 1]}, "o": {"a": 0, "k": 100}});
        let root = serde_json::json!({
            "fr": 60, "ip": 0, "op": 60, "w": 24, "h": 24,
            "layers": [
                {"ty": 4, "ind": 1, "td": 1, "ip": 0, "op": 60, "ks": {}, "shapes": [{"ty": "sh", "ks": sq(2.)}, fill.clone()]},
                {"ty": 4, "ind": 2, "tt": 2, "ip": 0, "op": 60, "ks": {},
                    "masksProperties": [{"mode": "s", "inv": false, "pt": sq(8.), "o": {"a": 0, "k": 100}}],
                    "shapes": [{"ty": "sh", "ks": sq(0.)}, fill]}
            ]
        });
        let svg = frame_svg_tinted(&root, 0., 24, Some(("#123456", 1.)));
        assert!(svg.contains(r#"mask-type="luminance""#), "{svg}");
        assert!(svg.contains(r##"fill="#000000" fill-opacity="1.000""##), "{svg}");
        assert!(svg.contains(r##"<g fill="#000000" fill-opacity="1.000" fill-rule="nonzero""##), "{svg}");
        assert!(svg.contains(r##"<g fill="#123456""##), "{svg}");
        assert_eq!(svg.matches(r##"<rect x="-100000""##).count(), 2, "{svg}");
    }

    #[test]
    fn draws_a_filled_shape_layer() {
        let root = serde_json::json!({
            "fr": 60, "ip": 0, "op": 60, "w": 100, "h": 100,
            "layers": [{"ty": 4, "ind": 1, "ip": 0, "op": 60, "ks": {"p": {"a": 0, "k": [50, 50]}},
                "shapes": [{"ty": "gr", "it": [
                    {"ty": "el", "p": {"a": 0, "k": [0, 0]}, "s": {"a": 0, "k": [20, 20]}},
                    {"ty": "fl", "c": {"a": 0, "k": [1, 0, 0, 1]}, "o": {"a": 0, "k": 100}},
                    {"ty": "tr"}
                ]}]}]
        });
        let svg = frame_svg(&root, 0., 80);
        assert!(svg.contains(r##"fill="#ff0000""##), "{svg}");
        assert!(svg.contains("a10.00 10.00"), "{svg}");
        assert_eq!(timeline(&root), (0., 2., 30, 33));
    }
}
