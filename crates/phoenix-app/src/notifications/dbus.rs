use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{StreamExt, channel::mpsc};
use zbus::zvariant::{Structure, Value};

use super::{Target, Toast};

pub enum Cmd {
    Show(Toast),
    Close(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Click {
    pub target: Target,
    pub token: Option<String>,
}

static TX: Mutex<Option<mpsc::UnboundedSender<Cmd>>> = Mutex::new(None);

fn tx() -> std::sync::MutexGuard<'static, Option<mpsc::UnboundedSender<Cmd>>> {
    TX.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn send(cmd: Cmd) -> Result<(), Cmd> {
    match tx().as_ref() {
        Some(t) => t.unbounded_send(cmd).map_err(mpsc::TrySendError::into_inner),
        None => Err(cmd),
    }
}

pub fn spawn(clicks: mpsc::UnboundedSender<Click>) -> bool {
    let (t, rx) = mpsc::unbounded::<Cmd>();
    *tx() = Some(t);
    let spawned = std::thread::Builder::new().name("phoenix-notify".into()).spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::warn!("desktop notifications unavailable: {e}");
                return;
            }
        };
        if let Err(e) = rt.block_on(serve(rx, clicks)) {
            tracing::warn!("desktop notifications unavailable: {e}");
        }
        *tx() = None;
    });
    if spawned.is_err() {
        *tx() = None;
    }
    spawned.is_ok()
}

#[derive(Default)]
pub struct Live {
    by_tag: HashMap<u64, u32>,
    targets: HashMap<u32, (u64, Target)>,
    tokens: HashMap<u32, String>,
}

impl Live {
    pub fn replaces(&self, tag: u64) -> u32 {
        self.by_tag.get(&tag).copied().unwrap_or(0)
    }
    pub fn shown(&mut self, tag: u64, id: u32, target: Target) {
        if let Some(old) = self.by_tag.insert(tag, id).filter(|o| *o != id) {
            self.targets.remove(&old);
        }
        self.targets.insert(id, (tag, target));
    }
    pub fn take_tag(&mut self, tag: u64) -> Option<u32> {
        let id = self.by_tag.remove(&tag)?;
        self.targets.remove(&id);
        self.tokens.remove(&id);
        Some(id)
    }
    pub fn token(&mut self, id: u32, token: String) {
        self.tokens.insert(id, token);
    }
    pub fn clicked(&mut self, id: u32) -> Option<Click> {
        let (_, target) = *self.targets.get(&id)?;
        Some(Click { target, token: self.tokens.remove(&id) })
    }
    pub fn closed(&mut self, id: u32) {
        if let Some((tag, _)) = self.targets.remove(&id)
            && self.by_tag.get(&tag) == Some(&id)
        {
            self.by_tag.remove(&tag);
        }
        self.tokens.remove(&id);
    }
}

pub fn escape_markup(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

type Image = Arc<(i32, i32, Vec<u8>)>;

pub fn circle(rgba: &mut [u8], size: u32) {
    let r = size as f32 / 2.;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let cover = (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0., 1.);
            let a = &mut rgba[((y * size + x) * 4 + 3) as usize];
            *a = (*a as f32 * cover).round() as u8;
        }
    }
}

pub fn avatar_image(bytes: &[u8], round: bool) -> Option<(i32, i32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?;
    let side = img.width().min(img.height()).min(128);
    let img = img.resize_to_fill(side, side, image::imageops::FilterType::Triangle);
    let mut rgba = img.to_rgba8().into_raw();
    if round {
        circle(&mut rgba, side);
    }
    Some((side as i32, side as i32, rgba))
}

async fn fetch(http: &reqwest::Client, cache: &mut HashMap<String, Image>, url: &str, round: bool) -> Option<Image> {
    let key = format!("{}{url}", if round { "o:" } else { "s:" });
    if let Some(i) = cache.get(&key) {
        return Some(i.clone());
    }
    let r = http.get(url).send().await.ok()?;
    if !r.status().is_success() {
        return None;
    }
    let img = Arc::new(avatar_image(&r.bytes().await.ok()?, round)?);
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.insert(key, img.clone());
    Some(img)
}

pub fn hints(image: Option<&Image>) -> HashMap<&'static str, Value<'static>> {
    let mut h: HashMap<&'static str, Value<'static>> = HashMap::new();
    h.insert("desktop-entry", Value::from(crate::APP_ID));
    h.insert("category", Value::from("im.received"));
    h.insert("suppress-sound", Value::from(true));
    h.insert("urgency", Value::U8(1));
    if let Some(i) = image {
        let (w, hgt, data) = (**i).clone();
        h.insert("image-data", Value::Structure(Structure::from((w, hgt, w * 4, true, 8i32, 4i32, data))));
    }
    h
}

fn args(m: &zbus::Message) -> Option<(u32, String)> {
    m.body().deserialize::<(u32, String)>().ok()
}

fn closed_id(m: &zbus::Message) -> Option<u32> {
    m.body().deserialize::<(u32, u32)>().ok().map(|(id, _)| id)
}

async fn serve(mut rx: mpsc::UnboundedReceiver<Cmd>, clicks: mpsc::UnboundedSender<Click>) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(&conn, "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.freedesktop.Notifications").await?;
    let caps: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap_or_default();
    let markup = caps.iter().any(|c| c == "body-markup");
    let actions: Vec<&str> = if caps.iter().any(|c| c == "actions") { vec!["default", ""] } else { Vec::new() };
    let mut invoked = proxy.receive_signal("ActionInvoked").await?;
    let mut tokens = proxy.receive_signal("ActivationToken").await?;
    let mut closed = proxy.receive_signal("NotificationClosed").await?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().map_err(|e| zbus::Error::Failure(e.to_string()))?;
    let mut images: HashMap<String, Image> = HashMap::new();
    let mut live = Live::default();
    loop {
        tokio::select! {
            biased;
            Some(m) = tokens.next() => {
                if let Some((id, token)) = args(&m) {
                    live.token(id, token);
                }
            }
            Some(m) = invoked.next() => {
                if let Some((id, key)) = args(&m)
                    && key == "default"
                    && let Some(c) = live.clicked(id)
                {
                    let _ = clicks.unbounded_send(c);
                }
            }
            Some(m) = closed.next() => {
                if let Some(id) = closed_id(&m) {
                    live.closed(id);
                }
            }
            cmd = rx.next() => match cmd {
                None => return Ok(()),
                Some(Cmd::Close(tag)) => {
                    if let Some(id) = live.take_tag(tag) {
                        let _: zbus::Result<()> = proxy.call("CloseNotification", &(id,)).await;
                    }
                }
                Some(Cmd::Show(t)) => {
                    let image = match &t.icon {
                        Some(u) => fetch(&http, &mut images, u, t.round).await,
                        None => None,
                    };
                    let body = if markup { escape_markup(&t.body) } else { t.body.clone() };
                    let args = ("Discord Phoenix", live.replaces(t.tag), crate::APP_ID, t.title.as_str(), body.as_str(), &actions, hints(image.as_ref()), -1i32);
                    match proxy.call::<_, _, u32>("Notify", &args).await {
                        Ok(id) => live.shown(t.tag, id, t.target),
                        Err(e) => tracing::warn!("notification failed: {e}"),
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_notification_per_channel() {
        let mut l = Live::default();
        assert_eq!(l.replaces(5), 0);
        l.shown(5, 10, Target::Channel(5));
        assert_eq!(l.replaces(5), 10);
        l.shown(5, 10, Target::Channel(5));
        l.token(10, "tok".into());
        assert_eq!(l.clicked(10), Some(Click { target: Target::Channel(5), token: Some("tok".into()) }));
        assert_eq!(l.clicked(10).and_then(|c| c.token), None);
        l.shown(6, 11, Target::Channel(6));
        assert_eq!(l.take_tag(5), Some(10));
        assert_eq!(l.take_tag(5), None);
        assert_eq!(l.clicked(10), None);
        l.closed(11);
        assert_eq!(l.replaces(6), 0);
        l.shown(6, 12, Target::Channel(6));
        l.shown(6, 13, Target::Channel(6));
        l.closed(12);
        assert_eq!(l.replaces(6), 13);
    }

    #[test]
    fn markup_and_circle() {
        assert_eq!(escape_markup("a <b> & c"), "a &lt;b&gt; &amp; c");
        let mut px = vec![255u8; 16 * 16 * 4];
        circle(&mut px, 16);
        assert_eq!(px[3], 0, "corner is transparent");
        assert_eq!(px[((8 * 16 + 8) * 4 + 3) as usize], 255, "centre is opaque");
        let h = hints(Some(&Arc::new((1, 1, vec![0, 0, 0, 255]))));
        assert_eq!(h["desktop-entry"], Value::from(crate::APP_ID));
        assert!(matches!(h["image-data"], Value::Structure(_)));
        assert_eq!(h["suppress-sound"], Value::from(true));
    }
}
