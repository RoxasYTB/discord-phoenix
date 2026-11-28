use std::{path::PathBuf, time::Duration};

use gpui::{Context, SharedString};

use crate::app::PhoenixApp;

pub const ENABLED: &str = "clips-enabled";
pub const LENGTH: &str = "clips-length";
pub const SECONDS: [u64; 3] = [30, 60, 120];

#[derive(Clone)]
pub struct Source {
    pub title: String,
    #[cfg(windows)]
    target: phoenix_voice::media::desktop::Target,
}

pub fn sources() -> Result<Vec<Source>, String> {
    #[cfg(windows)]
    {
        phoenix_voice::media::desktop::Capture::list(true)
            .map(|sources| sources.into_iter().map(|(target, title, _)| Source { title, target }).collect())
            .map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    Ok(Vec::new())
}

type ExportResult = Result<PathBuf, String>;
type Save = (PathBuf, Duration, futures::channel::oneshot::Sender<ExportResult>);

#[derive(Default)]
pub struct Clips {
    sender: Option<std::sync::mpsc::SyncSender<Save>>,
    generation: u64,
    pub title: Option<String>,
    pub saving: bool,
    pub message: Option<SharedString>,
    pub last_saved: Option<PathBuf>,
}

impl Clips {
    pub fn active(&self) -> bool {
        self.sender.is_some()
    }

    pub fn stop(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.sender = None;
        self.title = None;
    }
}

pub fn folder(app: &PhoenixApp) -> PathBuf {
    app.prefs
        .local
        .clips_directory
        .clone()
        .unwrap_or_else(|| crate::paths::downloads_dir().join("Phoenix Clips"))
}

pub fn start(app: &mut PhoenixApp, source: Source, cx: &mut Context<PhoenixApp>) {
    if !app.prefs.local.flag(ENABLED, false) || !cfg!(windows) {
        return;
    }
    app.clips.stop();
    let generation = app.clips.generation;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let (finished, completion) = futures::channel::oneshot::channel();
    app.clips.title = Some(source.title.clone());
    app.clips.message = None;
    app.clips.sender = Some(tx);
    if std::thread::Builder::new()
        .name("phoenix-clips".into())
        .spawn(move || {
            #[cfg(windows)]
            let result = windows_capture::capture(source, rx);
            #[cfg(not(windows))]
            let result = {
                let _ = (source, rx);
                Ok::<(), String>(())
            };
            let _ = finished.send(result);
        })
        .is_err()
    {
        app.clips.stop();
        app.clips.message = Some(crate::t!("game.clips.failed"));
        cx.notify();
        return;
    }
    cx.spawn(async move |app, cx| {
        let result = completion.await;
        let _ = app.update(cx, |a, cx| {
            if a.clips.generation != generation {
                return;
            }
            a.clips.stop();
            a.clips.message = Some(match result {
                Ok(Err(error)) => format!("{} {error}", crate::t!("game.clips.failed")).into(),
                _ => crate::t!("game.clips.stopped"),
            });
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

pub fn stop(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    app.clips.stop();
    app.clips.message = Some(crate::t!("game.clips.stopped"));
    cx.notify();
}

pub fn activity(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl gpui::IntoElement {
    use crate::{
        assets::Icon,
        theme::{c, generated::idx, tok, z},
        ui::{
            controls::{self, BtnKind},
            icon,
            tooltip::Tipped as _,
        },
    };
    use gpui::{ParentElement, Styled, div, prelude::*};

    let label = app.clips.title.clone().unwrap_or_default();
    div()
        .flex()
        .items_center()
        .gap(z(6.))
        .p(z(8.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(div().size(z(7.)).flex_none().rounded_full().bg(c(tok(idx::TEXT_FEEDBACK_CRITICAL))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(z(12.))
                .child(label),
        )
        .when(!app.clips.saving, |d| {
            d.child(
                controls::button_sm("game-clip-save-sidebar", crate::t!("game.clips"), BtnKind::Secondary)
                    .on_click(cx.listener(|a, _, _, cx| save(a, cx)))
                    .tip(crate::t!("game.clips.save")),
            )
        })
        .child(
            div()
                .id("game-clip-stop-sidebar")
                .flex_none()
                .cursor_pointer()
                .p(z(4.))
                .on_click(cx.listener(|a, _, _, cx| stop(a, cx)))
                .tip(crate::t!("game.clips.stop"))
                .child(icon(Icon::Close, 16., c(tok(idx::TEXT_MUTED)))),
        )
}

pub fn save(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(sender) = app.clips.sender.as_ref().filter(|_| !app.clips.saving) else {
        return;
    };
    let (response, result) = futures::channel::oneshot::channel();
    let length = SECONDS[app.prefs.local.pick(LENGTH, 0).min(SECONDS.len() - 1)];
    if sender.try_send((folder(app), Duration::from_secs(length), response)).is_err() {
        return;
    }
    app.clips.saving = true;
    app.clips.message = Some(crate::t!("game.clips.saving"));
    cx.spawn(async move |app, cx| {
        let result = result.await;
        let _ = app.update(cx, |a, cx| {
            a.clips.saving = false;
            let message = match result {
                Ok(Ok(path)) => {
                    a.clips.last_saved = Some(path);
                    crate::t!("game.clips.saved")
                }
                Ok(Err(error)) => format!("{} {error}", crate::t!("game.clips.failed")).into(),
                Err(_) => crate::t!("game.clips.stopped"),
            };
            a.clips.message = Some(message.clone());
            a.parity_toast(message, cx);
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

#[cfg(windows)]
mod windows_capture {
    use std::{
        collections::VecDeque,
        fs,
        io::Write,
        process::Stdio,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    use phoenix_voice::{
        codec::{Codec, VideoEncoderConfig},
        media::{
            VideoEncoder, VideoFrame,
            desktop::{Capture, Target},
            ffmpeg::{FfmpegH264, ffmpeg_command},
        },
    };

    use super::{ExportResult, PathBuf, Save, Source};

    const FPS: u32 = 24;
    const MAX_BYTES: usize = 64 * 1024 * 1024;

    struct Group {
        start: Instant,
        frames: Vec<Arc<[u8]>>,
        bytes: usize,
    }

    type Latest = Arc<Mutex<Option<Result<Arc<[u8]>, String>>>>;

    struct FramePump {
        stop: Arc<AtomicBool>,
        latest: Latest,
    }

    impl FramePump {
        fn start(target: Target, width: u32, height: u32, period: Duration) -> Result<Self, String> {
            let pump = Self {
                stop: Arc::new(AtomicBool::new(false)),
                latest: Arc::new(Mutex::new(None)),
            };
            let stop = pump.stop.clone();
            let latest = pump.latest.clone();
            std::thread::Builder::new()
                .name("phoenix-clip-source".into())
                .spawn(move || {
                    if let Err(error) = capture_frames(target, width, height, period, &stop, &latest)
                        && let Ok(mut latest) = latest.lock()
                    {
                        *latest = Some(Err(error));
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(pump)
        }
    }

    impl Drop for FramePump {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    fn capture_frames(target: Target, width: u32, height: u32, period: Duration, stop: &AtomicBool, latest: &Latest) -> Result<(), String> {
        let source = Capture::open(target).map_err(|e| e.to_string())?;
        let mut last_frame = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            let now = Instant::now();
            if let (Capture::Window(window), Target::Window { pid, .. }) = (&source, target)
                && window.pid().ok() != Some(pid)
            {
                return Err("The selected window was closed.".into());
            }
            if let Some(data) = source.frame(width, height) {
                *latest.lock().map_err(|e| e.to_string())? = Some(Ok(data.into()));
                last_frame = now;
            } else if now.duration_since(last_frame) >= Duration::from_secs(3) {
                return Err("The selected window can no longer be captured.".into());
            }
            std::thread::sleep(period.saturating_sub(now.elapsed()));
        }
        Ok(())
    }

    pub(super) fn capture(source: Source, messages: mpsc::Receiver<Save>) -> Result<(), String> {
        let (width, height) = Capture::open(source.target)
            .map_err(|e| e.to_string())?
            .size()
            .filter(|(w, h)| *w > 0 && *h > 0)
            .ok_or("Window is unavailable.")?;
        let scale = (1280.0 / f64::from(width)).min(720.0 / f64::from(height)).min(1.0);
        let width = ((f64::from(width) * scale).round() as u32 & !1).max(2);
        let height = ((f64::from(height) * scale).round() as u32 & !1).max(2);
        let period = Duration::from_secs_f64(1.0 / f64::from(FPS));
        let pump = FramePump::start(source.target, width, height, period)?;
        let mut config = VideoEncoderConfig::new(Codec::H264, width, height, FPS, 3_000_000, true);
        config.keyframe_interval = FPS;
        let mut encoder = FfmpegH264::spawn(&config).map_err(|e| e.to_string())?;
        let mut queue: VecDeque<Group> = VecDeque::new();
        let mut total_bytes = 0;
        let start = Instant::now();
        let mut deadline = start;
        loop {
            match messages.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok((directory, length, reply)) => {
                    let limit = Instant::now().checked_sub(length).unwrap_or(start);
                    let first = queue.iter().rposition(|group| group.start <= limit).unwrap_or(0);
                    let frames: Vec<_> = queue.iter().skip(first).flat_map(|group| group.frames.iter().cloned()).collect();
                    std::thread::spawn(move || {
                        let _ = reply.send(export(directory, frames));
                    });
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let now = Instant::now();
            deadline = (deadline + period).max(now);
            let data = pump.latest.lock().map_err(|e| e.to_string())?.clone();
            let Some(data) = data else { continue };
            let data = data?;
            let frame = VideoFrame {
                width,
                height,
                pts_us: start.elapsed().as_micros() as u64,
                data,
            };
            let Some(encoded) = encoder.encode(&frame, false).map_err(|e| e.to_string())? else {
                continue;
            };
            if encoded.keyframe {
                queue.push_back(Group {
                    start: now,
                    frames: Vec::new(),
                    bytes: 0,
                });
            }
            if let Some(group) = queue.back_mut() {
                group.bytes += encoded.data.len();
                total_bytes += encoded.data.len();
                group.frames.push(encoded.data.into());
            }
            while queue
                .front()
                .is_some_and(|group| now.duration_since(group.start) > Duration::from_secs(121) || total_bytes > MAX_BYTES)
            {
                total_bytes -= queue.pop_front().unwrap().bytes;
            }
        }
    }

    fn export(directory: PathBuf, frames: Vec<Arc<[u8]>>) -> ExportResult {
        if frames.is_empty() {
            return Err("The clip buffer is still empty.".into());
        }
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f");
        let path = directory.join(format!("Phoenix_{stamp}-{}.mp4", std::process::id()));
        let reserved = fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| e.to_string())?;
        drop(reserved);
        let result = (|| -> Result<(), String> {
            let mut process = ffmpeg_command()
                .args(["-hide_banner", "-loglevel", "error", "-r"])
                .arg(FPS.to_string())
                .args(["-f", "h264", "-i", "pipe:0", "-c:v", "copy", "-movflags", "+faststart", "-y"])
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            let written = {
                let Some(mut input) = process.stdin.take() else {
                    return Err("FFmpeg input is unavailable.".into());
                };
                frames.iter().try_for_each(|frame| input.write_all(frame))
            };
            let status = process.wait().map_err(|e| e.to_string())?;
            written.map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("FFmpeg could not export the clip.".into());
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&path);
        }
        result.map(|()| path)
    }
}
