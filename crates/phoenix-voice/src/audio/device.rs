use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig, SupportedStreamConfig};
use tokio::sync::mpsc;

use super::dsp::{Agc, HighPass, NoiseGate, NoiseSuppressor, Resampler, convert_channels, level_db, to_i16};
use super::mixer::Mixer;
use super::{AudioControl, lock};
use crate::connection::{AudioIn, AudioOut};
use crate::media::AudioEncoder;
use crate::media::opus::OpusEncoder;
use crate::{Error, FRAME_SAMPLES, Result};

const RATE: u32 = 48_000;
const MAX_OUT_MS: usize = 120;
const OUT_TARGET_MS: usize = 60;

pub fn input_devices() -> Vec<String> {
    if let Some(v) = super::pulse::devices(true).filter(|v| !v.is_empty()) {
        return v.into_iter().map(|d| d.description).collect();
    }
    let host = cpal::default_host();
    let mut v: Vec<String> = host.input_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
    if let Some(n) = host.default_input_device().and_then(|d| d.name().ok()) {
        v.retain(|x| *x != n);
        v.insert(0, n);
    }
    v
}

pub fn output_devices() -> Vec<String> {
    if let Some(v) = super::pulse::devices(false).filter(|v| !v.is_empty()) {
        return v.into_iter().map(|d| d.description).collect();
    }
    let host = cpal::default_host();
    let mut v: Vec<String> = host.output_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
    if let Some(n) = host.default_output_device().and_then(|d| d.name().ok()) {
        v.retain(|x| *x != n);
        v.insert(0, n);
    }
    v
}

fn find_device(input: bool, name: Option<&str>) -> Option<Device> {
    let host = cpal::default_host();
    if let Some(name) = name {
        let found = if input { host.input_devices().ok() } else { host.output_devices().ok() }
            .and_then(|mut it| it.find(|d| d.name().is_ok_and(|n| n == name)));
        if found.is_some() {
            return found;
        }
        if let Some(node) = super::pulse::node_for(input, name) {
            super::pulse::route_own_streams(input, node);
            return if input { host.default_input_device() } else { host.default_output_device() };
        }
        tracing::warn!(device = name, "audio device not found, using default");
    }
    if input { host.default_input_device() } else { host.default_output_device() }
}

fn pick_config(dev: &Device, input: bool) -> Option<SupportedStreamConfig> {
    let ranges: Vec<_> = if input {
        dev.supported_input_configs().ok()?.collect()
    } else {
        dev.supported_output_configs().ok()?.collect()
    };
    let ok_fmt = |f: SampleFormat| matches!(f, SampleFormat::F32 | SampleFormat::I16);
    let mut best = None;
    for r in &ranges {
        if ok_fmt(r.sample_format()) && r.min_sample_rate().0 <= RATE && r.max_sample_rate().0 >= RATE && r.channels() <= 2 {
            let pref = (r.sample_format() == SampleFormat::F32) as u8 * 2 + (r.channels() == if input { 1 } else { 2 }) as u8;
            if best.as_ref().is_none_or(|(p, _)| pref > *p) {
                best = Some((pref, (*r).with_sample_rate(SampleRate(RATE))));
            }
        }
    }
    if let Some((_, c)) = best {
        return Some(c);
    }
    let d = if input { dev.default_input_config().ok()? } else { dev.default_output_config().ok()? };
    ok_fmt(d.sample_format()).then_some(d)
}

#[derive(Clone)]
pub struct PlaybackSink {
    mixer: Arc<Mutex<Mixer>>,
}

impl PlaybackSink {
    pub fn push(&self, a: AudioIn) {
        lock(&self.mixer).push(a);
    }
    pub fn remove_user(&self, user: u64) {
        lock(&self.mixer).remove_user(user);
    }
    pub fn stats(&self) -> Vec<(u64, super::mixer::TrackStats)> {
        lock(&self.mixer).stats()
    }
    pub fn output_level_db(&self) -> f32 {
        lock(&self.mixer).output_level_db()
    }
}

pub struct AudioEngine {
    stop: Arc<AtomicBool>,
    io_stop: Option<std_mpsc::Sender<()>>,
    threads: Vec<JoinHandle<()>>,
    sink: PlaybackSink,
}

impl AudioEngine {
    pub fn start(
        control: Arc<AudioControl>,
        input: Option<String>,
        output: Option<String>,
        out_tx: mpsc::UnboundedSender<AudioOut>,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let mixer = Arc::new(Mutex::new(Mixer::default()));
        let out_queue: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let (cap_tx, cap_rx) = std_mpsc::channel::<Vec<f32>>();
        let (io_stop_tx, io_stop_rx) = std_mpsc::channel::<()>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<(Option<(u32, usize)>, Option<(u32, usize)>)>();

        let mut threads = Vec::new();
        let oq = out_queue.clone();
        let output_name = output.clone();
        threads.push(
            std::thread::Builder::new()
                .name("phoenix-audio-io".into())
                .spawn(move || {
                    let synthetic = input.as_deref() == Some(SYNTHETIC_INPUT);
                    let tone_stop = Arc::new(AtomicBool::new(false));
                    let (istream, ifmt) = if synthetic {
                        let ts = tone_stop.clone();
                        let _ = std::thread::Builder::new().name("phoenix-audio-tone".into()).spawn(move || synthetic_tone(cap_tx, ts));
                        (None, Some((RATE, 1)))
                    } else {
                        open_input(input.as_deref(), cap_tx).unzip()
                    };
                    let (ostream, ofmt) = open_output(output.as_deref(), oq).unzip();
                    let _ = ready_tx.send((ifmt, ofmt));
                    let _keep = (istream, ostream);
                    let _ = io_stop_rx.recv();
                    tone_stop.store(true, Ordering::Relaxed);
                })
                .map_err(Error::Io)?,
        );
        let (ifmt, ofmt) = ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| Error::Timeout("audio devices"))?;
        if std::env::var_os("PHOENIX_SOUND_LOG").is_some() {
            eprintln!("voice-audio: engine input={ifmt:?} output={ofmt:?} output_device={output_name:?}");
        }

        if let Some((rate, ch)) = ifmt {
            let control = control.clone();
            let stop = stop.clone();
            threads.push(
                std::thread::Builder::new()
                    .name("phoenix-audio-enc".into())
                    .spawn(move || encode_loop(control, stop, cap_rx, rate, ch, out_tx))
                    .map_err(Error::Io)?,
            );
        } else {
            tracing::warn!("no microphone: voice will be receive-only");
        }
        if let Some((rate, ch)) = ofmt {
            let control = control.clone();
            let stop = stop.clone();
            let mixer = mixer.clone();
            threads.push(
                std::thread::Builder::new()
                    .name("phoenix-audio-mix".into())
                    .spawn(move || mix_loop(control, stop, mixer, out_queue, rate, ch))
                    .map_err(Error::Io)?,
            );
        } else {
            tracing::warn!("no output device: incoming audio is discarded");
        }
        Ok(Self { stop, io_stop: Some(io_stop_tx), threads, sink: PlaybackSink { mixer } })
    }

    pub fn sink(&self) -> PlaybackSink {
        self.sink.clone()
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.io_stop.take();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub const SYNTHETIC_INPUT: &str = "phoenix-synthetic-tone";

pub fn synthetic_frame(start: usize, len: usize) -> Vec<f32> {
    (start..start + len).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin() * 0.03).collect()
}

fn synthetic_tone(tx: std_mpsc::Sender<Vec<f32>>, stop: Arc<AtomicBool>) {
    let mut n = 0usize;
    let mut next = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if tx.send(synthetic_frame(n, FRAME_SAMPLES)).is_err() {
            return;
        }
        n += FRAME_SAMPLES;
        next += Duration::from_millis(20);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
    }
}

pub(crate) fn open_input(name: Option<&str>, tx: std_mpsc::Sender<Vec<f32>>) -> Option<(Stream, (u32, usize))> {
    let dev = find_device(true, name)?;
    let sc = pick_config(&dev, true)?;
    let fmt = sc.sample_format();
    let cfg: StreamConfig = sc.into();
    let ch = cfg.channels as usize;
    let err = |e| tracing::warn!(error = %e, "audio input stream error");
    let stream = match fmt {
        SampleFormat::F32 => dev.build_input_stream(
            &cfg,
            move |d: &[f32], _: &_| {
                let _ = tx.send(convert_channels(d, ch, 1));
            },
            err,
            None,
        ),
        _ => dev.build_input_stream(
            &cfg,
            move |d: &[i16], _: &_| {
                let f: Vec<f32> = d.iter().map(|s| *s as f32 / 32768.0).collect();
                let _ = tx.send(convert_channels(&f, ch, 1));
            },
            err,
            None,
        ),
    };
    let stream = stream.map_err(|e| tracing::warn!(error = %e, "open microphone")).ok()?;
    stream.play().map_err(|e| tracing::warn!(error = %e, "start microphone")).ok()?;
    Some((stream, (cfg.sample_rate.0, ch)))
}

fn open_output(name: Option<&str>, q: Arc<Mutex<VecDeque<f32>>>) -> Option<(Stream, (u32, usize))> {
    let dev = find_device(false, name)?;
    let sc = pick_config(&dev, false)?;
    let fmt = sc.sample_format();
    let cfg: StreamConfig = sc.into();
    let ch = cfg.channels as usize;
    let err = |e| tracing::warn!(error = %e, "audio output stream error");
    let stream = match fmt {
        SampleFormat::F32 => dev.build_output_stream(
            &cfg,
            move |d: &mut [f32], _: &_| {
                let mut q = lock(&q);
                for s in d.iter_mut() {
                    *s = q.pop_front().unwrap_or(0.0);
                }
            },
            err,
            None,
        ),
        _ => dev.build_output_stream(
            &cfg,
            move |d: &mut [i16], _: &_| {
                let mut q = lock(&q);
                for s in d.iter_mut() {
                    *s = (q.pop_front().unwrap_or(0.0).clamp(-1.0, 1.0) * 32767.0) as i16;
                }
            },
            err,
            None,
        ),
    };
    let stream = stream.map_err(|e| tracing::warn!(error = %e, "open speakers")).ok()?;
    stream.play().map_err(|e| tracing::warn!(error = %e, "start speakers")).ok()?;
    Some((stream, (cfg.sample_rate.0, ch)))
}

pub struct Processor {
    hp: HighPass,
    ns: NoiseSuppressor,
    agc: Agc,
    gate: NoiseGate,
    ptt_last: Option<Instant>,
}

impl Default for Processor {
    fn default() -> Self {
        Self { hp: HighPass::new(80.0, RATE), ns: NoiseSuppressor::default(), agc: Agc::default(), gate: NoiseGate::default(), ptt_last: None }
    }
}

impl Processor {
    pub fn process(&mut self, control: &AudioControl, frame: &mut [f32]) -> (f32, bool) {
        if control.noise_suppression() {
            self.hp.process(frame);
            self.ns.process(frame);
        }
        let g = control.input_gain();
        if g != 1.0 {
            frame.iter_mut().for_each(|s| *s *= g);
        }
        let db = level_db(frame);
        self.gate.automatic = control.gate_automatic();
        self.gate.threshold_db = control.gate_threshold_db();
        let vad = self.gate.process(db);
        let open = if control.push_to_talk() {
            let now = Instant::now();
            if control.ptt_pressed() {
                self.ptt_last = Some(now);
                true
            } else {
                self.ptt_last.is_some_and(|t| now.duration_since(t) <= Duration::from_millis(control.ptt_release_ms() as u64))
            }
        } else {
            vad
        };
        if control.agc() {
            self.agc.process(frame, open && vad);
        }
        (db, open)
    }
}

fn encode_loop(
    control: Arc<AudioControl>,
    stop: Arc<AtomicBool>,
    rx: std_mpsc::Receiver<Vec<f32>>,
    rate: u32,
    _ch: usize,
    out: mpsc::UnboundedSender<AudioOut>,
) {
    let Ok(mut enc) = OpusEncoder::new(false, 64_000) else {
        tracing::warn!("opus encoder unavailable");
        return;
    };
    if let Err(e) = enc.enable_fec(10) {
        tracing::debug!(error = %e, "opus fec");
    }
    let mut rs = Resampler::new(rate, RATE, 1);
    let mut proc = Processor::default();
    let mut pending: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES * 4);
    while !stop.load(Ordering::Relaxed) {
        let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
        rs.process(&chunk, &mut pending);
        while pending.len() >= FRAME_SAMPLES {
            let mut frame: Vec<f32> = pending.drain(..FRAME_SAMPLES).collect();
            let (db, open) = proc.process(&control, &mut frame);
            control.set_input_level_db(db);
            let send = open && !control.muted() && !control.deafened();
            control.set_transmitting(send);
            let msg = if !send {
                AudioOut::End
            } else {
                match enc.encode(&to_i16(&frame)) {
                    Ok(p) => AudioOut::Frame(p),
                    Err(_) => AudioOut::End,
                }
            };
            if out.send(msg).is_err() {
                control.set_transmitting(false);
                return;
            }
        }
    }
    control.set_transmitting(false);
}

pub struct MicTest {
    stop: Arc<AtomicBool>,
    io_stop: Option<std_mpsc::Sender<()>>,
    threads: Vec<JoinHandle<()>>,
    level: Arc<std::sync::atomic::AtomicU32>,
    open: Arc<AtomicBool>,
}

impl MicTest {
    pub fn start(control: Arc<AudioControl>, input: Option<String>, output: Option<String>, loopback: bool) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let out_queue: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let (cap_tx, cap_rx) = std_mpsc::channel::<Vec<f32>>();
        let (io_stop_tx, io_stop_rx) = std_mpsc::channel::<()>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<(Option<(u32, usize)>, Option<(u32, usize)>)>();
        let oq = out_queue.clone();
        let mut threads = vec![
            std::thread::Builder::new()
                .name("phoenix-mictest-io".into())
                .spawn(move || {
                    let (istream, ifmt) = open_input(input.as_deref(), cap_tx).unzip();
                    let (ostream, ofmt) = if loopback { open_output(output.as_deref(), oq).unzip() } else { (None, None) };
                    let _ = ready_tx.send((ifmt, ofmt));
                    let _keep = (istream, ostream);
                    let _ = io_stop_rx.recv();
                })
                .map_err(Error::Io)?,
        ];
        let (ifmt, ofmt) = ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| Error::Timeout("audio devices"))?;
        let Some((irate, _)) = ifmt else {
            drop(io_stop_tx);
            for t in threads {
                let _ = t.join();
            }
            return Err(Error::Unsupported("no microphone".into()));
        };
        let level = Arc::new(std::sync::atomic::AtomicU32::new((-100.0f32).to_bits()));
        let open = Arc::new(AtomicBool::new(false));
        let (l2, o2, s2) = (level.clone(), open.clone(), stop.clone());
        threads.push(
            std::thread::Builder::new()
                .name("phoenix-mictest".into())
                .spawn(move || {
                    let mut rs_in = Resampler::new(irate, RATE, 1);
                    let mut rs_out = ofmt.map(|(r, ch)| (Resampler::new(RATE, r, ch), ch, r as usize * ch * MAX_OUT_MS / 1000));
                    let mut proc = Processor::default();
                    let mut pending = Vec::with_capacity(FRAME_SAMPLES * 4);
                    let mut buf = Vec::new();
                    while !s2.load(Ordering::Relaxed) {
                        let Ok(chunk) = cap_rx.recv_timeout(Duration::from_millis(200)) else { continue };
                        rs_in.process(&chunk, &mut pending);
                        while pending.len() >= FRAME_SAMPLES {
                            let mut frame: Vec<f32> = pending.drain(..FRAME_SAMPLES).collect();
                            let (db, gate_open) = proc.process(&control, &mut frame);
                            l2.store(db.to_bits(), Ordering::Relaxed);
                            o2.store(gate_open, Ordering::Relaxed);
                            if let Some((rs, ch, cap)) = rs_out.as_mut() {
                                let g = control.output_gain();
                                if !gate_open {
                                    frame.iter_mut().for_each(|s| *s = 0.0);
                                }
                                let f: Vec<f32> = frame.iter().map(|s| s * g).collect();
                                let f = convert_channels(&f, 1, *ch);
                                buf.clear();
                                rs.process(&f, &mut buf);
                                let mut q = lock(&out_queue);
                                if q.len() + buf.len() <= *cap {
                                    q.extend(buf.iter().copied());
                                }
                            }
                        }
                    }
                })
                .map_err(Error::Io)?,
        );
        Ok(Self { stop, io_stop: Some(io_stop_tx), threads, level, open })
    }

    pub fn level_db(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    pub fn gate_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.io_stop.take();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for MicTest {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn mix_loop(
    control: Arc<AudioControl>,
    stop: Arc<AtomicBool>,
    mixer: Arc<Mutex<Mixer>>,
    q: Arc<Mutex<VecDeque<f32>>>,
    rate: u32,
    ch: usize,
) {
    let mut rs = Resampler::new(RATE, rate, ch);
    let cap = rate as usize * ch * MAX_OUT_MS / 1000;
    let target = rate as usize * ch * OUT_TARGET_MS / 1000;
    let log = std::env::var_os("PHOENIX_SOUND_LOG").is_some();
    let mut last_log = Instant::now();
    let mut underruns = 0u64;
    let mut buf = Vec::with_capacity(FRAME_SAMPLES * 4);
    while !stop.load(Ordering::Relaxed) {
        let queued = lock(&q).len();
        if queued == 0 {
            underruns += 1;
        }
        let mut pending = queued;
        while pending < target && !stop.load(Ordering::Relaxed) {
            let frame = lock(&mixer).mix_frame(&control);
            let frame = convert_channels(&frame, 2, ch);
            buf.clear();
            rs.process(&frame, &mut buf);
            let mut q = lock(&q);
            if q.len() + buf.len() <= cap {
                q.extend(buf.iter().copied());
            }
            pending = q.len();
            if buf.is_empty() {
                break;
            }
        }
        if log && last_log.elapsed() >= Duration::from_secs(1) {
            last_log = Instant::now();
            let mut m = lock(&mixer);
            for (user, s) in m.stats() {
                eprintln!(
                    "voice-recv: user={user} ssrc={} received={} decoded={} fec={} plc={} late={} dropped={} resets={} level_db={:.1} peak_db={:.1}",
                    s.ssrc, s.received, s.decoded, s.recovered, s.concealed, s.late, s.dropped, s.resets, s.level_db, s.peak_db
                );
            }
            eprintln!(
                "voice-recv: mixer out_db={:.1} peak_db={:.1} queue_ms={} underruns={underruns} deafened={} output_gain={:.2}",
                m.output_level_db(),
                m.output_peak_db(),
                pending * 1000 / (rate as usize * ch).max(1),
                control.deafened(),
                control.output_gain()
            );
            m.reset_peaks();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
