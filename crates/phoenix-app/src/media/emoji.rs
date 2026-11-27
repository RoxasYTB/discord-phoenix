use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, StyledImage, Window, div, img, prelude::*, px};

pub const TWEMOJI_BASE: &str = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@16.0.1/assets/72x72";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomEmoji {
    pub name: SharedString,
    pub id: u64,
    pub animated: bool,
}

const CDN_SIZES: [u32; 17] = [16, 20, 22, 24, 28, 32, 40, 44, 48, 56, 60, 64, 80, 96, 100, 128, 160];

pub fn cdn_size(want: u32) -> u32 {
    CDN_SIZES.iter().copied().find(|s| *s >= want).unwrap_or_else(|| want.next_power_of_two().min(4096))
}

impl CustomEmoji {
    pub fn url(&self, size: u32) -> SharedString {
        if let Some(d) = LOCAL_DIR.get() {
            let ext = if self.animated { "gif" } else { "png" };
            return format!("local/{d}/{}.{ext}", self.id).into();
        }
        let anim = if self.animated { "&animated=true" } else { "" };
        format!("https://cdn.discordapp.com/emojis/{}.webp?size={}{anim}", self.id, cdn_size(size)).into()
    }

    pub fn static_url(&self, size: u32) -> SharedString {
        if let Some(d) = LOCAL_DIR.get() {
            return format!("local/{d}/{}.png", self.id).into();
        }
        format!("https://cdn.discordapp.com/emojis/{}.webp?size={}", self.id, cdn_size(size)).into()
    }

    pub fn unsized_url(&self, play: bool) -> Option<SharedString> {
        if LOCAL_DIR.get().is_some() {
            return None;
        }
        let ext = if self.animated && play { "gif" } else { "png" };
        Some(format!("https://cdn.discordapp.com/emojis/{}.{ext}", self.id).into())
    }
}

static LOCAL_DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_local_dir(dir: String) {
    let _ = LOCAL_DIR.set(dir);
}

static ANIMATE_EMOJI: AtomicBool = AtomicBool::new(true);
static WINDOW_FOCUSED: AtomicBool = AtomicBool::new(true);

pub fn sync_motion(animate_emoji: bool, focused: bool) {
    ANIMATE_EMOJI.store(animate_emoji, Ordering::Relaxed);
    WINDOW_FOCUSED.store(focused, Ordering::Relaxed);
}

pub fn window_focused() -> bool {
    WINDOW_FOCUSED.load(Ordering::Relaxed)
}

pub fn animate_emoji() -> bool {
    ANIMATE_EMOJI.load(Ordering::Relaxed)
}

pub fn should_play(auto: bool, hovered: bool) -> bool {
    WINDOW_FOCUSED.load(Ordering::Relaxed) && ((auto && ANIMATE_EMOJI.load(Ordering::Relaxed)) || hovered)
}

#[derive(IntoElement)]
pub struct CustomEmojiImage {
    emoji: CustomEmoji,
    size: f32,
    auto: bool,
}

impl CustomEmojiImage {
    pub fn new(emoji: CustomEmoji, size: f32, auto: bool) -> Self {
        Self { emoji, size, auto }
    }
}

impl RenderOnce for CustomEmojiImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { emoji, size, auto } = self;
        let px_size = (size * 2.) as u32;
        let fb: SharedString = format!(":{}:", emoji.name).into();
        let text = move || div().text_size(px(size * 0.9)).child(fb.clone()).into_any_element();
        let retry = move |src: Option<SharedString>| {
            let text = text.clone();
            move || match src.clone() {
                Some(u) => img(u).size(px(size)).flex_none().with_fallback(text.clone()).into_any_element(),
                None => text(),
            }
        };
        if !emoji.animated {
            let fallback = retry(emoji.unsized_url(false));
            return img(emoji.url(px_size)).size(px(size)).flex_none().with_fallback(fallback).into_any_element();
        }
        let key = ("phx-emoji", emoji.id as usize);
        let hovered = window.use_keyed_state(key, cx, |_, _| false);
        let play = should_play(auto, *hovered.read(cx));
        let src = if play { emoji.url(px_size) } else { emoji.static_url(px_size) };
        let fallback = retry(emoji.unsized_url(play));
        div()
            .id(key)
            .flex_none()
            .size(px(size))
            .on_hover(move |h, _, cx| {
                hovered.update(cx, |s, cx| {
                    if *s != *h {
                        *s = *h;
                        cx.notify();
                    }
                })
            })
            .child(img(src.clone()).id(gpui::ElementId::Name(src)).size(px(size)).with_fallback(fallback))
            .into_any_element()
    }
}

pub fn parse_custom(s: &str) -> Option<CustomEmoji> {
    let inner = s.strip_prefix('<')?.strip_suffix('>')?;
    let (animated, rest) = match inner.strip_prefix("a:") {
        Some(r) => (true, r),
        None => (false, inner.strip_prefix(':')?),
    };
    let (name, id) = rest.split_once(':')?;
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(CustomEmoji { name: name.to_owned().into(), id: id.parse().ok()?, animated })
}

pub fn find_custom(text: &str) -> Vec<(std::ops::Range<usize>, CustomEmoji)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(start) = text[i..].find('<').map(|p| p + i) {
        match text[start..].find('>').map(|p| p + start + 1) {
            Some(end) => {
                if let Some(e) = parse_custom(&text[start..end]) {
                    out.push((start..end, e));
                    i = end;
                } else {
                    i = start + 1;
                }
            }
            None => break,
        }
    }
    out
}

struct Table {
    canon: std::collections::HashMap<&'static str, &'static str>,
    starts: std::collections::HashSet<char>,
    max_chars: usize,
}

fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let mut canon = std::collections::HashMap::new();
        let extra: Vec<&'static str> =
            (0x1F1E6..=0x1F1FF).chain(0x1F3FB..=0x1F3FF).filter_map(char::from_u32).map(|c| leak(c.to_string())).collect();
        for s in super::emoji_seqs::SEQUENCES.iter().copied().chain(extra) {
            canon.insert(s, s);
            if !s.contains('\u{200D}') && s.contains('\u{FE0F}') {
                canon.entry(leak(s.replace('\u{FE0F}', ""))).or_insert(s);
            }
        }
        if let Some(eye) = super::emoji_seqs::SEQUENCES.iter().copied().find(|s| s.starts_with("\u{1F441}\u{FE0F}\u{200D}")) {
            canon.insert("\u{1F441}\u{200D}\u{1F5E8}", eye);
        }
        let starts = canon.keys().filter_map(|k| k.chars().next()).collect();
        let max_chars = canon.keys().map(|k| k.chars().count()).max().unwrap_or(1);
        Table { canon, starts, max_chars }
    })
}

pub fn unicode_emoji_at(text: &str, i: usize) -> Option<usize> {
    let t = table();
    let rest = &text[i..];
    if !rest.chars().next().is_some_and(|c| t.starts.contains(&c)) {
        return None;
    }
    let ends: Vec<usize> = rest.char_indices().skip(1).map(|(j, _)| j).chain([rest.len()]).take(t.max_chars).collect();
    let end = ends.into_iter().rev().find(|&e| t.canon.contains_key(&rest[..e]))?;
    let end = if rest[end..].starts_with('\u{FE0F}') { end + 3 } else { end };
    Some(i + end)
}

fn canonical(e: &str) -> Option<&'static str> {
    let t = table();
    t.canon.get(e).or_else(|| t.canon.get(e.strip_suffix('\u{FE0F}')?)).copied()
}

pub fn twemoji_code(e: &str) -> Option<String> {
    let e = canonical(e)?;
    let eye = e.starts_with("\u{1F441}\u{FE0F}\u{200D}");
    let keep = e.contains('\u{200D}') && !eye;
    let parts: Vec<String> = e.chars().filter(|&c| keep || c != '\u{FE0F}').map(|c| format!("{:x}", c as u32)).collect();
    Some(parts.join("-"))
}

pub fn twemoji_url(e: &str) -> Option<SharedString> {
    twemoji_code(e).map(|c| format!("{TWEMOJI_BASE}/{c}.png").into())
}

pub fn emoji_element(token: &str, size: f32) -> AnyElement {
    emoji_element_with(token, size, true)
}

pub fn emoji_element_hover(token: &str, size: f32) -> AnyElement {
    emoji_element_with(token, size, false)
}

fn emoji_element_with(token: &str, size: f32, auto: bool) -> AnyElement {
    if let Some(e) = parse_custom(token) {
        return CustomEmojiImage::new(e, size, auto).into_any_element();
    }
    let url = twemoji_url(token);
    let text: SharedString = token.to_owned().into();
    match url {
        Some(u) => {
            let fb = text.clone();
            img(u)
                .size(px(size))
                .flex_none()
                .with_fallback(move || div().text_size(px(size * 0.9)).child(fb.clone()).into_any_element())
                .into_any_element()
        }
        None => div().text_size(px(size)).child(text).into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_tokens() {
        assert_eq!(parse_custom("<:blob:123>"), Some(CustomEmoji { name: "blob".into(), id: 123, animated: false }));
        assert!(parse_custom("<a:dance:9>").unwrap().animated);
        assert_eq!(parse_custom("<@123>"), None);
        let found = find_custom("hi <:a:1> and <a:b:2> <#3>");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, 3..9);
    }

    #[test]
    fn client_cdn_urls() {
        let e = CustomEmoji { name: "dance".into(), id: 9, animated: true };
        assert_eq!(e.url(48).as_ref(), "https://cdn.discordapp.com/emojis/9.webp?size=48&animated=true");
        assert_eq!(e.static_url(48).as_ref(), "https://cdn.discordapp.com/emojis/9.webp?size=48");
        let s = CustomEmoji { animated: false, ..e };
        assert_eq!(s.url(60).as_ref(), "https://cdn.discordapp.com/emojis/9.webp?size=60");
    }

    #[test]
    fn cdn_sizes() {
        assert_eq!(cdn_size(38), 40);
        assert_eq!(cdn_size(44), 44);
        assert_eq!(cdn_size(1), 16);
        assert_eq!(cdn_size(200), 256);
    }

    #[test]
    fn unsized_fallback_keeps_the_format() {
        let e = CustomEmoji { name: "x".into(), id: 7, animated: true };
        assert_eq!(e.unsized_url(true).as_deref(), Some("https://cdn.discordapp.com/emojis/7.gif"));
        assert_eq!(e.unsized_url(false).as_deref(), Some("https://cdn.discordapp.com/emojis/7.png"));
    }

    #[test]
    fn twemoji_codes() {
        assert_eq!(twemoji_code("👍").as_deref(), Some("1f44d"));
        assert_eq!(twemoji_code("❤️").as_deref(), Some("2764"));
        assert_eq!(twemoji_code("👩‍💻").as_deref(), Some("1f469-200d-1f4bb"));
        assert_eq!(twemoji_code("a"), None);
        assert_eq!(twemoji_code("\u{1FAE9}").as_deref(), Some("1fae9"));
        assert_eq!(twemoji_code("☺").as_deref(), Some("263a"));
        assert_eq!(twemoji_code("#⃣").as_deref(), Some("23-20e3"));
        assert_eq!(twemoji_code("🏳️‍🌈").as_deref(), Some("1f3f3-fe0f-200d-1f308"));
        assert_eq!(twemoji_code("👁️‍🗨️").as_deref(), Some("1f441-200d-1f5e8"));
        assert_eq!(twemoji_code("👍🏽").as_deref(), Some("1f44d-1f3fd"));
        assert_eq!(twemoji_code("🏴󠁧󠁢󠁳󠁣󠁴󠁿").as_deref(), Some("1f3f4-e0067-e0062-e0073-e0063-e0074-e007f"));
        for t in ["→", "★", "∞", "✓", "■", "♡", "1"] {
            assert_eq!(twemoji_code(t), None, "{t}");
        }
    }
}
