use crate::{Error, Result};
use xcap::{Monitor, Window, image::{RgbaImage, imageops}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Window { id: u32, pid: u32 },
    Monitor(u32),
}

pub enum Capture {
    Window(Window),
    Monitor(Monitor),
}

impl Capture {
    pub fn list(windows: bool) -> Result<Vec<(Target, String, Self)>> {
        let error = |e: xcap::XCapError| Error::Encoder(e.to_string());
        if windows {
            Ok(Window::all().map_err(error)?.into_iter().filter_map(|w| {
                let title = w.title().ok()?;
                if title.is_empty() || w.is_minimized().unwrap_or(false) { return None; }
                Some((Target::Window { id: w.id().ok()?, pid: w.pid().ok()? }, title, Self::Window(w)))
            }).collect())
        } else {
            Ok(Monitor::all().map_err(error)?.into_iter().filter_map(|m| {
                Some((Target::Monitor(m.id().ok()?), format!("{} × {} · {}", m.width().ok()?, m.height().ok()?, m.id().ok()?), Self::Monitor(m)))
            }).collect())
        }
    }

    pub fn open(target: Target) -> Result<Self> {
        Self::list(matches!(target, Target::Window { .. }))?.into_iter()
            .find(|(id, _, _)| *id == target).map(|(_, _, source)| source)
            .ok_or_else(|| Error::Encoder("Capture source is no longer available".into()))
    }

    fn capture(&self) -> Option<RgbaImage> {
        match self {
            Self::Window(w) => w.capture_image().ok(),
            Self::Monitor(m) => m.capture_image().ok(),
        }
    }

    pub fn thumbnail(&self) -> Option<RgbaImage> {
        self.capture().map(|frame| imageops::thumbnail(&frame, 256, 144))
    }

    pub fn frame(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        let frame = self.capture()?;
        if frame.dimensions() == (width, height) { return Some(frame.into_raw()); }
        let scale = (width as f64 / frame.width() as f64).min(height as f64 / frame.height() as f64);
        let w = ((frame.width() as f64 * scale).round() as u32).clamp(1, width);
        let h = ((frame.height() as f64 * scale).round() as u32).clamp(1, height);
        let scaled = imageops::resize(&frame, w, h, imageops::FilterType::Nearest);
        let mut out = RgbaImage::new(width, height);
        imageops::replace(&mut out, &scaled, ((width-w)/2).into(), ((height-h)/2).into());
        Some(out.into_raw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an interactive desktop with Phoenix open"]
    fn captures_the_selected_phoenix_window_and_display() {
        for windows in [true, false] {
            let (target, _, capture) = Capture::list(windows).unwrap().into_iter()
                .find(|(_, title, _)| !windows || title == "Discord Phoenix")
                .expect("test source available");
            assert!(capture.thumbnail().is_some());
            let frame = Capture::open(target).unwrap().frame(320, 180).unwrap();
            assert_eq!(frame.len(), 320 * 180 * 4);
        }
    }
}
