use std::io::Read;
use std::process::{Child, Stdio};
#[cfg(unix)]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::ffmpeg::FfmpegH264;
use super::testsrc::TestPattern;
use super::{EncodedVideo, VideoEncoder, VideoFrame};
use crate::codec::{Codec, VideoEncoderConfig};
use crate::{Error, Result};

pub const TEST_PATTERN: &str = "phoenix-test-pattern";

#[derive(Debug)]
pub enum Input {
    TestPattern,
    #[cfg(any(windows, target_os = "macos"))]
    Selected(super::desktop::Target),
    Device(String),
    #[cfg(windows)]
    Desktop,
    #[cfg(unix)]
    PipeWire { fd: std::os::fd::OwnedFd, node: u32 },
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(windows)]
            (Self::Desktop, Self::Desktop) => true,
            #[cfg(any(windows, target_os = "macos"))]
            (Self::Selected(a), Self::Selected(b)) => a == b,
            (Self::TestPattern, Self::TestPattern) => true,
            (Self::Device(a), Self::Device(b)) => a == b,
            #[cfg(unix)]
            (Self::PipeWire { node: a, .. }, Self::PipeWire { node: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl Input {
    pub fn from_name(name: Option<&str>) -> Self {
        match name {
            Some(TEST_PATTERN) => Self::TestPattern,
            #[cfg(windows)]
            other => Self::Device(other.map(str::to_owned).unwrap_or_default()),
            #[cfg(not(windows))]
            Some(d) if d.starts_with("/dev/video") => Self::Device(d.to_owned()),
            #[cfg(target_os = "macos")]
            other => Self::Device(other.unwrap_or("0").to_owned()),
            #[cfg(not(any(windows, target_os = "macos")))]
            _ => Self::Device("/dev/video0".into()),
        }
    }
}

#[cfg(windows)]
pub fn windows_cameras() -> Vec<String> {
    let Ok(output) = super::ffmpeg::ffmpeg_command()
        .args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"])
        .stdin(Stdio::null())
        .output() else { return Vec::new() };
    String::from_utf8_lossy(&output.stderr).lines().filter_map(|line| {
        let name = line.trim_end().strip_suffix("\" (video)")?;
        Some(name.split_once('"')?.1.to_owned())
    }).collect()
}

pub type Preview = Arc<Mutex<Option<VideoFrame>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureParams {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
}

pub struct Camera {
    stop: Arc<AtomicBool>,
    reconfig: Arc<Mutex<Option<CaptureParams>>>,
    thread: Option<JoinHandle<()>>,
    pub preview: Preview,
}

struct DeviceReader {
    child: Child,
    stdout: std::process::ChildStdout,
}

impl DeviceReader {
    fn open(input: &Input, width: u32, height: u32, fps: u32) -> Result<Self> {
        #[cfg(unix)]
        if let Input::PipeWire { fd, node } = input {
            return Self::pipewire(fd, *node, width, height, fps);
        }
        let mut cmd = super::ffmpeg::ffmpeg_command();
        cmd.args(["-hide_banner", "-loglevel", "error"]);
        match input {
            #[cfg(windows)]
            Input::Desktop => { cmd.args(["-f", "gdigrab", "-framerate", &fps.to_string(), "-i", "desktop"]); }
            #[cfg(windows)]
            Input::Device(name) => {
                let name = if name.is_empty() {
                    windows_cameras().into_iter().next().ok_or_else(|| Error::Encoder("No video capture device found".into()))?
                } else { name.clone() };
                cmd.args(["-f", "dshow", "-i", &format!("video={name}")]);
            }
            #[cfg(target_os = "macos")]
            Input::Device(dev) => { cmd.args(["-f", "avfoundation", "-framerate", &fps.to_string(), "-i", &format!("{dev}:none")]); }
            #[cfg(not(any(windows, target_os = "macos")))]
            Input::Device(dev) => { cmd.args(["-f", "v4l2", "-framerate", &fps.to_string(), "-i", dev]); }
            _ => return Err(Error::Encoder("Invalid capture source".into())),
        }
        let filter = match input {
            #[cfg(windows)]
            Input::Desktop => format!("scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"),
            _ => format!("scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}"),
        };
        let mut child = cmd
            .args(["-vf", &filter])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Encoder(format!("ffmpeg capture: {e}")))?;
        let stdout = child.stdout.take().ok_or(Error::Encoder("ffmpeg capture stdout".into()))?;
        Ok(Self { child, stdout })
    }

    #[cfg(unix)]
    fn pipewire(fd: &std::os::fd::OwnedFd, node: u32, width: u32, height: u32, fps: u32) -> Result<Self> {
        let dup = fd.try_clone().map_err(Error::Io)?;
        let caps = format!("video/x-raw,format=RGBA,width={width},height={height},framerate={fps}/1,pixel-aspect-ratio=1/1");
        let mut child = Command::new("gst-launch-1.0")
            .args(["-q", "pipewiresrc", "fd=0", &format!("path={node}"), "do-timestamp=true", "keepalive-time=1000", "always-copy=true"])
            .args(["!", "videoconvert", "!", "videoscale", "add-borders=true", "!", "videorate", "drop-only=true", "!", &caps, "!", "fdsink", "fd=1", "sync=false"])
            .stdin(Stdio::from(dup))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Encoder(format!("gst-launch pipewiresrc: {e}")))?;
        let stdout = child.stdout.take().ok_or(Error::Encoder("pipewiresrc stdout".into()))?;
        Ok(Self { child, stdout })
    }

    fn frame(&mut self, len: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.stdout.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

impl Drop for DeviceReader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Camera {
    pub fn start(
        input: Input,
        width: u32,
        height: u32,
        fps: u32,
        on_frame: impl FnMut(EncodedVideo) + Send + 'static,
    ) -> Result<Self> {
        Self::start_with(input, width, height, fps, 1_500_000, false, on_frame)
    }

    pub fn start_with(
        input: Input,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        screen: bool,
        on_frame: impl FnMut(EncodedVideo) + Send + 'static,
    ) -> Result<Self> {
        Self::start_with_factory(
            input,
            CaptureParams { width, height, fps, bitrate },
            screen,
            |cfg| Ok(Box::new(FfmpegH264::spawn(cfg)?)),
            on_frame,
        )
    }

    pub fn start_with_factory<F>(
        input: Input,
        params: CaptureParams,
        screen: bool,
        factory: F,
        mut on_frame: impl FnMut(EncodedVideo) + Send + 'static,
    ) -> Result<Self>
    where
        F: Fn(&VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>> + Send + 'static,
    {
        let CaptureParams { width, height, fps, bitrate } = params;
        let stop = Arc::new(AtomicBool::new(false));
        let reconfig: Arc<Mutex<Option<CaptureParams>>> = Arc::new(Mutex::new(None));
        let preview: Preview = Arc::new(Mutex::new(None));
        let cfg = VideoEncoderConfig::new(Codec::H264, width, height, fps, bitrate, screen);
        let mut enc = factory(&cfg)?;
        #[cfg(any(windows, target_os = "macos"))]
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let needs_reader = match &input {
            Input::TestPattern => false,
            #[cfg(any(windows, target_os = "macos"))]
            Input::Selected(_) => false,
            _ => true,
        };
        let initial_reader = if !needs_reader { None } else { Some(DeviceReader::open(&input, width, height, fps)?) };
        let (s2, r2, p2) = (stop.clone(), reconfig.clone(), preview.clone());
        let thread = std::thread::Builder::new()
            .name("phoenix-camera".into())
            .spawn(move || {
                #[cfg(any(windows, target_os = "macos"))]
                let selected = {
                    let source = match &input {
                        Input::Selected(id) => super::desktop::Capture::open(*id).map(Some),
                        _ => Ok(None),
                    };
                    match source {
                        Ok(handle) => { let _ = ready_tx.send(Ok(())); handle }
                        Err(error) => { let _ = ready_tx.send(Err(error)); return; }
                    }
                };
                let mut current = params;
                let mut pattern = TestPattern::new(width, height, fps);
                let live = !matches!(input, Input::TestPattern);
                let mut device = initial_reader;
                let started = Instant::now();
                let mut period = Duration::from_micros(1_000_000 / u64::from(fps.max(1)));
                let mut next = Instant::now();
                let mut first = true;
                while !s2.load(Ordering::Relaxed) {
                    if let Some(target) = r2.lock().ok().and_then(|mut slot| slot.take())
                        && target != current
                    {
                        let new_cfg = VideoEncoderConfig::new(Codec::H264, target.width, target.height, target.fps, target.bitrate, screen);
                        match factory(&new_cfg) {
                            Ok(new_enc) => {
                                enc = new_enc;
                                if (target.width, target.height, target.fps) != (current.width, current.height, current.fps) {
                                    pattern = TestPattern::new(target.width, target.height, target.fps);
                                    drop(device.take());
                                    if needs_reader {
                                        device = DeviceReader::open(&input, target.width, target.height, target.fps).ok();
                                    }
                                    period = Duration::from_micros(1_000_000 / u64::from(target.fps.max(1)));
                                }
                                current = target;
                                first = true;
                            }
                            Err(e) => tracing::warn!(error = %e, "camera: reconfigure failed"),
                        }
                    }
                    let frame = match (live, device.as_mut()) {
                        (true, Some(d)) => match d.frame((current.width * current.height * 4) as usize) {
                            Some(data) => VideoFrame { width: current.width, height: current.height, pts_us: started.elapsed().as_micros() as u64, data: Arc::from(data) },
                            None => break,
                        },
                        #[cfg(any(windows, target_os = "macos"))]
                        (true, None) if selected.is_some() => {
                            let Some(data) = selected.as_ref().and_then(|source| source.frame(current.width, current.height)) else { break };
                            VideoFrame { width: current.width, height: current.height, pts_us: started.elapsed().as_micros() as u64, data: Arc::from(data) }
                        }
                        (true, None) => break,
                        (false, _) => pattern.frame(),
                    };
                    if let Ok(mut p) = p2.lock() {
                        *p = Some(frame.clone());
                    }
                    match enc.encode(&frame, first) {
                        Ok(Some(e)) => on_frame(e),
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(error = %e, "camera: encoder failed");
                            break;
                        }
                    }
                    first = false;
                    if !needs_reader {
                        next += period;
                        let now = Instant::now();
                        if next > now {
                            std::thread::sleep(next - now);
                        } else {
                            next = now;
                        }
                    }
                }
            })
            .map_err(Error::Io)?;
        #[cfg(any(windows, target_os = "macos"))]
        ready_rx.recv().map_err(|e| Error::Encoder(e.to_string()))??;
        Ok(Self { stop, reconfig, thread: Some(thread), preview })
    }

    pub fn reconfigure(&self, width: u32, height: u32, fps: u32, bitrate: u32) {
        if let Ok(mut slot) = self.reconfig.lock() {
            *slot = Some(CaptureParams { width, height, fps, bitrate });
        }
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_names() {
        assert_eq!(Input::from_name(Some(TEST_PATTERN)), Input::TestPattern);
        assert_eq!(Input::from_name(Some("/dev/video2")), Input::Device("/dev/video2".into()));
        #[cfg(not(any(windows, target_os = "macos")))]
        assert_eq!(Input::from_name(None), Input::Device("/dev/video0".into()));
        #[cfg(windows)]
        {
            assert_eq!(Input::from_name(None), Input::Device(String::new()));
            assert_eq!(Input::from_name(Some("USB Camera")), Input::Device("USB Camera".into()));
        }
    }

    #[test]
    fn hot_reconfigure_recreates_encoder_and_forces_keyframe() {
        struct FakeEncoder {
            log: Arc<Mutex<Vec<(u32, u32, bool)>>>,
        }
        impl VideoEncoder for FakeEncoder {
            fn codec(&self) -> Codec {
                Codec::H264
            }
            fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>> {
                self.log.lock().unwrap().push((frame.width, frame.height, force_keyframe));
                Ok(Some(EncodedVideo { codec: Codec::H264, keyframe: force_keyframe, data: vec![0, 0, 0, 1, 0x65], pts_us: frame.pts_us }))
            }
            fn set_bitrate(&mut self, _bps: u32) -> Result<()> {
                Ok(())
            }
        }

        type Log<T> = Arc<Mutex<Vec<T>>>;
        let configs: Log<(u32, u32, u32, u32)> = Arc::new(Mutex::new(Vec::new()));
        let frames: Arc<Mutex<Vec<(u32, u32, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let (c2, f2) = (configs.clone(), frames.clone());
        let cam = Camera::start_with_factory(
            Input::TestPattern,
            CaptureParams { width: 640, height: 360, fps: 60, bitrate: 1_500_000 },
            true,
            move |cfg| {
                c2.lock().unwrap().push((cfg.width, cfg.height, cfg.fps, cfg.bitrate_bps));
                Ok(Box::new(FakeEncoder { log: f2.clone() }))
            },
            |_| {},
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while frames.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!frames.lock().unwrap().is_empty());

        cam.reconfigure(1280, 720, 60, 4_500_000);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !frames.lock().unwrap().iter().any(|&(w, h, _)| (w, h) == (1280, 720)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }

        cam.reconfigure(1280, 720, 60, 4_500_000);
        std::thread::sleep(Duration::from_millis(40));
        cam.stop();

        let cfgs = configs.lock().unwrap().clone();
        assert_eq!(cfgs, vec![(640, 360, 60, 1_500_000), (1280, 720, 60, 4_500_000)]);
        let fl = frames.lock().unwrap();
        let first_new = fl.iter().find(|&&(w, h, _)| (w, h) == (1280, 720)).copied();
        assert_eq!(first_new, Some((1280, 720, true)));
    }
}
