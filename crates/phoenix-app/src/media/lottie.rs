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

fn geometry(items_: &[Value], f: f64, m: &M, out: &mut Vec<(String, M)>) {
    for it in items_ {
        if hidden(it) {
            continue;
        }
        let d = match ty(it) {
            "sh" => path_d(&value(it.get("ks"), f)),
            "el" => ellipse_d(vec2(&value(it.get("p"), f), [0., 0.]), vec2(&value(it.get("s"), f), [0., 0.])),
            "rc" => rect_d(
                vec2(&value(it.get("p"), f), [0., 0.]),
                vec2(&value(it.get("s"), f), [0., 0.]),
                num_of(&value(it.get("r"), f), 0.),
            ),
            "gr" => {
                let (gm, _) = transform(group_tr(it), f);
                geometry(items(it.get("it")), f, &mul(m, &gm), out);
                continue;
            }
            _ => continue,
        };
        if !d.is_empty() {
            out.push((d, *m));
        }
    }
}

fn shapes(items_: &[Value], f: f64, out: &mut String) {
    for (j, it) in items_.iter().enumerate().rev() {
        if hidden(it) {
            continue;
        }
        match ty(it) {
            "gr" => {
                let (m, o) = transform(group_tr(it), f);
                let _ = write!(out, r#"<g transform="{}" opacity="{o:.3}">"#, mstr(&m));
                shapes(items(it.get("it")), f, out);
                out.push_str("</g>");
            }
            kind @ ("fl" | "st") => {
                let mut geo = Vec::new();
                geometry(&items_[..j], f, &IDENT, &mut geo);
                if geo.is_empty() {
                    continue;
                }
                let c = color(&value(it.get("c"), f));
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
                for (d, m) in &geo {
                    let _ = write!(out, r#"<path d="{d}" transform="{}"/>"#, mstr(m));
                }
                out.push_str("</g>");
            }
            _ => {}
        }
    }
}

struct Ctx<'a> {
    assets: HashMap<&'a str, &'a [Value]>,
    ids: u32,
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
        Some(4) => shapes(items(l.get("shapes")), f, &mut body),
        Some(0) if depth < 8 => {
            let Some(sub) = l.get("refId").and_then(Value::as_str).and_then(|r| cx.assets.get(r).copied()) else {
                return String::new();
            };
            let sr = g("sr", 1.);
            let lf = (f - g("st", 0.)) / if sr == 0. { 1. } else { sr };
            body = layers(cx, sub, lf, Some((g("w", 0.), g("h", 0.))), depth + 1);
        }
        Some(1) => {
            let sc = l.get("sc").and_then(Value::as_str).unwrap_or("#000000");
            let _ = write!(body, r#"<rect width="{}" height="{}" fill="{sc}"/>"#, g("sw", 0.), g("sh", 0.));
        }
        _ => return String::new(),
    }
    let masks: Vec<&Value> = items(l.get("masksProperties"))
        .iter()
        .filter(|mk| mk.get("mode").and_then(Value::as_str) == Some("a") && mk.get("inv").and_then(Value::as_bool) != Some(true))
        .collect();
    if !masks.is_empty() {
        let id = cx.id();
        let mut md = String::new();
        for mk in masks {
            let mo = num_of(&value(mk.get("o"), f), 100.) / 100.;
            let _ = write!(md, r##"<path d="{}" fill="#ffffff" fill-opacity="{mo:.3}"/>"##, path_d(&value(mk.get("pt"), f)));
        }
        body = format!(r#"<mask id="m{id}" mask-type="alpha" {BIG}>{md}</mask><g mask="url(#m{id})">{body}</g>"#);
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
            && let Some(kind @ (1 | 3)) = tt
            && k > 0
            && flag(&ls[k - 1], "td")
        {
            let id = cx.id();
            let matte = layer(cx, &ls[k - 1], &by, f, depth);
            let mt = if kind == 1 { "alpha" } else { "luminance" };
            s = format!(r#"<mask id="t{id}" mask-type="{mt}" {BIG}>{matte}</mask><g mask="url(#t{id})">{s}</g>"#);
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
    let mut cx = Ctx { assets: HashMap::new(), ids: 0 };
    for a in items(root.get("assets")) {
        if let (Some(id), Some(ls)) = (a.get("id").and_then(Value::as_str), a.get("layers").and_then(Value::as_array)) {
            cx.assets.insert(id, ls.as_slice());
        }
    }
    let w = root.get("w").and_then(Value::as_f64).unwrap_or(512.);
    let h = root.get("h").and_then(Value::as_f64).unwrap_or(512.);
    let ph = (px as f64 * h / w.max(1.)).round();
    let body = layers(&mut cx, items(root.get("layers")), f, Some((w, h)), 0);
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
    let (ip, step, count, delay) = timeline(&root);
    let mut frames = Vec::new();
    for n in 0..if still { 1 } else { count } {
        let svg = frame_svg(&root, ip + n as f64 * step, (px / 2).max(1));
        let one = renderer.render_single_frame(svg.as_bytes(), 1.0).ok()?;
        let size = one.size(0);
        let (w, h) = (u32::try_from(size.width.0).ok()?, u32::try_from(size.height.0).ok()?);
        let buf = image::RgbaImage::from_raw(w, h, one.as_bytes(0)?.to_vec())?;
        frames.push(image::Frame::from_parts(buf, 0, 0, image::Delay::from_numer_denom_ms(delay, 1)));
    }
    Some(RenderImage::new(frames))
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
    if s.anims.len() >= MAX_ANIMS
        && let Some(old) = s.anims.iter().filter(|(_, (st, _))| !matches!(st, State::Loading)).min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone())
    {
        s.anims.remove(&old);
    }
    s.anims.insert(key.clone(), (State::Loading, Instant::now()));
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
