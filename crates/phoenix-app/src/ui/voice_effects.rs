use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use base64::Engine as _;
use gpui::{AnyElement, Context, RenderImage, Styled, div, img, prelude::*};
use serde_json::Value;

use crate::app::PhoenixApp;
use crate::model::Id;
use crate::theme::z;
use crate::voice::soundboard::Effect;

pub const COMP_W: f32 = 1280.;
pub const COMP_H: f32 = 720.;
pub const PREMIUM: u8 = 0;
pub const BASIC: u8 = 1;
pub const MAX_ACTIVE: usize = 50;
const RENDER_PX: u32 = 1280;

pub fn unescape_js(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('x') => {
                let h: String = it.by_ref().take(2).collect();
                if let Some(ch) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some('u') => {
                let h: String = it.by_ref().take(4).collect();
                if let Some(ch) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

pub fn extract(js: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = js[from..].find("JSON.parse('") {
        let start = from + i + "JSON.parse('".len();
        let bytes = js.as_bytes();
        let mut j = start;
        while j < bytes.len() && !(bytes[j] == b'\'' && bytes[j - 1] != b'\\') {
            j += 1;
        }
        from = j.min(js.len());
        let raw = &js[start..from];
        if raw.contains("\"nm\":\"FrameEffect_")
            && let Ok(v) = serde_json::from_str::<Value>(&unescape_js(raw))
        {
            out.push(v);
        }
    }
    out
}

pub fn animations() -> &'static Vec<Value> {
    static A: OnceLock<Vec<Value>> = OnceLock::new();
    A.get_or_init(|| {
        for cache in crate::icons::discord_caches() {
            for (_, entry) in crate::icons::cache::js_entries(&cache).into_iter().filter(|(n, _)| n.starts_with("web.")).take(4) {
                if let Some(js) = crate::icons::cache::read_body(&entry) {
                    let found = extract(&js);
                    if !found.is_empty() {
                        return found;
                    }
                }
            }
        }
        Vec::new()
    })
}

pub fn prewarm() {
    static WARMING: AtomicBool = AtomicBool::new(false);
    if WARMING.swap(true, Ordering::Relaxed) {
        return;
    }
    let _ = std::thread::Builder::new().name("voice-effects-prewarm".into()).spawn(|| {
        let _ = animations();
    });
}

pub fn pick(list: &[Value], kind: u8, id: u32) -> Option<&Value> {
    if kind == BASIC {
        return list.first();
    }
    let premium = list.get(1..)?;
    if premium.is_empty() {
        return list.first();
    }
    premium.get(id as usize).or_else(|| premium.get(id as usize % premium.len()))
}

pub fn with_image(anim: &Value, data_uri: &str) -> Value {
    let mut v = anim.clone();
    if let Some(assets) = v.get_mut("assets").and_then(Value::as_array_mut) {
        for a in assets.iter_mut() {
            if a.get("layers").is_none()
                && let Some(o) = a.as_object_mut()
            {
                o.insert("p".into(), Value::String(data_uri.to_owned()));
                o.insert("u".into(), Value::String(String::new()));
                o.insert("e".into(), Value::from(1));
            }
        }
    }
    v
}

pub fn emoji_url(e: &Effect) -> Option<String> {
    if let Some(id) = e.emoji_id {
        return Some(format!("https://cdn.discordapp.com/emojis/{}.png?size=64", id.0));
    }
    e.emoji_name.as_ref().and_then(|n| crate::media::emoji::twemoji_url(n.as_ref())).map(|u| u.to_string())
}

fn fetch(url: &str) -> Option<Vec<u8>> {
    static EMOJI_CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    let cache = EMOJI_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(bytes) = guard.get(url)
    {
        return Some(bytes.clone());
    }
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    let bytes = rt.block_on(async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().ok()?;
        let r = client.get(url).send().await.ok()?;
        if !r.status().is_success() {
            return None;
        }
        Some(r.bytes().await.ok()?.to_vec())
    })?;
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= 32 {
            guard.clear();
        }
        guard.insert(url.to_owned(), bytes.clone());
    }
    Some(bytes)
}

pub fn geometry(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let s = (w / COMP_W).max(h / COMP_H);
    let (iw, ih) = (COMP_W * s, COMP_H * s);
    ((w - iw) / 2., h - ih, iw, ih)
}

struct Active {
    key: u64,
    user: Id,
    until: Instant,
    img: Option<Arc<RenderImage>>,
}

thread_local! {
    static ACTIVE: RefCell<Vec<Active>> = const { RefCell::new(Vec::new()) };
    static NEXT: RefCell<u64> = const { RefCell::new(0) };
}

pub fn start(app: &PhoenixApp, e: &Effect, cx: &mut Context<PhoenixApp>) {
    if cx.reduce_motion() || Some(e.channel) != app.voice_channel {
        return;
    }
    let Some((kind, id)) = e.animation else { return };
    let Some(url) = emoji_url(e) else { return };
    if ACTIVE.with(|a| a.borrow().len()) >= MAX_ACTIVE {
        return;
    }
    let key = NEXT.with(|n| {
        let mut n = n.borrow_mut();
        *n += 1;
        *n
    });
    let user = e.user;
    let renderer = cx.svg_renderer();
    cx.spawn(async move |this, cx| {
        let made = cx
            .background_executor()
            .spawn(async move {
                let log = std::env::var_os("PHOENIX_SOUND_LOG").is_some();
                let t = Instant::now();
                let list = animations();
                let anim = pick(list, kind, id);
                if log {
                    eprintln!("phoenix-soundboard: effect animations={} picked={} after {:?}", list.len(), anim.is_some(), t.elapsed());
                }
                let anim = anim?;
                let bytes = fetch(&url);
                if log {
                    eprintln!("phoenix-soundboard: effect emoji {:?} bytes after {:?}", bytes.as_ref().map(Vec::len), t.elapsed());
                }
                let bytes = bytes?;
                let mime = if bytes.starts_with(b"\x89PNG") { "image/png" } else { "image/webp" };
                let uri = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes));
                let v = with_image(anim, &uri);
                let speed = if kind == PREMIUM { 0.8 } else { 1. };
                let dur = crate::media::lottie::duration(&v).div_f64(speed);
                let img = crate::media::lottie::rasterize_speed(&v, &renderer, RENDER_PX, speed);
                if log {
                    eprintln!(
                        "phoenix-soundboard: effect frames {:?} after {:?}, lasts {dur:?}",
                        img.as_ref().map(gpui::RenderImage::frame_count),
                        t.elapsed()
                    );
                }
                Some((Arc::new(img?), dur))
            })
            .await;
        let Some((img, dur)) = made else { return };
        ACTIVE.with(|a| a.borrow_mut().push(Active { key, user, until: Instant::now() + dur, img: Some(img) }));
        let _ = this.update(cx, |_, cx| cx.notify());
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            std::thread::sleep(dur);
            let _ = tx.send(());
        });
        let _ = rx.await;
        ACTIVE.with(|a| a.borrow_mut().retain(|x| x.key != key));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

pub fn clear() {
    ACTIVE.with(|a| a.borrow_mut().clear());
}

pub fn has_effects() -> bool {
    ACTIVE.with(|a| !a.borrow().is_empty())
}

pub fn overlay(user: Id, w: f32, h: f32) -> Option<AnyElement> {
    let now = Instant::now();
    let imgs: Vec<Arc<RenderImage>> = ACTIVE.with(|a| a.borrow().iter().filter(|x| x.user == user && x.until > now).filter_map(|x| x.img.clone()).collect());
    if imgs.is_empty() {
        return None;
    }
    let (l, t, iw, ih) = geometry(w, h);
    Some(
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .children(imgs.into_iter().map(|i| img(i).absolute().left(z(l)).top(z(t)).w(z(iw)).h(z(ih))))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_effect_json() {
        let js = r#"var a=JSON.parse('{"nm":"FrameEffect_10-1_Out","fr":60,"ip":2,"op":81,"w":1280,"h":720,"assets":[{"id":"image_0","w":35,"h":35,"u":"","p":"","e":1}],"layers":[]}'),b=JSON.parse('{"x":1}'),c=JSON.parse('{"nm":"FrameEffect_01-1_Out","d":"it\'s"}');"#;
        let v = extract(js);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1]["d"], "it's");
        assert_eq!(pick(&v, BASIC, 7).and_then(|x| x["nm"].as_str()), Some("FrameEffect_10-1_Out"));
        assert_eq!(pick(&v, PREMIUM, 0).and_then(|x| x["nm"].as_str()), Some("FrameEffect_01-1_Out"));
        let w = with_image(&v[0], "data:image/png;base64,AA");
        assert_eq!(w["assets"][0]["p"], "data:image/png;base64,AA");
        assert!((crate::media::lottie::duration(&v[0]).as_secs_f64() - 79. / 60.).abs() < 1e-6);
    }

    #[test]
    fn slice_anchors_bottom_centre() {
        let (l, t, w, h) = geometry(596., 335.25);
        assert!((w - 596.).abs() < 0.01 && (h - 335.25).abs() < 0.01);
        assert!(l.abs() < 0.01 && t.abs() < 0.01);
        let (l, t, w, h) = geometry(400., 400.);
        assert!((h - 400.).abs() < 0.01);
        assert!((l - (400. - w) / 2.).abs() < 0.01);
        assert!(t.abs() < 0.01);
    }
}
