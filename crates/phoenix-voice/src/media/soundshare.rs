use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use super::AudioEncoder;
use super::opus::{FRAME_SAMPLES, OpusEncoder};
use crate::connection::AudioOut;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Tone,
    Monitor,
}

pub const BITRATE: i32 = 128_000;

pub struct Soundshare {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct Parec(Child);

impl Drop for Parec {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn tone_frame(n: u64) -> Vec<i16> {
    let mut pcm = Vec::with_capacity(FRAME_SAMPLES * 2);
    for i in 0..FRAME_SAMPLES as u64 {
        let t = (n * FRAME_SAMPLES as u64 + i) as f32 / 48_000.;
        let s = ((t * 330. * std::f32::consts::TAU).sin() * 1200.) as i16;
        pcm.extend_from_slice(&[s, s]);
    }
    pcm
}

impl Soundshare {
    pub fn start(source: Source, out: UnboundedSender<AudioOut>) -> Result<Self> {
        let mut enc = OpusEncoder::new(true, BITRATE)?;
        let mut parec = match source {
            Source::Monitor => Some(Parec(
                Command::new("parec")
                    .args(["--device=@DEFAULT_MONITOR@", "--format=s16le", "--rate=48000", "--channels=2", "--latency-msec=20"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| Error::Encoder(format!("parec: {e}")))?,
            )),
            Source::Tone => None,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("phoenix-soundshare".into())
            .spawn(move || {
                let mut stdout = parec.as_mut().and_then(|p| p.0.stdout.take());
                let mut buf = vec![0u8; FRAME_SAMPLES * 4];
                let mut next = Instant::now();
                let mut n = 0u64;
                while !s2.load(Ordering::Relaxed) {
                    let pcm: Vec<i16> = match stdout.as_mut() {
                        Some(r) => {
                            if r.read_exact(&mut buf).is_err() {
                                break;
                            }
                            buf.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes([b[0], b[1]])).collect()
                        }
                        None => {
                            next += Duration::from_millis(20);
                            let now = Instant::now();
                            if next > now {
                                std::thread::sleep(next - now);
                            } else {
                                next = now;
                            }
                            tone_frame(n)
                        }
                    };
                    n += 1;
                    let frame = if pcm.iter().all(|s| s.unsigned_abs() < 8) {
                        AudioOut::End
                    } else {
                        match enc.encode(&pcm) {
                            Ok(f) => AudioOut::Frame(f),
                            Err(_) => continue,
                        }
                    };
                    if out.send(frame).is_err() {
                        break;
                    }
                }
                drop(parec);
            })
            .map_err(Error::Io)?;
        Ok(Self { stop, thread: Some(thread) })
    }
}

impl Drop for Soundshare {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tone_frames_are_20ms_stereo() {
        let f = super::tone_frame(3);
        assert_eq!(f.len(), super::FRAME_SAMPLES * 2);
        assert!(f.iter().any(|s| *s != 0));
    }
}
