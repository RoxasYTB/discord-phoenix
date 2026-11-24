use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

use super::dsp::{Resampler, convert_channels};

const IDLE_CLOSE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn duration(&self) -> Duration {
        let frames = self.samples.len() / usize::from(self.channels.max(1));
        Duration::from_secs_f64(frames as f64 / f64::from(self.rate.max(1)))
    }

    pub fn adapt(&self, rate: u32, channels: u16) -> Vec<f32> {
        let mixed = convert_channels(&self.samples, usize::from(self.channels.max(1)), usize::from(channels.max(1)));
        let mut r = Resampler::new(self.rate, rate, usize::from(channels.max(1)));
        let mut out = Vec::with_capacity(mixed.len() * rate as usize / self.rate.max(1) as usize + 16);
        r.process(&mixed, &mut out);
        out
    }
}

#[derive(Clone, Debug)]
pub struct Handle {
    stopped: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
}

impl Handle {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }
    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0., 1.).to_bits(), Ordering::Relaxed);
    }
}

struct Voice {
    pcm: Arc<Pcm>,
    buf: Option<(u32, u16, Arc<Vec<f32>>)>,
    pos: usize,
    looped: bool,
    handle: Handle,
}

struct Shared {
    voices: Vec<Voice>,
    format: Option<(u32, u16)>,
}

enum Cmd {
    Wake,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Shared { voices: Vec::new(), format: None }))
}

fn lock() -> std::sync::MutexGuard<'static, Shared> {
    shared().lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn worker() -> &'static mpsc::Sender<Cmd> {
    static TX: OnceLock<mpsc::Sender<Cmd>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new().name("sound-effects".into()).spawn(move || run(rx));
        tx
    })
}

impl Handle {
    pub fn new(volume: f32) -> Self {
        Handle { stopped: Arc::new(AtomicBool::new(false)), volume: Arc::new(AtomicU32::new(volume.clamp(0., 1.).to_bits())) }
    }
}

pub fn start(handle: &Handle, pcm: Arc<Pcm>, looped: bool) {
    if handle.is_stopped() {
        return;
    }
    lock().voices.push(Voice { pcm, buf: None, pos: 0, looped, handle: handle.clone() });
    let _ = worker().send(Cmd::Wake);
}

pub fn play(pcm: Arc<Pcm>, volume: f32, looped: bool) -> Handle {
    let handle = Handle::new(volume);
    start(&handle, pcm, looped);
    handle
}

fn mix(out: &mut [f32]) {
    out.fill(0.);
    let mut s = lock();
    let Some((rate, ch)) = s.format else { return };
    for v in &mut s.voices {
        if v.handle.is_stopped() {
            continue;
        }
        let Some((_, _, buf)) = v.buf.clone().filter(|(r, c, _)| (*r, *c) == (rate, ch)) else { continue };
        if buf.is_empty() {
            v.handle.stop();
            continue;
        }
        let gain = f32::from_bits(v.handle.volume.load(Ordering::Relaxed));
        let mut i = 0;
        while i < out.len() {
            if v.pos >= buf.len() {
                if v.looped {
                    v.pos = 0;
                } else {
                    v.handle.stop();
                    break;
                }
            }
            let n = (out.len() - i).min(buf.len() - v.pos);
            for (o, x) in out[i..i + n].iter_mut().zip(&buf[v.pos..v.pos + n]) {
                *o += x * gain;
            }
            i += n;
            v.pos += n;
        }
    }
    s.voices.retain(|v| !v.handle.is_stopped());
    for o in out.iter_mut() {
        *o = o.clamp(-1., 1.);
    }
}

fn prepare() {
    let (fmt, todo): (Option<(u32, u16)>, Vec<Arc<Pcm>>) = {
        let s = lock();
        let f = s.format;
        (f, s.voices.iter().filter(|v| v.buf.as_ref().is_none_or(|(r, c, _)| Some((*r, *c)) != f)).map(|v| v.pcm.clone()).collect())
    };
    let Some((rate, ch)) = fmt else { return };
    let done: Vec<(Arc<Pcm>, Arc<Vec<f32>>)> = todo.into_iter().map(|p| {
        let b = Arc::new(p.adapt(rate, ch));
        (p, b)
    }).collect();
    let mut s = lock();
    for v in s.voices.iter_mut().filter(|v| v.buf.as_ref().is_none_or(|(r, c, _)| (*r, *c) != (rate, ch))) {
        if let Some((_, b)) = done.iter().find(|(p, _)| Arc::ptr_eq(p, &v.pcm)) {
            v.buf = Some((rate, ch, b.clone()));
            v.pos = 0;
        }
    }
}

fn open() -> Option<Stream> {
    let dev = cpal::default_host().default_output_device()?;
    let cfg = dev.default_output_config().ok()?;
    let conf: StreamConfig = cfg.config();
    let (rate, ch) = (conf.sample_rate.0, conf.channels);
    let err = |e| tracing::warn!(error = %e, "sound effect output error");
    let stream = match cfg.sample_format() {
        SampleFormat::F32 => dev.build_output_stream(&conf, move |d: &mut [f32], _| mix(d), err, None).ok()?,
        SampleFormat::I16 => {
            let mut tmp = Vec::new();
            dev.build_output_stream(
                &conf,
                move |d: &mut [i16], _| {
                    tmp.resize(d.len(), 0.);
                    mix(&mut tmp);
                    for (o, x) in d.iter_mut().zip(&tmp) {
                        *o = (x * f32::from(i16::MAX)) as i16;
                    }
                },
                err,
                None,
            )
            .ok()?
        }
        f => {
            tracing::warn!(format = ?f, "unsupported output sample format for sound effects");
            return None;
        }
    };
    lock().format = Some((rate, ch));
    stream.play().ok()?;
    Some(stream)
}

fn run(rx: mpsc::Receiver<Cmd>) {
    let mut stream: Option<Stream> = None;
    loop {
        let busy = !lock().voices.is_empty();
        if busy && stream.is_none() {
            stream = open();
            if stream.is_none() {
                lock().voices.clear();
            }
        }
        prepare();
        let wait = if stream.is_some() { Duration::from_millis(250) } else { Duration::from_secs(3600) };
        match rx.recv_timeout(wait) {
            Ok(Cmd::Wake) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if stream.is_some() && lock().voices.is_empty() {
            if let Ok(Cmd::Wake) = rx.recv_timeout(IDLE_CLOSE) {
                continue;
            }
            if lock().voices.is_empty() {
                stream = None;
                lock().format = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapt_and_mix_with_gain_and_end() {
        let pcm = Pcm { rate: 24_000, channels: 1, samples: vec![0.5; 240] };
        assert_eq!(pcm.duration(), Duration::from_millis(10));
        let a = pcm.adapt(48_000, 2);
        assert!((a.len() as i64 - 960).abs() <= 4, "{}", a.len());
        let h = Handle::new(0.4);
        {
            let mut s = lock();
            s.format = Some((48_000, 2));
            s.voices.push(Voice { pcm: Arc::new(pcm.clone()), buf: None, pos: 0, looped: false, handle: h.clone() });
        }
        prepare();
        let mut out = vec![0.; 2000];
        mix(&mut out);
        assert!((out[100] - 0.2).abs() < 1e-3);
        assert_eq!(out[1990], 0.);
        assert!(h.is_stopped());
        lock().format = None;
    }
}
