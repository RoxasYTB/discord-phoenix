//! Remote media: disk cache, CDN URL helpers, emoji images, and shared
//! view state (revealed spoilers, the image viewer overlay).
//!
//! Everything here is drawn with `img(url)`, which only fetches once the
//! element is painted. The message list is virtualized, so only visible rows
//! ever hit the network.

pub mod cache;
pub mod emoji;
pub mod emoji_seqs;
pub mod lottie;
pub mod lru;

use std::collections::HashSet;

use gpui::{AnyElement, App, Global, ParentElement, SharedString, Styled, div, img, prelude::*, px};

/// Discord's attachment thumbnail box.
pub const MAX_W: f32 = 550.;
pub const MAX_H: f32 = 350.;

/// Fit `w x h` inside `max_w x max_h`, never upscaling. Returns at least 1px.
pub fn fit(w: u32, h: u32, max_w: f32, max_h: f32) -> (f32, f32) {
    if w == 0 || h == 0 {
        return (max_w.min(400.), max_h.min(225.));
    }
    let s = (max_w / w as f32).min(max_h / h as f32).min(1.);
    ((w as f32 * s).round().max(1.), (h as f32 * s).round().max(1.))
}

fn is_media_proxy(url: &str) -> bool {
    url.starts_with("https://media.discordapp.net/") || url.starts_with("https://images-ext-")
}

/// Rewrite an attachment/embed URL so Discord's media proxy serves it at
/// `w x h` (pass 2x the drawn size for HiDPI). Non-proxy URLs are returned
/// with only the cdn host swapped to the proxy when possible.
pub fn resized(url: &str, w: u32, h: u32) -> SharedString {
    let base = url
        .strip_prefix("https://cdn.discordapp.com/attachments/")
        .map(|rest| format!("https://media.discordapp.net/attachments/{rest}"))
        .unwrap_or_else(|| url.to_owned());
    if !is_media_proxy(&base) || w == 0 || h == 0 {
        return base.into();
    }
    let stripped = strip_params(&base, &["width", "height", "format"]);
    let sep = if stripped.contains('?') { '&' } else { '?' };
    format!("{stripped}{sep}width={w}&height={h}").into()
}

/// First frame of a video as a still image (media proxy `format=webp`).
pub fn video_poster(url: &str, w: u32, h: u32) -> SharedString {
    let r = resized(url, w, h);
    if !is_media_proxy(&r) {
        return r;
    }
    let sep = if r.contains('?') { '&' } else { '?' };
    format!("{r}{sep}format=webp").into()
}

/// Animated GIF variant of a gifv embed (Tenor/Giphy) that GPUI can play.
/// Discord plays the embed's mp4; GPUI only animates GIF/WebP, so Tenor
/// links are rewritten to the smallest gif rendition that covers the displayed
/// width `w` (tinygif `AAAAM` ≤ 220 px, else mediumgif `AAAAd`; the full `AAAAC`
/// gif is ~2x the decoded memory for no visible gain) and Giphy's `giphy.mp4`
/// to the animated WebP (`200w.webp` ≤ 200 px, else `giphy.webp`), which GPUI
/// decodes and is about half the size of the gif. Proxied URLs
/// (`images-ext-*/external/<sig>/https/media.tenor.com/...`) are unwrapped.
pub fn gifv_animated(url: &str, w: f32) -> Option<SharedString> {
    let url = url.split('?').next().unwrap_or(url);
    for host in ["media.tenor.com/", "media1.tenor.com/", "c.tenor.com/"] {
        if let Some(i) = url.find(host) {
            let mut parts = url[i + host.len()..].split('/').filter(|s| !s.is_empty() && *s != "m");
            let (code, slug) = (parts.next()?, parts.next()?);
            if code.len() < 6 || !code.is_ascii() {
                return None;
            }
            let stem = slug.rsplit_once('.').map_or(slug, |(s, _)| s);
            let variant = if w <= 220. { "AAAAM" } else { "AAAAd" };
            return Some(format!("https://media.tenor.com/{}{variant}/{stem}.gif", &code[..code.len() - 5]).into());
        }
    }
    // Klipy (the client's GIF provider since 2026): the embed thumbnail is the
    // provider's animated WebP (live: 72 frames, ~160 KB), the video an mp4.
    if let Some(i) = url.find("static.klipy.com/") {
        let path = &url[i..];
        return [".webp", ".gif"].iter().any(|x| path.ends_with(x)).then(|| format!("https://{path}").into());
    }
    if let Some(i) = url.find("giphy.com/media/") {
        let rest = &url[i + "giphy.com/media/".len()..];
        let id = rest.split('/').next().filter(|s| !s.is_empty())?;
        let file = if w <= 200. { "200w.webp" } else { "giphy.webp" };
        return Some(format!("https://media.giphy.com/media/{id}/{file}").into());
    }
    None
}

fn strip_params(url: &str, names: &[&str]) -> String {
    let Some((base, q)) = url.split_once('?') else { return url.to_owned() };
    let kept: Vec<&str> = q
        .split('&')
        .filter(|kv| !names.iter().any(|n| kv.split('=').next() == Some(n)))
        .filter(|kv| !kv.is_empty())
        .collect();
    if kept.is_empty() { base.to_owned() } else { format!("{base}?{}", kept.join("&")) }
}

/// Static variant of an animated (`a_` hash) avatar/icon URL.
pub fn static_variant(url: &str) -> SharedString {
    if let Some(i) = url.find(".gif") {
        let mut s = url.to_owned();
        s.replace_range(i..i + 4, ".png");
        return s.into();
    }
    url.to_owned().into()
}

pub fn is_animated(url: &str) -> bool {
    url.contains(".gif")
}

/// Round avatar image that plays its GIF only while hovered (Discord shows
/// the static frame otherwise).
pub fn avatar_image(url: SharedString, size: f32) -> AnyElement {
    if !is_animated(&url) {
        return img(url).size(px(size)).rounded_full().flex_none().into_any_element();
    }
    let group = SharedString::new_static("phoenix-avatar");
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .group(group.clone())
        .child(img(static_variant(&url)).size(px(size)).rounded_full())
        .child(
            img(url.clone())
                .id(anim_id(&url))
                .absolute()
                .top_0()
                .left_0()
                .size(px(size))
                .rounded_full()
                .invisible()
                .group_hover(group, |s| s.visible()),
        )
        .into_any_element()
}

thread_local! {
    /// The GIF under the pointer (gifv embed or GIF attachment), which plays even
    /// with "Play GIFs when Discord is focused" off, like the client.
    static HOVERED_GIF: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
    /// The sticker under the pointer, which plays when "Sticker animations" is set
    /// to "Animate on interaction" (`animate_stickers == 1`).
    static HOVERED_STICKER: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
}

/// Whether the GIF keyed `key` should play: autoplay on, or hovered. GPUI then
/// only advances frames while the window is active (focus pause, as the client).
pub fn gif_plays(key: &SharedString) -> bool {
    crate::ui::settings::effects::gif_autoplay() || HOVERED_GIF.with(|h| h.borrow().as_ref() == Some(key))
}

/// Wraps a GIF so hovering it plays it when autoplay is off (re-render on change).
pub fn gif_hover(key: SharedString, child: impl IntoElement) -> gpui::Stateful<gpui::Div> {
    div().id(gpui::ElementId::Name(format!("gif-hover-{key}").into())).child(child).on_hover(move |h, window, _| {
        let changed = HOVERED_GIF.with(|cur| {
            let mut cur = cur.borrow_mut();
            let next = if *h { Some(key.clone()) } else if cur.as_ref() == Some(&key) { None } else { cur.clone() };
            let changed = *cur != next;
            *cur = next;
            changed
        });
        if changed && !crate::ui::settings::effects::gif_autoplay() {
            window.refresh();
        }
    })
}

/// Whether the sticker keyed `key` should play: Always (0), or On interaction (1) while hovered.
pub fn sticker_plays(key: &SharedString) -> bool {
    crate::ui::settings::effects::sticker_autoplay()
        || (crate::ui::settings::effects::sticker_hover_plays()
            && HOVERED_STICKER.with(|h| h.borrow().as_ref() == Some(key)))
}

/// Wraps an animated sticker so hovering it plays it when "Sticker animations"
/// is set to "Animate on interaction" (re-render on change).
pub fn sticker_hover(key: SharedString, child: impl IntoElement) -> gpui::Stateful<gpui::Div> {
    div().id(gpui::ElementId::Name(format!("sticker-hover-{key}").into())).child(child).on_hover(move |h, window, _| {
        let changed = HOVERED_STICKER.with(|cur| {
            let mut cur = cur.borrow_mut();
            let next = if *h { Some(key.clone()) } else if cur.as_ref() == Some(&key) { None } else { cur.clone() };
            let changed = *cur != next;
            *cur = next;
            changed
        });
        if changed && crate::ui::settings::effects::sticker_hover_plays() {
            window.refresh();
        }
    })
}

/// GPUI only plays frames of an `img` that has an element id (its frame index
/// and timer live in the element state); an id-less `img` of a GIF or animated
/// WebP stays on its first frame forever. Every image that may animate gets one.
pub fn anim_id(src: &SharedString) -> gpui::ElementId {
    gpui::ElementId::Name(src.clone())
}

/// Playing URL of a sticker in a message. APNG stickers come as the CDN's
/// animated WebP (`.webp?animated=true`, measured: 25 frames, 180 KB against
/// 471 KB for the APNG), since GPUI decodes APNG as a single frame.
pub fn sticker_play_url(id: u64, format: StickerFormat) -> Option<SharedString> {
    match format {
        StickerFormat::Apng => Some(format!("https://media.discordapp.net/stickers/{id}.webp?size=320&animated=true").into()),
        _ => sticker_url(id, format),
    }
}

pub fn sticker_url(id: u64, format: StickerFormat) -> Option<SharedString> {
    let ext = match format {
        StickerFormat::Png | StickerFormat::Apng => "png",
        StickerFormat::Gif => "gif",
        StickerFormat::Lottie => return None,
    };
    Some(format!("https://media.discordapp.net/stickers/{id}.{ext}?size=320").into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StickerFormat {
    #[default]
    Png,
    Apng,
    Lottie,
    Gif,
}

/// Image currently shown full-window.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewerImage {
    pub url: SharedString,
    pub original: SharedString,
    pub filename: SharedString,
    pub width: u32,
    pub height: u32,
    /// Message author shown in the viewer's top bar (`topBar_` author block).
    pub author: Option<ViewerAuthor>,
}

/// Author block of the media viewer: avatar, coloured name, message time.
#[derive(Clone, Debug)]
pub struct ViewerAuthor {
    pub user: Option<crate::model::User>,
    pub name: SharedString,
    pub color: gpui::Hsla,
    pub time: SharedString,
}

impl PartialEq for ViewerAuthor {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name && self.time == o.time && self.user.as_ref().map(|u| u.id) == o.user.as_ref().map(|u| u.id)
    }
}

thread_local! {
    static CURRENT_AUTHOR: std::cell::RefCell<Option<ViewerAuthor>> = const { std::cell::RefCell::new(None) };
}

/// Set while a message's media is being built, so clicks open the viewer with its author.
pub fn set_current_author(a: Option<ViewerAuthor>) {
    CURRENT_AUTHOR.with(|c| *c.borrow_mut() = a);
}

pub fn current_author() -> Option<ViewerAuthor> {
    CURRENT_AUTHOR.with(|c| c.borrow().clone())
}

#[derive(Default)]
pub struct MediaState {
    pub revealed: HashSet<SharedString>,
    pub viewer: Option<ViewerImage>,
}

impl Global for MediaState {}

pub fn is_revealed(key: &SharedString, cx: &App) -> bool {
    crate::ui::settings::effects::spoilers_always() || cx.try_global::<MediaState>().is_some_and(|s| s.revealed.contains(key))
}

pub fn reveal(key: SharedString, cx: &mut App) {
    cx.default_global::<MediaState>().revealed.insert(key);
    cx.refresh_windows();
}

pub fn open_viewer(img: ViewerImage, cx: &mut App) {
    cx.default_global::<MediaState>().viewer = Some(img);
    cx.refresh_windows();
}

pub fn close_viewer(cx: &mut App) -> bool {
    let had = cx.try_global::<MediaState>().is_some_and(|s| s.viewer.is_some());
    if had {
        cx.default_global::<MediaState>().viewer = None;
        cx.refresh_windows();
    }
    had
}

#[cfg(test)]
mod tests {
    #[test]
    fn gifv_animated_rewrites_tenor_and_giphy() {
        let g = |u: &str| super::gifv_animated(u, 400.).map(|s| s.to_string());
        assert_eq!(g("https://media.tenor.com/6n0lkxsBEQkAAAPo/cat.mp4").as_deref(), Some("https://media.tenor.com/6n0lkxsBEQkAAAAd/cat.gif"));
        assert_eq!(
            g("https://images-ext-1.discordapp.net/external/abc/https/media.tenor.com/6n0lkxsBEQkAAAAe/cat.png?width=10").as_deref(),
            Some("https://media.tenor.com/6n0lkxsBEQkAAAAd/cat.gif")
        );
        assert_eq!(
            super::gifv_animated("https://media.tenor.com/6n0lkxsBEQkAAAPo/cat.mp4", 200.).as_deref(),
            Some("https://media.tenor.com/6n0lkxsBEQkAAAAM/cat.gif")
        );
        assert_eq!(g("https://media.giphy.com/media/xyz/giphy.mp4").as_deref(), Some("https://media.giphy.com/media/xyz/giphy.webp"));
        assert_eq!(
            super::gifv_animated("https://media.giphy.com/media/xyz/giphy.mp4", 150.).as_deref(),
            Some("https://media.giphy.com/media/xyz/200w.webp")
        );
        assert_eq!(g("https://example.com/a.mp4"), None);
        // Klipy: the animated WebP thumbnail plays, the mp4 does not.
        assert_eq!(
            g("https://images-ext-1.discordapp.net/external/q/https/static.klipy.com/ii/k/4f/0f/x.webp?format=webp&width=4").as_deref(),
            Some("https://static.klipy.com/ii/k/4f/0f/x.webp")
        );
        assert_eq!(g("https://static.klipy.com/ii/k/4f/0f/y.mp4"), None);
    }

    use super::*;

    #[test]
    fn fit_keeps_ratio_and_caps() {
        assert_eq!(fit(1920, 1080, MAX_W, MAX_H), (550., 309.));
        assert_eq!(fit(1000, 2000, MAX_W, MAX_H), (175., 350.));
        assert_eq!(fit(100, 50, MAX_W, MAX_H), (100., 50.));
    }

    #[test]
    fn resize_goes_through_media_proxy() {
        assert_eq!(
            resized("https://cdn.discordapp.com/attachments/1/2/a.png?ex=1&width=5", 100, 50).as_ref(),
            "https://media.discordapp.net/attachments/1/2/a.png?ex=1&width=100&height=50"
        );
        assert!(video_poster("https://cdn.discordapp.com/attachments/1/2/a.mp4", 10, 10).ends_with("format=webp"));
    }

    #[test]
    fn static_avatar() {
        assert_eq!(
            static_variant("https://cdn.discordapp.com/avatars/1/a_x.gif?size=128").as_ref(),
            "https://cdn.discordapp.com/avatars/1/a_x.png?size=128"
        );
    }
}
