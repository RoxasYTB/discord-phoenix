use std::path::{Path, PathBuf};

use gpui::Context;

pub fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("json") => "application/json",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        _ => "image/jpeg",
    }
}

pub fn encode(bytes: &[u8], mime: &str) -> String {
    use base64::Engine;
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn data_uri(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(encode(&bytes, mime_of(path)))
}

pub fn decode(uri: &str) -> Option<std::sync::Arc<gpui::Image>> {
    use std::{
        cell::RefCell,
        collections::HashMap,
        hash::{Hash, Hasher},
    };

    use base64::Engine;
    thread_local! {
        static CACHE: RefCell<HashMap<u64, std::sync::Arc<gpui::Image>>> = RefCell::new(HashMap::new());
    }
    let (mime, body) = uri.strip_prefix("data:")?.split_once(";base64,")?;
    let format = match mime {
        "image/png" => gpui::ImageFormat::Png,
        "image/gif" => gpui::ImageFormat::Gif,
        "image/webp" => gpui::ImageFormat::Webp,
        "image/jpeg" | "image/jpg" => gpui::ImageFormat::Jpeg,
        _ => return None,
    };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    uri.hash(&mut h);
    let key = h.finish();
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(hit);
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(body).ok()?;
    let image = std::sync::Arc::new(gpui::Image::from_bytes(format, bytes));
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 32 {
            c.clear();
        }
        c.insert(key, image.clone());
    });
    Some(image)
}

pub fn accepts(path: &Path, exts: &[&str]) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

pub const IMAGES: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
pub const STICKERS: [&str; 5] = ["png", "apng", "gif", "json", "webp"];
pub const SOUNDS: [&str; 2] = ["mp3", "ogg"];

pub fn pick<T: 'static>(exts: &'static [&'static str], cx: &mut Context<T>, done: impl FnOnce(&mut T, PathBuf, &mut Context<T>) + 'static) {
    let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(Some(mut paths))) = rx.await
            && let Some(p) = paths.pop()
            && accepts(&p, exts)
        {
            let _ = this.update(cx, |t, cx| {
                done(t, p, cx);
                cx.notify();
            });
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_uris_use_the_file_type() {
        assert_eq!(encode(b"abc", "image/png"), "data:image/png;base64,YWJj");
        assert_eq!(mime_of(Path::new("a.PNG")), "image/png");
        assert_eq!(mime_of(Path::new("a.mp3")), "audio/mpeg");
        assert!(accepts(Path::new("x.Webp"), &IMAGES));
        assert!(!accepts(Path::new("x.exe"), &IMAGES));
        assert!(accepts(Path::new("x.json"), &STICKERS));
    }

    #[test]
    fn data_uris_decode_back_to_images() {
        let img = decode("data:image/png;base64,YWJj").unwrap();
        assert_eq!(img.format, gpui::ImageFormat::Png);
        assert_eq!(img.bytes, b"abc");
        assert!(decode("data:text/plain;base64,YWJj").is_none());
        assert!(decode("abc").is_none());
    }
}
