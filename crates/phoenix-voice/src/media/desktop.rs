use crate::{Error, Result};
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
        let frame = self.capture()?;
        if frame.dimensions() == (width, height) {
            return Some(frame.into_raw());
        }
        Some(letterbox(&frame, width, height, imageops::FilterType::Nearest).into_raw())
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
