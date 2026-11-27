pub mod audio_file;
pub mod cache;
pub mod emoji;
pub mod emoji_seqs;
pub mod lottie;
pub mod lru;
pub mod video_file;

use std::collections::HashSet;

use gpui::{AnyElement, App, Global, ParentElement, SharedString, Styled, div, img, prelude::*, px};

pub const MAX_W: f32 = 550.;
pub const MAX_H: f32 = 350.;

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

pub fn video_poster(url: &str, w: u32, h: u32) -> SharedString {
    let r = resized(url, w, h);
    if !is_media_proxy(&r) {
        return r;
    }
    let sep = if r.contains('?') { '&' } else { '?' };
    format!("{r}{sep}format=webp").into()
}

pub fn gif_preview(url: &str, w: u32, h: u32, play: bool) -> SharedString {
    let resized = resized(url, w, h);
    if !is_media_proxy(&resized) {
        return resized;
    }
    let base = strip_params(&resized, &["format", "animated"]);
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}format=webp&animated={play}").into()
}

pub fn gif_image_sources(url: &str, original: &str, w: u32, h: u32, play: bool) -> Vec<SharedString> {
    let mut sources = Vec::new();
    if play && original.starts_with("https://cdn.discordapp.com/attachments/") && is_animated(original) {
        sources.push(original.into());
    }
    let preview = gif_preview(url, w, h, play);
    if !sources.contains(&preview) {
        sources.push(preview);
    }
    if play && !sources.iter().any(|source| source.as_ref() == url) {
        sources.push(url.into());
    }
    sources
}

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

pub fn signed_url_expired(url: &str, now: i64) -> bool {
    url.split(['?', '&']).skip(1).find_map(|p| p.strip_prefix("ex=")).and_then(|h| i64::from_str_radix(h, 16).ok()).is_some_and(|ex| ex <= now + 60)
}

pub fn static_variant(url: &str) -> SharedString {
    if let Some(i) = url.find(".gif") {
        let mut s = url.to_owned();
        s.replace_range(i..i + 4, ".png");
        return s.into();
    }
    url.to_owned().into()
}

pub fn is_animated(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("gif"))
        || url.split(['?', '&', '#']).skip(1).any(|param| param == "animated=true")
}

pub fn with_cdn_size(url: &str, size: u32) -> SharedString {
    let Some(q) = url.find("?size=").or_else(|| url.find("&size=")) else { return url.to_owned().into() };
    let start = q + "?size=".len();
    let end = url[start..].find('&').map_or(url.len(), |e| start + e);
    format!("{}{size}{}", &url[..start], &url[end..]).into()
}

pub fn avatar_image(url: SharedString, size: f32) -> AnyElement {
    if !is_animated(&url) {
        return lru::cached(img(url)).size(px(size)).rounded_full().flex_none().into_any_element();
    }
    let hovered = emoji::window_focused() && HOVERED_AVATAR.with(|h| h.borrow().as_ref() == Some(&url));
    let src = if hovered { url.clone() } else { static_variant(&url) };
    let mut im = img(src);
    if hovered && let Some(cache) = lru::chat() {
        im = im.image_cache(&cache);
    }
    let key = url.clone();
    div()
        .id(gpui::ElementId::Name(format!("avatar-hover-{url}").into()))
        .flex_none()
        .size(px(size))
        .child(im.id(anim_id(&url)).size(px(size)).rounded_full())
        .on_hover(move |h, window, _| {
            let changed = HOVERED_AVATAR.with(|cur| {
                let mut cur = cur.borrow_mut();
                let next = if *h { Some(key.clone()) } else if cur.as_ref() == Some(&key) { None } else { cur.clone() };
                let changed = *cur != next;
                *cur = next;
                changed
            });
            if changed {
                window.refresh();
            }
        })
        .into_any_element()
}

thread_local! {
    static HOVERED_AVATAR: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
    static HOVERED_GIF: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
    static HOVERED_STICKER: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
}

pub fn gif_plays(key: &SharedString) -> bool {
    emoji::window_focused()
        && !crate::ui::settings::effects::reduced_motion()
        && (crate::ui::settings::effects::gif_autoplay() || HOVERED_GIF.with(|h| h.borrow().as_ref() == Some(key)))
}

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

pub fn sticker_plays(key: &SharedString) -> bool {
    emoji::window_focused()
        && (crate::ui::settings::effects::sticker_autoplay()
            || (crate::ui::settings::effects::sticker_hover_plays()
                && HOVERED_STICKER.with(|h| h.borrow().as_ref() == Some(key))))
}

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

pub fn anim_id(src: &SharedString) -> gpui::ElementId {
    gpui::ElementId::Name(src.clone())
}

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

#[derive(Clone, Debug, PartialEq)]
pub struct ViewerImage {
    pub url: SharedString,
    pub original: SharedString,
    pub filename: SharedString,
    pub width: u32,
    pub height: u32,
    pub author: Option<ViewerAuthor>,
    pub video: Option<SharedString>,
}

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
    fn animated_previews_request_frames_and_preserve_attachment_signatures() {
        let url = "https://cdn.discordapp.com/attachments/1/2/a.GIF?ex=1&is=2&hm=3&format=png&animated=false";
        assert!(is_animated(url));
        assert!(!is_animated("https://example.com/preview.png?name=file.gif"));
        assert_eq!(
            gif_preview(url, 100, 50, true).as_ref(),
            "https://media.discordapp.net/attachments/1/2/a.GIF?ex=1&is=2&hm=3&width=100&height=50&format=webp&animated=true"
        );
        assert!(gif_preview(url, 100, 50, false).ends_with("format=webp&animated=false"));
        assert_eq!(gif_preview("https://media.tenor.com/id/a.gif", 100, 50, true).as_ref(), "https://media.tenor.com/id/a.gif");
    }

    #[test]
    fn animated_attachment_falls_back_to_discords_signed_proxy() {
        let original = "https://cdn.discordapp.com/attachments/1/2/a.gif?ex=1&is=2&hm=3";
        let proxy = "https://media.discordapp.net/attachments/1/2/a.gif?ex=4&is=5&hm=6";
        let sources = gif_image_sources(proxy, original, 480, 280, true);
        assert_eq!(sources.iter().map(|url| url.as_ref()).collect::<Vec<_>>(), [
            original,
            "https://media.discordapp.net/attachments/1/2/a.gif?ex=4&is=5&hm=6&width=480&height=280&format=webp&animated=true",
            proxy,
        ]);
        let still = gif_image_sources(proxy, original, 480, 280, false);
        assert_eq!(still.len(), 1);
        assert!(still[0].ends_with("animated=false"));
        assert_eq!(gif_image_sources(original, original, 480, 280, true).len(), 2);
    }

    #[test]
    fn signed_urls_expire_on_their_ex_param() {
        let u = "https://media.discordapp.net/attachments/1/2/card.png?ex=6b064152&is=6b04efd2&hm=00&width=1100";
        assert!(super::signed_url_expired(u, 0x6b064152));
        assert!(!super::signed_url_expired(u, 0x6b064152 - 3600));
        assert!(!super::signed_url_expired("https://cdn.discordapp.com/avatars/1/a.png?size=128", i64::MAX / 2));
    }

    #[test]
    fn cdn_size_is_replaced() {
        assert_eq!(super::with_cdn_size("https://cdn.discordapp.com/avatars/1/a.png?size=128", 32).as_ref(), "https://cdn.discordapp.com/avatars/1/a.png?size=32");
        assert_eq!(super::with_cdn_size("https://cdn.discordapp.com/a.webp?size=128&animated=true", 32).as_ref(), "https://cdn.discordapp.com/a.webp?size=32&animated=true");
        assert_eq!(super::with_cdn_size("https://cdn.discordapp.com/embed/avatars/2.png", 32).as_ref(), "https://cdn.discordapp.com/embed/avatars/2.png");
    }

    #[test]
    fn static_avatar() {
        assert_eq!(
            static_variant("https://cdn.discordapp.com/avatars/1/a_x.gif?size=128").as_ref(),
            "https://cdn.discordapp.com/avatars/1/a_x.png?size=128"
        );
    }
}
