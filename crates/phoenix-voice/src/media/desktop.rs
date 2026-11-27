use crate::{Error, Result};
use windows_sys::Win32::{
    Graphics::Gdi::{BI_RGB, BITMAP, BITMAPINFO, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, GetObjectW, SelectObject},
    UI::WindowsAndMessaging::{CURSOR_SHOWING, CURSORINFO, DI_NORMAL, DrawIconEx, GA_ROOT, GetAncestor, GetCursorInfo, GetIconInfo, ICONINFO, WindowFromPoint},
};
use xcap::image::{RgbaImage, imageops};
use xcap::{Monitor, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Window { id: u32, pid: u32 },
    Monitor(u32),
}

pub enum Capture {
    Window(Window),
    Monitor(Monitor),
}

fn failed(e: xcap::XCapError) -> Error {
    Error::Encoder(e.to_string())
}

impl Capture {
    pub fn list(windows: bool) -> Result<Vec<(Target, String, Self)>> {
        if windows {
            let own = std::process::id();
            Ok(Window::all()
                .map_err(failed)?
                .into_iter()
                .filter_map(|w| {
                    let title = w.title().ok()?;
                    let pid = w.pid().ok()?;
                    if title.is_empty() || pid == own || w.is_minimized().unwrap_or(false) {
                        return None;
                    }
                    Some((Target::Window { id: w.id().ok()?, pid }, title, Self::Window(w)))
                })
                .collect())
        } else {
            let mut monitors: Vec<(bool, Target, Self)> = Monitor::all()
                .map_err(failed)?
                .into_iter()
                .filter_map(|m| Some((!m.is_primary().unwrap_or(false), Target::Monitor(m.id().ok()?), Self::Monitor(m))))
                .collect();
            monitors.sort_by_key(|(secondary, _, _)| *secondary);
            Ok(monitors.into_iter().map(|(_, target, m)| (target, String::new(), m)).collect())
        }
    }

    pub fn open(target: Target) -> Result<Self> {
        Self::list(matches!(target, Target::Window { .. }))?
            .into_iter()
            .find(|(id, _, _)| *id == target)
            .map(|(_, _, source)| source)
            .ok_or_else(|| Error::Encoder("capture source is no longer available".into()))
    }

    fn capture(&self) -> Option<RgbaImage> {
        match self {
            Self::Window(w) => w.capture_image().ok(),
            Self::Monitor(m) => m.capture_image().ok(),
        }
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        match self {
            Self::Window(w) => Some((w.width().ok()?, w.height().ok()?)),
            Self::Monitor(m) => Some((m.width().ok()?, m.height().ok()?)),
        }
    }

    pub fn thumbnail(&self, width: u32, height: u32) -> Option<RgbaImage> {
        self.capture().map(|frame| letterbox(&frame, width, height, imageops::FilterType::Triangle))
    }

    pub fn frame(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        let mut frame = self.capture()?;
        if let (Ok(x), Ok(y), window) = match self {
            Self::Window(w) => (w.x(), w.y(), w.id().ok()),
            Self::Monitor(m) => (m.x(), m.y(), None),
        } {
            draw_cursor(&mut frame, (x, y), window);
        }
        if frame.dimensions() == (width, height) {
            return Some(frame.into_raw());
        }
        Some(letterbox(&frame, width, height, imageops::FilterType::Nearest).into_raw())
    }
}

fn clip_cursor(x: i64, y: i64, width: i64, height: i64, frame: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let left = x.max(0);
    let top = y.max(0);
    let right = (x + width).min(i64::from(frame.0));
    let bottom = (y + height).min(i64::from(frame.1));
    (right > left && bottom > top).then(|| (left as u32, top as u32, (right - left) as u32, (bottom - top) as u32))
}

fn draw_cursor(frame: &mut RgbaImage, origin: (i32, i32), window: Option<u32>) {
    use std::{mem::size_of, ptr::null_mut};

    // SAFETY: Win32 fills initialized structs, and the DIB pointer is only used within its allocated pixel bounds while the bitmap is alive.
    unsafe {
        let mut cursor = CURSORINFO { cbSize: size_of::<CURSORINFO>() as u32, ..Default::default() };
        if GetCursorInfo(&mut cursor) == 0 || cursor.flags & CURSOR_SHOWING == 0 || cursor.hCursor.is_null() {
            return;
        }
        let x = i64::from(cursor.ptScreenPos.x) - i64::from(origin.0);
        let y = i64::from(cursor.ptScreenPos.y) - i64::from(origin.1);
        if !(0..i64::from(frame.width())).contains(&x) || !(0..i64::from(frame.height())).contains(&y) {
            return;
        }
        if window.is_some_and(|id| GetAncestor(WindowFromPoint(cursor.ptScreenPos), GA_ROOT) as usize as u32 != id) {
            return;
        }
        let mut icon = ICONINFO::default();
        if GetIconInfo(cursor.hCursor, &mut icon) == 0 {
            return;
        }
        let monochrome = icon.hbmColor.is_null();
        let bitmap = if monochrome { icon.hbmMask } else { icon.hbmColor };
        let mut dims = BITMAP::default();
        let read_dims = GetObjectW(bitmap, size_of::<BITMAP>() as i32, (&mut dims as *mut BITMAP).cast());
        for handle in [icon.hbmMask, icon.hbmColor] {
            if !handle.is_null() {
                DeleteObject(handle);
            }
        }
        let icon_width = dims.bmWidth;
        let icon_height = if monochrome { dims.bmHeight / 2 } else { dims.bmHeight };
        if read_dims == 0 || icon_width <= 0 || icon_height <= 0 {
            return;
        }
        let left = x - i64::from(icon.xHotspot);
        let top = y - i64::from(icon.yHotspot);
        let Some((cx, cy, width, height)) = clip_cursor(left, top, i64::from(icon_width), i64::from(icon_height), frame.dimensions()) else { return };
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = size_of::<windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width as i32;
        info.bmiHeader.biHeight = -(height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return;
        }
        let mut bits = null_mut();
        let dib = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if !dib.is_null() && !bits.is_null() {
            let len = width as usize * height as usize * 4;
            let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), len);
            for (i, bgra) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let [r, g, b, _] = frame.get_pixel(cx + i as u32 % width, cy + i as u32 / width).0;
                bgra.copy_from_slice(&[b, g, r, 255]);
            }
            let previous = SelectObject(dc, dib);
            if !previous.is_null() && previous as isize != -1 {
                if DrawIconEx(dc, (left - i64::from(cx)) as i32, (top - i64::from(cy)) as i32, cursor.hCursor, icon_width, icon_height, 0, null_mut(), DI_NORMAL) != 0 {
                    GdiFlush();
                    let pixels = std::slice::from_raw_parts(bits.cast::<u8>(), len);
                    for (i, bgra) in pixels.as_chunks::<4>().0.iter().enumerate() {
                        let dst = &mut frame.get_pixel_mut(cx + i as u32 % width, cy + i as u32 / width).0;
                        dst[..3].copy_from_slice(&[bgra[2], bgra[1], bgra[0]]);
                    }
                }
                SelectObject(dc, previous);
            }
        }
        if !dib.is_null() {
            DeleteObject(dib);
        }
        DeleteDC(dc);
    }
}

fn letterbox(frame: &RgbaImage, width: u32, height: u32, filter: imageops::FilterType) -> RgbaImage {
    let (fw, fh) = (frame.width().max(1), frame.height().max(1));
    let scale = (f64::from(width) / f64::from(fw)).min(f64::from(height) / f64::from(fh));
    let w = ((f64::from(fw) * scale).round() as u32).clamp(1, width);
    let h = ((f64::from(fh) * scale).round() as u32).clamp(1, height);
    let scaled = imageops::resize(frame, w, h, filter);
    let mut out = RgbaImage::from_pixel(width, height, xcap::image::Rgba([0, 0, 0, 255]));
    imageops::replace(&mut out, &scaled, i64::from((width - w) / 2), i64::from((height - h) / 2));
    out
}

#[cfg(test)]
mod tests {
    use super::clip_cursor;

    #[test]
    fn cursor_bitmap_is_clipped_to_the_shared_surface() {
        assert_eq!(clip_cursor(-4, -7, 32, 32, (1280, 720)), Some((0, 0, 28, 25)));
        assert_eq!(clip_cursor(1278, 719, 32, 32, (1280, 720)), Some((1278, 719, 2, 1)));
        assert_eq!(clip_cursor(1280, 720, 32, 32, (1280, 720)), None);
    }
}
