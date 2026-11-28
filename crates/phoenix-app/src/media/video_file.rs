use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phoenix_voice::media::ffmpeg;

pub const MAX_DOWNLOAD: usize = 200 * 1024 * 1024;
pub const MAX_FPS: f64 = 30.;
const AUDIO_BUFFER_SECS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Probe {
    pub duration: Option<f64>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub audio: bool,
    pub alpha_decoder: Option<&'static str>,
}

fn parse_clock(s: &str) -> Option<f64> {
    let mut it = s.trim().split(':');
    let (h, m, sec) = (it.next()?.parse::<f64>().ok()?, it.next()?.parse::<f64>().ok()?, it.next()?.parse::<f64>().ok()?);
    Some(h * 3600. + m * 60. + sec)
}

fn dims(line: &str) -> Option<(u32, u32)> {
    line.split([',', ' ']).find_map(|tok| {
        let (a, b) = tok.split_once('x')?;
        if a.len() < 2 || b.len() < 2 || !a.bytes().all(|c| c.is_ascii_digit()) || !b.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some((a.parse().ok()?, b.parse().ok()?))
    })
}

pub fn parse_probe(stderr: &str) -> Option<Probe> {
    let mut p = Probe::default();
    let mut video = false;
    let mut rotated = false;
    let mut codec = None;
    let mut alpha = false;
    for line in stderr.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("Duration:") {
            p.duration = parse_clock(rest.split(',').next().unwrap_or("")).filter(|d| *d > 0.);
        } else if l.starts_with("Stream #") && l.contains(": Video:") && !video {
            video = true;
            codec = l.split(": Video: ").nth(1).and_then(|r| r.split([' ', ',']).next()).map(str::to_owned);
            if let Some((w, h)) = dims(l) {
                p.width = w;
                p.height = h;
            }
            p.fps = l
                .split(',')
                .find_map(|part| part.trim().strip_suffix(" fps").and_then(|v| v.trim().parse::<f64>().ok()))
                .unwrap_or(MAX_FPS);
        } else if l.starts_with("Stream #") && l.contains(": Audio:") {
            p.audio = true;
        } else if l.contains("rotation of") && (l.contains("90.00") || l.contains("270.00")) {
            rotated = true;
        } else if l.starts_with("alpha_mode") && l.ends_with(": 1") {
            alpha = true;
        }
    }
    if alpha {
        p.alpha_decoder = match codec.as_deref() {
            Some("vp9") => Some("libvpx-vp9"),
            Some("vp8") => Some("libvpx"),
            _ => None,
        };
    }
    if rotated {
        std::mem::swap(&mut p.width, &mut p.height);
    }
    video.then_some(p)
}

pub fn probe(path: &Path) -> Option<Probe> {
    let out = ffmpeg::ffmpeg_command().arg("-hide_banner").arg("-i").arg(path).stdin(Stdio::null()).stdout(Stdio::null()).output().ok()?;
    parse_probe(&String::from_utf8_lossy(&out.stderr))
}

fn alpha_decoder(path: &Path) -> Option<&'static str> {
    static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Option<&'static str>>>> = std::sync::OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if let Some(d) = seen.lock().ok()?.get(path) {
        return *d;
    }
    let d = probe(path).and_then(|p| p.alpha_decoder);
    if let Ok(mut m) = seen.lock() {
        m.insert(path.to_path_buf(), d);
    }
    d
}

pub fn available() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(ffmpeg::available)
}

pub fn output_fps(probe_fps: f64) -> f64 {
    if probe_fps.is_finite() && probe_fps > 0. { probe_fps.min(MAX_FPS) } else { MAX_FPS }
}

pub fn video_args(path: &Path, start: f64, rate: f64, fps: f64, max: (u32, u32), looped: bool, decoder: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"].map(String::from).to_vec();
    if let Some(d) = decoder {
        a.extend(["-c:v".into(), d.to_owned()]);
    }
    if looped {
        a.extend(["-stream_loop".into(), "-1".into()]);
    }
    if start > 0. {
        a.extend(["-ss".into(), format!("{start:.3}")]);
    }
    a.extend(["-i".into(), path.to_string_lossy().into_owned(), "-an".into(), "-sn".into(), "-vf".into()]);
    a.push(format!(
        "setpts=PTS/{rate:.4},fps={fps:.3},scale=w='min(iw,{})':h='min(ih,{})':force_original_aspect_ratio=decrease:force_divisible_by=2",
        max.0.max(2),
        max.1.max(2)
    ));
    a.extend(["-f", "image2pipe", "-c:v", "pam", "-pix_fmt", "rgba", "-"].map(String::from));
    a
}

pub fn audio_args(path: &Path, start: f64, rate: f64, out: (u32, u16)) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"].map(String::from).to_vec();
    if start > 0. {
        a.extend(["-ss".into(), format!("{start:.3}")]);
    }
    a.extend(["-i".into(), path.to_string_lossy().into_owned(), "-vn".into(), "-sn".into()]);
    if (rate - 1.).abs() > 1e-3 {
        a.extend(["-af".into(), format!("atempo={:.4}", rate.clamp(0.5, 2.))]);
    }
    a.extend(["-f".into(), "f32le".into(), "-ac".into(), out.1.to_string(), "-ar".into(), out.0.to_string(), "-".into()]);
    a
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("phoenix-video-{}", std::process::id()))
}

pub fn sweep_stale() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let Ok(dir) = std::fs::read_dir(std::env::temp_dir()) else { return };
        for e in dir.flatten() {
            let name = e.file_name();
            let Some(pid) = name.to_str().and_then(|n| n.strip_prefix("phoenix-video-")).and_then(|p| p.parse::<u32>().ok()) else { continue };
            if pid != std::process::id() && !Path::new(&format!("/proc/{pid}")).exists() && cfg!(target_os = "linux") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    });
}

pub fn temp_path(url: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let ext = url.split('?').next().and_then(|u| u.rsplit('/').next()).and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase());
    let ext = ext.filter(|e| e.len() <= 5 && e.bytes().all(|c| c.is_ascii_alphanumeric())).unwrap_or_else(|| "bin".into());
    temp_dir().join(format!("{:016x}.{ext}", h.finish()))
}

pub struct Frame {
    pub time: f64,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

struct AudioShared {
    buf: Mutex<VecDeque<f32>>,
    played: AtomicU64,
    paused: AtomicBool,
    volume: AtomicU32,
    stop: AtomicBool,
    eof: AtomicBool,
    failed: AtomicBool,
}

struct AudioOut {
    shared: Arc<AudioShared>,
    child: Option<Child>,
    rate_hz: u32,
    channels: u16,
}

fn fill(shared: &AudioShared, out: &mut [f32], channels: usize) {
    out.fill(0.);
    if shared.paused.load(Ordering::Relaxed) {
        return;
    }
    let gain = f32::from_bits(shared.volume.load(Ordering::Relaxed));
    let mut b = shared.buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let n = out.len().min(b.len());
    let n = n - n % channels.max(1);
    for (o, x) in out[..n].iter_mut().zip(b.drain(..n)) {
        *o = (x * gain).clamp(-1., 1.);
    }
    shared.played.fetch_add((n / channels.max(1)) as u64, Ordering::Relaxed);
}

fn device_format() -> Option<(u32, u16)> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let dev = cpal::default_host().default_output_device()?;
    let cfg = dev.default_output_config().ok()?;
    Some((cfg.sample_rate().0, cfg.channels()))
}

fn run_output(shared: Arc<AudioShared>, ready: SyncSender<bool>) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let open = || -> Option<cpal::Stream> {
        let dev = cpal::default_host().default_output_device()?;
        let cfg = dev.default_output_config().ok()?;
        let conf: cpal::StreamConfig = cfg.config();
        let ch = usize::from(conf.channels);
        let err = |e| tracing::warn!(error = %e, "video audio output error");
        let s = shared.clone();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_output_stream(&conf, move |d: &mut [f32], _| fill(&s, d, ch), err, None).ok()?,
            cpal::SampleFormat::I16 => {
                let mut tmp = Vec::new();
                dev.build_output_stream(
                    &conf,
                    move |d: &mut [i16], _| {
                        tmp.resize(d.len(), 0.);
                        fill(&s, &mut tmp, ch);
                        for (o, x) in d.iter_mut().zip(&tmp) {
                            *o = (x * f32::from(i16::MAX)) as i16;
                        }
                    },
                    err,
                    None,
                )
                .ok()?
            }
            _ => return None,
        };
        stream.play().ok()?;
        Some(stream)
    };
    let stream = open();
    let _ = ready.send(stream.is_some());
    if stream.is_none() {
        shared.failed.store(true, Ordering::Relaxed);
        return;
    }
    while !shared.stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(stream);
}

impl AudioOut {
    fn spawn(path: &Path, start: f64, rate: f64, volume: f32) -> Option<Self> {
        let (rate_hz, channels) = device_format()?;
        let shared = Arc::new(AudioShared {
            buf: Mutex::new(VecDeque::new()),
            played: AtomicU64::new(0),
            paused: AtomicBool::new(true),
            volume: AtomicU32::new(volume.clamp(0., 1.).to_bits()),
            stop: AtomicBool::new(false),
            eof: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let mut child = ffmpeg::ffmpeg_command()
            .args(audio_args(path, start, rate, (rate_hz, channels)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut stdout = child.stdout.take()?;
        let cap = rate_hz as usize * usize::from(channels) * AUDIO_BUFFER_SECS;
        let s = shared.clone();
        std::thread::Builder::new()
            .name("phoenix-video-audio-in".into())
            .spawn(move || {
                let mut chunk = vec![0u8; 16 * 1024];
                let mut carry: Vec<u8> = Vec::new();
                loop {
                    if s.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let full = s.buf.lock().map(|b| b.len() >= cap).unwrap_or(true);
                    if full {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    let n = match stdout.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    carry.extend_from_slice(&chunk[..n]);
                    let whole = carry.len() - carry.len() % 4;
                    let samples = carry[..whole].as_chunks::<4>().0.iter().copied().map(f32::from_le_bytes);
                    if let Ok(mut b) = s.buf.lock() {
                        b.extend(samples);
                    }
                    carry.drain(..whole);
                }
                s.eof.store(true, Ordering::Relaxed);
            })
            .ok()?;
        let (tx, rx) = sync_channel(1);
        let s = shared.clone();
        std::thread::Builder::new().name("phoenix-video-audio-out".into()).spawn(move || run_output(s, tx)).ok()?;
        if !rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false) {
            shared.stop.store(true, Ordering::Relaxed);
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        Some(Self { shared, child: Some(child), rate_hz, channels })
    }

    fn played_secs(&self) -> f64 {
        self.shared.played.load(Ordering::Relaxed) as f64 / f64::from(self.rate_hz.max(1))
    }

    fn drained(&self) -> bool {
        self.shared.eof.load(Ordering::Relaxed) && self.shared.buf.lock().map(|b| b.len() < usize::from(self.channels.max(1))).unwrap_or(true)
    }
}

impl Drop for AudioOut {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

pub struct Session {
    child: Child,
    frames: Receiver<Frame>,
    pending: Option<Frame>,
    video_done: bool,
    audio: Option<AudioOut>,
    start: f64,
    rate: f64,
    paused: bool,
    wall: f64,
    resumed: Option<Instant>,
    drained_at: Option<f64>,
    pub size: (u32, u32),
}

pub struct SessionSpec<'a> {
    pub path: &'a Path,
    pub start: f64,
    pub rate: f64,
    pub fps: f64,
    pub max: (u32, u32),
    pub looped: bool,
    pub audio: bool,
    pub volume: f32,
}

impl Session {
    pub fn spawn(spec: &SessionSpec) -> Option<Self> {
        let mut child = ffmpeg::ffmpeg_command()
            .args(video_args(spec.path, spec.start, spec.rate, spec.fps, spec.max, spec.looped, alpha_decoder(spec.path)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdout = child.stdout.take()?;
        let (tx, rx) = sync_channel::<Frame>(3);
        let (start, step) = (spec.start, spec.rate / spec.fps.max(1.));
        std::thread::Builder::new()
            .name("phoenix-video-in".into())
            .spawn(move || {
                let mut r = std::io::BufReader::with_capacity(1 << 20, stdout);
                let mut k = 0u64;
                while let Some((width, height, data)) = ffmpeg::read_pam(&mut r) {
                    if tx.send(Frame { time: start + k as f64 * step, width, height, data }).is_err() {
                        break;
                    }
                    k += 1;
                }
            })
            .ok()?;
        let audio = if spec.audio && !spec.looped { AudioOut::spawn(spec.path, spec.start, spec.rate, spec.volume) } else { None };
        Some(Self {
            child,
            frames: rx,
            pending: None,
            video_done: false,
            audio,
            start: spec.start,
            rate: spec.rate,
            paused: true,
            wall: 0.,
            resumed: None,
            drained_at: None,
            size: spec.max,
        })
    }

    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }

    fn wall_secs(&self) -> f64 {
        self.wall + self.resumed.map_or(0., |t| t.elapsed().as_secs_f64())
    }

    pub fn position(&self) -> f64 {
        if let Some(d) = self.drained_at {
            return d + self.wall_secs() * self.rate;
        }
        match &self.audio {
            Some(a) => self.start + a.played_secs() * self.rate,
            None => self.start + self.wall_secs() * self.rate,
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        if paused {
            self.wall = self.wall_secs();
            self.resumed = None;
        } else {
            self.resumed = Some(Instant::now());
        }
        if let Some(a) = &self.audio {
            a.shared.paused.store(paused, Ordering::Relaxed);
        }
    }

    pub fn set_volume(&self, v: f32) {
        if let Some(a) = &self.audio {
            a.shared.volume.store(v.clamp(0., 1.).to_bits(), Ordering::Relaxed);
        }
    }

    pub fn ready(&self) -> bool {
        self.pending.is_some()
    }

    pub fn finished(&self) -> bool {
        self.video_done && self.pending.is_none() && self.audio.as_ref().is_none_or(AudioOut::drained)
    }

    pub fn poll(&mut self) -> Option<Frame> {
        if self.drained_at.is_none()
            && let Some(a) = &self.audio
            && (a.drained() || a.shared.failed.load(Ordering::Relaxed))
            && !self.video_done
        {
            let pos = self.start + a.played_secs() * self.rate;
            self.wall = 0.;
            self.resumed = (!self.paused).then(Instant::now);
            self.drained_at = Some(pos);
        }
        let clock = self.position();
        let mut shown = None;
        loop {
            if self.pending.is_none() {
                match self.frames.try_recv() {
                    Ok(f) => self.pending = Some(f),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.video_done = true;
                        break;
                    }
                }
            }
            if !self.pending.as_ref().is_some_and(|f| f.time <= clock + 0.005) {
                break;
            }
            shown = self.pending.take();
        }
        shown
    }

    pub fn first_frame(&mut self) -> Option<Frame> {
        if self.pending.is_none() {
            match self.frames.try_recv() {
                Ok(f) => self.pending = Some(f),
                Err(TryRecvError::Disconnected) => self.video_done = true,
                Err(TryRecvError::Empty) => {}
            }
        }
        let f = self.pending.as_ref()?;
        Some(Frame { time: f.time, width: f.width, height: f.height, data: f.data.clone() })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':\n  Duration: 00:01:05.50, start: 0.000000, bitrate: 512 kb/s\n  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 640x360 [SAR 1:1 DAR 16:9], 400 kb/s, 23.98 fps, 23.98 tbr, 24k tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, stereo, fltp, 96 kb/s (default)\nAt least one output file must be specified\n";

    #[test]
    fn probe_reads_duration_size_rate_and_audio() {
        let p = parse_probe(SAMPLE).unwrap();
        assert_eq!(p.duration, Some(65.5));
        assert_eq!((p.width, p.height), (640, 360));
        assert!((p.fps - 23.98).abs() < 1e-6);
        assert!(p.audio);
        let silent = SAMPLE.replace("  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, stereo, fltp, 96 kb/s (default)\n", "");
        assert!(!parse_probe(&silent).unwrap().audio);
        let rotated = SAMPLE.replace("At least", "      displaymatrix: rotation of -90.00 degrees\nAt least");
        assert_eq!(parse_probe(&rotated).map(|p| (p.width, p.height)), Some((360, 640)));
        assert!(parse_probe("  Duration: N/A\n  Stream #0:0: Audio: opus\n").is_none());
        let webm = "  Duration: N/A, start: -0.007000, bitrate: N/A\n  Stream #0:0: Video: vp9 (Profile 0), yuv420p(tv), 1280x720, SAR 1:1 DAR 16:9, 30 fps, 30 tbr, 1k tbn (default)\n";
        let w = parse_probe(webm).unwrap();
        assert_eq!((w.duration, w.width, w.height, w.audio), (None, 1280, 720, false));
    }

    #[test]
    fn decoder_arguments() {
        let p = Path::new("/tmp/x.mp4");
        let v = video_args(p, 12.5, 1.5, 30., (825, 525), false, None);
        let pos = |a: &[String], k: &str| a.iter().position(|x| x == k);
        assert!(pos(&v, "-ss").unwrap() < pos(&v, "-i").unwrap());
        assert_eq!(v[pos(&v, "-ss").unwrap() + 1], "12.500");
        assert!(v.iter().any(|x| x.starts_with("setpts=PTS/1.5000,fps=30.000,scale=w='min(iw,825)'")));
        assert!(pos(&v, "-stream_loop").is_none());
        let l = video_args(p, 0., 1., 15., (100, 100), true, None);
        assert!(pos(&l, "-stream_loop").unwrap() < pos(&l, "-i").unwrap());
        assert!(pos(&l, "-ss").is_none());
        let a = audio_args(p, 3., 2., (48_000, 2));
        assert!(a.iter().any(|x| x == "atempo=2.0000"));
        assert_eq!(&a[a.len() - 7..], ["-f", "f32le", "-ac", "2", "-ar", "48000", "-"].map(String::from));
        assert!(!audio_args(p, 0., 1., (44_100, 1)).iter().any(|x| x == "-af"));
        assert_eq!(output_fps(59.94), 30.);
        assert_eq!(output_fps(0.), 30.);
        assert_eq!(output_fps(24.), 24.);
        let w = video_args(p, 0., 1., 30., (100, 100), true, Some("libvpx-vp9"));
        assert!(pos(&w, "-c:v").unwrap() < pos(&w, "-i").unwrap());
        let probe = parse_probe("  Duration: 00:00:04.00, start: 0.000000\n  Stream #0:0: Video: vp9 (Profile 0), yuv420p(tv), 360x360, 30 fps\n      alpha_mode      : 1\n").unwrap();
        assert_eq!(probe.alpha_decoder, Some("libvpx-vp9"));
    }

    #[test]
    fn temp_names_keep_a_safe_extension() {
        let a = temp_path("https://cdn.discordapp.com/attachments/1/2/clip.MP4?ex=1&hm=2");
        assert_eq!(a.extension().and_then(|e| e.to_str()), Some("mp4"));
        assert_ne!(a, temp_path("https://cdn.discordapp.com/attachments/1/3/clip.mp4"));
        assert_eq!(temp_path("https://x/y/noext").extension().and_then(|e| e.to_str()), Some("bin"));
        assert!(a.starts_with(temp_dir()));
    }

    fn generated(dir: &Path, name: &str, args: &[&str]) -> Option<PathBuf> {
        let out = dir.join(name);
        let ok = ffmpeg::ffmpeg_command()
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(args)
            .arg(&out)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        ok.then_some(out)
    }

    #[test]
    fn plays_generated_mp4_and_webm_files() {
        if !available() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("phoenix-video-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let cases = [
            ("a.mp4", vec!["-f", "lavfi", "-i", "color=c=red:s=160x90:r=10:d=1", "-c:v", "libx264", "-pix_fmt", "yuv420p"]),
            ("b.webm", vec!["-f", "lavfi", "-i", "color=c=red:s=160x90:r=10:d=1", "-c:v", "libvpx", "-deadline", "realtime"]),
        ];
        for (name, args) in cases {
            let Some(path) = generated(&dir, name, &args) else { continue };
            let p = probe(&path).unwrap_or_else(|| panic!("{name} probe"));
            assert_eq!((p.width, p.height), (160, 90), "{name}");
            assert!(!p.audio, "{name}");
            let spec = SessionSpec { path: &path, start: 0.5, rate: 1., fps: output_fps(p.fps), max: (80, 80), looped: false, audio: false, volume: 0. };
            let mut s = Session::spawn(&spec).unwrap();
            let t = Instant::now();
            let mut first = None;
            while first.is_none() && t.elapsed() < Duration::from_secs(20) {
                first = s.first_frame();
                std::thread::sleep(Duration::from_millis(10));
            }
            let f = first.unwrap_or_else(|| panic!("{name} frame"));
            assert!(f.width == 80 && (44..=46).contains(&f.height), "{name} {}x{}", f.width, f.height);
            assert!((f.time - 0.5).abs() < 1e-9);
            let px = &f.data[(20 * 80 + 40) * 4..][..4];
            assert!(px[2] > 200 && px[1] < 60 && px[0] < 60, "{name} {px:?}");
            s.set_paused(false);
            let t = Instant::now();
            while !s.finished() && t.elapsed() < Duration::from_secs(20) {
                let _ = s.poll();
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(s.finished(), "{name} never ended");
            assert!(s.position() >= 0.9, "{name} {}", s.position());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
