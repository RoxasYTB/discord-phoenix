use std::{collections::HashSet, io::{Cursor, Write}, sync::Arc};

use futures::AsyncReadExt;
use gpui::{BackgroundExecutor, http_client::HttpClient};
use image::{AnimationDecoder, ImageDecoder, ImageFormat, codecs::{gif::{GifDecoder, GifEncoder, Repeat}, webp::WebPDecoder}};

use crate::{backend::BackendError, discord_backend::http::host_allowed, ui::pickers::types::GifItem};

pub async fn download(
    http: Arc<dyn HttpClient>,
    gif: GifItem,
    limit: u64,
    executor: &BackgroundExecutor,
) -> Result<(Vec<u8>, gpui::SharedString), BackendError> {
    let mut tried = HashSet::new();
    for url in gif.source.into_iter().chain([gif.preview, gif.url]) {
        if !host_allowed(&url) || !tried.insert(url.clone()) {
            continue;
        }
        let Ok(mut response) = http.get(&url, Default::default(), true).await else { continue };
        if !response.status().is_success() {
            continue;
        }
        let mut bytes = Vec::new();
        if response.body_mut().take(limit.saturating_add(1)).read_to_end(&mut bytes).await.is_err() || bytes.len() as u64 > limit {
            continue;
        }
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        if let Some(gif) = executor.spawn(async move { animated_gif(bytes, limit) }).await {
            return Ok((gif, url));
        }
    }
    Err(BackendError(crate::t!("picker.gif_no_animation").to_string()))
}

fn animated_gif(bytes: Vec<u8>, max_bytes: usize) -> Option<Vec<u8>> {
    if bytes.len() > max_bytes {
        return None;
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    match image::guess_format(&bytes).ok()? {
        ImageFormat::Gif => {
            let mut decoder = GifDecoder::new(Cursor::new(&bytes)).ok()?;
            decoder.set_limits(limits).ok()?;
            let mut frames = decoder.into_frames();
            frames.next()?.ok()?;
            frames.next()?.ok()?;
            drop(frames);
            Some(bytes)
        }
        ImageFormat::WebP => {
            let mut decoder = WebPDecoder::new(Cursor::new(&bytes)).ok()?;
            decoder.set_limits(limits).ok()?;
            if !decoder.has_animation() {
                return None;
            }
            let mut frames = decoder.into_frames();
            let first = frames.next()?.ok()?;
            let second = frames.next()?.ok()?;
            let mut out = BoundedOutput { bytes: Vec::new(), limit: max_bytes, overflowed: false };
            let mut encoder = GifEncoder::new_with_speed(&mut out, 10);
            encoder.set_repeat(Repeat::Infinite).ok()?;
            encoder.encode_frame(first).ok()?;
            encoder.encode_frame(second).ok()?;
            encoder.try_encode_frames(frames).ok()?;
            drop(encoder);
            (!out.overflowed).then_some(out.bytes)
        }
        _ => None,
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.overflowed = true;
            return Err(std::io::Error::other("GIF exceeds upload limit"));
        }
        self.bytes.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_frame_preview_never_becomes_an_attachment() {
        fn make(colors: &[[u8; 4]]) -> Vec<u8> {
            let mut data = Vec::new();
            let mut encoder = GifEncoder::new(&mut data);
            for color in colors {
                encoder.encode_frame(image::Frame::new(image::RgbaImage::from_pixel(2, 2, image::Rgba(*color)))).unwrap();
            }
            drop(encoder);
            data
        }
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        assert!(animated_gif(make(&[red]), 8192).is_none());
        let gif = make(&[red, blue]);
        assert_eq!(animated_gif(gif.clone(), 8192), Some(gif));
    }
}
