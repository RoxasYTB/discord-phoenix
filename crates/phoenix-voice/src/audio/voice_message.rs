use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::device::{Processor, open_input};
use super::dsp::{Resampler, convert_channels, to_i16};
use super::effects::Pcm;
use super::ogg;
use super::{AudioControl, lock};
use crate::media::opus::{OpusDecoder, OpusEncoder};
use crate::media::{AudioDecoder, AudioEncoder};
use crate::{Error, FRAME_SAMPLES, Result};

pub const FILENAME: &str = "voice-message.ogg";
pub const CONTENT_TYPE: &str = "audio/ogg";
pub const FLAG_IS_VOICE_MESSAGE: u64 = 1 << 13;
pub const MAX_DURATION: Duration = Duration::from_millis(1_199_500);
pub const WARN_APPROACHING: Duration = Duration::from_millis(1_169_500);
pub const WARN_IMMINENT: Duration = Duration::from_millis(1_184_500);
pub const MIN_DURATION_SECS: f64 = 0.5;
pub const WAVEFORM_LEN: usize = 256;
pub const PLAYBACK_RATES: [f32; 4] = [0.75, 1.0, 1.5, 2.0];
pub const BITRATE: i32 = 64_000;
const RATE: u32 = 48_000;

pub enum Source {
    Microphone(Option<String>),
    Pcm { rate: u32, samples: Vec<f32> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub ogg: Vec<u8>,
    pub duration_secs: f64,
    pub samples: Vec<u8>,
}

impl Recording {
    pub fn waveform(&self) -> Vec<u8> {
        waveform(&self.samples)
    }
}

pub fn level_sample(db: f32) -> u8 {
    ((db.clamp(-100., 0.) + 100.) / 100. * 255.).round() as u8
}

pub fn downsample(e: &[f32], t: usize) -> Vec<f32> {
    if t == 0 || e.len() <= t {
        return e.to_vec();
    }
    let n = e.len() as f64 / t as f64;
    let mut out = Vec::with_capacity(t);
    let mut a = 0usize;
    while out.len() < t {
        let end = ((out.len() + 1) as f64 * n).round() as usize;
        let (mut sum, mut cnt) = (0f32, 0u32);
        for v in e.iter().take(end.min(e.len())).skip(a) {
            sum += v;
            cnt += 1;
        }
        out.push(if cnt == 0 { 0. } else { sum / cnt as f32 });
        a = end;
    }
    out
}

pub fn waveform(samples: &[u8]) -> Vec<u8> {
    let f: Vec<f32> = samples.iter().map(|&s| f32::from(s)).collect();
    downsample(&f, WAVEFORM_LEN).into_iter().map(|v| v.round().min(255.) as u8).collect()
}

pub fn tone(rate: u32, secs: f32, hz: f32, amplitude: f32) -> Vec<f32> {
    let n = (rate as f32 * secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let env = 0.55 + 0.45 * (std::f32::consts::TAU * 1.5 * t).sin();
            (std::f32::consts::TAU * hz * t).sin() * amplitude * env
        })
        .collect()
}

struct Encoder {
    control: Arc<AudioControl>,
    proc: Processor,
    rs: Resampler,
    pending: Vec<f32>,
    opus: OpusEncoder,
    packets: Vec<Vec<u8>>,
    real_samples: u64,
    samples: Arc<Mutex<Vec<u8>>>,
}

impl Encoder {
    fn new(control: Arc<AudioControl>, input_rate: u32, samples: Arc<Mutex<Vec<u8>>>) -> Result<Self> {
        Ok(Self {
            control,
            proc: Processor::default(),
            rs: Resampler::new(input_rate, RATE, 1),
            pending: Vec::with_capacity(FRAME_SAMPLES * 4),
            opus: OpusEncoder::new(false, BITRATE)?,
            packets: Vec::new(),
            real_samples: 0,
            samples,
        })
    }

    fn full(&self) -> bool {
        self.real_samples >= MAX_DURATION.as_millis() as u64 * u64::from(RATE) / 1000
    }

    fn push(&mut self, chunk: &[f32]) -> Result<()> {
        if self.full() {
            return Ok(());
        }
        self.rs.process(chunk, &mut self.pending);
        while self.pending.len() >= FRAME_SAMPLES {
            let frame: Vec<f32> = self.pending.drain(..FRAME_SAMPLES).collect();
            self.frame(frame, FRAME_SAMPLES)?;
        }
        Ok(())
    }

    fn frame(&mut self, mut frame: Vec<f32>, real: usize) -> Result<()> {
        let (db, _) = self.proc.process(&self.control, &mut frame);
        lock(&self.samples).push(level_sample(db));
        self.packets.push(self.opus.encode(&to_i16(&frame))?);
        self.real_samples += real as u64;
        Ok(())
    }

    fn finish(mut self) -> Result<Recording> {
        if !self.pending.is_empty() && !self.full() {
            let real = self.pending.len();
            let mut frame = std::mem::take(&mut self.pending);
            frame.resize(FRAME_SAMPLES, 0.);
            self.frame(frame, real)?;
        }
        let serial = rand::random::<u32>();
        let ogg = ogg::write_opus(&self.packets, FRAME_SAMPLES as u64, self.real_samples, serial);
        let samples = lock(&self.samples).clone();
        Ok(Recording { ogg, duration_secs: self.real_samples as f64 / f64::from(RATE), samples })
    }
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<u8>>>,
    io_stop: Option<std_mpsc::Sender<()>>,
    io: Option<JoinHandle<()>>,
    worker: Option<JoinHandle<Result<Recording>>>,
    started: Instant,
}

impl Recorder {
    pub fn start(control: Arc<AudioControl>, source: Source) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = std_mpsc::channel::<Vec<f32>>();
        let (io_stop_tx, io_stop_rx) = std_mpsc::channel::<()>();
        let (rate, io) = match source {
            Source::Microphone(name) => {
                let (ready_tx, ready_rx) = std_mpsc::channel::<Option<u32>>();
                let io = std::thread::Builder::new()
                    .name("phoenix-voicemsg-mic".into())
                    .spawn(move || {
                        let opened = open_input(name.as_deref(), tx);
                        let _ = ready_tx.send(opened.as_ref().map(|(_, (r, _))| *r));
                        let _keep = opened;
                        let _ = io_stop_rx.recv();
                    })
                    .map_err(Error::Io)?;
                let rate = ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| Error::Timeout("microphone"))?;
                let Some(rate) = rate else {
                    drop(io_stop_tx);
                    let _ = io.join();
                    return Err(Error::Unsupported("no microphone".into()));
                };
                (rate, io)
            }
            Source::Pcm { rate, samples: pcm } => {
                let s2 = stop.clone();
                let io = std::thread::Builder::new()
                    .name("phoenix-voicemsg-src".into())
                    .spawn(move || {
                        let chunk = (rate as usize / 100).max(1);
                        let t0 = Instant::now();
                        for (i, c) in pcm.chunks(chunk).enumerate() {
                            if s2.load(Ordering::Relaxed) || tx.send(c.to_vec()).is_err() {
                                return;
                            }
                            let due = t0 + Duration::from_millis(10 * (i as u64 + 1));
                            if let Some(d) = due.checked_duration_since(Instant::now()) {
                                std::thread::sleep(d);
                            }
                        }
                        let _ = io_stop_rx.recv();
                    })
                    .map_err(Error::Io)?;
                (rate, io)
            }
        };
        let mut enc = Encoder::new(control, rate, samples.clone())?;
        let s3 = stop.clone();
        let worker = std::thread::Builder::new()
            .name("phoenix-voicemsg".into())
            .spawn(move || {
                loop {
                    match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(chunk) => enc.push(&chunk)?,
                        Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    if s3.load(Ordering::Relaxed) {
                        while let Ok(chunk) = rx.try_recv() {
                            enc.push(&chunk)?;
                        }
                        break;
                    }
                }
                enc.finish()
            })
            .map_err(Error::Io)?;
        Ok(Self { stop, samples, io_stop: Some(io_stop_tx), io: Some(io), worker: Some(worker), started: Instant::now() })
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn samples(&self) -> Vec<u8> {
        lock(&self.samples).clone()
    }

    pub fn sample_count(&self) -> usize {
        lock(&self.samples).len()
    }

    pub fn last_sample(&self) -> Option<u8> {
        lock(&self.samples).last().copied()
    }

    fn halt(&mut self) -> Option<JoinHandle<Result<Recording>>> {
        self.stop.store(true, Ordering::Relaxed);
        self.io_stop.take();
        if let Some(io) = self.io.take() {
            let _ = io.join();
        }
        self.worker.take()
    }

    pub fn stop(mut self) -> Result<Recording> {
        let worker = self.halt().ok_or_else(|| Error::Protocol("recorder already stopped".into()))?;
        worker.join().map_err(|_| Error::Protocol("voice message encoder panicked".into()))?
    }

    pub fn cancel(mut self) {
        if let Some(w) = self.halt() {
            let _ = w.join();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Some(w) = self.halt() {
            let _ = w.join();
        }
    }
}

pub fn encode_ogg(pcm: &Pcm) -> Result<Vec<u8>> {
    let ch = usize::from(pcm.channels.max(1));
    let mono = if ch == 1 { pcm.samples.clone() } else { convert_channels(&pcm.samples, ch, 1) };
    let mut rs = Resampler::new(pcm.rate.max(1), RATE, 1);
    let mut pending = Vec::with_capacity(mono.len());
    rs.process(&mono, &mut pending);
    let real = pending.len() as u64;
    let mut opus = OpusEncoder::new(false, BITRATE)?;
    let mut packets = Vec::with_capacity(pending.len() / FRAME_SAMPLES + 1);
    for frame in pending.chunks(FRAME_SAMPLES) {
        let mut f = frame.to_vec();
        f.resize(FRAME_SAMPLES, 0.);
        packets.push(opus.encode(&to_i16(&f))?);
    }
    Ok(ogg::write_opus(&packets, FRAME_SAMPLES as u64, real, rand::random::<u32>()))
}

pub fn encode_sound_ogg(pcm: &Pcm) -> Result<Vec<u8>> {
    let ch = usize::from(pcm.channels.max(1));
    if ch > 2 {
        return Err(Error::Encoder("Only handles up to 2 channels".into()));
    }
    let mut rs = Resampler::new(pcm.rate.max(1), RATE, ch);
    let mut pending = Vec::with_capacity(pcm.samples.len());
    rs.process(&pcm.samples, &mut pending);
    let mut opus = OpusEncoder::for_media(ch == 2)?;
    let frame = FRAME_SAMPLES * ch;
    let mut packets = Vec::with_capacity(pending.len() / frame + 1);
    for chunk in pending.chunks(frame) {
        let mut f = chunk.to_vec();
        f.resize(frame, 0.);
        packets.push(opus.encode(&to_i16(&f))?);
    }
    Ok(ogg::write_webcodecs(&packets, FRAME_SAMPLES as u64, ch as u8, RATE))
}

pub fn decode_ogg(data: &[u8]) -> Result<Pcm> {
    let s = ogg::read_opus(data).ok_or(Error::Malformed("not an Ogg Opus file"))?;
    let stereo = s.channels >= 2;
    let ch = if stereo { 2 } else { 1 };
    let mut dec = OpusDecoder::new(stereo)?;
    let mut pcm: Vec<f32> = Vec::with_capacity(s.packets.len() * FRAME_SAMPLES * ch);
    for p in &s.packets {
        match dec.decode(Some(p)) {
            Ok(v) => pcm.extend(v.iter().map(|&x| f32::from(x) / 32768.)),
            Err(e) => tracing::debug!(error = %e, "voice message packet"),
        }
    }
    let skip = usize::from(s.pre_skip) * ch;
    let mut samples: Vec<f32> = pcm.get(skip..).map(<[f32]>::to_vec).unwrap_or_default();
    let total = s.duration_samples() as usize * ch;
    if total > 0 && total < samples.len() {
        samples.truncate(total);
    }
    let samples = if stereo { convert_channels(&samples, 2, 1) } else { samples };
    Ok(Pcm { rate: RATE, channels: 1, samples })
}

pub fn time_stretch(x: &[f32], rate: f32, sample_rate: u32) -> Vec<f32> {
    if (rate - 1.).abs() < 1e-3 || x.is_empty() || rate <= 0. {
        return x.to_vec();
    }
    let n = (sample_rate as usize * 30 / 1000).max(64) & !1;
    let hs = n / 2;
    let ha = (hs as f32 * rate).round().max(1.) as usize;
    let tol = sample_rate as usize * 8 / 1000;
    if x.len() < n + tol + hs {
        let mut r = Resampler::new((sample_rate as f32 * rate) as u32, sample_rate, 1);
        let mut out = Vec::new();
        r.process(x, &mut out);
        return out;
    }
    let win: Vec<f32> = (0..n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()).collect();
    let frames = (x.len() - n - tol) / ha + 1;
    let mut y = vec![0f32; frames * hs + n];
    let mut prev = 0usize;
    for k in 0..frames {
        let target = k * ha;
        let pos = if k == 0 {
            0
        } else {
            let nat = (prev + hs).min(x.len() - hs);
            let lo = target.saturating_sub(tol);
            let hi = (target + tol).min(x.len() - n);
            let score = |p: usize| -> f32 { (0..hs).step_by(4).map(|i| x[nat + i] * x[p + i]).sum() };
            let mut best = target.min(hi);
            let mut best_c = f32::MIN;
            let mut p = lo;
            while p <= hi {
                let c = score(p);
                if c > best_c {
                    best_c = c;
                    best = p;
                }
                p += 4;
            }
            for p in best.saturating_sub(3).max(lo)..=(best + 3).min(hi) {
                let c = score(p);
                if c > best_c {
                    best_c = c;
                    best = p;
                }
            }
            best
        };
        for ((o, &v), &w) in y[k * hs..k * hs + n].iter_mut().zip(&x[pos..pos + n]).zip(&win) {
            *o += v * w;
        }
        prev = pos;
    }
    y.truncate(((x.len() as f32 / rate) as usize).min(y.len()));
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_upload_keeps_stereo_in_the_webcodecs_layout() {
        let frames = 24_000usize;
        let samples: Vec<f32> = (0..frames).flat_map(|i| {
            let t = i as f32 / 48_000.;
            [(t * 440. * std::f32::consts::TAU).sin() * 0.3, (t * 660. * std::f32::consts::TAU).sin() * 0.3]
        }).collect();
        let ogg_bytes = encode_sound_ogg(&Pcm { rate: 48_000, channels: 2, samples }).unwrap();
        let s = ogg::read_opus(&ogg_bytes).unwrap();
        assert_eq!(s.channels, 2);
        assert_eq!(s.pre_skip, 0);
        assert_eq!(s.packets.len(), 25);
        assert_eq!(s.last_granule, 25 * 960);
        assert_eq!(&ogg_bytes[14..18], &[0, 0, 0, 1]);
        assert_eq!(ogg_bytes[5], 2);
        let tags = ogg_bytes.windows(8).position(|w| w == b"OpusTags").unwrap();
        assert_eq!(&ogg_bytes[tags + 8..tags + 16], &[0; 8]);
        let mono = encode_sound_ogg(&Pcm { rate: 44_100, channels: 1, samples: vec![0.; 4410] }).unwrap();
        assert_eq!(ogg::read_opus(&mono).unwrap().channels, 1);
        assert!(encode_sound_ogg(&Pcm { rate: 48_000, channels: 6, samples: vec![0.; 60] }).is_err());
    }

    #[test]
    fn level_mapping_matches_client() {
        assert_eq!(level_sample(-100.), 0);
        assert_eq!(level_sample(-150.), 0);
        assert_eq!(level_sample(0.), 255);
        assert_eq!(level_sample(10.), 255);
        assert_eq!(level_sample(-50.), 128);
    }

    #[test]
    fn waveform_downsamples_to_256() {
        let s: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let w = waveform(&s);
        assert_eq!(w.len(), WAVEFORM_LEN);
        let short = waveform(&[1, 2, 3]);
        assert_eq!(short, vec![1, 2, 3]);
        assert!(waveform(&[]).is_empty());
        assert_eq!(downsample(&[1., 3., 5., 7.], 2), vec![2., 6.]);
    }

    #[test]
    fn encode_decode_tone() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let mut enc = Encoder::new(Arc::new(AudioControl::default()), 44_100, samples).unwrap();
        let t = tone(44_100, 1.25, 440., 0.5);
        for c in t.chunks(441) {
            enc.push(c).unwrap();
        }
        let rec = enc.finish().unwrap();
        assert!((rec.duration_secs - 1.25).abs() < 0.03, "{}", rec.duration_secs);
        assert!(rec.samples.len() >= 62);
        assert!(rec.samples.iter().any(|&s| s > 150));
        let pcm = decode_ogg(&rec.ogg).unwrap();
        assert_eq!(pcm.rate, 48_000);
        let secs = pcm.samples.len() as f64 / 48_000.;
        assert!((secs - rec.duration_secs).abs() < 0.03, "{secs}");
        assert!(pcm.samples.iter().map(|s| s.abs()).fold(0., f32::max) > 0.05);
    }

    #[test]
    fn encode_pcm_to_ogg() {
        let t = tone(44_100, 0.5, 440., 0.5);
        let stereo: Vec<f32> = t.iter().flat_map(|&x| [x, x]).collect();
        let ogg = encode_ogg(&Pcm { rate: 44_100, channels: 2, samples: stereo }).unwrap();
        assert_eq!(&ogg[..4], b"OggS");
        let pcm = decode_ogg(&ogg).unwrap();
        let secs = pcm.samples.len() as f64 / 48_000.;
        assert!((secs - 0.5).abs() < 0.03, "{secs}");
    }

    #[test]
    fn stretch_keeps_length_ratio() {
        let x = tone(48_000, 2.0, 220., 0.4);
        for r in PLAYBACK_RATES {
            let y = time_stretch(&x, r, 48_000);
            let expect = x.len() as f32 / r;
            assert!((y.len() as f32 - expect).abs() / expect < 0.03, "{r}: {}", y.len());
        }
        assert_eq!(time_stretch(&x, 1.0, 48_000), x);
    }
}
