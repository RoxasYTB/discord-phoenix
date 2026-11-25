use crate::model::{Embed, EmbedMedia};

pub fn provider_of(e: &Embed) -> Option<&str> {
    let name = e.provider.as_deref();
    if matches!(name, Some("YouTube" | "TikTok")) {
        return name;
    }
    let host = e.video.as_ref().and_then(|v| reqwest::Url::parse(&v.url).ok()).and_then(|u| u.host_str().map(str::to_owned));
    match host.as_deref() {
        Some("www.youtube.com") => Some("YouTube"),
        Some("www.tiktok.com") => Some("TikTok"),
        _ => name,
    }
}

pub fn media_max(e: &Embed) -> (f32, f32) {
    let portrait = e.video.as_ref().is_some_and(|v| v.height > v.width);
    if e.provider.as_deref() == Some("TikTok") || (portrait && provider_of(e) == Some("YouTube")) { (400., 450.) } else { (400., 300.) }
}

pub fn sizing_media(e: &Embed) -> Option<&EmbedMedia> {
    e.image.as_ref().or(e.video.as_ref())
}

pub fn max_width(e: &Embed) -> Option<f32> {
    let o = sizing_media(e)?;
    let (mw, mh) = media_max(e);
    let (w, _) = crate::media::fit(o.width, o.height, mw, mh);
    let kind = e.kind.as_deref();
    if kind == Some("video") || w >= 300. {
        Some(w + 32.)
    } else if kind == Some("rich") && !e.images.is_empty() {
        Some(520.)
    } else {
        None
    }
}

pub fn border_color(e: &Embed) -> Option<u32> {
    e.color.filter(|c| *c != 0xFF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(w: u32, h: u32, url: &str) -> EmbedMedia {
        EmbedMedia { url: url.into(), width: w, height: h }
    }

    #[test]
    fn media_bounds_follow_the_client() {
        let landscape = Embed { kind: Some("video".into()), provider: Some("YouTube".into()), video: Some(media(1280, 720, "https://www.youtube.com/embed/x")), ..Default::default() };
        assert_eq!(media_max(&landscape), (400., 300.));
        assert_eq!(max_width(&landscape), Some(432.));
        let short = Embed { kind: Some("video".into()), provider: Some("YouTube".into()), video: Some(media(1080, 1920, "https://www.youtube.com/embed/y")), ..Default::default() };
        assert_eq!(media_max(&short), (400., 450.));
        assert_eq!(max_width(&short), Some(253. + 32.));
        let by_host = Embed { kind: Some("video".into()), video: Some(media(720, 1280, "https://www.youtube.com/embed/z")), ..Default::default() };
        assert_eq!(media_max(&by_host), (400., 450.));
        let tiktok = Embed { provider: Some("TikTok".into()), ..Default::default() };
        assert_eq!(media_max(&tiktok), (400., 450.));
    }

    #[test]
    fn embed_width_follows_the_client() {
        let article = Embed { kind: Some("article".into()), image: Some(media(200, 100, "https://x.y/a.png")), ..Default::default() };
        assert_eq!(max_width(&article), None);
        let wide = Embed { kind: Some("rich".into()), image: Some(media(1200, 600, "https://x.y/a.png")), ..Default::default() };
        assert_eq!(max_width(&wide), Some(432.));
        let gallery = Embed { kind: Some("rich".into()), image: Some(media(100, 100, "https://x.y/a.png")), images: vec![media(100, 100, "https://x.y/b.png")], ..Default::default() };
        assert_eq!(max_width(&gallery), Some(520.));
        assert_eq!(max_width(&Embed::default()), None);
        assert_eq!(border_color(&Embed { color: Some(0xFFFFFF), ..Default::default() }), None);
        assert_eq!(border_color(&Embed { color: Some(0xFF0000), ..Default::default() }), Some(0xFF0000));
    }
}
